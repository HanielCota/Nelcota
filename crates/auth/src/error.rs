//! Authentication error vocabulary shared by the workflows.
use crate::InvalidCredential;
use axum::http::StatusCode;
use nelcota_core::ApiError;
pub(crate) fn invalid_grant(message: &str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_grant", message)
}

pub(crate) fn validation(message: &str) -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        message,
    )
}

/// A credential rejected by the `credentials` rules becomes a validation error.
pub(crate) fn invalid(err: InvalidCredential) -> ApiError {
    validation(err.0)
}
