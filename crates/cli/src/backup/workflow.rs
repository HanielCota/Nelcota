//! Backup, retention and restore sequencing for a project.
use super::remote::{sync_files_down, sync_files_up, upload_s3};
use crate::{
    host::Host,
    lifecycle::HEALTH_TIMEOUT,
    project::{Project, Service},
    util::{self, ok, step},
};
use anyhow::{Context, bail};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};
/// `pg_dump -Fc` as the postgres user, written to `backups/`.
pub fn backup(
    host: &Host,
    project: &Project,
    upload: bool,
    keep: Option<usize>,
) -> anyhow::Result<PathBuf> {
    let dir = project.path("backups");
    crate::private_fs::dir(&dir)?;
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

/// When the newest local dump of the project was written.
pub(crate) fn latest(project: &Project) -> Option<std::time::SystemTime> {
    fs::read_dir(project.path("backups"))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "dump"))
        .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
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
