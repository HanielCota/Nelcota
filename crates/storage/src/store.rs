//! Where the bytes live (D77): a local directory or an S3-compatible bucket,
//! both behind `object_store`. Keys are `<bucket>/<version>` (D78); object
//! names never reach the store.

use std::{path::PathBuf, sync::Arc, time::Duration};

use futures_util::stream::BoxStream;
use nelcota_core::{Config, config::StorageBackend};
use object_store::{
    GetOptions, GetRange, GetResult, ObjectMeta, ObjectStore, ObjectStoreExt, PutPayload,
    WriteMultipart,
    aws::{AmazonS3, AmazonS3Builder},
    local::LocalFileSystem,
    path::Path as Key,
    signer::{Method, SignedUrlOptions, Signer, Url},
};
use uuid::Uuid;

/// Part size of an upload: S3 refuses parts under 5 MiB (except the last);
/// on disk a part is a write, so smaller ones keep memory low.
const S3_PART: usize = 8 * 1024 * 1024;
const DISK_PART: usize = 1024 * 1024;
/// Parts of one upload in flight at once.
pub const PARTS_IN_FLIGHT: usize = 2;

pub struct Store {
    inner: Arc<dyn ObjectStore>,
    backend: Backend,
}

enum Backend {
    Disk { root: PathBuf },
    S3 { s3: Arc<AmazonS3> },
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not create the storage directory {0}: {1}")]
    Directory(PathBuf, std::io::Error),
    #[error(transparent)]
    Store(#[from] object_store::Error),
}

impl Store {
    /// The configured store, or `None` when storage is off.
    pub fn from_config(config: &Config) -> Result<Option<Self>, StoreError> {
        match config.storage_backend {
            StorageBackend::Off => Ok(None),
            StorageBackend::Disk => {
                let root = config
                    .storage_dir
                    .clone()
                    .expect("validated: disk needs a directory");
                Self::disk(root).map(Some)
            }
            StorageBackend::S3 => {
                // reqwest's rustls runs on `ring` (D16); installing twice is harmless.
                let _ = rustls::crypto::ring::default_provider().install_default();
                let mut builder = AmazonS3Builder::new()
                    .with_bucket_name(config.storage_s3_bucket.clone().unwrap_or_default())
                    .with_region(&config.storage_s3_region)
                    .with_access_key_id(config.storage_s3_access_key_id.clone().unwrap_or_default())
                    .with_secret_access_key(
                        config
                            .storage_s3_secret_access_key
                            .as_ref()
                            .map(|s| s.expose().to_owned())
                            .unwrap_or_default(),
                    )
                    .with_virtual_hosted_style_request(config.storage_s3_virtual_hosted);
                if let Some(endpoint) = config
                    .storage_s3_endpoint
                    .as_deref()
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                {
                    builder = builder
                        .with_endpoint(endpoint)
                        .with_allow_http(endpoint.starts_with("http://"));
                }
                let s3 = Arc::new(builder.build()?);
                Ok(Some(Store {
                    inner: s3.clone(),
                    backend: Backend::S3 { s3 },
                }))
            }
        }
    }

    /// A store on a local directory, created if missing.
    pub fn disk(root: PathBuf) -> Result<Self, StoreError> {
        std::fs::create_dir_all(&root).map_err(|e| StoreError::Directory(root.clone(), e))?;
        let fs = LocalFileSystem::new_with_prefix(&root)?.with_automatic_cleanup(true);
        Ok(Store {
            inner: Arc::new(fs),
            backend: Backend::Disk { root },
        })
    }

    pub fn key(bucket: &str, version: Uuid) -> Key {
        Key::from_iter([bucket, &version.to_string()])
    }

    /// Free space left for the disk backend; `None` on S3.
    pub fn free_bytes(&self) -> Option<u64> {
        match &self.backend {
            Backend::Disk { root } => match fs4::available_space(root) {
                Ok(free) => Some(free),
                Err(err) => {
                    tracing::error!(error = %err, "could not read the storage disk's free space");
                    Some(0)
                }
            },
            Backend::S3 { .. } => None,
        }
    }

    pub fn is_s3(&self) -> bool {
        matches!(self.backend, Backend::S3 { .. })
    }

    /// Starts writing a new object, in parts.
    pub async fn writer(&self, key: &Key) -> Result<WriteMultipart, StoreError> {
        let part = if self.is_s3() { S3_PART } else { DISK_PART };
        let upload = self.inner.put_multipart(key).await?;
        Ok(WriteMultipart::new_with_chunk_size(upload, part))
    }

    /// Writes a small object in one request.
    pub async fn put(&self, key: &Key, bytes: bytes::Bytes) -> Result<(), StoreError> {
        self.inner.put(key, PutPayload::from(bytes)).await?;
        Ok(())
    }

    pub async fn get(&self, key: &Key, range: Option<GetRange>) -> Result<GetResult, StoreError> {
        let options = GetOptions {
            range,
            ..GetOptions::default()
        };
        Ok(self.inner.get_opts(key, options).await?)
    }

    /// Deletes bytes; a key that is already gone is not an error.
    pub async fn delete(&self, key: &Key) -> Result<(), StoreError> {
        match self.inner.delete(key).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    /// Deletes bytes after a commit: a failure leaves them for the collector.
    pub async fn delete_quietly(&self, key: &Key) {
        if let Err(err) = self.delete(key).await {
            tracing::warn!(key = %key, error = %err, "could not delete stored bytes; the collector will retry");
        }
    }

    pub fn list(&self) -> BoxStream<'static, object_store::Result<ObjectMeta>> {
        self.inner.list(None)
    }

    /// S3 only: a presigned GET that also fixes the response's type and
    /// disposition, so the provider serves the file as this server would.
    pub async fn presigned_get(
        &self,
        key: &Key,
        expires_in: Duration,
        content_type: &str,
        disposition: &str,
    ) -> Result<Option<Url>, StoreError> {
        let Backend::S3 { s3 } = &self.backend else {
            return Ok(None);
        };
        let options = SignedUrlOptions::new().with_query([
            ("response-content-type", content_type),
            ("response-content-disposition", disposition),
        ]);
        let url = s3
            .signed_url_opts(Method::GET, key, expires_in, &options)
            .await?;
        Ok(Some(url))
    }
}
