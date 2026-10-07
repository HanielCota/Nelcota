//! Contas dos usuários finais pelo painel: criar e redefinir senha. As regras
//! de email e senha são as mesmas do cadastro público (`nelcota_auth`).
//!
//! Não há convite nem confirmação de email: o nelcota não envia emails e o
//! login não exige `email_confirmed_at`. Botões para isso não teriam efeito.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use nelcota_auth::{InvalidCredential, hash_password, normalize_email, validate_password};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::error::SqlState;

use crate::{AdminState, ApiError, api::is_uuid};

/// Regra de credencial violada vira 400. Função (e não `From`): o tipo é de
/// outro crate e conflitaria com a conversão genérica de erros do `ApiError`.
fn invalid(err: InvalidCredential) -> ApiError {
    ApiError::bad_request(err.0)
}

/// argon2 consome CPU por dezenas de ms: fora das threads do runtime async.
async fn hash(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await?
        .ok_or_else(|| ApiError::from("falha ao gerar o hash da senha"))
}

#[derive(Deserialize)]
pub struct CreateUser {
    email: String,
    password: String,
}

/// `POST /admin/api/users`
pub async fn create(
    State(state): State<AdminState>,
    Json(body): Json<CreateUser>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let email = normalize_email(&body.email).map_err(invalid)?;
    validate_password(&body.password).map_err(invalid)?;
    let hash = hash(body.password).await?;
    let client = state.db.get().await?;
    let row = client
        .query_one(
            "INSERT INTO auth.users (email, encrypted_password) VALUES ($1, $2) RETURNING id::text",
            &[&email, &hash],
        )
        .await
        .map_err(|err| {
            if err.code() == Some(&SqlState::UNIQUE_VIOLATION) {
                ApiError::conflict("já existe um usuário com este email")
            } else {
                ApiError::from(err)
            }
        })?;
    // Só o fato vai para o log; email e senha, não.
    tracing::info!("usuário criado pelo painel");
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": row.get::<_, String>(0), "email": email, "message": "usuário criado" })),
    ))
}

#[derive(Deserialize)]
pub struct SetPassword {
    password: String,
}

/// `PUT /admin/api/users/{id}/password`: troca a senha e encerra as sessões
/// abertas (quem redefine costuma suspeitar de acesso indevido).
pub async fn set_password(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(body): Json<SetPassword>,
) -> Result<Json<Value>, ApiError> {
    if !is_uuid(&id) {
        return Err(ApiError::bad_request("id inválido"));
    }
    validate_password(&body.password).map_err(invalid)?;
    let hash = hash(body.password).await?;
    let mut client = state.db.get().await?;
    let tx = client.transaction().await?;
    let updated = tx
        .execute(
            "UPDATE auth.users SET encrypted_password = $2, updated_at = now() WHERE id = $1::text::uuid",
            &[&id, &hash],
        )
        .await?;
    if updated == 0 {
        return Err(ApiError::not_found("usuário não encontrado"));
    }
    let sessions = tx
        .execute(
            "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1::text::uuid AND revoked_at IS NULL",
            &[&id],
        )
        .await?;
    tx.commit().await?;
    tracing::info!(sessoes = sessions, "senha redefinida pelo painel");
    Ok(Json(json!({
        "message": format!("senha redefinida; {sessions} sessão(ões) encerrada(s)"),
        "sessions_revoked": sessions,
    })))
}
