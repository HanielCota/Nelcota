//! Operação da instalação: up/down/status/logs, backup/restore e upgrade com
//! rollback automático.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, bail};

use crate::{
    project::Project,
    util::{self, ok, step, warn},
};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

pub fn up(project: &Project) -> anyhow::Result<()> {
    project.require()?;
    step("Subindo postgres, app e caddy");
    project.compose_ok(&["up", "-d"])?;
    project.wait_healthy("app", HEALTH_TIMEOUT)?;
    ok("app saudável");
    let domain = project
        .env_value("NELCOTA_DOMAIN")?
        .unwrap_or_else(|| "localhost".into());
    println!();
    println!("No ar: https://{domain}/health");
    Ok(())
}

pub fn down(project: &Project, volumes: bool) -> anyhow::Result<()> {
    project.require()?;
    if volumes {
        warn("--volumes: os dados do Postgres e os certificados serão APAGADOS");
        project.compose_ok(&["down", "--volumes"])
    } else {
        project.compose_ok(&["down"])
    }
}

pub fn status(project: &Project) -> anyhow::Result<()> {
    project.require()?;
    project.compose_ok(&["ps"])?;
    println!();
    match project.in_app(&["healthcheck"]) {
        Ok(()) => ok("API respondendo (/health 200)"),
        Err(_) => warn("API não respondeu ao healthcheck"),
    }
    if let Some(version) = project.env_value("NELCOTA_VERSION")? {
        println!("  versão do app: {version}");
    }
    Ok(())
}

pub fn logs(project: &Project, follow: bool, service: Option<&str>) -> anyhow::Result<()> {
    project.require()?;
    let mut args = vec!["logs", "--tail", "200"];
    if follow {
        args.push("-f");
    }
    if let Some(service) = service {
        args.push(service);
    }
    project.compose_ok(&args)
}

/// Senha nova para o admin do painel: grava só o hash no `.env` e recria o app.
pub fn admin_password(project: &Project) -> anyhow::Result<()> {
    project.require()?;
    let password = util::secret(20);
    let hash = nelcota_auth::hash_password(&password).context("falha ao gerar o hash")?;
    project.set_env_value("NELCOTA_ADMIN_PASSWORD_HASH", &format!("'{hash}'"))?;
    step("Reiniciando o app com a senha nova");
    project.compose_ok(&["up", "-d", "--force-recreate", "app"])?;
    project.wait_healthy("app", HEALTH_TIMEOUT)?;
    println!();
    println!("  Senha nova do painel: {password}");
    println!("  (mostrada só agora; o .env guarda apenas o hash argon2id)");
    Ok(())
}

/// `pg_dump -Fc` dentro do container do Postgres, gravado em `backups/`.
pub fn backup(project: &Project, upload: bool, keep: Option<usize>) -> anyhow::Result<PathBuf> {
    project.require()?;
    let dir = project.path("backups");
    fs::create_dir_all(&dir)?;
    let name = format!("nelcota-{}.dump", util::timestamp());
    let path = dir.join(&name);
    step(&format!("Gerando backup {name}"));

    let file = fs::File::create(&path)?;
    let status = Command::new("docker")
        .args(["compose", "--project-directory"])
        .arg(&project.dir)
        .arg("-f")
        .arg(project.path("docker-compose.yml"))
        .args([
            "exec", "-T", "postgres", "pg_dump", "-U", "postgres", "-Fc", "postgres",
        ])
        .stdout(Stdio::from(file))
        .status()
        .context("falha ao executar pg_dump")?;
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if !status.success() || size == 0 {
        let _ = fs::remove_file(&path);
        bail!("pg_dump falhou ({status})");
    }
    ok(&format!("{} ({} KB)", path.display(), size / 1024));

    if upload {
        upload_s3(project, &path, &name)?;
    }
    if let Some(keep) = keep {
        prune(&dir, keep)?;
    }
    Ok(path)
}

