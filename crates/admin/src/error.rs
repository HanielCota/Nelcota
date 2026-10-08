//! Panel error representation and HTTP adaptation.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
/// Panel API error: `{"error": "<English text>", "code": "...", "params": {...}}`.
///
/// `code` and `params` let the panel show the error in the user's language
/// (crates/admin/ui/src/lib/i18n/messages/errors.ts); `error` stays the
/// English text for other API consumers. Errors that come straight from
/// Postgres carry no code, so the panel shows their text as-is.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    code: Option<&'static str>,
    params: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError {
            status,
            message: message.into(),
            code: Some(code),
            params: None,
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    /// Values for the placeholders of the translated message.
    pub fn params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }

    /// Error without a code, shown verbatim (e.g. a Postgres message).
    pub fn raw(status: StatusCode, message: impl Into<String>) -> Self {
        ApiError {
            status,
            message: message.into(),
            code: None,
            params: None,
        }
    }
}

/// Any other error (database, pool, JSON) becomes a 500 with its message: the
/// admin owns the database and needs the text to act on it.
impl<E: std::fmt::Display> From<E> for ApiError {
    fn from(err: E) -> Self {
        ApiError::raw(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.message });
        if let Some(code) = self.code {
            body["code"] = json!(code);
        }
        if let Some(params) = self.params {
            body["params"] = params;
        }
        (self.status, Json(body)).into_response()
    }
}

pub(crate) async fn api_not_found() -> ApiError {
    ApiError::not_found("route_not_found", "route not found")
}

/// Error of a query built from what the admin typed (filter value of the
/// wrong type, constraint violation…): 400 with Postgres' text, no code.
pub(crate) fn user_query_error(err: tokio_postgres::Error) -> ApiError {
    ApiError::raw(
        axum::http::StatusCode::BAD_REQUEST,
        err.as_db_error()
            .map_or_else(|| err.to_string(), |db| db.message().to_owned()),
    )
}
