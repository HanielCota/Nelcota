//! Embedded panel files and their cache policy.
use axum::{
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
/// Frontend build (`npm run build` in `crates/admin/ui`).
#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Ui;

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn ui_file(path: &str) -> Response {
    let Some(file) = Ui::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        [(header::CONTENT_TYPE, content_type(path))],
        Body::from(file.data.into_owned()),
    )
        .into_response()
}

/// Files with a hash in their name (built by Vite): long, immutable cache.
pub(crate) async fn asset(Path(file): Path<String>) -> Response {
    let mut response = ui_file(&format!("assets/{file}"));
    if response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// Every panel route returns the SPA's `index.html` (client-side routing).
/// The page itself holds no data: everything comes from the authenticated API.
pub(crate) async fn spa() -> Response {
    let mut response = ui_file("index.html");
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}
