//! Automatic REST API over the Postgres catalog, always executed under RLS.
//!
//! - `GET    /rest/v1/`                 OpenAPI (filtered by role)
//! - `GET    /rest/v1/{table}`          read with filters, ordering and paging
//! - `POST   /rest/v1/{table}`          insert (object or array); upsert with
//!   `Prefer: resolution=merge-duplicates|ignore-duplicates` (+ `on_conflict`)
//! - `PATCH  /rest/v1/{table}?filters`  update
//! - `DELETE /rest/v1/{table}?filters`  delete
//! - `POST   /rest/v1/rpc/{function}`   SQL function call
//!
//! Each request runs in a transaction with the JWT role and claims
//! (`nelcota_core::db::begin_request`). The API does not decide permissions:
//! Postgres does (GRANTs + RLS).

pub mod catalog;
pub mod openapi;
pub mod query;
pub mod typescript;

mod http;
mod operations;
use axum::{
    Router,
    extract::FromRef,
    routing::{get, post},
};
pub use catalog::{Catalog, CatalogHandle, spawn_reload_listener};
use deadpool_postgres::Pool;
use nelcota_auth::SharedVerifier;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct ApiSettings {
    /// Row cap per read (`NELCOTA_MAX_ROWS`); `None` = no cap.
    pub max_rows: Option<i64>,
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Pool: FromRef<S>,
    SharedVerifier: FromRef<S>,
    Arc<CatalogHandle>: FromRef<S>,
    Arc<ApiSettings>: FromRef<S>,
{
    Router::new()
        .route("/rest/v1", get(http::openapi))
        .route("/rest/v1/", get(http::openapi))
        .route(
            "/rest/v1/{table}",
            get(http::read)
                .post(http::create)
                .patch(http::update)
                .delete(http::remove),
        )
        .route("/rest/v1/rpc/{function}", post(http::rpc))
}
