//! `nelcota init <domínio>`: checa o ambiente, gera segredos e escreve
//! `docker-compose.yml`, `Caddyfile` e `.env` (0600, fora do git).

use std::{
    fs,
    net::{IpAddr, TcpListener, ToSocketAddrs},
    process::Command,
};

use anyhow::{Context, bail};

use crate::{
    InitArgs,
    project::{Project, write_private},
    util::{self, ask, interactive, ok, step, warn},
};

const COMPOSE_TEMPLATE: &str = include_str!("../../../deploy/docker-compose.tmpl.yml");
const CADDY_TEMPLATE: &str = include_str!("../../../deploy/Caddyfile.tmpl");
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

struct S3 {
    endpoint: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
}

pub fn run(project: &Project, args: InitArgs) -> anyhow::Result<()> {
    let domain = match (&args.domain, args.local) {
        (_, true) => "localhost".to_owned(),
        (Some(domain), false) => domain.trim().to_lowercase(),
        (None, false) => bail!("informe o domínio (`nelcota init api.meuapp.com`) ou use --local"),
    };
    if !args.local && !is_valid_domain(&domain) {
        bail!("domínio inválido: {domain}");
    }
    if project.exists() && !args.force {
        bail!(
            "já existe uma instalação em {} (use --force para sobrescrever; os segredos atuais serão perdidos)",
            project.dir.display()
        );
    }

    step("Checando o ambiente");
    let ram_mb = total_ram_mb();
    if args.skip_checks {
        warn("checagens puladas (--skip-checks)");
    } else {
        check_docker()?;
        check_ports();
        match ram_mb {
            Some(mb) => ok(&format!("RAM: {mb} MB")),
            None => warn("não foi possível medir a RAM; assumindo 2 GB"),
        }
        if !args.local {
            check_dns(&domain);
        }
    }

    let profile = match &args.profile {
        Some(p) if PROFILES.iter().any(|(name, _)| name == p) => p.clone(),
        Some(p) => bail!("perfil desconhecido: {p} (use 1gb, 2gb, 4gb ou 8gb)"),
        None => profile_for(ram_mb.unwrap_or(2048)).to_owned(),
    };
    ok(&format!("perfil do Postgres: {profile}"));

    let s3 = backup_destination(&args);

    step("Gerando segredos");
    let postgres_password = util::secret(40);
    let authenticator_password = util::secret(40);
    let jwt_private_key = nelcota_auth::generate_ed25519_private_key();
    let admin_email = args
        .email
        .clone()
        .unwrap_or_else(|| format!("admin@{domain}"));
    let admin_password = util::secret(20);
    let admin_hash = nelcota_auth::hash_password(&admin_password)
        .context("falha ao gerar o hash da senha do admin")?;
    ok("senhas do Postgres, chave Ed25519 dos JWTs e senha do admin");

    step(&format!("Escrevendo arquivos em {}", project.dir.display()));
    fs::create_dir_all(&project.dir)?;
    fs::create_dir_all(project.path("migrations"))?;
    fs::create_dir_all(project.path("backups"))?;

    let version = args
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    let pool_size = match profile.as_str() {
        "1gb" => 8,
        "2gb" => 15,
        _ => 25,
    };
    let issuer = format!("https://{domain}");
    let mut env = format!(
        "# Gerado por `nelcota init` em {now}.\n\
         # CONTÉM SEGREDOS: permissão 600, fora do git. Guarde uma cópia segura\n\
         # (sem ela, um restore em outra máquina invalida as sessões dos usuários).\n\n\
         NELCOTA_DOMAIN={domain}\n\
         NELCOTA_IMAGE={image}\n\
         NELCOTA_VERSION={version}\n\n\
         POSTGRES_PASSWORD={postgres_password}\n\
         NELCOTA_DATABASE_URL=postgres://postgres:{postgres_password}@postgres:5432/postgres\n\
         NELCOTA_AUTHENTICATOR_PASSWORD={authenticator_password}\n\
         NELCOTA_JWT_PRIVATE_KEY={jwt_private_key}\n\
         NELCOTA_JWT_ISSUER={issuer}\n\
         NELCOTA_DB_POOL_SIZE={pool_size}\n\
         NELCOTA_LOG_FORMAT=json\n\n\
         NELCOTA_ADMIN_EMAIL={admin_email}\n\
         NELCOTA_ADMIN_PASSWORD_HASH='{admin_hash}'\n",
        now = util::timestamp(),
        image = args.image,
    );
    if let Some(s3) = &s3 {
        env.push_str(&format!(
            "\nNELCOTA_BACKUP_S3_ENDPOINT={}\nNELCOTA_BACKUP_S3_BUCKET={}\nNELCOTA_BACKUP_S3_ACCESS_KEY={}\nNELCOTA_BACKUP_S3_SECRET_KEY={}\nNELCOTA_BACKUP_S3_REGION={}\n",
            s3.endpoint, s3.bucket, s3.access_key, s3.secret_key, s3.region
        ));
    }
    write_private(&project.path(".env"), &env)?;
    ok(".env (permissão 600)");

    fs::write(project.path("docker-compose.yml"), render_compose(&profile))?;
    ok("docker-compose.yml");
    fs::write(
        project.path("Caddyfile"),
        render_caddyfile(&domain, args.local),
    )?;
    ok("Caddyfile");
    fs::write(project.path(".gitignore"), ".env\nbackups/\n")?;
    fs::write(
        project.path("migrations/README.md"),
        "Migrações SQL puras, aplicadas em ordem por `nelcota migrate`.\n\
         Nomes: V1__criar_tabelas.sql, V2__adicionar_indices.sql, ...\n\
         Controle em nelcota.user_migrations (versão, nome, checksum, data).\n",
    )?;
    ok(".gitignore, migrations/, backups/");

    if !args.local {
        setup_host(project, &args, s3.is_some());
    }

    println!();
    println!("Pronto. Próximo passo:  nelcota up");
    println!();
    println!("  API:    https://{domain}/rest/v1/");
    println!("  Auth:   https://{domain}/auth/v1/");
    println!("  Painel: https://{domain}/admin/");
    println!();
    println!("  Admin do painel: {admin_email}");
    println!("  Senha:           {admin_password}");
    println!("  (mostrada só agora; o .env guarda apenas o hash argon2id)");
    println!();
    println!("  Token service_role (IGNORA o RLS; só no seu backend): nelcota token service-role");
    Ok(())
}

