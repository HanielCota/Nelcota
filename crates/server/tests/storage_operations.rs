//! The storage operation interface, without constructing HTTP requests.
mod common;

use bytes::Bytes;
use common::TestApp;
use futures_util::stream;
use nelcota_core::Claims;
use nelcota_storage::UploadOptions;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

async fn bucket(app: &TestApp) {
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets (id) VALUES ('docs');
         CREATE POLICY own_folder ON storage.objects FOR ALL TO authenticated
           USING (bucket_id = 'docs' AND (storage.foldername(name))[1] = auth.uid()::text)
           WITH CHECK (bucket_id = 'docs' AND (storage.foldername(name))[1] = auth.uid()::text);",
        )
        .await
        .unwrap();
}
fn claims(user: uuid::Uuid) -> Claims {
    Claims::from_payload(json!({"role": "authenticated", "sub": user})).unwrap()
}

#[tokio::test]
async fn typed_operations_preserve_rls_and_reject_before_polling_bytes() {
    let app = TestApp::spawn().await;
    bucket(&app).await;
    let a = claims(app.user_a);
    let b = claims(app.user_b);
    let name = format!("{}/notes.txt", app.user_a);
    let outcome = app
        .storage
        .upload(
            a.clone(),
            "docs",
            &name,
            UploadOptions {
                content_length: Some(5),
                content_type: Some("text/plain".into()),
                replace: false,
            },
            Box::pin(stream::iter([Ok(Bytes::from_static(b"hello"))])),
        )
        .await
        .unwrap();
    assert_eq!(outcome.object.name, name);
    assert_eq!(outcome.object.size, 5);
    assert!(!outcome.replaced && !outcome.public);
    let listing = app
        .storage
        .list(&a, "docs", &format!("{}/", app.user_a), None, 0)
        .await
        .unwrap();
    assert_eq!(listing.objects.len(), 1);
    assert_eq!(listing.objects[0].id, outcome.object.id);
    assert!(
        app.storage
            .list(&b, "docs", &format!("{}/", app.user_a), None, 0)
            .await
            .unwrap()
            .objects
            .is_empty()
    );
    assert!(app.storage.object(&b, "docs", &name).await.is_err());

    let polled = Arc::new(AtomicBool::new(false));
    let counter = polled.clone();
    let bytes = stream::once(async move {
        counter.store(true, Ordering::SeqCst);
        Ok(Bytes::from_static(b"forbidden"))
    });
    let denied = app
        .storage
        .upload(
            b,
            "docs",
            &name,
            UploadOptions {
                replace: true,
                ..Default::default()
            },
            Box::pin(bytes),
        )
        .await;
    assert!(denied.is_err());
    assert!(
        !polled.load(Ordering::SeqCst),
        "authorization must precede byte consumption"
    );
    assert_eq!(app.storage.object(&a, "docs", &name).await.unwrap().size, 5);
    app.storage.delete(&a, "docs", &name).await.unwrap();
    assert!(app.storage.object(&a, "docs", &name).await.is_err());
}

#[tokio::test]
async fn interrupted_operation_does_not_publish_metadata_or_leave_bytes() {
    let app = TestApp::spawn().await;
    bucket(&app).await;
    let name = format!("{}/interrupted.txt", app.user_a);
    let chunks = stream::iter([
        Ok(Bytes::from(vec![b'x'; 64 * 1024])),
        Err(std::io::Error::other("connection interrupted")),
    ]);
    let result = app
        .storage
        .upload(
            claims(app.user_a),
            "docs",
            &name,
            UploadOptions {
                content_type: Some("text/plain".into()),
                ..Default::default()
            },
            Box::pin(chunks),
        )
        .await;
    assert!(result.is_err());
    let row = app
        .admin_client
        .query_one("SELECT count(*) FROM storage.objects", &[])
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 0);
    let files = app.storage.store.list();
    use futures_util::StreamExt;
    assert_eq!(files.count().await, 0);
}
