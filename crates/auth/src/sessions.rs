//! Session lifecycle and token rotation, independent of HTTP handlers.
use crate::{
    AuthState,
    db::{USER_JSON, begin_auth, db_error},
    error::{invalid_grant, validation},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::Transaction;
use jsonwebtoken::get_current_timestamp;
use nelcota_core::{ApiError, Role};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use uuid::Uuid;
/// Opaque token (refresh, recovery link): 32 random bytes. Only the SHA-256
/// goes to the database.
pub(crate) fn new_opaque_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("system randomness source unavailable");
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    (token, hash)
}

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

pub(crate) async fn refresh(state: &AuthState, token: Option<String>) -> Result<Value, ApiError> {
    let Some(token) = token.filter(|t| !t.is_empty() && t.len() <= 128) else {
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

pub(crate) fn session_of(claims: &nelcota_core::Claims) -> Result<(Uuid, Uuid), ApiError> {
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
pub(crate) async fn logout(
    state: &AuthState,
    claims: &nelcota_core::Claims,
) -> Result<(), ApiError> {
    let (user_id, session_id) = session_of(claims)?;
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
    Ok(())
}
