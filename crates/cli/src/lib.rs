//! Comandos do binário `nelcota`.
//!
//! O mesmo binário serve a API (`nelcota serve`, o padrão) e opera o host:
//! uma pasta com um Caddy compartilhado e N projetos isolados (cada um com o
//! próprio Postgres, app, chaves e backups). Comandos que precisam do banco
//! rodam direto quando há `NELCOTA_DATABASE_URL` no ambiente (dentro do
//! container ou num shell de dev) e, num host, são repassados ao container
//! `app` do projeto com `docker compose exec`.

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
    about = "BaaS sobre Postgres puro: API REST, auth e deploy num binário só."
)]
pub struct Cli {
    /// Pasta do host (onde ficam nelcota-host.json, caddy/ e projects/).
    #[arg(
        short = 'C',
        long = "dir",
        global = true,
        env = "NELCOTA_ROOT",
        default_value = "."
    )]
    pub dir: PathBuf,

    /// Projeto (obrigatório quando o host tem mais de um).
    #[arg(short = 'p', long = "project", global = true)]
    pub project: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Sobe o servidor HTTP (padrão quando nenhum comando é dado).
    Serve,
    /// Cria um projeto (e o host, na primeira vez).
    Init(Box<InitArgs>),
    /// Lista os projetos do host e o estado de cada um.
    Projects,
    /// Sobe os projetos e o Caddy (todos, ou só o do -p).
    Up,
    /// Para um projeto (ou todos com --all). Com --volumes, APAGA os dados.
    Down {
        #[arg(long)]
        volumes: bool,
        #[arg(long)]
        all: bool,
    },
    /// Estado dos projetos (todos, ou só o do -p).
    Status,
    /// Logs de um projeto.
    Logs {
        #[arg(short, long)]
        follow: bool,
        /// postgres ou app (padrão: os dois).
        service: Option<String>,
    },
    /// Remove um projeto: backup final em archive/, containers e dados apagados.
    Remove {
        /// Não pede confirmação.
        #[arg(long)]
        yes: bool,
        /// Mantém a pasta do projeto (migrations, backups).
        #[arg(long)]
        keep_files: bool,
    },
    /// Login dos painéis: um para todos (shared) ou um por projeto.
    PanelLogin {
        #[arg(value_enum)]
        mode: PanelLogin,
    },
    /// Ambiente local de desenvolvimento (Postgres em container + servidor).
    Dev(DevArgs),
    /// Aplica migrações SQL do diretório (arquivos V<n>__<nome>.sql).
    Migrate {
        #[arg(long, default_value = "migrations")]
        path: PathBuf,
    },
    /// Atualiza a imagem do app com backup antes e rollback se falhar.
    Upgrade {
        /// Versão alvo (padrão: a versão deste binário).
        #[arg(long)]
        version: Option<String>,
        /// Todos os projetos, um por vez.
        #[arg(long)]
        all: bool,
    },
    /// Gera um dump do banco em backups/ (e envia ao S3 com --upload).
    Backup {
        #[arg(long)]
        upload: bool,
        /// Mantém só os N dumps locais mais recentes.
        #[arg(long)]
        keep: Option<usize>,
        /// Todos os projetos.
        #[arg(long)]
        all: bool,
    },
    /// Restaura um dump (substitui o banco atual do projeto).
    Restore {
        file: PathBuf,
        /// Não pede confirmação.
        #[arg(long)]
        yes: bool,
    },
    /// Gera tipos TypeScript a partir do schema exposto.
    Types {
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Emite tokens de serviço.
    Token {
        #[command(subcommand)]
        kind: TokenKind,
    },
    /// Gera uma chave privada Ed25519 nova (para NELCOTA_JWT_PRIVATE_KEY).
    Keygen,
    /// Senha nova para o painel (do host no login único; do projeto no login por projeto).
    AdminPassword,
    /// Healthcheck local (usado pelo Docker): sai com 0 se /health responde 200.
    Healthcheck {
        #[arg(long, default_value = "127.0.0.1:8000")]
        addr: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TokenKind {
    /// JWT com role service_role (IGNORA O RLS: nunca use no frontend).
    ServiceRole {
        #[arg(long, default_value_t = 3650)]
        days: u64,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Domínio do projeto (ex.: api.loja.com). Sem ele, use --project com um
    /// domínio base (subdomínio) ou --local.
    pub domain: Option<String>,
    /// Nome do projeto (padrão: derivado do domínio, ex.: api.loja.com → loja).
    #[arg(long)]
    pub project: Option<String>,
    /// Domínio base do host: `--project loja` vira `loja.<base>`.
    #[arg(long)]
    pub base_domain: Option<String>,
    /// Host local, sem domínio público: HTTPS em https://<projeto>.localhost.
    #[arg(long)]
    pub local: bool,
    /// Login dos painéis ao criar o host: um para todos (padrão) ou por projeto.
    #[arg(long, value_enum, default_value_t = PanelLogin::Shared)]
    pub panel_login: PanelLogin,
    /// Email do administrador (padrão: admin@<domínio>).
    #[arg(long)]
    pub email: Option<String>,
    /// Não faz perguntas (usa os padrões e as flags).
    #[arg(long, short)]
    pub yes: bool,
    /// Imagem do app.
    #[arg(long, env = "NELCOTA_IMAGE", default_value = "ghcr.io/nelcota/nelcota")]
    pub image: String,
    /// Tag da imagem (padrão: a versão deste binário).
    #[arg(long)]
    pub version: Option<String>,
    /// Perfil do Postgres: 1gb, 2gb, 4gb ou 8gb (padrão: pela RAM dividida entre os projetos).
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
    /// Configura o ufw (libera SSH, 80 e 443 e ativa).
    #[arg(long)]
    pub firewall: bool,
    /// Pula as checagens de ambiente.
    #[arg(long)]
    pub skip_checks: bool,
}

#[derive(Args, Debug)]
pub struct DevArgs {
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub listen: std::net::SocketAddr,
    /// Porta local do Postgres de desenvolvimento.
    #[arg(long, default_value_t = 54322)]
    pub db_port: u16,
}

/// O que o `main` deve fazer depois do comando.
pub enum Outcome {
    Done,
    /// Subir o servidor HTTP com esta configuração.
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
                bail!("informe o projeto a remover: nelcota -p <nome> remove");
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
                // Um projeto com problema não impede o backup dos outros.
                if let Err(err) = ops::backup(&host, project, upload, keep) {
                    util::warn(&format!("{err:#}"));
                    failed.push(project.name.clone());
                }
            }
            if !failed.is_empty() {
                bail!("backup falhou em: {}", failed.join(", "));
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
