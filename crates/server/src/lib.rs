//! Montagem do servidor: estado compartilhado, rotas e middlewares.
//! Fica numa lib (e não só no `main`) para os testes de integração usarem o
//! mesmo `Router` de produção.

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

/// Faz a primeira introspecção do schema exposto.
pub async fn load_catalog(pool: &Pool, schema: &str) -> anyhow::Result<Arc<CatalogHandle>> {
    let client = pool.get().await?;
    let catalog = Catalog::load(&**client, schema).await?;
    tracing::info!(
        schema,
        tabelas = catalog.tables.len(),
        funcoes = catalog.functions.len(),
        "catálogo carregado"
    );
    Ok(Arc::new(CatalogHandle::new(catalog)))
}

pub fn app(
    state: AppState,
    auth: AuthState,
    admin: Option<nelcota_admin::AdminState>,
    request_timeout: Duration,
) -> Router {
    // A API é chamada direto do navegador, de qualquer origem; a proteção é
    // o JWT + RLS, não o CORS. Cookies não são usados (nada de credentials).
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static("prefer"),
        ]);

    let mut router = Router::new()
        .route("/health", get(health))
        .merge(nelcota_api::router())
        .with_state(state)
        .merge(nelcota_auth::router(auth));
    if let Some(admin) = admin {
        router = router.merge(nelcota_admin::router(admin));
    }
    router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(cors)
        .layer(CompressionLayer::new())
        // O span padrão registra método e URI, nunca headers (Authorization).
        .layer(TraceLayer::new_for_http())
}

/// `GET /health`: 200 se o Postgres responde, 503 caso contrário.
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
