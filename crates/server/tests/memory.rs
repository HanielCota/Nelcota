//! Background resource ownership with real PostgreSQL connections.
mod common;
use common::*;
use nelcota_api::{CatalogHandle, spawn_reload_listener};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn stopping_listeners_releases_connections_even_during_blocked_refresh() {
    let app = TestApp::spawn().await;
    let pool = deadpool_postgres::Pool::builder(deadpool_postgres::Manager::new(
        app.admin.clone(),
        tokio_postgres::NoTls,
    ))
    .max_size(1)
    .runtime(deadpool_postgres::Runtime::Tokio1)
    .build()
    .unwrap();
    // Introspection cannot acquire this pool. Shutdown must cancel that wait
    // and the separately spawned PostgreSQL connection driver.
    let held = pool.get().await.unwrap();
    let name = format!("nelcota-memory-{}", uuid::Uuid::now_v7());
    let mut config = app.admin.clone();
    config.application_name(&name);
    let connected = || async {
        app.admin_client
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE application_name=$1",
                &[&name],
            )
            .await
            .unwrap()
            .get::<_, i64>(0)
    };
    for _ in 0..8 {
        let catalog = Arc::new(CatalogHandle::new((*app.catalog.get()).clone()));
        let weak = Arc::downgrade(&catalog);
        let listener = spawn_reload_listener(catalog, pool.clone(), config.clone());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while connected().await != 1 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "listener did not connect"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        listener.shutdown().await;
        assert!(weak.upgrade().is_none(), "listener retained its catalog");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while connected().await != 0 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "listener left a PostgreSQL driver alive"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    drop(held);
}
