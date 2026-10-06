//! Host: a pasta raiz que reúne o Caddy compartilhado e os projetos.
//!
//! ```text
//! <raiz>/
//! ├── nelcota-host.json   registro (fonte da verdade): modo de login, domínio base, projetos
//! ├── host.env            segredos do host (0600): admin, segredo de SSO, S3
//! ├── shared/             lista pública de projetos, montada nos apps (somente leitura)
//! ├── caddy/              Caddy compartilhado
//! └── projects/<nome>/    um projeto isolado (compose, .env, migrations/, backups/)
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::{envfile::EnvFile, naming, project::Project};

pub const MANIFEST: &str = "nelcota-host.json";

/// Como os admins entram nos painéis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum PanelLogin {
    /// Um login para todos os painéis do host (SSO entre projetos).
    #[default]
    Shared,
    /// Cada projeto com o próprio login.
    PerProject,
}

impl PanelLogin {
    pub fn as_str(self) -> &'static str {
        match self {
            PanelLogin::Shared => "shared",
            PanelLogin::PerProject => "per-project",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub domain: String,
}

impl ProjectEntry {
    pub fn url(&self) -> String {
        format!("https://{}", self.domain)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub panel_login: PanelLogin,
    /// Domínio base para subdomínios (`--project loja` → `loja.<base>`).
    #[serde(default)]
    pub base_domain: Option<String>,
    /// Instalação local: HTTPS com certificado interno do Caddy.
    #[serde(default)]
    pub local: bool,
    /// Imagem do app (a tag fica no `.env` de cada projeto).
    pub image: String,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
}

pub struct Host {
    root: PathBuf,
}

impl Host {
    pub fn new(root: &Path) -> Self {
        Host {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn exists(&self) -> bool {
        self.root.join(MANIFEST).is_file()
    }

    pub fn require(&self) -> anyhow::Result<Manifest> {
        if !self.exists() {
            bail!(
                "nenhum host em {} ({MANIFEST} não encontrado). Rode `nelcota init` ou use -C <raiz>.",
                self.root.display()
            );
        }
        self.manifest()
    }

    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        let path = self.root.join(MANIFEST);
        let bytes =
            fs::read(&path).with_context(|| format!("não foi possível ler {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("{} inválido", path.display()))
    }

    /// Grava o registro de forma atômica (arquivo temporário + rename).
    pub fn save(&self, manifest: &Manifest) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(format!("{MANIFEST}.tmp"));
        fs::write(&tmp, serde_json::to_string_pretty(manifest)? + "\n")?;
        fs::rename(&tmp, self.root.join(MANIFEST))?;
        Ok(())
    }

    pub fn secrets(&self) -> EnvFile {
        EnvFile::new(self.root.join("host.env"))
    }

    pub fn caddy_dir(&self) -> PathBuf {
        self.root.join("caddy")
    }

    pub fn shared_dir(&self) -> PathBuf {
        self.root.join("shared")
    }

    pub fn archive_dir(&self) -> PathBuf {
        self.root.join("archive")
    }

    pub fn project_dir(&self, name: &str) -> PathBuf {
        self.root.join("projects").join(name)
    }

    pub fn project(&self, entry: &ProjectEntry) -> Project {
        Project::new(&entry.name, &self.project_dir(&entry.name))
    }

    pub fn projects(&self, manifest: &Manifest) -> Vec<Project> {
        manifest.projects.iter().map(|e| self.project(e)).collect()
    }

    /// Projeto pelo `-p`; sem `-p`, o único projeto do host.
    pub fn select(&self, manifest: &Manifest, name: Option<&str>) -> anyhow::Result<Project> {
        let entry = select_entry(manifest, name)?;
        Ok(self.project(entry))
    }
}

pub fn select_entry<'a>(
    manifest: &'a Manifest,
    name: Option<&str>,
) -> anyhow::Result<&'a ProjectEntry> {
    match (name, manifest.projects.as_slice()) {
        (Some(name), projects) => {
            naming::validate_project_name(name)?;
            projects.iter().find(|p| p.name == name).ok_or_else(|| {
                anyhow::anyhow!("projeto '{name}' não existe (veja `nelcota projects`)")
            })
        }
        (None, [only]) => Ok(only),
        (None, []) => bail!("nenhum projeto ainda: crie um com `nelcota init`"),
        (None, projects) => {
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            bail!(
                "há {} projetos ({}): escolha um com -p <nome>",
                names.len(),
                names.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(names: &[&str]) -> Manifest {
        Manifest {
            version: 1,
            panel_login: PanelLogin::Shared,
            base_domain: None,
            local: true,
            image: "nelcota".into(),
            projects: names
                .iter()
                .map(|n| ProjectEntry {
                    name: (*n).into(),
                    domain: format!("{n}.localhost"),
                })
                .collect(),
        }
    }

    #[test]
    fn selecao_de_projeto() {
        let one = manifest(&["loja"]);
        assert_eq!(select_entry(&one, None).unwrap().name, "loja");
        let two = manifest(&["loja", "blog"]);
        assert!(
            select_entry(&two, None)
                .unwrap_err()
                .to_string()
                .contains("-p <nome>")
        );
        assert_eq!(select_entry(&two, Some("blog")).unwrap().name, "blog");
        assert!(select_entry(&two, Some("nada")).is_err());
        assert!(select_entry(&two, Some("../etc")).is_err());
        assert!(select_entry(&manifest(&[]), None).is_err());
    }

    #[test]
    fn registro_ida_e_volta() {
        let root = std::env::temp_dir().join(format!("nelcota-host-{}", std::process::id()));
        let host = Host::new(&root);
        let m = manifest(&["loja"]);
        host.save(&m).unwrap();
        let back = host.manifest().unwrap();
        assert_eq!(back.projects, m.projects);
        assert_eq!(back.panel_login, PanelLogin::Shared);
        assert!(
            fs::read_to_string(root.join(MANIFEST))
                .unwrap()
                .contains("\"panel_login\": \"shared\"")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
