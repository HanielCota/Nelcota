//! `Json` and `Query` extractors whose rejections are [`ApiError`]s, so a
//! malformed body or query string gets the same `{"code", "message"}` JSON as
//! every other error instead of axum's plain-text rejection.
//!
//! The status codes are axum's (400 for broken JSON, 422 for JSON of the
//! wrong shape, 415 without a JSON `Content-Type`, 413 over the body limit).

use axum::{
    extract::{
        FromRequest, FromRequestParts, Request,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::ApiError;

/// JSON body (and JSON response, like `axum::Json`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

/// Query string deserialized into `T`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        let status = rejection.status();
        let code = match status {
            StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
            StatusCode::UNSUPPORTED_MEDIA_TYPE => "unsupported_media_type",
            _ => "invalid_body",
        };
        ApiError::new(status, code, rejection.body_text())
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            rejection.body_text(),
        )
    }
}

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(request, state).await?;
        Ok(Json(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Query(value) =
            axum::extract::Query::<T>::from_request_parts(parts, state).await?;
        Ok(Query(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::post};
    use tower::ServiceExt;

    #[derive(serde::Deserialize)]
    struct Input {
        #[allow(dead_code)]
        n: i32,
    }

    async fn handler(Query(_): Query<Input>, Json(_): Json<Input>) -> &'static str {
        "ok"
    }

    async fn call(uri: &str, content_type: Option<&str>, body: &str) -> (StatusCode, String) {
        let mut request = Request::post(uri);
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = Router::new()
            .route("/", post(handler))
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json["code"].as_str().unwrap().to_owned())
    }

    #[tokio::test]
    async fn rejections_are_json_api_errors() {
        let json = Some("application/json");
        assert_eq!(
            call("/?n=x", json, r#"{"n":1}"#).await,
            (StatusCode::BAD_REQUEST, "invalid_query".into())
        );
        assert_eq!(
            call("/?n=1", json, "{").await,
            (StatusCode::BAD_REQUEST, "invalid_body".into())
        );
        assert_eq!(
            call("/?n=1", json, r#"{"n":"x"}"#).await,
            (StatusCode::UNPROCESSABLE_ENTITY, "invalid_body".into())
        );
        assert_eq!(
            call("/?n=1", None, r#"{"n":1}"#).await,
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type".into()
            )
        );
    }
}
