//! Introspecção do catálogo do Postgres: tabelas, views, colunas, tipos, PKs,
//! FKs, funções e privilégios das roles da API.
//!
//! O catálogo é a fonte de verdade dos identificadores: nenhuma tabela, coluna
//! ou função vinda da URL chega ao SQL sem existir aqui.

use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures_util::{StreamExt, stream};
use nelcota_core::Role;
use serde::Serialize;
use tokio_postgres::{AsyncMessage, GenericClient, NoTls};

/// Canal de `NOTIFY` que dispara a recarga (`NOTIFY nelcota, 'reload schema'`).
pub const RELOAD_CHANNEL: &str = "nelcota";

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Privileges {
    pub select: bool,
    pub insert: bool,
    pub update: bool,
    pub delete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    Table,
    View,
    MaterializedView,
    ForeignTable,
}

#[derive(Clone, Debug, Serialize)]
pub struct Column {
    pub name: String,
    /// Tipo sem modificador, como o Postgres o formata (ex.: `character varying`,
    /// `integer[]`, `public.humor`). Usado em casts.
    pub type_name: String,
    /// Tipo com modificador (ex.: `character varying(80)`), para documentação.
    pub full_type: String,
    /// Categoria do tipo (`pg_type.typcategory`): N número, S texto, B bool...
    pub category: char,
    /// Tipo do elemento, se for array.
    pub element_type: Option<String>,
    pub enum_values: Vec<String>,
    pub nullable: bool,
    pub has_default: bool,
    pub generated: bool,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub foreign_schema: String,
    pub foreign_table: String,
    pub foreign_columns: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Table {
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    pub rls_enabled: bool,
    pub rls_forced: bool,
    pub comment: Option<String>,
    /// Privilégios de anon, authenticated e service_role (nessa ordem).
    pub privileges: [Privileges; 3],
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn privileges_for(&self, role: Role) -> Privileges {
        self.privileges[role_index(role)]
    }

    /// Tabela comum (não view) exposta sem RLS: qualquer role com GRANT vê
    /// todas as linhas. O painel alerta sobre isso.
    pub fn exposed_without_rls(&self) -> bool {
        self.kind == TableKind::Table
            && !self.rls_enabled
            && self.privileges[..2]
                .iter()
                .any(|p| p.select || p.insert || p.update || p.delete)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Argument {
    pub name: String,
    pub type_name: String,
    pub has_default: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Function {
    pub name: String,
    pub args: Vec<Argument>,
    pub return_type: String,
    pub returns_set: bool,
    pub returns_void: bool,
    /// `i` imutável, `s` estável, `v` volátil.
    pub volatility: char,
    pub comment: Option<String>,
    /// EXECUTE para anon, authenticated e service_role.
    pub executable: [bool; 3],
}

impl Function {
    pub fn executable_by(&self, role: Role) -> bool {
        self.executable[role_index(role)]
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Catalog {
    pub schema: String,
    pub tables: BTreeMap<String, Table>,
    /// Funções por nome (pode haver sobrecargas).
    pub functions: BTreeMap<String, Vec<Function>>,
}

fn role_index(role: Role) -> usize {
    match role {
        Role::Anon => 0,
        Role::Authenticated => 1,
        Role::ServiceRole => 2,
    }
}

/// Objetos criados por extensões ficam de fora.
const NOT_FROM_EXTENSION: &str =
    "NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.objid = c.oid AND d.deptype = 'e')";

impl Catalog {
    /// Lê o catálogo do schema exposto.
    pub async fn load(
        client: &impl GenericClient,
        schema: &str,
    ) -> Result<Self, tokio_postgres::Error> {
        let mut tables = BTreeMap::new();

        let privilege =
            |role: &str, p: &str| format!("has_table_privilege('{role}', c.oid, '{p}')");
        let mut privilege_columns = Vec::new();
        for role in ["anon", "authenticated", "service_role"] {
            for p in ["SELECT", "INSERT", "UPDATE", "DELETE"] {
                privilege_columns.push(privilege(role, p));
            }
        }
        let rows = client
            .query(
                &format!(
                    "SELECT c.relname::text, c.relkind::text, c.relrowsecurity, c.relforcerowsecurity,
                            obj_description(c.oid, 'pg_class'), {}
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE n.nspname = $1 AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
                       AND {NOT_FROM_EXTENSION}",
                    privilege_columns.join(", ")
                ),
                &[&schema],
            )
            .await?;
        for row in rows {
            let name: String = row.get(0);
            let kind = match row.get::<_, String>(1).as_str() {
                "v" => TableKind::View,
                "m" => TableKind::MaterializedView,
                "f" => TableKind::ForeignTable,
                _ => TableKind::Table,
            };
            let mut privileges = [Privileges::default(); 3];
            for (i, p) in privileges.iter_mut().enumerate() {
                let base = 5 + i * 4;
                *p = Privileges {
                    select: row.get(base),
                    insert: row.get(base + 1),
                    update: row.get(base + 2),
                    delete: row.get(base + 3),
                };
            }
            tables.insert(
                name.clone(),
                Table {
                    name,
                    kind,
                    columns: Vec::new(),
                    primary_key: Vec::new(),
                    foreign_keys: Vec::new(),
                    rls_enabled: row.get(2),
                    rls_forced: row.get(3),
                    comment: row.get(4),
                    privileges,
                },
            );
        }

        let rows = client
            .query(
                "SELECT c.relname::text, a.attname::text,
                        format_type(a.atttypid, NULL), format_type(a.atttypid, a.atttypmod),
                        t.typcategory::text,
                        CASE WHEN t.typcategory = 'A' THEN format_type(t.typelem, NULL) END,
                        ARRAY(SELECT e.enumlabel::text FROM pg_enum e
                              WHERE e.enumtypid = a.atttypid ORDER BY e.enumsortorder),
                        NOT a.attnotnull,
                        a.atthasdef OR a.attidentity <> '' OR a.attgenerated <> '',
                        a.attgenerated <> '' OR a.attidentity = 'a',
                        col_description(c.oid, a.attnum)
                 FROM pg_attribute a
                 JOIN pg_class c ON c.oid = a.attrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN pg_type t ON t.oid = a.atttypid
                 WHERE n.nspname = $1 AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
                   AND a.attnum > 0 AND NOT a.attisdropped
                 ORDER BY c.relname, a.attnum",
                &[&schema],
            )
            .await?;
        for row in rows {
            let Some(table) = tables.get_mut(&row.get::<_, String>(0)) else {
                continue;
            };
            table.columns.push(Column {
                name: row.get(1),
                type_name: row.get(2),
                full_type: row.get(3),
                category: row.get::<_, String>(4).chars().next().unwrap_or('X'),
                element_type: row.get(5),
                enum_values: row.get(6),
                nullable: row.get(7),
                has_default: row.get(8),
                generated: row.get(9),
                comment: row.get(10),
            });
        }

        let rows = client
            .query(
                "SELECT c.relname::text, con.contype::text, con.conname::text,
                        ARRAY(SELECT a.attname::text FROM unnest(con.conkey) WITH ORDINALITY k(attnum, ord)
                              JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum
                              ORDER BY k.ord),
                        fn.nspname::text, fc.relname::text,
                        ARRAY(SELECT a.attname::text FROM unnest(con.confkey) WITH ORDINALITY k(attnum, ord)
                              JOIN pg_attribute a ON a.attrelid = con.confrelid AND a.attnum = k.attnum
                              ORDER BY k.ord)
                 FROM pg_constraint con
                 JOIN pg_class c ON c.oid = con.conrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 LEFT JOIN pg_class fc ON fc.oid = con.confrelid
                 LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace
                 WHERE n.nspname = $1 AND con.contype IN ('p', 'f')
                 ORDER BY c.relname, con.conname",
                &[&schema],
            )
            .await?;
        for row in rows {
            let Some(table) = tables.get_mut(&row.get::<_, String>(0)) else {
                continue;
            };
            let columns: Vec<String> = row.get(3);
            if row.get::<_, String>(1) == "p" {
                table.primary_key = columns;
            } else {
                table.foreign_keys.push(ForeignKey {
                    name: row.get(2),
                    columns,
                    foreign_schema: row.get(4),
                    foreign_table: row.get(5),
                    foreign_columns: row.get(6),
                });
            }
        }

        let mut functions: BTreeMap<String, Vec<Function>> = BTreeMap::new();
        let rows = client
            .query(
                "SELECT p.proname::text,
                        coalesce(p.proargnames, '{}')::text[],
                        coalesce(p.proargmodes::text[], '{}'),
                        ARRAY(SELECT format_type(t, NULL) FROM unnest(p.proargtypes) t),
                        p.pronargdefaults::int4,
                        format_type(p.prorettype, NULL),
                        p.proretset,
                        p.prorettype = 'void'::regtype,
                        p.provolatile::text,
                        obj_description(p.oid, 'pg_proc'),
                        has_function_privilege('anon', p.oid, 'EXECUTE'),
                        has_function_privilege('authenticated', p.oid, 'EXECUTE'),
                        has_function_privilege('service_role', p.oid, 'EXECUTE')
                 FROM pg_proc p
                 JOIN pg_namespace n ON n.oid = p.pronamespace
                 WHERE n.nspname = $1 AND p.prokind = 'f'
                   AND p.prorettype <> 'trigger'::regtype
                   AND p.prorettype <> 'event_trigger'::regtype
                   AND NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.objid = p.oid AND d.deptype = 'e')",
                &[&schema],
            )
            .await?;
        'functions: for row in rows {
            let names: Vec<String> = row.get(1);
            let modes: Vec<String> = row.get(2);
            let types: Vec<String> = row.get(3);
            let defaults = usize::try_from(row.get::<_, i32>(4)).unwrap_or(0);
            // Nomes dos argumentos de entrada (modos i, b, v; sem modos = todos IN).
            let in_names: Vec<&String> = if modes.is_empty() {
                names.iter().collect()
            } else {
                names
                    .iter()
                    .zip(&modes)
                    .filter(|(_, m)| matches!(m.as_str(), "i" | "b" | "v"))
                    .map(|(n, _)| n)
                    .collect()
            };
            let mut args = Vec::with_capacity(types.len());
            let first_default = types.len().saturating_sub(defaults);
            for (i, type_name) in types.into_iter().enumerate() {
                // Só funções com argumentos nomeados são chamáveis por JSON.
                let Some(name) = in_names.get(i).filter(|n| !n.is_empty()) else {
                    continue 'functions;
                };
                args.push(Argument {
                    name: (*name).clone(),
                    type_name,
                    has_default: i >= first_default,
                });
            }
            let function = Function {
                name: row.get(0),
                args,
                return_type: row.get(5),
                returns_set: row.get(6),
                returns_void: row.get(7),
                volatility: row.get::<_, String>(8).chars().next().unwrap_or('v'),
                comment: row.get(9),
                executable: [row.get(10), row.get(11), row.get(12)],
            };
            functions
                .entry(function.name.clone())
                .or_default()
                .push(function);
        }

        Ok(Catalog {
            schema: schema.to_owned(),
            tables,
            functions,
        })
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }
}

/// Catálogo compartilhado, trocado atomicamente a cada recarga.
pub struct CatalogHandle {
    current: RwLock<Arc<Catalog>>,
}

impl CatalogHandle {
    pub fn new(catalog: Catalog) -> Self {
        CatalogHandle {
            current: RwLock::new(Arc::new(catalog)),
        }
    }

    pub fn get(&self) -> Arc<Catalog> {
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn replace(&self, catalog: Catalog) {
        *self.current.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(catalog);
    }

    pub async fn reload(&self, pool: &deadpool_postgres::Pool) -> Result<(), String> {
        let schema = self.get().schema.clone();
        let client = pool.get().await.map_err(|e| e.to_string())?;
        let catalog = Catalog::load(&**client, &schema)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(
            tabelas = catalog.tables.len(),
            funcoes = catalog.functions.len(),
            "catálogo recarregado"
        );
        self.replace(catalog);
        Ok(())
    }
}

/// Mantém um `LISTEN nelcota` e recarrega o catálogo a cada notificação.
/// Reconecta sozinho; a cada reconexão recarrega (pode ter perdido avisos).
pub fn spawn_reload_listener(
    handle: Arc<CatalogHandle>,
    pool: deadpool_postgres::Pool,
    config: tokio_postgres::Config,
) {
    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            match listen_once(&handle, &pool, &config).await {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(err) => {
                    tracing::warn!(error = %err, "listener de recarga do catálogo caiu; reconectando");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    });
}

async fn listen_once(
    handle: &CatalogHandle,
    pool: &deadpool_postgres::Pool,
    config: &tokio_postgres::Config,
) -> Result<(), tokio_postgres::Error> {
    let (client, mut connection) = config.connect(NoTls).await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let driver = tokio::spawn(async move {
        let mut messages = stream::poll_fn(move |cx| connection.poll_message(cx));
        while let Some(message) = messages.next().await {
            match message {
                Ok(AsyncMessage::Notification(_)) => {
                    let _ = tx.send(());
                }
                Ok(_) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    });
    client
        .batch_execute(&format!("LISTEN {RELOAD_CHANNEL}"))
        .await?;
    if let Err(err) = handle.reload(pool).await {
        tracing::error!(error = %err, "falha ao recarregar o catálogo");
    }
    while rx.recv().await.is_some() {
        // Agrupa rajadas de DDL (ex.: uma migração inteira) numa recarga só.
        tokio::time::sleep(Duration::from_millis(100)).await;
        while rx.try_recv().is_ok() {}
        if let Err(err) = handle.reload(pool).await {
            tracing::error!(error = %err, "falha ao recarregar o catálogo");
        }
    }
    drop(client);
    match driver.await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}
