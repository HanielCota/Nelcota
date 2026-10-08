//! Shared table metadata for the panel's tables, policies and overview.
use crate::{AdminState, ApiError};
use nelcota_api::{
    Catalog,
    catalog::{Table, TableKind},
    query,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio_postgres::Client;

pub(crate) fn table_or_404(state: &AdminState, name: &str) -> Result<Table, ApiError> {
    state.catalog.get().table(name).cloned().ok_or_else(|| {
        ApiError::not_found(
            "table_not_found",
            format!("table '{name}' does not exist in the exposed schema"),
        )
        .params(json!({ "table": name }))
    })
}

pub(crate) async fn policy_counts(
    client: &Client,
    schema: &str,
) -> Result<HashMap<String, usize>, ApiError> {
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
pub(crate) fn rls_json(table: &Table, policies: usize) -> Value {
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

pub(crate) fn grants(table: &Table, role: usize) -> Vec<&'static str> {
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

pub(crate) fn kind(table: &Table) -> &'static str {
    match table.kind {
        TableKind::Table => "table",
        TableKind::View => "view",
        TableKind::MaterializedView => "materialized_view",
        TableKind::ForeignTable => "foreign_table",
    }
}

pub(crate) fn exposed(catalog: &Catalog) -> Vec<&str> {
    catalog
        .tables
        .values()
        .filter(|t| t.exposed_without_rls())
        .map(|t| t.name.as_str())
        .collect()
}

/// Exact count for small (or never analysed) tables; the planner's estimate
/// for large ones, so it never costs a seq scan.
pub(crate) async fn row_count(
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

pub(crate) async fn estimates(
    client: &Client,
    schema: &str,
) -> Result<HashMap<String, i64>, ApiError> {
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
