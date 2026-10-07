//! A project on the host: a folder with `.env`, `migrations/` and `backups/`,
//! and the operations on its two services (Postgres and the app). How they
//! run depends on the host's [`Runtime`]: containers from the project's
//! `docker-compose.yml`, or systemd units on the machine itself.

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, bail};

use crate::{envfile::EnvFile, host::Runtime, native};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Postgres,
    App,
}

impl Service {
    /// The compose service name.
    fn compose(self) -> &'static str {
        match self {
            Service::Postgres => "postgres",
            Service::App => "app",
        }
    }
}

pub struct Project {
    pub name: String,
    pub dir: PathBuf,
    pub runtime: Runtime,
}

impl Project {
    pub fn new(name: &str, dir: &Path, runtime: Runtime) -> Self {
        Project {
            name: name.to_owned(),
            dir: dir.to_path_buf(),
            runtime,
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn env(&self) -> EnvFile {
        EnvFile::new(self.path(".env"))
    }

    pub fn exists(&self) -> bool {
        match self.runtime {
            Runtime::Docker => self.path("docker-compose.yml").is_file(),
            Runtime::Systemd => native::unit_path(&self.name).is_file(),
        }
    }

    fn unit(&self, service: Service) -> String {
        match service {
            Service::Postgres => native::POSTGRES_UNIT.to_owned(),
            Service::App => native::app_unit(&self.name),
        }
    }

    fn units(&self, services: &[Service]) -> Vec<String> {
        services.iter().map(|s| self.unit(*s)).collect()
    }

    // ------------------------------------------------------------ lifecycle

    /// Starts everything (and, on systemd, enables it at boot).
    pub fn up(&self) -> anyhow::Result<()> {
        match self.runtime {
            Runtime::Docker => self.compose_ok(&["up", "-d"]),
            Runtime::Systemd => {
                let mut args = vec!["enable".to_owned(), "--now".to_owned()];
                args.extend(self.units(&[Service::Postgres, Service::App]));
                native::systemctl(&args)
            }
        }
    }

    /// Stops everything. With `volumes`, also deletes the data (Docker only:
    /// on systemd, `remove` drops the cluster).
    pub fn down(&self, volumes: bool) -> anyhow::Result<()> {
        match (self.runtime, volumes) {
            (Runtime::Docker, true) => self.compose_ok(&["down", "--volumes"]),
            (Runtime::Docker, false) => self.compose_ok(&["down"]),
            (Runtime::Systemd, true) => {
                bail!("--volumes is for Docker hosts; to delete the data use `nelcota remove`")
            }
            (Runtime::Systemd, false) => self.stop(&[Service::App, Service::Postgres]),
        }
    }

    pub fn start(&self, services: &[Service]) -> anyhow::Result<()> {
        match self.runtime {
            Runtime::Docker => {
                let mut args = vec!["up", "-d"];
                args.extend(services.iter().map(|s| s.compose()));
                self.compose_ok(&args)
            }
            Runtime::Systemd => {
                let units = self.units(services);
                // A unit that failed before (a refused recovery) must be cleared to start.
                let _ =
                    native::systemctl_quiet(&[&["reset-failed".to_owned()], &units[..]].concat());
                native::systemctl(&[&["start".to_owned()], &units[..]].concat())
            }
        }
    }

    pub fn stop(&self, services: &[Service]) -> anyhow::Result<()> {
        match self.runtime {
            Runtime::Docker => {
                let mut args = vec!["stop"];
                args.extend(services.iter().map(|s| s.compose()));
                self.compose_ok(&args)
            }
            Runtime::Systemd => {
                native::systemctl(&[&["stop".to_owned()], &self.units(services)[..]].concat())
            }
        }
    }

    /// Restarts a service so it rereads its configuration (on Docker, a new
    /// container: also picks up `.env` and image changes).
    pub fn recreate(&self, service: Service) -> anyhow::Result<()> {
        match self.runtime {
            Runtime::Docker => {
                self.compose_ok(&["up", "-d", "--force-recreate", service.compose()])
            }
            Runtime::Systemd => {
                let unit = self.unit(service);
                let _ = native::systemctl_quiet(&["reset-failed".to_owned(), unit.clone()]);
                native::systemctl(&["restart".to_owned(), unit])
            }
        }
    }

    /// Has the app been created (container or unit)?
    pub fn has_app(&self) -> bool {
        match self.runtime {
            Runtime::Docker => self.has_container("app"),
            Runtime::Systemd => self.exists(),
        }
    }

    pub fn logs(&self, follow: bool, service: Option<&str>) -> anyhow::Result<()> {
        match self.runtime {
            Runtime::Docker => {
                let mut args = vec!["logs", "--tail", "200"];
                if follow {
                    args.push("-f");
                }
                if let Some(service) = service {
                    args.push(service);
                }
                self.compose_ok(&args)
            }
            Runtime::Systemd => {
                let units = match service {
                    None => self.units(&[Service::Postgres, Service::App]),
                    Some("postgres") => self.units(&[Service::Postgres]),
                    Some("app") => self.units(&[Service::App]),
                    Some(other) => bail!("unknown service '{other}' (postgres or app)"),
                };
                let mut command = Command::new("journalctl");
                command.args(["--no-pager", "-n", "200"]);
                if follow {
                    command.arg("-f");
                }
                for unit in &units {
                    command.args(["-u", unit]);
                }
                run(&mut command, "journalctl")
            }
        }
    }

    // --------------------------------------------------------------- health

    /// Health of the service (`healthy`, `starting`, `unhealthy`...), if running.
    pub fn health(&self, service: Service) -> Option<String> {
        match self.runtime {
            Runtime::Docker => {
                let id = self.compose_output(&["ps", "-q", service.compose()]).ok()?;
                let id = id.trim();
                if id.is_empty() {
                    return None;
                }
                let output = Command::new("docker")
                    .args(["inspect", "--format", "{{.State.Health.Status}}", id])
                    .output()
                    .ok()?;
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
            }
            Runtime::Systemd => {
                let state = native::unit_state(&self.unit(service))?;
                match state.as_str() {
                    "active" => {}
                    "failed" => return Some("unhealthy".into()),
                    "activating" | "reloading" => return Some("starting".into()),
                    _ => return None,
                }
                let up = match service {
                    Service::App => crate::util::healthcheck(native::APP_LISTEN),
                    Service::Postgres => self
                        .as_postgres(&["pg_isready", "-q"])
                        .stderr(Stdio::null())
                        .status()
                        .is_ok_and(|s| s.success()),
                };
                Some(if up { "healthy" } else { "starting" }.into())
            }
        }
    }

    /// Waits for the service to become `healthy`.
    pub fn wait_healthy(&self, service: Service, timeout: Duration) -> anyhow::Result<()> {
        let name = service.compose();
        let deadline = Instant::now() + timeout;
        loop {
            match self.health(service).as_deref() {
                Some("healthy") => return Ok(()),
                Some("unhealthy") => {
                    bail!("[{}] the {name} service became unhealthy", self.name)
                }
                _ => {}
            }
            if Instant::now() > deadline {
                bail!(
                    "[{}] the {name} service did not become healthy within {}s",
                    self.name,
                    timeout.as_secs()
                );
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// Did Postgres die since it was last (re)started? A recovery whose
    /// target the archive does not reach makes it exit.
    pub fn postgres_crashed(&self) -> bool {
        match self.runtime {
            Runtime::Docker => {
                let Ok(id) = self.compose_output_quiet(&["ps", "-a", "-q", "postgres"]) else {
                    return false;
                };
                Command::new("docker")
                    .args(["inspect", "--format", "{{.RestartCount}}", id.trim()])
                    .output()
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .trim()
                            .parse::<u64>()
                            .ok()
                    })
                    .is_some_and(|n| n > 0)
            }
            Runtime::Systemd => {
                native::unit_state(native::POSTGRES_UNIT).as_deref() == Some("failed")
            }
        }
    }

    // ------------------------------------------------- commands on the data

    /// A program run as the `postgres` user next to the running database
    /// (`pg_dump`, `psql`, `pgbackrest`...): superuser over the local socket.
    pub fn as_postgres(&self, args: &[&str]) -> Command {
        match self.runtime {
            Runtime::Docker => {
                let mut command = self.compose_command();
                command
                    .args(["exec", "-T", "-u", "postgres", "postgres"])
                    .args(args);
                command
            }
            Runtime::Systemd => {
                let mut command = Command::new("runuser");
                command.args(["-u", "postgres", "--"]).args(args);
                command
            }
        }
    }

    /// Like [`Project::as_postgres`], with the database stopped (a restore
    /// that rewrites the data directory).
    pub fn as_postgres_offline(&self, args: &[&str]) -> Command {
        match self.runtime {
            Runtime::Docker => {
                let (program, rest) = args.split_first().expect("a program to run");
                let mut command = self.compose_command();
                command
                    .args([
                        "run",
                        "--rm",
                        "--no-deps",
                        "-T",
                        "-u",
                        "postgres",
                        "--entrypoint",
                    ])
                    .arg(program)
                    .arg("postgres")
                    .args(rest);
                command
            }
            Runtime::Systemd => self.as_postgres(args),
        }
    }

    /// Runs `nelcota <args>` with the app's configuration: inside its
    /// container, or as a local process with the project's `.env`.
    pub fn in_app(&self, args: &[&str]) -> anyhow::Result<()> {
        let mut command = self.app_command(args)?;
        run(&mut command, &format!("nelcota {}", args.join(" ")))
    }

    pub fn in_app_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = self
            .app_command(args)?
            .stderr(Stdio::inherit())
            .output()
            .context("could not run nelcota")?;
        if !output.status.success() {
            bail!(
                "[{}] `nelcota {}` failed ({})",
                self.name,
                args.join(" "),
                output.status
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn app_command(&self, args: &[&str]) -> anyhow::Result<Command> {
        match self.runtime {
            Runtime::Docker => {
                let mut command = self.compose_command();
                command
                    .args(["exec", "-T", "app", "/usr/local/bin/nelcota"])
                    .args(args);
                Ok(command)
            }
            Runtime::Systemd => {
                let mut command = Command::new(std::env::current_exe()?);
                command.envs(self.env().read()?).args(args);
                Ok(command)
            }
        }
    }

    /// Where the app sees the project's migrations.
    pub fn app_migrations_dir(&self) -> String {
        match self.runtime {
            Runtime::Docker => "/migrations".into(),
            Runtime::Systemd => self.path("migrations").display().to_string(),
        }
    }

    // ------------------------------------------------------- docker compose

    fn compose_command(&self) -> Command {
        compose_command(&self.dir)
    }

    /// `docker compose <args>` with the output on the terminal.
    pub fn compose(&self, args: &[&str]) -> anyhow::Result<ExitStatus> {
        self.compose_command()
            .args(args)
            .status()
            .context("could not run `docker compose` (is Docker installed?)")
    }

    pub fn compose_ok(&self, args: &[&str]) -> anyhow::Result<()> {
        let status = self.compose(args)?;
        if !status.success() {
            bail!(
                "[{}] `docker compose {}` failed ({status})",
                self.name,
                args.join(" ")
            );
        }
        Ok(())
    }

    /// `docker compose <args>` capturing standard output.
    pub fn compose_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = self
            .compose_command()
            .args(args)
            .stderr(Stdio::inherit())
            .output()
            .context("could not run `docker compose`")?;
        if !output.status.success() {
            bail!(
                "[{}] `docker compose {}` failed ({})",
                self.name,
                args.join(" "),
                output.status
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Like [`Project::compose_output`], without echoing errors: for polling
    /// a state that is expected to fail for a while.
    pub fn compose_output_quiet(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = self
            .compose_command()
            .args(args)
            .stderr(Stdio::null())
            .output()
            .context("could not run `docker compose`")?;
        if !output.status.success() {
            bail!("[{}] `docker compose {}` failed", self.name, args.join(" "));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Does the service's container exist (created, running or stopped)?
    pub fn has_container(&self, service: &str) -> bool {
        self.compose_output(&["ps", "-a", "-q", service])
            .map(|out| !out.trim().is_empty())
            .unwrap_or(false)
    }
}

/// Runs a command with the output on the terminal; a non-zero exit is an error.
pub fn run(command: &mut Command, what: &str) -> anyhow::Result<()> {
    let status = command
        .status()
        .with_context(|| format!("could not run {what}"))?;
    if !status.success() {
        bail!("{what} failed ({status})");
    }
    Ok(())
}

/// `docker compose --project-directory <dir> -f <dir>/docker-compose.yml`.
pub fn compose_command(dir: &Path) -> Command {
    let mut command = Command::new("docker");
    command
        .arg("compose")
        .arg("--project-directory")
        .arg(dir)
        .arg("-f")
        .arg(dir.join("docker-compose.yml"));
    command
}
