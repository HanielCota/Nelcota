//! Postgres catalog introspection: tables, views, columns, types, PKs, FKs,
//! functions and the privileges of the API roles.
//!
//! The catalog is the source of truth for identifiers: no table, column or
//! function coming from the URL reaches SQL unless it exists here.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures_util::{StreamExt, stream};
use nelcota_core::Role;
use serde::Serialize;
use tokio_postgres::{AsyncMessage, GenericClient, NoTls};

/// `NOTIFY` channel that triggers a reload (`NOTIFY nelcota, 'reload schema'`).
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
    /// Type without modifier, as Postgres formats it (e.g. `character varying`,
    /// `integer[]`, `public.mood`). Used in casts.
    pub type_name: String,
    /// Type with modifier (e.g. `character varying(80)`), for documentation.
    pub full_type: String,
    /// Type category (`pg_type.typcategory`): N number, S text, B bool...
    pub category: char,
    /// Element type, for arrays.
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
    /// Privileges of anon, authenticated and service_role (in that order).
    pub privileges: [Privileges; 3],
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn privileges_for(&self, role: Role) -> Privileges {
        self.privileges[role_index(role)]
    }

    /// Plain table (not a view) exposed without RLS: any role with a GRANT sees
    /// every row. The panel warns about it.
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
    /// `i` immutable, `s` stable, `v` volatile.
    pub volatility: char,
    pub comment: Option<String>,
    /// EXECUTE for anon, authenticated and service_role.
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
    /// Functions by name (there may be overloads).
    pub functions: BTreeMap<String, Vec<Function>>,
}

fn role_index(role: Role) -> usize {
    match role {
        Role::Anon => 0,
        Role::Authenticated => 1,
        Role::ServiceRole => 2,
    }
}

/// Objects created by extensions are left out.
const NOT_FROM_EXTENSION: &str =
    "NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.objid = c.oid AND d.deptype = 'e')";

impl Catalog {
    /// Reads the catalog of the exposed schema.
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
            // Names of the input arguments (modes i, b, v; no modes = all IN).
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
                // Only functions with named arguments are callable through JSON.
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

/// Shared catalog, swapped atomically on each reload.
pub struct CatalogHandle {
    current: RwLock<Arc<Catalog>>,
    reload_lock: tokio::sync::Mutex<()>,
    retrying: AtomicBool,
}

impl CatalogHandle {
    pub fn new(catalog: Catalog) -> Self {
        CatalogHandle {
            current: RwLock::new(Arc::new(catalog)),
            reload_lock: tokio::sync::Mutex::new(()),
            retrying: AtomicBool::new(false),
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
        let _guard = self.reload_lock.lock().await;
        self.reload_locked(pool).await
    }

    async fn reload_locked(&self, pool: &deadpool_postgres::Pool) -> Result<(), String> {
        let schema = self.get().schema.clone();
        let client = pool.get().await.map_err(|e| e.to_string())?;
        let catalog = Catalog::load(&**client, &schema)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(
            tables = catalog.tables.len(),
            functions = catalog.functions.len(),
            "catalog reloaded"
        );
        self.replace(catalog);
        Ok(())
    }

    /// A committed change stays successful when introspection fails. A single
    /// worker reconciles the catalog without executing the mutation again.
    pub async fn refresh(self: &Arc<Self>, pool: &deadpool_postgres::Pool) -> bool {
        let _guard = self.reload_lock.lock().await;
        if self.reload_locked(pool).await.is_ok() {
            return true;
        }
        tracing::warn!("catalog refresh pending; scheduling reconciliation");
        if !self.retrying.swap(true, Ordering::AcqRel) {
            let handle = self.clone();
            let pool = pool.clone();
            tokio::spawn(async move {
                let mut delay = Duration::from_secs(1);
                loop {
                    tokio::time::sleep(delay).await;
                    // Reset the worker flag under the reload lock. A concurrent
                    // failed refresh must not lose its request to start a worker.
                    let _guard = handle.reload_lock.lock().await;
                    match handle.reload_locked(&pool).await {
                        Ok(()) => {
                            handle.retrying.store(false, Ordering::Release);
                            break;
                        }
                        Err(error) => {
                            tracing::warn!(%error, "catalog reconciliation failed; retrying")
                        }
                    }
                    delay = (delay * 2).min(Duration::from_secs(30));
                }
            });
        }
        false
    }
}

/// Keeps a `LISTEN nelcota` open and reloads the catalog on each notification.
/// Reconnects on its own and reloads on each reconnection (notices may have
/// been missed).
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
                    tracing::warn!(error = %err, "catalog reload listener dropped; reconnecting");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    });
}

async fn listen_once(
    handle: &Arc<CatalogHandle>,
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
    handle.refresh(pool).await;
    while rx.recv().await.is_some() {
        // Groups bursts of DDL (e.g. a whole migration) into a single reload.
        tokio::time::sleep(Duration::from_millis(100)).await;
        while rx.try_recv().is_ok() {}
        handle.refresh(pool).await;
    }
    drop(client);
    match driver.await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}
