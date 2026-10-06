//! Handlers JSON do painel: visão geral, tabelas (dados e escrita), usuários,
//! policies e o schema para o autocomplete do editor SQL.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use nelcota_api::{
    Catalog,
    catalog::{Table, TableKind},
    query,
};
use serde::Deserialize;
use serde_json::{Map, Value, json, value::RawValue};

use crate::{AdminState, ApiError};

type ApiResult = Result<Json<Value>, ApiError>;
type Row = HashMap<String, Box<RawValue>>;

const MAX_PAGE_SIZE: i64 = 500;

/// Texto de um valor JSON vindo do Postgres, sem passar por `f64` (numeric
/// mantém todas as casas). `None` = NULL.
fn raw_text(value: Option<&RawValue>) -> Option<String> {
    let raw = value?.get();
    if raw == "null" {
        None
    } else if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).ok()
    } else {
        Some(raw.to_owned())
    }
}

fn table_or_404(state: &AdminState, name: &str) -> Result<Table, ApiError> {
    state
        .catalog
        .get()
        .table(name)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("tabela '{name}' não existe no schema exposto")))
}

async fn policy_counts(
    state: &AdminState,
    schema: &str,
) -> Result<HashMap<String, usize>, ApiError> {
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT tablename::text, count(*) FROM pg_policies WHERE schemaname = $1 GROUP BY 1",
            &[&schema],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get(0), usize::try_from(r.get::<_, i64>(1)).unwrap_or(0)))
        .collect())
}

/// Estado do RLS: `ok` (com policies), `warn` (sem policies), `danger`
/// (exposta sem RLS), `none` (sem RLS e sem GRANT) ou `view`.
fn rls_json(table: &Table, policies: usize) -> Value {
    let (state, label) = if table.kind != TableKind::Table {
        ("view", "view".to_owned())
    } else {
        match (table.rls_enabled, policies) {
            (false, _) if table.exposed_without_rls() => ("danger", "sem RLS".to_owned()),
            (false, _) => ("none", "sem RLS (sem GRANT)".to_owned()),
            (true, 0) => ("warn", "RLS sem policies".to_owned()),
            (true, n) => ("ok", format!("RLS · {n} policies")),
        }
    };
    json!({
        "state": state,
        "label": label,
        "enabled": table.rls_enabled,
        "forced": table.rls_forced,
        "policies": policies,
    })
}

fn grants(table: &Table, role: usize) -> Vec<&'static str> {
    let p = table.privileges[role];
    [
        (p.select, "SELECT"),
        (p.insert, "INSERT"),
        (p.update, "UPDATE"),
        (p.delete, "DELETE"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| name)
    .collect()
}

fn kind(table: &Table) -> &'static str {
    match table.kind {
        TableKind::Table => "table",
        TableKind::View => "view",
        TableKind::MaterializedView => "materialized_view",
        TableKind::ForeignTable => "foreign_table",
    }
}

fn exposed(catalog: &Catalog) -> Vec<&str> {
    catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .map(|t| t.name.as_str())
        .collect()
}

/// Contagem exata para tabelas pequenas (ou nunca analisadas); estimativa do
/// planejador nas grandes, para não custar um seq scan.
async fn row_count(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &Table,
    estimate: i64,
) -> (Option<i64>, bool) {
    if table.kind == TableKind::Table && estimate < 10_000 {
        let sql = format!(
            "SELECT count(*) FROM {}.{}",
            query::ident(schema),
            query::ident(&table.name)
        );
        if let Ok(row) = client.query_one(sql.as_str(), &[]).await {
            return (Some(row.get(0)), true);
        }
    }
    ((estimate >= 0).then_some(estimate), false)
}

