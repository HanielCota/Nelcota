use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use nelcota_auth::{AuthSettings, AuthState, Keys, Passwords, RateLimiter};
use nelcota_core::{Config, config::LogFormat, db};
use nelcota_server::{AppState, app};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().context("configuração inválida")?;
    init_tracing(config.log_format);

    let admin = config.database_config()?;
    db::bootstrap(&admin, config.authenticator_password.expose())
        .await
        .context("falha ao preparar o banco")?;

    let keys = Arc::new(
        Keys::new(
            config.jwt_private_key(),
            config.jwt_secret().map(str::as_bytes),
        )
        .context("chave de JWT inválida")?,
    );
    tracing::info!(alg = ?keys.algorithm(), "assinatura de JWT");
    let pool = db::api_pool(
        &admin,
        config.authenticator_password.expose(),
        config.db_pool_size,
    );
    let state = AppState {
        pool: pool.clone(),
        verifier: keys.clone(),
    };
    let auth = AuthState {
        pool,
        keys,
        passwords: Arc::new(Passwords::new(hash_concurrency())),
        limiter: Arc::new(RateLimiter::new(config.auth_rate_limit_per_minute)),
        settings: Arc::new(AuthSettings {
            issuer: config.jwt_issuer.clone(),
            access_ttl_secs: config.jwt_expiry_secs,
            refresh_ttl_days: config.refresh_token_ttl_days,
            signup_enabled: config.signup_enabled,
            trust_proxy: config.trust_proxy,
        }),
    };
    let router = app(
        state,
        auth,
        Duration::from_secs(config.request_timeout_secs),
    );

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("não foi possível escutar em {}", config.listen))?;
    tracing::info!(addr = %config.listen, "nelcota no ar");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Hashes argon2 simultâneos: cada um usa ~19 MiB; com até 4 o pico fica
/// abaixo de 80 MiB mesmo sob ataque.
fn hash_concurrency() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

fn init_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));
    let builder = fmt().with_env_filter(filter);
    match format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("encerrando");
}
