//! `service_role` token issuing from the panel. The token is not stored: it
//! shows once for the admin to copy. Whoever can open the panel already owns
//! the database, so issuing one does not widen what they can do.

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

/// Signing keys and issuer (`iss`) of the project's tokens.
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
        return Err(ApiError::bad_request(
            "invalid_token_validity",
            format!("validity between 1 and {MAX_DAYS} days"),
        )
        .params(json!({ "max": MAX_DAYS })));
    }
    let token = state
        .tokens
        .keys
        .service_role_token(&state.tokens.issuer, body.days)?;
    // Only the fact goes to the log; the token, never.
    tracing::warn!(days = body.days, "service_role token issued by the panel");
    let expires_at = jsonwebtoken::get_current_timestamp() + body.days * 86_400;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({ "token": token, "expires_at": expires_at })),
    )
        .into_response())
}
