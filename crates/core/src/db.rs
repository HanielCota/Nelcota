//! Postgres access.
//!
//! - [`bootstrap`]: applies the internal migrations and sets the
//!   `authenticator` role password (admin connection, used only at startup).
//! - [`api_pool`]: the API pool, always connected as `authenticator`.
//! - [`begin_request`]: opens a request transaction already carrying the JWT
//!   role and claims. It is the only way to run SQL on behalf of a user.

use std::time::Duration;

use deadpool_postgres::{
    Manager, ManagerConfig, Object, Pool, RecyclingMethod, Runtime, Transaction,
};
use tokio_postgres::NoTls;

use crate::Claims;

mod embedded {
    refinery::embed_migrations!("../../migrations");
}

/// Control table of the internal migrations (documented in docs/architecture.md).
pub const MIGRATIONS_TABLE: &str = "nelcota.schema_migrations";

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("failed to connect to Postgres: {0}")]
    Connect(#[source] tokio_postgres::Error),
    #[error("failed to prepare the database: {0}")]
    Sql(#[from] tokio_postgres::Error),
    #[error("failed to apply migrations: {0}")]
    Migration(#[from] refinery::Error),
}

/// Applies the internal migrations and (re)sets the `authenticator` password.
/// Idempotent; an advisory lock prevents races between instances.
pub async fn bootstrap(
    admin: &tokio_postgres::Config,
    authenticator_password: &str,
    statement_timeout_secs: u64,
) -> Result<(), BootstrapError> {
    let (mut client, connection) = admin
        .connect(NoTls)
        .await
        .map_err(BootstrapError::Connect)?;
    let connection = tokio::spawn(async move {
        if let Err(err) = connection.await {
            tracing::error!(error = %err, "admin connection closed with an error");
        }
    });

    client
        .batch_execute(
            "SELECT pg_advisory_lock(hashtext('nelcota.migrations'));
             CREATE SCHEMA IF NOT EXISTS nelcota;",
        )
        .await?;

    let mut runner = embedded::migrations::runner();
    runner.set_migration_table_name(MIGRATIONS_TABLE);
    let report = runner.run_async(&mut client).await?;
    for migration in report.applied_migrations() {
        tracing::info!(migration = %migration, "migration applied");
    }

    // The SCRAM verifier is computed here: the plain-text password never
    // travels nor shows up in Postgres statement logs.
    let verifier = postgres_protocol::password::scram_sha_256(authenticator_password.as_bytes());
    let statement: String = client
        .query_one(
            "SELECT format('ALTER ROLE authenticator WITH PASSWORD %L', $1::text)",
            &[&verifier],
        )
        .await?
        .get(0);
    client.batch_execute(&statement).await?;
    // Per-statement time cap on the API connections. It covers every request
    // role: `ALTER ROLE anon SET ...` would have no effect, because Postgres
    // only applies per-role settings at login (authenticator).
    let statement: String = client
        .query_one(
            "SELECT format('ALTER ROLE authenticator SET statement_timeout = %L', $1::text)",
            &[&format!("{statement_timeout_secs}s")],
        )
        .await?
        .get(0);
    client.batch_execute(&statement).await?;
    client
        .batch_execute("SELECT pg_advisory_unlock(hashtext('nelcota.migrations'))")
        .await?;

    drop(client);
    let _ = connection.await;
    Ok(())
}

/// Connection config as `authenticator` (same host/database as the admin URL).
pub fn authenticator_config(
    admin: &tokio_postgres::Config,
    authenticator_password: &str,
) -> tokio_postgres::Config {
    let mut config = admin.clone();
    config
        .user("authenticator")
        .password(authenticator_password)
        .application_name("nelcota");
    config
}

/// API pool: same host/database as the admin URL, but as `authenticator`.
pub fn api_pool(
    admin: &tokio_postgres::Config,
    authenticator_password: &str,
    max_size: usize,
) -> Pool {
    let config = authenticator_config(admin, authenticator_password);
    // `Fast` runs nothing when a connection is returned: role and claims are
    // transaction-scoped (`is_local = true`) and die at COMMIT/ROLLBACK.
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(max_size)
        .runtime(Runtime::Tokio1)
        .wait_timeout(Some(Duration::from_secs(5)))
        .create_timeout(Some(Duration::from_secs(5)))
        .build()
        .expect("runtime is set, build cannot fail")
}

/// Admin pool (schema owner role), used only by the panel. Small and with its
/// own `statement_timeout`.
pub fn admin_pool(admin: &tokio_postgres::Config, max_size: usize) -> Pool {
    let mut config = admin.clone();
    config
        .application_name("nelcota-admin")
        .options("-c statement_timeout=30s");
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(max_size)
        .runtime(Runtime::Tokio1)
        .wait_timeout(Some(Duration::from_secs(5)))
        .create_timeout(Some(Duration::from_secs(5)))
        .build()
        .expect("runtime is set, build cannot fail")
}

/// Opens the request transaction and assumes the JWT role/claims.
///
/// Equivalent to `SET LOCAL ROLE <role>` + `set_config('request.jwt.claims', ..., true)`,
/// in a single round trip and with both values as parameters. If the
/// transaction is not committed (error, panic, early return), dropping it
/// rolls back and nothing leaks into the connection's next use.
pub async fn begin_request<'a>(
    client: &'a mut Object,
    claims: &Claims,
) -> Result<Transaction<'a>, tokio_postgres::Error> {
    let tx = client.transaction().await?;
    let statement = tx
        .prepare_cached(
            "SELECT set_config('role', $1, true), set_config('request.jwt.claims', $2, true)",
        )
        .await?;
    tx.execute(&statement, &[&claims.role().as_str(), &claims.as_json()])
        .await?;
    Ok(tx)
}
