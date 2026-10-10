//! Storage in the panel: buckets with their usage, and a file browser. Files
//! go through the storage crate as `service_role` (the admin owns the
//! database anyway), so uploads get the same checks as the API: size,
//! type, quota and free disk.

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use nelcota_core::Claims;
use nelcota_storage::{BucketError, BucketSettings, StorageState};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AdminState, ApiError};

fn service_role() -> Claims {
    Claims::from_payload(json!({ "role": "service_role" })).expect("valid claims")
}

/// The storage crate's error, in the panel's shape.
fn from_storage(err: nelcota_core::ApiError) -> ApiError {
    ApiError::new(err.status(), err.code(), err.message())
}

fn enabled(state: &AdminState) -> Result<&StorageState, ApiError> {
    state.storage.as_ref().ok_or_else(|| {
        ApiError::not_found(
            "storage_disabled",
            "file storage is off (NELCOTA_STORAGE_BACKEND)",
        )
    })
}

fn bucket_id(id: &str) -> Result<&str, ApiError> {
    if nelcota_storage::valid_bucket(id) {
        Ok(id)
    } else {
        Err(ApiError::bad_request(
            "invalid_bucket_name",
            "invalid bucket name: lowercase letters, digits, '-' and '_', up to 63",
        ))
    }
}

/// `GET /admin/api/storage`: whether storage is on, and each bucket with its
/// file count and bytes.
pub async fn overview(
    State(state): State<AdminState>,
) -> Result<Json<crate::contracts::StorageOverview>, ApiError> {
    let Some(storage) = &state.storage else {
        return crate::contracts::response::<crate::contracts::StorageOverview>(
            json!({ "enabled": false }),
        );
    };
    let client = state.db.get().await?;
    let row = client
        .query_one(
            "SELECT coalesce(json_agg(b ORDER BY b.id), '[]')::text FROM (
                SELECT k.id, k.public, k.file_size_limit, k.allowed_mime_types, k.created_at,
                       count(o.id) AS files, coalesce(sum(o.size), 0)::bigint AS bytes
                  FROM storage.buckets k LEFT JOIN storage.objects o ON o.bucket_id = k.id
                 GROUP BY k.id) b",
            &[],
        )
        .await?;
    let buckets: Value = serde_json::from_str(row.get(0))?;
    let settings = &storage.settings;
    crate::contracts::response::<crate::contracts::StorageOverview>(json!({
        "enabled": true,
        "backend": storage.backend(),
        "max_file_size": settings.max_file_size,
        "max_total_size": settings.max_total_size,
        "public_url": settings.public_url,
        "buckets": buckets,
    }))
}

