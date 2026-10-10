use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use nelcota_client::{
    Client, RequestOptions,
    auth::{AuthEventKind, OAuthProvider, Session},
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn session(token: &str, refresh: &str) -> Session {
    serde_json::from_value(json!({"access_token":token, "refresh_token":refresh, "token_type":"bearer",
        "expires_in":900, "expires_at":4_000_000_000_u64, "user":{"id":"u1","email":"a@example.com",
        "email_confirmed_at":null,"user_metadata":{},"created_at":"2026-10-09","last_sign_in_at":null}})).unwrap()
}
async fn server(fail: bool) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let count = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .route(
            "/auth/v1/token",
            post(
                move |State(count): State<Arc<AtomicUsize>>, Json(body): Json<Value>| async move {
                    assert_eq!(body["refresh_token"], "r0");
                    count.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if fail {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"code":"invalid_grant","message":"spent"})),
                        )
                    } else {
                        (
                            StatusCode::OK,
                            Json(serde_json::to_value(session("a1", "r1")).unwrap()),
                        )
                    }
                },
            ),
        )
        .route(
            "/auth/v1/logout",
            post(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        )
        .with_state(count.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, count, task)
}

#[tokio::test]
async fn concurrent_clones_refresh_once_and_logout_cannot_restore_session() {
    let (url, count, server) = server(false).await;
    let client = Client::builder(url).build().unwrap();
    client
        .auth()
        .set_session(session("a0", "r0"))
        .await
        .unwrap();
    let mut events = client.auth().subscribe();
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let auth = client.clone().auth();
        tasks.push(tokio::spawn(async move {
            auth.refresh_session().await.unwrap()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().refresh_token, "r1");
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(events.recv().await.unwrap().kind, AuthEventKind::Refreshed);
    assert_eq!(
        client.auth().sign_out().await.unwrap_err().status(),
        Some(503)
    );
    assert!(client.auth().get_session().await.unwrap().is_none());
    assert_eq!(events.recv().await.unwrap().kind, AuthEventKind::SignedOut);
    server.abort();
}

#[tokio::test]
async fn rotation_finishes_when_waiter_is_cancelled_and_failed_rotation_is_shared() {
    let (url, count, server_task) = server(false).await;
    let client = Client::builder(url).build().unwrap();
    client
        .auth()
        .set_session(session("a0", "r0"))
        .await
        .unwrap();
    let token = nelcota_client::CancellationToken::new();
    let auth = client
        .with_options(RequestOptions {
            timeout: None,
            cancellation: Some(token.clone()),
        })
        .auth();
    let task = tokio::spawn(async move { auth.refresh_session().await });
    while count.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    token.cancel();
    assert_eq!(task.await.unwrap().unwrap_err().code(), "aborted");
    assert_eq!(
        client
            .auth()
            .get_session()
            .await
            .unwrap()
            .unwrap()
            .refresh_token,
        "r1"
    );
    server_task.abort();

    let (url, count, server_task) = server(true).await;
    let client = Client::builder(url).build().unwrap();
    let mut expired = session("a0", "r0");
    expired.expires_at = 1;
    client.auth().set_session(expired).await.unwrap();
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let auth = client.auth();
        tasks.push(tokio::spawn(async move { auth.refresh_session().await }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap_err().code(), "invalid_grant");
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(client.auth().get_session().await.unwrap().is_none());
    server_task.abort();
}

#[tokio::test]
async fn logout_serializes_with_rotation_and_sessions_are_redacted() {
    let (url, count, server_task) = server(false).await;
    let client = Client::builder(url).build().unwrap();
    let initial = session("secret-access", "r0");
    assert!(!format!("{initial:?}").contains("secret-access"));
    client.auth().set_session(initial).await.unwrap();
    let auth = client.auth();
    let rotating = tokio::spawn(async move { auth.refresh_session().await });
    while count.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    assert!(client.auth().sign_out().await.is_err());
    rotating.await.unwrap().unwrap();
    assert!(client.auth().get_session().await.unwrap().is_none());
    server_task.abort();
}

#[test]
fn oauth_and_email_links_keep_secrets_out_of_debug_and_clean_callback_urls() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};
    let client = Client::builder("https://api.example.com").build().unwrap();
    let flow = client
        .auth()
        .begin_oauth(OAuthProvider::Github, "https://app.example.com/callback")
        .unwrap();
    assert_eq!(flow.verifier.len(), 43);
    let params: std::collections::BTreeMap<_, _> = flow.url.query_pairs().into_owned().collect();
    assert_eq!(params.len(), 4);
    assert_eq!(params["code_challenge_method"], "S256");
    assert!(!flow.url.as_str().contains(&flow.verifier));
    assert_eq!(
        params["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(flow.verifier.as_bytes()))
    );
    assert!(!format!("{flow:?}").contains(&flow.verifier));
    let mut url =
        url::Url::parse("https://app.example.com/callback#type=signup&token=secret-email").unwrap();
    let link = nelcota_client::auth::EmailLink::parse(&mut url).unwrap();
    assert!(url.fragment().is_none());
    assert_eq!(link.token, "secret-email");
    assert!(!format!("{link:?}").contains("secret-email"));
}

#[tokio::test]
async fn custom_storage_restores_sessions_and_background_refresh_stops_on_drop() {
    use nelcota_client::auth::{MemoryStorage, SessionStorage};
    let (url, count, server_task) = server(false).await;
    let storage = Arc::new(MemoryStorage::default());
    let mut expired = session("a0", "r0");
    expired.expires_at = 1;
    storage.save(Some(&expired)).await.unwrap();
    let client = Client::builder(url)
        .session_storage(storage.clone())
        .build()
        .unwrap();
    let mut events = client.auth().subscribe();
    let background = client.auth().start_auto_refresh();
    let event = tokio::time::timeout(Duration::from_secs(3), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.kind, AuthEventKind::Refreshed);
    assert_eq!(storage.load().await.unwrap().unwrap().refresh_token, "r1");
    drop(background);
    client.auth().set_session(expired).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server_task.abort();
}

#[tokio::test]
async fn file_storage_round_trips_atomically_and_privately() {
    use nelcota_client::auth::{FileStorage, SessionStorage};
    let dir = std::env::temp_dir().join(format!(
        "nelcota-sdk-file-storage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = FileStorage::new(dir.join("nested").join("session.json"));
    assert!(store.load().await.unwrap().is_none());
    store.save(Some(&session("a0", "r0"))).await.unwrap();
    store.save(Some(&session("a1", "r1"))).await.unwrap();
    assert_eq!(store.load().await.unwrap(), Some(session("a1", "r1")));
    let entries: Vec<_> = std::fs::read_dir(dir.join("nested"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries, vec![std::ffi::OsString::from("session.json")]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    store.save(None).await.unwrap();
    store.save(None).await.unwrap();
    assert!(store.load().await.unwrap().is_none());
    std::fs::write(store.path(), "not json").unwrap();
    assert_eq!(
        store.load().await.unwrap_err().code(),
        "session_storage_error"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
