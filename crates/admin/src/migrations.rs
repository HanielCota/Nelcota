//! HTTP routes for the project's migrations.
mod files;
mod operations;
pub use files::default_dir;

use crate::{
    AdminState, ApiError,
    contracts::{ExportedMigration, MigrationsData},
};
use axum::{
    Json,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ExportRequest {
    name: String,
}

pub async fn list(State(state): State<AdminState>) -> Result<Json<MigrationsData>, ApiError> {
    Ok(Json(
        operations::list(&state.db, state.migrations_dir.as_deref()).await?,
    ))
}

pub async fn export(
    State(state): State<AdminState>,
    Json(body): Json<ExportRequest>,
) -> Result<Json<ExportedMigration>, ApiError> {
    Ok(Json(
        operations::generate(&state.db, state.migrations_dir.as_deref(), &body.name).await?,
    ))
}

pub async fn file(
    State(state): State<AdminState>,
    Path(version): Path<i32>,
) -> Result<Response, ApiError> {
    let file = operations::file(&state.db, version).await?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/sql; charset=utf-8".to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file.filename),
            ),
        ],
        file.sql,
    )
        .into_response())
}
