//! The "Connect an app" page's download: the schema's TypeScript types, the
//! same file `nelcota types` writes, so a project can be typed without the CLI.
use crate::AdminState;
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

/// `GET /admin/api/typescript`
pub async fn typescript(State(state): State<AdminState>) -> Response {
    let code = nelcota_api::typescript::generate(&state.catalog.get());
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"database.ts\"",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        code,
    )
        .into_response()
}
