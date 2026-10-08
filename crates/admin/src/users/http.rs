//! End-user account and session administration from the panel.
//! Account creation does not confirm email or send invitations.
use crate::{AdminState, ApiError};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use nelcota_auth::{hash_password, normalize_email, validate_password};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::error::SqlState;
type ApiResult<T = Value> = Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct UsersQuery {
    #[serde(default)]
    page: i64,
    q: Option<String>,
}

pub async fn users(
    State(state): State<AdminState>,
    Query(params): Query<UsersQuery>,
) -> ApiResult<crate::contracts::UsersResponse> {
    const SIZE: i64 = 50;
    let page = params.page.max(0);
    let search = params.q.unwrap_or_default();
    let pattern = format!(
        "%{}%",
        search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let client = state.db.get().await?;
    let rows = client
        .query(
            "SELECT u.id::text, u.email, u.created_at::text, u.last_sign_in_at::text,
                    u.email_confirmed_at::text,
                    (SELECT count(*) FROM auth.sessions s WHERE s.user_id = u.id AND s.revoked_at IS NULL)
             FROM auth.users u WHERE u.email LIKE $1
             ORDER BY u.created_at DESC LIMIT $2 OFFSET $3",
            &[&pattern, &(SIZE + 1), &(page * SIZE)],
        )
        .await?;
    let total: i64 = client
        .query_one("SELECT count(*) FROM auth.users", &[])
        .await?
        .get(0);
    let users: Vec<Value> = rows
        .iter()
        .take(SIZE as usize)
        .map(|r| {
            json!({
                "id": r.get::<_, String>(0),
                "email": r.get::<_, String>(1),
                "created_at": r.get::<_, String>(2),
                "last_sign_in_at": r.get::<_, Option<String>>(3),
                "email_confirmed_at": r.get::<_, Option<String>>(4),
                "sessions": r.get::<_, i64>(5),
            })
        })
        .collect();
    crate::contracts::response::<crate::contracts::UsersResponse>(json!({
        "total": total,
        "page": page,
        "has_next": rows.len() as i64 > SIZE,
        "users": users,
    }))
}

/// uuid format check (8-4-4-4-12 hex).
fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The user id in the path is not a uuid.
fn invalid_id() -> ApiError {
    ApiError::bad_request("invalid_id", "invalid id")
}

pub async fn revoke_sessions(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(invalid_id());
    }
    let client = state.db.get().await?;
    let n = client
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    Ok(Json(
        json!({ "message": format!("{n} session(s) ended"), "count": n }),
    ))
}

pub async fn delete_user(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    if !is_uuid(&id) {
        return Err(invalid_id());
    }
    let client = state.db.get().await?;
    let n = client
        .execute("DELETE FROM auth.users WHERE id = $1::text::uuid", &[&id])
        .await?;
    if n == 0 {
        return Err(user_not_found());
    }
    Ok(Json(json!({ "message": "user deleted" })))
}

fn user_not_found() -> ApiError {
    ApiError::not_found("user_not_found", "user not found")
}

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

#[cfg(test)]
mod tests {
    use super::is_uuid;
    #[test]
    fn validates_uuid() {
        assert!(is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b92"));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("054f8cd2-decb-4c78-91a1-f351bd8f5b9z"));
    }
}
