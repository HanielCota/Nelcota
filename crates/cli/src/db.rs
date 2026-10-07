//! Comandos que falam com o banco: migrate, types e token.
//!
//! Rodam direto quando há configuração no ambiente (container, dev); num
//! host do `nelcota init`, são repassados ao container `app` do projeto.

use std::{fs, path::Path};

use anyhow::{Context, bail};
use nelcota_core::Config;
use refinery::{Migration, Runner};
use tokio_postgres::NoTls;

use crate::{
    dev,
    host::Host,
    project::Project,
    util::{ok, step},
};

/// Tabela de controle das migrações do usuário (documentada em docs/deploy.md).
pub const USER_MIGRATIONS_TABLE: &str = "nelcota.user_migrations";

enum Target {
    Direct(Box<Config>),
    Container(Project),
}

fn target(host: &Host, selection: Option<&str>) -> anyhow::Result<Target> {
    if std::env::var_os("NELCOTA_DATABASE_URL").is_some() {
        return Ok(Target::Direct(Box::new(Config::load()?)));
    }
    if host.exists() {
        let manifest = host.manifest()?;
        return Ok(Target::Container(host.select(&manifest, selection)?));
    }
    if let Some(config) = dev::saved_config(host.root())? {
        return Ok(Target::Direct(Box::new(config)));
    }
    bail!(
        "sem banco configurado: defina NELCOTA_DATABASE_URL, rode num projeto do `nelcota init` \
         ou inicie o ambiente com `nelcota dev`"
    )
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

async fn connect(config: &Config) -> anyhow::Result<tokio_postgres::Client> {
    let (client, connection) = config
        .database_config()?
        .connect(NoTls)
        .await
        .context("falha ao conectar no Postgres")?;
    tokio::spawn(connection);
    Ok(client)
}

/// Lê `V<n>__<nome>.sql` do diretório. CRLF vira LF para o checksum não
/// depender do sistema operacional de quem fez checkout.
fn load_migrations(dir: &Path) -> anyhow::Result<Vec<Migration>> {
    if !dir.is_dir() {
        bail!("diretório de migrações não encontrado: {}", dir.display());
    }
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    paths.sort();
    let mut migrations = Vec::with_capacity(paths.len());
    for path in paths {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("nome de arquivo inválido")?;
        let sql = fs::read_to_string(&path)?.replace("\r\n", "\n");
        migrations.push(
            Migration::unapplied(stem, &sql)
                .with_context(|| format!("nome inválido: {stem} (use V<n>__<nome>.sql)"))?,
        );
    }
    Ok(migrations)
}

pub fn migrate(host: &Host, selection: Option<&str>, dir: &Path) -> anyhow::Result<()> {
    match target(host, selection)? {
        Target::Container(project) => {
            // O compose monta ./migrations em /migrations (somente leitura).
            if dir != Path::new("migrations") {
                bail!("num projeto do `nelcota init`, as migrações ficam em ./migrations");
            }
            project.in_app(&["migrate", "--path", "/migrations"])
        }
        Target::Direct(config) => {
            let migrations = load_migrations(dir)?;
            if migrations.is_empty() {
                println!("Nenhuma migração em {}.", dir.display());
                return Ok(());
            }
            runtime()?.block_on(async {
                let mut client = connect(&config).await?;
                client
                    .batch_execute("CREATE SCHEMA IF NOT EXISTS nelcota")
                    .await?;
                let mut runner = Runner::new(&migrations).set_abort_divergent(true);
                runner.set_migration_table_name(USER_MIGRATIONS_TABLE);
                step(&format!("Aplicando migrações de {}", dir.display()));
                let report = runner.run_async(&mut client).await?;
                if report.applied_migrations().is_empty() {
                    ok("nada novo: o banco já está atualizado");
                }
                for migration in report.applied_migrations() {
                    ok(&migration.to_string());
                }
                // Recarga do catálogo mesmo sem event trigger.
                client
                    .batch_execute("NOTIFY nelcota, 'reload schema'")
                    .await?;
                Ok(())
            })
        }
    }
}

pub fn types(host: &Host, selection: Option<&str>, out: Option<&Path>) -> anyhow::Result<()> {
    let code = match target(host, selection)? {
        Target::Container(project) => project.in_app_output(&["types"])?,
        Target::Direct(config) => runtime()?.block_on(async {
            let client = connect(&config).await?;
            let catalog = nelcota_api::Catalog::load(&client, &config.db_schema).await?;
            Ok::<_, anyhow::Error>(nelcota_api::typescript::generate(&catalog))
        })?,
    };
    match out {
        Some(path) => {
            fs::write(path, code)?;
            ok(&format!("tipos gravados em {}", path.display()));
        }
        None => print!("{code}"),
    }
    Ok(())
}

pub fn service_role_token(host: &Host, selection: Option<&str>, days: u64) -> anyhow::Result<()> {
    match target(host, selection)? {
        Target::Container(project) => {
            let token =
                project.in_app_output(&["token", "service-role", "--days", &days.to_string()])?;
            print!("{token}");
        }
        Target::Direct(config) => {
            let keys = nelcota_auth::Keys::new(
                config.jwt_private_key(),
                config.jwt_secret().map(str::as_bytes),
            )?;
            let token = keys.service_role_token(&config.jwt_issuer, days)?;
            eprintln!(
                "ATENÇÃO: este token ignora o RLS. Use só no seu backend, nunca no frontend."
            );
            println!("{token}");
        }
    }
    Ok(())
}
