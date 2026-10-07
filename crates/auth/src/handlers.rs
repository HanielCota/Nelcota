//! `/auth/v1/*` endpoints: signup, login, refresh with rotation, logout,
//! current user, password recovery and JWKS.
//!
//! The `auth.*` tables are accessed with `SET LOCAL ROLE nelcota_auth`, an
//! internal role that no JWT can assume.

use std::{net::IpAddr, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::{ConnectInfo, FromRef, FromRequestParts, Query, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::{Object, Pool, Transaction};
use jsonwebtoken::get_current_timestamp;
use nelcota_core::{ApiError, Role};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::error::SqlState;
use uuid::Uuid;

use crate::{
    Auth, JwtVerifier, Keys, Mailer, Passwords, RateLimiter, SharedVerifier,
    credentials::{InvalidCredential, normalize_email, validate_password},
    recovery,
};

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

pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/auth/v1/signup", post(signup))
        .route("/auth/v1/token", post(token))
        .route("/auth/v1/logout", post(logout))
        .route("/auth/v1/user", get(user))
        .route("/auth/v1/recover", post(recovery::recover))
        .route("/auth/v1/verify", post(recovery::verify))
        .route("/auth/v1/.well-known/jwks.json", get(jwks))
        .with_state(state)
}

// ---------------------------------------------------------------- errors

pub(crate) fn invalid_grant(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_grant", message)
}

pub(crate) fn validation(message: &str) -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        message,
    )
}

pub(crate) fn db_error(err: tokio_postgres::Error) -> ApiError {
    ApiError::from_db(err, Role::ServiceRole)
}

// ---------------------------------------------------------------- helpers

/// Address of the TCP connection, when available (absent in `oneshot` tests).
pub(crate) struct PeerAddr(pub(crate) Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for PeerAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(PeerAddr(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0),
        ))
    }
}

/// Client IP. Behind a proxy (`trust_proxy`), uses the rightmost entry of
/// `X-Forwarded-For` (the one OUR proxy added).
pub(crate) fn client_ip(
    settings: &AuthSettings,
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
) -> Option<IpAddr> {
    if settings.trust_proxy {
        let forwarded = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|v| v.trim().parse().ok());
        if forwarded.is_some() {
            return forwarded;
        }
    }
    peer.map(|addr| addr.ip())
}

pub(crate) fn limit(state: &AuthState, key: &str) -> Result<(), ApiError> {
    state
        .limiter
        .check(key)
        .map_err(|wait| ApiError::rate_limited(wait.as_secs()))
}

/// A credential rejected by the `credentials` rules becomes a validation error.
pub(crate) fn invalid(err: InvalidCredential) -> ApiError {
    validation(err.0)
}

/// Opaque token (refresh, recovery link): 32 random bytes. Only the SHA-256
/// goes to the database.
pub(crate) fn new_opaque_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("system randomness source unavailable");
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    (token, hash)
}

pub(crate) async fn begin_auth(client: &mut Object) -> Result<Transaction<'_>, ApiError> {
    let tx = client.transaction().await.map_err(db_error)?;
    tx.batch_execute("SET LOCAL ROLE nelcota_auth")
        .await
        .map_err(db_error)?;
    Ok(tx)
}

const USER_JSON: &str = "json_build_object(
    'id', u.id,
    'email', u.email,
    'email_confirmed_at', u.email_confirmed_at,
    'user_metadata', u.raw_user_meta_data,
    'created_at', u.created_at,
    'last_sign_in_at', u.last_sign_in_at)";

struct SessionUser {
    id: Uuid,
    email: String,
    json: Value,
}

/// Creates a session + first refresh token, records the login and returns the response.
pub(crate) async fn start_session(
    state: &AuthState,
    tx: &Transaction<'_>,
    user_id: Uuid,
    ip: Option<IpAddr>,
    user_agent: Option<&str>,
) -> Result<Value, ApiError> {
    let session_id: Uuid = tx
        .query_one(
            "INSERT INTO auth.sessions (user_id, ip, user_agent) VALUES ($1, $2, $3) RETURNING id",
            &[&user_id, &ip, &user_agent],
        )
        .await
        .map_err(db_error)?
        .get(0);
    let row = tx
        .query_one(
            &format!(
                "UPDATE auth.users u SET last_sign_in_at = now() WHERE u.id = $1
                 RETURNING u.id, u.email, {USER_JSON}"
            ),
            &[&user_id],
        )
        .await
        .map_err(db_error)?;
    let user = SessionUser {
        id: row.get(0),
        email: row.get(1),
        json: row.get(2),
    };
    issue_tokens(state, tx, session_id, &user).await
}

