//! Arquivos de um projeto novo: `docker-compose.yml`, `.env` (segredos do
//! projeto), `migrations/` e `backups/`.

use std::fs;

use anyhow::bail;

use crate::{
    envfile::write_private,
    host::{Host, ProjectEntry},
    util,
};

const COMPOSE_TEMPLATE: &str = include_str!("../../../deploy/project-compose.tmpl.yml");
const PROFILES: [(&str, &str); 4] = [
    (
        "1gb",
        include_str!("../../../deploy/postgres/profiles/1gb.conf"),
    ),
    (
        "2gb",
        include_str!("../../../deploy/postgres/profiles/2gb.conf"),
    ),
    (
        "4gb",
        include_str!("../../../deploy/postgres/profiles/4gb.conf"),
    ),
    (
        "8gb",
        include_str!("../../../deploy/postgres/profiles/8gb.conf"),
    ),
];

pub struct NewProject<'a> {
    pub entry: &'a ProjectEntry,
    pub profile: &'a str,
    pub image: &'a str,
    pub version: &'a str,
}

/// Perfil do Postgres para a RAM disponível a cada projeto.
pub fn profile_for(ram_mb: u64) -> &'static str {
    match ram_mb {
        0..1536 => "1gb",
        1536..3072 => "2gb",
        3072..6144 => "4gb",
        _ => "8gb",
    }
}

/// Perfil pedido (validado) ou escolhido pela RAM dividida entre os projetos.
pub fn choose_profile(
    requested: Option<&str>,
    ram_mb: u64,
    projects: usize,
) -> anyhow::Result<&'static str> {
    match requested {
        Some(p) => PROFILES
            .iter()
            .map(|(name, _)| *name)
            .find(|name| *name == p)
            .ok_or_else(|| anyhow::anyhow!("perfil desconhecido: {p} (use 1gb, 2gb, 4gb ou 8gb)")),
        None => Ok(profile_for(ram_mb / projects.max(1) as u64)),
    }
}

/// Converte um perfil `.conf` em argumentos `-c chave=valor` do compose.
pub fn postgres_args(profile: &str) -> String {
    let conf = PROFILES
        .iter()
        .find(|(name, _)| *name == profile)
        .map_or("", |(_, conf)| conf);
    conf.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| format!("      - -c\n      - {}={}", k.trim(), v.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_compose(project: &str, profile: &str) -> String {
    COMPOSE_TEMPLATE
        .replace("{{GENERATED_AT}}", &util::timestamp())
        .replace("{{PROFILE}}", profile)
        .replace("{{POSTGRES_ARGS}}", &postgres_args(profile))
        .replace("{{PROJECT}}", project)
}

fn pool_size(profile: &str) -> u32 {
    match profile {
        "1gb" => 8,
        "2gb" => 15,
        _ => 25,
    }
}

/// Cria a pasta do projeto com segredos próprios. Falha se já existir.
pub fn create(host: &Host, new: &NewProject) -> anyhow::Result<()> {
    let dir = host.project_dir(&new.entry.name);
    if dir.join("docker-compose.yml").exists() {
        bail!("já existe um projeto em {}", dir.display());
    }
    fs::create_dir_all(dir.join("migrations"))?;
    fs::create_dir_all(dir.join("backups"))?;

    let postgres_password = util::secret(40);
    let env = format!(
        "# Projeto \"{name}\". Gerado por `nelcota init` em {now}.\n\
         # CONTÉM SEGREDOS: permissão 600, fora do git. Guarde uma cópia segura\n\
         # (sem ela, um restore em outra máquina invalida as sessões dos usuários).\n\n\
         NELCOTA_DOMAIN={domain}\n\
         NELCOTA_IMAGE={image}\n\
         NELCOTA_VERSION={version}\n\n\
         POSTGRES_PASSWORD={postgres_password}\n\
         NELCOTA_DATABASE_URL=postgres://postgres:{postgres_password}@postgres:5432/postgres\n\
         NELCOTA_AUTHENTICATOR_PASSWORD={authenticator}\n\
         NELCOTA_JWT_PRIVATE_KEY={jwt}\n\
         NELCOTA_JWT_ISSUER=https://{domain}\n\
         NELCOTA_DB_POOL_SIZE={pool}\n\
         NELCOTA_LOG_FORMAT=json\n\n\
         # Recuperação de senha por email (opcional): o SMTP do seu provedor e a\n\
         # página do app que recebe o link. Depois: nelcota -p {name} up\n\
         # NELCOTA_SMTP_URL=smtps://usuario:senha@smtp.exemplo.com:465\n\
         # NELCOTA_SMTP_FROM=Nome <nao-responda@{domain}>\n\
         # NELCOTA_PASSWORD_RECOVERY_URL=https://app.exemplo.com/nova-senha\n",
        name = new.entry.name,
        now = util::timestamp(),
        domain = new.entry.domain,
        image = new.image,
        version = new.version,
        authenticator = util::secret(40),
        jwt = nelcota_auth::generate_ed25519_private_key(),
        pool = pool_size(new.profile),
    );
    write_private(&dir.join(".env"), &env)?;
    fs::write(
        dir.join("docker-compose.yml"),
        render_compose(&new.entry.name, new.profile),
    )?;
    fs::write(dir.join(".gitignore"), ".env\nbackups/\n")?;
    fs::write(
        dir.join("migrations/README.md"),
        "Migrações SQL puras, aplicadas em ordem por `nelcota migrate`.\n\
         Nomes: V1__criar_tabelas.sql, V2__adicionar_indices.sql, ...\n\
         Controle em nelcota.user_migrations (versão, nome, checksum, data).\n",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfil_pela_ram_por_projeto() {
        assert_eq!(profile_for(960), "1gb");
        assert_eq!(profile_for(3900), "4gb");
        assert_eq!(choose_profile(None, 4096, 1).unwrap(), "4gb");
        assert_eq!(choose_profile(None, 4096, 2).unwrap(), "2gb");
        assert_eq!(choose_profile(None, 4096, 4).unwrap(), "1gb");
        assert_eq!(choose_profile(Some("2gb"), 512, 9).unwrap(), "2gb");
        assert!(choose_profile(Some("3gb"), 4096, 1).is_err());
    }

    #[test]
    fn compose_do_projeto() {
        let compose = render_compose("loja", "1gb");
        assert!(!compose.contains("{{"));
        assert!(compose.contains("name: nelcota-loja"));
        assert!(compose.contains("aliases: [app-loja]"));
        assert!(compose.contains("NELCOTA_PROJECT_NAME: loja"));
        assert!(compose.contains("      - -c\n      - shared_buffers=128MB"));
        assert!(compose.contains("internal: true"));
        assert!(!compose.contains("ports:"), "só o Caddy publica portas");
    }
}
