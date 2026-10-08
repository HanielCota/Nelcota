//! Password recovery by email.
//!
//! 1. `POST /auth/v1/recover {email}`: if the account exists, sends a link to
//!    the app page (`NELCOTA_PASSWORD_RECOVERY_URL#type=recovery&token=...`).
//!    The response is the same whether or not the account exists.
//! 2. `POST /auth/v1/verify {type: "recovery", token, password}`: changes the
//!    password, ends the other sessions and returns a new session.

use axum::http::StatusCode;
use nelcota_core::ApiError;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use uuid::Uuid;

use crate::{
    AuthState, Email,
    credentials::{normalize_email, validate_password},
    db::{begin_auth, db_error},
    error::{invalid, invalid_grant},
    sessions::{new_opaque_token, start_session},
};

/// Lifetime of the link.
const TTL_MINUTES: i32 = 60;
/// Minimum interval between two emails to the same account: the endpoint is
/// public and must not be usable to flood someone's inbox.
const COOLDOWN_SECONDS: i32 = 60;

fn disabled() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "recovery_disabled",
        "password recovery is disabled: the project has no SMTP configured",
    )
}

fn expired_link() -> ApiError {
    invalid_grant("invalid or expired recovery link")
}

/// Email text. The token goes in the fragment: the browser does not send it to
/// the app's server, so it never shows in logs or the `Referer`.
pub(crate) fn recovery_email(to: &str, recovery_url: &str, token: &str) -> Email {
    let link = format!("{recovery_url}#type=recovery&token={token}");
    Email {
        to: to.to_owned(),
        subject: "Reset your password".into(),
        text: format!(
            "We received a request to reset the password of the account {to}.\n\n\
             To choose a new password, open the link below. It is valid for 1 hour \
             and can be used only once:\n\n{link}\n\n\
             If this was not you, ignore this email: your password stays the same.\n"
        ),
    }
}

pub(crate) fn ensure_enabled(state: &AuthState) -> Result<(), ApiError> {
    if state.mailer.is_none() || state.settings.recovery_url.is_none() {
        return Err(disabled());
    }
    Ok(())
}

/// Issues a link after committing its token; delivery does not reveal accounts.
pub(crate) async fn request(state: &AuthState, email: &str) -> Result<(), ApiError> {
    ensure_enabled(state)?;
    let mailer = state.mailer.clone().ok_or_else(disabled)?;
    let recovery_url = state
        .settings
        .recovery_url
        .as_deref()
        .ok_or_else(disabled)?;
    let email = normalize_email(email).map_err(invalid)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let user: Option<Uuid> = tx
        .query_opt("SELECT id FROM auth.users WHERE email = $1", &[&email])
        .await
        .map_err(db_error)?
        .map(|row| row.get(0));
    let mut outgoing = None;
    if let Some(user_id) = user {
        let recent: bool = tx
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM auth.one_time_tokens
                    WHERE user_id = $1 AND kind = 'recovery'
                      AND created_at > now() - make_interval(secs => $2))",
                &[&user_id, &f64::from(COOLDOWN_SECONDS)],
            )
            .await
            .map_err(db_error)?
            .get(0);
        if !recent {
            // One link at a time: asking again voids the previous ones.
            tx.execute(
                "DELETE FROM auth.one_time_tokens WHERE user_id = $1 AND kind = 'recovery'",
                &[&user_id],
            )
            .await
            .map_err(db_error)?;
            let (token, hash) = new_opaque_token();
            tx.execute(
                "INSERT INTO auth.one_time_tokens (user_id, kind, token_hash, expires_at)
                 VALUES ($1, 'recovery', $2, now() + make_interval(mins => $3))",
                &[&user_id, &hash, &TTL_MINUTES],
            )
            .await
            .map_err(db_error)?;
            outgoing = Some(recovery_email(&email, recovery_url, &token));
        }
    }
    tx.commit().await.map_err(db_error)?;

    if let Some(message) = outgoing {
        // In the background: the response time does not reveal whether the
        // account exists, and an SMTP failure is not an error for the requester
        // (it goes to the log).
        tokio::spawn(async move {
            if let Err(err) = mailer.send(message).await {
                tracing::error!(error = %err, "failed to send the password recovery email");
            }
        });
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct VerifyBody {
    #[serde(rename = "type")]
    kind: String,
    token: String,
    password: String,
}

/// Consumes a link and replaces the password and session atomically.
pub(crate) async fn complete(
    state: &AuthState,
    body: VerifyBody,
    ip: Option<IpAddr>,
    agent: Option<&str>,
) -> Result<Value, ApiError> {
    if body.kind != "recovery" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_type",
            "type must be recovery",
        ));
    }
    if body.token.is_empty() || body.token.len() > 128 {
        return Err(expired_link());
    }
    validate_password(&body.password).map_err(invalid)?;
    let hash = Sha256::digest(body.token.as_bytes()).to_vec();

    // Check the link before argon2 (expensive): an invalid link costs no CPU.
    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    {
        let tx = begin_auth(&mut client).await?;
        let valid = tx
            .query_opt(
                "SELECT 1 FROM auth.one_time_tokens
                 WHERE token_hash = $1 AND kind = 'recovery'
                   AND used_at IS NULL AND expires_at > now()",
                &[&hash],
            )
            .await
            .map_err(db_error)?
            .is_some();
        tx.commit().await.map_err(db_error)?;
        if !valid {
            return Err(expired_link());
        }
    }
    drop(client);

    let phc = state
        .passwords
        .hash(body.password)
        .await
        .ok_or_else(ApiError::internal)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    // Atomic consumption: two submissions of the same link do not change the
    // password twice.
    let user_id: Uuid = tx
        .query_opt(
            "UPDATE auth.one_time_tokens SET used_at = now()
             WHERE token_hash = $1 AND kind = 'recovery'
               AND used_at IS NULL AND expires_at > now()
             RETURNING user_id",
            &[&hash],
        )
        .await
        .map_err(db_error)?
        .ok_or_else(expired_link)?
        .get(0);
    // Opening the link proves the person receives the account's emails.
    tx.execute(
        "UPDATE auth.users
            SET encrypted_password = $2, updated_at = now(),
                email_confirmed_at = coalesce(email_confirmed_at, now())
          WHERE id = $1",
        &[&user_id, &phc],
    )
    .await
    .map_err(db_error)?;
    // Whoever signed in with the old password loses access (refresh tokens die
    // immediately; access JWTs already issued stay valid until they expire).
    tx.execute(
        "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    let session = start_session(state, &tx, user_id, ip, agent).await?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(user_id = %user_id, "password reset through the recovery link");
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::recovery_email;

    #[test]
    fn link_carries_the_token_in_the_fragment() {
        let email = recovery_email("ana@x.com", "https://app.x.com/new-password", "abc_-123");
        assert_eq!(email.to, "ana@x.com");
        assert!(
            email
                .text
                .contains("https://app.x.com/new-password#type=recovery&token=abc_-123")
        );
        assert!(email.text.contains("ana@x.com"));
    }
}
