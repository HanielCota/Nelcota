//! File storage (D77-D80): metadata in `storage.objects` under RLS, bytes on
//! disk or S3. Every read or write of a row runs as the caller's role, like
//! the REST API; the internal `nelcota_storage` role only reads bucket
//! settings, serves public and signed files, and finds orphaned bytes.

mod buckets;
mod db;
mod gc;
mod mime;
mod objects;
mod path;
mod serve;
mod store;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{DefaultBodyLimit, FromRef},
    routing::{get, post},
};
use deadpool_postgres::Pool;
use nelcota_auth::{Keys, SharedVerifier};
use nelcota_core::Config;
use tokio::sync::Semaphore;

pub use gc::{collect as collect_orphans, spawn_collector};
pub use store::{Store, StoreError};

/// Uploads streaming at once; more wait for a slot. Bounds the memory held
/// in upload parts.
const CONCURRENT_UPLOADS: usize = 32;

#[derive(Clone)]
pub struct StorageState {
    pub pool: Pool,
    pub keys: Arc<Keys>,
    pub store: Arc<Store>,
    pub settings: Arc<StorageSettings>,
    uploads: Arc<Semaphore>,
}

impl StorageState {
    pub fn new(pool: Pool, keys: Arc<Keys>, store: Arc<Store>, settings: StorageSettings) -> Self {
        StorageState {
            pool,
            keys,
            store,
            settings: Arc::new(settings),
            uploads: Arc::new(Semaphore::new(CONCURRENT_UPLOADS)),
        }
    }
}

impl FromRef<StorageState> for SharedVerifier {
    fn from_ref(state: &StorageState) -> Self {
        state.keys.clone()
    }
}

/// Server-wide limits (D80) and the origin of public URLs.
#[derive(Clone, Debug)]
pub struct StorageSettings {
    pub max_file_size: u64,
    pub max_total_size: Option<u64>,
    pub min_free_bytes: u64,
    pub public_url: Option<String>,
    pub upload_timeout: Duration,
}

impl StorageSettings {
    pub fn from_config(config: &Config) -> Self {
        StorageSettings {
            max_file_size: config.storage_max_file_size,
            max_total_size: config.storage_max_total_size,
            min_free_bytes: config.storage_min_free_bytes,
            public_url: config
                .storage_public_url
                .as_deref()
                .map(|u| u.trim().trim_end_matches('/').to_owned())
                .filter(|u| !u.is_empty()),
            upload_timeout: Duration::from_secs(config.storage_upload_timeout_secs),
        }
    }
}

/// Storage routes. They carry no request timeout or compression (D78): the
/// caller mounts them outside those layers.
pub fn router(state: StorageState) -> Router {
    // Only the routes that take a file lift the body limit; the JSON ones
    // keep axum's default.
    let uploads = Router::new()
        .route(
            "/storage/v1/object/{bucket}/{*name}",
            get(objects::download)
                .post(objects::create)
                .put(objects::upsert)
                .delete(objects::remove),
        )
        .layer(DefaultBodyLimit::disable());
    Router::new()
        .route(
            "/storage/v1/bucket",
            get(buckets::list).post(buckets::create),
        )
        .route(
            "/storage/v1/bucket/{id}",
            get(buckets::one)
                .put(buckets::update)
                .delete(buckets::remove),
        )
        .route("/storage/v1/object/list/{bucket}", post(objects::list))
        .route(
            "/storage/v1/object/public/{bucket}/{*name}",
            get(objects::public),
        )
        .route(
            "/storage/v1/object/sign/{bucket}/{*name}",
            get(objects::signed).post(objects::sign),
        )
        .merge(uploads)
        .with_state(state)
}
