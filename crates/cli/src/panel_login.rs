//! Login dos painéis: único para o host (padrão) ou um por projeto.
//!
//! Este módulo é o único que escreve as credenciais de admin nos `.env`.
//!
//! - **Único** (`shared`): email, hash e segredo de SSO ficam no `host.env` e
//!   são copiados para todos os projetos. Entrar num painel abre os outros
//!   (handoff de SSO no seletor de projetos).
//! - **Por projeto** (`per-project`): cada projeto tem email e senha próprios;
//!   sem segredo de SSO, o seletor só leva ao painel do outro projeto, que
//!   pede o login dele.

use crate::{
    envfile::EnvFile,
    host::{Host, Manifest, PanelLogin},
    project::Project,
    util,
};

const EMAIL: &str = "NELCOTA_ADMIN_EMAIL";
const HASH: &str = "NELCOTA_ADMIN_PASSWORD_HASH";
const SSO_SECRET: &str = "NELCOTA_ADMIN_SSO_SECRET";

/// Senha gerada agora (mostrada uma única vez; só o hash é gravado).
pub struct NewPassword {
    pub scope: String,
    pub email: String,
    pub password: String,
}

fn hash(password: &str) -> anyhow::Result<String> {
    nelcota_auth::hash_password(password)
        .ok_or_else(|| anyhow::anyhow!("falha ao gerar o hash da senha"))
}

fn new_credentials(file: &EnvFile, email: &str, scope: &str) -> anyhow::Result<NewPassword> {
    let password = util::secret(20);
    file.set(EMAIL, email)?;
    file.set(HASH, &hash(&password)?)?;
    Ok(NewPassword {
        scope: scope.to_owned(),
        email: email.to_owned(),
        password,
    })
}

/// Email padrão do admin do host.
pub fn host_email(host: &Host) -> anyhow::Result<Option<String>> {
    host.secrets().get(EMAIL)
}

/// Credenciais do host novas (email, senha e segredo de SSO) no `host.env`.
pub fn init_host(host: &Host, email: &str) -> anyhow::Result<NewPassword> {
    let secrets = host.secrets();
    secrets.set(SSO_SECRET, &util::secret(48))?;
    new_credentials(&secrets, email, "todos os projetos")
}

/// Escreve no `.env` do projeto as credenciais do modo atual. No modo por
/// projeto, gera credenciais próprias se o projeto ainda não tiver.
pub fn apply(
    host: &Host,
    manifest: &Manifest,
    project: &Project,
) -> anyhow::Result<Option<NewPassword>> {
    let env = project.env();
    let secrets = host.secrets();
    match manifest.panel_login {
        PanelLogin::Shared => {
            for key in [EMAIL, HASH, SSO_SECRET] {
                match secrets.get(key)? {
                    Some(value) => env.set(key, &value)?,
                    None => env.remove(key)?,
                }
            }
            Ok(None)
        }
        PanelLogin::PerProject => {
            env.remove(SSO_SECRET)?;
            let shared_hash = secrets.get(HASH)?;
            let own = env.get(HASH)?.filter(|h| Some(h) != shared_hash.as_ref());
            if own.is_some() {
                return Ok(None);
            }
            let email = host_email(host)?.unwrap_or_else(|| format!("admin@{}", project.name));
            new_credentials(&env, &email, &project.name).map(Some)
        }
    }
}

/// Troca o modo de login do host e reaplica em todos os projetos.
pub fn switch(
    host: &Host,
    manifest: &mut Manifest,
    mode: PanelLogin,
) -> anyhow::Result<Vec<NewPassword>> {
    let mut generated = Vec::new();
    if mode == PanelLogin::Shared && host.secrets().get(SSO_SECRET)?.is_none() {
        host.secrets().set(SSO_SECRET, &util::secret(48))?;
    }
    if mode == PanelLogin::PerProject {
        // Força credenciais novas: a senha única não deve continuar valendo.
        for project in host.projects(manifest) {
            project.env().remove(HASH)?;
        }
    }
    manifest.panel_login = mode;
    host.save(manifest)?;
    for project in host.projects(manifest) {
        if let Some(new) = apply(host, manifest, &project)? {
            generated.push(new);
        }
    }
    Ok(generated)
}

