//! Storage in the panel: buckets with usage and a file browser, behind the
//! panel login, going through the same checks as the storage API.

mod common;

use axum::http::{Method, StatusCode};
use common::panel::{login, send};
use common::*;
use serde_json::json;

async fn upload(app: &TestApp, cookie: &str, bucket: &str, name: &str, body: &[u8]) -> Reply {
    app.raw(
        Method::POST,
        &format!("/admin/api/storage/buckets/{bucket}/upload?name={name}"),
        &[("cookie", cookie), ("content-type", "text/plain")],
        String::from_utf8(body.to_vec()).unwrap(),
    )
    .await
}

#[tokio::test]
async fn buckets_and_files_from_the_panel() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;

    let reply = send(
        &app,
        Method::GET,
        "/admin/api/storage",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["enabled"], true);
    assert_eq!(reply.body["backend"], "disk");
    assert_eq!(reply.body["buckets"], json!([]));

    // Validation in the panel's language.
    for (body, code) in [
        (json!({ "id": "Bad Name" }), "invalid_bucket_name"),
        (json!({ "id": "sign" }), "invalid_bucket_name"),
        (
            json!({ "id": "x", "allowed_mime_types": ["png"] }),
            "invalid_mime_type",
        ),
        (
            json!({ "id": "x", "file_size_limit": 0 }),
            "invalid_size_limit",
        ),
    ] {
        let reply = send(
            &app,
            Method::POST,
            "/admin/api/storage/buckets",
            &cookie,
            body,
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body["code"], code);
    }
    let docs = json!({ "id": "docs", "allowed_mime_types": [" Text/* ", ""] });
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/storage/buckets",
        &cookie,
        docs.clone(),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let reply = send(
        &app,
        Method::POST,
        "/admin/api/storage/buckets",
        &cookie,
        docs,
    )
    .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.body["code"], "bucket_exists");

    // Uploads get the API's checks: the bucket only takes text.
    let reply = upload(&app, &cookie, "docs", "notes/a.txt", b"hello").await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let reply = upload(&app, &cookie, "docs", "b.txt", b"%PDF-1.7").await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(reply.body["code"], "mime_type_not_allowed");
    assert!(reply.body["error"].is_string(), "the panel's error shape");
    let reply = upload(&app, &cookie, "docs", "notes/a.txt", b"again").await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/storage/buckets/docs/upload?name=notes/a.txt&replace=true",
            &[("cookie", &cookie), ("content-type", "text/plain")],
            "replaced".into(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);

    let reply = send(
        &app,
        Method::GET,
        "/admin/api/storage",
        &cookie,
        json!(null),
    )
    .await;
    let bucket = &reply.body["buckets"][0];
    assert_eq!(
        (
            bucket["id"].clone(),
            bucket["files"].clone(),
            bucket["bytes"].clone()
        ),
        (json!("docs"), json!(1), json!(8))
    );
    assert_eq!(bucket["allowed_mime_types"], json!(["text/*"]));

    let reply = send(
        &app,
        Method::GET,
        "/admin/api/storage/buckets/docs/objects",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.body["folders"], json!(["notes"]));
    assert_eq!(reply.body["objects"], json!([]));
    assert_eq!(reply.body["has_next"], false);
    let reply = send(
        &app,
        Method::GET,
        "/admin/api/storage/buckets/docs/objects?prefix=notes/",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.body["objects"][0]["name"], "notes/a.txt");

    // A download keeps the file's sandbox policy, not the panel's.
    let reply = app
        .raw(
            Method::GET,
            "/admin/api/storage/buckets/docs/file?name=notes/a.txt",
            &[("cookie", &cookie)],
            String::new(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.text, "replaced");
    assert!(
        reply.headers["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert!(
        reply.headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );

    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/storage/buckets/docs",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.body["code"], "bucket_not_empty");
    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/storage/buckets/docs/file?name=notes/a.txt",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = send(
        &app,
        Method::PUT,
        "/admin/api/storage/buckets/docs",
        &cookie,
        json!({ "public": true }),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = send(
        &app,
        Method::DELETE,
        "/admin/api/storage/buckets/docs",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let reply = send(
        &app,
        Method::PUT,
        "/admin/api/storage/buckets/docs",
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn panel_storage_needs_a_session_and_the_same_origin() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    send(
        &app,
        Method::POST,
        "/admin/api/storage/buckets",
        &cookie,
        json!({ "id": "docs" }),
    )
    .await;

    let reply = app
        .raw(
            Method::POST,
            "/admin/api/storage/buckets/docs/upload?name=a.txt",
            &[],
            "x".into(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .raw(Method::GET, "/admin/api/storage", &[], String::new())
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let reply = app
        .raw(
            Method::POST,
            "/admin/api/storage/buckets/docs/upload?name=a.txt",
            &[
                ("cookie", &cookie),
                ("origin", "https://evil.example"),
                ("host", "shop.example.com"),
            ],
            "x".into(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.body["code"], "origin_not_allowed");
}