async fn estimates(state: &AdminState, schema: &str) -> Result<HashMap<String, i64>, ApiError> {
    let client = state.db.get().await?;
    Ok(client
        .query(
            "SELECT c.relname::text, c.reltuples::int8 FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1",
            &[&schema],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect())
}

pub async fn overview(State(state): State<AdminState>) -> ApiResult {
    let catalog = state.catalog.get();
    let policies = policy_counts(&state, &catalog.schema).await?;
    let estimates = estimates(&state, &catalog.schema).await?;
    let client = state.db.get().await?;
    let users: i64 = client
        .query_one("SELECT count(*) FROM auth.users", &[])
        .await?
        .get(0);

    let mut tables = Vec::new();
    for table in catalog.tables.values() {
        let estimate = estimates.get(&table.name).copied().unwrap_or(-1);
        let (rows, exact) = row_count(&client, &catalog.schema, table, estimate).await;
        tables.push(json!({
            "name": table.name,
            "kind": kind(table),
            "comment": table.comment,
            "rows": rows,
            "rows_exact": exact,
            "rls": rls_json(table, policies.get(&table.name).copied().unwrap_or(0)),
            "grants": { "anon": grants(table, 0), "authenticated": grants(table, 1) },
        }));
    }
    Ok(Json(json!({
        "schema": catalog.schema,
        "counts": {
            "tables": catalog.tables.len(),
            "users": users,
            "policies": policies.values().sum::<usize>(),
            "functions": catalog.functions.values().map(Vec::len).sum::<usize>(),
        },
        "exposed_without_rls": exposed(&catalog),
        "tables": tables,
    })))
}

pub async fn tables(State(state): State<AdminState>) -> ApiResult {
    let catalog = state.catalog.get();
    let policies = policy_counts(&state, &catalog.schema).await?;
    let list: Vec<Value> = catalog
        .tables
        .values()
        .map(|t| {
            json!({
                "name": t.name,
                "kind": kind(t),
                "has_pk": !t.primary_key.is_empty(),
                "rls": rls_json(t, policies.get(&t.name).copied().unwrap_or(0)),
            })
        })
        .collect();
    Ok(Json(json!({ "schema": catalog.schema, "tables": list })))
}

/// Tabelas e colunas para o autocomplete do CodeMirror (`schema.tabela`).
pub async fn schema(State(state): State<AdminState>) -> ApiResult {
    let catalog = state.catalog.get();
    let mut tables = Map::new();
    for table in catalog.tables.values() {
        let columns: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        tables.insert(table.name.clone(), json!(columns));
    }
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT table_schema || '.' || table_name, array_agg(column_name::text ORDER BY ordinal_position)
             FROM information_schema.columns WHERE table_schema = 'auth'
             GROUP BY 1",
            &[],
        )
        .await?;
    for row in rows {
        tables.insert(row.get(0), json!(row.get::<_, Vec<String>>(1)));
    }
    Ok(Json(json!({ "schema": catalog.schema, "tables": tables })))
}

#[derive(Deserialize)]
pub struct TableQuery {
    #[serde(default)]
    page: i64,
    size: Option<i64>,
    sort: Option<String>,
    #[serde(default)]
    desc: bool,
}

pub async fn table(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Query(params): Query<TableQuery>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let catalog = state.catalog.get();
    let page = params.page.max(0);
    let size = params.size.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let mut pairs = vec![
        ("limit".to_owned(), (size + 1).to_string()),
        ("offset".to_owned(), (page * size).to_string()),
    ];
    // Ordenação: a coluna pedida (validada pelo parser contra o catálogo) ou a PK.
    let order = match &params.sort {
        Some(col) => Some(format!(
            "{col}.{}",
            if params.desc { "desc" } else { "asc" }
        )),
        None if !table.primary_key.is_empty() => Some(table.primary_key.join(",")),
        None => None,
    };
    if let Some(order) = order {
        pairs.push(("order".into(), order));
    }
    let request =
        query::parse_request(&pairs, &table).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let sql = query::select(&catalog.schema, &table, &request, None);
    let client = state.db.get().await?;
    let row = client
        .query_one(sql.text.as_str(), &sql.param_refs())
        .await?;
    let raw_rows: Vec<Row> = serde_json::from_str(&row.get::<_, String>(0))?;
    let has_next = raw_rows.len() as i64 > size;
    let rows: Vec<Value> = raw_rows
        .iter()
        .take(size as usize)
        .map(|r| {
            let map: Map<String, Value> = table
                .columns
                .iter()
                .map(|c| {
                    let text = raw_text(r.get(&c.name).map(AsRef::as_ref));
                    (c.name.clone(), text.map_or(Value::Null, Value::String))
                })
                .collect();
            Value::Object(map)
        })
        .collect();

    let estimate = estimates(&state, &catalog.schema)
        .await?
        .get(&table.name)
        .copied()
        .unwrap_or(-1);
    let (total, exact) = row_count(&client, &catalog.schema, &table, estimate).await;
    let policies = policy_counts(&state, &catalog.schema).await?;
    let columns: Vec<Value> = table
        .columns
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "type": c.type_name,
                "full_type": c.full_type,
                "category": c.category.to_string(),
                "nullable": c.nullable,
                "has_default": c.has_default,
                "generated": c.generated,
                "enum_values": c.enum_values,
                "is_pk": table.primary_key.contains(&c.name),
                "comment": c.comment,
            })
        })
        .collect();

    Ok(Json(json!({
        "table": {
            "name": table.name,
            "kind": kind(&table),
            "comment": table.comment,
            "primary_key": table.primary_key,
            "editable": !table.primary_key.is_empty() && table.kind != TableKind::MaterializedView,
            "insertable": table.kind != TableKind::MaterializedView,
            "exposed_without_rls": table.exposed_without_rls(),
            "rls": rls_json(&table, policies.get(&table.name).copied().unwrap_or(0)),
            "columns": columns,
        },
        "rows": rows,
        "page": page,
        "size": size,
        "has_next": has_next,
        "total": total,
        "total_exact": exact,
    })))
}

