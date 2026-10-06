//! Projeto de instalação: diretório com `docker-compose.yml` e `.env`.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, bail};

pub struct Project {
    pub dir: PathBuf,
}

impl Project {
    pub fn new(dir: &Path) -> Self {
        Project {
            dir: dir.to_path_buf(),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn exists(&self) -> bool {
        self.path("docker-compose.yml").is_file()
    }

    pub fn require(&self) -> anyhow::Result<()> {
        if !self.exists() {
            bail!(
                "nenhum projeto em {} (docker-compose.yml não encontrado). Rode `nelcota init` antes, ou use -C <dir>.",
                self.dir.display()
            );
        }
        Ok(())
    }

    fn compose_command(&self) -> Command {
        let mut command = Command::new("docker");
        command
            .arg("compose")
            .arg("--project-directory")
            .arg(&self.dir)
            .arg("-f")
            .arg(self.path("docker-compose.yml"));
        command
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
            bail!("`docker compose {}` falhou ({status})", args.join(" "));
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
                "`docker compose {}` falhou ({})",
                args.join(" "),
                output.status
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

    /// Espera o healthcheck do serviço ficar `healthy`.
    pub fn wait_healthy(&self, service: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let id = self.compose_output(&["ps", "-q", service])?;
            let id = id.trim();
            if !id.is_empty() {
                let output = Command::new("docker")
                    .args(["inspect", "--format", "{{.State.Health.Status}}", id])
                    .output()?;
                let health = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if health == "healthy" {
                    return Ok(());
                }
                if health == "unhealthy" {
                    bail!("o serviço {service} ficou unhealthy");
                }
            }
            if Instant::now() > deadline {
                bail!(
                    "o serviço {service} não ficou saudável em {}s",
                    timeout.as_secs()
                );
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn read_env(&self) -> anyhow::Result<Vec<(String, String)>> {
        let path = self.path(".env");
        let text = fs::read_to_string(&path)
            .with_context(|| format!("não foi possível ler {}", path.display()))?;
        Ok(parse_env(&text))
    }

    pub fn env_value(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .read_env()?
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .filter(|v| !v.is_empty()))
    }

    /// Troca (ou acrescenta) uma variável do `.env`, preservando o resto.
    pub fn set_env_value(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let path = self.path(".env");
        let text = fs::read_to_string(&path)?;
        let mut found = false;
        let mut lines: Vec<String> = text
            .lines()
            .map(|line| {
                if line.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
                    found = true;
                    format!("{key}={value}")
                } else {
                    line.to_owned()
                }
            })
            .collect();
        if !found {
            lines.push(format!("{key}={value}"));
        }
        write_private(&path, &(lines.join("\n") + "\n"))
    }
}

/// `CHAVE=valor`, ignorando comentários; aspas simples/duplas são removidas.
pub fn parse_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .or_else(|| v.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
                .unwrap_or(v);
            (k.trim().to_owned(), v.to_owned())
        })
        .collect()
}

/// Grava um arquivo legível só pelo dono (0600 em Unix).
pub fn write_private(path: &Path, content: &str) -> anyhow::Result<()> {
    fs::write(path, content)
        .with_context(|| format!("não foi possível gravar {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_env_com_aspas_e_comentarios() {
        let env = parse_env("# comentário\nA=1\nB='$argon2id$v=19$x'\nC=\"com espaço\"\n\nD=\n");
        assert_eq!(
            env,
            vec![
                ("A".into(), "1".into()),
                ("B".into(), "$argon2id$v=19$x".into()),
                ("C".into(), "com espaço".into()),
                ("D".into(), String::new()),
            ]
        );
    }
}
