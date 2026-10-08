//! Public bucket routes adapt HTTP to the shared operation interface.
use crate::{Bucket, BucketError, BucketSettings, StorageState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use nelcota_auth::Auth;
use nelcota_core::ApiError;
use serde::Deserialize;

impl From<BucketError> for ApiError {
    fn from(error: BucketError) -> Self {
        let (status, code, message) = match error {
            BucketError::InvalidSizeLimit => (
                StatusCode::BAD_REQUEST,
                "invalid_bucket",
                "file_size_limit must be > 0",
            ),
            BucketError::InvalidMimeType(_) => (
                StatusCode::BAD_REQUEST,
                "invalid_bucket",
                "allowed_mime_types holds types like 'image/png' or 'image/*'",
            ),
            BucketError::Exists => (
                StatusCode::CONFLICT,
                "bucket_exists",
                "a bucket with this name already exists",
            ),
            BucketError::NotFound => (
                StatusCode::NOT_FOUND,
                "bucket_not_found",
                "bucket not found",
            ),
            BucketError::NotEmpty { .. } => (
                StatusCode::CONFLICT,
                "bucket_not_empty",
                "the bucket still has files; delete them first",
            ),
            BucketError::Request(error) => return error,
        };
        Self::new(status, code, message)
    }
}

#[derive(Deserialize)]
pub(crate) struct NewBucket {
    id: String,
    #[serde(flatten)]
    settings: BucketSettings,
}

pub(crate) async fn list(
    State(state): State<StorageState>,
    Auth(claims): Auth,
) -> Result<Json<Vec<Bucket>>, ApiError> {
    Ok(Json(state.buckets(&claims).await?))
}

pub(crate) async fn one(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
) -> Result<Json<Bucket>, ApiError> {
    Ok(Json(state.bucket(&claims, &id).await?))
}

pub(crate) async fn create(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Json(new): Json<NewBucket>,
) -> Result<(StatusCode, Json<Bucket>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.create_bucket(&claims, &new.id, new.settings).await?),
    ))
}

pub(crate) async fn update(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
    Json(settings): Json<BucketSettings>,
) -> Result<Json<Bucket>, ApiError> {
    Ok(Json(state.update_bucket(&claims, &id, settings).await?))
}

pub(crate) async fn remove(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.delete_bucket(&claims, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
