//! Operação dos projetos: up/down/status/logs, backup/restore e upgrade com
//! rollback automático.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, bail};

use crate::{
    caddy,
    host::{Host, Manifest},
    project::Project,
    util::{self, ok, step, warn},
};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Sobe os projetos (cada um até o healthcheck) e o Caddy.
pub fn up(host: &Host, manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    caddy::ensure_network()?;
    caddy::write(host, manifest)?;
    for project in projects {
        step(&format!("Subindo {}", project.name));
        project.compose_ok(&["up", "-d"])?;
        project.wait_healthy("app", HEALTH_TIMEOUT)?;
        ok(&format!("{} saudável", project.name));
    }
    caddy::up(host)?;
    println!();
    for entry in manifest
        .projects
        .iter()
        .filter(|e| projects.iter().any(|p| p.name == e.name))
    {
        println!("  {:<16} {}/health", entry.name, entry.url());
    }
    Ok(())
}

pub fn down(project: &Project, volumes: bool) -> anyhow::Result<()> {
    if volumes {
        warn(&format!(
            "--volumes: os dados de {} serão APAGADOS",
            project.name
        ));
        project.compose_ok(&["down", "--volumes"])
    } else {
        project.compose_ok(&["down"])
    }
}

pub fn status(manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    println!("{:<16} {:<32} {:<10} VERSÃO", "PROJETO", "DOMÍNIO", "APP");
    for project in projects {
        let domain = manifest
            .projects
            .iter()
            .find(|e| e.name == project.name)
            .map_or("", |e| e.domain.as_str());
        let health = project.health("app").unwrap_or_else(|| "parado".into());
        let version = project.env().get("NELCOTA_VERSION")?.unwrap_or_default();
        println!(
            "{:<16} {:<32} {:<10} {}",
            project.name, domain, health, version
        );
    }
    Ok(())
}

pub fn logs(project: &Project, follow: bool, service: Option<&str>) -> anyhow::Result<()> {
    let mut args = vec!["logs", "--tail", "200"];
    if follow {
        args.push("-f");
    }
    if let Some(service) = service {
        args.push(service);
    }
    project.compose_ok(&args)
}

/// Recria os apps já criados (para aplicar mudanças no `.env`).
pub fn recreate_apps(projects: &[Project]) -> anyhow::Result<()> {
    for project in projects.iter().filter(|p| p.has_container("app")) {
        step(&format!("Reiniciando o app de {}", project.name));
        project.compose_ok(&["up", "-d", "--force-recreate", "app"])?;
        project.wait_healthy("app", HEALTH_TIMEOUT)?;
    }
    Ok(())
}

/// `pg_dump -Fc` dentro do container do Postgres, gravado em `backups/`.
pub fn backup(
    host: &Host,
    project: &Project,
    upload: bool,
    keep: Option<usize>,
) -> anyhow::Result<PathBuf> {
    let dir = project.path("backups");
    fs::create_dir_all(&dir)?;
    let name = format!("nelcota-{}-{}.dump", project.name, util::timestamp());
    let path = dir.join(&name);
    step(&format!("Backup de {}: {name}", project.name));

    let file = fs::File::create(&path)?;
    let status = project.compose_piped(
        &[
            "exec", "-T", "postgres", "pg_dump", "-U", "postgres", "-Fc", "postgres",
        ],
        Stdio::null(),
        Stdio::from(file),
    )?;
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if !status.success() || size == 0 {
        let _ = fs::remove_file(&path);
        bail!("[{}] pg_dump falhou ({status})", project.name);
    }
    ok(&format!("{} ({} KB)", path.display(), size / 1024));

    if upload {
        upload_s3(host, &project.name, &path, &name)?;
    }
    if let Some(keep) = keep {
        prune(&dir, keep)?;
    }
    Ok(path)
}

/// Envia ao S3-compatible (credenciais do `host.env`), em `<bucket>/<projeto>/`.
fn upload_s3(host: &Host, project: &str, path: &Path, name: &str) -> anyhow::Result<()> {
    let secrets = host.secrets();
    let get = |key: &str| -> anyhow::Result<String> {
        secrets
            .get(key)?
            .with_context(|| format!("{key} não configurado no host.env (veja docs/backup.md)"))
    };
    let endpoint = get("NELCOTA_BACKUP_S3_ENDPOINT")?;
    let bucket = get("NELCOTA_BACKUP_S3_BUCKET")?;
    let access_key = get("NELCOTA_BACKUP_S3_ACCESS_KEY")?;
    let secret_key = get("NELCOTA_BACKUP_S3_SECRET_KEY")?;
    let region = secrets
        .get("NELCOTA_BACKUP_S3_REGION")?
        .unwrap_or_else(|| "us-east-1".into());
    let dir = fs::canonicalize(path.parent().unwrap_or(Path::new(".")))?;

    step(&format!("Enviando para s3://{bucket}/{project}/{name}"));
    let status = Command::new("docker")
        .args(["run", "--rm", "-v"])
        .arg(format!("{}:/backups:ro", dir.display()))
        .args([
            "-e",
            "AWS_ACCESS_KEY_ID",
            "-e",
            "AWS_SECRET_ACCESS_KEY",
            "-e",
            "AWS_DEFAULT_REGION",
        ])
        .env("AWS_ACCESS_KEY_ID", access_key)
        .env("AWS_SECRET_ACCESS_KEY", secret_key)
        .env("AWS_DEFAULT_REGION", region)
        .args(["amazon/aws-cli", "s3", "cp"])
        .arg(format!("/backups/{name}"))
        .arg(format!("s3://{bucket}/{project}/{name}"))
        .args(["--endpoint-url", &endpoint])
        .status()
        .context("falha ao executar o aws-cli")?;
    if !status.success() {
        bail!("upload para o S3 falhou ({status})");
    }
    ok("enviado");
    Ok(())
}

