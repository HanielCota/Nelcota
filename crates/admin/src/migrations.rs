//! Migrações a partir do painel (migração V6).
//!
//! O painel aplica DDL direto no banco, mas o deploy confia em `migrations/`.
//! Cada DDL do painel fica em `nelcota.panel_changes`; "Gerar migração" junta
//! os pendentes num `V<n>__<nome>.sql` e o registra em
//! `nelcota.user_migrations` exatamente como o `nelcota migrate` faria (mesmo
//! checksum do refinery). Quem coloca o arquivo em `migrations/` e faz commit
//! não roda nada duas vezes; quem o edita é barrado pelo `migrate`.

use std::{
    collections::BTreeMap,
    path::{Path as FsPath, PathBuf},
};

use axum::{
    Json,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AdminState, ApiError};

/// Tabela do `nelcota migrate` (crates/cli/src/db.rs).
const USER_MIGRATIONS: &str = "nelcota.user_migrations";

/// DDL que o refinery usa para a tabela de controle: criada aqui quando o
/// projeto ainda não rodou nenhum `migrate`.
const CREATE_USER_MIGRATIONS: &str = "CREATE TABLE IF NOT EXISTS nelcota.user_migrations(
             version INT4 PRIMARY KEY,
             name VARCHAR(255),
             applied_on VARCHAR(255),
             checksum VARCHAR(255))";

/// `V12__criar_pedidos.sql` → `(12, "criar_pedidos")`.
pub fn parse_filename(filename: &str) -> Option<(i32, String)> {
    let stem = filename.strip_suffix(".sql")?;
    let (version, name) = stem.strip_prefix('V')?.split_once("__")?;
    let version = version.parse().ok().filter(|v| *v > 0)?;
    Some((version, name.to_owned()))
}

/// Arquivos de migração da pasta do projeto (vazio se ela não existe).
fn folder_migrations(dir: Option<&FsPath>) -> BTreeMap<i32, String> {
    let Some(Ok(entries)) = dir.map(std::fs::read_dir) else {
        return BTreeMap::new();
    };
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|name| parse_filename(&name))
        .collect()
}

/// Nome do arquivo gerado: minúsculas, dígitos e `_`, como o resto das migrações.
fn valid_name(name: &str) -> bool {
    (1..=60).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with('_')
}

/// Próxima versão livre: acima de tudo que já existe no banco, na pasta ou
/// entre as geradas pelo painel (um arquivo ainda não aplicado também ocupa
/// o número).
pub fn next_version(taken: impl IntoIterator<Item = i32>) -> i32 {
    taken.into_iter().max().unwrap_or(0) + 1
}

/// Texto do arquivo. Cada alteração vira um bloco com a mensagem do painel
/// como comentário; comandos terminam em `;` para rodarem num lote só.
pub fn render(generated_at: &str, changes: &[(String, String, Vec<String>)]) -> String {
    let mut sql = format!(
        "-- Gerada pelo painel do Nelcota em {generated_at}.\n\
         -- Já consta como aplicada no banco onde foi gerada: coloque este arquivo\n\
         -- em migrations/ e faça commit. Não edite (o checksum mudaria e o\n\
         -- `nelcota migrate` recusaria).\n"
    );
    for (applied_at, summary, statements) in changes {
        sql.push_str(&format!("\n-- {summary} ({applied_at})\n"));
        for statement in statements {
            let statement = statement.trim().trim_end_matches(';');
            sql.push_str(statement);
            sql.push_str(";\n");
        }
    }
    sql
}

/// Versões aplicadas (`user_migrations` pode ainda não existir).
async fn applied(
    client: &tokio_postgres::Client,
) -> Result<BTreeMap<i32, (String, String)>, ApiError> {
    let exists: bool = client
        .query_one("SELECT to_regclass($1) IS NOT NULL", &[&USER_MIGRATIONS])
        .await?
        .get(0);
    if !exists {
        return Ok(BTreeMap::new());
    }
    let rows = client
        .query(
            "SELECT version, coalesce(name, ''), coalesce(applied_on, '') FROM nelcota.user_migrations",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect())
}

