//! Nomes de projeto e domínios: validação e resolução de
//! `nelcota init [DOMÍNIO] [--project NOME]`.
//!
//! - Domínio próprio: `nelcota init api.loja.com` → projeto "loja".
//! - Subdomínio do domínio base do host: `nelcota init --project loja`
//!   com `--base-domain exemplo.com` → `loja.exemplo.com`.
//! - Local: `nelcota init --local --project loja` → `loja.localhost`.

use anyhow::bail;

/// Primeiros rótulos que não servem como nome de projeto (`api.loja.com` → "loja").
const GENERIC_LABELS: [&str; 6] = ["api", "app", "www", "admin", "painel", "db"];

pub fn validate_project_name(name: &str) -> anyhow::Result<()> {
    let ok = (1..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if !ok {
        bail!("nome de projeto inválido: '{name}' (use a-z, 0-9 e hífen, até 32 caracteres)");
    }
    Ok(())
}

pub fn is_valid_domain(domain: &str) -> bool {
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

/// Nome sugerido para o projeto a partir do domínio.
pub fn project_name_from_domain(domain: &str) -> String {
    let labels: Vec<&str> = domain.split('.').collect();
    let label = match labels.as_slice() {
        [first, second, _, ..] if GENERIC_LABELS.contains(first) => second,
        [first, ..] => first,
        [] => "projeto",
    };
    let name: String = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(32)
        .collect();
    name.trim_matches('-').to_owned()
}

/// Nome e domínio finais de um projeto novo.
pub fn resolve(
    domain: Option<&str>,
    project: Option<&str>,
    base_domain: Option<&str>,
    local: bool,
) -> anyhow::Result<(String, String)> {
    let domain = domain.map(|d| d.trim().trim_end_matches('.').to_lowercase());
    let (name, domain) = match (domain, project, base_domain, local) {
        (Some(domain), project, _, _) => {
            let name = project.map_or_else(|| project_name_from_domain(&domain), str::to_owned);
            (name, domain)
        }
        (None, Some(project), _, true) => (project.to_owned(), format!("{project}.localhost")),
        (None, Some(project), Some(base), false) => {
            (project.to_owned(), format!("{project}.{base}"))
        }
        (None, None, _, true) => ("local".to_owned(), "local.localhost".to_owned()),
        (None, Some(_), None, false) => bail!(
            "informe o domínio (`nelcota init api.loja.com`) ou defina um domínio base \
             (`nelcota init --project loja --base-domain exemplo.com`)"
        ),
        (None, None, _, false) => {
            bail!("informe o domínio (`nelcota init api.loja.com`) ou o projeto com --project")
        }
    };
    validate_project_name(&name)?;
    if !is_valid_domain(&domain) {
        bail!("domínio inválido: {domain}");
    }
    Ok((name, domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_vem_do_dominio() {
        assert_eq!(project_name_from_domain("api.loja.com"), "loja");
        assert_eq!(project_name_from_domain("loja.com.br"), "loja");
        assert_eq!(project_name_from_domain("meu_app.exemplo.com"), "meu-app");
        assert_eq!(project_name_from_domain("api.com"), "api");
    }

    #[test]
    fn dominio_proprio_subdominio_e_local() {
        assert_eq!(
            resolve(Some("API.Loja.com."), None, None, false).unwrap(),
            ("loja".into(), "api.loja.com".into())
        );
        assert_eq!(
            resolve(Some("api.loja.com"), Some("vendas"), None, false).unwrap(),
            ("vendas".into(), "api.loja.com".into())
        );
        assert_eq!(
            resolve(None, Some("blog"), Some("exemplo.com"), false).unwrap(),
            ("blog".into(), "blog.exemplo.com".into())
        );
        assert_eq!(
            resolve(None, Some("blog"), None, true).unwrap(),
            ("blog".into(), "blog.localhost".into())
        );
        assert!(resolve(None, Some("blog"), None, false).is_err());
        assert!(resolve(None, None, None, false).is_err());
        assert!(resolve(Some("x.com; rm -rf /"), None, None, false).is_err());
        assert!(resolve(None, Some("Blog!"), None, true).is_err());
    }

    #[test]
    fn valida_nome_de_projeto() {
        assert!(validate_project_name("loja-2").is_ok());
        for bad in ["", "-loja", "loja-", "Loja", "a.b", "../x", &"x".repeat(33)] {
            assert!(validate_project_name(bad).is_err(), "{bad}");
        }
    }
}
