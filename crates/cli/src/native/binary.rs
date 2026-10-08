//! Binary deployment and rollback through atomic file replacement.
use anyhow::bail;
use std::{fs, path::PathBuf};
const LIB_DIR: &str = "/usr/local/lib/nelcota";

/// The binary the unit runs, and the one kept for a rollback.
pub fn deployed_binary() -> PathBuf {
    PathBuf::from(format!("{LIB_DIR}/nelcota"))
}

fn previous_binary() -> PathBuf {
    PathBuf::from(format!("{LIB_DIR}/nelcota.previous"))
}

/// Copies this binary to where the unit runs it. The one there before is
/// kept as `nelcota.previous`.
pub fn deploy_binary() -> anyhow::Result<()> {
    fs::create_dir_all(LIB_DIR)?;
    let target = deployed_binary();
    if target.is_file() {
        fs::copy(&target, previous_binary())?;
    }
    // Copy then rename: the running binary cannot be overwritten in place.
    let tmp = target.with_extension("new");
    fs::copy(std::env::current_exe()?, &tmp)?;
    fs::rename(&tmp, &target)?;
    Ok(())
}

/// Puts back the binary from before the last [`deploy_binary`].
pub fn rollback_binary() -> anyhow::Result<()> {
    let previous = previous_binary();
    if !previous.is_file() {
        bail!("no previous binary at {}", previous.display());
    }
    fs::rename(previous, deployed_binary())?;
    Ok(())
}
