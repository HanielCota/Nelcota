//! Authentication dependencies and configuration.
use crate::{JwtVerifier, Keys, Mailer, Passwords, RateLimiter, SharedVerifier};
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
    /// App page that receives the password recovery link.
    pub recovery_url: Option<String>,
}

#[derive(Clone)]
pub struct AuthState {
    pub pool: Pool,
    pub keys: Arc<Keys>,
    pub passwords: Arc<Passwords>,
    pub limiter: Arc<RateLimiter>,
    pub settings: Arc<AuthSettings>,
    /// Email sending; without it (or without `recovery_url`), password
    /// recovery answers `recovery_disabled`.
    pub mailer: Option<Arc<dyn Mailer>>,
}

impl FromRef<AuthState> for SharedVerifier {
    fn from_ref(state: &AuthState) -> Self {
        state.keys.clone() as Arc<dyn JwtVerifier>
    }
}
