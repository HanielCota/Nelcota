//! Um projeto do host: pasta com `docker-compose.yml` e `.env`, e as
//! operações de `docker compose` sobre ele.

use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, bail};

use crate::envfile::EnvFile;

pub struct Project {
    pub name: String,
    pub dir: PathBuf,
}

impl Project {
    pub fn new(name: &str, dir: &Path) -> Self {
        Project {
            name: name.to_owned(),
            dir: dir.to_path_buf(),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn env(&self) -> EnvFile {
        EnvFile::new(self.path(".env"))
    }

    pub fn exists(&self) -> bool {
        self.path("docker-compose.yml").is_file()
    }

    fn compose_command(&self) -> Command {
        compose_command(&self.dir)
    }

    /// `docker compose <args>` com a saída no terminal.
    pub fn compose(&self, args: &[&str]) -> anyhow::Result<ExitStatus> {
        self.compose_command()
            .args(args)
            .status()
            .context("não foi possível executar `docker compose` (o Docker está instalado?)")
    }

    pub fn compose_ok(&self, args: &[&str]) -> anyhow::Result<()> {
        let status = self.compose(args)?;
        if !status.success() {
            bail!(
                "[{}] `docker compose {}` falhou ({status})",
                self.name,
                args.join(" ")
            );
        }
        Ok(())
    }

    /// `docker compose <args>` capturando a saída padrão.
    pub fn compose_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = self
            .compose_command()
            .args(args)
            .stderr(Stdio::inherit())
            .output()
            .context("não foi possível executar `docker compose`")?;
        if !output.status.success() {
            bail!(
                "[{}] `docker compose {}` falhou ({})",
                self.name,
                args.join(" "),
                output.status
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// `docker compose` com stdin/stdout ligados a arquivos (pg_dump/pg_restore).
    pub fn compose_piped(
        &self,
        args: &[&str],
        stdin: Stdio,
        stdout: Stdio,
    ) -> anyhow::Result<ExitStatus> {
        self.compose_command()
            .args(args)
            .stdin(stdin)
            .stdout(stdout)
            .status()
            .context("não foi possível executar `docker compose`")
    }

    /// Roda `nelcota <args>` dentro do container `app`.
    pub fn in_app(&self, args: &[&str]) -> anyhow::Result<()> {
        let mut full = vec!["exec", "-T", "app", "/usr/local/bin/nelcota"];
        full.extend_from_slice(args);
        self.compose_ok(&full)
    }

    pub fn in_app_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let mut full = vec!["exec", "-T", "app", "/usr/local/bin/nelcota"];
        full.extend_from_slice(args);
        self.compose_output(&full)
    }

    /// O container do serviço existe (criado, rodando ou parado)?
    pub fn has_container(&self, service: &str) -> bool {
        self.compose_output(&["ps", "-a", "-q", service])
            .map(|out| !out.trim().is_empty())
            .unwrap_or(false)
    }

    /// Estado do healthcheck do serviço (`healthy`, `starting`, ...), se rodando.
    pub fn health(&self, service: &str) -> Option<String> {
        let id = self.compose_output(&["ps", "-q", service]).ok()?;
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

    /// Espera o healthcheck do serviço ficar `healthy`.
    pub fn wait_healthy(&self, service: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.health(service).as_deref() {
                Some("healthy") => return Ok(()),
                Some("unhealthy") => bail!("[{}] o serviço {service} ficou unhealthy", self.name),
                _ => {}
            }
            if Instant::now() > deadline {
                bail!(
                    "[{}] o serviço {service} não ficou saudável em {}s",
                    self.name,
                    timeout.as_secs()
                );
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
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
