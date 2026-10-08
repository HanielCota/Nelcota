//! Reconnecting PostgreSQL notification listener.
use super::{CatalogHandle, RELOAD_CHANNEL};
use futures_util::{StreamExt, stream};
use std::{sync::Arc, time::Duration};
use tokio_postgres::{AsyncMessage, NoTls};

/// Keeps a `LISTEN nelcota` open and reloads the catalog on each notification.
/// Reconnects on its own and reloads on each reconnection (notices may have
/// been missed).
pub fn spawn_reload_listener(
    handle: Arc<CatalogHandle>,
    pool: deadpool_postgres::Pool,
    config: tokio_postgres::Config,
) {
    tokio::spawn(async move {
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
    });
}

async fn listen_once(
    handle: &Arc<CatalogHandle>,
    pool: &deadpool_postgres::Pool,
    config: &tokio_postgres::Config,
) -> Result<(), tokio_postgres::Error> {
    let (client, mut connection) = config.connect(NoTls).await?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let driver = tokio::spawn(async move {
        let mut messages = stream::poll_fn(move |cx| connection.poll_message(cx));
        while let Some(message) = messages.next().await {
            match message {
                Ok(AsyncMessage::Notification(_)) => {
                    let _ = tx.send(());
                }
                Ok(_) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    });
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
    match driver.await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}
