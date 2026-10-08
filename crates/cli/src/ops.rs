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
    host::{Host, Manifest, Runtime},
    native,
    project::{Project, Service},
    util::{self, ok, step, warn},
};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Starts the projects (each one until its healthcheck passes) and Caddy.
pub fn up(host: &Host, manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    if manifest.runtime == Runtime::Docker {
        caddy::ensure_network()?;
    }
    caddy::write(host, manifest)?;
    for project in projects {
        step(&format!("Starting {}", project.name));
        project.up()?;
        project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
        ok(&format!("{} healthy", project.name));
    }
    caddy::up(host, manifest.runtime)?;
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
    if volumes && project.runtime == Runtime::Docker {
        warn(&format!(
            "--volumes: the data of {} will be DELETED",
            project.name
        ));
    }
    project.down(volumes)
}

pub fn status(manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    println!("{:<16} {:<32} {:<10} VERSION", "PROJECT", "DOMAIN", "APP");
    for project in projects {
        let domain = manifest
            .projects
            .iter()
            .find(|e| e.name == project.name)
            .map_or("", |e| e.domain.as_str());
        let health = project
            .health(Service::App)
            .unwrap_or_else(|| "stopped".into());
        let version = project.env().get("NELCOTA_VERSION")?.unwrap_or_default();
        println!(
            "{:<16} {:<32} {:<10} {}",
            project.name, domain, health, version
        );
    }
    Ok(())
}

pub fn logs(project: &Project, follow: bool, service: Option<&str>) -> anyhow::Result<()> {
    project.logs(follow, service)
}

/// Recreates the apps that already exist (to apply `.env` changes).
pub fn recreate_apps(projects: &[Project]) -> anyhow::Result<()> {
    for project in projects.iter().filter(|p| p.has_app()) {
        step(&format!("Restarting the {} app", project.name));
        project.recreate(Service::App)?;
        project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
    }
    Ok(())
}

