//! Signup, password authentication and account lookup.
use crate::{
    AuthState, confirmation,
    credentials::{normalize_email, validate_password},
    db::{USER_JSON, begin_auth, db_error},
    error::{invalid_email, invalid_grant, validation, weak_password},
    links::{self, LinkKind},
    rate_limit::limit,
    sessions::{session_of, start_session},
};
use axum::http::StatusCode;
use deadpool_postgres::Transaction;
use nelcota_core::ApiError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::IpAddr;
use tokio_postgres::error::SqlState;
use uuid::Uuid;

/// Claim an unconfirmed account through independently verified inbox ownership.
/// The caller holds the account's row lock. Credentials established before that
/// proof may belong to someone else; only the newly created session is retained.
pub(crate) async fn confirm_inbox_owner(
    tx: &Transaction<'_>,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let confirmed = tx
        .execute(
            "UPDATE auth.users
                SET encrypted_password = NULL, email_confirmed_at = now(), updated_at = now()
              WHERE id = $1 AND email_confirmed_at IS NULL",
            &[&user_id],
        )
        .await
        .map_err(db_error)?;
    if confirmed == 0 {
        return Ok(());
    }
    tx.execute(
        "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    tx.execute(
        "DELETE FROM auth.one_time_tokens WHERE user_id = $1",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    tx.execute(
        "DELETE FROM auth.identities WHERE user_id = $1",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    tx.execute(
        "DELETE FROM auth.flow_states WHERE user_id = $1",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    tracing::warn!(user_id = %user_id, "unconfirmed account claimed by its verified inbox owner");
    Ok(())
}
#[derive(Deserialize)]
pub(crate) struct Signup {
    email: String,
    password: String,
    #[serde(default)]
    data: Option<Value>,
}

/// `POST /auth/v1/signup` `{email, password, data?}` → 201 + session, or 201 +
/// `{user}` when the email must be confirmed first.
pub(crate) async fn signup(
    state: &AuthState,
    body: Signup,
    ip: Option<IpAddr>,
    user_agent: Option<&str>,
) -> Result<Value, ApiError> {
    if !state.settings.signup_enabled {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "signup_disabled",
            "signup is disabled",
        ));
    }

    let email = normalize_email(&body.email).map_err(invalid_email)?;
    validate_password(&body.password).map_err(weak_password)?;
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
    if !confirmation::required(state) {
        let session = start_session(state, &tx, user_id, ip, user_agent).await?;
        tx.commit().await.map_err(db_error)?;
        return Ok(session);
    }
    let (pending, token) = confirmation::pending_signup(&tx, user_id).await?;
    tx.commit().await.map_err(db_error)?;
    if let Some(token) = token {
        links::deliver(state, LinkKind::Signup, &email, &token);
    }
    Ok(pending)
}

pub(crate) async fn password_grant(
    state: &AuthState,
    email: Option<String>,
    password: Option<String>,
    ip: Option<IpAddr>,
    user_agent: Option<&str>,
) -> Result<Value, ApiError> {
    let (Some(email), Some(password)) = (email, password) else {
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
                "SELECT id, encrypted_password
                 FROM auth.users WHERE email = $1",
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
    if !state.passwords.verify(password, phc.clone()).await {
        return Err(invalid_grant("invalid email or password"));
    }
    let user_id = user_id.expect("verify only succeeds for an existing user");
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    // Password resets update this same row before revoking sessions. Keep its
    // lock until the new session commits, but never while running argon2.
    let current = tx
        .query_opt(
            "SELECT encrypted_password, email_confirmed_at IS NOT NULL
             FROM auth.users WHERE id = $1 FOR UPDATE",
            &[&user_id],
        )
        .await
        .map_err(db_error)?
        .ok_or_else(|| invalid_grant("invalid email or password"))?;
    if current.get::<_, Option<String>>(0) != phc {
        return Err(invalid_grant("invalid email or password"));
    }
    confirmation::ensure_confirmed(state, current.get(1))?;
    let session = start_session(state, &tx, user_id, ip, user_agent).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(session)
}

/// `GET /auth/v1/user` (Bearer): data of the token's user.
pub(crate) async fn user(
    state: &AuthState,
    claims: &nelcota_core::Claims,
) -> Result<Value, ApiError> {
    let (user_id, _) = session_of(claims)?;
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
    Ok(row.get(0))
}
