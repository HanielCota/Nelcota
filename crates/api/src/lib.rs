//! Automatic REST API over the Postgres catalog, always executed under RLS.
//!
//! - `GET    /rest/v1/`                 OpenAPI (filtered by role)
//! - `GET    /rest/v1/{table}`          read with filters, ordering and paging
//! - `POST   /rest/v1/{table}`          insert (object or array); upsert with
//!   `Prefer: resolution=merge-duplicates|ignore-duplicates` (+ `on_conflict`)
//! - `PATCH  /rest/v1/{table}?filters`  update
//! - `DELETE /rest/v1/{table}?filters`  delete
//! - `POST   /rest/v1/rpc/{function}`   SQL function call
//!
//! Each request runs in a transaction with the JWT role and claims
//! (`nelcota_core::db::begin_request`). The API does not decide permissions:
//! Postgres does (GRANTs + RLS).

pub mod catalog;
pub mod openapi;
pub mod query;
pub mod typescript;

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{FromRef, Path, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use deadpool_postgres::{Pool, Transaction};
use nelcota_auth::{Auth, SharedVerifier};
use nelcota_core::{ApiError, Claims, db};
use serde_json::{Map, Value};

pub use catalog::{Catalog, CatalogHandle, spawn_reload_listener};
use query::{QueryError, Resolution, Sql};

#[derive(Clone, Debug, Default)]
pub struct ApiSettings {
    /// Row cap per read (`NELCOTA_MAX_ROWS`); `None` = no cap.
    pub max_rows: Option<i64>,
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Pool: FromRef<S>,
    SharedVerifier: FromRef<S>,
    Arc<CatalogHandle>: FromRef<S>,
    Arc<ApiSettings>: FromRef<S>,
{
    Router::new()
        .route("/rest/v1", get(openapi))
        .route("/rest/v1/", get(openapi))
        .route(
            "/rest/v1/{table}",
            get(read).post(create).patch(update).delete(remove),
        )
        .route("/rest/v1/rpc/{function}", post(rpc))
}

// ------------------------------------------------------------------ helpers

fn bad_query(err: QueryError) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_query", err.to_string())
}

fn not_found(kind: &str, name: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        format!("{kind} '{name}' does not exist in the exposed schema"),
    )
}

fn pairs(raw: Option<String>) -> Vec<(String, String)> {
    form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
        .into_owned()
        .collect()
}

fn parse_body(bytes: &Bytes) -> Result<Value, ApiError> {
    if bytes.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_slice(bytes).map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            format!("invalid JSON: {e}"),
        )
    })
}

#[derive(Default)]
struct Prefer {
    representation: bool,
    count_exact: bool,
    resolution: Option<Resolution>,
}

impl Prefer {
    fn from_headers(headers: &HeaderMap) -> Self {
        let mut prefer = Prefer::default();
        for value in headers.get_all("prefer") {
            for token in value.to_str().unwrap_or_default().split(',') {
                match token.trim() {
                    "return=representation" => prefer.representation = true,
                    "return=minimal" => prefer.representation = false,
                    "count=exact" => prefer.count_exact = true,
                    "resolution=merge-duplicates" => prefer.resolution = Some(Resolution::Merge),
                    "resolution=ignore-duplicates" => prefer.resolution = Some(Resolution::Ignore),
                    _ => {}
                }
            }
        }
        prefer
    }
}

async fn fetch_json(
    tx: &Transaction<'_>,
    sql: &Sql,
) -> Result<(String, i64), tokio_postgres::Error> {
    let statement = tx.prepare_cached(&sql.text).await?;
    let row = tx.query_one(&statement, &sql.param_refs()).await?;
    Ok((row.get(0), row.get(1)))
}

async fn execute(tx: &Transaction<'_>, sql: &Sql) -> Result<u64, tokio_postgres::Error> {
    let statement = tx.prepare_cached(&sql.text).await?;
    tx.execute(&statement, &sql.param_refs()).await
}

