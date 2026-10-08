//! Starting, stopping and observing projects and the host proxy.
use crate::{
    caddy,
    host::{Host, Manifest, Runtime},
    project::{Project, Service},
    util::{ok, step, warn},
};
use std::time::Duration;
pub(crate) const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Starts the projects (each one until its healthcheck passes) and Caddy.
pub fn up(host: &Host, manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    if manifest.runtime == Runtime::Docker {
        caddy::ensure_network()?;
    }
    caddy::write(host, manifest)?;
    for project in projects {
        step(&format!("Starting {}", project.name));
        project.up()?;
        project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
        ok(&format!("{} healthy", project.name));
    }
    caddy::up(host, manifest.runtime)?;
    println!();
    for entry in manifest
        .projects
        .iter()
        .filter(|e| projects.iter().any(|p| p.name == e.name))
    {
        println!("  {:<16} {}/health", entry.name, entry.url());
    }
    Ok(())
}

pub fn down(project: &Project, volumes: bool) -> anyhow::Result<()> {
    if volumes && project.runtime == Runtime::Docker {
        warn(&format!(
            "--volumes: the data of {} will be DELETED",
            project.name
        ));
    }
    project.down(volumes)
}

pub fn status(manifest: &Manifest, projects: &[Project]) -> anyhow::Result<()> {
    println!("{:<16} {:<32} {:<10} VERSION", "PROJECT", "DOMAIN", "APP");
    for project in projects {
        let domain = manifest
            .projects
            .iter()
            .find(|e| e.name == project.name)
            .map_or("", |e| e.domain.as_str());
        let health = project
            .health(Service::App)
            .unwrap_or_else(|| "stopped".into());
        let version = project.env().get("NELCOTA_VERSION")?.unwrap_or_default();
        println!(
            "{:<16} {:<32} {:<10} {}",
            project.name, domain, health, version
        );
    }
    Ok(())
}

pub fn logs(project: &Project, follow: bool, service: Option<&str>) -> anyhow::Result<()> {
    project.logs(follow, service)
}

/// Recreates the apps that already exist (to apply `.env` changes).
pub fn recreate_apps(projects: &[Project]) -> anyhow::Result<()> {
    for project in projects.iter().filter(|p| p.has_app()) {
        step(&format!("Restarting the {} app", project.name));
        project.recreate(Service::App)?;
        project.wait_healthy(Service::App, HEALTH_TIMEOUT)?;
    }
    Ok(())
}
