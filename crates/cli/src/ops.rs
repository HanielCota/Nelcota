//! Project operations: up/down/status/logs, backup/restore and upgrade with
//! automatic rollback.

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

/// Starts the projects (each one until its healthcheck passes) and Caddy.
pub fn up(host: &Host, manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    caddy::ensure_network()?;
    caddy::write(host, manifest)?;
    for project in projects {
        step(&format!("Starting {}", project.name));
        project.compose_ok(&["up", "-d"])?;
        project.wait_healthy("app", HEALTH_TIMEOUT)?;
        ok(&format!("{} healthy", project.name));
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
            "--volumes: the data of {} will be DELETED",
            project.name
        ));
        project.compose_ok(&["down", "--volumes"])
    } else {
        project.compose_ok(&["down"])
    }
}

pub fn status(manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    println!("{:<16} {:<32} {:<10} VERSION", "PROJECT", "DOMAIN", "APP");
    for project in projects {
        let domain = manifest
            .projects
            .iter()
            .find(|e| e.name == project.name)
            .map_or("", |e| e.domain.as_str());
        let health = project.health("app").unwrap_or_else(|| "stopped".into());
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

/// Recreates the apps that already exist (to apply `.env` changes).
pub fn recreate_apps(projects: &[Project]) -> anyhow::Result<()> {
    for project in projects.iter().filter(|p| p.has_container("app")) {
        step(&format!("Restarting the {} app", project.name));
        project.compose_ok(&["up", "-d", "--force-recreate", "app"])?;
        project.wait_healthy("app", HEALTH_TIMEOUT)?;
    }
    Ok(())
}

/// `pg_dump -Fc` inside the Postgres container, written to `backups/`.
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
    step(&format!("Backup of {}: {name}", project.name));

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
        bail!("[{}] pg_dump failed ({status})", project.name);
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

/// Uploads to S3-compatible storage (credentials from `host.env`), under `<bucket>/<project>/`.
fn upload_s3(host: &Host, project: &str, path: &Path, name: &str) -> anyhow::Result<()> {
    let secrets = host.secrets();
    let get = |key: &str| -> anyhow::Result<String> {
        secrets
            .get(key)?
            .with_context(|| format!("{key} is not set in host.env (see docs/backup.md)"))
    };
    let endpoint = get("NELCOTA_BACKUP_S3_ENDPOINT")?;
    let bucket = get("NELCOTA_BACKUP_S3_BUCKET")?;
    let access_key = get("NELCOTA_BACKUP_S3_ACCESS_KEY")?;
    let secret_key = get("NELCOTA_BACKUP_S3_SECRET_KEY")?;
    let region = secrets
        .get("NELCOTA_BACKUP_S3_REGION")?
        .unwrap_or_else(|| "us-east-1".into());
    let dir = fs::canonicalize(path.parent().unwrap_or(Path::new(".")))?;

    step(&format!("Uploading to s3://{bucket}/{project}/{name}"));
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
        .context("could not run aws-cli")?;
    if !status.success() {
        bail!("upload to S3 failed ({status})");
    }
    ok("uploaded");
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
        ok(&format!("removed {}", old.display()));
    }
    Ok(())
}

/// Restores a dump: stops the app, recreates the objects in a single
/// transaction and starts the app again.
pub fn restore(project: &Project, file: &Path, yes: bool) -> anyhow::Result<()> {
    if !file.is_file() {
        bail!("file not found: {}", file.display());
    }
    if !yes
        && !(util::interactive()
            && util::confirm(&format!(
                "This REPLACES the {} database with the contents of {}. Continue?",
                project.name,
                file.display()
            )))
    {
        bail!("restore cancelled (use --yes to skip the question)");
    }
    step(&format!("Stopping the {} app", project.name));
    project.compose_ok(&["stop", "app"])?;
    step(&format!("Restoring {}", file.display()));
    let result = restore_dump(project, file);
    step("Starting the app");
    project.compose_ok(&["up", "-d", "app"])?;
    result?;
    project.wait_healthy("app", HEALTH_TIMEOUT)?;
    ok("restore finished and app healthy");
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
        bail!("pg_restore failed ({status}); the database was not changed (single transaction)");
    }
    ok("data restored");
    Ok(())
}

/// Backup → new image → healthcheck. On failure: goes back to the previous
/// image and restores the backup (the new version may have applied migrations).
pub fn upgrade(host: &Host, project: &Project, version: Option<&str>) -> anyhow::Result<()> {
    let env = project.env();
    let current = env
        .get("NELCOTA_VERSION")?
        .context("NELCOTA_VERSION missing from .env")?;
    let target = version.map_or_else(|| env!("CARGO_PKG_VERSION").to_owned(), str::to_owned);
    println!("Upgrading {}: {current} → {target}", project.name);

    let dump = backup(host, project, false, None)
        .context("pre-upgrade backup failed; nothing was changed")?;

    env.set("NELCOTA_VERSION", &target)?;
    step("Pulling the new image");
    if !project
        .compose(&["pull", "app"])
        .map(|s| s.success())
        .unwrap_or(false)
    {
        warn("could not pull the image (continuing with the local image, if any)");
    }
    step("Restarting the app on the new version");
    let healthy = project
        .compose_ok(&["up", "-d", "app"])
        .and_then(|()| project.wait_healthy("app", HEALTH_TIMEOUT));

    match healthy {
        Ok(()) => {
            ok(&format!("{} healthy on version {target}", project.name));
            println!("Pre-upgrade backup: {}", dump.display());
            Ok(())
        }
        Err(err) => {
            warn(&format!("version {target} did not become healthy: {err}"));
            step(&format!("Rolling back to {current}"));
            env.set("NELCOTA_VERSION", &current)?;
            project.compose_ok(&["stop", "app"])?;
            restore_dump(project, &dump)?;
            project.compose_ok(&["up", "-d", "app"])?;
            project.wait_healthy("app", HEALTH_TIMEOUT)?;
            ok(&format!(
                "rollback finished: version {current}, database restored"
            ));
            bail!(
                "upgrade of {} to {target} failed and was reverted",
                project.name
            )
        }
    }
}
