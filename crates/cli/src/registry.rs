//! Public project list (`shared/projects.json`), mounted read-only into the
//! apps for the panel's project switcher. Name and URL only: never secrets.

use std::fs;

use serde_json::json;

use crate::host::{Host, Manifest};

pub fn render(manifest: &Manifest) -> String {
    let projects: Vec<_> = manifest
        .projects
        .iter()
        .map(|p| json!({ "name": p.name, "url": p.url() }))
        .collect();
    serde_json::to_string_pretty(&json!({
        "panel_login": manifest.panel_login.as_str(),
        "projects": projects,
    }))
    .expect("JSON serializes")
        + "\n"
}

/// Writes atomically into the mounted folder (the apps reread it on every request).
pub fn write(host: &Host, manifest: &Manifest) -> anyhow::Result<()> {
    let dir = host.shared_dir();
    fs::create_dir_all(&dir)?;
    let tmp = dir.join("projects.json.tmp");
    fs::write(&tmp, render(manifest))?;
    fs::rename(&tmp, dir.join("projects.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{PanelLogin, ProjectEntry};

    #[test]
    fn only_name_and_url() {
        let manifest = Manifest {
            version: 1,
            panel_login: PanelLogin::PerProject,
            base_domain: Some("example.com".into()),
            local: false,
            runtime: crate::host::Runtime::Docker,
            image: "nelcota".into(),
            projects: vec![ProjectEntry {
                name: "shop".into(),
                domain: "shop.example.com".into(),
            }],
        };
        let value: serde_json::Value = serde_json::from_str(&render(&manifest)).unwrap();
        assert_eq!(
            value,
            json!({
                "panel_login": "per-project",
                "projects": [{ "name": "shop", "url": "https://shop.example.com" }],
            })
        );
    }
}
