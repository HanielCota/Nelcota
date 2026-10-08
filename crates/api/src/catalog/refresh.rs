//! Atomic snapshots and serialized reconciliation after schema changes.
use super::Catalog;
use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Shared catalog, swapped atomically on each reload.
pub struct CatalogHandle {
    current: RwLock<Arc<Catalog>>,
    reload_lock: tokio::sync::Mutex<()>,
    retrying: AtomicBool,
}

impl CatalogHandle {
    pub fn new(catalog: Catalog) -> Self {
        CatalogHandle {
            current: RwLock::new(Arc::new(catalog)),
            reload_lock: tokio::sync::Mutex::new(()),
            retrying: AtomicBool::new(false),
        }
    }

    pub fn get(&self) -> Arc<Catalog> {
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn replace(&self, catalog: Catalog) {
        *self.current.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(catalog);
    }

    pub async fn reload(&self, pool: &deadpool_postgres::Pool) -> Result<(), String> {
        let _guard = self.reload_lock.lock().await;
        self.reload_locked(pool).await
    }

    async fn reload_locked(&self, pool: &deadpool_postgres::Pool) -> Result<(), String> {
        let schema = self.get().schema.clone();
        let client = pool.get().await.map_err(|e| e.to_string())?;
        let catalog = Catalog::load(&**client, &schema)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(
            tables = catalog.tables.len(),
            functions = catalog.functions.len(),
            "catalog reloaded"
        );
        self.replace(catalog);
        Ok(())
    }

    /// A committed change stays successful when introspection fails. A single
    /// worker reconciles the catalog without executing the mutation again.
    pub async fn refresh(self: &Arc<Self>, pool: &deadpool_postgres::Pool) -> bool {
        let _guard = self.reload_lock.lock().await;
        if self.reload_locked(pool).await.is_ok() {
            return true;
        }
        tracing::warn!("catalog refresh pending; scheduling reconciliation");
        if !self.retrying.swap(true, Ordering::AcqRel) {
            let handle = self.clone();
            let pool = pool.clone();
            tokio::spawn(async move {
                let mut delay = Duration::from_secs(1);
                loop {
                    tokio::time::sleep(delay).await;
                    // Reset the worker flag under the reload lock. A concurrent
                    // failed refresh must not lose its request to start a worker.
                    let _guard = handle.reload_lock.lock().await;
                    match handle.reload_locked(&pool).await {
                        Ok(()) => {
                            handle.retrying.store(false, Ordering::Release);
                            break;
                        }
                        Err(error) => {
                            tracing::warn!(%error, "catalog reconciliation failed; retrying")
                        }
                    }
                    delay = (delay * 2).min(Duration::from_secs(30));
                }
            });
        }
        false
    }
}
