//! Checagens do ambiente antes de instalar: Docker, portas, RAM e DNS.

use std::{
    fs,
    net::{IpAddr, TcpListener, ToSocketAddrs},
    process::Command,
};

use anyhow::bail;

use crate::util::{ok, warn};

pub fn docker() -> anyhow::Result<()> {
    match Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
    {
        Ok(out) if out.status.success() => {
            ok(&format!(
                "Docker {}",
                String::from_utf8_lossy(&out.stdout).trim()
            ));
        }
        Ok(_) => {
            bail!("o Docker está instalado mas o daemon não responde (systemctl start docker)")
        }
        Err(_) => {
            bail!("Docker não encontrado. Instale com: curl -fsSL https://get.docker.com | sh")
        }
    }
    match Command::new("docker")
        .args(["compose", "version", "--short"])
        .output()
    {
        Ok(out) if out.status.success() => {
            ok(&format!(
                "Docker Compose {}",
                String::from_utf8_lossy(&out.stdout).trim()
            ));
            Ok(())
        }
        _ => bail!("plugin `docker compose` não encontrado (apt install docker-compose-plugin)"),
    }
}

pub fn ports() {
    for port in [80u16, 443] {
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(_) => ok(&format!("porta {port} livre")),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                warn(&format!(
                    "sem permissão para testar a porta {port} (rode como root para checar)"
                ));
            }
            Err(_) => warn(&format!(
                "porta {port} em uso: pare o serviço que a ocupa (nginx? apache?)"
            )),
        }
    }
}

/// RAM total da máquina em MB (Linux).
pub fn total_ram_mb() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// O domínio aponta para o IP público desta máquina?
pub fn dns(domain: &str) {
    let resolved: Vec<IpAddr> = (domain, 443)
        .to_socket_addrs()
        .map(|addrs| addrs.map(|a| a.ip()).collect())
        .unwrap_or_default();
    if resolved.is_empty() {
        warn(&format!(
            "{domain} não resolve no DNS ainda: crie um registro A apontando para esta máquina \
             (o HTTPS só funciona depois disso)"
        ));
        return;
    }
    let public = Command::new("curl")
        .args(["-fsS", "--max-time", "4", "https://api.ipify.org"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<IpAddr>()
                .ok()
        });
    match public {
        Some(ip) if resolved.contains(&ip) => ok(&format!("DNS: {domain} → {ip} (esta máquina)")),
        Some(ip) => warn(&format!(
            "DNS: {domain} → {resolved:?}, mas o IP público desta máquina é {ip}"
        )),
        None => ok(&format!(
            "DNS: {domain} → {resolved:?} (IP público não verificado)"
        )),
    }
}
