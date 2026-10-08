//! Signup, password authentication and account lookup.
use crate::{
    AuthState, confirmation,
    credentials::{normalize_email, validate_password},
    db::{USER_JSON, begin_auth, db_error},
    error::{invalid, invalid_grant, validation},
    links::{self, LinkKind},
    rate_limit::limit,
    sessions::{session_of, start_session},
};
use axum::http::StatusCode;
use nelcota_core::ApiError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::IpAddr;
use tokio_postgres::error::SqlState;
use uuid::Uuid;
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
    let found: Option<(Uuid, Option<String>, bool)> = {
        let tx = begin_auth(&mut client).await?;
        let row = tx
            .query_opt(
                "SELECT id, encrypted_password, email_confirmed_at IS NOT NULL
                 FROM auth.users WHERE email = $1",
                &[&email],
            )
            .await
            .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        row.map(|r| (r.get(0), r.get(1), r.get(2)))
    };
    // The connection is released while argon2 runs.
    drop(client);

    let (user_id, phc, confirmed) = match found {
        Some((id, phc, confirmed)) => (Some(id), phc, confirmed),
        None => (None, None, false),
    };
    if !state.passwords.verify(password, phc).await {
        return Err(invalid_grant("invalid email or password"));
    }
    let user_id = user_id.expect("verify only succeeds for an existing user");
    // Only after the password: the answer reveals nothing to someone without it.
    confirmation::ensure_confirmed(state, confirmed)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
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
