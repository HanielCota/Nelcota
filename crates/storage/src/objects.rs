//! `/storage/v1/object/*`: upload, download, list, sign and delete files.

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::StreamExt;
use nelcota_auth::Auth;
use nelcota_core::{ApiError, Claims, db::begin_request};
use object_store::path::Path as Key;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_postgres::error::SqlState;
use uuid::Uuid;

use crate::{
    StorageState, db,
    mime::{self, SNIFF_BYTES},
    path,
    serve::{self, Access},
    store::{PARTS_IN_FLIGHT, Store},
};

/// Longest a signed URL may live: a week.
const MAX_SIGNED_SECS: u64 = 7 * 86_400;
const MAX_LIST: i64 = 1000;
/// `typ` of a signed URL token: never accepted as an API token (it has no
/// `role`), and API tokens are never accepted as signatures.
const SIGNED_TYP: &str = "nelcota-storage-url";

/// Characters escaped in a path segment of a generated URL.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/');

fn url_path(kind: &str, bucket: &str, name: &str) -> String {
    let name: Vec<String> = name
        .split('/')
        .map(|s| utf8_percent_encode(s, SEGMENT).to_string())
        .collect();
    format!("/storage/v1/object/{kind}{bucket}/{}", name.join("/"))
}

fn bucket_not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "bucket_not_found",
        "bucket not found",
    )
}

fn storage_full(message: &str) -> ApiError {
    ApiError::new(StatusCode::INSUFFICIENT_STORAGE, "storage_full", message)
}

fn too_large(limit: u64) -> ApiError {
    ApiError::new(
        StatusCode::PAYLOAD_TOO_LARGE,
        "file_too_large",
        format!("the file exceeds the limit of {limit} bytes"),
    )
}

fn store_failed(err: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %err, "storage backend failed");
    ApiError::internal()
}

/// Maps a database error of a write: an existing name is its own case.
fn write_error(err: tokio_postgres::Error, claims: &Claims) -> ApiError {
    if err.code() == Some(&SqlState::UNIQUE_VIOLATION) {
        return ApiError::new(
            StatusCode::CONFLICT,
            "object_exists",
            "a file with this name already exists; use PUT to replace it",
        );
    }
    ApiError::from_db(err, claims.role())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `POST`: a new name only.
    Create,
    /// `PUT`: create or replace.
    Upsert,
}

/// `POST /storage/v1/object/{bucket}/{*name}`
pub async fn create(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    write(state, claims, &bucket, &name, &headers, body, Mode::Create).await
}

/// `PUT /storage/v1/object/{bucket}/{*name}`
pub async fn upsert(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    write(state, claims, &bucket, &name, &headers, body, Mode::Upsert).await
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

async fn write(
    state: StorageState,
    claims: Claims,
    bucket: &str,
    name: &str,
    headers: &HeaderMap,
    body: Body,
    mode: Mode,
) -> Result<Response, ApiError> {
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
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
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
    let declared_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
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
            let status = match (mode, replaced) {
                (Mode::Upsert, Some(_)) => StatusCode::OK,
                _ => StatusCode::CREATED,
            };
            let mut body = json!({
                "id": id,
                "bucket": bucket,
                "name": name,
                "size": received.size,
                "mime_type": received.mime_type,
                "etag": received.etag,
            });
            if settings.public {
                body["public_url"] = json!(format!(
                    "{}{}",
                    state.settings.public_url.as_deref().unwrap_or(""),
                    url_path("public/", bucket, &name)
                ));
            }
            Ok((status, Json(body)).into_response())
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
    body: Body,
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
    let mut stream = body.into_data_stream();
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

#[derive(Deserialize)]
pub struct DownloadQuery {
    /// Any value serves the file as an attachment.
    download: Option<String>,
    token: Option<String>,
}

/// `GET /storage/v1/object/{bucket}/{*name}`: under the caller's policies.
pub async fn download(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let object = {
        let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, &claims)
            .await
            .map_err(|e| ApiError::from_db(e, claims.role()))?;
        let sql = format!(
            "SELECT {} FROM storage.objects WHERE bucket_id = $1 AND name = $2",
            db::OBJECT_COLUMNS
        );
        let row = tx
            .query_opt(&sql, &[&bucket, &name])
            .await
            .map_err(|e| ApiError::from_db(e, claims.role()))?;
        row.as_ref().map(db::Object::from_row)
    }
    .ok_or_else(serve::not_found)?;
    serve::respond(
        &state,
        bucket,
        &name,
        &object,
        &headers,
        Access::Private,
        query.download.is_some(),
    )
    .await
}

/// `GET /storage/v1/object/public/{bucket}/{*name}`: anyone, if the bucket
/// is public.
pub async fn public(
    State(state): State<StorageState>,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    match db::bucket(&state.pool, bucket).await? {
        Some(settings) if settings.public => {}
        _ => return Err(bucket_not_found()),
    }
    let object = db::object(&state.pool, bucket, &name)
        .await?
        .ok_or_else(serve::not_found)?;
    serve::respond(
        &state,
        bucket,
        &name,
        &object,
        &headers,
        Access::Public,
        query.download.is_some(),
    )
    .await
}

#[derive(Deserialize)]
pub struct SignRequest {
    expires_in: u64,
}

/// `POST /storage/v1/object/sign/{bucket}/{*name}`: a URL that lets anyone
/// holding it read the file for `expires_in` seconds. Only a caller whose
/// policies let them read the file can sign it.
pub async fn sign(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    Json(request): Json<SignRequest>,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    if !(1..=MAX_SIGNED_SECS).contains(&request.expires_in) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_expiry",
            format!("expires_in between 1 and {MAX_SIGNED_SECS} seconds"),
        ));
    }
    let visible = {
        let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, &claims)
            .await
            .map_err(|e| ApiError::from_db(e, claims.role()))?;
        tx.query_opt(
            "SELECT 1 FROM storage.objects WHERE bucket_id = $1 AND name = $2",
            &[&bucket, &name],
        )
        .await
        .map_err(|e| ApiError::from_db(e, claims.role()))?
        .is_some()
    };
    if !visible {
        return Err(serve::not_found());
    }
    let now = jsonwebtoken_now();
    let token = state
        .keys
        .sign(&json!({
            "typ": SIGNED_TYP,
            "bucket": bucket,
            "name": name,
            "iat": now,
            "exp": now + request.expires_in,
        }))
        .map_err(store_failed)?;
    let url = format!("{}?token={token}", url_path("sign/", bucket, &name));
    Ok(Json(json!({ "signed_url": url })).into_response())
}