/// Issues an access JWT + a new refresh token in the session.
async fn issue_tokens(
    state: &AuthState,
    tx: &Transaction<'_>,
    session_id: Uuid,
    user: &SessionUser,
) -> Result<Value, ApiError> {
    let (refresh_token, hash) = new_opaque_token();
    let ttl_days = i32::try_from(state.settings.refresh_ttl_days).unwrap_or(i32::MAX);
    tx.execute(
        "INSERT INTO auth.refresh_tokens (session_id, token_hash, expires_at)
         VALUES ($1, $2, now() + make_interval(days => $3))",
        &[&session_id, &hash, &ttl_days],
    )
    .await
    .map_err(db_error)?;

    let now = get_current_timestamp();
    let expires_at = now + state.settings.access_ttl_secs;
    let claims = json!({
        "iss": state.settings.issuer,
        "aud": "authenticated",
        "sub": user.id,
        "role": "authenticated",
        "email": user.email,
        "session_id": session_id,
        "iat": now,
        "exp": expires_at,
    });
    let access_token = state.keys.sign(&claims).map_err(|err| {
        tracing::error!(error = %err, "failed to sign JWT");
        ApiError::internal()
    })?;

    Ok(json!({
        "access_token": access_token,
        "token_type": "bearer",
        "expires_in": state.settings.access_ttl_secs,
        "expires_at": expires_at,
        "refresh_token": refresh_token,
        "user": user.json,
    }))
}

pub(crate) fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

// ---------------------------------------------------------------- handlers

#[derive(Deserialize)]
struct SignupBody {
    email: String,
    password: String,
    #[serde(default)]
    data: Option<Value>,
}

/// `POST /auth/v1/signup` `{email, password, data?}` → 201 + session.
async fn signup(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<SignupBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !state.settings.signup_enabled {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "signup_disabled",
            "signup is disabled",
        ));
    }
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("signup:{}", ip_key(ip)))?;

    let email = normalize_email(&body.email).map_err(invalid)?;
    validate_password(&body.password).map_err(invalid)?;
    let metadata = match body.data {
        None => json!({}),
        Some(data @ Value::Object(_)) => data,
        Some(_) => return Err(validation("data must be a JSON object")),
    };

    let hash = state
        .passwords
        .hash(body.password)
        .await
        .ok_or_else(ApiError::internal)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let inserted = tx
        .query_one(
            "INSERT INTO auth.users (email, encrypted_password, raw_user_meta_data)
             VALUES ($1, $2, $3) RETURNING id",
            &[&email, &hash, &metadata],
        )
        .await;
    let user_id: Uuid = match inserted {
        Ok(row) => row.get(0),
        Err(err) if err.code() == Some(&SqlState::UNIQUE_VIOLATION) => {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "user_already_exists",
                "a user with this email already exists",
            ));
        }
        Err(err) => return Err(db_error(err)),
    };
    let session = start_session(&state, &tx, user_id, ip, user_agent(&headers)).await?;
    tx.commit().await.map_err(db_error)?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub(crate) fn ip_key(ip: Option<IpAddr>) -> String {
    ip.map(|i| i.to_string()).unwrap_or_default()
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
        "password" => password_grant(&state, body, ip, &headers).await?,
        "refresh_token" => refresh_grant(&state, body).await?,
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

async fn password_grant(
    state: &AuthState,
    body: TokenBody,
    ip: Option<IpAddr>,
    headers: &HeaderMap,
) -> Result<Value, ApiError> {
    let (Some(email), Some(password)) = (body.email, body.password) else {
        return Err(validation("email and password are required"));
    };
    let Ok(email) = normalize_email(&email) else {
        return Err(invalid_grant("invalid email or password"));
    };
    // Per-account limit on top of the per-IP one (distributed attacker).
    limit(state, &format!("login:{email}"))?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let found: Option<(Uuid, Option<String>)> = {
        let tx = begin_auth(&mut client).await?;
        let row = tx
            .query_opt(
                "SELECT id, encrypted_password FROM auth.users WHERE email = $1",
                &[&email],
            )
            .await
            .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        row.map(|r| (r.get(0), r.get(1)))
    };
    // The connection is released while argon2 runs.
    drop(client);

    let (user_id, phc) = match found {
        Some((id, phc)) => (Some(id), phc),
        None => (None, None),
    };
    if !state.passwords.verify(password, phc).await {
        return Err(invalid_grant("invalid email or password"));
    }
    let user_id = user_id.expect("verify only succeeds for an existing user");

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let session = start_session(state, &tx, user_id, ip, user_agent(headers)).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(session)
}

async fn refresh_grant(state: &AuthState, body: TokenBody) -> Result<Value, ApiError> {
    let Some(token) = body
        .refresh_token
        .filter(|t| !t.is_empty() && t.len() <= 128)
    else {
        return Err(validation("refresh_token is required"));
    };
    let hash = Sha256::digest(token.as_bytes()).to_vec();

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    // FOR UPDATE serializes concurrent refreshes of the same token.
    let row = tx
        .query_opt(
            &format!(
                "SELECT rt.id, rt.revoked, rt.expires_at < now(), s.id, s.revoked_at IS NOT NULL,
                        u.id, u.email, {USER_JSON}
                 FROM auth.refresh_tokens rt
                 JOIN auth.sessions s ON s.id = rt.session_id
                 JOIN auth.users u ON u.id = s.user_id
                 WHERE rt.token_hash = $1
                 FOR UPDATE OF rt, s"
            ),
            &[&hash],
        )
        .await
        .map_err(db_error)?;
    let Some(row) = row else {
        return Err(invalid_grant("invalid refresh token"));
    };
    let (token_id, revoked, expired, session_id, session_revoked): (i64, bool, bool, Uuid, bool) =
        (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));

    if session_revoked {
        return Err(invalid_grant("session ended"));
    }
    if revoked {
        // Reuse: someone holds a copy of an already rotated token. End the whole
        // session (every token of the family stops working).
        tx.execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE id = $1",
            &[&session_id],
        )
        .await
        .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        tracing::warn!(session_id = %session_id, "refresh token reuse detected; session revoked");
        return Err(invalid_grant("refresh token reused; session ended"));
    }
    if expired {
        return Err(invalid_grant("refresh token expired"));
    }

    tx.execute(
        "UPDATE auth.refresh_tokens SET revoked = true WHERE id = $1",
        &[&token_id],
    )
    .await
    .map_err(db_error)?;
    tx.execute(
        "UPDATE auth.sessions SET refreshed_at = now() WHERE id = $1",
        &[&session_id],
    )
    .await
    .map_err(db_error)?;
    let user = SessionUser {
        id: row.get(5),
        email: row.get(6),
        json: row.get(7),
    };
    let session = issue_tokens(state, &tx, session_id, &user).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(session)
}

