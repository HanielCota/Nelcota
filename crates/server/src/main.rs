use std::{sync::Arc, time::Duration};

use anyhow::Context;
use nelcota_auth::Hs256Verifier;
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

    let state = AppState {
        pool: db::api_pool(
            &admin,
            config.authenticator_password.expose(),
            config.db_pool_size,
        ),
        verifier: Arc::new(Hs256Verifier::new(config.jwt_secret.expose().as_bytes())),
    };
    let router = app(state, Duration::from_secs(config.request_timeout_secs));

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("não foi possível escutar em {}", config.listen))?;
    tracing::info!(addr = %config.listen, "nelcota no ar");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
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
