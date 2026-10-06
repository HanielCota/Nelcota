//! Erro HTTP da API, no formato `{"code": "...", "message": "..."}`.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;
use tokio_postgres::error::SqlState;

use crate::Role;

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

    /// 429 com `Retry-After`.
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        ApiError {
            retry_after_secs: Some(retry_after_secs.max(1)),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "muitas tentativas; aguarde e tente de novo",
            )
        }
    }

    /// Token ausente quando exigido, inválido, expirado ou com role desconhecida.
    pub fn invalid_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "JWT inválido ou expirado",
        )
    }

    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "erro interno",
        )
    }

    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "banco de dados indisponível",
        )
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Traduz um erro do Postgres. Falta de permissão vira 401 para `anon`
    /// (precisa logar) e 403 para as demais roles, como no PostgREST.
    pub fn from_db(err: tokio_postgres::Error, role: Role) -> Self {
        let Some(db) = err.as_db_error() else {
            tracing::error!(error = %err, "falha de comunicação com o Postgres");
            return Self::unavailable();
        };
        let status = match *db.code() {
            SqlState::INSUFFICIENT_PRIVILEGE if role == Role::Anon => StatusCode::UNAUTHORIZED,
            SqlState::INSUFFICIENT_PRIVILEGE => StatusCode::FORBIDDEN,
            SqlState::UNDEFINED_TABLE | SqlState::UNDEFINED_FUNCTION => StatusCode::NOT_FOUND,
            SqlState::UNIQUE_VIOLATION | SqlState::FOREIGN_KEY_VIOLATION => StatusCode::CONFLICT,
            SqlState::NOT_NULL_VIOLATION
            | SqlState::CHECK_VIOLATION
            | SqlState::INVALID_TEXT_REPRESENTATION => StatusCode::BAD_REQUEST,
            SqlState::QUERY_CANCELED => StatusCode::GATEWAY_TIMEOUT,
            _ => {
                tracing::error!(code = db.code().code(), error = %db, "erro inesperado do Postgres");
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
        tracing::error!(error = %err, "não foi possível obter conexão do pool");
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
