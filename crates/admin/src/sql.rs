//! HTTP adaptation for the SQL editor.
mod executor;
mod run_as;
pub use executor::{SqlBusy, SqlExecutor};
use run_as::RunAs;

use crate::{AdminState, ApiError};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Map, json};

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct SqlRequest {
    sql: String,
    /// Who the run acts as; the database owner when absent.
    #[serde(default)]
    run_as: RunAs,
}

pub async fn run(State(state): State<AdminState>, Json(request): Json<SqlRequest>) -> Response {
    let claims = match request.run_as.claims(&state.db).await {
        Ok(claims) => claims,
        Err(err) => return err.into_response(),
    };
    match state.sql.execute(&state.db_config, &request.sql, claims.as_ref()).await {
        Ok(result) => Json(result).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": "SQL execution is busy; wait for a running query to finish", "code": "sql_busy"
        }))).into_response(),
    }
}

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
