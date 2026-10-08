//! A configured provider: where to send the person, how to redeem the code it
//! returns and how to read the account behind it.
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::{github, google};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Google,
    GitHub,
}

impl ProviderKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "google" => Some(ProviderKind::Google),
            "github" => Some(ProviderKind::GitHub),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Google => "google",
            ProviderKind::GitHub => "github",
        }
    }
}

/// The provider's URLs. The defaults are the real ones; tests point them at a
/// local fake.
#[derive(Clone, Debug)]
pub struct ProviderEndpoints {
    pub authorize: String,
    pub token: String,
    /// The account's profile, read with the access token.
    pub profile: String,
    /// The account's email list, for providers that keep it apart (GitHub).
    pub emails: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Provider {
    pub(crate) kind: ProviderKind,
    client_id: String,
    client_secret: String,
    endpoints: ProviderEndpoints,
}

/// The account at the provider, as far as sign-in needs it.
#[derive(Debug)]
pub(crate) struct ProviderIdentity {
    /// The provider's stable account id (`sub` in OIDC).
    pub subject: String,
    pub email: Option<String>,
    /// Whether the provider vouches that the person owns `email`.
    pub email_verified: bool,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// The profile as received.
    pub data: Value,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct ProviderError(pub String);

impl From<reqwest::Error> for ProviderError {
    fn from(error: reqwest::Error) -> Self {
        // reqwest's message carries the URL, never the request body (secret, code).
        ProviderError(error.to_string())
    }
}

impl Provider {
    pub fn google(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new(
            ProviderKind::Google,
            client_id,
            client_secret,
            google::endpoints(),
        )
    }

    pub fn github(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new(
            ProviderKind::GitHub,
            client_id,
            client_secret,
            github::endpoints(),
        )
    }

    fn new(
        kind: ProviderKind,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        endpoints: ProviderEndpoints,
    ) -> Self {
        Provider {
            kind,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            endpoints,
        }
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    /// Same provider, other URLs (tests).
    pub fn with_endpoints(self, endpoints: ProviderEndpoints) -> Self {
        Provider { endpoints, ..self }
    }

    fn scopes(&self) -> &'static str {
        match self.kind {
            ProviderKind::Google => google::SCOPES,
            ProviderKind::GitHub => github::SCOPES,
        }
    }

    /// The provider's consent page for this sign-in.
    pub(crate) fn authorize_url(&self, callback: &str, state: &str, challenge: &str) -> String {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", callback)
            .append_pair("scope", self.scopes())
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .finish();
        format!("{}?{query}", self.endpoints.authorize)
    }

    /// Redeems the provider's code and reads the account behind it. The calls
    /// go server to server over TLS, so the answers come from the provider.
    pub(crate) async fn identity(
        &self,
        http: &reqwest::Client,
        callback: &str,
        code: &str,
        verifier: &str,
    ) -> Result<ProviderIdentity, ProviderError> {
        let token = self.exchange(http, callback, code, verifier).await?;
        match self.kind {
            ProviderKind::Google => google::profile(http, &self.endpoints, &token).await,
            ProviderKind::GitHub => github::profile(http, &self.endpoints, &token).await,
        }
    }

    async fn exchange(
        &self,
        http: &reqwest::Client,
        callback: &str,
        code: &str,
        verifier: &str,
    ) -> Result<String, ProviderError> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: Option<String>,
            error: Option<String>,
        }
        let form = form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("redirect_uri", callback)
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", &self.client_secret)
            .append_pair("code_verifier", verifier)
            .finish();
        let response = http
            .post(&self.endpoints.token)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(form)
            .send()
            .await?;
        let status = response.status();
        let body: TokenResponse = read_json(response).await?;
        match body {
            TokenResponse {
                access_token: Some(token),
                ..
            } if status.is_success() => Ok(token),
            TokenResponse { error, .. } => Err(ProviderError(format!(
                "code exchange refused ({status}): {}",
                error.as_deref().unwrap_or("no access_token")
            ))),
        }
    }
}

/// GET with the access token, as JSON.
pub(super) async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<T, ProviderError> {
    let response = http
        .get(url)
        .bearer_auth(token)
        .header("accept", "application/json")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(ProviderError(format!(
            "{url} answered {}",
            response.status()
        )));
    }
    read_json(response).await
}

/// Reads a JSON body of bounded size: a provider is trusted to answer, not to
/// answer small.
async fn read_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ProviderError> {
    const LIMIT: usize = 256 * 1024;
    if response
        .content_length()
        .is_some_and(|len| len > LIMIT as u64)
    {
        return Err(ProviderError("provider response too large".into()));
    }
    let body = response.bytes().await?;
    if body.len() > LIMIT {
        return Err(ProviderError("provider response too large".into()));
    }
    serde_json::from_slice(&body)
        .map_err(|error| ProviderError(format!("unexpected provider response: {error}")))
}

/// HTTP client for the providers: short timeout, no redirects.
pub(super) fn client() -> Result<reqwest::Client, String> {
    // reqwest's rustls runs on `ring` (D16); installing twice is harmless.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("nelcota/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("could not build the HTTP client: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_carries_pkce_and_state() {
        let provider = Provider::github("client-1", "secret");
        let url = provider.authorize_url("https://api.x.com/auth/v1/callback", "st", "ch");
        assert!(url.starts_with("https://github.com/login/oauth/authorize?"));
        assert!(url.contains("client_id=client-1"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fapi.x.com%2Fauth%2Fv1%2Fcallback"));
        assert!(url.contains("state=st"));
        assert!(url.contains("code_challenge=ch&code_challenge_method=S256"));
        assert!(!url.contains("secret"));
    }

    #[test]
    fn kinds_round_trip() {
        for kind in [ProviderKind::Google, ProviderKind::GitHub] {
            assert_eq!(ProviderKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ProviderKind::parse("facebook"), None);
    }
}