/// Converte os valores vindos do formulário (texto ou null) para JSON:
/// json/jsonb são interpretados como JSON; o resto vai como texto e o
/// Postgres converte para o tipo da coluna.
fn to_values(table: &Table, values: Map<String, Value>) -> Result<Map<String, Value>, ApiError> {
    let mut out = Map::new();
    for (key, value) in values {
        let Some(col) = table.column(&key).filter(|c| !c.generated) else {
            return Err(ApiError::bad_request(format!(
                "coluna desconhecida ou gerada: {key}"
            )));
        };
        let json = match value {
            Value::Null => Value::Null,
            Value::String(s) if matches!(col.type_name.as_str(), "json" | "jsonb") => {
                serde_json::from_str(&s).map_err(|e| {
                    ApiError::bad_request(format!("{}: JSON inválido ({e})", col.name))
                })?
            }
            Value::String(s) => Value::String(s),
            other => Value::String(other.to_string()),
        };
        out.insert(col.name.clone(), json);
    }
    Ok(out)
}

/// Pares `coluna=eq.valor` da chave primária.
fn pk_filters(table: &Table, pk: &Map<String, Value>) -> Result<Vec<query::Filter>, ApiError> {
    if table.primary_key.is_empty() {
        return Err(ApiError::bad_request(
            "tabela sem chave primária: edição pelo painel indisponível (use o editor SQL)",
        ));
    }
    let pairs: Vec<(String, String)> = table
        .primary_key
        .iter()
        .map(|col| {
            let value = match pk.get(col) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => {
                    return Err(ApiError::bad_request(format!(
                        "valor da chave '{col}' ausente"
                    )));
                }
                Some(other) => other.to_string(),
            };
            Ok((col.clone(), format!("eq.{value}")))
        })
        .collect::<Result<_, _>>()?;
    Ok(query::parse_request(&pairs, table)
        .map_err(|e| ApiError::bad_request(e.to_string()))?
        .filters)
}

/// Executa uma escrita e devolve a mensagem de erro do Postgres como 400.
async fn execute(state: &AdminState, sql: &query::Sql) -> Result<u64, ApiError> {
    let client = state.db.get().await?;
    client
        .execute(sql.text.as_str(), &sql.param_refs())
        .await
        .map_err(|e| {
            ApiError::bad_request(
                e.as_db_error()
                    .map_or_else(|| e.to_string(), |db| db.message().to_owned()),
            )
        })
}

#[derive(Deserialize)]
pub struct InsertRequest {
    values: Map<String, Value>,
}

pub async fn insert_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Json(body): Json<InsertRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let values = to_values(&table, body.values)?;
    let sql = query::insert(&schema, &table, Value::Object(values), None)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    execute(&state, &sql).await?;
    Ok(Json(json!({ "message": "linha inserida" })))
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pk: Map<String, Value>,
    values: Map<String, Value>,
}

pub async fn update_row(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let filters = pk_filters(&table, &body.pk)?;
    let values = to_values(&table, body.values)?;
    let sql = query::update(&schema, &table, Value::Object(values), &filters, None)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let n = execute(&state, &sql).await?;
    Ok(Json(
        json!({ "message": format!("{n} linha(s) atualizada(s)"), "count": n }),
    ))
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pks: Vec<Map<String, Value>>,
}

/// Apaga várias linhas (seleção na grade) numa transação só.
pub async fn delete_rows(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Json(body): Json<DeleteRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    if body.pks.is_empty() {
        return Err(ApiError::bad_request("nenhuma linha selecionada"));
    }
    let statements: Vec<query::Sql> = body
        .pks
        .iter()
        .map(|pk| {
            Ok(query::delete(
                &schema,
                &table,
                &pk_filters(&table, pk)?,
                None,
            ))
        })
        .collect::<Result<_, ApiError>>()?;
    let mut client = state.db.get().await?;
    let tx = client.transaction().await?;
    let mut total = 0;
    for sql in &statements {
        total += tx
            .execute(sql.text.as_str(), &sql.param_refs())
            .await
            .map_err(|e| {
                ApiError::bad_request(
                    e.as_db_error()
                        .map_or_else(|| e.to_string(), |db| db.message().to_owned()),
                )
            })?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({ "message": format!("{total} linha(s) apagada(s)"), "count": total }),
    ))
}

#[derive(Deserialize)]
pub struct UsersQuery {
    #[serde(default)]
    page: i64,
    q: Option<String>,
}

