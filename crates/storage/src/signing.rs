//! Signed-file grants: authorization, token issuing and grant verification.
use crate::{
    StorageState, db,
    error::{object_not_found, store_failed},
    path,
};
use axum::http::StatusCode;
use nelcota_core::{ApiError, Claims, db::begin_request};
use serde_json::json;
/// Longest a signed URL may live: a week.
const MAX_SIGNED_SECS: u64 = 7 * 86_400;
const SIGNED_TYP: &str = "nelcota-storage-url";
pub(crate) async fn token(
    state: &StorageState,
    claims: &Claims,
    bucket: &str,
    name: &str,
    expires_in: u64,
) -> Result<String, ApiError> {
    let bucket = path::bucket(bucket)?;
    let name = path::object(name)?;
    if !(1..=MAX_SIGNED_SECS).contains(&expires_in) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_expiry",
            format!("expires_in between 1 and {MAX_SIGNED_SECS} seconds"),
        ));
    }
    let visible = {
        let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
        let tx = begin_request(&mut client, claims)
            .await
            .map_err(|e| ApiError::from_db(e, claims.role()))?;
        tx.query_opt(
            "SELECT 1 FROM storage.objects WHERE bucket_id = $1 AND name = $2",
            &[&bucket, &name],
        )
        .await
        .map_err(|e| ApiError::from_db(e, claims.role()))?
        .is_some()
    };
    if !visible {
        return Err(object_not_found());
    }
    let now = jsonwebtoken_now();
    let token = state
        .keys
        .sign(&json!({
            "typ": SIGNED_TYP,
            "bucket": bucket,
            "name": name,
            "iat": now,
            "exp": now + expires_in,
        }))
        .map_err(store_failed)?;
    Ok(token)
}

fn jsonwebtoken_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn remaining_secs(payload: &serde_json::Value, now: u64) -> Option<u64> {
    payload["exp"]
        .as_u64()?
        .checked_sub(now)
        .filter(|&secs| secs > 0)
}

pub(crate) async fn resolve(
    state: &StorageState,
    bucket: &str,
    name: &str,
    token: Option<&str>,
) -> Result<(db::Object, u64), ApiError> {
    let bucket = path::bucket(bucket)?;
    let name = path::object(name)?;
    let invalid = || {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_signature",
            "invalid or expired signed URL",
        )
    };
    let payload = token
        .and_then(|t| state.keys.verify_payload(t).ok())
        .ok_or_else(invalid)?;
    let matches = payload["typ"] == SIGNED_TYP
        && payload["bucket"] == bucket
        && payload["name"].as_str() == Some(name.as_str());
    if !matches {
        return Err(invalid());
    }
    // File grants have explicit deadlines; API JWT clock leeway does not apply.
    remaining_secs(&payload, jsonwebtoken_now()).ok_or_else(invalid)?;
    let object = db::object(&state.pool, bucket, &name)
        .await?
        .ok_or_else(object_not_found)?;
    // A slow catalog lookup must not hand S3 an already expired grant.
    let secs = remaining_secs(&payload, jsonwebtoken_now()).ok_or_else(invalid)?;
    Ok((object, secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_deadlines_have_no_clock_leeway() {
        let payload = json!({ "exp": 100 });
        assert_eq!(remaining_secs(&payload, 99), Some(1));
        assert_eq!(remaining_secs(&payload, 100), None);
        assert_eq!(remaining_secs(&payload, 101), None);
        assert_eq!(remaining_secs(&payload, 129), None);
        for malformed in [json!({}), json!({ "exp": -1 }), json!({ "exp": "100" })] {
            assert_eq!(remaining_secs(&malformed, 99), None);
        }
    }
}
