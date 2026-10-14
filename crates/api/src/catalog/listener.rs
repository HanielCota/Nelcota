//! Reconnecting PostgreSQL notification listener.
use super::{CatalogHandle, RELOAD_CHANNEL};
use futures_util::{StreamExt, stream};
use std::{sync::Arc, time::Duration};
use tokio_postgres::{AsyncMessage, NoTls};

/// Owns the listener and its reconnect loop. Dropping it stops both.
#[must_use = "keep the listener alive until the host shuts down"]
pub struct ReloadListener(tokio::task::JoinHandle<()>);

impl ReloadListener {
    pub async fn shutdown(mut self) {
        self.0.abort();
        let _ = (&mut self.0).await;
    }
}

impl Drop for ReloadListener {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct Driver(tokio::task::JoinHandle<Result<(), tokio_postgres::Error>>);

impl Drop for Driver {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Keeps a `LISTEN nelcota` open and reloads the catalog on each notification.
/// Reconnects on its own and reloads on each reconnection (notices may have
/// been missed).
/// Keep the returned owner until shutdown; dropping it cancels the listener.
pub fn spawn_reload_listener(
    handle: Arc<CatalogHandle>,
    pool: deadpool_postgres::Pool,
    config: tokio_postgres::Config,
) -> ReloadListener {
    ReloadListener(tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            match listen_once(&handle, &pool, &config).await {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(err) => {
                    tracing::warn!(error = %err, "catalog reload listener dropped; reconnecting");
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }))
}

async fn listen_once(
    handle: &Arc<CatalogHandle>,
    pool: &deadpool_postgres::Pool,
    config: &tokio_postgres::Config,
) -> Result<(), tokio_postgres::Error> {
    let (client, mut connection) = config.connect(NoTls).await?;
    // A notification means "reload the current catalog", not a separate job.
    // Keep one pending reload while introspection is slow. Never wait on the
    // sender: this task also drives replies to LISTEN on the same connection.
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let mut driver = Driver(tokio::spawn(async move {
        let mut messages = stream::poll_fn(move |cx| connection.poll_message(cx));
        while let Some(message) = messages.next().await {
            match message {
                Ok(AsyncMessage::Notification(_)) => {
                    let _ = tx.try_send(());
                }
                Ok(_) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }));
    client
        .batch_execute(&format!("LISTEN {RELOAD_CHANNEL}"))
        .await?;
    handle.refresh(pool).await;
    while rx.recv().await.is_some() {
        // Groups bursts of DDL (e.g. a whole migration) into a single reload.
        tokio::time::sleep(Duration::from_millis(100)).await;
        while rx.try_recv().is_ok() {}
        handle.refresh(pool).await;
    }
    drop(client);
    match (&mut driver.0).await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;

    #[tokio::test]
    async fn dropping_listener_releases_catalog_and_reconnect_task() {
        let pool = deadpool_postgres::Pool::builder(deadpool_postgres::Manager::new(
            tokio_postgres::Config::new(),
            tokio_postgres::NoTls,
        ))
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .unwrap();
        pool.close();
        let runtime = tokio::runtime::Handle::current();
        let baseline = runtime.metrics().num_alive_tasks();
        let mut references = Vec::new();
        for _ in 0..32 {
            let catalog = Arc::new(CatalogHandle::new(Catalog::default()));
            references.push(Arc::downgrade(&catalog));
            let listener =
                spawn_reload_listener(catalog, pool.clone(), tokio_postgres::Config::new());
            tokio::task::yield_now().await;
            drop(listener);
        }
        tokio::time::timeout(Duration::from_secs(1), async {
            while runtime.metrics().num_alive_tasks() > baseline {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("discarded listeners left reconnect tasks alive");
        assert!(references.iter().all(|weak| weak.upgrade().is_none()));
    }
}