fn jsonwebtoken_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `GET /storage/v1/object/sign/{bucket}/{*name}?token=...`
pub async fn signed(
    State(state): State<StorageState>,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let invalid = || {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_signature",
            "invalid or expired signed URL",
        )
    };
    let payload = query
        .token
        .as_deref()
        .and_then(|t| state.keys.verify_payload(t).ok())
        .ok_or_else(invalid)?;
    let matches = payload["typ"] == SIGNED_TYP
        && payload["bucket"] == bucket
        && payload["name"].as_str() == Some(name.as_str());
    if !matches {
        return Err(invalid());
    }
    let secs = payload["exp"]
        .as_u64()
        .unwrap_or(0)
        .saturating_sub(jsonwebtoken_now());
    let object = db::object(&state.pool, bucket, &name)
        .await?
        .ok_or_else(serve::not_found)?;
    serve::respond(
        &state,
        bucket,
        &name,
        &object,
        &headers,
        Access::Signed { secs },
        query.download.is_some(),
    )
    .await
}

/// `DELETE /storage/v1/object/{bucket}/{*name}`: the row under the caller's
/// policies, then the bytes.
pub async fn remove(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let db_err = |e| ApiError::from_db(e, claims.role());
    let version: Uuid = {
        let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, &claims).await.map_err(db_err)?;
        let row = tx
            .query_opt(
                "DELETE FROM storage.objects WHERE bucket_id = $1 AND name = $2 RETURNING version",
                &[&bucket, &name],
            )
            .await
            .map_err(db_err)?
            .ok_or_else(serve::not_found)?;
        tx.commit().await.map_err(db_err)?;
        row.get(0)
    };
    state
        .store
        .delete_quietly(&Store::key(bucket, version))
        .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    prefix: String,
    limit: Option<i64>,
    #[serde(default)]
    offset: i64,
}

/// `POST /storage/v1/object/list/{bucket}`: the folders and files right
/// under `prefix`, as far as the caller's policies show them.
pub async fn list(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(bucket): Path<String>,
    Json(request): Json<ListRequest>,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let prefix = path::prefix(&request.prefix)?;
    let limit = request.limit.unwrap_or(100).clamp(1, MAX_LIST);
    let offset = request.offset.max(0);
    let pattern = format!(
        "{}%",
        prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let skip = prefix.chars().count() as i32;
    let db_err = |e| ApiError::from_db(e, claims.role());
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_request(&mut client, &claims).await.map_err(db_err)?;
    let row = tx
        .query_one(
            "WITH under AS (
                SELECT o.*, substr(o.name, $3 + 1) AS rest
                  FROM storage.objects o
                 WHERE o.bucket_id = $1 AND o.name LIKE $2 ESCAPE '\\'
             )
             SELECT json_build_object(
                'folders', coalesce((
                    SELECT json_agg(f ORDER BY f) FROM (
                        SELECT DISTINCT split_part(rest, '/', 1) AS f
                          FROM under WHERE strpos(rest, '/') > 0
                         ORDER BY 1 LIMIT $4
                    ) d), '[]'),
                'objects', coalesce((
                    SELECT json_agg(json_build_object(
                        'id', id, 'name', name, 'size', size, 'mime_type', mime_type,
                        'etag', etag, 'owner', owner, 'metadata', metadata,
                        'created_at', created_at, 'updated_at', updated_at) ORDER BY name)
                      FROM (SELECT * FROM under WHERE strpos(rest, '/') = 0
                             ORDER BY name LIMIT $4 OFFSET $5) page), '[]')
             )::text",
            &[&bucket, &pattern, &skip, &limit, &offset],
        )
        .await
        .map_err(db_err)?;
    let body: String = row.get(0);
    Ok(([(header::CONTENT_TYPE, "application/json")], body).into_response())
}
