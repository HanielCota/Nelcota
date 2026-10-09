//! Server assembly: shared state, routes and middleware.
//! It lives in a lib (not only in `main`) so the integration tests use the
//! same `Router` as production.

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{FromRef, Request, State},
    http::{HeaderName, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use deadpool_postgres::Pool;
use nelcota_api::{ApiSettings, Catalog, CatalogHandle};
use nelcota_auth::{AuthState, SharedVerifier};
use serde_json::json;
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

#[derive(Clone, FromRef)]
pub struct AppState {
    pub pool: Pool,
    pub verifier: SharedVerifier,
    pub catalog: Arc<CatalogHandle>,
    pub api: Arc<ApiSettings>,
}

/// Runs the first introspection of the exposed schema.
pub async fn load_catalog(pool: &Pool, schema: &str) -> anyhow::Result<Arc<CatalogHandle>> {
    let client = pool.get().await?;
    let catalog = Catalog::load(&**client, schema).await?;
    tracing::info!(
        schema,
        tables = catalog.tables.len(),
        functions = catalog.functions.len(),
        "catalog loaded"
    );
    Ok(Arc::new(CatalogHandle::new(catalog)))
}

pub fn app(
    state: AppState,
    auth: AuthState,
    admin: Option<nelcota_admin::AdminState>,
    storage: Option<nelcota_storage::StorageState>,
    request_timeout: Duration,
) -> Router {
    // The API is called straight from the browser, from any origin; the
    // protection is JWT + RLS, not CORS. Cookies are not used (no credentials).
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::RANGE,
            header::IF_NONE_MATCH,
            header::IF_RANGE,
            HeaderName::from_static("prefer"),
        ])
        // What a browser client needs to read: paging totals, file metadata,
        // the applied preferences and how long to wait after a 429.
        .expose_headers([
            header::CONTENT_RANGE,
            header::CONTENT_DISPOSITION,
            header::ETAG,
            header::LAST_MODIFIED,
            header::RETRY_AFTER,
            HeaderName::from_static("preference-applied"),
        ]);

    let verifier = state.verifier.clone();
    let mut router = Router::new()
        .route("/health", get(health))
        .merge(nelcota_api::router())
        .with_state(state)
        .merge(nelcota_auth::router(auth));
    let observed = admin.as_ref().map(|admin| Observed {
        denied: admin.denied.clone(),
        metrics: admin.metrics.clone(),
    });
    if let Some(admin) = admin.clone() {
        router = router.merge(nelcota_admin::router(admin));
    }
    router = router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(CompressionLayer::new());
    // Storage stays outside the timeout (uploads take as long as they take,
    // with their own cap) and gzip (files are served byte for byte, by range).
    if let Some(storage) = storage {
        router = router.merge(nelcota_storage::router(storage));
        if let Some(admin) = admin {
            router = router.merge(nelcota_admin::upload_router(admin));
        }
    }
    // API traffic feeds the panel's overview charts and its "Recently
    // blocked" list.
    if let Some(observed) = observed {
        router = router.layer(middleware::from_fn_with_state(
            (observed, verifier),
            observe,
        ));
    }
    router
        .layer(cors)
        // The default span records method and URI, never headers (Authorization).
        .layer(TraceLayer::new_for_http())
}

/// Where API traffic is recorded for the panel.
#[derive(Clone)]
struct Observed {
    denied: Arc<nelcota_admin::DeniedLog>,
    metrics: Arc<nelcota_admin::Metrics>,
}

/// Counts every API request (not the panel's own routes) per minute, and
/// records those answered with 401, 403 or 429. The role comes from verifying
/// the token again, only for refused requests; the token itself and the
/// query string are never kept. Latency is time to the response head.
async fn observe(
    State((observed, verifier)): State<(Observed, SharedVerifier)>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if path.starts_with("/admin") || path == "/health" {
        return next.run(request).await;
    }
    let started = std::time::Instant::now();
    let method = request.method().to_string();
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).to_owned());
    let response = next.run(request).await;
    let status = response.status();
    observed.metrics.record(
        nelcota_admin::Area::of(&path),
        status.as_u16(),
        started.elapsed(),
    );
    if !matches!(status.as_u16(), 401 | 403 | 429) {
        return response;
    }
    let (role, user_id, email) = match bearer.map(|token| verifier.verify(&token)) {
        None => ("anon".to_owned(), None, None),
        Some(Ok(claims)) => (
            claims.role().as_str().to_owned(),
            claims.sub().map(|id| id.to_string()),
            claims
                .claim("email")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
        ),
        Some(Err(_)) => ("invalid_token".to_owned(), None, None),
    };
    let info = response.extensions().get::<nelcota_core::ErrorInfo>();
    observed
        .denied
        .record(nelcota_admin::contracts::DeniedRequest {
            at: nelcota_admin::now_rfc3339(),
            method,
            path,
            status: status.as_u16(),
            code: info.map_or_else(
                || format!("http_{}", status.as_u16()),
                |i| i.code.to_owned(),
            ),
            message: info.map_or_else(String::new, |i| i.message.clone()),
            role,
            user_id,
            email,
        });
    response
}

/// `GET /health`: 200 if Postgres answers, 503 otherwise.
async fn health(State(pool): State<Pool>) -> impl IntoResponse {
    let ok = match pool.get().await {
        Ok(client) => client.simple_query("SELECT 1").await.is_ok(),
        Err(_) => false,
    };
    let version = env!("CARGO_PKG_VERSION");
    if ok {
        (
            StatusCode::OK,
            Json(json!({ "status": "ok", "version": version })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable", "version": version })),
        )
    }
}
