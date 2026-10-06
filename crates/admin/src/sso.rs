//! Login único entre os painéis dos projetos de um mesmo host.
//!
//! Cookies não atravessam domínios diferentes (`api.loja.com` → `api.blog.com`),
//! então a sessão é passada por um *handoff*: o painel de origem, com o admin
//! já logado, emite um token curto para o projeto de destino; o destino o
//! valida e cria a própria sessão.
//!
//! - Token: JWT HS256 com o segredo compartilhado do host (`NELCOTA_ADMIN_SSO_SECRET`),
//!   `aud` = projeto de destino, validade de 60 s e `jti` de uso único.
//! - Ele viaja no fragmento da URL (`#sso=`), que não vai para logs de servidor
//!   nem para o `Referer`; a SPA o lê e o envia por POST.
//! - Sem segredo configurado (modo "login por projeto"), o handoff não existe.

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

/// Chaves do handoff e o registro dos tokens já usados (anti-replay).
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

    /// Token para o admin `email` entrar no projeto `audience`.
    pub fn issue(
        &self,
        email: &str,
        audience: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let mut jti = [0u8; 16];
        getrandom::fill(&mut jti).expect("fonte de aleatoriedade do sistema indisponível");
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

    /// Valida um token emitido para este projeto e para o admin configurado.
    /// Cada token só pode ser usado uma vez.
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

/// `POST /admin/api/sso/handoff {project}` (exige sessão): URL do painel do
/// projeto de destino já com o token no fragmento.
pub async fn handoff(
    State(state): State<AdminState>,
    Json(body): Json<HandoffRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let sso = state.host.sso.as_ref().ok_or_else(|| {
        ApiError::bad_request("login único desligado: entre no painel de cada projeto")
    })?;
    let target = projects::find(&state.host, &body.project).ok_or_else(|| {
        ApiError::not_found(format!("projeto '{}' não existe neste host", body.project))
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

/// `POST /admin/api/sso {token}` (público): troca o token por uma sessão.
pub async fn redeem(State(state): State<AdminState>, Json(body): Json<RedeemRequest>) -> Response {
    let Some(sso) = state.host.sso.as_ref() else {
        return ApiError(StatusCode::NOT_FOUND, "login único desligado".into()).into_response();
    };
    match sso.redeem(&body.token, &state.host.project, &state.credentials.email) {
        Ok(()) => {
            tracing::info!("login no painel por handoff de SSO");
            (
                [(header::SET_COOKIE, crate::new_session_cookie(&state))],
                Json(json!({ "email": state.credentials.email })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::warn!(?err, "handoff de SSO recusado");
            ApiError(
                StatusCode::UNAUTHORIZED,
                "link de acesso inválido ou expirado: entre de novo".into(),
            )
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"segredo-compartilhado-de-teste-com-32+";

    #[test]
    fn token_vale_uma_vez_e_so_para_o_destino() {
        let sso = Sso::new(SECRET);
        let token = sso.issue("admin@x.com", "blog").unwrap();
        assert_eq!(
            sso.redeem(&token, "loja", "admin@x.com"),
            Err(RedeemError::Invalid)
        );
        assert_eq!(
            sso.redeem(&token, "blog", "outro@x.com"),
            Err(RedeemError::Invalid)
        );
        assert_eq!(sso.redeem(&token, "blog", "ADMIN@x.com"), Ok(()));
        assert_eq!(
            sso.redeem(&token, "blog", "admin@x.com"),
            Err(RedeemError::Replayed)
        );
    }

    #[test]
    fn segredo_diferente_e_token_vencido_sao_recusados() {
        let token = Sso::new(SECRET).issue("admin@x.com", "blog").unwrap();
        let other = Sso::new(b"outro-segredo-qualquer-com-32-bytes!!");
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
