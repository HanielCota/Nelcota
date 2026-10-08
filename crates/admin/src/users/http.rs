//! HTTP adaptation for end-user account and session administration.
//! Account creation does not confirm email or send invitations.
use super::operations;
use crate::{AdminState, ApiError, contracts::UsersResponse};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

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
) -> ApiResult<UsersResponse> {
    Ok(Json(
        operations::list(
            &state.db,
            params.page,
            params.q.as_deref().unwrap_or_default(),
        )
        .await?,
    ))
}

pub async fn revoke_sessions(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    let count = operations::revoke_sessions(&state.db, &id).await?;
    Ok(Json(
        json!({ "message": format!("{count} session(s) ended"), "count": count }),
    ))
}

pub async fn delete_user(State(state): State<AdminState>, Path(id): Path<String>) -> ApiResult {
    operations::delete(&state.db, &id).await?;
    Ok(Json(json!({ "message": "user deleted" })))
}

#[derive(Deserialize)]
pub struct CreateUser {
    email: String,
    password: String,
}

pub async fn create(
    State(state): State<AdminState>,
    Json(body): Json<CreateUser>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let user = operations::create(&state.db, &body.email, body.password).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user.id, "email": user.email, "message": "user created",
        })),
    ))
}

#[derive(Deserialize)]
pub struct SetPassword {
    password: String,
}

pub async fn set_password(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(body): Json<SetPassword>,
) -> ApiResult {
    let sessions = operations::set_password(&state.db, &id, body.password).await?;
    Ok(Json(json!({
        "message": format!("password reset; {sessions} session(s) ended"),
        "sessions_revoked": sessions,
    })))
}
