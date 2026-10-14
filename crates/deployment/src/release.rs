use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_BINARY: u64 = 256 * 1024 * 1024;

pub fn valid_project(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub architecture: String,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
    pub image_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub schema: u32,
    pub ci_run_id: u64,
    pub version: String,
    pub source_sha: String,
    pub minimum_controller: String,
    pub protocol: u32,
    pub minimum_app: String,
    pub validation_mode: bool,
    pub migration_version: u32,
    pub artifacts: Vec<Artifact>,
}
impl Release {
    pub fn validate(&self, controller: &str, current: &str) -> anyhow::Result<()> {
        if self.ci_run_id == 0
            || self.migration_version == 0
            || self.migration_version > i32::MAX as u32
            || self.schema != 1
            || self.protocol != PROTOCOL_VERSION
            || !self.validation_mode
        {
            bail!("unsupported release protocol");
        }
        let version = semver::Version::parse(&self.version)?;
        let current = semver::Version::parse(current)?;
        let minimum = semver::Version::parse(&self.minimum_app)?;
        let required = semver::Version::parse(&self.minimum_controller)?;
        let controller = semver::Version::parse(controller)?;
        if !version.pre.is_empty()
            || !version.build.is_empty()
            || version <= current
            || current < minimum
            || controller < required
        {
            bail!("incompatible release version");
        }
        if self.source_sha.len() != 40
            || !self
                .source_sha
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            bail!("invalid release revision");
        }
        if self.artifacts.len() != 2 {
            bail!("incomplete release");
        }
        for architecture in ["x86_64", "aarch64"] {
            let entries: Vec<_> = self
                .artifacts
                .iter()
                .filter(|entry| entry.architecture == architecture)
                .collect();
            let artifact = entries.first().context("missing release architecture")?;
            if entries.len() != 1
                || artifact.filename != format!("nelcota-{architecture}-unknown-linux-musl")
                || artifact.bytes == 0
                || artifact.bytes > MAX_BINARY
                || !valid_digest(&artifact.sha256)
                || !valid_digest(&artifact.image_digest)
            {
                bail!("invalid release artifact");
            }
        }
        Ok(())
    }
    pub fn artifact(&self, architecture: &str) -> anyhow::Result<&Artifact> {
        self.artifacts
            .iter()
            .find(|entry| entry.architecture == architecture)
            .context("unsupported host architecture")
    }
}
