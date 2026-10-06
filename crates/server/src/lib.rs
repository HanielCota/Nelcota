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
use nelcota_auth::{AuthState, SharedVerifier};
use serde_json::json;
use tower_http::{
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

#[derive(Clone, FromRef)]
pub struct AppState {
    pub pool: Pool,
    pub verifier: SharedVerifier,
}

pub fn app(state: AppState, auth: AuthState, request_timeout: Duration) -> Router {
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

    Router::new()
        .route("/health", get(health))
        .merge(nelcota_api::router())
        .with_state(state)
        .merge(nelcota_auth::router(auth))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(cors)
        // O span padrão registra método e URI, nunca headers (Authorization).
        .layer(TraceLayer::new_for_http())
}

/// `GET /health`: 200 se o Postgres responde, 503 caso contrário.
async fn health(State(pool): State<Pool>) -> impl IntoResponse {
    let ok = match pool.get().await {
        Ok(client) => client.simple_query("SELECT 1").await.is_ok(),
        Err(_) => false,
    };
    if ok {
        (StatusCode::OK, Json(json!({ "status": "ok" })))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "unavailable" })),
        )
    }
}
