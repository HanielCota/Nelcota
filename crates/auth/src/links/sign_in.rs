//! Signing in by opening a signup confirmation or a magic link.
use std::net::IpAddr;

use nelcota_core::ApiError;
use serde_json::Value;

use super::{LinkKind, tokens};
use crate::{
    AuthState,
    db::{begin_auth, db_error},
    error::invalid_grant,
    sessions::start_session,
};

fn expired_link() -> ApiError {
    invalid_grant("invalid or expired link")
}

/// Consumes the link, marks the email as confirmed and starts a session.
pub(crate) async fn sign_in(
    state: &AuthState,
    kind: LinkKind,
    token: &str,
    ip: Option<IpAddr>,
    user_agent: Option<&str>,
) -> Result<Value, ApiError> {
    debug_assert_ne!(kind, LinkKind::Recovery, "recovery also sets a password");
    let hash = tokens::hash(token).ok_or_else(expired_link)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let user_id = tokens::consume(&tx, kind, &hash)
        .await?
        .ok_or_else(expired_link)?;
    // Opening the link proves the person receives the account's emails.
    tx.execute(
        "UPDATE auth.users SET email_confirmed_at = coalesce(email_confirmed_at, now())
         WHERE id = $1",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    let session = start_session(state, &tx, user_id, ip, user_agent).await?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(user_id = %user_id, kind = kind.as_str(), "signed in through an email link");
    Ok(session)
}
