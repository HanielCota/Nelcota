//! Autenticação: emissão e verificação de JWT, senhas, sessões e refresh
//! tokens, e o extrator de claims para o axum.
//!
//! A verificação fica atrás da trait [`JwtVerifier`] para que a origem dos
//! tokens seja substituível (chaves locais hoje; um provedor OIDC externo
//! depois). Este crate só AUTENTICA: autorização é exclusivamente do RLS.

mod handlers;
mod keys;
mod password;
mod rate_limit;

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use nelcota_core::{ApiError, Claims};

pub use handlers::{AuthSettings, AuthState, router};
pub use keys::{KeyError, Keys, generate_ed25519_private_key};
pub use password::Passwords;
pub use rate_limit::RateLimiter;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("token inválido: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("algoritmo ou chave (kid) desconhecidos")]
    UnknownKey,
    #[error(transparent)]
    Claims(#[from] nelcota_core::InvalidClaims),
}

/// Valida um JWT (assinatura, expiração, formato das claims).
pub trait JwtVerifier: Send + Sync + 'static {
    fn verify(&self, token: &str) -> Result<Claims, VerifyError>;
}

/// Verificador compartilhado no estado do axum.
pub type SharedVerifier = Arc<dyn JwtVerifier>;

/// Extrator das claims do request.
///
/// - sem `Authorization`: request `anon`;
/// - `Authorization: Bearer <jwt>` válido: as claims do token;
/// - qualquer outra coisa (token inválido, expirado, role desconhecida,
///   esquema diferente de Bearer): 401. Nunca cai silenciosamente para `anon`.
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
                // O token em si nunca é logado.
                tracing::debug!(error = %err, "JWT rejeitado");
                Err(ApiError::invalid_token())
            }
        }
    }
}
