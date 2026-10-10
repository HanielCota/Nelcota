//! The error contract: every error response is `{"code", "message"}` JSON,
//! database errors add `sqlstate`/`details`/`hint`/`constraint`, and transient
//! database failures are retryable 503s.

mod common;

use axum::http::{Method, StatusCode, header};
use common::*;
use serde_json::json;

#[tokio::test]
async fn database_errors_carry_postgres_fields() {
    let app = TestApp::spawn().await;
    let s = service_token();

    // Unique violation: constraint and detail from Postgres.
    let reply = app
        .post(
            "/rest/v1/products",
            Some(&s),
            json!({ "id": 1, "name": "X", "price": 1 }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.body["code"], "db_error");
    assert_eq!(reply.body["sqlstate"], "23505");
    assert_eq!(reply.body["constraint"], "products_pkey");
    assert!(
        reply.body["details"]
            .as_str()
            .unwrap()
            .contains("already exists"),
        "{}",
        reply.body
    );
    assert!(reply.body.get("hint").is_none(), "{}", reply.body);
    assert!(reply.body["message"].as_str().unwrap().ends_with("(23505)"));

    // RAISE with a custom P0 SQLSTATE: a 400 with the function's own text.
    let reply = app
        .post("/rest/v1/rpc/fail_with_detail", None, json!({}))
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{}", reply.body);
    assert_eq!(
        reply.body,
        json!({
            "code": "db_error",
            "message": "nothing to ship (P0002)",
            "sqlstate": "P0002",
            "details": "the cart is empty",
            "hint": "add an item first",
        })
    );

    // The default RAISE keeps its 400 and only gains the SQLSTATE.
    let reply = app.post("/rest/v1/rpc/fail", None, json!({})).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["sqlstate"], "P0001");

    // A serialization failure can be retried: 503 + Retry-After.
    let reply = app
        .post("/rest/v1/rpc/fail_transient", None, json!({}))
        .await;
    assert_eq!(
        reply.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        reply.body
    );
    assert_eq!(reply.body["code"], "unavailable");
    assert_eq!(reply.body["sqlstate"], "40001");
    assert_eq!(reply.headers[header::RETRY_AFTER], "1");
}

/// The `{"code", "message"}` JSON of an error reply.
fn assert_api_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.text);
    assert_eq!(
        reply.headers[header::CONTENT_TYPE],
        "application/json",
        "{}",
        reply.text
    );
    assert_eq!(reply.body["code"], code, "{}", reply.text);
    assert!(reply.body["message"].is_string(), "{}", reply.text);
}

#[tokio::test]
async fn every_error_is_json() {
    let app = TestApp::spawn().await;
    let json = [("content-type", "application/json")];

    // Unknown route and a known route with the wrong method.
    let reply = app.request(Method::GET, "/nope", None, None).await;
    assert_api_error(&reply, StatusCode::NOT_FOUND, "not_found");
    let reply = app
        .request(Method::PUT, "/rest/v1/products", None, None)
        .await;
    assert_api_error(&reply, StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    assert!(reply.headers.contains_key(header::ALLOW));

    // Auth: broken JSON, JSON of the wrong shape, no JSON Content-Type,
    // missing query parameter.
    let reply = app
        .raw(Method::POST, "/auth/v1/signup", &json, "{".into())
        .await;
    assert_api_error(&reply, StatusCode::BAD_REQUEST, "invalid_body");
    let reply = app
        .raw(
            Method::POST,
            "/auth/v1/signup",
            &json,
            r#"{"email": 1}"#.into(),
        )
        .await;
    assert_api_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "invalid_body");
    let reply = app
        .raw(
            Method::POST,
            "/auth/v1/recover",
            &[],
            r#"{"email":"a@b.co"}"#.into(),
        )
        .await;
    assert_api_error(
        &reply,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    );
    let reply = app
        .raw(Method::POST, "/auth/v1/token", &json, "{}".into())
        .await;
    assert_api_error(&reply, StatusCode::BAD_REQUEST, "invalid_query");

    // Storage JSON routes.
    let reply = app
        .request_with(
            Method::POST,
            "/storage/v1/object/list/files",
            Some(&service_token()),
            None,
            &json,
        )
        .await;
    assert_api_error(&reply, StatusCode::BAD_REQUEST, "invalid_body");

    // A REST body over the limit (2 MiB by default).
    let big = format!(r#"{{"name": "{}", "price": 1}}"#, "x".repeat(3 << 20));
    let bearer = format!("Bearer {}", service_token());
    let reply = app
        .raw(
            Method::POST,
            "/rest/v1/products",
            &[
                ("content-type", "application/json"),
                ("authorization", &bearer),
            ],
            big,
        )
        .await;
    assert_api_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large");

    // The panel keeps its own error shape.
    let reply = app
        .request(Method::GET, "/admin/api/nope", None, None)
        .await;
    assert!(reply.body.get("error").is_some(), "{}", reply.text);
}
