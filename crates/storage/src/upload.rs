//! Streamed file writes, quota enforcement and atomic metadata publication.
use crate::{
    StorageState, db,
    error::{bucket_not_found, storage_full, store_failed, too_large, write_error},
    mime::{self, SNIFF_BYTES},
    path,
    store::{PARTS_IN_FLIGHT, Store},
};
use axum::http::StatusCode;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use nelcota_core::{ApiError, Claims, db::begin_request};
use object_store::path::Path as Key;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::pin::Pin;
use uuid::Uuid;

pub type UploadStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;
#[derive(Default)]
pub struct UploadOptions {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub replace: bool,
}
#[derive(Serialize)]
pub struct UploadedObject {
    pub id: Uuid,
    pub bucket: String,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub etag: String,
}
pub struct UploadOutcome {
    pub object: UploadedObject,
    pub replaced: bool,
    pub public: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `POST`: a new name only.
    Create,
    /// `PUT`: create or replace.
    Upsert,
}

const INSERT: &str =
    "INSERT INTO storage.objects (id, bucket_id, name, version, size, mime_type, etag)
     VALUES ($1, $2, $3, $4, $5, $6, $7)";
const UPSERT: &str =
    "INSERT INTO storage.objects (id, bucket_id, name, version, size, mime_type, etag)
     VALUES ($1, $2, $3, $4, $5, $6, $7)
     ON CONFLICT (bucket_id, name) DO UPDATE
        SET version = EXCLUDED.version, size = EXCLUDED.size,
            mime_type = EXCLUDED.mime_type, etag = EXCLUDED.etag, updated_at = now()
     RETURNING id";

/// Owns byte storage and the metadata commit; HTTP adapters supply a byte stream.
impl StorageState {
    pub async fn upload(
        &self,
        claims: Claims,
        bucket: &str,
        name: &str,
        options: UploadOptions,
        body: UploadStream,
    ) -> Result<UploadOutcome, ApiError> {
        write(self.clone(), claims, bucket, name, options, body).await
    }
}
async fn write(
    state: StorageState,
    claims: Claims,
    bucket: &str,
    name: &str,
    options: UploadOptions,
    body: UploadStream,
) -> Result<UploadOutcome, ApiError> {
    let mode = if options.replace {
        Mode::Upsert
    } else {
        Mode::Create
    };
    let bucket = path::bucket(bucket)?;
    let name = path::object(name)?;
    let settings = db::bucket(&state.pool, bucket)
        .await?
        .ok_or_else(bucket_not_found)?;
    let limit = settings
        .file_size_limit
        .map_or(state.settings.max_file_size, |l| {
            l.min(state.settings.max_file_size)
        });
    let declared = options.content_length;
    if declared.is_some_and(|len| len > limit) {
        return Err(too_large(limit));
    }
    // Without a Content-Length the room is enforced as the bytes arrive.
    let room = room(&state).await?;
    if let Some(room) = room
        && declared.unwrap_or(1) > room.bytes
    {
        return Err(room.full());
    }

    // The policy check before any byte is taken (D78): the same write with
    // placeholder values, rolled back.
    {
        let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, &claims)
            .await
            .map_err(|e| ApiError::from_db(e, claims.role()))?;
        let sql = if mode == Mode::Create { INSERT } else { UPSERT };
        let placeholder: [&(dyn tokio_postgres::types::ToSql + Sync); 7] = [
            &Uuid::now_v7(),
            &bucket,
            &name,
            &Uuid::now_v7(),
            &0i64,
            &"application/octet-stream",
            &"",
        ];
        tx.execute(sql, &placeholder)
            .await
            .map_err(|e| write_error(e, &claims))?;
    }

    let _slot = state
        .uploads
        .acquire()
        .await
        .map_err(|_| ApiError::unavailable())?;
    let version = Uuid::now_v7();
    let key = Store::key(bucket, version);
    let declared_type = options.content_type.as_deref();
    let received = tokio::time::timeout(
        state.settings.upload_timeout,
        receive(
            &state.store,
            &key,
            body,
            limit,
            room,
            declared_type,
            settings.allowed_mime_types.as_deref(),
        ),
    )
    .await
    .unwrap_or_else(|_| {
        Err(ApiError::new(
            StatusCode::REQUEST_TIMEOUT,
            "upload_timeout",
            "the upload took too long",
        ))
    });
    let received = match received {
        Ok(received) => received,
        Err(err) => {
            state.store.delete_quietly(&key).await;
            return Err(err);
        }
    };

    match record(&state, &claims, bucket, &name, version, &received, mode).await {
        Ok((id, replaced)) => {
            if let Some(old) = replaced.filter(|old| *old != version) {
                state.store.delete_quietly(&Store::key(bucket, old)).await;
            }
            Ok(UploadOutcome {
                object: UploadedObject {
                    id,
                    bucket: bucket.to_owned(),
                    name,
                    size: received.size,
                    mime_type: received.mime_type,
                    etag: received.etag,
                },
                replaced: replaced.is_some(),
                public: settings.public,
            })
        }
        Err(err) => {
            state.store.delete_quietly(&key).await;
            Err(err)
        }
    }
}

