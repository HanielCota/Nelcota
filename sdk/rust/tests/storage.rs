use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::header,
    response::Response,
    routing::any,
};
use nelcota_client::{Client, storage::OpenOptions};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    )
}

#[tokio::test]
async fn storage_redirect_does_not_leak_bearer_token_to_another_origin() {
    let seen = Arc::new(tokio::sync::Mutex::new(None));
    let destination = Router::new().fallback(any(|State(seen): State<Arc<tokio::sync::Mutex<Option<bool>>>>, request: Request| async move {
        *seen.lock().await = Some(request.headers().contains_key(header::AUTHORIZATION));
        Response::builder().status(206).body(Body::from("file bytes")).unwrap()
    })).with_state(seen.clone());
    let (target, task2) = serve(destination).await;
    let origin = Router::new().fallback(any(move |request: Request| {
        let target = target.clone();
        async move {
            assert_eq!(
                request.headers()[header::AUTHORIZATION],
                "Bearer private-token"
            );
            Response::builder()
                .status(307)
                .header(
                    header::LOCATION,
                    format!("{target}/signed-file?token=secret"),
                )
                .body(Body::empty())
                .unwrap()
        }
    }));
    let (url, task1) = serve(origin).await;
    let client = Client::builder(url)
        .access_token("private-token")
        .build()
        .unwrap();
    let response = client
        .storage()
        .from("files")
        .unwrap()
        .open("hello.txt", OpenOptions::default())
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.text().await.unwrap(), "file bytes");
    assert_eq!(*seen.lock().await, Some(false));
    task1.abort();
    task2.abort();
}

#[tokio::test]
async fn interrupted_buffered_reads_retry_but_write_responses_do_not() {
    let seen = Arc::new(AtomicUsize::new(0));
    let router = Router::new()
        .fallback(any(
            |State(seen): State<Arc<AtomicUsize>>, request: Request| async move {
                let attempt = seen.fetch_add(1, Ordering::SeqCst);
                if request.method() == "GET" && attempt == 1 {
                    return Response::new(Body::from("[]"));
                }
                let body = Body::from_stream(futures_util::stream::iter([
                    Ok(bytes::Bytes::from_static(b"[")),
                    Err(std::io::Error::other("interrupted response")),
                ]));
                Response::new(body)
            },
        ))
        .with_state(seen.clone());
    let (url, task) = serve(router).await;
    let client = Client::builder(url).build().unwrap();
    client.from("notes").execute::<Vec<Value>>().await.unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 2);
    let error = client
        .from("notes")
        .insert(&json!({"body":"only once"}))
        .select("id")
        .execute::<Vec<Value>>()
        .await
        .unwrap_err();
    assert_eq!(error.code(), "network_error");
    assert_eq!(seen.load(Ordering::SeqCst), 3);
    task.abort();
}
