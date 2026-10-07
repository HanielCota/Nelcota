//! Single sign-on between the panels of projects on the same host.
//!
//! Cookies do not cross domains (`api.shop.com` → `api.blog.com`), so the
//! session is passed through a *handoff*: the source panel, with the admin
//! already signed in, issues a short-lived token for the target project; the
//! target validates it and creates its own session.
//!
//! - Token: HS256 JWT with the host's shared secret (`NELCOTA_ADMIN_SSO_SECRET`),
//!   `aud` = target project, 60 s validity and a single-use `jti`.
//! - It travels in the URL fragment (`#sso=`), which reaches neither server
//!   logs nor the `Referer`; the SPA reads it and sends it by POST.
//! - Without a configured secret ("per-project login" mode), there is no handoff.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, get_current_timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{AdminState, ApiError, projects};

const TOKEN_TTL_SECS: u64 = 60;
const ISSUER: &str = "nelcota-admin";

#[derive(Serialize, Deserialize)]
struct HandoffClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: u64,
    exp: u64,
    jti: String,
}

/// Handoff keys and the record of tokens already used (anti-replay).
pub struct Sso {
    encoding: EncodingKey,
    decoding: DecodingKey,
    used: Mutex<HashMap<String, Instant>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RedeemError {
    Invalid,
    Replayed,
}

impl Sso {
    pub fn new(secret: &[u8]) -> Self {
        Sso {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            used: Mutex::new(HashMap::new()),
        }
    }

    /// Token for the admin `email` to enter the project `audience`.
    pub fn issue(
        &self,
        email: &str,
        audience: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let mut jti = [0u8; 16];
        getrandom::fill(&mut jti).expect("system randomness source unavailable");
        let now = get_current_timestamp();
        let claims = HandoffClaims {
            iss: ISSUER.into(),
            sub: email.into(),
            aud: audience.into(),
            iat: now,
            exp: now + TOKEN_TTL_SECS,
            jti: URL_SAFE_NO_PAD.encode(jti),
        };
        jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
    }

    /// Validates a token issued for this project and the configured admin.
    /// Each token can be used only once.
    pub fn redeem(&self, token: &str, project: &str, email: &str) -> Result<(), RedeemError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[project]);
        validation.set_issuer(&[ISSUER]);
        validation.leeway = 5;
        let data = jsonwebtoken::decode::<HandoffClaims>(token, &self.decoding, &validation)
            .map_err(|_| RedeemError::Invalid)?;
        if !data.claims.sub.eq_ignore_ascii_case(email) {
            return Err(RedeemError::Invalid);
        }
        let mut used = self.used.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        used.retain(|_, expires| *expires > now);
        if used.contains_key(&data.claims.jti) {
            return Err(RedeemError::Replayed);
        }
        used.insert(
            data.claims.jti,
            now + Duration::from_secs(TOKEN_TTL_SECS + 10),
        );
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct HandoffRequest {
    project: String,
}

/// `POST /admin/api/sso/handoff {project}` (needs a session): URL of the
/// target project's panel with the token already in the fragment.
pub async fn handoff(
    State(state): State<AdminState>,
    Json(body): Json<HandoffRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let sso = state.host.sso.as_ref().ok_or_else(|| {
        ApiError::bad_request(
            "sso_disabled",
            "single sign-on is off: sign in to each project's panel",
        )
    })?;
    let target = projects::find(&state.host, &body.project).ok_or_else(|| {
        ApiError::not_found(
            "project_not_found",
            format!("project '{}' does not exist on this host", body.project),
        )
        .params(json!({ "project": body.project }))
    })?;
    let token = sso.issue(&state.credentials.email, &target.name)?;
    Ok(Json(json!({
        "url": format!("{}/admin/#sso={token}", target.url.trim_end_matches('/')),
    })))
}

#[derive(Deserialize)]
pub struct RedeemRequest {
    token: String,
}

/// `POST /admin/api/sso {token}` (public): trades the token for a session.
pub async fn redeem(State(state): State<AdminState>, Json(body): Json<RedeemRequest>) -> Response {
    let Some(sso) = state.host.sso.as_ref() else {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "sso_disabled",
            "single sign-on is off",
        )
        .into_response();
    };
    match sso.redeem(&body.token, &state.host.project, &state.credentials.email) {
        Ok(()) => {
            tracing::info!("panel login through an SSO handoff");
            (
                [(header::SET_COOKIE, crate::new_session_cookie(&state))],
                Json(json!({ "email": state.credentials.email })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::warn!(?err, "SSO handoff refused");
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "sso_link_invalid",
                "invalid or expired access link: sign in again",
            )
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"shared-test-secret-with-32-bytes-or-more";

    #[test]
    fn token_works_once_and_only_for_the_target() {
        let sso = Sso::new(SECRET);
        let token = sso.issue("admin@x.com", "blog").unwrap();
        assert_eq!(
            sso.redeem(&token, "shop", "admin@x.com"),
            Err(RedeemError::Invalid)
        );
        assert_eq!(
            sso.redeem(&token, "blog", "other@x.com"),
            Err(RedeemError::Invalid)
        );
        assert_eq!(sso.redeem(&token, "blog", "ADMIN@x.com"), Ok(()));
        assert_eq!(
            sso.redeem(&token, "blog", "admin@x.com"),
            Err(RedeemError::Replayed)
        );
    }

    #[test]
    fn other_secret_and_expired_token_are_refused() {
        let token = Sso::new(SECRET).issue("admin@x.com", "blog").unwrap();
        let other = Sso::new(b"some-other-secret-with-32-bytes-too!!");
        assert_eq!(
            other.redeem(&token, "blog", "admin@x.com"),
            Err(RedeemError::Invalid)
        );

        let now = get_current_timestamp();
        let expired = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &HandoffClaims {
                iss: ISSUER.into(),
                sub: "admin@x.com".into(),
                aud: "blog".into(),
                iat: now - 600,
                exp: now - 300,
                jti: "x".into(),
            },
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        assert_eq!(
            Sso::new(SECRET).redeem(&expired, "blog", "admin@x.com"),
            Err(RedeemError::Invalid)
        );
    }
}