/// The panel accepts whitespace and empty MIME inputs from its forms.
/// Validation and writes belong to the shared storage operations.
fn form_settings(mut settings: BucketSettings) -> BucketSettings {
    settings.allowed_mime_types = settings
        .allowed_mime_types
        .map(|list| {
            list.into_iter()
                .map(|entry| entry.trim().to_ascii_lowercase())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|list| !list.is_empty());
    settings
}

fn from_bucket(error: BucketError, id: &str) -> ApiError {
    match error {
        BucketError::InvalidSizeLimit => ApiError::bad_request(
            "invalid_size_limit",
            "the size limit must be greater than zero",
        ),
        BucketError::InvalidMimeType(entry) => ApiError::bad_request(
            "invalid_mime_type",
            format!("invalid type: {entry} (use image/png or image/*)"),
        )
        .params(json!({ "type": entry })),
        BucketError::Exists => {
            ApiError::conflict("bucket_exists", "a bucket with this name already exists")
                .params(json!({ "bucket": id }))
        }
        BucketError::NotFound => ApiError::not_found("bucket_not_found", "bucket not found")
            .params(json!({ "bucket": id })),
        BucketError::NotEmpty { count } => {
            let error = ApiError::conflict(
                "bucket_not_empty",
                "the bucket still has files; delete them first",
            );
            match count {
                Some(count) => error.params(json!({ "count": count })),
                None => error,
            }
        }
        BucketError::Request(error) => from_storage(error),
    }
}

#[derive(Deserialize)]
pub struct NewBucket {
    id: String,
    #[serde(flatten)]
    settings: BucketSettings,
}

/// `POST /admin/api/storage/buckets`
pub async fn create_bucket(
    State(state): State<AdminState>,
    Json(new): Json<NewBucket>,
) -> Result<StatusCode, ApiError> {
    let storage = enabled(&state)?;
    let id = bucket_id(&new.id)?;
    storage
        .create_bucket(&service_role(), id, form_settings(new.settings))
        .await
        .map_err(|error| from_bucket(error, id))?;
    Ok(StatusCode::CREATED)
}

/// `PUT /admin/api/storage/buckets/{id}`
pub async fn update_bucket(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(settings): Json<BucketSettings>,
) -> Result<StatusCode, ApiError> {
    let storage = enabled(&state)?;
    let id = bucket_id(&id)?;
    storage
        .update_bucket(&service_role(), id, form_settings(settings))
        .await
        .map_err(|error| from_bucket(error, id))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /admin/api/storage/buckets/{id}`: only an empty bucket.
pub async fn delete_bucket(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let storage = enabled(&state)?;
    let id = bucket_id(&id)?;
    storage
        .delete_bucket(&service_role(), id)
        .await
        .map_err(|error| from_bucket(error, id))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    offset: i64,
}

/// Files listed per page in the panel.
const PAGE: i64 = 100;

/// `GET /admin/api/storage/buckets/{id}/objects?prefix=&offset=`
pub async fn list(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let storage = enabled(&state)?;
    let body = storage
        .list(
            &service_role(),
            &id,
            &query.prefix,
            Some(PAGE + 1),
            query.offset,
        )
        .await
        .map_err(from_storage)?;
    // Both lists use the same offset independently. One extra entry in either
    // list tells whether there is a next page.
    let mut page: Value = serde_json::to_value(body)?;
    let objects = page["objects"]
        .as_array_mut()
        .map(std::mem::take)
        .unwrap_or_default();
    let folders = page["folders"]
        .as_array_mut()
        .map(std::mem::take)
        .unwrap_or_default();
    let has_next = objects.len() as i64 > PAGE || folders.len() as i64 > PAGE;
    page["folders"] = Value::Array(folders.into_iter().take(PAGE as usize).collect());
    page["objects"] = Value::Array(objects.into_iter().take(PAGE as usize).collect());
    page["has_next"] = json!(has_next);
    Ok(crate::contracts::response::<crate::contracts::StorageListing>(page)?.into_response())
}

#[derive(Deserialize)]
pub struct FileQuery {
    name: String,
    #[serde(default)]
    replace: bool,
}

/// `GET /admin/api/storage/buckets/{id}/file?name=`: always as a download.
pub async fn download(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let storage = enabled(&state)?;
    nelcota_storage::http::download(storage, &service_role(), &id, &query.name, &headers, true)
        .await
        .map_err(from_storage)
}

/// `DELETE /admin/api/storage/buckets/{id}/file?name=`
pub async fn delete(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Result<StatusCode, ApiError> {
    let storage = enabled(&state)?;
    storage
        .delete(&service_role(), &id, &query.name)
        .await
        .map_err(from_storage)?;
    tracing::info!(
        bucket = id,
        name = query.name,
        "file deleted from the panel"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/api/storage/buckets/{id}/upload?name=&replace=`: the body is
/// the file. Mounted outside the request timeout (see [`crate::upload_router`]).
pub async fn upload(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let storage = enabled(&state)?;
    let outcome = storage
        .upload(
            service_role(),
            &id,
            &query.name,
            nelcota_storage::http::upload_options(&headers, query.replace),
            nelcota_storage::http::upload_stream(body),
        )
        .await
        .map_err(from_storage)?;
    Ok(nelcota_storage::http::upload_response(storage, outcome))
}
