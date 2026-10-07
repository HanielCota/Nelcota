//! Commands of the `nelcota` binary.
//!
//! The same binary serves the API (`nelcota serve`, the default) and operates
//! the host: a folder with a shared Caddy and N isolated projects (each with its
//! own Postgres, app, keys and backups). Commands that need the database run
//! directly when `NELCOTA_DATABASE_URL` is in the environment (inside the
//! container or in a dev shell) and, on a host, are forwarded to the project's
//! `app` container with `docker compose exec`.

mod caddy;
mod checks;
mod db;
mod dev;
mod envfile;
mod host;
mod init;
mod machine;
mod naming;
mod ops;
mod panel_login;
mod pitr;
mod project;
mod projects;
mod registry;
mod scaffold;
mod util;

use std::path::PathBuf;

use anyhow::bail;
use clap::{Args, Parser, Subcommand};
use nelcota_core::Config;

pub use host::PanelLogin;

#[derive(Parser, Debug)]
#[command(
    name = "nelcota",
    version,
    about = "BaaS on plain Postgres: REST API, auth and deploy in a single binary."
)]
pub struct Cli {
    /// Host folder (where nelcota-host.json, caddy/ and projects/ live).
    #[arg(
        short = 'C',
        long = "dir",
        global = true,
        env = "NELCOTA_ROOT",
        default_value = "."
    )]
    pub dir: PathBuf,

    /// Project (required when the host has more than one).
    #[arg(short = 'p', long = "project", global = true)]
    pub project: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Starts the HTTP server (the default when no command is given).
    Serve,
    /// Creates a project (and the host, the first time).
    Init(Box<InitArgs>),
    /// Lists the host's projects and the state of each one.
    Projects,
    /// Starts the projects and Caddy (all of them, or only the one from -p).
    Up,
    /// Stops a project (or all with --all). With --volumes, DELETES the data.
    Down {
        #[arg(long)]
        volumes: bool,
        #[arg(long)]
        all: bool,
    },
    /// State of the projects (all of them, or only the one from -p).
    Status,
    /// Logs of a project.
    Logs {
        #[arg(short, long)]
        follow: bool,
        /// postgres or app (default: both).
        service: Option<String>,
    },
    /// Removes a project: final backup in archive/, containers and data deleted.
    Remove {
        /// Does not ask for confirmation.
        #[arg(long)]
        yes: bool,
        /// Keeps the project folder (migrations, backups).
        #[arg(long)]
        keep_files: bool,
    },
    /// Panel login: one for all (shared) or one per project.
    PanelLogin {
        #[arg(value_enum)]
        mode: PanelLogin,
    },
    /// Local development environment (Postgres in a container + server).
    Dev(DevArgs),
    /// Applies SQL migrations from the directory (files V<n>__<name>.sql).
    Migrate {
        #[arg(long, default_value = "migrations")]
        path: PathBuf,
    },
    /// Updates the app image, with a backup first and a rollback if it fails.
    Upgrade {
        /// Target version (default: this binary's version).
        #[arg(long)]
        version: Option<String>,
        /// All projects, one at a time.
        #[arg(long)]
        all: bool,
    },
    /// Dumps the database to backups/ (and uploads it to S3 with --upload).
    Backup {
        #[arg(long)]
        upload: bool,
        /// Keeps only the N most recent local dumps.
        #[arg(long)]
        keep: Option<usize>,
        /// All projects.
        #[arg(long)]
        all: bool,
    },
    /// Restores a dump (replaces the project's current database).
    Restore {
        file: PathBuf,
        /// Does not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Point-in-time recovery: WAL archived to S3 with pgBackRest.
    Pitr {
        #[command(subcommand)]
        action: PitrAction,
    },
    /// Generates TypeScript types from the exposed schema.
    Types {
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Issues service tokens.
    Token {
        #[command(subcommand)]
        kind: TokenKind,
    },
    /// Generates a new Ed25519 private key (for NELCOTA_JWT_PRIVATE_KEY).
    Keygen,
    /// New panel password (the host's with single sign-on; the project's with per-project login).
    AdminPassword,
    /// Local healthcheck (used by Docker): exits with 0 if /health answers 200.
    Healthcheck {
        #[arg(long, default_value = "127.0.0.1:8000")]
        addr: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TokenKind {
    /// JWT with the service_role role (BYPASSES RLS: never use it in a frontend).
    ServiceRole {
        #[arg(long, default_value_t = 3650)]
        days: u64,
    },
}

#[derive(Subcommand, Debug)]
pub enum PitrAction {
    /// Archives every WAL segment to the host's S3 and takes a first full backup.
    Enable,
    /// Stops archiving (the backups already in S3 are kept).
    Disable,
    /// Base backups and the time range that can be restored.
    Status,
    /// Takes a base backup now (full on Sundays, differential otherwise).
    Backup,
    /// Restores the database to a moment (REPLACES the current data).
    Restore {
        /// Target moment, e.g. "2026-10-07 14:30:00+00" (default: the last archived write).
        #[arg(long)]
        time: Option<String>,
        /// Does not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Project domain (e.g. api.shop.com). Without it, use --project with a
    /// base domain (subdomain) or --local.
    pub domain: Option<String>,
    /// Project name (default: derived from the domain, e.g. api.shop.com → shop).
    #[arg(long)]
    pub project: Option<String>,
    /// Host base domain: `--project shop` becomes `shop.<base>`.
    #[arg(long)]
    pub base_domain: Option<String>,
    /// Local host, without a public domain: HTTPS at https://<project>.localhost.
    #[arg(long)]
    pub local: bool,
    /// Panel login when creating the host: one for all (default) or per project.
    #[arg(long, value_enum, default_value_t = PanelLogin::Shared)]
    pub panel_login: PanelLogin,
    /// Administrator email (default: admin@<domain>).
    #[arg(long)]
    pub email: Option<String>,
    /// Asks no questions (uses the defaults and the flags).
    #[arg(long, short)]
    pub yes: bool,
    /// App image.
    #[arg(
        long,
        env = "NELCOTA_IMAGE",
        default_value = "ghcr.io/hanielcota/nelcota"
    )]
    pub image: String,
    /// Image tag (default: this binary's version).
    #[arg(long)]
    pub version: Option<String>,
    /// Postgres profile: 1gb, 2gb, 4gb or 8gb (default: by the RAM split across the projects).
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub s3_endpoint: Option<String>,
    #[arg(long)]
    pub s3_bucket: Option<String>,
    #[arg(long)]
    pub s3_access_key: Option<String>,
    #[arg(long)]
    pub s3_secret_key: Option<String>,
    #[arg(long, default_value = "us-east-1")]
    pub s3_region: String,
    /// Configures ufw (allows SSH, 80 and 443 and enables it).
    #[arg(long)]
    pub firewall: bool,
    /// Skips the environment checks.
    #[arg(long)]
    pub skip_checks: bool,
}

#[derive(Args, Debug)]
pub struct DevArgs {
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub listen: std::net::SocketAddr,
    /// Local port of the development Postgres.
    #[arg(long, default_value_t = 54322)]
    pub db_port: u16,
}

/// What `main` should do after the command.
pub enum Outcome {
    Done,
    /// Start the HTTP server with this configuration.
    Serve(Box<Config>),
}

pub fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let host = host::Host::new(&cli.dir);
    let selection = cli.project.as_deref();
    let done = |r: anyhow::Result<()>| r.map(|()| Outcome::Done);

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => Ok(Outcome::Serve(Box::new(Config::load()?))),
        Command::Init(mut args) => {
            if args.project.is_none() {
                args.project = cli.project.clone();
            }
            done(init::run(&host, *args))
        }
        Command::Projects => done(projects::list(&host)),
        Command::Up => {
            let manifest = host.require()?;
            let targets = match selection {
                Some(_) => vec![host.select(&manifest, selection)?],
                None => host.projects(&manifest),
            };
            done(ops::up(&host, &manifest, &targets))
        }
        Command::Down { volumes, all } => {
            let manifest = host.require()?;
            if all {
                for project in host.projects(&manifest) {
                    ops::down(&project, volumes)?;
                }
                done(caddy::down(&host, volumes))
            } else {
                done(ops::down(&host.select(&manifest, selection)?, volumes))
            }
        }
        Command::Status => {
            let manifest = host.require()?;
            let targets = match selection {
                Some(_) => vec![host.select(&manifest, selection)?],
                None => host.projects(&manifest),
            };
            done(ops::status(&manifest, &targets))
        }
        Command::Logs { follow, service } => {
            let manifest = host.require()?;
            done(ops::logs(
                &host.select(&manifest, selection)?,
                follow,
                service.as_deref(),
            ))
        }
        Command::Remove { yes, keep_files } => {
            let Some(name) = selection else {
                bail!("name the project to remove: nelcota -p <name> remove");
            };
            done(projects::remove(&host, name, yes, keep_files))
        }
        Command::PanelLogin { mode } => done(projects::set_panel_login(&host, mode)),
        Command::Dev(args) => dev::run(&cli.dir, args),
        Command::Migrate { path } => done(db::migrate(&host, selection, &path)),
        Command::Upgrade { version, all } => {
            let manifest = host.require()?;
            if all {
                for project in host.projects(&manifest) {
                    ops::upgrade(&host, &project, version.as_deref())?;
                }
                Ok(Outcome::Done)
            } else {
                done(ops::upgrade(
                    &host,
                    &host.select(&manifest, selection)?,
                    version.as_deref(),
                ))
            }
        }
        Command::Backup { upload, keep, all } => {
            let manifest = host.require()?;
            let targets = if all {
                host.projects(&manifest)
            } else {
                vec![host.select(&manifest, selection)?]
            };
            let mut failed = Vec::new();
            for project in &targets {
                // A project with a problem does not stop the others from being backed up.
                let result = ops::backup(&host, project, upload, keep).and_then(|_| {
                    if pitr::enabled(project) {
                        pitr::backup(project)?;
                    }
                    Ok(())
                });
                if let Err(err) = result {
                    util::warn(&format!("{err:#}"));
                    failed.push(project.name.clone());
                }
            }
            if !failed.is_empty() {
                bail!("backup failed for: {}", failed.join(", "));
            }
            Ok(Outcome::Done)
        }
        Command::Restore { file, yes } => {
            let manifest = host.require()?;
            done(ops::restore(
                &host.select(&manifest, selection)?,
                &file,
                yes,
            ))
        }
        Command::Pitr { action } => {
            let manifest = host.require()?;
            let project = host.select(&manifest, selection)?;
            done(match action {
                PitrAction::Enable => pitr::enable(&host, &project),
                PitrAction::Disable => pitr::disable(&project),
                PitrAction::Status => pitr::status(&project),
                PitrAction::Backup => pitr::backup(&project),
                PitrAction::Restore { time, yes } => pitr::restore(&project, time.as_deref(), yes),
            })
        }
        Command::Types { out } => done(db::types(&host, selection, out.as_deref())),
        Command::Token {
            kind: TokenKind::ServiceRole { days },
        } => done(db::service_role_token(&host, selection, days)),
        Command::Keygen => {
            println!("{}", nelcota_auth::generate_ed25519_private_key());
            Ok(Outcome::Done)
        }
        Command::AdminPassword => {
            let manifest = host.require()?;
            let project = match manifest.panel_login {
                PanelLogin::Shared => None,
                PanelLogin::PerProject => Some(host.select(&manifest, selection)?),
            };
            let new = panel_login::reset(&host, &manifest, project.as_ref())?;
            let affected = match project {
                Some(project) => vec![project],
                None => host.projects(&manifest),
            };
            ops::recreate_apps(&affected)?;
            panel_login::print(&[new]);
            Ok(Outcome::Done)
        }
        Command::Healthcheck { addr } => {
            if util::healthcheck(&addr) {
                Ok(Outcome::Done)
            } else {
                std::process::exit(1)
            }
        }
    }
}
