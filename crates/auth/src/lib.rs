//! Authentication: JWT issuing and verification, passwords, sessions and
//! refresh tokens, and the claims extractor for axum.
//!
//! Verification sits behind the [`JwtVerifier`] trait so the token source can
//! be swapped (local keys today; an external OIDC provider later). This crate
//! only AUTHENTICATES: authorization belongs exclusively to RLS.

mod accounts;
mod confirmation;
mod credentials;
mod db;
mod error;
mod handlers;
mod keys;
mod links;
mod mail;
mod password;
mod rate_limit;
mod recovery;
mod request;
mod sessions;
mod state;
mod verify;

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use nelcota_core::{ApiError, Claims};

pub use credentials::{InvalidCredential, normalize_email, validate_password};
pub use handlers::router;
pub use keys::{KeyError, Keys, generate_ed25519_private_key};
pub use mail::{Email, MailError, Mailer, SmtpMailer};
pub use password::{Passwords, hash_password, verify_password};
pub use rate_limit::RateLimiter;
pub use state::{AuthSettings, AuthState, EmailLinks};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("invalid token: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("unknown algorithm or key (kid)")]
    UnknownKey,
    #[error(transparent)]
    Claims(#[from] nelcota_core::InvalidClaims),
}

/// Validates a JWT (signature, expiry, claims format).
pub trait JwtVerifier: Send + Sync + 'static {
    fn verify(&self, token: &str) -> Result<Claims, VerifyError>;
}

/// Verifier shared in the axum state.
pub type SharedVerifier = Arc<dyn JwtVerifier>;

/// Extractor of the request claims.
///
/// - no `Authorization`: an `anon` request;
/// - a valid `Authorization: Bearer <jwt>`: the token's claims;
/// - anything else (invalid or expired token, unknown role, a scheme other
///   than Bearer): 401. Never silently falls back to `anon`.
pub struct Auth(pub Claims);

impl<S> FromRequestParts<S> for Auth
where
    SharedVerifier: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Ok(Auth(Claims::anon()));
        };
        let token = value
            .to_str()
            .ok()
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(ApiError::invalid_token)?;

        let verifier = SharedVerifier::from_ref(state);
        match verifier.verify(token) {
            Ok(claims) => Ok(Auth(claims)),
            Err(err) => {
                // The token itself is never logged.
                tracing::debug!(error = %err, "JWT rejected");
                Err(ApiError::invalid_token())
            }
        }
    }
}
