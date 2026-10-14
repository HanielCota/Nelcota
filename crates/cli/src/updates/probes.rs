//! Read-only host probes have bounded output and deadlines; they never use a shell.
use anyhow::{Context, bail};
use std::{
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn output(source: &Command) -> anyhow::Result<String> {
    let mut command = Command::new(source.get_program());
    command.args(source.get_args());
    if let Some(path) = source.get_current_dir() {
        command.current_dir(path);
    }
    for (key, value) in source.get_envs() {
        match value {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
    let mut file = tempfile::tempfile()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("host probe failed");
            }
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("host probe timed out");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    if file.metadata()?.len() > 65536 {
        bail!("host probe output too large");
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = String::new();
    file.take(65537).read_to_string(&mut bytes)?;
    Ok(bytes)
}
pub fn database_bytes(project: &crate::project::Project) -> anyhow::Result<u64> {
    let mut command = project.as_postgres(&[
        "psql",
        "-XqAt",
        "-v",
        "ON_ERROR_STOP=1",
        "-c",
        "SELECT pg_database_size(current_database())",
    ]);
    command.env("PGOPTIONS", "-c statement_timeout=4000");
    output(&command)?
        .trim()
        .parse()
        .context("invalid database size")
}
pub fn quiescent(project: &crate::project::Project) -> anyhow::Result<()> {
    let mut command=project.as_postgres(&["psql","-XqAt","-v","ON_ERROR_STOP=1","-c","SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND application_name IN ('nelcota','nelcota-admin') AND pid<>pg_backend_pid()"]);
    command.env("PGOPTIONS", "-c statement_timeout=4000");
    if output(&command)?.trim() != "0" {
        bail!("application database connections remain after stop");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probes_keep_exit_failure_and_reject_unbounded_output() {
        assert!(output(&Command::new("false")).is_err());
        assert!(output(Command::new("head").args(["-c", "65537", "/dev/zero"])).is_err());
        assert_eq!(
            output(Command::new("printf").arg("healthy")).unwrap(),
            "healthy"
        );
    }
}
