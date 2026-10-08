//! HTTP routes and response adaptation for authentication.
use crate::{
    Auth, AuthState, accounts,
    rate_limit::limit,
    recovery,
    request::{PeerAddr, client_ip, ip_key, user_agent},
    sessions,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use nelcota_core::ApiError;
use serde::Deserialize;
use serde_json::Value;
pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/auth/v1/signup", post(signup))
        .route("/auth/v1/token", post(token))
        .route("/auth/v1/logout", post(logout))
        .route("/auth/v1/user", get(user))
        .route("/auth/v1/recover", post(recover))
        .route("/auth/v1/verify", post(verify))
        .route("/auth/v1/.well-known/jwks.json", get(jwks))
        .with_state(state)
}

async fn signup(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<accounts::Signup>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let ip = client_ip(&state.settings, &headers, peer);
    if state.settings.signup_enabled {
        limit(&state, &format!("signup:{}", ip_key(ip)))?;
    }
    let session = accounts::signup(&state, body, ip, user_agent(&headers)).await?;
    Ok((StatusCode::CREATED, Json(session)))
}
async fn logout(
    State(state): State<AuthState>,
    Auth(claims): Auth,
) -> Result<StatusCode, ApiError> {
    sessions::logout(&state, &claims).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn user(State(state): State<AuthState>, Auth(claims): Auth) -> Result<Json<Value>, ApiError> {
    Ok(Json(accounts::user(&state, &claims).await?))
}
#[derive(Deserialize)]
struct GrantQuery {
    grant_type: String,
}

#[derive(Deserialize)]
struct TokenBody {
    email: Option<String>,
    password: Option<String>,
    refresh_token: Option<String>,
}

/// `POST /auth/v1/token?grant_type=password|refresh_token`.
async fn token(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Query(query): Query<GrantQuery>,
    Json(body): Json<TokenBody>,
) -> Result<Json<Value>, ApiError> {
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("token:{}", ip_key(ip)))?;

    let session = match query.grant_type.as_str() {
        "password" => {
            accounts::password_grant(&state, body.email, body.password, ip, user_agent(&headers))
                .await?
        }
        "refresh_token" => sessions::refresh(&state, body.refresh_token).await?,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "grant_type must be password or refresh_token",
            ));
        }
    };
    Ok(Json(session))
}

/// `GET /auth/v1/.well-known/jwks.json`: public keys to validate our JWTs.
async fn jwks(State(state): State<AuthState>) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        state.keys.jwks_json().to_owned(),
    )
}

#[derive(Deserialize)]
struct RecoverBody {
    email: String,
}

async fn recover(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<RecoverBody>,
) -> Result<Json<Value>, ApiError> {
    recovery::ensure_enabled(&state)?;
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("recover:{}", ip_key(ip)))?;
    recovery::request(&state, &body.email).await?;
    Ok(Json(serde_json::json!({})))
}

async fn verify(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<recovery::VerifyBody>,
) -> Result<Json<Value>, ApiError> {
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("verify:{}", ip_key(ip)))?;
    Ok(Json(
        recovery::complete(&state, body, ip, user_agent(&headers)).await?,
    ))
}
