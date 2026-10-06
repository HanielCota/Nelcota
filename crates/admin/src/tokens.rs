//! Emissão de tokens `service_role` pelo painel. O token não é guardado:
//! aparece uma vez para o admin copiar. Quem tem acesso ao painel já é o
//! dono do banco, então emitir não amplia o que ele pode fazer.

use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use nelcota_auth::Keys;
use serde::Deserialize;
use serde_json::json;

use crate::{AdminState, ApiError};

/// Chaves de assinatura e emissor (`iss`) dos tokens do projeto.
pub struct TokenIssuer {
    pub keys: Arc<Keys>,
    pub issuer: String,
}

const MAX_DAYS: u64 = 3650;

#[derive(Deserialize)]
pub struct ServiceRoleRequest {
    days: u64,
}

/// `POST /admin/api/tokens/service-role`
pub async fn service_role(
    State(state): State<AdminState>,
    Json(body): Json<ServiceRoleRequest>,
) -> Result<Response, ApiError> {
    if !(1..=MAX_DAYS).contains(&body.days) {
        return Err(ApiError::bad_request(format!(
            "validade entre 1 e {MAX_DAYS} dias"
        )));
    }
    let token = state
        .tokens
        .keys
        .service_role_token(&state.tokens.issuer, body.days)?;
    // Só o fato fica no log; o token, nunca.
    tracing::warn!(dias = body.days, "token service_role emitido pelo painel");
    let expires_at = jsonwebtoken::get_current_timestamp() + body.days * 86_400;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({ "token": token, "expires_at": expires_at })),
    )
        .into_response())
}
