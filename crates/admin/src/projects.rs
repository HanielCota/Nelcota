//! Host projects: the public list (for the panel's switcher) and each app's
//! status.
//!
//! The list comes from `shared/projects.json`, written by the CLI and mounted
//! read-only into the apps. It holds only names and URLs, never secrets, and
//! is re-read on every request: new projects show up without restarting
//! anything.

use std::{path::PathBuf, time::Duration};

use axum::{
    Json,
    extract::State,
    http::{StatusCode, Uri},
};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::{AdminState, ApiError, sso::Sso};

/// How this app fits into the host.
pub struct HostLink {
    /// This project's name.
    pub project: String,
    /// `shared/projects.json` (absent outside a host, e.g. `nelcota dev`).
    pub registry: Option<PathBuf>,
    /// Single sign-on (handoff) enabled.
    pub sso: Option<Sso>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistryProject {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub panel_login: String,
    #[serde(default)]
    pub projects: Vec<RegistryProject>,
}

fn read_registry(host: &HostLink) -> Registry {
    host.registry
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Project by name, from the host's list.
pub fn find(host: &HostLink, name: &str) -> Option<RegistryProject> {
    read_registry(host)
        .projects
        .into_iter()
        .find(|p| p.name == name)
}

/// Valid project names (the same format the CLI accepts); used to build the
/// internal address `app-<name>`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `GET /admin/api/projects`
pub async fn list(State(state): State<AdminState>) -> Json<Value> {
    let registry = read_registry(&state.host);
    let mut projects: Vec<Value> = registry
        .projects
        .iter()
        .map(|p| json!({ "name": p.name, "url": p.url, "current": p.name == state.host.project }))
        .collect();
    if projects.is_empty() {
        projects.push(json!({ "name": state.host.project, "url": null, "current": true }));
    }
    Json(json!({
        "current": state.host.project,
        "sso": state.host.sso.is_some(),
        "projects": projects,
    }))
}

/// Calls `GET /health` on a project's app over the host's internal network.
async fn probe(name: &str) -> Value {
    probe_url(&format!("http://app-{name}:8000/health")).await
}

/// Healthy = `200` within 2 s. The version comes from the JSON body when the
/// app answers at all, so an unhealthy app still shows which version it runs.
async fn probe_url(url: &str) -> Value {
    let down = json!({ "healthy": false, "version": null, "latency_ms": null });
    let Ok(uri) = url.parse::<Uri>() else {
        return down;
    };
    let started = std::time::Instant::now();
    let client = Client::builder(TokioExecutor::new()).build_http::<Empty<Bytes>>();
    let response = timeout(Duration::from_secs(2), async {
        let response = client.get(uri).await.ok()?;
        let status = response.status();
        let body = response.into_body().collect().await.ok()?.to_bytes();
        Some((status, body))
    })
    .await;
    let Ok(Some((status, body))) = response else {
        return down;
    };
    let version = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("version").cloned())
        .unwrap_or(Value::Null);
    json!({
        "healthy": status == StatusCode::OK,
        "version": version,
        "latency_ms": started.elapsed().as_millis() as u64,
    })
}

/// `GET /admin/api/projects/status`: each project's status (in parallel).
pub async fn status(State(state): State<AdminState>) -> Result<Json<Value>, ApiError> {
    let registry = read_registry(&state.host);
    let probes = registry.projects.iter().map(|p| {
        let name = p.name.clone();
        async move {
            if valid_name(&name) {
                probe(&name).await
            } else {
                json!({ "healthy": false, "version": null, "latency_ms": null })
            }
        }
    });
    let results = futures_util::future::join_all(probes).await;
    let projects: Vec<Value> = registry
        .projects
        .iter()
        .zip(results)
        .map(|(p, mut status)| {
            status["name"] = json!(p.name);
            status["url"] = json!(p.url);
            status["current"] = json!(p.name == state.host.project);
            status
        })
        .collect();
    Ok(Json(
        json!({ "current": state.host.project, "projects": projects }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serves `app` on a free local port and returns its base URL.
    async fn serve(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn healthy_app_reports_its_version_even_with_a_chunked_body() {
        // A streamed body goes out chunked: the old hand-written parser read the
        // chunk sizes as part of the JSON and lost the version.
        let app = axum::Router::new().route(
            "/health",
            axum::routing::get(|| async {
                let chunks = futures_util::stream::iter([
                    Ok::<_, std::convert::Infallible>(Bytes::from_static(b"{\"status\":\"ok\",")),
                    Ok(Bytes::from_static(b"\"version\":\"1.2.3\"}")),
                ]);
                axum::body::Body::from_stream(chunks)
            }),
        );
        let status = probe_url(&format!("{}/health", serve(app).await)).await;
        assert_eq!(status["healthy"], true);
        assert_eq!(status["version"], "1.2.3");
        assert!(status["latency_ms"].is_u64());
    }

    #[tokio::test]
    async fn an_error_status_is_unhealthy_but_still_answers() {
        let app = axum::Router::new().route(
            "/health",
            axum::routing::get(|| async {
                (StatusCode::SERVICE_UNAVAILABLE, r#"{"version":"1.2.3"}"#)
            }),
        );
        let status = probe_url(&format!("{}/health", serve(app).await)).await;
        assert_eq!(status["healthy"], false);
        assert_eq!(status["version"], "1.2.3");
        assert!(status["latency_ms"].is_u64());
    }

    #[tokio::test]
    async fn unreachable_or_slow_apps_are_down() {
        // Nothing listens on this port.
        let closed = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap()
        };
        let status = probe_url(&format!("http://{closed}/health")).await;
        assert_eq!(
            status,
            json!({ "healthy": false, "version": null, "latency_ms": null })
        );

        let slow = axum::Router::new().route(
            "/health",
            axum::routing::get(|| async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                "late"
            }),
        );
        let started = std::time::Instant::now();
        let status = probe_url(&format!("{}/health", serve(slow).await)).await;
        assert_eq!(status["healthy"], false);
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "gave up after the 2 s timeout"
        );
    }

    #[test]
    fn only_safe_names_become_internal_addresses() {
        assert!(valid_name("shop"));
        assert!(valid_name("blog-2"));
        assert!(!valid_name("Shop"));
        assert!(!valid_name("a.b"));
        assert!(!valid_name("x:80"));
        assert!(!valid_name(""));
    }
}