/// Writes the row once the bytes are stored. Returns the row id and, when a
/// file was replaced, the version its bytes are under.
async fn record(
    state: &StorageState,
    claims: &Claims,
    bucket: &str,
    name: &str,
    version: Uuid,
    received: &Received,
    mode: Mode,
) -> Result<(Uuid, Option<Uuid>), ApiError> {
    let db_err = |e| write_error(e, claims);
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_request(&mut client, claims).await.map_err(db_err)?;
    let id = Uuid::now_v7();
    let size = i64::try_from(received.size).unwrap_or(i64::MAX);
    let values: [&(dyn tokio_postgres::types::ToSql + Sync); 7] = [
        &id,
        &bucket,
        &name,
        &version,
        &size,
        &received.mime_type,
        &received.etag,
    ];
    let (id, replaced) = match mode {
        Mode::Create => {
            tx.execute(INSERT, &values).await.map_err(db_err)?;
            (id, None)
        }
        Mode::Upsert => {
            // Under the caller's policies: replacing needs SELECT anyway.
            let old: Option<Uuid> = tx
                .query_opt(
                    "SELECT version FROM storage.objects
                      WHERE bucket_id = $1 AND name = $2 FOR UPDATE",
                    &[&bucket, &name],
                )
                .await
                .map_err(db_err)?
                .map(|r| r.get(0));
            let row = tx.query_one(UPSERT, &values).await.map_err(db_err)?;
            (row.get(0), old)
        }
    };
    tx.commit().await.map_err(db_err)?;
    Ok((id, replaced))
}

/// Bytes an upload may still add before the total quota or the disk guard
/// (D80) refuses it; the tighter of the two.
#[derive(Clone, Copy)]
struct Room {
    bytes: u64,
    disk: bool,
}

impl Room {
    fn full(self) -> ApiError {
        if self.disk {
            tracing::warn!("upload refused: the storage disk is almost full");
            storage_full("the server's disk is almost full")
        } else {
            storage_full("the project's storage quota is full")
        }
    }
}

async fn room(state: &StorageState) -> Result<Option<Room>, ApiError> {
    let mut room: Option<Room> = None;
    if let Some(max) = state.settings.max_total_size {
        let used = db::used_bytes(&state.pool).await?;
        room = Some(Room {
            bytes: max.saturating_sub(used),
            disk: false,
        });
    }
    if let Some(free) = state.store.free_bytes() {
        let disk = free.saturating_sub(state.settings.min_free_bytes);
        if room.is_none_or(|r| disk < r.bytes) {
            room = Some(Room {
                bytes: disk,
                disk: true,
            });
        }
    }
    Ok(room)
}

struct Received {
    size: u64,
    mime_type: String,
    etag: String,
}

/// Streams the body to `key`, sniffing the type from the first bytes and
/// enforcing the size limit and the room left as bytes arrive.
async fn receive(
    store: &Store,
    key: &Key,
    body: UploadStream,
    limit: u64,
    room: Option<Room>,
    declared_type: Option<&str>,
    allowed: Option<&[String]>,
) -> Result<Received, ApiError> {
    let interrupted = |_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "upload_interrupted",
            "the upload was interrupted",
        )
    };
    let mut stream = body;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut take = |chunk: &Bytes| {
        size += chunk.len() as u64;
        hasher.update(chunk);
        match room {
            _ if size > limit => Err(too_large(limit)),
            Some(room) if size > room.bytes => Err(room.full()),
            _ => Ok(()),
        }
    };

    // The first bytes decide the type before anything is written.
    let mut head = Vec::new();
    let mut ended = true;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(interrupted)?;
        take(&chunk)?;
        head.extend_from_slice(&chunk);
        if head.len() >= SNIFF_BYTES {
            ended = false;
            break;
        }
    }
    let mime_type = mime::effective(&head, declared_type);
    if !mime::allowed(&mime_type, allowed) {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "mime_type_not_allowed",
            format!("this bucket does not accept {mime_type}"),
        ));
    }

    if ended {
        store
            .put(key, Bytes::from(head))
            .await
            .map_err(store_failed)?;
    } else {
        let mut writer = store.writer(key).await.map_err(store_failed)?;
        writer.write(&head);
        let mut result = Ok(());
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk
                .map_err(interrupted)
                .and_then(|c| take(&c).map(|()| c))
            {
                Ok(chunk) => chunk,
                Err(err) => {
                    result = Err(err);
                    break;
                }
            };
            if let Err(err) = writer.wait_for_capacity(PARTS_IN_FLIGHT).await {
                result = Err(store_failed(err));
                break;
            }
            writer.put(chunk);
        }
        match result {
            Ok(()) => {
                writer.finish().await.map_err(store_failed)?;
            }
            Err(err) => {
                let _ = writer.abort().await;
                return Err(err);
            }
        }
    }
    Ok(Received {
        size,
        mime_type,
        etag: hex(&hasher.finalize()),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
