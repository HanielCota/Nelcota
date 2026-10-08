//! HTTP adaptation for the SQL editor.
mod executor;
pub use executor::SqlExecutor;

use crate::AdminState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SqlRequest {
    sql: String,
}

pub async fn run(State(state): State<AdminState>, Json(request): Json<SqlRequest>) -> Response {
    match state.sql.execute(&state.db_config, &request.sql).await {
        Ok(result) => Json(result).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": "SQL execution is busy; wait for a running query to finish", "code": "sql_busy"
        }))).into_response(),
    }
}
