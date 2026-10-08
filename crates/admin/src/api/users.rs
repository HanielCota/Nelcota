//! Users panel handlers.
use super::*;

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
pub(crate) fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The user id in the path is not a uuid.
pub(crate) fn invalid_id() -> ApiError {
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

pub(crate) fn user_not_found() -> ApiError {
    ApiError::not_found("user_not_found", "user not found")
}
