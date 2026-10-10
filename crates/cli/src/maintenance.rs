//! A persistent proxy gate. It stays closed on failure until recovery succeeds;
//! dropping a guard must never expose a partially restored project.
use std::{fs, path::PathBuf};

use crate::{caddy, host::Host, project::Project};
use anyhow::Context;

pub(crate) const MARKER: &str = ".maintenance";

#[must_use]
pub(crate) struct Maintenance<'a> {
    host: &'a Host,
    project: &'a Project,
    marker: PathBuf,
}

impl<'a> Maintenance<'a> {
    pub(crate) fn enter(host: &'a Host, project: &'a Project) -> anyhow::Result<Self> {
        let marker = project.path(MARKER);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
            .context(
                "project already in maintenance; recover it with `nelcota up` before upgrading",
            )?;
        let guard = Self {
            host,
            project,
            marker,
        };
        guard.apply()?;
        Ok(guard)
    }

    /// Like [`Maintenance::enter`], but also takes over a gate left closed by
    /// a failed upgrade or restore: a restore is how such a project recovers.
    pub(crate) fn hold(host: &'a Host, project: &'a Project) -> anyhow::Result<Self> {
        let marker = project.path(MARKER);
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&marker)
            .context("could not close the maintenance gate")?;
        let guard = Self {
            host,
            project,
            marker,
        };
        guard.apply()?;
        Ok(guard)
    }

    fn apply(&self) -> anyhow::Result<()> {
        let manifest = self.host.require()?;
        caddy::write(self.host, &manifest)?;
        caddy::reload(self.host, self.project.runtime)
    }

    pub(crate) fn finish(self) -> anyhow::Result<()> {
        fs::remove_file(&self.marker)?;
        if let Err(error) = self.apply() {
            // Preserve the gate on disk too, so a later proxy restart is safe.
            fs::write(&self.marker, b"")?;
            let _ = caddy::write(self.host, &self.host.require()?);
            return Err(error).context("project healthy, but maintenance could not be lifted");
        }
        Ok(())
    }
}

/// An explicit `up` recovers a gate left by a failed/interrupted upgrade,
/// after the caller has checked the app's health.
pub(crate) fn recover(project: &Project) -> anyhow::Result<()> {
    let marker = project.path(MARKER);
    if marker.is_file() {
        fs::remove_file(marker)?;
    }
    Ok(())
}
