//! Overview panel handlers.
use crate::{
    AdminState, ApiError,
    tables::catalog::{estimates, exposed, grants, kind, policy_counts, rls_json, row_count},
};
use axum::{Json, extract::State};
use futures_util::future::{join_all, try_join3};
use serde_json::json;
type ApiResult<T> = Result<Json<T>, ApiError>;

pub async fn overview(State(state): State<AdminState>) -> ApiResult<crate::contracts::Overview> {
    let catalog = state.catalog.get();
    // A single connection, with the independent queries pipelined on it.
    let client = state.db.get().await?;
    let (policies, estimates, users) = try_join3(
        policy_counts(&client, &catalog.schema),
        estimates(&client, &catalog.schema),
        async {
            Ok::<_, ApiError>(
                client
                    .query_one(
                        "SELECT count(*), count(last_sign_in_at) FROM auth.users",
                        &[],
                    )
                    .await?,
            )
        },
    )
    .await?;
    let (signed_in_users, users): (i64, i64) = (users.get(1), users.get(0));

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
    crate::contracts::response::<crate::contracts::Overview>(json!({
        "schema": catalog.schema,
        "counts": {
            "tables": catalog.tables.len(),
            "users": users,
            "signed_in_users": signed_in_users,
            "policies": policies.values().sum::<usize>(),
            "functions": catalog.functions.values().map(Vec::len).sum::<usize>(),
        },
        "exposed_without_rls": exposed(&catalog),
        "tables": tables,
    }))
}
