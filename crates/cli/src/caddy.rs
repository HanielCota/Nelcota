//! Caddy compartilhado: um site por projeto, gerado a partir do registro.

use std::{fs, process::Command};

use anyhow::{Context, bail};

use crate::{
    host::{Host, Manifest},
    project::compose_command,
    util::ok,
};

/// Rede Docker que liga o Caddy aos apps dos projetos.
pub const NETWORK: &str = "nelcota_edge";

const COMPOSE: &str = include_str!("../../../deploy/caddy-compose.yml");

/// Caddyfile com um bloco por projeto.
pub fn render(manifest: &Manifest) -> String {
    let mut out = String::from(
        "# Gerado pelo nelcota a partir de nelcota-host.json. Não edite à mão:\n\
         # use `nelcota init` / `nelcota remove`.\n",
    );
    for project in &manifest.projects {
        let tls = if manifest.local {
            "\ttls internal\n"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n{domain} {{\n{tls}\tencode zstd gzip\n\treverse_proxy app-{name}:8000\n\theader {{\n\
             \t\t-Server\n\t\tStrict-Transport-Security \"max-age=31536000\"\n\
             \t\tX-Content-Type-Options \"nosniff\"\n\t}}\n}}\n",
            domain = project.domain,
            name = project.name,
        ));
    }
    out
}

/// Grava o compose do Caddy (se ainda não existir) e o Caddyfile.
pub fn write(host: &Host, manifest: &Manifest) -> anyhow::Result<()> {
    let dir = host.caddy_dir();
    fs::create_dir_all(dir.join("config"))?;
    let compose = dir.join("docker-compose.yml");
    if !compose.is_file() {
        fs::write(&compose, COMPOSE)?;
    }
    // Troca atômica dentro da pasta montada (bind de arquivo único não veria o rename).
    let tmp = dir.join("config").join("Caddyfile.tmp");
    fs::write(&tmp, render(manifest))?;
    fs::rename(&tmp, dir.join("config").join("Caddyfile"))?;
    Ok(())
}

pub fn ensure_network() -> anyhow::Result<()> {
    let exists = Command::new("docker")
        .args(["network", "inspect", NETWORK])
        .output()
        .context("Docker não encontrado")?
        .status
        .success();
    if !exists {
        let status = Command::new("docker")
            .args(["network", "create", NETWORK])
            .status()?;
        if !status.success() {
            bail!("não foi possível criar a rede {NETWORK}");
        }
    }
    Ok(())
}

fn running(host: &Host) -> bool {
    compose_command(&host.caddy_dir())
        .args(["ps", "-q", "caddy"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

/// Sobe o Caddy (ou recarrega a configuração, se já estiver no ar).
pub fn up(host: &Host) -> anyhow::Result<()> {
    if running(host) {
        return reload(host);
    }
    let status = compose_command(&host.caddy_dir())
        .args(["up", "-d"])
        .status()?;
    if !status.success() {
        bail!("não foi possível subir o Caddy");
    }
    ok("caddy no ar");
    Ok(())
}

/// Aplica o Caddyfile novo sem derrubar conexões (se o Caddy estiver no ar).
pub fn reload(host: &Host) -> anyhow::Result<()> {
    if !running(host) {
        return Ok(());
    }
    let status = compose_command(&host.caddy_dir())
        .args([
            "exec",
            "-T",
            "caddy",
            "caddy",
            "reload",
            "--config",
            "/etc/caddy/Caddyfile",
            "--adapter",
            "caddyfile",
        ])
        .status()?;
    if !status.success() {
        bail!("o Caddy recusou a configuração nova (veja `docker logs nelcota-edge-caddy-1`)");
    }
    ok("caddy recarregado");
    Ok(())
}

pub fn down(host: &Host, volumes: bool) -> anyhow::Result<()> {
    let mut args = vec!["down"];
    if volumes {
        args.push("--volumes");
    }
    compose_command(&host.caddy_dir()).args(&args).status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{PanelLogin, ProjectEntry};

    fn manifest(local: bool) -> Manifest {
        Manifest {
            version: 1,
            panel_login: PanelLogin::Shared,
            base_domain: None,
            local,
            image: "nelcota".into(),
            projects: vec![
                ProjectEntry {
                    name: "loja".into(),
                    domain: "api.loja.com".into(),
                },
                ProjectEntry {
                    name: "blog".into(),
                    domain: "blog.exemplo.com".into(),
                },
            ],
        }
    }

    #[test]
    fn um_site_por_projeto() {
        let caddyfile = render(&manifest(false));
        assert!(
            caddyfile.contains("api.loja.com {\n\tencode zstd gzip\n\treverse_proxy app-loja:8000")
        );
        assert!(caddyfile.contains("blog.exemplo.com {"));
        assert!(caddyfile.contains("reverse_proxy app-blog:8000"));
        assert!(!caddyfile.contains("tls internal"));
        assert!(render(&manifest(true)).contains("api.loja.com {\n\ttls internal\n"));
    }
}
