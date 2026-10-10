//! API HTTP error, shaped as `{"code": "...", "message": "..."}`.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;
use tokio_postgres::error::SqlState;

use crate::Role;

/// The code and message of an error response, attached to the response's
/// extensions so a layer (the panel's log of denied requests) can read them
/// without parsing the body.
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after_secs: Option<u64>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError {
            status,
            code,
            message: message.into(),
            retry_after_secs: None,
        }
    }

    /// 429 with `Retry-After`.
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        ApiError {
            retry_after_secs: Some(retry_after_secs.max(1)),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many attempts; wait and try again",
            )
        }
    }

    /// Token missing when required, invalid, expired or with an unknown role.
    pub fn invalid_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "invalid or expired JWT",
        )
    }

    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
    }

    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "database unavailable",
        )
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Maps a Postgres error. Missing privileges become 401 for `anon` (must
    /// sign in) and 403 for the other roles, as in PostgREST.
    pub fn from_db(err: tokio_postgres::Error, role: Role) -> Self {
        let Some(db) = err.as_db_error() else {
            tracing::error!(error = %err, "failed to communicate with Postgres");
            return Self::unavailable();
        };
        let code = db.code().code();
        if code == "54000" && db.message() == "NELCOTA_RESPONSE_TOO_LARGE" {
            return Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "response_too_large",
                "the JSON response exceeds the 8 MiB budget; narrow the selection or page the results",
            );
        }
        let status = match *db.code() {
            SqlState::INSUFFICIENT_PRIVILEGE if role == Role::Anon => StatusCode::UNAUTHORIZED,
            SqlState::INSUFFICIENT_PRIVILEGE => StatusCode::FORBIDDEN,
            SqlState::UNDEFINED_TABLE => StatusCode::NOT_FOUND,
            SqlState::UNIQUE_VIOLATION
            | SqlState::FOREIGN_KEY_VIOLATION
            | SqlState::EXCLUSION_VIOLATION => StatusCode::CONFLICT,
            SqlState::QUERY_CANCELED => StatusCode::GATEWAY_TIMEOUT,
            // Incompatible type/operator, generated column, RAISE EXCEPTION in a
            // user function: a client error, not a server one.
            // Upserts: `on_conflict` matching no unique constraint, or a batch
            // that hits the same key twice.
            SqlState::UNDEFINED_FUNCTION
            | SqlState::UNDEFINED_COLUMN
            | SqlState::DATATYPE_MISMATCH
            | SqlState::GENERATED_ALWAYS
            | SqlState::RAISE_EXCEPTION
            | SqlState::INVALID_COLUMN_REFERENCE
            | SqlState::CARDINALITY_VIOLATION => StatusCode::BAD_REQUEST,
            // Class 22 (invalid data) and 23 (integrity): 400.
            _ if code.starts_with("22") || code.starts_with("23") => StatusCode::BAD_REQUEST,
            _ => {
                tracing::error!(code = db.code().code(), error = %db, "unexpected Postgres error");
                return Self::internal();
            }
        };
        Self::new(
            status,
            "db_error",
            format!("{} ({})", db.message(), db.code().code()),
        )
    }

    pub fn from_pool(err: deadpool_postgres::PoolError) -> Self {
        tracing::error!(error = %err, "could not get a connection from the pool");
        Self::unavailable()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(json!({ "code": self.code, "message": self.message })),
        )
            .into_response();
        response.extensions_mut().insert(ErrorInfo {
            code: self.code,
            message: self.message,
        });
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static(r#"Bearer error="invalid_token""#),
            );
        }
        if let Some(secs) = self.retry_after_secs {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(secs));
        }
        response
    }
}
