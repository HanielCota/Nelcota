//! Restore sequencing, timeline promotion and post-restore health.
use super::{
    HEALTH_TIMEOUT,
    commands::{self, pgbackrest},
    enabled,
};
use crate::{
    project::{Project, Service},
    util::{self, ok, step, warn},
};
use anyhow::bail;
use std::time::{Duration, Instant};
const RECOVERY_TIMEOUT: Duration = Duration::from_secs(3600);

/// Restores the database to `time` (or to the last archived write) and
/// starts a new timeline from there.
pub fn restore(project: &Project, time: Option<&str>, yes: bool) -> anyhow::Result<()> {
    if !enabled(project) {
        bail!("PITR is not enabled for {}", project.name);
    }
    let moment = time.unwrap_or("the last archived write");
    if !yes
        && !(util::interactive()
            && util::confirm(&format!(
                "This REPLACES the {} database with its state at {moment}. \
                 Writes after that are discarded. Continue?",
                project.name
            )))
    {
        bail!("restore cancelled (use --yes to skip the question)");
    }

    step(&format!("Stopping {}", project.name));
    project.stop(&[Service::App, Service::Postgres])?;

    step(&format!("Restoring {} to {moment}", project.name));
    // --delta only rewrites files that differ from the backup.
    if let Err(err) = commands::restore(project, time) {
        // pgBackRest refuses most problems (no backup for that moment, S3
        // unreachable) before touching the data: bring the project back.
        step("Restore refused; starting the project again");
        let _ = project.start(&[Service::Postgres, Service::App]);
        return Err(err.context(
            "the restore did not run; if Postgres does not start, run the restore again",
        ));
    }

    step("Replaying WAL (Postgres starts once it reaches the target)");
    // A fresh start: a crash from now on is this recovery failing.
    project.recreate(Service::Postgres)?;
    wait_for_promotion(project)?;
    ok("database restored");

    // The restore started a new timeline. Backups taken on the old one after
    // the fork cannot serve it, so the next restore needs a backup from here.
    step("New full backup on the restored timeline");
    if let Err(err) = pgbackrest(project, &["--type=full", "backup"]) {
        warn(&format!(
            "{err:#}: take one with `nelcota -p {} pitr backup` before relying on another restore",
            project.name
        ));
    }

    step("Starting the app");
    project.start(&[Service::App])?;
    project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
    ok(&format!(
        "{} restored to {moment} and healthy",
        project.name
    ));
    Ok(())
}

/// Waits until Postgres has left recovery. A target the archive does not
/// reach makes Postgres exit: that is a failure.
fn wait_for_promotion(project: &Project) -> anyhow::Result<()> {
    let deadline = Instant::now() + RECOVERY_TIMEOUT;
    loop {
        if commands::in_recovery(project) == Some(false) {
            return Ok(());
        }
        if project.postgres_crashed() {
            project.stop(&[Service::Postgres])?;
            bail!(
                "Postgres could not finish the recovery (is the target inside the archive? \
                 see `nelcota -p {0} pitr status` and `nelcota -p {0} logs postgres`); \
                 Postgres was stopped: run the restore again with another --time",
                project.name
            );
        }
        if Instant::now() > deadline {
            bail!(
                "recovery still running after {}s: follow it with `nelcota -p {} logs -f postgres`",
                RECOVERY_TIMEOUT.as_secs(),
                project.name
            );
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
