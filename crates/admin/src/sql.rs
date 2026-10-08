//! Isolated SQL connections with bounded concurrency and streamed results.
use crate::AdminState;
use crate::contracts::{SqlError, SqlResponse, SqlResult};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{StreamExt, pin_mut};
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio_postgres::{NoTls, SimpleQueryMessage};

const MAX_ROWS: usize = 1000;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESULTS: usize = 32;

/// Shared by every request. Dedicated connections never contaminate a pool.
pub struct SqlExecutor {
    slots: Semaphore,
}
impl Default for SqlExecutor {
    fn default() -> Self {
        Self {
            slots: Semaphore::new(4),
        }
    }
}
#[derive(Deserialize)]
pub struct SqlRequest {
    sql: String,
}
struct Results {
    results: Vec<SqlResult>,
    results_truncated: bool,
    current: SqlResult,
    bytes: usize,
}
impl Results {
    fn new() -> Self {
        Self {
            results: Vec::new(),
            results_truncated: false,
            current: SqlResult::default(),
            bytes: 0,
        }
    }
    fn message(&mut self, message: SimpleQueryMessage) {
        let retain = self.results.len() < MAX_RESULTS;
        match message {
            SimpleQueryMessage::RowDescription(columns) if retain => {
                self.current.columns = columns.iter().map(|c| c.name().to_owned()).collect();
                self.bytes += self.current.columns.iter().map(|c| c.len()).sum::<usize>();
            }
            SimpleQueryMessage::Row(row) if retain => {
                let bytes = (0..row.len())
                    .map(|i| row.get(i).map_or(4, |v| v.len() + 8))
                    .sum::<usize>();
                if self.current.rows.len() < MAX_ROWS
                    && self.bytes.saturating_add(bytes) <= MAX_BYTES
                {
                    self.bytes += bytes;
                    self.current.rows.push(
                        (0..row.len())
                            .map(|i| row.get(i).map(str::to_owned))
                            .collect(),
                    );
                } else {
                    self.current.truncated = true;
                }
            }
            SimpleQueryMessage::CommandComplete(count) => {
                if retain {
                    self.current.count = count;
                    self.results.push(std::mem::take(&mut self.current));
                } else {
                    self.results_truncated = true;
                }
            }
            _ => {}
        }
    }
}
/// Dropping the HTTP future also closes its dedicated database connection.
struct Driver {
    task: tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>,
    cancel: tokio_postgres::CancelToken,
    completed: bool,
}
impl Drop for Driver {
    fn drop(&mut self) {
        // Closing a socket alone need not interrupt a running Postgres query.
        if !self.completed {
            let cancel = self.cancel.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        cancel.cancel_query(NoTls),
                    )
                    .await;
                });
            }
        }
        self.task.abort();
    }
}
pub async fn run(State(state): State<AdminState>, Json(request): Json<SqlRequest>) -> Response {
    let Ok(_slot) = state.sql.slots.try_acquire() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "error": "SQL execution is busy; wait for a running query to finish", "code": "sql_busy"
        }))).into_response();
    };
    tracing::info!(bytes = request.sql.len(), "panel SQL editor run");
    let mut config = state.db_config.clone();
    config
        .application_name("nelcota-admin-sql")
        .options("-c statement_timeout=30s");
    let (client, connection) = match config.connect(NoTls).await {
        Ok(connection) => connection,
        Err(err) => return query_error(err),
    };
    let mut driver = Driver {
        task: tokio::spawn(connection),
        cancel: client.cancel_token(),
        completed: false,
    };
    let stream = match client.simple_query_raw(&request.sql).await {
        Ok(stream) => stream,
        Err(err) => {
            driver.completed = true;
            return query_error(err);
        }
    };
    pin_mut!(stream);
    let mut results = Results::new();
    // Drain even after the display budget is spent: batches must still run
    // every statement and retain their transaction semantics.
    while let Some(message) = stream.next().await {
        match message {
            Ok(message) => results.message(message),
            Err(err) => {
                driver.completed = true;
                return query_error(err);
            }
        }
    }
    driver.completed = true;
    Json(SqlResponse::Success {
        results: results.results,
        results_truncated: results.results_truncated,
    })
    .into_response()
}
fn query_error(err: tokio_postgres::Error) -> Response {
    let error = match err.as_db_error() {
        Some(db) => SqlError {
            message: db.message().to_owned(),
            code: Some(db.code().code().to_owned()),
            detail: db.detail().map(str::to_owned),
            hint: db.hint().map(str::to_owned),
            position: match db.position() {
                Some(tokio_postgres::error::ErrorPosition::Original(p)) => Some(*p),
                _ => None,
            },
        },
        None => SqlError {
            message: err.to_string(),
            code: None,
            detail: None,
            hint: None,
            position: None,
        },
    };
    Json(SqlResponse::Failure { error }).into_response()
}
