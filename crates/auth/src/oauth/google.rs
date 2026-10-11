//! Google, through OpenID Connect's userinfo endpoint.
use openidconnect::{EmptyAdditionalClaims, UserInfoClaims, core::CoreGenderClaim};
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
    let info: UserInfoClaims<EmptyAdditionalClaims, CoreGenderClaim> =
        UserInfoClaims::from_json::<ProviderError>(
            &serde_json::to_vec(&data)
                .map_err(|_| ProviderError("unexpected Google profile".into()))?,
            None,
        )
        .map_err(|_| ProviderError("unexpected Google profile".into()))?;
    Ok(ProviderIdentity {
        subject: info.subject().as_str().into(),
        email: info.email().map(|email| email.as_str().into()),
        email_verified: info.email_verified().unwrap_or(false),
        name: info
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.as_str().into()),
        avatar_url: info
            .picture()
            .and_then(|picture| picture.get(None))
            .map(|url| url.as_str().into()),
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
