//! Database-owned event capture and durable, at-least-once HTTP delivery.
//! Management, queue leases, signing and network policy have separate owners.
pub mod destination;
pub mod management;
pub mod models;
mod queue;
pub mod signing;
mod transport;
mod worker;

pub use worker::{Worker, WorkerHandle};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{message}")]
    Invalid {
        code: &'static str,
        message: &'static str,
    },
    #[error("{message}")]
    Conflict {
        code: &'static str,
        message: &'static str,
    },
    #[error("webhook or delivery not found")]
    NotFound,
    #[error("database unavailable")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("webhook database operation failed")]
    Database(#[from] tokio_postgres::Error),
    #[error("could not generate a signing key")]
    Random(#[from] getrandom::Error),
}

fn invalid(code: &'static str, message: &'static str) -> Error {
    Error::Invalid { code, message }
}
