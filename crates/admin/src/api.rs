//! Panel JSON handlers: overview, tables (data and writes), users, policies
//! and the schema for the SQL editor's autocomplete.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use futures_util::future::{join_all, try_join, try_join3};
use nelcota_api::{
    Catalog,
    catalog::{Table, TableKind},
    query,
};
use serde::Deserialize;
use serde_json::{Map, Value, json, value::RawValue};
use tokio_postgres::Client;

use crate::{AdminState, ApiError};

type ApiResult = Result<Json<Value>, ApiError>;
pub(crate) type Row = HashMap<String, Box<RawValue>>;

const MAX_PAGE_SIZE: i64 = 500;

/// Text of a JSON value coming from Postgres, without going through `f64`
/// (numeric keeps every digit). `None` = NULL.
pub(crate) fn raw_text(value: Option<&RawValue>) -> Option<String> {
    let raw = value?.get();
    if raw == "null" {
        None
    } else if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).ok()
    } else {
        Some(raw.to_owned())
    }
}

pub(crate) fn table_or_404(state: &AdminState, name: &str) -> Result<Table, ApiError> {
    state.catalog.get().table(name).cloned().ok_or_else(|| {
        ApiError::not_found(
            "table_not_found",
            format!("table '{name}' does not exist in the exposed schema"),
        )
        .params(json!({ "table": name }))
    })
}

