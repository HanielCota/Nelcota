//! End-user accounts from the panel: create and reset passwords. The email and
//! password rules are the same as public signup (`nelcota_auth`).
//!
//! There is no invite and no email confirmation here: account creation by the
//! admin does not send email, and sign-in does not require
//! `email_confirmed_at`.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use nelcota_auth::{hash_password, normalize_email, validate_password};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::error::SqlState;

use crate::{
    AdminState, ApiError,
    api::{invalid_id, is_uuid, user_not_found},
};

/// Longest password `validate_password` accepts (kept in sync with nelcota_auth).
const MAX_PASSWORD: usize = 256;
/// Shortest password `validate_password` accepts.
const MIN_PASSWORD: usize = 8;

fn invalid_email() -> ApiError {
    ApiError::bad_request("invalid_email", "invalid email")
}

/// Password rule broken, with a code per rule so the panel can translate it.
fn check_password(password: &str) -> Result<(), ApiError> {
    validate_password(password).map_err(|err| {
        let code = if password.chars().count() < MIN_PASSWORD {
            "password_too_short"
        } else {
            "password_too_long"
        };
        ApiError::bad_request(code, err.0)
            .params(json!({ "min": MIN_PASSWORD, "max": MAX_PASSWORD }))
    })
}

/// argon2 burns tens of milliseconds of CPU: off the async runtime's threads.
async fn hash(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await?
        .ok_or_else(|| ApiError::from("failed to hash the password"))
}

#[derive(Deserialize)]
pub struct CreateUser {
    email: String,
    password: String,
}

/// `POST /admin/api/users`
pub async fn create(
    State(state): State<AdminState>,
    Json(body): Json<CreateUser>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let email = normalize_email(&body.email).map_err(|_| invalid_email())?;
    check_password(&body.password)?;
    let hash = hash(body.password).await?;
    let client = state.db.get().await?;
    let row = client
        .query_one(
            "INSERT INTO auth.users (email, encrypted_password) VALUES ($1, $2) RETURNING id::text",
            &[&email, &hash],
        )
        .await
        .map_err(|err| {
            if err.code() == Some(&SqlState::UNIQUE_VIOLATION) {
                ApiError::conflict(
                    "user_already_exists",
                    "a user with this email already exists",
                )
            } else {
                ApiError::from(err)
            }
        })?;
    // Only the fact goes to the log; email and password, no.
    tracing::info!("user created by the panel");
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": row.get::<_, String>(0), "email": email, "message": "user created" })),
    ))
}

#[derive(Deserialize)]
pub struct SetPassword {
    password: String,
}

/// `PUT /admin/api/users/{id}/password`: changes the password and ends the
/// open sessions (whoever resets one usually suspects unwanted access).
pub async fn set_password(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(body): Json<SetPassword>,
) -> Result<Json<Value>, ApiError> {
    if !is_uuid(&id) {
        return Err(invalid_id());
    }
    check_password(&body.password)?;
    let hash = hash(body.password).await?;
    let mut client = state.db.get().await?;
    let tx = client.transaction().await?;
    let updated = tx
        .execute(
            "UPDATE auth.users SET encrypted_password = $2, updated_at = now() WHERE id = $1::text::uuid",
            &[&id, &hash],
        )
        .await?;
    if updated == 0 {
        return Err(user_not_found());
    }
    let sessions = tx
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    tx.commit().await?;
    tracing::info!(sessions, "password reset by the panel");
    Ok(Json(json!({
        "message": format!("password reset; {sessions} session(s) ended"),
        "sessions_revoked": sessions,
    })))
}
