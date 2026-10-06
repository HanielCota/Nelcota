//! Acesso ao Postgres.
//!
//! - [`bootstrap`]: aplica as migrações internas e define a senha da role
//!   `authenticator` (conexão administrativa, usada só na inicialização).
//! - [`api_pool`]: pool da API, sempre conectado como `authenticator`.
//! - [`begin_request`]: abre a transação de um request já com a role e as
//!   claims do JWT. É o único caminho para executar SQL em nome de um usuário.

use std::time::Duration;

use deadpool_postgres::{
    Manager, ManagerConfig, Object, Pool, RecyclingMethod, Runtime, Transaction,
};
use tokio_postgres::NoTls;

use crate::Claims;

mod embedded {
    refinery::embed_migrations!("../../migrations");
}

/// Tabela de controle das migrações internas (documentada em docs/arquitetura.md).
pub const MIGRATIONS_TABLE: &str = "nelcota.schema_migrations";

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("falha ao conectar no Postgres: {0}")]
    Connect(#[source] tokio_postgres::Error),
    #[error("falha ao preparar o banco: {0}")]
    Sql(#[from] tokio_postgres::Error),
    #[error("falha ao aplicar migrações: {0}")]
    Migration(#[from] refinery::Error),
}

/// Aplica as migrações internas e (re)define a senha do `authenticator`.
/// Idempotente; um advisory lock evita corrida entre instâncias.
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
            tracing::error!(error = %err, "conexão administrativa encerrada com erro");
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
        tracing::info!(migration = %migration, "migração aplicada");
    }

    // O verificador SCRAM é calculado aqui: a senha em texto puro nunca
    // trafega nem aparece em log de statement do Postgres.
    let verifier = postgres_protocol::password::scram_sha_256(authenticator_password.as_bytes());
    let statement: String = client
        .query_one(
            "SELECT format('ALTER ROLE authenticator WITH PASSWORD %L', $1::text)",
            &[&verifier],
        )
        .await?
        .get(0);
    client.batch_execute(&statement).await?;
    // Teto de duração por statement nas conexões da API. Vale para todas as
    // roles do request: `ALTER ROLE anon SET ...` não teria efeito, porque o
    // Postgres só aplica as configurações por role no login (authenticator).
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

/// Config de conexão como `authenticator` (mesmo host/banco da URL administrativa).
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

/// Pool da API: mesmo host/banco da URL administrativa, mas como `authenticator`.
pub fn api_pool(
    admin: &tokio_postgres::Config,
    authenticator_password: &str,
    max_size: usize,
) -> Pool {
    let config = authenticator_config(admin, authenticator_password);
    // `Fast` não roda nada ao devolver a conexão: role e claims são definidas
    // com escopo de transação (`is_local = true`) e morrem no COMMIT/ROLLBACK.
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
        .expect("runtime definido, build não falha")
}

/// Abre a transação do request e assume a role/claims do JWT.
///
/// Equivale a `SET LOCAL ROLE <role>` + `set_config('request.jwt.claims', ..., true)`,
/// numa única ida ao banco e com os dois valores como parâmetros. Se a
/// transação não for confirmada (erro, panic, early return), o drop faz
/// ROLLBACK e nada vaza para o próximo uso da conexão.
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
