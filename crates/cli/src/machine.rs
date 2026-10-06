//! Configuração da máquina na criação do host (Linux, como root): cron de
//! backup de todos os projetos, firewall e atualizações automáticas.

use std::{fs, process::Command};

use crate::{
    host::Host,
    util::{ok, step, warn},
};

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
}

pub fn setup(host: &Host, has_s3: bool, firewall: bool) {
    if !cfg!(target_os = "linux") || !is_root() {
        warn("rode como root numa VPS Linux para instalar o cron de backup e o firewall");
        return;
    }
    step("Configurando a máquina");
    install_backup_cron(host, has_s3);
    configure_firewall(firewall);
    if !Command::new("dpkg")
        .args(["-s", "unattended-upgrades"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        warn(
            "recomendado: apt install unattended-upgrades (atualizações de segurança automáticas)",
        );
    }
}

fn install_backup_cron(host: &Host, has_s3: bool) {
    let root = fs::canonicalize(host.root()).unwrap_or_else(|_| host.root().to_path_buf());
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/usr/local/bin/nelcota".into());
    let upload = if has_s3 { " --upload" } else { "" };
    let cron = format!(
        "# Backup diário de todos os projetos do nelcota (gerado por `nelcota init`)\n\
         0 3 * * * root {exe} -C {} backup --all{upload} --keep 7 >> /var/log/nelcota-backup.log 2>&1\n",
        root.display()
    );
    match fs::write("/etc/cron.d/nelcota-backup", cron) {
        Ok(()) => ok("backup diário às 03:00 (/etc/cron.d/nelcota-backup)"),
        Err(e) => warn(&format!("não foi possível instalar o cron de backup: {e}")),
    }
}

fn configure_firewall(enable: bool) {
    let has_ufw = Command::new("ufw").arg("--version").output().is_ok();
    if !has_ufw {
        return;
    }
    if !enable {
        warn(
            "recomendado: nelcota init --firewall (ou: ufw allow OpenSSH && ufw allow 80/tcp && ufw allow 443 && ufw enable)",
        );
        return;
    }
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
}
