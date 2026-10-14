use anyhow::bail;
use nelcota_deployment::{read_private, valid_project};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub email: String,
    pub password_hash: String,
    pub projects: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: u32,
    pub root: PathBuf,
    pub origin: String,
    pub controller_uid: u32,
    pub controller_gid: u32,
    pub verifier: PathBuf,
    pub operators: Vec<Operator>,
    pub project_ids: std::collections::BTreeMap<String, String>,
    pub enable_installation: bool,
    pub validation_report: Option<PathBuf>,
    pub minimum_free_bytes: u64,
    pub timeout_seconds: u64,
}
impl Policy {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.uid() != 0 {
                bail!("worker policy must be root-owned");
            }
        }
        trusted_path(path)?;
        let policy: Self = serde_json::from_slice(&read_private(path, 65536)?)?;
        trusted_path(&policy.root)?;
        trusted_state(&policy.root.join("updates"))?;
        let url = url::Url::parse(&policy.origin)?;
        if policy.schema != 1
            || policy.controller_uid == 0
            || policy.controller_gid == 0
            || !policy.root.is_absolute()
            || policy.root.canonicalize()? != policy.root
            || url.scheme() != "https"
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || policy.origin != url.origin().ascii_serialization()
            || policy.operators.is_empty()
            || policy.operators.len() > 16
            || policy.timeout_seconds < 60
            || policy.timeout_seconds > 7200
            || policy.minimum_free_bytes < 1024 * 1024 * 1024
        {
            bail!("invalid worker policy");
        }
        let mut emails = std::collections::HashSet::new();
        for operator in &policy.operators {
            if !emails.insert(&operator.email) {
                bail!("duplicate operator");
            }
            if operator.email.is_empty()
                || operator.email.len() > 254
                || operator.email.trim() != operator.email
                || operator.email.chars().any(char::is_control)
                || operator
                    .projects
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != operator.projects.len()
                || !operator.password_hash.starts_with("$argon2id$")
                || operator.projects.is_empty()
                || operator.projects.len() > 32
                || operator.projects.iter().any(|name| !valid_project(name))
            {
                bail!("invalid deployment operator");
            }
        }
        trusted_path(&policy.root.join("nelcota-host.json"))?;
        trusted_path(&policy.root.join("host.env"))?;
        read_private(&policy.root.join("host.env"), 65536)?;
        let host = crate::host::Host::new(&policy.root);
        let manifest = host.require()?;
        trusted_path(&host.shared_dir())?;
        if manifest.runtime == crate::host::Runtime::Docker {
            trusted_path(&host.caddy_dir().join("docker-compose.yml"))?;
            trusted_path(&host.caddy_dir().join("config"))?;
        }
        for (name, id) in &policy.project_ids {
            if !valid_project(name) || !nelcota_deployment::valid_digest(id) {
                bail!("invalid enrolled project identity");
            }
        }
        for name in policy
            .operators
            .iter()
            .flat_map(|operator| &operator.projects)
        {
            let project = host.select(&manifest, Some(name))?;
            trusted_path(&project.path(".env"))?;
            read_private(&project.path(".env"), 65536)?;
            trusted_path(&project.path(".deployment-project-id"))?;
            if !policy.project_enrolled(name) {
                bail!("project identity changed; enroll it again over SSH");
            }
        }

        Ok(policy)
    }
    pub fn project_enrolled(&self, name: &str) -> bool {
        let expected = match self.project_ids.get(name) {
            Some(id) => id,
            None => return false,
        };
        read_private(
            &self
                .root
                .join("projects")
                .join(name)
                .join(".deployment-project-id"),
            64,
        )
        .is_ok_and(|bytes| bytes == expected.as_bytes())
    }
    pub fn installation_validated(&self) -> bool {
        if !self.enable_installation {
            return false;
        }
        let report = match &self.validation_report {
            Some(path) => path,
            None => return false,
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if fs::symlink_metadata(report).map_or(true, |metadata| metadata.uid() != 0) {
                return false;
            }
        }
        if trusted_path(report).is_err() {
            return false;
        }
        let bytes = match read_private(report, 65536) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => return false,
        };
        static DIGEST: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let digest = match DIGEST.get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| nelcota_deployment::digest_file(&path, 256 * 1024 * 1024).ok())
        }) {
            Some(digest) => digest,
            None => return false,
        };
        value["schema"] == 1
            && value["status"] == "passed"
            && value["binary_sha256"] == digest.as_str()
            && [
                "docker_upgrade",
                "docker_rollback",
                "systemd_upgrade",
                "systemd_rollback",
                "worker_crash",
                "power_loss",
                "provenance",
                "browser_reconnect",
                "offsite_restore",
            ]
            .iter()
            .all(|name| value["checks"][*name] == true)
    }
}

pub(super) fn trusted_state(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut pending = vec![path.to_path_buf()];
    let mut count = 0usize;
    while let Some(path) = pending.pop() {
        count += 1;
        if count > 16384 {
            bail!("deployment state inventory is too large; archive completed history");
        }
        let metadata = fs::symlink_metadata(&path)?;
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || metadata.file_type().is_symlink()
        {
            bail!("deployment state must be root-owned without symlinks or writable members");
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(())
}

pub(super) fn trusted_path(path: &Path) -> anyhow::Result<()> {
    if !path.is_absolute() {
        bail!("deployment path must be absolute");
    }
    for component in path.ancestors() {
        let metadata = fs::symlink_metadata(component)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
                || metadata.file_type().is_symlink()
            {
                bail!("deployment path must be root-owned without writable ancestors");
            }
        }
    }
    Ok(())
}