fn json_response(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// Runs the body in a transaction with the request role/claims.
macro_rules! in_request_tx {
    ($pool:expr, $claims:expr, |$tx:ident| $body:block) => {{
        let role = $claims.role();
        let db_err = |e| ApiError::from_db(e, role);
        let mut client = $pool.get().await.map_err(ApiError::from_pool)?;
        let $tx = db::begin_request(&mut client, &$claims)
            .await
            .map_err(db_err)?;
        let result = async { $body }.await.map_err(db_err)?;
        $tx.commit().await.map_err(db_err)?;
        result
    }};
}

// ------------------------------------------------------------------ handlers

async fn openapi(State(catalog): State<Arc<CatalogHandle>>, Auth(claims): Auth) -> Json<Value> {
    Json(openapi::document(&catalog.get(), claims.role()))
}

async fn read(
    State(pool): State<Pool>,
    State(catalog): State<Arc<CatalogHandle>>,
    State(settings): State<Arc<ApiSettings>>,
    Path(name): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    Auth(claims): Auth,
) -> Result<Response, ApiError> {
    let catalog = catalog.get();
    let table = catalog
        .table(&name)
        .ok_or_else(|| not_found("table", &name))?;
    let request = query::parse_request(&pairs(raw), table).map_err(bad_query)?;
    reject_on_conflict(&request)?;
    let prefer = Prefer::from_headers(&headers);
    let sql = query::select(&catalog.schema, table, &request, settings.max_rows);
    let count_sql = prefer
        .count_exact
        .then(|| query::count(&catalog.schema, table, &request));

    let (body, rows, total) = in_request_tx!(pool, claims, |tx| {
        let (body, rows) = fetch_json(&tx, &sql).await?;
        let total = match &count_sql {
            Some(count_sql) => {
                let statement = tx.prepare_cached(&count_sql.text).await?;
                let row = tx.query_one(&statement, &count_sql.param_refs()).await?;
                Some(row.get::<_, i64>(0))
            }
            None => None,
        };
        Ok::<_, tokio_postgres::Error>((body, rows, total))
    });

    let offset = request.offset.unwrap_or(0);
    let total_text = total.map_or_else(|| "*".to_owned(), |t| t.to_string());
    let range = if rows == 0 {
        format!("*/{total_text}")
    } else {
        format!("{offset}-{}/{total_text}", offset + rows - 1)
    };
    let mut response = json_response(StatusCode::OK, body);
    if let Ok(value) = HeaderValue::from_str(&range) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    if prefer.count_exact {
        response.headers_mut().insert(
            "preference-applied",
            HeaderValue::from_static("count=exact"),
        );
    }
    Ok(response)
}

/// Response of a write: the representation (JSON array) or just the status.
async fn write(
    pool: &Pool,
    claims: &Claims,
    sql: Sql,
    representation: bool,
    status_with_body: StatusCode,
    status_minimal: StatusCode,
) -> Result<Response, ApiError> {
    if representation {
        let (body, _) = in_request_tx!(pool, claims, |tx| { fetch_json(&tx, &sql).await });
        Ok(json_response(status_with_body, body))
    } else {
        in_request_tx!(pool, claims, |tx| { execute(&tx, &sql).await });
        Ok(status_minimal.into_response())
    }
}

async fn create(
    State(pool): State<Pool>,
    State(catalog): State<Arc<CatalogHandle>>,
    Path(name): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    Auth(claims): Auth,
    bytes: Bytes,
) -> Result<Response, ApiError> {
    let catalog = catalog.get();
    let table = catalog
        .table(&name)
        .ok_or_else(|| not_found("table", &name))?;
    let request = query::parse_request(&pairs(raw), table).map_err(bad_query)?;
    if !request.filters.is_empty() {
        return Err(bad_query(QueryError::Invalid(
            "POST does not accept filters".into(),
        )));
    }
    let prefer = Prefer::from_headers(&headers);
    let upsert = match (prefer.resolution, &request.on_conflict) {
        (Some(resolution), on_conflict) => {
            Some(query::Upsert::new(table, on_conflict.as_deref(), resolution).map_err(bad_query)?)
        }
        (None, Some(_)) => {
            return Err(bad_query(QueryError::Invalid(
                "on_conflict needs Prefer: resolution=merge-duplicates or ignore-duplicates".into(),
            )));
        }
        (None, None) => None,
    };
    let body = parse_body(&bytes)?;
    let representation = prefer.representation.then_some(&request.select);
    let sql = query::insert(
        &catalog.schema,
        table,
        body,
        representation,
        upsert.as_ref(),
    )
    .map_err(bad_query)?;
    let mut response = write(
        &pool,
        &claims,
        sql,
        prefer.representation,
        StatusCode::CREATED,
        StatusCode::CREATED,
    )
    .await?;
    if let Some(upsert) = upsert {
        let applied = match upsert.resolution {
            Resolution::Merge => "resolution=merge-duplicates",
            Resolution::Ignore => "resolution=ignore-duplicates",
        };
        response
            .headers_mut()
            .insert("preference-applied", HeaderValue::from_static(applied));
    }
    Ok(response)
}

/// `on_conflict` only means something to an upsert (POST).
fn reject_on_conflict(request: &query::Request) -> Result<(), ApiError> {
    if request.on_conflict.is_some() {
        return Err(bad_query(QueryError::Invalid(
            "on_conflict only applies to POST with Prefer: resolution=...".into(),
        )));
    }
    Ok(())
}

/// PATCH/DELETE require at least one filter: prevents deleting/changing the
/// whole table by mistake (for that, use an explicit filter such as
/// `id=not.is.null`).
fn require_filters(request: &query::Request) -> Result<(), ApiError> {
    if request.filters.is_empty() {
        return Err(bad_query(QueryError::Invalid(
            "provide at least one filter (e.g. ?id=eq.1)".into(),
        )));
    }
    Ok(())
}

async fn update(
    State(pool): State<Pool>,
    State(catalog): State<Arc<CatalogHandle>>,
    Path(name): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    Auth(claims): Auth,
    bytes: Bytes,
) -> Result<Response, ApiError> {
    let catalog = catalog.get();
    let table = catalog
        .table(&name)
        .ok_or_else(|| not_found("table", &name))?;
    let request = query::parse_request(&pairs(raw), table).map_err(bad_query)?;
    require_filters(&request)?;
    reject_on_conflict(&request)?;
    let prefer = Prefer::from_headers(&headers);
    let body = parse_body(&bytes)?;
    let representation = prefer.representation.then_some(&request.select);
    let sql = query::update(
        &catalog.schema,
        table,
        body,
        &request.filters,
        representation,
    )
    .map_err(bad_query)?;
    write(
        &pool,
        &claims,
        sql,
        prefer.representation,
        StatusCode::OK,
        StatusCode::NO_CONTENT,
    )
    .await
}

async fn remove(
    State(pool): State<Pool>,
    State(catalog): State<Arc<CatalogHandle>>,
    Path(name): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    Auth(claims): Auth,
) -> Result<Response, ApiError> {
    let catalog = catalog.get();
    let table = catalog
        .table(&name)
        .ok_or_else(|| not_found("table", &name))?;
    let request = query::parse_request(&pairs(raw), table).map_err(bad_query)?;
    require_filters(&request)?;
    reject_on_conflict(&request)?;
    let prefer = Prefer::from_headers(&headers);
    let representation = prefer.representation.then_some(&request.select);
    let sql = query::delete(&catalog.schema, table, &request.filters, representation);
    write(
        &pool,
        &claims,
        sql,
        prefer.representation,
        StatusCode::OK,
        StatusCode::NO_CONTENT,
    )
    .await
}

async fn rpc(
    State(pool): State<Pool>,
    State(catalog): State<Arc<CatalogHandle>>,
    Path(name): Path<String>,
    Auth(claims): Auth,
    bytes: Bytes,
) -> Result<Response, ApiError> {
    let catalog = catalog.get();
    let candidates = catalog
        .functions
        .get(&name)
        .ok_or_else(|| not_found("function", &name))?;
    let Value::Object(args) = parse_body(&bytes)? else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            "the arguments must be a JSON object",
        ));
    };
    let function = query::resolve_function(candidates, &args).map_err(bad_query)?;
    let sql = query::rpc(&catalog.schema, function, args);

    if function.returns_void {
        in_request_tx!(pool, claims, |tx| { execute(&tx, &sql).await });
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let body = in_request_tx!(pool, claims, |tx| {
        let statement = tx.prepare_cached(&sql.text).await?;
        let row = tx.query_one(&statement, &sql.param_refs()).await?;
        Ok::<_, tokio_postgres::Error>(row.get::<_, Option<String>>(0))
    });
    Ok(json_response(
        StatusCode::OK,
        body.unwrap_or_else(|| "null".into()),
    ))
}
