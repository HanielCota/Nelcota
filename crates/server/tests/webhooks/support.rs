use crate::common::TestApp;
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use nelcota_core::db;
use nelcota_webhooks::{
    Worker,
    destination::Policy,
    management,
    models::{EndpointInput, NewSecret},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, task::JoinHandle};

pub fn input(url: &str) -> EndpointInput {
    EndpointInput {
        name: "Orders".into(),
        table: "todos".into(),
        url: url.into(),
        events: vec!["INSERT".into(), "UPDATE".into(), "DELETE".into()],
        enabled: true,
        max_attempts: 6,
    }
}
pub fn body(url: &str) -> Value {
    json!({ "name":"Orders","table":"todos","url":url,"events":["INSERT","UPDATE","DELETE"],"enabled":true,"max_attempts":6 })
}
pub async fn hook(app: &TestApp, url: &str, max_attempts: i32) -> NewSecret {
    let mut value = input(url);
    value.max_attempts = max_attempts;
    management::create(
        &db::admin_pool(&app.admin, 2),
        "public",
        value,
        Policy {
            allow_loopback: true,
        },
    )
    .await
    .unwrap()
}
pub async fn insert(app: &TestApp, title: &str) {
    app.admin_client
        .execute(
            "INSERT INTO public.todos (user_id,title) VALUES ($1,$2)",
            &[&app.user_a, &title],
        )
        .await
        .unwrap();
}
pub async fn latest(app: &TestApp) -> String {
    app.admin_client
        .query_one(
            "SELECT id::text FROM nelcota.webhook_deliveries ORDER BY id DESC LIMIT 1",
            &[],
        )
        .await
        .unwrap()
        .get(0)
}
pub async fn count(app: &TestApp) -> i64 {
    app.admin_client
        .query_one("SELECT count(*) FROM nelcota.webhook_deliveries", &[])
        .await
        .unwrap()
        .get(0)
}
pub fn worker(app: &TestApp) -> Worker {
    Worker::new(
        db::admin_pool(&app.admin, 2),
        Policy {
            allow_loopback: true,
        },
    )
    .unwrap()
}
pub async fn make_due(app: &TestApp) {
    app.admin_client.execute("UPDATE nelcota.webhook_deliveries SET next_attempt_at=clock_timestamp()-interval '1 second' WHERE status='pending'", &[]).await.unwrap();
}

#[derive(Clone)]
pub struct Received {
    pub headers: HeaderMap,
    pub body: String,
}
#[derive(Clone)]
struct ReceiverState {
    requests: Arc<Mutex<Vec<Received>>>,
    statuses: Arc<Mutex<VecDeque<u16>>>,
    hold_first: Option<Arc<Notify>>,
    retry_after: Option<&'static str>,
}
pub struct Receiver {
    pub url: String,
    state: ReceiverState,
    task: JoinHandle<()>,
}
impl Receiver {
    pub async fn new(statuses: impl IntoIterator<Item = u16>) -> Self {
        Self::with_options(statuses, None, None).await
    }
    pub async fn with_options(
        statuses: impl IntoIterator<Item = u16>,
        hold_first: Option<Arc<Notify>>,
        retry_after: Option<&'static str>,
    ) -> Self {
        let state = ReceiverState {
            requests: Arc::default(),
            statuses: Arc::new(Mutex::new(statuses.into_iter().collect())),
            hold_first,
            retry_after,
        };
        async fn receive(
            State(state): State<ReceiverState>,
            headers: HeaderMap,
            body: Bytes,
        ) -> impl IntoResponse {
            let first = {
                let mut requests = state.requests.lock().unwrap();
                let first = requests.is_empty();
                requests.push(Received {
                    headers,
                    body: String::from_utf8(body.to_vec()).unwrap(),
                });
                first
            };
            let status = state.statuses.lock().unwrap().pop_front().unwrap_or(204);
            if first && let Some(hold) = state.hold_first {
                hold.notified().await;
            }
            let mut response = (
                StatusCode::from_u16(status).unwrap(),
                "receiver body must not be kept: a-sensitive-value",
            )
                .into_response();
            response
                .headers_mut()
                .insert("location", "/redirect".parse().unwrap());
            if let Some(delay) = state.retry_after {
                response
                    .headers_mut()
                    .insert("retry-after", delay.parse().unwrap());
            }
            response
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let router = Router::new()
            .route("/hook", post(receive))
            .route("/redirect", post(receive))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { url, state, task }
    }
    pub fn requests(&self) -> Vec<Received> {
        self.state.requests.lock().unwrap().clone()
    }
    pub async fn wait_for(&self, count: usize) {
        for _ in 0..200 {
            if self.requests().len() >= count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("expected {count} requests, got {}", self.requests().len());
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}
