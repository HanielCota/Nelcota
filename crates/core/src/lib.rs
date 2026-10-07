//! Shared Nelcota pieces: configuration, HTTP errors, claims/roles and Postgres
//! access (pool, migrations and the per-request transaction scope).

pub mod claims;
pub mod config;
pub mod db;
pub mod error;

pub use claims::{Claims, InvalidClaims, Role};
pub use config::{Config, MailConfig, Secret};
pub use error::ApiError;