async fn exported(client: &tokio_postgres::Client) -> Result<BTreeMap<i32, String>, ApiError> {
    let rows = client
        .query(
            "SELECT version, filename FROM nelcota.panel_migrations",
            &[],
        )
        .await?;
    Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// `GET /admin/api/migrations`
pub async fn list(State(state): State<AdminState>) -> Result<Json<Value>, ApiError> {
    let client = state.db.get().await?;
    let applied = applied(&client).await?;
    let exported = exported(&client).await?;
    let dir = state.migrations_dir.as_deref();
    let folder = folder_migrations(dir);

    let versions: std::collections::BTreeSet<i32> = applied
        .keys()
        .chain(folder.keys())
        .chain(exported.keys())
        .copied()
        .collect();
    let migrations: Vec<Value> = versions
        .iter()
        .rev()
        .map(|v| {
            let name = applied
                .get(v)
                .map(|(n, _)| n.clone())
                .or_else(|| folder.get(v).cloned())
                .or_else(|| {
                    exported
                        .get(v)
                        .and_then(|f| parse_filename(f))
                        .map(|(_, n)| n)
                })
                .unwrap_or_default();
            json!({
                "version": v,
                "name": name,
                "applied_on": applied.get(v).map(|(_, at)| at),
                // `null` quando a pasta não está acessível ao servidor.
                "in_folder": dir.map(|_| folder.contains_key(v)),
                "from_panel": exported.contains_key(v),
            })
        })
        .collect();

    let pending: Vec<Value> = client
        .query(
            "SELECT id, to_char(applied_at, 'YYYY-MM-DD\"T\"HH24:MI:SSTZH:TZM'), summary, statements
               FROM nelcota.panel_changes WHERE exported_in IS NULL ORDER BY id",
            &[],
        )
        .await?
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, i64>(0),
                "applied_at": r.get::<_, String>(1),
                "summary": r.get::<_, String>(2),
                "statements": r.get::<_, Vec<String>>(3),
            })
        })
        .collect();

    Ok(Json(json!({
        "migrations": migrations,
        "pending": pending,
        "next_version": next_version(versions.iter().copied()),
        "folder": dir.map(|d| d.display().to_string()),
    })))
}

#[derive(Deserialize)]
pub struct ExportRequest {
    name: String,
}

/// `POST /admin/api/migrations` `{name}`: gera a migração com as alterações
/// pendentes e a registra como aplicada neste banco.
pub async fn export(
    State(state): State<AdminState>,
    Json(body): Json<ExportRequest>,
) -> Result<Json<Value>, ApiError> {
    let name = body.name.trim();
    if !valid_name(name) {
        return Err(ApiError::bad_request(
            "nome com letras minúsculas, números e _ (até 60), ex.: criar_pedidos",
        ));
    }
    let mut client = state.db.get().await?;
    let tx = client.transaction().await?;
    // Uma geração por vez: duas abas não disputam o mesmo número.
    tx.batch_execute("LOCK TABLE nelcota.panel_changes IN SHARE ROW EXCLUSIVE MODE")
        .await?;
    let rows = tx
        .query(
            "SELECT id, to_char(applied_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI \"UTC\"'),
                    summary, statements
               FROM nelcota.panel_changes WHERE exported_in IS NULL ORDER BY id",
            &[],
        )
        .await?;
    if rows.is_empty() {
        return Err(ApiError::conflict(
            "nenhuma alteração do painel fora das migrações",
        ));
    }
    let ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
    let changes: Vec<(String, String, Vec<String>)> = rows
        .iter()
        .map(|r| (r.get(1), r.get(2), r.get(3)))
        .collect();

    tx.batch_execute(CREATE_USER_MIGRATIONS).await?;
    let taken: Vec<i32> = tx
        .query(
            "SELECT version FROM nelcota.user_migrations
             UNION SELECT version FROM nelcota.panel_migrations",
            &[],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .chain(folder_migrations(state.migrations_dir.as_deref()).into_keys())
        .collect();
    let version = next_version(taken);

    let (generated_at, applied_on): (String, String) = {
        let row = tx
            .query_one(
                "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI \"UTC\"'),
                        to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
                &[],
            )
            .await?;
        (row.get(0), row.get(1))
    };
    let sql = render(&generated_at, &changes);
    let stem = format!("V{version}__{name}");
    let filename = format!("{stem}.sql");
    // O mesmo cálculo que o `migrate` fará ao ler o arquivo.
    let checksum = refinery::Migration::unapplied(&stem, &sql)
        .map_err(|e| ApiError::from(format!("migração inválida: {e}")))?
        .checksum()
        .to_string();

    tx.execute(
        "INSERT INTO nelcota.user_migrations (version, name, applied_on, checksum)
         VALUES ($1, $2, $3, $4)",
        &[&version, &name, &applied_on, &checksum],
    )
    .await?;
    tx.execute(
        "INSERT INTO nelcota.panel_migrations (version, filename, sql) VALUES ($1, $2, $3)",
        &[&version, &filename, &sql],
    )
    .await?;
    tx.execute(
        "UPDATE nelcota.panel_changes SET exported_in = $1 WHERE id = ANY($2)",
        &[&version, &ids],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        versao = version,
        alteracoes = ids.len(),
        "migração gerada pelo painel"
    );
    Ok(Json(json!({
        "version": version,
        "filename": filename,
        "sql": sql,
        "message": format!("{filename} gerada com {} alteração(ões)", ids.len()),
    })))
}

/// `GET /admin/api/migrations/{version}/file`: o arquivo gerado, idêntico.
pub async fn file(
    State(state): State<AdminState>,
    Path(version): Path<i32>,
) -> Result<Response, ApiError> {
    let client = state.db.get().await?;
    let row = client
        .query_opt(
            "SELECT filename, sql FROM nelcota.panel_migrations WHERE version = $1",
            &[&version],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("migração não foi gerada pelo painel"))?;
    let filename: String = row.get(0);
    let sql: String = row.get(1);
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/sql; charset=utf-8".to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        sql,
    )
        .into_response())
}

