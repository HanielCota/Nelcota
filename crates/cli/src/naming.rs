//! Project names and domains: validation and resolution of
//! `nelcota init [DOMAIN] [--project NAME]`.
//!
//! - Own domain: `nelcota init api.shop.com` → project "shop".
//! - Subdomain of the host's base domain: `nelcota init --project shop`
//!   with `--base-domain example.com` → `shop.example.com`.
//! - Local: `nelcota init --local --project shop` → `shop.localhost`.

use anyhow::bail;

/// Leading labels that do not make a project name (`api.shop.com` → "shop").
/// "painel" is Portuguese for "panel", common in Brazilian domains.
const GENERIC_LABELS: [&str; 7] = ["api", "app", "www", "admin", "panel", "painel", "db"];

pub fn validate_project_name(name: &str) -> anyhow::Result<()> {
    let ok = (1..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if !ok {
        bail!("invalid project name: '{name}' (use a-z, 0-9 and hyphens, up to 32 characters)");
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

/// Project name suggested from the domain.
pub fn project_name_from_domain(domain: &str) -> String {
    let labels: Vec<&str> = domain.split('.').collect();
    let label = match labels.as_slice() {
        [first, second, _, ..] if GENERIC_LABELS.contains(first) => second,
        [first, ..] => first,
        [] => "project",
    };
    let name: String = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(32)
        .collect();
    name.trim_matches('-').to_owned()
}

/// Final name and domain of a new project.
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
            "give the domain (`nelcota init api.shop.com`) or set a base domain \
             (`nelcota init --project shop --base-domain example.com`)"
        ),
        (None, None, _, false) => {
            bail!("give the domain (`nelcota init api.shop.com`) or the project with --project")
        }
    };
    validate_project_name(&name)?;
    if !is_valid_domain(&domain) {
        bail!("invalid domain: {domain}");
    }
    Ok((name, domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_comes_from_the_domain() {
        assert_eq!(project_name_from_domain("api.shop.com"), "shop");
        assert_eq!(project_name_from_domain("shop.com.br"), "shop");
        assert_eq!(project_name_from_domain("panel.shop.com"), "shop");
        assert_eq!(project_name_from_domain("my_app.example.com"), "my-app");
        assert_eq!(project_name_from_domain("api.com"), "api");
    }

    #[test]
    fn own_domain_subdomain_and_local() {
        assert_eq!(
            resolve(Some("API.Shop.com."), None, None, false).unwrap(),
            ("shop".into(), "api.shop.com".into())
        );
        assert_eq!(
            resolve(Some("api.shop.com"), Some("sales"), None, false).unwrap(),
            ("sales".into(), "api.shop.com".into())
        );
        assert_eq!(
            resolve(None, Some("blog"), Some("example.com"), false).unwrap(),
            ("blog".into(), "blog.example.com".into())
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
    fn validates_project_name() {
        assert!(validate_project_name("shop-2").is_ok());
        for bad in ["", "-shop", "shop-", "Shop", "a.b", "../x", &"x".repeat(33)] {
            assert!(validate_project_name(bad).is_err(), "{bad}");
        }
    }
}
