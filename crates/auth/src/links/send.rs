//! Requesting links: the public `recover`, `magiclink` and `resend` endpoints.
use std::sync::Arc;

use axum::http::StatusCode;
use nelcota_core::ApiError;
use uuid::Uuid;

use super::{LinkKind, emails::compose, tokens::issue};
use crate::{
    AuthState, Mailer,
    credentials::normalize_email,
    db::{begin_auth, db_error},
    error::invalid,
};

fn disabled(kind: LinkKind) -> ApiError {
    let (code, message) = match kind {
        LinkKind::Recovery => (
            "recovery_disabled",
            "password recovery is disabled: the project has no SMTP configured",
        ),
        LinkKind::Signup => (
            "confirmation_disabled",
            "email confirmation is disabled in this project",
        ),
        LinkKind::MagicLink => (
            "magiclink_disabled",
            "magic link sign-in is disabled in this project",
        ),
    };
    ApiError::new(StatusCode::FORBIDDEN, code, message)
}

/// The mailer and the app page of `kind`, or the error saying the flow is off
/// (no SMTP, or no page configured for it).
pub(crate) fn ensure_enabled(
    state: &AuthState,
    kind: LinkKind,
) -> Result<(&Arc<dyn Mailer>, &str), ApiError> {
    match (&state.mailer, state.settings.links.url(kind)) {
        (Some(mailer), Some(url)) => Ok((mailer, url)),
        _ => Err(disabled(kind)),
    }
}

/// Emails a link of `kind` to the account behind `email`, when it is eligible.
/// The outcome is the same either way, so the endpoint does not reveal which
/// accounts exist.
pub(crate) async fn send(state: &AuthState, kind: LinkKind, email: &str) -> Result<(), ApiError> {
    ensure_enabled(state, kind)?;
    let email = normalize_email(email).map_err(invalid)?;

    let mut client = state.pool.get().await.map_err(ApiError::from_pool)?;
    let tx = begin_auth(&mut client).await?;
    let user: Option<Uuid> = tx
        .query_opt(
            "SELECT id FROM auth.users
             WHERE email = $1 AND (email_confirmed_at IS NULL OR NOT $2)",
            &[&email, &kind.only_unconfirmed()],
        )
        .await
        .map_err(db_error)?
        .map(|row| row.get(0));
    let token = match user {
        Some(user_id) => issue(&tx, user_id, kind).await?,
        None => None,
    };
    tx.commit().await.map_err(db_error)?;

    if let Some(token) = token {
        deliver(state, kind, &email, &token);
    }
    Ok(())
}

/// Sends a committed link in the background: the response time does not
/// reveal whether the account exists, and an SMTP failure is not the
/// requester's error (it goes to the log).
pub(crate) fn deliver(state: &AuthState, kind: LinkKind, to: &str, token: &str) {
    // Links are only issued while their flow is enabled.
    let Ok((mailer, page_url)) = ensure_enabled(state, kind) else {
        return;
    };
    let message = compose(kind, to, page_url, token);
    let mailer = mailer.clone();
    tokio::spawn(async move {
        if let Err(err) = mailer.send(message).await {
            tracing::error!(error = %err, kind = kind.as_str(), "failed to send a link email");
        }
    });
}
