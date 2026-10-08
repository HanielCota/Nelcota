//! HTTP adapters for object operations, shared by public routes and the panel.
use crate::{
    StorageState, UploadOptions, UploadOutcome, UploadStream, path,
    serve::{self, Access},
    signing,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use nelcota_auth::Auth;
use nelcota_core::{ApiError, Claims};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde::Deserialize;
use serde_json::json;
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

#[derive(Deserialize)]
pub struct DownloadQuery {
    /// Any value serves the file as an attachment.
    download: Option<String>,
    token: Option<String>,
}

#[derive(Deserialize)]
pub struct SignRequest {
    expires_in: u64,
}

#[derive(Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    prefix: String,
    limit: Option<i64>,
    #[serde(default)]
    offset: i64,
}

/// Converts transport metadata to the upload operation's input.
pub fn upload_options(headers: &HeaderMap, replace: bool) -> UploadOptions {
    UploadOptions {
        content_length: headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok()),
        content_type: headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        replace,
    }
}
pub fn upload_stream(body: Body) -> UploadStream {
    Box::pin(
        body.into_data_stream()
            .map(|chunk| chunk.map_err(std::io::Error::other)),
    )
}
pub fn upload_response(state: &StorageState, outcome: UploadOutcome) -> Response {
    let status = if outcome.replaced {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let public_url = outcome.public.then(|| {
        format!(
            "{}{}",
            state.settings.public_url.as_deref().unwrap_or(""),
            url_path("public/", &outcome.object.bucket, &outcome.object.name)
        )
    });
    let mut body = serde_json::to_value(outcome.object).expect("uploaded object serializes");
    if let Some(url) = public_url {
        body["public_url"] = json!(url);
    }
    (status, Json(body)).into_response()
}
async fn upload(
    state: StorageState,
    claims: Claims,
    bucket: &str,
    name: &str,
    headers: &HeaderMap,
    body: Body,
    replace: bool,
) -> Result<Response, ApiError> {
    let outcome = state
        .upload(
            claims,
            bucket,
            name,
            upload_options(headers, replace),
            upload_stream(body),
        )
        .await?;
    Ok(upload_response(&state, outcome))
}
pub(crate) async fn create(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    upload(state, claims, &bucket, &name, &headers, body, false).await
}
pub(crate) async fn upsert(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    upload(state, claims, &bucket, &name, &headers, body, true).await
}
/// Adapts an authorized file read to conditional/ranged HTTP serving.
pub async fn download(
    state: &StorageState,
    claims: &Claims,
    bucket: &str,
    name: &str,
    headers: &HeaderMap,
    attachment: bool,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(bucket)?;
    let name = path::object(name)?;
    let object = state.object(claims, bucket, &name).await?;
    serve::respond(
        state,
        bucket,
        &name,
        &object,
        headers,
        Access::Private,
        attachment,
    )
    .await
}
pub(crate) async fn private_download(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    download(
        &state,
        &claims,
        &bucket,
        &name,
        &headers,
        query.download.is_some(),
    )
    .await
}
pub(crate) async fn public(
    State(state): State<StorageState>,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let object = state.public_object(bucket, &name).await?;
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
pub(crate) async fn sign(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
    Json(request): Json<SignRequest>,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let token = signing::token(&state, &claims, bucket, &name, request.expires_in).await?;
    let url = format!("{}?token={token}", url_path("sign/", bucket, &name));
    Ok(Json(json!({"signed_url": url})).into_response())
}
pub(crate) async fn signed(
    State(state): State<StorageState>,
    Path((bucket, name)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let bucket = path::bucket(&bucket)?;
    let name = path::object(&name)?;
    let (object, secs) = signing::resolve(&state, bucket, &name, query.token.as_deref()).await?;
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
pub(crate) async fn remove(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path((bucket, name)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    state.delete(&claims, &bucket, &name).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
pub(crate) async fn list(
    State(state): State<StorageState>,
    Auth(claims): Auth,
    Path(bucket): Path<String>,
    Json(request): Json<ListRequest>,
) -> Result<Response, ApiError> {
    let listing = state
        .list(
            &claims,
            &bucket,
            &request.prefix,
            request.limit,
            request.offset,
        )
        .await?;
    Ok(Json(listing).into_response())
}
