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
    .context("could not prepare the database")?;

    let keys = Arc::new(
        Keys::new(
            config.jwt_private_key(),
            config.jwt_secret().map(str::as_bytes),
        )
        .context("invalid JWT key")?,
    );
    tracing::info!(alg = ?keys.algorithm(), "JWT signing");
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
    // Auth email (single-use links). Without SMTP those endpoints answer
    // `*_disabled`; a half-done configuration already failed in `validate`.
    let mail = config.mail()?;
    let mailer = match &mail {
        Some(mail) => {
            let smtp = nelcota_auth::SmtpMailer::new(mail.smtp_url, mail.from)?;
            tracing::info!(
                sender = mail.from,
                signup_confirmation = mail.confirmation_url.is_some(),
                magic_link = mail.magic_link_url.is_some(),
                "auth email links enabled"
            );
            Some(Arc::new(smtp) as Arc<dyn nelcota_auth::Mailer>)
        }
        None => None,
    };
    let oauth = match config.oauth()? {
        Some(settings) => {
            let mut providers = Vec::new();
            if let Some((id, secret)) = settings.google {
                providers.push(nelcota_auth::Provider::google(id, secret));
            }
            if let Some((id, secret)) = settings.github {
                providers.push(nelcota_auth::Provider::github(id, secret));
            }
            let names: Vec<_> = providers.iter().map(|p| p.kind().as_str()).collect();
            let oauth = nelcota_auth::OAuth::new(
                settings.api_url,
                settings.redirect_urls.iter().copied(),
                providers,
            )
            .map_err(anyhow::Error::msg)
            .context("invalid sign-in provider settings")?;
            tracing::info!(providers = ?names, "sign-in providers enabled");
            Some(Arc::new(oauth))
        }
        None => None,
    };
    let passwords = Arc::new(Passwords::new(hash_concurrency()));
    let auth = AuthState {
        mailer,
        oauth,
        pool: pool.clone(),
        keys: keys.clone(),
        passwords: passwords.clone(),
        limiter: Arc::new(RateLimiter::new(config.auth_rate_limit_per_minute)),
        settings: Arc::new(AuthSettings {
            issuer: config.jwt_issuer.clone(),
            access_ttl_secs: config.jwt_expiry_secs,
            refresh_ttl_days: config.refresh_token_ttl_days,
            signup_enabled: config.signup_enabled,
            trust_proxy: config.trust_proxy,
            links: mail
                .map(|m| nelcota_auth::EmailLinks {
                    recovery: Some(m.recovery_url.to_owned()),
                    signup_confirmation: m.confirmation_url.map(str::to_owned),
                    magic_link: m.magic_link_url.map(str::to_owned),
                })
                .unwrap_or_default(),
        }),
    };
    let storage = match nelcota_storage::Store::from_config(&config)
        .context("could not open the file storage")?
    {
        Some(store) => {
            let store = Arc::new(store);
            tracing::info!(backend = ?config.storage_backend, "file storage enabled");
            nelcota_storage::spawn_collector(pool.clone(), store.clone());
            Some(nelcota_storage::StorageState::new(
                pool.clone(),
                keys.clone(),
                store,
                nelcota_storage::StorageSettings::from_config(&config),
            ))
        }
        None => None,
    };
    let admin = match (&config.admin_email, &config.admin_password_hash) {
        (Some(email), Some(hash)) if !email.is_empty() && !hash.expose().is_empty() => {
            Some(nelcota_admin::AdminState {
                sql: Arc::default(),
                // 3: a long export holds its own and the panel keeps going with the others.
                db: db::admin_pool(&admin, 3),
                db_config: admin.clone(),
                catalog: state.catalog.clone(),
                credentials: Arc::new(nelcota_admin::Credentials {
                    email: email.clone(),
                    password_hash: hash.expose().to_owned(),
                }),
                sessions: Arc::default(),
                limiter: Arc::new(RateLimiter::new(10)),
                passwords: passwords.clone(),
                trust_proxy: config.trust_proxy,
                secure_cookies: config.trust_proxy,
                host: Arc::new(host_link(&config)),
                tokens: Arc::new(nelcota_admin::TokenIssuer {
                    keys: keys.clone(),
                    issuer: config.jwt_issuer.clone(),
                }),
                migrations_dir: nelcota_admin::default_migrations_dir(
                    config.migrations_dir.clone(),
                ),
                storage: storage.clone(),
                sign_in: Arc::new(sign_in(&config)?),
                denied: Arc::default(),
                metrics: Arc::default(),
            })
        }
        _ => {
            tracing::info!(
                "panel disabled (set NELCOTA_ADMIN_EMAIL and NELCOTA_ADMIN_PASSWORD_HASH)"
            );
            None
        }
    };
    let router = app(
        state,
        auth,
        admin,
        storage,
        Duration::from_secs(config.request_timeout_secs),
    );

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("could not listen on {}", config.listen))?;
    tracing::info!(addr = %config.listen, "nelcota is up");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// How the app's users can sign in, for the panel's read-only summary.
fn sign_in(config: &Config) -> anyhow::Result<nelcota_admin::contracts::SignIn> {
    let mail = config.mail()?;
    let oauth = config.oauth()?;
    Ok(nelcota_admin::contracts::SignIn {
        signup_enabled: config.signup_enabled,
        email: mail.is_some(),
        email_confirmation: mail.as_ref().is_some_and(|m| m.confirmation_url.is_some()),
        password_recovery: mail.is_some(),
        magic_link: mail.as_ref().is_some_and(|m| m.magic_link_url.is_some()),
        providers: nelcota_admin::contracts::SignInProviders {
            google: oauth.as_ref().is_some_and(|o| o.google.is_some()),
            github: oauth.as_ref().is_some_and(|o| o.github.is_some()),
        },
        redirect_urls: oauth
            .as_ref()
            .map(|o| o.redirect_urls.iter().map(|u| (*u).to_owned()).collect())
            .unwrap_or_default(),
        callback_url: oauth
            .as_ref()
            .map(|o| format!("{}/auth/v1/callback", o.api_url.trim_end_matches('/'))),
        access_ttl_secs: config.jwt_expiry_secs,
        refresh_ttl_days: config.refresh_token_ttl_days,
        rate_limit_per_minute: config.auth_rate_limit_per_minute,
    })
}

/// Current project, the host's list and single sign-on (if there is a shared secret).
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

/// Concurrent argon2 hashes: each uses ~19 MiB; with at most 4 the peak stays
/// under 80 MiB even under attack.
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
    tracing::info!("shutting down");
}
