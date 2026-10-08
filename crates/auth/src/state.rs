//! Authentication dependencies and configuration.
use crate::{JwtVerifier, Keys, Mailer, Passwords, RateLimiter, SharedVerifier, links::LinkKind};
use axum::extract::FromRef;
use deadpool_postgres::Pool;
use std::sync::Arc;
/// Settings of the auth endpoints.
#[derive(Clone, Debug)]
pub struct AuthSettings {
    pub issuer: String,
    pub access_ttl_secs: u64,
    pub refresh_ttl_days: u32,
    pub signup_enabled: bool,
    pub trust_proxy: bool,
    pub links: EmailLinks,
}

/// App pages that receive the links sent by email. A flow without its page
/// is off.
#[derive(Clone, Debug, Default)]
pub struct EmailLinks {
    pub recovery: Option<String>,
    /// Also makes signup wait for the email to be confirmed.
    pub signup_confirmation: Option<String>,
    pub magic_link: Option<String>,
}

impl EmailLinks {
    pub(crate) fn url(&self, kind: LinkKind) -> Option<&str> {
        match kind {
            LinkKind::Recovery => &self.recovery,
            LinkKind::Signup => &self.signup_confirmation,
            LinkKind::MagicLink => &self.magic_link,
        }
        .as_deref()
    }
}

#[derive(Clone)]
pub struct AuthState {
    pub pool: Pool,
    pub keys: Arc<Keys>,
    pub passwords: Arc<Passwords>,
    pub limiter: Arc<RateLimiter>,
    pub settings: Arc<AuthSettings>,
    /// Email sending; without it, every email link flow is off.
    pub mailer: Option<Arc<dyn Mailer>>,
}

impl FromRef<AuthState> for SharedVerifier {
    fn from_ref(state: &AuthState) -> Self {
        state.keys.clone() as Arc<dyn JwtVerifier>
    }
}
