//! Autenticação: verifica o JWT do request e entrega as [`Claims`].
//!
//! A verificação fica atrás da trait [`JwtVerifier`] para que a origem dos
//! tokens seja substituível (HS256 local hoje; EdDSA + JWKS no Marco 2; um
//! provedor OIDC externo depois). Este crate só AUTENTICA: autorização é
//! exclusivamente do RLS do Postgres.

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use nelcota_core::{ApiError, Claims};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("token inválido: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error(transparent)]
    Claims(#[from] nelcota_core::InvalidClaims),
}

/// Valida um JWT (assinatura, expiração, formato das claims).
pub trait JwtVerifier: Send + Sync + 'static {
    fn verify(&self, token: &str) -> Result<Claims, VerifyError>;
}

/// Verificador HS256 com segredo compartilhado.
pub struct Hs256Verifier {
    key: DecodingKey,
    validation: Validation,
}

impl Hs256Verifier {
    pub fn new(secret: &[u8]) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 30;
        // `aud` entra no contrato no Marco 2; por ora não é verificada.
        validation.validate_aud = false;
        Hs256Verifier {
            key: DecodingKey::from_secret(secret),
            validation,
        }
    }
}

impl JwtVerifier for Hs256Verifier {
    fn verify(&self, token: &str) -> Result<Claims, VerifyError> {
        let data = jsonwebtoken::decode::<Value>(token, &self.key, &self.validation)?;
        Ok(Claims::from_payload(data.claims)?)
    }
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