/// `pg_dump -Fc` as the postgres user, written to `backups/`.
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
    let status = project
        .as_postgres(&["pg_dump", "-Fc", "postgres"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .status()
        .context("could not run pg_dump")?;
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if !status.success() || size == 0 {
        let _ = fs::remove_file(&path);
        bail!("[{}] pg_dump failed ({status})", project.name);
    }
    ok(&format!("{} ({} KB)", path.display(), size / 1024));

    if upload {
        upload_s3(host, &project.name, &path, &name)?;
        sync_files_up(host, project)?;
    }
    if let Some(keep) = keep {
        prune(&dir, keep)?;
    }
    Ok(path)
}

/// The host's backup bucket (credentials from `host.env`).
struct BackupS3 {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
}

impl BackupS3 {
    fn from_host(host: &Host) -> anyhow::Result<Self> {
        let secrets = host.secrets();
        let get = |key: &str| -> anyhow::Result<String> {
            secrets
                .get(key)?
                .with_context(|| format!("{key} is not set in host.env (see docs/backup.md)"))
        };
        Ok(BackupS3 {
            endpoint: get("NELCOTA_BACKUP_S3_ENDPOINT")?,
            bucket: get("NELCOTA_BACKUP_S3_BUCKET")?,
            access_key: get("NELCOTA_BACKUP_S3_ACCESS_KEY")?,
            secret_key: get("NELCOTA_BACKUP_S3_SECRET_KEY")?,
            region: secrets
                .get("NELCOTA_BACKUP_S3_REGION")?
                .unwrap_or_else(|| "us-east-1".into()),
        })
    }

    /// Runs `aws s3 <args>` in the `amazon/aws-cli` image with `dir` mounted
    /// at `/data` (as `user`, when given).
    fn run(
        &self,
        dir: &Path,
        read_only: bool,
        user: Option<&str>,
        args: &[&str],
    ) -> anyhow::Result<()> {
        let dir = fs::canonicalize(dir)?.display().to_string();
        // Windows marks canonical paths as verbatim (`\\?\C:\...`), which
        // `docker -v` reads as extra colons.
        let dir = dir.strip_prefix(r"\\?\").unwrap_or(&dir);
        let mut command = Command::new("docker");
        command
            .args(["run", "--rm", "-v"])
            .arg(format!("{dir}:/data{}", if read_only { ":ro" } else { "" }));
        if let Some(user) = user {
            command.args(["--user", user]);
        }
        let status = command
            .args([
                "-e",
                "AWS_ACCESS_KEY_ID",
                "-e",
                "AWS_SECRET_ACCESS_KEY",
                "-e",
                "AWS_DEFAULT_REGION",
            ])
            .env("AWS_ACCESS_KEY_ID", &self.access_key)
            .env("AWS_SECRET_ACCESS_KEY", &self.secret_key)
            .env("AWS_DEFAULT_REGION", &self.region)
            .args(["amazon/aws-cli", "s3"])
            .args(args)
            .args(["--endpoint-url", &self.endpoint])
            .status()
            .context("could not run aws-cli")?;
        if !status.success() {
            bail!("aws s3 {} failed ({status})", args.first().unwrap_or(&""));
        }
        Ok(())
    }

    fn files_url(&self, project: &str) -> String {
        format!("s3://{}/{project}/storage/", self.bucket)
    }
}

/// Uploads to S3-compatible storage (credentials from `host.env`), under `<bucket>/<project>/`.
fn upload_s3(host: &Host, project: &str, path: &Path, name: &str) -> anyhow::Result<()> {
    let s3 = BackupS3::from_host(host)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    step(&format!("Uploading to s3://{}/{project}/{name}", s3.bucket));
    s3.run(
        dir,
        true,
        None,
        &[
            "cp",
            &format!("/data/{name}"),
            &format!("s3://{}/{project}/{name}", s3.bucket),
        ],
    )?;
    ok("uploaded");
    Ok(())
}

/// Mirrors the project's files to `<bucket>/<project>/storage/`. Keys are
/// immutable versions (D78), so each run only sends what is new, and files
/// deleted in the app are deleted there too.
fn sync_files_up(host: &Host, project: &Project) -> anyhow::Result<()> {
    let dir = project.storage_dir();
    if !project.stores_files_on_disk() || !dir.is_dir() {
        return Ok(());
    }
    let s3 = BackupS3::from_host(host)?;
    step(&format!("Syncing files to {}", s3.files_url(&project.name)));
    s3.run(
        &dir,
        true,
        None,
        &[
            "sync",
            "/data",
            &s3.files_url(&project.name),
            "--delete",
            "--only-show-errors",
        ],
    )?;
    ok("files synced");
    Ok(())
}

/// Brings the files back from the backup bucket, replacing the local ones.
fn sync_files_down(host: &Host, project: &Project) -> anyhow::Result<()> {
    let dir = project.storage_dir();
    fs::create_dir_all(&dir)?;
    let s3 = BackupS3::from_host(host)?;
    step(&format!(
        "Restoring files from {}",
        s3.files_url(&project.name)
    ));
    // On Docker, as the image's user so the app can still replace and delete
    // them; systemd hands its state directory to the unit's dynamic user.
    let user = (project.runtime == Runtime::Docker).then_some("65532:65532");
    s3.run(
        &dir,
        false,
        user,
        &[
            "sync",
            &s3.files_url(&project.name),
            "/data",
            "--delete",
            "--only-show-errors",
        ],
    )?;
    ok("files restored");
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
/// With `files`, also brings the files back from the backup bucket (what
/// `backup --upload` mirrored), replacing the local ones.
pub fn restore(
    host: &Host,
    project: &Project,
    file: &Path,
    files: bool,
    yes: bool,
) -> anyhow::Result<()> {
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
    project.stop(&[Service::App])?;
    step(&format!("Restoring {}", file.display()));
    let mut result = restore_dump(project, file);
    if files && result.is_ok() {
        result = sync_files_down(host, project);
    }
    step("Starting the app");
    project.start(&[Service::App])?;
    result?;
    project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
    ok("restore finished and app healthy");
    Ok(())
}

fn restore_dump(project: &Project, file: &Path) -> anyhow::Result<()> {
    let input = fs::File::open(file)?;
    let status = project
        .as_postgres(&[
            "pg_restore",
            "-d",
            "postgres",
            "--clean",
            "--if-exists",
            "--single-transaction",
            "--exit-on-error",
        ])
        .stdin(Stdio::from(input))
        .status()
        .context("could not run pg_restore")?;
    if !status.success() {
        bail!("pg_restore failed ({status}); the database was not changed (single transaction)");
    }
    ok("data restored");
    Ok(())
}

/// Backup → new version → healthcheck. On failure: goes back to the previous
/// version and restores the backup (the new version may have applied
/// migrations). On Docker the version is the image tag; on systemd, this
/// binary replaces the one the unit runs.
pub fn upgrade(host: &Host, project: &Project, version: Option<&str>) -> anyhow::Result<()> {
    let env = project.env();
    let current = env
        .get("NELCOTA_VERSION")?
        .context("NELCOTA_VERSION missing from .env")?;
    let this_binary = env!("CARGO_PKG_VERSION");
    if project.runtime == Runtime::Systemd && version.is_some_and(|v| v != this_binary) {
        bail!(
            "on a systemd host the project runs this binary ({this_binary}): install the version \
             you want first (NELCOTA_VERSION=x.y.z install.sh), then run `nelcota upgrade`"
        );
    }
    let target = version.map_or_else(|| this_binary.to_owned(), str::to_owned);
    println!("Upgrading {}: {current} → {target}", project.name);

    let dump = backup(host, project, false, None)
        .context("pre-upgrade backup failed; nothing was changed")?;

    env.set("NELCOTA_VERSION", &target)?;
    match project.runtime {
        Runtime::Docker => {
            step("Pulling the new image");
            if !project
                .compose(&["pull", "app"])
                .map(|s| s.success())
                .unwrap_or(false)
            {
                warn("could not pull the image (continuing with the local image, if any)");
            }
        }
        Runtime::Systemd => native::deploy_binary()?,
    }
    step("Restarting the app on the new version");
    let healthy = project
        .recreate(Service::App)
        .and_then(|()| project.wait_healthy(Service::App, HEALTH_TIMEOUT));

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
            if project.runtime == Runtime::Systemd {
                native::rollback_binary()?;
            }
            project.stop(&[Service::App])?;
            restore_dump(project, &dump)?;
            project.recreate(Service::App)?;
            project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
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
