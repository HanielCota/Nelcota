//! Database lifecycle for a reusable auth host.
pub use nelcota_core::db::{AUTH_MIGRATION_VERSIONS, BootstrapError};

/// Initialize auth and shared context objects with the original migration
/// checksums. Safe on existing full databases; a full host can later promote
/// an auth-only database without losing its users, sessions or tokens.
pub async fn bootstrap(
    admin: &tokio_postgres::Config,
    authenticator_password: &str,
    statement_timeout_secs: u64,
) -> Result<(), BootstrapError> {
    nelcota_core::db::bootstrap_migrations(
        admin,
        authenticator_password,
        statement_timeout_secs,
        AUTH_MIGRATION_VERSIONS,
    )
    .await
}