fn session_of(claims: &nelcota_core::Claims) -> Result<(Uuid, Uuid), ApiError> {
    let user_id = claims
        .sub()
        .filter(|_| claims.role() == Role::Authenticated)
        .ok_or_else(ApiError::invalid_token)?;
    let session_id = claims
        .claim("session_id")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .ok_or_else(ApiError::invalid_token)?;
    Ok((user_id, session_id))
}

/// `POST /auth/v1/logout` (Bearer): ends the token's session. The access JWT
/// stays valid until it expires (stateless); refresh tokens die immediately.
async fn logout(
    State(state): State<AuthState>,
    Auth(claims): Auth,
) -> Result<StatusCode, ApiError> {
    let (user_id, session_id) = session_of(&claims)?;
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    tx.execute(
        "UPDATE auth.sessions SET revoked_at = now()
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        &[&session_id, &user_id],
    )
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /auth/v1/user` (Bearer): data of the token's user.
async fn user(State(state): State<AuthState>, Auth(claims): Auth) -> Result<Json<Value>, ApiError> {
    let (user_id, _) = session_of(&claims)?;
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let row = tx
        .query_opt(
            &format!("SELECT {USER_JSON} FROM auth.users u WHERE u.id = $1"),
            &[&user_id],
        )
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    let row = row.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "user_not_found",
            "the user no longer exists",
        )
    })?;
    Ok(Json(row.get(0)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_forwarded_for_only_with_trust_proxy() {
        let mut settings = AuthSettings {
            issuer: "t".into(),
            access_ttl_secs: 1,
            refresh_ttl_days: 1,
            signup_enabled: true,
            trust_proxy: false,
            recovery_url: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.1.1.1, 2.2.2.2".parse().unwrap());
        let peer = Some("9.9.9.9:1234".parse().unwrap());
        assert_eq!(
            client_ip(&settings, &headers, peer),
            Some("9.9.9.9".parse().unwrap())
        );
        settings.trust_proxy = true;
        // Only the rightmost entry is trustworthy (the client forges the rest).
        assert_eq!(
            client_ip(&settings, &headers, peer),
            Some("2.2.2.2".parse().unwrap())
        );
    }
}
