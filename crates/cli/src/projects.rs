//! Comandos sobre o conjunto de projetos: listar, remover e trocar o modo
//! de login dos painéis.

use std::fs;

use anyhow::bail;

use crate::{
    caddy,
    host::{Host, PanelLogin},
    naming, ops, panel_login, registry,
    util::{self, ok, step, warn},
};

/// `nelcota projects`
pub fn list(host: &Host) -> anyhow::Result<()> {
    let manifest = host.require()?;
    println!(
        "Login dos painéis: {}{}",
        manifest.panel_login.as_str(),
        manifest
            .base_domain
            .as_deref()
            .map(|b| format!(" · domínio base: {b}"))
            .unwrap_or_default()
    );
    println!();
    if manifest.projects.is_empty() {
        println!("Nenhum projeto. Crie com `nelcota init`.");
        return Ok(());
    }
    ops::status(&manifest, &host.projects(&manifest))
}

/// `nelcota remove -p <nome>`: backup final em `archive/`, containers e dados
/// apagados, projeto fora do Caddy e do registro.
pub fn remove(host: &Host, name: &str, yes: bool, keep_files: bool) -> anyhow::Result<()> {
    naming::validate_project_name(name)?;
    let mut manifest = host.require()?;
    let Some(entry) = manifest.projects.iter().find(|p| p.name == name).cloned() else {
        bail!("projeto '{name}' não existe");
    };
    if !yes
        && !(util::interactive()
            && util::confirm(&format!(
                "Remover o projeto {name} ({})? Um backup final vai para archive/ e os dados são apagados.",
                entry.domain
            )))
    {
        bail!("remoção cancelada (use --yes para não perguntar)");
    }

    let project = host.project(&entry);
    if project.health("postgres").is_some() {
        let dump = ops::backup(host, &project, false, None)?;
        fs::create_dir_all(host.archive_dir())?;
        let archived = host
            .archive_dir()
            .join(dump.file_name().unwrap_or_default());
        fs::copy(&dump, &archived)?;
        ok(&format!("backup final em {}", archived.display()));
    } else {
        warn("Postgres parado: removendo sem backup final");
    }
    if project.exists() {
        ops::down(&project, true)?;
    }

    manifest.projects.retain(|p| p.name != name);
    host.save(&manifest)?;
    registry::write(host, &manifest)?;
    caddy::write(host, &manifest)?;
    caddy::reload(host)?;

    if keep_files {
        ok(&format!("arquivos mantidos em {}", project.dir.display()));
    } else {
        fs::remove_dir_all(&project.dir)?;
        ok(&format!("pasta {} removida", project.dir.display()));
    }
    ok(&format!("projeto {name} removido"));
    Ok(())
}

/// `nelcota panel-login shared|per-project`
pub fn set_panel_login(host: &Host, mode: PanelLogin) -> anyhow::Result<()> {
    let mut manifest = host.require()?;
    if manifest.panel_login == mode {
        println!("O login dos painéis já é {}.", mode.as_str());
        return Ok(());
    }
    step(&format!(
        "Login dos painéis: {} → {}",
        manifest.panel_login.as_str(),
        mode.as_str()
    ));
    let generated = panel_login::switch(host, &mut manifest, mode)?;
    registry::write(host, &manifest)?;
    ops::recreate_apps(&host.projects(&manifest))?;
    if mode == PanelLogin::Shared {
        println!();
        println!("Login único ativo: use o email e a senha do host em qualquer painel.");
        println!("(Esqueceu a senha? `nelcota admin-password`.)");
    }
    panel_login::print(&generated);
    Ok(())
}
