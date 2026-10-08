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

type ApiResult<T = Value> = Result<Json<T>, ApiError>;
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

mod overview;
mod policies;
mod tables;
mod users;
pub(crate) use overview::*;
pub(crate) use policies::*;
pub(crate) use tables::*;
pub(crate) use users::*;

/// Tables and columns for CodeMirror's autocomplete (`schema.table`).
pub async fn schema(
    State(state): State<AdminState>,
) -> ApiResult<crate::contracts::SchemaResponse> {
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
    crate::contracts::response::<crate::contracts::SchemaResponse>(
        json!({ "schema": catalog.schema, "tables": tables }),
    )
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
