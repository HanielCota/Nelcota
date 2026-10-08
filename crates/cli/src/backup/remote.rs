//! S3-compatible transport for immutable backup pairs.
use crate::{
    host::Host,
    project::Project,
    util::{ok, step},
};
use anyhow::{Context, bail};
use std::{fs, path::Path, process::Command};
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
pub(super) fn upload_s3(host: &Host, project: &str, path: &Path, name: &str) -> anyhow::Result<()> {
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
pub(super) fn sync_files_up(host: &Host, project: &Project, dump: &Path) -> anyhow::Result<()> {
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
pub(super) fn sync_files_down(host: &Host, project: &Project, dump: &Path) -> anyhow::Result<()> {
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