pub async fn users(State(state): State<AdminState>, Query(params): Query<UsersQuery>) -> ApiResult {
    const SIZE: i64 = 50;
    let page = params.page.max(0);
    let search = params.q.unwrap_or_default();
    let pattern = format!(
        "%{}%",
        search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT u.id::text, u.email, u.created_at::text, u.last_sign_in_at::text,
                    u.email_confirmed_at::text,
                    (SELECT count(*) FROM auth.sessions s WHERE s.user_id = u.id AND s.revoked_at IS NULL)
             FROM auth.users u WHERE u.email LIKE $1
             ORDER BY u.created_at DESC LIMIT $2 OFFSET $3",
            &[&pattern, &(SIZE + 1), &(page * SIZE)],
        )
        .await?;
    let total: i64 = client
        .query_one("SELECT count(*) FROM auth.users", &[])
        .await?
        .get(0);
    let users: Vec<Value> = rows
        .iter()
        .take(SIZE as usize)
        .map(|r| {
            json!({
                "id": r.get::<_, String>(0),
                "email": r.get::<_, String>(1),
                "created_at": r.get::<_, String>(2),
                "last_sign_in_at": r.get::<_, Option<String>>(3),
                "email_confirmed_at": r.get::<_, Option<String>>(4),
                "sessions": r.get::<_, i64>(5),
            })
        })
        .collect();
    Ok(Json(json!({
        "total": total,
        "page": page,
        "has_next": rows.len() as i64 > SIZE,
        "users": users,
    })))
}

/// Validação de formato de uuid (8-4-4-4-12 hex).
fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub async fn revoke_sessions(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(ApiError::bad_request("id inválido"));
    }
    let client = state.db.get().await?;
    let n = client
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    Ok(Json(
        json!({ "message": format!("{n} sessão(ões) encerrada(s)"), "count": n }),
    ))
}

pub async fn delete_user(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(ApiError::bad_request("id inválido"));
    }
    let client = state.db.get().await?;
    let n = client
        .execute("DELETE FROM auth.users WHERE id = $1::text::uuid", &[&id])
        .await?;
    if n == 0 {
        return Err(ApiError::not_found("usuário não encontrado"));
    }
    Ok(Json(json!({ "message": "usuário apagado" })))
}

pub async fn policies(State(state): State<AdminState>) -> ApiResult {
    let catalog = state.catalog.get();
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT tablename::text, policyname::text, permissive, roles::text[], cmd,
                    qual, with_check
             FROM pg_policies WHERE schemaname = $1 ORDER BY tablename, policyname",
            &[&catalog.schema],
        )
        .await?;
    let mut by_table: HashMap<String, Vec<Value>> = HashMap::new();
    for row in &rows {
        by_table.entry(row.get(0)).or_default().push(json!({
            "name": row.get::<_, String>(1),
            "permissive": row.get::<_, String>(2) == "PERMISSIVE",
            "roles": row.get::<_, Vec<String>>(3),
            "command": row.get::<_, String>(4),
            "using": row.get::<_, Option<String>>(5),
            "check": row.get::<_, Option<String>>(6),
        }));
    }
    let tables: Vec<Value> = catalog
        .tables
        .values()
        .filter(|t| t.kind == TableKind::Table)
        .map(|t| {
            let list = by_table.remove(&t.name).unwrap_or_default();
            json!({
                "name": t.name,
                "rls": rls_json(t, list.len()),
                "exposed_without_rls": t.exposed_without_rls(),
                "policies": list,
            })
        })
        .collect();
    let anon_functions: Vec<&str> = catalog
        .functions
        .values()
        .flatten()
        .filter(|f| f.executable_by(nelcota_core::Role::Anon))
        .map(|f| f.name.as_str())
        .collect();
    Ok(Json(json!({
        "schema": catalog.schema,
        "exposed_without_rls": exposed(&catalog),
        "tables": tables,
        "anon_functions": anon_functions,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valores_preservam_o_texto_do_postgres() {
        let raw = |s: &str| serde_json::from_str::<Box<RawValue>>(s).unwrap();
        assert_eq!(raw_text(Some(&raw("\"<b>\""))).as_deref(), Some("<b>"));
        assert_eq!(
            raw_text(Some(&raw("12345678901234567890.10"))).as_deref(),
            Some("12345678901234567890.10")
        );
        assert_eq!(raw_text(Some(&raw("null"))), None);
        assert_eq!(
            raw_text(Some(&raw("{\"a\": 1}"))).as_deref(),
            Some("{\"a\": 1}")
        );
    }

    #[test]
    fn valida_uuid() {
        assert!(is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b92"));
        assert!(!is_uuid("nao-e-uuid"));
        assert!(!is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b9z"));
    }
}
