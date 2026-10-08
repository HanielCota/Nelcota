//! Google, through OpenID Connect's userinfo endpoint.
use serde::Deserialize;
use serde_json::Value;

use super::provider::{ProviderEndpoints, ProviderError, ProviderIdentity, get_json};

pub(super) const SCOPES: &str = "openid email profile";

pub(super) fn endpoints() -> ProviderEndpoints {
    ProviderEndpoints {
        authorize: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token: "https://oauth2.googleapis.com/token".into(),
        profile: "https://openidconnect.googleapis.com/v1/userinfo".into(),
        emails: None,
    }
}

pub(super) async fn profile(
    http: &reqwest::Client,
    endpoints: &ProviderEndpoints,
    token: &str,
) -> Result<ProviderIdentity, ProviderError> {
    let data: Value = get_json(http, &endpoints.profile, token).await?;
    identity(data)
}

fn identity(data: Value) -> Result<ProviderIdentity, ProviderError> {
    #[derive(Deserialize)]
    struct UserInfo {
        sub: String,
        email: Option<String>,
        #[serde(default)]
        email_verified: bool,
        name: Option<String>,
        picture: Option<String>,
    }
    let info: UserInfo = serde_json::from_value(data.clone())
        .map_err(|error| ProviderError(format!("unexpected Google profile: {error}")))?;
    Ok(ProviderIdentity {
        subject: info.sub,
        email: info.email,
        email_verified: info.email_verified,
        name: info.name,
        avatar_url: info.picture,
        data,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn reads_the_oidc_claims() {
        let identity = super::identity(json!({
            "sub": "1078", "email": "ana@gmail.com", "email_verified": true,
            "name": "Ana", "picture": "https://lh3.googleusercontent.com/a",
        }))
        .unwrap();
        assert_eq!(identity.subject, "1078");
        assert_eq!(identity.email.as_deref(), Some("ana@gmail.com"));
        assert!(identity.email_verified);
        assert_eq!(identity.name.as_deref(), Some("Ana"));

        let unverified = super::identity(json!({ "sub": "9", "email": "x@y.com" })).unwrap();
        assert!(!unverified.email_verified);
        assert!(super::identity(json!({ "email": "no-sub@y.com" })).is_err());
    }
}
