//! The error contract: every error response is `{"code", "message"}` JSON,
//! database errors add `sqlstate`/`details`/`hint`/`constraint`, and transient
//! database failures are retryable 503s.

mod common;

use axum::http::{StatusCode, header};
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
    assert!(
        reply.body["message"]
            .as_str()
            .unwrap()
            .ends_with("(23505)")
    );

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
