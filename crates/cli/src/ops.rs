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
    let name = format!(
        "nelcota-{}-{}-{}.dump",
        project.name,
        util::timestamp(),
        util::secret(6)
    );
    let path = dir.join(&name);
    step(&format!("Backup of {}: {name}", project.name));

    crate::backup::capture(project, &path)?;
    let size = fs::metadata(&path)?.len();
    ok(&format!("{} ({} KB)", path.display(), size / 1024));

    if upload {
        // Publish the immutable file snapshot before making its dump available.
        sync_files_up(host, project, &path)?;
        upload_s3(host, &project.name, &path, &name)?;
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

    fn files_url(&self, project: &str, dump: &Path) -> anyhow::Result<String> {
        let name = dump
            .file_name()
            .and_then(|n| n.to_str())
            .context("invalid dump filename")?;
        Ok(format!("s3://{}/{project}/{name}.files/", self.bucket))
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

/// Each dump has its own immutable snapshot; later deletions cannot alter it.
fn sync_files_up(host: &Host, project: &Project, dump: &Path) -> anyhow::Result<()> {
    let dir = crate::backup::bundle_path(dump)?;
    if !dir.is_dir() {
        return Ok(());
    }
    let s3 = BackupS3::from_host(host)?;
    let url = s3.files_url(&project.name, dump)?;
    step(&format!("Uploading file snapshot to {url}"));
    s3.run(
        &dir,
        true,
        None,
        &["sync", "/data", &url, "--only-show-errors"],
    )?;
    ok("files synced");
    Ok(())
}

/// Fetch the matching snapshot into the backup directory, never the live store.
fn sync_files_down(host: &Host, project: &Project, dump: &Path) -> anyhow::Result<()> {
    let dir = crate::backup::bundle_path(dump)?;
    fs::create_dir_all(&dir)?;
    let s3 = BackupS3::from_host(host)?;
    let url = s3.files_url(&project.name, dump)?;
    step(&format!("Fetching file snapshot from {url}"));
    s3.run(
        &dir,
        false,
        None,
        &["sync", &url, "/data", "--only-show-errors"],
    )?;
    ok("files restored");
    Ok(())
}

pub(crate) fn prune(dir: &Path, keep: usize) -> anyhow::Result<()> {
    let mut dumps: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "dump"))
        .collect();
    // Random suffixes avoid same-second collisions; use file time for retention.
    dumps.sort_by_cached_key(|path| {
        (
            fs::metadata(path)
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH),
            path.clone(),
        )
    });
    let excess = dumps.len().saturating_sub(keep);
    for old in dumps.into_iter().take(excess) {
        crate::backup::remove_bundle(&old)?;
        fs::remove_file(&old)?;
        ok(&format!("removed {}", old.display()));
    }
    Ok(())
}

/// Restores a dump: stops the app, recreates the objects in a single
/// transaction and starts the app again.
/// With `files`, verifies the matching immutable snapshot before any change.
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
    let manifest = if files {
        if !crate::backup::bundle_path(file)?
            .join("manifest.json")
            .is_file()
        {
            sync_files_down(host, project, file)?;
        }
        Some(crate::backup::validate(file)?)
    } else {
        None
    };
    step(&format!("Stopping the {} app", project.name));
    project.stop(&[Service::App])?;
    step(&format!("Restoring {}", file.display()));
    // Prepare immutable bytes first. A copy failure cannot leave restored
    // metadata pointing to missing files; extra versions are harmless on rollback.
    let result = if let Some(manifest) = manifest.as_ref() {
        crate::backup::restore_files(project, file, manifest)
            .and_then(|()| restore_dump(project, file))
    } else {
        restore_dump(project, file)
    };
    step("Starting the app");
    project.start(&[Service::App])?;
    result?;
    project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
    ok("restore finished and app healthy");
    Ok(())
}

pub(crate) fn restore_dump(project: &Project, file: &Path) -> anyhow::Result<()> {
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
