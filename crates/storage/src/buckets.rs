//! `/storage/v1/bucket`: buckets, under the caller's role. Only
//! `service_role` can create, change or drop one (the other roles have no
//! write privilege on `storage.buckets`), and a policy can let them see some.

use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use nelcota_auth::Auth;
use nelcota_core::{ApiError, Claims, db::begin_request};
use serde::Deserialize;
use tokio_postgres::{error::SqlState, types::ToSql};

use crate::{StorageState, path};

const COLUMNS: &str = "id, public, file_size_limit, allowed_mime_types, created_at, updated_at";

#[derive(Deserialize)]
pub struct NewBucket {
    id: String,
    #[serde(flatten)]
    settings: Settings,
}

/// A bucket's settings; a `PUT` replaces all of them (absent = default).
#[derive(Deserialize)]
pub struct Settings {
    #[serde(default)]
    public: bool,
    file_size_limit: Option<i64>,
    allowed_mime_types: Option<Vec<String>>,
}

impl Settings {
    fn validate(&self) -> Result<(), ApiError> {
        let invalid = |message: &str| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_bucket",
                message.to_owned(),
            )
        };
        if self.file_size_limit.is_some_and(|l| l <= 0) {
            return Err(invalid("file_size_limit must be > 0"));
        }
        let entry_ok = |entry: &String| {
            let mut parts = entry.split('/');
            let token = |s: &str| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
            };
            match (parts.next(), parts.next(), parts.next()) {
                (Some(kind), Some(sub), None) => token(kind) && (sub == "*" || token(sub)),
                _ => false,
            }
        };
        if let Some(list) = &self.allowed_mime_types
            && !list.iter().all(entry_ok)
        {
            return Err(invalid(
                "allowed_mime_types holds types like 'image/png' or 'image/*'",
            ));
        }
        Ok(())
    }

    fn mime_types(&self) -> Option<Vec<String>> {
        self.allowed_mime_types
            .as_ref()
            .map(|list| list.iter().map(|m| m.to_ascii_lowercase()).collect())
    }
}

/// Runs one statement that returns JSON text as the caller.
async fn json_as(
    state: &StorageState,
    claims: &Claims,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<Option<String>, ApiError> {
    let db_err = |e: tokio_postgres::Error| match e.code() {
        Some(&SqlState::UNIQUE_VIOLATION) => ApiError::new(
            StatusCode::CONFLICT,
            "bucket_exists",
            "a bucket with this name already exists",
        ),
        Some(&SqlState::FOREIGN_KEY_VIOLATION) => ApiError::new(
            StatusCode::CONFLICT,
            "bucket_not_empty",
            "the bucket still has files; delete them first",
        ),
        _ => ApiError::from_db(e, claims.role()),
    };
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_request(&mut client, claims).await.map_err(db_err)?;
    let row = tx.query_opt(sql, params).await.map_err(db_err)?;
    tx.commit().await.map_err(db_err)?;
    Ok(row.map(|r| r.get(0)))
}

fn json(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "bucket_not_found",
        "bucket not found",
    )
}

/// `GET /storage/v1/bucket`
pub async fn list(
    State(state): State<StorageState>,
    Auth(claims): Auth,
) -> Result<Response, ApiError> {
    let sql = format!(
        "SELECT coalesce(json_agg(b ORDER BY b.id), '[]')::text
           FROM (SELECT {COLUMNS} FROM storage.buckets) b"
    );
    let body = json_as(&state, &claims, &sql, &[])
        .await?
        .unwrap_or_default();
    Ok(json(StatusCode::OK, body))
}

/// `GET /storage/v1/bucket/{id}`
pub async fn one(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let id = path::bucket(&id)?;
    let sql = format!(
        "SELECT row_to_json(b)::text FROM (SELECT {COLUMNS} FROM storage.buckets WHERE id = $1) b"
    );
    let body = json_as(&state, &claims, &sql, &[&id])
        .await?
        .ok_or_else(not_found)?;
    Ok(json(StatusCode::OK, body))
}

/// `POST /storage/v1/bucket`
pub async fn create(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Json(new): Json<NewBucket>,
) -> Result<Response, ApiError> {
    let id = path::bucket(&new.id)?;
    new.settings.validate()?;
    let sql = format!(
        "WITH b AS (
            INSERT INTO storage.buckets (id, public, file_size_limit, allowed_mime_types)
            VALUES ($1, $2, $3, $4) RETURNING {COLUMNS})
         SELECT row_to_json(b)::text FROM b"
    );
    let body = json_as(
        &state,
        &claims,
        &sql,
        &[
            &id,
            &new.settings.public,
            &new.settings.file_size_limit,
            &new.settings.mime_types(),
        ],
    )
    .await?
    .unwrap_or_default();
    Ok(json(StatusCode::CREATED, body))
}

/// `PUT /storage/v1/bucket/{id}`
pub async fn update(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
    Json(settings): Json<Settings>,
) -> Result<Response, ApiError> {
    let id = path::bucket(&id)?;
    settings.validate()?;
    let sql = format!(
        "WITH b AS (
            UPDATE storage.buckets
               SET public = $2, file_size_limit = $3, allowed_mime_types = $4, updated_at = now()
             WHERE id = $1 RETURNING {COLUMNS})
         SELECT row_to_json(b)::text FROM b"
    );
    let body = json_as(
        &state,
        &claims,
        &sql,
        &[
            &id,
            &settings.public,
            &settings.file_size_limit,
            &settings.mime_types(),
        ],
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(json(StatusCode::OK, body))
}

/// `DELETE /storage/v1/bucket/{id}`: only an empty bucket.
pub async fn remove(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let id = path::bucket(&id)?;
    json_as(
        &state,
        &claims,
        "DELETE FROM storage.buckets WHERE id = $1 RETURNING id",
        &[&id],
    )
    .await?
    .ok_or_else(not_found)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
