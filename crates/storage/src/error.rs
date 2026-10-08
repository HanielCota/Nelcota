//! Storage failure vocabulary shared by operations and HTTP adapters.
use axum::http::StatusCode;
use nelcota_core::{ApiError, Claims};
use tokio_postgres::error::SqlState;
pub(crate) fn bucket_not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "bucket_not_found",
        "bucket not found",
    )
}

pub(crate) fn storage_full(message: &str) -> ApiError {
    ApiError::new(StatusCode::INSUFFICIENT_STORAGE, "storage_full", message)
}

pub(crate) fn too_large(limit: u64) -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "file_too_large",
        format!("the file exceeds the limit of {limit} bytes"),
    )
}

pub(crate) fn store_failed(err: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %err, "storage backend failed");
    ApiError::internal()
}

/// Maps a database error of a write: an existing name is its own case.
pub(crate) fn write_error(err: tokio_postgres::Error, claims: &Claims) -> ApiError {
    if err.code() == Some(&SqlState::UNIQUE_VIOLATION) {
        return ApiError::new(
            StatusCode::CONFLICT,
            "object_exists",
            "a file with this name already exists; use PUT to replace it",
        );
    }
    ApiError::from_db(err, claims.role())
}

pub(crate) fn object_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "object_not_found", "file not found")
}