/// Senha nova: do host (login único) ou do projeto (login por projeto).
pub fn reset(
    host: &Host,
    manifest: &Manifest,
    project: Option<&Project>,
) -> anyhow::Result<NewPassword> {
    match (manifest.panel_login, project) {
        (PanelLogin::Shared, _) => {
            let email = host_email(host)?.unwrap_or_else(|| "admin@localhost".into());
            let new = new_credentials(&host.secrets(), &email, "todos os projetos")?;
            for project in host.projects(manifest) {
                apply(host, manifest, &project)?;
            }
            Ok(new)
        }
        (PanelLogin::PerProject, Some(project)) => {
            let env = project.env();
            let email = env
                .get(EMAIL)?
                .unwrap_or_else(|| format!("admin@{}", project.name));
            new_credentials(&env, &email, &project.name)
        }
        (PanelLogin::PerProject, None) => {
            anyhow::bail!("login por projeto: escolha o projeto com -p <nome>")
        }
    }
}

pub fn print(passwords: &[NewPassword]) {
    for p in passwords {
        println!();
        println!("  Painel ({}):", p.scope);
        println!("    email: {}", p.email);
        println!("    senha: {}", p.password);
    }
    if !passwords.is_empty() {
        println!("  (senhas mostradas só agora; os arquivos guardam apenas o hash argon2id)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::ProjectEntry;

    fn setup(name: &str) -> (Host, Manifest) {
        let root =
            std::env::temp_dir().join(format!("nelcota-login-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let host = Host::new(&root);
        let manifest = Manifest {
            version: 1,
            panel_login: PanelLogin::Shared,
            base_domain: None,
            local: true,
            image: "nelcota".into(),
            projects: vec![
                ProjectEntry {
                    name: "loja".into(),
                    domain: "loja.localhost".into(),
                },
                ProjectEntry {
                    name: "blog".into(),
                    domain: "blog.localhost".into(),
                },
            ],
        };
        for p in &manifest.projects {
            std::fs::create_dir_all(host.project_dir(&p.name)).unwrap();
            std::fs::write(
                host.project_dir(&p.name).join(".env"),
                "POSTGRES_PASSWORD=x\n",
            )
            .unwrap();
        }
        host.save(&manifest).unwrap();
        (host, manifest)
    }

    #[test]
    fn login_unico_copia_credenciais_e_segredo_para_todos() {
        let (host, manifest) = setup("shared");
        init_host(&host, "admin@exemplo.com").unwrap();
        for project in host.projects(&manifest) {
            assert!(apply(&host, &manifest, &project).unwrap().is_none());
            let env = project.env();
            assert_eq!(env.get(EMAIL).unwrap(), host.secrets().get(EMAIL).unwrap());
            assert_eq!(env.get(HASH).unwrap(), host.secrets().get(HASH).unwrap());
            assert!(env.get(SSO_SECRET).unwrap().is_some_and(|s| s.len() >= 32));
            assert_eq!(env.get("POSTGRES_PASSWORD").unwrap().as_deref(), Some("x"));
        }
        std::fs::remove_dir_all(host.root()).unwrap();
    }

    #[test]
    fn trocar_para_por_projeto_gera_senhas_proprias_e_tira_o_sso() {
        let (host, mut manifest) = setup("switch");
        init_host(&host, "admin@exemplo.com").unwrap();
        for project in host.projects(&manifest) {
            apply(&host, &manifest, &project).unwrap();
        }
        let generated = switch(&host, &mut manifest, PanelLogin::PerProject).unwrap();
        assert_eq!(generated.len(), 2);
        let shared_hash = host.secrets().get(HASH).unwrap();
        for project in host.projects(&manifest) {
            let env = project.env();
            assert!(env.get(SSO_SECRET).unwrap().is_none());
            assert_ne!(env.get(HASH).unwrap(), shared_hash);
        }
        assert_eq!(host.manifest().unwrap().panel_login, PanelLogin::PerProject);

        // E de volta para o login único.
        assert!(
            switch(&host, &mut manifest, PanelLogin::Shared)
                .unwrap()
                .is_empty()
        );
        for project in host.projects(&manifest) {
            assert_eq!(project.env().get(HASH).unwrap(), shared_hash);
        }
        std::fs::remove_dir_all(host.root()).unwrap();
    }
}