/// Envia ao S3-compatible com a imagem oficial do aws-cli (nada a instalar).
fn upload_s3(project: &Project, path: &Path, name: &str) -> anyhow::Result<()> {
    let get = |key: &str| -> anyhow::Result<String> {
        project
            .env_value(key)?
            .with_context(|| format!("{key} não configurado no .env (veja docs/backup.md)"))
    };
    let endpoint = get("NELCOTA_BACKUP_S3_ENDPOINT")?;
    let bucket = get("NELCOTA_BACKUP_S3_BUCKET")?;
    let access_key = get("NELCOTA_BACKUP_S3_ACCESS_KEY")?;
    let secret_key = get("NELCOTA_BACKUP_S3_SECRET_KEY")?;
    let region = project
        .env_value("NELCOTA_BACKUP_S3_REGION")?
        .unwrap_or_else(|| "us-east-1".into());
    let dir = fs::canonicalize(path.parent().unwrap_or(Path::new(".")))?;

    step(&format!("Enviando para s3://{bucket}/{name}"));
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
        .arg(format!("s3://{bucket}/{name}"))
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
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("nelcota-") && n.ends_with(".dump"))
        })
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
    project.require()?;
    if !file.is_file() {
        bail!("arquivo não encontrado: {}", file.display());
    }
    if !yes
        && !(util::interactive()
            && util::confirm(&format!(
                "Isso SUBSTITUI o banco atual pelo conteúdo de {}. Continuar?",
                file.display()
            )))
    {
        bail!("restore cancelado (use --yes para não perguntar)");
    }

    step("Parando o app");
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
    let status = Command::new("docker")
        .args(["compose", "--project-directory"])
        .arg(&project.dir)
        .arg("-f")
        .arg(project.path("docker-compose.yml"))
        .args([
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
        ])
        .stdin(Stdio::from(input))
        .status()
        .context("falha ao executar pg_restore")?;
    if !status.success() {
        bail!("pg_restore falhou ({status}); o banco não foi alterado (transação única)");
    }
    ok("dados restaurados");
    Ok(())
}

/// Backup → nova imagem → healthcheck. Se falhar: volta a imagem anterior e
/// restaura o backup (a versão nova pode ter aplicado migrações).
pub fn upgrade(project: &Project, version: Option<String>) -> anyhow::Result<()> {
    project.require()?;
    let current = project
        .env_value("NELCOTA_VERSION")?
        .context("NELCOTA_VERSION ausente no .env")?;
    let target = version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    println!("Atualizando o app: {current} → {target}");

    let dump =
        backup(project, false, None).context("backup pré-upgrade falhou; nada foi alterado")?;

    project.set_env_value("NELCOTA_VERSION", &target)?;
    step("Baixando a imagem nova");
    let pulled = project
        .compose(&["pull", "app"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !pulled {
        warn("não foi possível baixar a imagem (seguindo com a imagem local, se existir)");
    }
    step("Reiniciando o app na versão nova");
    let healthy = project
        .compose_ok(&["up", "-d", "app"])
        .and_then(|()| project.wait_healthy("app", HEALTH_TIMEOUT));

    match healthy {
        Ok(()) => {
            ok(&format!("app saudável na versão {target}"));
            println!("Backup pré-upgrade: {}", dump.display());
            Ok(())
        }
        Err(err) => {
            warn(&format!("a versão {target} não ficou saudável: {err}"));
            step(&format!("Rollback para {current}"));
            project.set_env_value("NELCOTA_VERSION", &current)?;
            project.compose_ok(&["stop", "app"])?;
            restore_dump(project, &dump)?;
            project.compose_ok(&["up", "-d", "app"])?;
            project.wait_healthy("app", HEALTH_TIMEOUT)?;
            ok(&format!(
                "rollback concluído: versão {current}, banco restaurado"
            ));
            bail!("upgrade para {target} falhou e foi revertido")
        }
    }
}
