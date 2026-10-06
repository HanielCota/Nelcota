//! Comandos do binário `nelcota`.
//!
//! O mesmo binário serve a API (`nelcota serve`, o padrão) e opera a
//! instalação (`init`, `up`, `migrate`, `backup`...). Comandos que precisam do
//! banco rodam direto quando há `NELCOTA_DATABASE_URL` no ambiente (dentro do
//! container ou num shell de dev) e, num projeto criado com `nelcota init`,
//! são repassados ao container `app` com `docker compose exec`.

mod db;
mod dev;
mod init;
mod ops;
mod project;
mod util;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use nelcota_core::Config;

#[derive(Parser, Debug)]
#[command(
    name = "nelcota",
    version,
    about = "BaaS sobre Postgres puro: API REST, auth e deploy num binário só."
)]
pub struct Cli {
    /// Diretório do projeto (onde ficam docker-compose.yml e .env).
    #[arg(short = 'C', long = "dir", global = true, default_value = ".")]
    pub dir: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Sobe o servidor HTTP (padrão quando nenhum comando é dado).
    Serve,
    /// Prepara uma instalação: checa o ambiente, gera segredos e arquivos.
    Init(Box<InitArgs>),
    /// Sobe os containers (postgres, app, caddy).
    Up,
    /// Para os containers. Com --volumes, APAGA os dados.
    Down {
        #[arg(long)]
        volumes: bool,
    },
    /// Mostra o estado dos containers e o healthcheck da API.
    Status,
    /// Logs dos containers.
    Logs {
        #[arg(short, long)]
        follow: bool,
        /// postgres, app ou caddy (padrão: todos).
        service: Option<String>,
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
    },
    /// Gera um dump do banco em backups/ (e envia ao S3 com --upload).
    Backup {
        #[arg(long)]
        upload: bool,
        /// Mantém só os N dumps locais mais recentes.
        #[arg(long)]
        keep: Option<usize>,
    },
    /// Restaura um dump (substitui o banco atual).
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
    /// Gera uma senha nova para o admin do painel e reinicia o app.
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
    /// Domínio que vai apontar para esta máquina (ex.: api.meuapp.com).
    pub domain: Option<String>,
    /// Instalação local, sem domínio: HTTPS em https://localhost (certificado interno).
    #[arg(long)]
    pub local: bool,
    /// Email do administrador do painel (padrão: admin@<domínio>).
    #[arg(long)]
    pub email: Option<String>,
    /// Não faz perguntas (usa os padrões e as flags).
    #[arg(long, short)]
    pub yes: bool,
    /// Sobrescreve uma instalação existente neste diretório.
    #[arg(long)]
    pub force: bool,
    /// Imagem do app.
    #[arg(long, env = "NELCOTA_IMAGE", default_value = "ghcr.io/nelcota/nelcota")]
    pub image: String,
    /// Tag da imagem (padrão: a versão deste binário).
    #[arg(long)]
    pub version: Option<String>,
    /// Perfil do Postgres: 1gb, 2gb, 4gb ou 8gb (padrão: pela RAM da máquina).
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
    let project = project::Project::new(&cli.dir);
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => Ok(Outcome::Serve(Box::new(Config::load()?))),
        Command::Init(args) => init::run(&project, *args).map(|()| Outcome::Done),
        Command::Up => ops::up(&project).map(|()| Outcome::Done),
        Command::Down { volumes } => ops::down(&project, volumes).map(|()| Outcome::Done),
        Command::Status => ops::status(&project).map(|()| Outcome::Done),
        Command::Logs { follow, service } => {
            ops::logs(&project, follow, service.as_deref()).map(|()| Outcome::Done)
        }
        Command::Dev(args) => dev::run(&project, args),
        Command::Migrate { path } => db::migrate(&project, &path).map(|()| Outcome::Done),
        Command::Upgrade { version } => ops::upgrade(&project, version).map(|()| Outcome::Done),
        Command::Backup { upload, keep } => {
            ops::backup(&project, upload, keep).map(|_| Outcome::Done)
        }
        Command::Restore { file, yes } => {
            ops::restore(&project, &file, yes).map(|()| Outcome::Done)
        }
        Command::Types { out } => db::types(&project, out.as_deref()).map(|()| Outcome::Done),
        Command::Token {
            kind: TokenKind::ServiceRole { days },
        } => db::service_role_token(&project, days).map(|()| Outcome::Done),
        Command::AdminPassword => ops::admin_password(&project).map(|()| Outcome::Done),
        Command::Keygen => {
            println!("{}", nelcota_auth::generate_ed25519_private_key());
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
