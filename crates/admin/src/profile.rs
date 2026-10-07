//! Foto de perfil do admin (`nelcota.admin_avatar`, migração V4).
//!
//! O painel recorta e reduz a imagem no navegador e envia em base64. Aqui só
//! se aceita PNG, JPEG ou WebP, reconhecidos pelos bytes iniciais (nunca pelo
//! tipo declarado): SVG ou HTML disfarçados de imagem não passam, e a resposta
//! sai com o tipo detectado e `nosniff`.

use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AdminState, ApiError};

/// Mesmo limite do CHECK da tabela.
pub const MAX_BYTES: usize = 256 * 1024;

/// Tipo da imagem pelos bytes iniciais (assinatura do formato).
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

fn owner(state: &AdminState) -> String {
    state.credentials.email.trim().to_lowercase()
}

/// `GET /admin/api/profile`: email e versão da foto (ms da última troca, ou
/// `null`). A versão entra na URL da imagem, que então pode ficar em cache.
pub async fn get(State(state): State<AdminState>) -> Result<Json<Value>, ApiError> {
    let client = state.db.get().await?;
    let row = client
        .query_opt(
            "SELECT (extract(epoch FROM updated_at) * 1000)::bigint
               FROM nelcota.admin_avatar WHERE email = $1",
            &[&owner(&state)],
        )
        .await?;
    let version: Option<i64> = row.map(|r| r.get(0));
    Ok(Json(
        json!({ "email": state.credentials.email, "avatar": version }),
    ))
}

/// `GET /admin/api/profile/avatar`
pub async fn image(State(state): State<AdminState>) -> Result<Response, ApiError> {
    let client = state.db.get().await?;
    let row = client
        .query_opt(
            "SELECT content_type, image FROM nelcota.admin_avatar WHERE email = $1",
            &[&owner(&state)],
        )
        .await?
        .ok_or_else(|| ApiError::not_found("sem foto de perfil"))?;
    let content_type: String = row.get(0);
    let image: Vec<u8> = row.get(1);
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            // A URL leva a versão (`?v=`): trocar a foto muda a URL.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_owned(),
            ),
        ],
        image,
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct Upload {
    /// Imagem em base64 (sem o prefixo `data:`).
    image: String,
}

/// `PUT /admin/api/profile/avatar`
pub async fn upload(
    State(state): State<AdminState>,
    Json(body): Json<Upload>,
) -> Result<Json<Value>, ApiError> {
    let bytes = STANDARD
        .decode(body.image.trim())
        .map_err(|_| ApiError::bad_request("imagem inválida (base64)"))?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(ApiError::bad_request(format!(
            "a foto deve ter até {} KB",
            MAX_BYTES / 1024
        )));
    }
    let content_type =
        sniff(&bytes).ok_or_else(|| ApiError::bad_request("use uma imagem PNG, JPEG ou WebP"))?;
    let client = state.db.get().await?;
    let row = client
        .query_one(
            "INSERT INTO nelcota.admin_avatar (email, content_type, image)
             VALUES ($1, $2, $3)
             ON CONFLICT (email) DO UPDATE
                SET content_type = excluded.content_type,
                    image = excluded.image,
                    updated_at = now()
             RETURNING (extract(epoch FROM updated_at) * 1000)::bigint",
            &[&owner(&state), &content_type, &bytes],
        )
        .await?;
    let version: i64 = row.get(0);
    Ok(Json(json!({ "avatar": version })))
}

/// `DELETE /admin/api/profile/avatar`
pub async fn remove(State(state): State<AdminState>) -> Result<Json<Value>, ApiError> {
    let client = state.db.get().await?;
    client
        .execute(
            "DELETE FROM nelcota.admin_avatar WHERE email = $1",
            &[&owner(&state)],
        )
        .await?;
    Ok(Json(json!({ "avatar": null })))
}

#[cfg(test)]
mod tests {
    use super::sniff;

    #[test]
    fn reconhece_os_formatos_aceitos() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("image/png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xe0\0\x10JFIF"), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some("image/webp"));
    }

    #[test]
    fn recusa_o_resto() {
        for bytes in [
            &b"<svg xmlns='http://www.w3.org/2000/svg'/>"[..],
            b"<!doctype html><script>",
            b"GIF89a",
            b"RIFF\x24\0\0\0WAVEfmt ",
            b"",
            b"\x89PN",
        ] {
            assert_eq!(sniff(bytes), None, "{bytes:?}");
        }
    }
}