async fn policy_counts(client: &Client, schema: &str) -> Result<HashMap<String, usize>, ApiError> {
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

/// RLS state: `ok` (with policies), `warn` (no policies), `danger` (exposed
/// without RLS), `none` (no RLS and no GRANT) or `view`.
fn rls_json(table: &Table, policies: usize) -> Value {
    let (state, label) = if table.kind != TableKind::Table {
        ("view", "view".to_owned())
    } else {
        match (table.rls_enabled, policies) {
            (false, _) if table.exposed_without_rls() => ("danger", "no RLS".to_owned()),
            (false, _) => ("none", "no RLS (no GRANT)".to_owned()),
            (true, 0) => ("warn", "RLS without policies".to_owned()),
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

/// Exact count for small (or never analysed) tables; the planner's estimate
/// for large ones, so it never costs a seq scan.
async fn row_count(
    client: &Client,
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

async fn estimates(client: &Client, schema: &str) -> Result<HashMap<String, i64>, ApiError> {
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
    // A single connection, with the independent queries pipelined on it.
    let client = state.db.get().await?;
    let (policies, estimates, users) = try_join3(
        policy_counts(&client, &catalog.schema),
        estimates(&client, &catalog.schema),
        async {
            Ok::<_, ApiError>(
                client
                    .query_one("SELECT count(*) FROM auth.users", &[])
                    .await?,
            )
        },
    )
    .await?;
    let users: i64 = users.get(0);

    let counts = join_all(catalog.tables.values().map(|table| {
        let estimate = estimates.get(&table.name).copied().unwrap_or(-1);
        row_count(&client, &catalog.schema, table, estimate)
    }))
    .await;

    let mut tables = Vec::new();
    for (table, (rows, exact)) in catalog.tables.values().zip(counts) {
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
    let client = state.db.get().await?;
    let policies = policy_counts(&client, &catalog.schema).await?;
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

/// Tables and columns for CodeMirror's autocomplete (`schema.table`).
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

/// Filter operators the panel accepts (a subset of the REST API's).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    Ilike,
    Is,
}

impl FilterOp {
    fn as_str(self) -> &'static str {
        match self {
            FilterOp::Eq => "eq",
            FilterOp::Neq => "neq",
            FilterOp::Gt => "gt",
            FilterOp::Gte => "gte",
            FilterOp::Lt => "lt",
            FilterOp::Lte => "lte",
            FilterOp::Like => "like",
            FilterOp::Ilike => "ilike",
            FilterOp::Is => "is",
        }
    }
}

/// A grid filter: `column operator value`, optionally negated.
#[derive(Debug, Deserialize)]
pub(crate) struct FilterSpec {
    column: String,
    op: FilterOp,
    value: String,
    #[serde(default)]
    not: bool,
}

impl FilterSpec {
    /// Pair in the REST API format (`column=not.op.value`), then validated by
    /// the same parser against the catalog.
    fn to_pair(&self) -> (String, String) {
        let not = if self.not { "not." } else { "" };
        (
            self.column.clone(),
            format!("{not}{}.{}", self.op.as_str(), self.value),
        )
    }
}

/// Sort and filters shared by listing and export.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct RowsQuery {
    pub sort: Option<String>,
    #[serde(default)]
    pub desc: bool,
    /// JSON array of [`FilterSpec`] (in the query string, since it is a GET).
    pub filters: Option<String>,
}

/// Builds the validated request (columns and operators checked against the
/// catalog). `page` = `Some((page, size))` to paginate.
pub(crate) fn build_request(
    table: &Table,
    params: &RowsQuery,
    page: Option<(i64, i64)>,
) -> Result<query::Request, ApiError> {
    let filters: Vec<FilterSpec> = match params.filters.as_deref() {
        None | Some("") => Vec::new(),
        Some(json) => serde_json::from_str(json).map_err(|e| {
            ApiError::bad_request("invalid_filters", format!("invalid filters: {e}"))
                .params(json!({ "detail": e.to_string() }))
        })?,
    };
    let mut pairs: Vec<(String, String)> = filters.iter().map(FilterSpec::to_pair).collect();
    if let Some((page, size)) = page {
        // One extra row tells whether there is a next page.
        pairs.push(("limit".into(), (size + 1).to_string()));
        pairs.push(("offset".into(), (page * size).to_string()));
    }
    // Sort: the requested column (validated by the parser) or the PK.
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
    query::parse_request(&pairs, table).map_err(invalid_query)
}

/// The REST API parser refused the request (unknown column, bad operator…).
pub(crate) fn invalid_query(err: impl std::fmt::Display) -> ApiError {
    let detail = err.to_string();
    ApiError::bad_request("invalid_query", format!("invalid query: {detail}"))
        .params(json!({ "detail": detail }))
}

/// Error of a query built from what the admin typed (filter value of the
/// wrong type, constraint violation…): 400 with Postgres' text, no code.
pub(crate) fn user_query_error(err: tokio_postgres::Error) -> ApiError {
    ApiError::raw(
        axum::http::StatusCode::BAD_REQUEST,
        err.as_db_error()
            .map_or_else(|| err.to_string(), |db| db.message().to_owned()),
    )
}

/// Count with a time cap: an unindexed filter on a big table must not freeze
/// the grid. `None` when it runs past the limit.
async fn bounded_count(client: &mut Client, sql: &query::Sql) -> Option<i64> {
    let tx = client.transaction().await.ok()?;
    tx.batch_execute("SET LOCAL statement_timeout = '3s'")
        .await
        .ok()?;
    let count = tx
        .query_one(sql.text.as_str(), &sql.param_refs())
        .await
        .ok()?
        .get(0);
    tx.commit().await.ok()?;
    Some(count)
}

/// Single-column foreign key to another exposed table: the grid uses it to
/// jump to the referenced row.
fn references(catalog: &Catalog, table: &Table, column: &str) -> Value {
    table
        .foreign_keys
        .iter()
        .find(|fk| {
            fk.columns.len() == 1
                && fk.columns[0] == column
                && fk.foreign_schema == catalog.schema
                && catalog.tables.contains_key(&fk.foreign_table)
        })
        .map_or(
            Value::Null,
            |fk| json!({ "table": fk.foreign_table, "column": fk.foreign_columns[0] }),
        )
}

// `RowsQuery` fields repeated instead of `#[serde(flatten)]`: with flatten,
// serde_urlencoded hands everything over as text and `page`/`desc` stop converting.
#[derive(Deserialize)]
pub struct TableQuery {
    #[serde(default)]
    page: i64,
    size: Option<i64>,
    sort: Option<String>,
    #[serde(default)]
    desc: bool,
    filters: Option<String>,
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
    let rows_query = RowsQuery {
        sort: params.sort,
        desc: params.desc,
        filters: params.filters,
    };
    let request = build_request(&table, &rows_query, Some((page, size)))?;
    let filtered = !request.filters.is_empty();
    let sql = query::select(&catalog.schema, &table, &request, None);

    // Rows, estimates and policies pipelined on the same connection.
    let mut client = state.db.get().await?;
    let (row, (estimates, policies)) = try_join(
        async {
            client
                .query_one(sql.text.as_str(), &sql.param_refs())
                .await
                .map_err(user_query_error)
        },
        try_join(
            estimates(&client, &catalog.schema),
            policy_counts(&client, &catalog.schema),
        ),
    )
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

    let estimate = estimates.get(&table.name).copied().unwrap_or(-1);
    let (total, exact) = if filtered {
        let count = query::count(&catalog.schema, &table, &request);
        let total = bounded_count(&mut client, &count).await;
        (total, total.is_some())
    } else {
        row_count(&client, &catalog.schema, &table, estimate).await
    };
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
                "references": references(&catalog, &table, &c.name),
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

/// Converts form values (text or null) to JSON: json/jsonb are parsed as
/// JSON; everything else goes as text and Postgres casts it to the column type.
fn to_values(table: &Table, values: Map<String, Value>) -> Result<Map<String, Value>, ApiError> {
    let mut out = Map::new();
    for (key, value) in values {
        let Some(col) = table.column(&key).filter(|c| !c.generated) else {
            return Err(ApiError::bad_request(
                "unknown_or_generated_column",
                format!("unknown or generated column: {key}"),
            )
            .params(json!({ "column": key })));
        };
        let json = match value {
            Value::Null => Value::Null,
            Value::String(s) if matches!(col.type_name.as_str(), "json" | "jsonb") => {
                serde_json::from_str(&s).map_err(|e| {
                    ApiError::bad_request(
                        "invalid_json_value",
                        format!("{}: invalid JSON ({e})", col.name),
                    )
                    .params(json!({ "column": col.name, "detail": e.to_string() }))
                })?
            }
            Value::String(s) => Value::String(s),
            other => Value::String(other.to_string()),
        };
        out.insert(col.name.clone(), json);
    }
    Ok(out)
}

/// `column=eq.value` pairs of the primary key.
fn pk_filters(table: &Table, pk: &Map<String, Value>) -> Result<Vec<query::Condition>, ApiError> {
    if table.primary_key.is_empty() {
        return Err(ApiError::bad_request(
            "no_primary_key",
            "table without a primary key: editing from the panel is unavailable (use the SQL editor)",
        ));
    }
    let pairs: Vec<(String, String)> = table
        .primary_key
        .iter()
        .map(|col| {
            let value = match pk.get(col) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => {
                    return Err(ApiError::bad_request(
                        "missing_key_value",
                        format!("missing value for key '{col}'"),
                    )
                    .params(json!({ "column": col })));
                }
                Some(other) => other.to_string(),
            };
            Ok((col.clone(), format!("eq.{value}")))
        })
        .collect::<Result<_, _>>()?;
    Ok(query::parse_request(&pairs, table)
        .map_err(invalid_query)?
        .filters)
}

/// Runs a write and returns Postgres' error message as a 400.
async fn execute(state: &AdminState, sql: &query::Sql) -> Result<u64, ApiError> {
    let client = state.db.get().await?;
    client
        .execute(sql.text.as_str(), &sql.param_refs())
        .await
        .map_err(user_query_error)
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
    let sql = query::insert(&schema, &table, Value::Object(values), None).map_err(invalid_query)?;
    execute(&state, &sql).await?;
    Ok(Json(json!({ "message": "row inserted" })))
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
        .map_err(invalid_query)?;
    let n = execute(&state, &sql).await?;
    Ok(Json(
        json!({ "message": format!("{n} row(s) updated"), "count": n }),
    ))
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pks: Vec<Map<String, Value>>,
}

/// Deletes several rows (the grid selection) in a single transaction.
pub async fn delete_rows(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Json(body): Json<DeleteRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    if body.pks.is_empty() {
        return Err(ApiError::bad_request(
            "no_rows_selected",
            "no rows selected",
        ));
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
            .map_err(user_query_error)?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({ "message": format!("{total} row(s) deleted"), "count": total }),
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

/// uuid format check (8-4-4-4-12 hex).
pub(crate) fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The user id in the path is not a uuid.
pub(crate) fn invalid_id() -> ApiError {
    ApiError::bad_request("invalid_id", "invalid id")
}

pub async fn revoke_sessions(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(invalid_id());
    }
    let client = state.db.get().await?;
    let n = client
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    Ok(Json(
        json!({ "message": format!("{n} session(s) ended"), "count": n }),
    ))
}

pub async fn delete_user(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(invalid_id());
    }
    let client = state.db.get().await?;
    let n = client
        .execute("DELETE FROM auth.users WHERE id = $1::text::uuid", &[&id])
        .await?;
    if n == 0 {
        return Err(user_not_found());
    }
    Ok(Json(json!({ "message": "user deleted" })))
}

pub(crate) fn user_not_found() -> ApiError {
    ApiError::not_found("user_not_found", "user not found")
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
    fn values_keep_postgres_text() {
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
    fn validates_uuid() {
        assert!(is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b92"));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b9z"));
    }
}
