//! Panel SQL editor. Each run uses a NEW admin connection, dropped at the
//! end: a `BEGIN` without `COMMIT` or a `SET ROLE` left by the admin never
//! contaminates the pool. The SQL text does not go to the log.

use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::{NoTls, SimpleQueryMessage};

use crate::AdminState;

const MAX_ROWS: usize = 1000;

#[derive(Deserialize)]
pub struct SqlRequest {
    sql: String,
}

pub async fn run(State(state): State<AdminState>, Json(request): Json<SqlRequest>) -> Response {
    tracing::info!(bytes = request.sql.len(), "panel SQL editor run");
    let mut config = state.db_config.clone();
    config
        .application_name("nelcota-admin-sql")
        .options("-c statement_timeout=30s");
    let client = match config.connect(NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(connection);
            client
        }
        Err(err) => {
            return Json(json!({ "error": { "message": err.to_string() } })).into_response();
        }
    };

    match client.simple_query(&request.sql).await {
        Ok(messages) => {
            let mut results = Vec::new();
            let mut columns: Vec<String> = Vec::new();
            let mut rows: Vec<Vec<Value>> = Vec::new();
            let mut truncated = false;
            for message in messages {
                match message {
                    SimpleQueryMessage::RowDescription(desc) => {
                        columns = desc.iter().map(|c| c.name().to_owned()).collect();
                    }
                    SimpleQueryMessage::Row(row) => {
                        if columns.is_empty() {
                            columns = row.columns().iter().map(|c| c.name().to_owned()).collect();
                        }
                        if rows.len() < MAX_ROWS {
                            rows.push(
                                (0..row.len())
                                    .map(|i| {
                                        row.get(i)
                                            .map_or(Value::Null, |v| Value::String(v.to_owned()))
                                    })
                                    .collect(),
                            );
                        } else {
                            truncated = true;
                        }
                    }
                    SimpleQueryMessage::CommandComplete(count) => {
                        results.push(json!({
                            "columns": std::mem::take(&mut columns),
                            "rows": std::mem::take(&mut rows),
                            "count": count,
                            "truncated": std::mem::replace(&mut truncated, false),
                        }));
                    }
                    _ => {}
                }
            }
            Json(json!({ "results": results })).into_response()
        }
        Err(err) => {
            let error = match err.as_db_error() {
                Some(db) => json!({
                    "message": db.message(),
                    "code": db.code().code(),
                    "detail": db.detail(),
                    "hint": db.hint(),
                    "position": match db.position() {
                        Some(tokio_postgres::error::ErrorPosition::Original(p)) => Some(*p),
                        _ => None,
                    },
                }),
                None => json!({ "message": err.to_string() }),
            };
            Json(json!({ "error": error })).into_response()
        }
    }
}
