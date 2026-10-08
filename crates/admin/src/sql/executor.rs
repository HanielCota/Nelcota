//! Dedicated SQL execution, result budgets and cancellation.
use crate::contracts::{SqlError, SqlResponse, SqlResult};
use futures_util::{StreamExt, pin_mut};
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

/// All execution slots are occupied. HTTP adapters choose how to report it.
pub struct SqlBusy;

impl SqlExecutor {
    pub async fn execute(
        &self,
        config: &tokio_postgres::Config,
        sql: &str,
    ) -> Result<SqlResponse, SqlBusy> {
        let _slot = self.slots.try_acquire().map_err(|_| SqlBusy)?;
        tracing::info!(bytes = sql.len(), "panel SQL editor run");
        let mut config = config.clone();
        config
            .application_name("nelcota-admin-sql")
            .options("-c statement_timeout=30s");
        let (client, connection) = match config.connect(NoTls).await {
            Ok(connection) => connection,
            Err(err) => return Ok(query_error(err)),
        };
        let mut driver = Driver {
            task: tokio::spawn(connection),
            cancel: client.cancel_token(),
            completed: false,
        };
        let stream = match client.simple_query_raw(sql).await {
            Ok(stream) => stream,
            Err(err) => {
                driver.completed = true;
                return Ok(query_error(err));
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
                    return Ok(query_error(err));
                }
            }
        }
        driver.completed = true;
        Ok(SqlResponse::Success {
            results: results.results,
            results_truncated: results.results_truncated,
        })
    }
}

fn query_error(err: tokio_postgres::Error) -> SqlResponse {
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
    SqlResponse::Failure { error }
}
