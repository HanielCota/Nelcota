//! `nelcota dev`: Postgres 17 in a local container + the server in the
//! foreground, with development secrets generated and kept in `.nelcota/dev.env`.

use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use nelcota_core::{Config, Secret, config::LogFormat};
use tokio_postgres::NoTls;

use std::path::Path;

use crate::{
    DevArgs, Outcome,
    envfile::{parse, write_private},
    util::{self, ok, step},
};

const STATE_FILE: &str = ".nelcota/dev.env";

struct DevState {
    postgres_password: String,
    authenticator_password: String,
    jwt_private_key: String,
    db_port: u16,
}

fn load_state(root: &Path) -> anyhow::Result<Option<DevState>> {
    let Ok(text) = fs::read_to_string(root.join(STATE_FILE)) else {
        return Ok(None);
    };
    let env = parse(&text);
    let get = |key: &str| {
        env.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .with_context(|| format!("{key} missing from {STATE_FILE}"))
    };
    Ok(Some(DevState {
        postgres_password: get("POSTGRES_PASSWORD")?,
        authenticator_password: get("AUTHENTICATOR_PASSWORD")?,
        jwt_private_key: get("JWT_PRIVATE_KEY")?,
        db_port: get("DB_PORT")?.parse()?,
    }))
}

fn config_for(state: &DevState, listen: std::net::SocketAddr) -> Config {
    Config {
        database_url: Secret::new(format!(
            "postgres://postgres:{}@127.0.0.1:{}/postgres",
            state.postgres_password, state.db_port
        )),
        authenticator_password: Secret::new(state.authenticator_password.clone()),
        jwt_private_key: Some(Secret::new(state.jwt_private_key.clone())),
        jwt_issuer: "nelcota-dev".into(),
        listen,
        log_format: LogFormat::Text,
        ..Config::default()
    }
}

/// Configuration of the already created dev environment (for `migrate`/`types`).
pub fn saved_config(root: &Path) -> anyhow::Result<Option<Config>> {
    Ok(load_state(root)?.map(|s| config_for(&s, ([127, 0, 0, 1], 8000).into())))
}

pub fn run(root: &Path, args: DevArgs) -> anyhow::Result<Outcome> {
    let state = match load_state(root)? {
        Some(state) => state,
        None => {
            let state = DevState {
                postgres_password: util::secret(24),
                authenticator_password: util::secret(24),
                jwt_private_key: nelcota_auth::generate_ed25519_private_key(),
                db_port: args.db_port,
            };
            fs::create_dir_all(root.join(".nelcota"))?;
            write_private(
                &root.join(STATE_FILE),
                &format!(
                    "# Development environment secrets (do not use in production).\n\
                     POSTGRES_PASSWORD={}\nAUTHENTICATOR_PASSWORD={}\nJWT_PRIVATE_KEY={}\nDB_PORT={}\n",
                    state.postgres_password,
                    state.authenticator_password,
                    state.jwt_private_key,
                    state.db_port
                ),
            )?;
            fs::write(root.join(".nelcota/.gitignore"), "*\n")?;
            state
        }
    };

    let container = format!("nelcota-dev-postgres-{}", state.db_port);
    step(&format!("Development Postgres ({container})"));
    let running = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", &container])
        .output()
        .context("Docker not found")?;
    if !running.status.success() {
        let status = Command::new("docker")
            .args(["run", "-d", "--name", &container, "-e"])
            .arg(format!("POSTGRES_PASSWORD={}", state.postgres_password))
            .arg("-p")
            .arg(format!("127.0.0.1:{}:5432", state.db_port))
            .arg("-v")
            .arg(format!("{container}-data:/var/lib/postgresql/data"))
            .args([
                "postgres:17-alpine",
                "postgres",
                "-c",
                "shared_preload_libraries=pg_stat_statements",
            ])
            .status()?;
        if !status.success() {
            bail!("could not create the {container} container");
        }
    } else if String::from_utf8_lossy(&running.stdout).trim() != "true" {
        Command::new("docker")
            .args(["start", &container])
            .status()?;
    }

    let config = config_for(&state, args.listen);
    wait_for_postgres(&config)?;
    ok(&format!("Postgres at 127.0.0.1:{}", state.db_port));

    let keys = nelcota_auth::Keys::new(config.jwt_private_key(), None)?;
    let service = keys.service_role_token(&config.jwt_issuer, 30)?;
    println!();
    println!("  API:      http://{}/rest/v1/", args.listen);
    println!("  Auth:     http://{}/auth/v1/", args.listen);
    println!("  Postgres: {}", config.database_url.expose());
    println!("  service_role (30 days, bypasses RLS): {service}");
    println!();
    println!("  Migrations: nelcota migrate   ·   Types: nelcota types -o database.ts");
    println!();
    Ok(Outcome::Serve(Box::new(config)))
}

fn wait_for_postgres(config: &Config) -> anyhow::Result<()> {
    let db = config.database_config()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(60);
    runtime.block_on(async {
        loop {
            match db.connect(NoTls).await {
                Ok(_) => return Ok(()),
                Err(_) if Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(err) => bail!("the development Postgres did not answer: {err}"),
            }
        }
    })
}
