//! Lista pública de projetos (`shared/projects.json`), montada somente leitura
//! nos apps para o seletor do painel. Só nome e URL: nunca segredos.

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
    .expect("JSON serializa")
        + "\n"
}

/// Grava de forma atômica na pasta montada (os apps releem a cada request).
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
    fn so_nome_e_url() {
        let manifest = Manifest {
            version: 1,
            panel_login: PanelLogin::PerProject,
            base_domain: Some("exemplo.com".into()),
            local: false,
            image: "nelcota".into(),
            projects: vec![ProjectEntry {
                name: "loja".into(),
                domain: "loja.exemplo.com".into(),
            }],
        };
        let value: serde_json::Value = serde_json::from_str(&render(&manifest)).unwrap();
        assert_eq!(
            value,
            json!({
                "panel_login": "per-project",
                "projects": [{ "name": "loja", "url": "https://loja.exemplo.com" }],
            })
        );
    }
}