fn prune(dir: &Path, keep: usize) -> anyhow::Result<()> {
    let mut dumps: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "dump"))
        .collect();
    dumps.sort();
    let excess = dumps.len().saturating_sub(keep);
    for old in dumps.into_iter().take(excess) {
        fs::remove_file(&old)?;
        ok(&format!("removido {}", old.display()));
    }
    Ok(())
}

/// Restaura um dump: para o app, recria os objetos numa transação só e sobe
/// o app de novo.
pub fn restore(project: &Project, file: &Path, yes: bool) -> anyhow::Result<()> {
    if !file.is_file() {
        bail!("arquivo não encontrado: {}", file.display());
    }
    if !yes
        && !(util::interactive()
            && util::confirm(&format!(
                "Isso SUBSTITUI o banco de {} pelo conteúdo de {}. Continuar?",
                project.name,
                file.display()
            )))
    {
        bail!("restore cancelado (use --yes para não perguntar)");
    }
    step(&format!("Parando o app de {}", project.name));
    project.compose_ok(&["stop", "app"])?;
    step(&format!("Restaurando {}", file.display()));
    let result = restore_dump(project, file);
    step("Subindo o app");
    project.compose_ok(&["up", "-d", "app"])?;
    result?;
    project.wait_healthy("app", HEALTH_TIMEOUT)?;
    ok("restore concluído e app saudável");
    Ok(())
}

fn restore_dump(project: &Project, file: &Path) -> anyhow::Result<()> {
    let input = fs::File::open(file)?;
    let status = project.compose_piped(
        &[
            "exec",
            "-T",
            "postgres",
            "pg_restore",
            "-U",
            "postgres",
            "-d",
            "postgres",
            "--clean",
            "--if-exists",
            "--single-transaction",
            "--exit-on-error",
        ],
        Stdio::from(input),
        Stdio::inherit(),
    )?;
    if !status.success() {
        bail!("pg_restore falhou ({status}); o banco não foi alterado (transação única)");
    }
    ok("dados restaurados");
    Ok(())
}

/// Backup → nova imagem → healthcheck. Se falhar: volta a imagem anterior e
/// restaura o backup (a versão nova pode ter aplicado migrações).
pub fn upgrade(host: &Host, project: &Project, version: Option<&str>) -> anyhow::Result<()> {
    let env = project.env();
    let current = env
        .get("NELCOTA_VERSION")?
        .context("NELCOTA_VERSION ausente no .env")?;
    let target = version.map_or_else(|| env!("CARGO_PKG_VERSION").to_owned(), str::to_owned);
    println!("Atualizando {}: {current} → {target}", project.name);

    let dump = backup(host, project, false, None)
        .context("backup pré-upgrade falhou; nada foi alterado")?;

    env.set("NELCOTA_VERSION", &target)?;
    step("Baixando a imagem nova");
    if !project
        .compose(&["pull", "app"])
        .map(|s| s.success())
        .unwrap_or(false)
    {
        warn("não foi possível baixar a imagem (seguindo com a imagem local, se existir)");
    }
    step("Reiniciando o app na versão nova");
    let healthy = project
        .compose_ok(&["up", "-d", "app"])
        .and_then(|()| project.wait_healthy("app", HEALTH_TIMEOUT));

    match healthy {
        Ok(()) => {
            ok(&format!("{} saudável na versão {target}", project.name));
            println!("Backup pré-upgrade: {}", dump.display());
            Ok(())
        }
        Err(err) => {
            warn(&format!("a versão {target} não ficou saudável: {err}"));
            step(&format!("Rollback para {current}"));
            env.set("NELCOTA_VERSION", &current)?;
            project.compose_ok(&["stop", "app"])?;
            restore_dump(project, &dump)?;
            project.compose_ok(&["up", "-d", "app"])?;
            project.wait_healthy("app", HEALTH_TIMEOUT)?;
            ok(&format!(
                "rollback concluído: versão {current}, banco restaurado"
            ));
            bail!(
                "upgrade de {} para {target} falhou e foi revertido",
                project.name
            )
        }
    }
}
