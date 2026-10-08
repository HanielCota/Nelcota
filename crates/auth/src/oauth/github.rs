//! GitHub, through its REST API: the profile and the email list are separate,
//! and only the email list says which address is verified.
use serde::Deserialize;
use serde_json::Value;

use super::provider::{ProviderEndpoints, ProviderError, ProviderIdentity, get_json};

pub(super) const SCOPES: &str = "read:user user:email";

pub(super) fn endpoints() -> ProviderEndpoints {
    ProviderEndpoints {
        authorize: "https://github.com/login/oauth/authorize".into(),
        token: "https://github.com/login/oauth/access_token".into(),
        profile: "https://api.github.com/user".into(),
        emails: Some("https://api.github.com/user/emails".into()),
    }
}

#[derive(Deserialize)]
struct Email {
    email: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    verified: bool,
}

pub(super) async fn profile(
    http: &reqwest::Client,
    endpoints: &ProviderEndpoints,
    token: &str,
) -> Result<ProviderIdentity, ProviderError> {
    let data: Value = get_json(http, &endpoints.profile, token).await?;
    let emails: Vec<Email> = match &endpoints.emails {
        Some(url) => get_json(http, url, token).await?,
        None => Vec::new(),
    };
    identity(data, emails)
}

fn identity(data: Value, emails: Vec<Email>) -> Result<ProviderIdentity, ProviderError> {
    #[derive(Deserialize)]
    struct User {
        id: u64,
        login: String,
        name: Option<String>,
        avatar_url: Option<String>,
    }
    let user: User = serde_json::from_value(data.clone())
        .map_err(|error| ProviderError(format!("unexpected GitHub profile: {error}")))?;
    // The primary address, and whether GitHub verified it.
    let primary = emails.into_iter().find(|email| email.primary);
    Ok(ProviderIdentity {
        subject: user.id.to_string(),
        email_verified: primary.as_ref().is_some_and(|email| email.verified),
        email: primary.map(|email| email.email),
        name: user.name.or(Some(user.login)),
        avatar_url: user.avatar_url,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::Email;
    use serde_json::json;

    fn email(address: &str, primary: bool, verified: bool) -> Email {
        Email {
            email: address.into(),
            primary,
            verified,
        }
    }

    #[test]
    fn uses_the_primary_address_and_its_verification() {
        let profile =
            json!({ "id": 583231, "login": "octocat", "name": null, "avatar_url": "https://a/1" });
        let identity = super::identity(
            profile.clone(),
            vec![
                email("old@x.com", false, true),
                email("cat@x.com", true, true),
            ],
        )
        .unwrap();
        assert_eq!(identity.subject, "583231");
        assert_eq!(identity.email.as_deref(), Some("cat@x.com"));
        assert!(identity.email_verified);
        assert_eq!(identity.name.as_deref(), Some("octocat"));

        let unverified =
            super::identity(profile.clone(), vec![email("cat@x.com", true, false)]).unwrap();
        assert!(!unverified.email_verified);
        let none = super::identity(profile, vec![]).unwrap();
        assert!(none.email.is_none());
    }
}
