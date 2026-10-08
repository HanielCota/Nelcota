//! App version changes and rollback coordinated with a pre-upgrade backup.
use crate::{
    backup,
    host::{Host, Runtime},
    lifecycle::HEALTH_TIMEOUT,
    native,
    project::{Project, Service},
    util::{ok, step, warn},
};
use anyhow::{Context, bail};
/// Backup → new version → healthcheck. On failure: goes back to the previous
/// version and restores the backup (the new version may have applied
/// migrations). On Docker the version is the image tag; on systemd, this
/// binary replaces the one the unit runs.
pub fn run(host: &Host, project: &Project, version: Option<&str>) -> anyhow::Result<()> {
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

    let dump = backup::run(host, project, false, None)
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
            backup::restore_dump(project, &dump)?;
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
