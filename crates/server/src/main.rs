use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use clap::Parser;
use nelcota_api::{ApiSettings, spawn_reload_listener};
use nelcota_auth::{AuthSettings, AuthState, Keys, Passwords, RateLimiter};
use nelcota_cli::{Cli, Outcome};
use nelcota_core::{Config, config::LogFormat, db};
use nelcota_server::{AppState, app, load_catalog};
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match nelcota_cli::run(cli).context("nelcota")? {
        Outcome::Done => Ok(()),
        Outcome::Serve(config) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(serve(*config)),
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    init_tracing(config.log_format);

    let admin = config.database_config()?;
    db::bootstrap(
        &admin,
        config.authenticator_password.expose(),
        config.statement_timeout_secs,
    )
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
    let catalog = load_catalog(&pool, &config.db_schema).await?;
    spawn_reload_listener(
        catalog.clone(),
        pool.clone(),
        db::authenticator_config(&admin, config.authenticator_password.expose()),
    );
    let state = AppState {
        pool: pool.clone(),
        verifier: keys.clone(),
        catalog,
        api: Arc::new(ApiSettings {
            max_rows: config.max_rows,
        }),
    };
    // Email do auth (recuperação de senha). Sem SMTP, o endpoint responde
    // `recovery_disabled`; configuração pela metade já falhou no `validate`.
    let mail = config.mail()?;
    let mailer = match &mail {
        Some(mail) => {
            let smtp = nelcota_auth::SmtpMailer::new(mail.smtp_url, mail.from)?;
            tracing::info!(
                remetente = mail.from,
                "recuperação de senha por email ligada"
            );
            Some(Arc::new(smtp) as Arc<dyn nelcota_auth::Mailer>)
        }
        None => None,
    };
    let auth = AuthState {
        mailer,
        pool,
        keys: keys.clone(),
        passwords: Arc::new(Passwords::new(hash_concurrency())),
        limiter: Arc::new(RateLimiter::new(config.auth_rate_limit_per_minute)),
        settings: Arc::new(AuthSettings {
            issuer: config.jwt_issuer.clone(),
            access_ttl_secs: config.jwt_expiry_secs,
            refresh_ttl_days: config.refresh_token_ttl_days,
            signup_enabled: config.signup_enabled,
            trust_proxy: config.trust_proxy,
            recovery_url: mail.map(|m| m.recovery_url.to_owned()),
        }),
    };
    let admin = match (&config.admin_email, &config.admin_password_hash) {
        (Some(email), Some(hash)) if !email.is_empty() && !hash.expose().is_empty() => {
            Some(nelcota_admin::AdminState {
                // 3: uma exportação longa segura a dela e o painel segue com as outras.
                db: db::admin_pool(&admin, 3),
                db_config: admin.clone(),
                catalog: state.catalog.clone(),
                credentials: Arc::new(nelcota_admin::Credentials {
                    email: email.clone(),
                    password_hash: hash.expose().to_owned(),
                }),
                sessions: Arc::default(),
                limiter: Arc::new(RateLimiter::new(10)),
                secure_cookies: config.trust_proxy,
                host: Arc::new(host_link(&config)),
                tokens: Arc::new(nelcota_admin::TokenIssuer {
                    keys,
                    issuer: config.jwt_issuer.clone(),
                }),
                migrations_dir: nelcota_admin::default_migrations_dir(
                    config.migrations_dir.clone(),
                ),
            })
        }
        _ => {
            tracing::info!(
                "painel desligado (defina NELCOTA_ADMIN_EMAIL e NELCOTA_ADMIN_PASSWORD_HASH)"
            );
            None
        }
    };
    let router = app(
        state,
        auth,
        admin,
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

/// Projeto atual, lista do host e login único (se houver segredo compartilhado).
fn host_link(config: &Config) -> nelcota_admin::HostLink {
    let sso = config
        .admin_sso_secret
        .as_ref()
        .map(|s| s.expose())
        .filter(|s| s.len() >= 32)
        .map(|s| nelcota_admin::Sso::new(s.as_bytes()));
    nelcota_admin::HostLink {
        project: config
            .project_name
            .clone()
            .unwrap_or_else(|| "nelcota".into()),
        registry: config.host_registry.clone(),
        sso,
    }
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
