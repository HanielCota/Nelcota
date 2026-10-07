//! Handlers de policies: criar, editar e apagar. SQL em `ddl::policy`,
//! execução em `apply`. Ligar o RLS da tabela é uma alteração de tabela
//! (`PATCH /tables/{nome}` com `set_rls`).

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    AdminState, ApiError,
    api::table_or_404,
    apply::apply,
    ddl::policy::{self, PolicyDef},
};

type ApiResult = Result<Json<Value>, ApiError>;

#[derive(Deserialize)]
pub struct PolicyRequest {
    policy: PolicyDef,
    #[serde(default)]
    preview: bool,
}

/// `POST /admin/api/tables/{name}/policies`
pub async fn create(
    State(state): State<AdminState>,
    Path(name): Path<String>,
    Json(body): Json<PolicyRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let statements = policy::create(&schema, &table.name, &body.policy)?;
    let message = format!("policy '{}' criada", body.policy.name);
    apply(&state, statements, body.preview, &message).await
}

/// `PUT /admin/api/tables/{name}/policies/{policy}`
pub async fn replace(
    State(state): State<AdminState>,
    Path((name, original)): Path<(String, String)>,
    Json(body): Json<PolicyRequest>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let statements = policy::replace(&schema, &table.name, &original, &body.policy)?;
    let message = format!("policy '{}' atualizada", body.policy.name);
    apply(&state, statements, body.preview, &message).await
}

#[derive(Deserialize)]
pub struct DropQuery {
    #[serde(default)]
    preview: bool,
}

/// `DELETE /admin/api/tables/{name}/policies/{policy}`
pub async fn drop(
    State(state): State<AdminState>,
    Path((name, policy_name)): Path<(String, String)>,
    Query(params): Query<DropQuery>,
) -> ApiResult {
    let table = table_or_404(&state, &name)?;
    let schema = state.catalog.get().schema.clone();
    let statements = policy::drop(&schema, &table.name, &policy_name);
    let message = format!("policy '{policy_name}' apagada");
    apply(&state, statements, params.preview, &message).await
}