/// Pasta das migrações do projeto: a configurada ou, no container do
/// projeto, `/migrations` (o compose a monta somente leitura).
pub fn default_dir(configured: Option<PathBuf>) -> Option<PathBuf> {
    configured.or_else(|| {
        let mounted = PathBuf::from("/migrations");
        mounted.is_dir().then_some(mounted)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_nome_dos_arquivos() {
        assert_eq!(
            parse_filename("V12__criar_pedidos.sql"),
            Some((12, "criar_pedidos".into()))
        );
        assert_eq!(parse_filename("V1__a.sql"), Some((1, "a".into())));
        for bad in [
            "V0__x.sql",
            "V1_x.sql",
            "1__x.sql",
            "V1__x.txt",
            "Vx__y.sql",
            "README.md",
        ] {
            assert_eq!(parse_filename(bad), None, "{bad}");
        }
    }

    #[test]
    fn proxima_versao_passa_de_tudo_que_ja_existe() {
        assert_eq!(next_version([]), 1);
        // Banco em V3, pasta com V5 ainda não aplicada: a gerada é V6.
        assert_eq!(next_version([1, 2, 3, 5]), 6);
    }

    #[test]
    fn nome_do_arquivo() {
        assert!(valid_name("criar_pedidos"));
        assert!(valid_name("v2_indices"));
        for bad in [
            "",
            "Criar",
            "com espaço",
            "acentuação",
            "_x",
            "a-b",
            &"a".repeat(61),
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn arquivo_termina_cada_comando_com_ponto_e_virgula() {
        let sql = render(
            "2026-10-07 12:00 UTC",
            &[(
                "2026-10-07 11:58 UTC".into(),
                "tabela pedidos criada".into(),
                vec![
                    "CREATE TABLE pedidos (id int)".into(),
                    "ALTER TABLE pedidos ENABLE ROW LEVEL SECURITY;".into(),
                ],
            )],
        );
        assert!(sql.starts_with("-- Gerada pelo painel do Nelcota em 2026-10-07 12:00 UTC."));
        assert!(sql.contains("\n-- tabela pedidos criada (2026-10-07 11:58 UTC)\nCREATE TABLE pedidos (id int);\nALTER TABLE pedidos ENABLE ROW LEVEL SECURITY;\n"));
    }
}
