//! Environment checks before installing: Docker, ports, RAM and DNS.

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
            bail!("Docker is installed but the daemon does not answer (systemctl start docker)")
        }
        Err(_) => {
            bail!("Docker not found. Install it with: curl -fsSL https://get.docker.com | sh")
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
        _ => bail!("`docker compose` plugin not found (apt install docker-compose-plugin)"),
    }
}

pub fn ports() {
    for port in [80u16, 443] {
        match TcpListener::bind(("0.0.0.0", port)) {
            Ok(_) => ok(&format!("port {port} free")),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                warn(&format!(
                    "no permission to test port {port} (run as root to check)"
                ));
            }
            Err(_) => warn(&format!(
                "port {port} in use: stop the service holding it (nginx? apache?)"
            )),
        }
    }
}

/// Total machine RAM in MB (Linux).
pub fn total_ram_mb() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Does the domain point at this machine's public IP?
pub fn dns(domain: &str) {
    let resolved: Vec<IpAddr> = (domain, 443)
        .to_socket_addrs()
        .map(|addrs| addrs.map(|a| a.ip()).collect())
        .unwrap_or_default();
    if resolved.is_empty() {
        warn(&format!(
            "{domain} does not resolve in DNS yet: create an A record pointing at this machine \
             (HTTPS only works after that)"
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
        Some(ip) if resolved.contains(&ip) => ok(&format!("DNS: {domain} → {ip} (this machine)")),
        Some(ip) => warn(&format!(
            "DNS: {domain} → {resolved:?}, but this machine's public IP is {ip}"
        )),
        None => ok(&format!(
            "DNS: {domain} → {resolved:?} (public IP not verified)"
        )),
    }
}
