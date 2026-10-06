//! `nelcota init [DOMÍNIO] [--project NOME]`: cria o host (na primeira vez) e
//! acrescenta um projeto. Só orquestra; cada passo vive no seu módulo.

use anyhow::bail;

use crate::{
    InitArgs, caddy, checks,
    envfile::write_private,
    host::{Host, Manifest, ProjectEntry},
    machine, naming, panel_login, registry, scaffold,
    util::{self, ask, interactive, ok, step, warn},
};

struct S3 {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
}

pub fn run(host: &Host, args: InitArgs) -> anyhow::Result<()> {
    let creating_host = !host.exists();
    let mut host_password = None;
    let mut manifest = if creating_host {
        let (manifest, password) = create_host(host, &args)?;
        host_password = password;
        manifest
    } else {
        let manifest = host.manifest()?;
        if args.local && !manifest.local {
            bail!("este host não é local; --local só vale na criação do host");
        }
        manifest
    };

    if let Some(base) = &args.base_domain {
        let base = base.trim().trim_end_matches('.').to_lowercase();
        if !naming::is_valid_domain(&base) {
            bail!("domínio base inválido: {base}");
        }
        manifest.base_domain = Some(base);
    }

    let (name, domain) = naming::resolve(
        args.domain.as_deref(),
        args.project.as_deref(),
        manifest.base_domain.as_deref(),
        manifest.local,
    )?;
    if manifest.projects.iter().any(|p| p.name == name) {
        bail!("já existe um projeto chamado '{name}' (use --project para outro nome)");
    }
    if manifest.projects.iter().any(|p| p.domain == domain) {
        bail!("o domínio {domain} já é usado por outro projeto");
    }

    step(&format!("Projeto \"{name}\" em {domain}"));
    if !manifest.local && !args.skip_checks {
        checks::dns(&domain);
    }
    let ram = checks::total_ram_mb().unwrap_or(2048);
    let profile =
        scaffold::choose_profile(args.profile.as_deref(), ram, manifest.projects.len() + 1)?;
    ok(&format!("perfil do Postgres: {profile}"));

    let entry = ProjectEntry { name, domain };
    let version = args
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    scaffold::create(
        host,
        &scaffold::NewProject {
            entry: &entry,
            profile,
            image: &manifest.image,
            version: &version,
        },
    )?;
    manifest.projects.push(entry.clone());
    host.save(&manifest)?;
    let project = host.project(&entry);
    let project_password = panel_login::apply(host, &manifest, &project)?;
    registry::write(host, &manifest)?;
    caddy::write(host, &manifest)?;
    caddy::reload(host)?;
    ok(&format!("arquivos em {}", project.dir.display()));

    println!();
    println!("Pronto. Próximo passo:  nelcota up");
    println!();
    println!("  API:    {}/rest/v1/", entry.url());
    println!("  Auth:   {}/auth/v1/", entry.url());
    println!("  Painel: {}/admin/", entry.url());
    let passwords: Vec<_> = host_password.into_iter().chain(project_password).collect();
    panel_login::print(&passwords);
    println!();
    println!(
        "  Token service_role (IGNORA o RLS; só no seu backend): nelcota -p {} token service-role",
        entry.name
    );
    Ok(())
}

/// Primeira vez: checagens, segredos do host, Caddy e configuração da máquina.
fn create_host(
    host: &Host,
    args: &InitArgs,
) -> anyhow::Result<(Manifest, Option<panel_login::NewPassword>)> {
    step(&format!("Criando o host em {}", host.root().display()));
    if args.skip_checks {
        warn("checagens puladas (--skip-checks)");
    } else {
        checks::docker()?;
        checks::ports();
        match checks::total_ram_mb() {
            Some(mb) => ok(&format!("RAM: {mb} MB")),
            None => warn("não foi possível medir a RAM; assumindo 2 GB"),
        }
    }

    let s3 = backup_destination(args);
    std::fs::create_dir_all(host.root())?;
    let mut secrets = format!(
        "# Segredos do host nelcota (gerado em {}). Permissão 600, fora do git.\n",
        util::timestamp()
    );
    if let Some(s3) = &s3 {
        secrets.push_str(&format!(
            "NELCOTA_BACKUP_S3_ENDPOINT={}\nNELCOTA_BACKUP_S3_BUCKET={}\nNELCOTA_BACKUP_S3_ACCESS_KEY={}\n\
             NELCOTA_BACKUP_S3_SECRET_KEY={}\nNELCOTA_BACKUP_S3_REGION={}\n",
            s3.endpoint, s3.bucket, s3.access_key, s3.secret_key, s3.region
        ));
    }
    write_private(host.secrets().path(), &secrets)?;

    let manifest = Manifest {
        version: 1,
        panel_login: args.panel_login,
        base_domain: None,
        local: args.local,
        image: args.image.clone(),
        projects: Vec::new(),
    };
    host.save(&manifest)?;

    let email = args.email.clone().unwrap_or_else(|| {
        let domain = args
            .base_domain
            .clone()
            .or_else(|| args.domain.clone())
            .unwrap_or_else(|| "localhost".into());
        format!("admin@{domain}")
    });
    let shared = panel_login::init_host(host, &email)?;
    ok(&format!(
        "login dos painéis: {}",
        manifest.panel_login.as_str()
    ));

    registry::write(host, &manifest)?;
    caddy::write(host, &manifest)?;
    std::fs::write(
        host.root().join(".gitignore"),
        "host.env\nprojects/*/.env\nprojects/*/backups/\narchive/\n",
    )?;
    if !args.local {
        machine::setup(host, s3.is_some(), args.firewall);
    }
    let shown = (manifest.panel_login == crate::host::PanelLogin::Shared).then_some(shared);
    Ok((manifest, shown))
}

fn backup_destination(args: &InitArgs) -> Option<S3> {
    if let (Some(endpoint), Some(bucket), Some(access_key), Some(secret_key)) = (
        &args.s3_endpoint,
        &args.s3_bucket,
        &args.s3_access_key,
        &args.s3_secret_key,
    ) {
        return Some(S3 {
            endpoint: endpoint.clone(),
            bucket: bucket.clone(),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
            region: args.s3_region.clone(),
        });
    }
    if args.yes || !interactive() {
        warn(
            "backup remoto não configurado (dumps ficam em projects/<nome>/backups/; veja docs/backup.md)",
        );
        return None;
    }
    println!();
    println!("Backup remoto em S3-compatible (AWS, Backblaze B2, Cloudflare R2, MinIO...).");
    let endpoint = ask("Endpoint S3 (Enter para pular)", "");
    if endpoint.is_empty() {
        warn("backup remoto não configurado");
        return None;
    }
    Some(S3 {
        endpoint,
        bucket: ask("Bucket", "nelcota-backups"),
        access_key: ask("Access key", ""),
        secret_key: ask("Secret key", ""),
        region: ask("Região", &args.s3_region),
    })
}
