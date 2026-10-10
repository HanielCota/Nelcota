//! Regression coverage for authentication and storage invariants under overlap.
mod common;

use axum::http::StatusCode;
use bytes::Bytes;
use common::{Options, TestApp};
use futures_util::stream;
use nelcota_core::Claims;
use nelcota_storage::{StorageState, UploadOptions};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[tokio::test]
async fn password_changed_during_login_cannot_create_a_new_session() {
    let app = Arc::new(TestApp::spawn().await);
    let reply = app
        .post(
            "/auth/v1/signup",
            None,
            json!({"email":"race@example.com", "password":"old-password-123"}),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let id: Uuid = reply.body["user"]["id"].as_str().unwrap().parse().unwrap();
    let hash = nelcota_auth::hash_password("new-password-123").unwrap();
    let (mut locker, connection) = app.admin.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    let reset = locker.transaction().await.unwrap();
    reset
        .query_one("SELECT id FROM auth.users WHERE id=$1 FOR UPDATE", &[&id])
        .await
        .unwrap();
    let logging_in = app.clone();
    let pending = tokio::spawn(async move {
        logging_in
            .post(
                "/auth/v1/token?grant_type=password",
                None,
                json!({"email":"race@example.com", "password":"old-password-123"}),
            )
            .await
    });
    wait_for_locks(
        &app,
        1,
        "query LIKE 'SELECT encrypted_password%' OR query LIKE 'INSERT INTO auth.sessions%'",
    )
    .await;
    reset
        .execute(
            "UPDATE auth.users SET encrypted_password=$2 WHERE id=$1",
            &[&id, &hash],
        )
        .await
        .unwrap();
    reset
        .execute(
            "UPDATE auth.sessions SET revoked_at=now() WHERE user_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    reset.commit().await.unwrap();
    let reply = pending.await.unwrap();
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_grant");
    let active: i64 = app
        .admin_client
        .query_one(
            "SELECT count(*) FROM auth.sessions WHERE user_id=$1 AND revoked_at IS NULL",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(active, 0);
}

#[tokio::test]
async fn concurrent_recovery_requests_issue_one_link_and_one_email() {
    let app = Arc::new(TestApp::spawn().await);
    app.post(
        "/auth/v1/signup",
        None,
        json!({"email":"links@example.com", "password":"old-password-123"}),
    )
    .await;
    let (mut locker, connection) = app.admin.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    let lock = locker.transaction().await.unwrap();
    lock.batch_execute("LOCK TABLE auth.one_time_tokens IN SHARE MODE")
        .await
        .unwrap();
    let a = app.clone();
    let first = tokio::spawn(async move {
        a.post(
            "/auth/v1/recover",
            None,
            json!({"email":"links@example.com"}),
        )
        .await
    });
    let b = app.clone();
    let second = tokio::spawn(async move {
        b.post(
            "/auth/v1/recover",
            None,
            json!({"email":"links@example.com"}),
        )
        .await
    });
    wait_for_locks(&app, 2,
        "query LIKE 'DELETE FROM auth.one_time_tokens%' OR query LIKE 'SELECT id FROM auth.users%FOR UPDATE%'").await;
    lock.commit().await.unwrap();
    assert_eq!(first.await.unwrap().status, StatusCode::OK);
    assert_eq!(second.await.unwrap().status, StatusCode::OK);
    let active: i64 = app
        .admin_client
        .query_one(
            "SELECT count(*) FROM auth.one_time_tokens WHERE kind='recovery' AND used_at IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(active, 1);
    app.outbox.wait_for(1).await;
    assert_eq!(app.outbox.sent().len(), 1);
}

async fn wait_for_locks(app: &TestApp, count: i64, predicate: &str) {
    let query = format!(
        "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock' AND ({predicate})"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if app
            .admin_client
            .query_one(&query, &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
            == count
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "operations never reached the synchronization point"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn service() -> Claims {
    Claims::from_payload(json!({"role":"service_role"})).unwrap()
}

async fn quota_app() -> TestApp {
    let app = TestApp::spawn_with(Options {
        storage: nelcota_storage::StorageSettings {
            max_total_size: Some(10),
            ..Options::default().storage
        },
        ..Options::default()
    })
    .await;
    app.admin_client
        .batch_execute("INSERT INTO storage.buckets (id) VALUES ('docs')")
        .await
        .unwrap();
    app
}

async fn six_bytes(
    storage: &StorageState,
    name: &str,
    declared: bool,
    replace: bool,
) -> Result<nelcota_storage::UploadOutcome, nelcota_core::ApiError> {
    storage
        .upload(
            service(),
            "docs",
            name,
            UploadOptions {
                content_length: declared.then_some(6),
                replace,
                ..Default::default()
            },
            Box::pin(stream::once(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(Bytes::from_static(b"abcdef"))
            })),
        )
        .await
}

#[tokio::test]
async fn concurrent_uploads_reserve_capacity_with_and_without_a_length() {
    let app = quota_app().await;
    for declared in [true, false] {
        let (first, second) = tokio::join!(
            six_bytes(&app.storage, "a.txt", declared, false),
            six_bytes(&app.storage, "b.txt", declared, false),
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let used: i64 = app
            .admin_client
            .query_one("SELECT sum(size)::bigint FROM storage.objects", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(used, 6);
        let name = if first.is_ok() { "a.txt" } else { "b.txt" };
        app.storage.delete(&service(), "docs", name).await.unwrap();
    }
}

#[tokio::test]
async fn separate_instances_check_the_final_committed_quota() {
    let app = quota_app().await;
    let second = StorageState::new(
        app.pool.clone(),
        app.keys.clone(),
        app.storage.store.clone(),
        (*app.storage.settings).clone(),
    );
    let (first, second) = tokio::join!(
        six_bytes(&app.storage, "a.txt", true, false),
        six_bytes(&second, "b.txt", true, false),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let used: i64 = app
        .admin_client
        .query_one("SELECT sum(size)::bigint FROM storage.objects", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(used, 6);
    use futures_util::StreamExt;
    assert_eq!(
        app.storage.store.list().count().await,
        1,
        "failed publication cleans its bytes"
    );
}

#[tokio::test]
async fn replacements_credit_existing_bytes_and_cancellation_releases_reservations() {
    let app = Arc::new(quota_app().await);
    six_bytes(&app.storage, "a.txt", true, false).await.unwrap();
    // Fill the quota exactly, then replace without growing it.
    app.storage
        .upload(
            service(),
            "docs",
            "b.txt",
            UploadOptions::default(),
            Box::pin(stream::iter([Ok(Bytes::from_static(b"1234"))])),
        )
        .await
        .unwrap();
    assert!(
        six_bytes(&app.storage, "a.txt", true, true)
            .await
            .unwrap()
            .replaced
    );
    assert!(
        six_bytes(&app.storage, "a.txt", false, true)
            .await
            .unwrap()
            .replaced
    );
    app.storage
        .delete(&service(), "docs", "a.txt")
        .await
        .unwrap();
    app.storage
        .delete(&service(), "docs", "b.txt")
        .await
        .unwrap();
    let (started, running) = tokio::sync::oneshot::channel();
    let uploading = app.clone();
    let pending = tokio::spawn(async move {
        uploading
            .storage
            .upload(
                service(),
                "docs",
                "cancelled.txt",
                UploadOptions::default(),
                Box::pin(stream::once(async move {
                    started.send(()).unwrap();
                    std::future::pending::<Result<Bytes, std::io::Error>>().await
                })),
            )
            .await
    });
    running.await.unwrap();
    pending.abort();
    let _ = pending.await;
    six_bytes(&app.storage, "after.txt", true, false)
        .await
        .unwrap();
}

#[tokio::test]
async fn folders_and_files_advance_independently_under_the_same_offset() {
    let app = TestApp::spawn().await;
    app.admin_client
        .batch_execute(
            "INSERT INTO storage.buckets (id) VALUES ('docs');
         INSERT INTO storage.objects (bucket_id,name,version,size,mime_type,etag)
         SELECT 'docs', name, gen_random_uuid(), 1, 'text/plain', 'x'
         FROM unnest(ARRAY['a/file.txt','b/file.txt','c/file.txt','x.txt','y.txt','z.txt']) name;",
        )
        .await
        .unwrap();
    let first = app
        .storage
        .list(&service(), "docs", "", Some(2), 0)
        .await
        .unwrap();
    let next = app
        .storage
        .list(&service(), "docs", "", Some(2), 2)
        .await
        .unwrap();
    assert_eq!(first.folders, ["a", "b"]);
    assert_eq!(next.folders, ["c"]);
    assert_eq!(next.objects[0].name, "z.txt");
}
