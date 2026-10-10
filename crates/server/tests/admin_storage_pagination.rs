mod common;

use axum::http::{Method, StatusCode};
use common::panel::{login, send};
use common::*;
use serde_json::json;

#[tokio::test]
async fn folder_only_pages_expose_the_next_page() {
    let app = TestApp::spawn().await;
    let cookie = login(&app).await;
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets (id) VALUES ('docs');
         INSERT INTO storage.objects (bucket_id, name, version, size, mime_type, etag)
         SELECT 'docs', 'folder' || lpad(n::text, 3, '0') || '/file.txt',
                gen_random_uuid(), 1, 'text/plain', 'etag'
         FROM generate_series(1, 101) n;",
        )
        .await
        .unwrap();

    let first = send(
        &app,
        Method::GET,
        "/admin/api/storage/buckets/docs/objects",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    assert_eq!(first.body["folders"].as_array().unwrap().len(), 100);
    assert_eq!(first.body["has_next"], true);
    assert_eq!(first.body["objects"], json!([]));

    let last = send(
        &app,
        Method::GET,
        "/admin/api/storage/buckets/docs/objects?offset=100",
        &cookie,
        json!(null),
    )
    .await;
    assert_eq!(last.status, StatusCode::OK, "{}", last.text);
    assert_eq!(last.body["folders"], json!(["folder101"]));
    assert_eq!(last.body["has_next"], false);
}
