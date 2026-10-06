//! Peças comuns do Nelcota: configuração, erros HTTP, claims/roles e acesso ao
//! Postgres (pool, migrações e o escopo de transação por request).

pub mod claims;
pub mod config;
pub mod db;
pub mod error;

pub use claims::{Claims, InvalidClaims, Role};
pub use config::{Config, Secret};
pub use error::ApiError;
