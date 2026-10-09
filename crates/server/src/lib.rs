//! Server assembly: shared state, routes and middleware.
//! It lives in a lib (not only in `main`) so the integration tests use the
//! same `Router` as production.

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{FromRef, State},
    http::{HeaderName, Method, StatusCode, header},
    response::IntoResponse,
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

    let mut router = Router::new()
        .route("/health", get(health))
        .merge(nelcota_api::router())
        .with_state(state)
        .merge(nelcota_auth::router(auth));
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
    router
        .layer(cors)
        // The default span records method and URI, never headers (Authorization).
        .layer(TraceLayer::new_for_http())
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
