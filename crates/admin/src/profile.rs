//! The admin's profile photo (`nelcota.admin_avatar`, migration V4).
//!
//! The panel crops and shrinks the image in the browser and sends it as
//! base64. Only PNG, JPEG or WebP are accepted, recognised by their leading
//! bytes (never by the declared type): SVG or HTML posing as an image does not
//! get through, and the response carries the detected type and `nosniff`.

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

/// Same limit as the table's CHECK.
pub const MAX_BYTES: usize = 256 * 1024;

/// Image type from its leading bytes (the format's signature).
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

/// `GET /admin/api/profile`: email and photo version (ms of the last change,
/// or `null`). The version goes into the image URL, which can then be cached.
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
        .ok_or_else(|| ApiError::not_found("no_profile_photo", "no profile photo"))?;
    let content_type: String = row.get(0);
    let image: Vec<u8> = row.get(1);
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            // The URL carries the version (`?v=`): changing the photo changes the URL.
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
    /// Base64 image (without the `data:` prefix).
    image: String,
}

/// `PUT /admin/api/profile/avatar`
pub async fn upload(
    State(state): State<AdminState>,
    Json(body): Json<Upload>,
) -> Result<Json<Value>, ApiError> {
    let bytes = STANDARD
        .decode(body.image.trim())
        .map_err(|_| ApiError::bad_request("invalid_image_base64", "invalid image (base64)"))?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(ApiError::bad_request(
            "photo_too_large",
            format!("the photo must be at most {} KB", MAX_BYTES / 1024),
        )
        .params(json!({ "max_kb": MAX_BYTES / 1024 })));
    }
    let content_type = sniff(&bytes).ok_or_else(|| {
        ApiError::bad_request("unsupported_image_type", "use a PNG, JPEG or WebP image")
    })?;
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
    fn recognises_the_accepted_formats() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("image/png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xe0\0\x10JFIF"), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some("image/webp"));
    }

    #[test]
    fn refuses_everything_else() {
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