fn is_valid_domain(domain: &str) -> bool {
    domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

fn profile_for(ram_mb: u64) -> &'static str {
    match ram_mb {
        0..1536 => "1gb",
        1536..3072 => "2gb",
        3072..6144 => "4gb",
        _ => "8gb",
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

fn render_compose(profile: &str) -> String {
    COMPOSE_TEMPLATE
        .replace("{{GENERATED_AT}}", &util::timestamp())
        .replace("{{PROFILE}}", profile)
        .replace("{{POSTGRES_ARGS}}", &postgres_args(profile))
}

fn render_caddyfile(domain: &str, local: bool) -> String {
    CADDY_TEMPLATE
        .replace("{{SITE}}", domain)
        .replace("{{TLS}}", if local { "\ttls internal\n" } else { "" })
}

fn check_docker() -> anyhow::Result<()> {
    let docker = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output();
    match docker {
        Ok(out) if out.status.success() => ok(&format!(
            "Docker {}",
            String::from_utf8_lossy(&out.stdout).trim()
        )),
        Ok(_) => {
            bail!("o Docker está instalado mas o daemon não responde (systemctl start docker)")
        }
        Err(_) => {
            bail!("Docker não encontrado. Instale com: curl -fsSL https://get.docker.com | sh")
        }
    }
    let compose = Command::new("docker")
        .args(["compose", "version", "--short"])
        .output();
    match compose {
        Ok(out) if out.status.success() => ok(&format!(
            "Docker Compose {}",
            String::from_utf8_lossy(&out.stdout).trim()
        )),
        _ => bail!("plugin `docker compose` não encontrado (apt install docker-compose-plugin)"),
    }
    Ok(())
}

fn check_ports() {
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

fn total_ram_mb() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

fn check_dns(domain: &str) {
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
        warn("backup remoto não configurado (dumps ficam só em backups/; veja docs/backup.md)");
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

/// Cron de backup, firewall e atualizações automáticas (Linux, como root).
fn setup_host(project: &Project, args: &InitArgs, has_s3: bool) {
    let is_root = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0");
    if !cfg!(target_os = "linux") || !is_root {
        warn("rode como root numa VPS Linux para instalar o cron de backup e o firewall");
        return;
    }

    step("Configurando a máquina");
    let dir = fs::canonicalize(&project.dir).unwrap_or_else(|_| project.dir.clone());
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/usr/local/bin/nelcota".into());
    let upload = if has_s3 { " --upload" } else { "" };
    let cron = format!(
        "# Backup diário do nelcota (gerado por `nelcota init`)\n\
         0 3 * * * root {exe} -C {} backup{upload} --keep 7 >> /var/log/nelcota-backup.log 2>&1\n",
        dir.display()
    );
    match fs::write("/etc/cron.d/nelcota-backup", cron) {
        Ok(()) => ok("backup diário às 03:00 (/etc/cron.d/nelcota-backup)"),
        Err(e) => warn(&format!("não foi possível instalar o cron de backup: {e}")),
    }

    let has_ufw = Command::new("ufw").arg("--version").output().is_ok();
    if args.firewall && has_ufw {
        let rules: [&[&str]; 4] = [
            &["allow", "OpenSSH"],
            &["allow", "80/tcp"],
            &["allow", "443"],
            &["--force", "enable"],
        ];
        let all_ok = rules.iter().all(|rule| {
            Command::new("ufw")
                .args(*rule)
                .output()
                .is_ok_and(|o| o.status.success())
        });
        if all_ok {
            ok("ufw: SSH, 80 e 443 liberados; o resto bloqueado");
        } else {
            warn("falha ao configurar o ufw; confira com `ufw status`");
        }
    } else if has_ufw {
        warn(
            "recomendado: nelcota init --firewall (ou: ufw allow OpenSSH && ufw allow 80/tcp && ufw allow 443 && ufw enable)",
        );
    }

    let unattended = Command::new("dpkg")
        .args(["-s", "unattended-upgrades"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !unattended {
        warn(
            "recomendado: apt install unattended-upgrades (atualizações de segurança automáticas)",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfil_pela_ram() {
        assert_eq!(profile_for(960), "1gb");
        assert_eq!(profile_for(1990), "2gb");
        assert_eq!(profile_for(3900), "4gb");
        assert_eq!(profile_for(16000), "8gb");
    }

    #[test]
    fn perfil_vira_argumentos_do_compose() {
        let args = postgres_args("1gb");
        assert!(args.contains("      - -c\n      - shared_buffers=128MB"));
        assert!(args.contains("shared_preload_libraries=pg_stat_statements"));
        assert!(!args.contains('#'));
    }

    #[test]
    fn compose_e_caddyfile_renderizados() {
        let compose = render_compose("2gb");
        assert!(!compose.contains("{{"));
        assert!(compose.contains("shared_buffers=384MB"));
        assert!(compose.contains("internal: true"));
        let caddy = render_caddyfile("api.exemplo.com", false);
        assert!(caddy.starts_with("# Gerado") && caddy.contains("api.exemplo.com {"));
        assert!(!caddy.contains("tls internal"));
        assert!(render_caddyfile("localhost", true).contains("tls internal"));
    }

    #[test]
    fn valida_dominio() {
        assert!(is_valid_domain("api.meuapp.com"));
        assert!(!is_valid_domain("meuapp"));
        assert!(!is_valid_domain("-x.com"));
        assert!(!is_valid_domain("a b.com"));
        assert!(!is_valid_domain("x.com; rm -rf /"));
    }
}
