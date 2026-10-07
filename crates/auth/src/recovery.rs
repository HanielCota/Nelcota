//! Recuperação de senha por email.
//!
//! 1. `POST /auth/v1/recover {email}`: se a conta existe, manda um link para
//!    a página do app (`NELCOTA_PASSWORD_RECOVERY_URL#type=recovery&token=...`).
//!    A resposta é a mesma exista ou não a conta.
//! 2. `POST /auth/v1/verify {type: "recovery", token, password}`: troca a
//!    senha, encerra as outras sessões e devolve uma sessão nova.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use nelcota_core::ApiError;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    Email,
    credentials::{normalize_email, validate_password},
    handlers::{
        AuthState, PeerAddr, begin_auth, client_ip, db_error, invalid, invalid_grant, ip_key,
        limit, new_opaque_token, start_session, user_agent,
    },
};

/// Validade do link.
const TTL_MINUTES: i32 = 60;
/// Intervalo mínimo entre dois emails para a mesma conta: o endpoint é
/// público e não pode servir para lotar a caixa de entrada de alguém.
const COOLDOWN_SECONDS: i32 = 60;

fn disabled() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "recovery_disabled",
        "recuperação de senha desabilitada: o projeto não tem SMTP configurado",
    )
}

fn expired_link() -> ApiError {
    invalid_grant("link de recuperação inválido ou expirado")
}

/// Texto do email. O token vai no fragmento: o navegador não o envia ao
/// servidor do app, então ele não aparece em logs nem no `Referer`.
pub(crate) fn recovery_email(to: &str, recovery_url: &str, token: &str) -> Email {
    let link = format!("{recovery_url}#type=recovery&token={token}");
    Email {
        to: to.to_owned(),
        subject: "Redefinir sua senha".into(),
        text: format!(
            "Recebemos um pedido para redefinir a senha da conta {to}.\n\n\
             Para escolher uma senha nova, abra o link abaixo. Ele vale por 1 hora \
             e só pode ser usado uma vez:\n\n{link}\n\n\
             Se não foi você, ignore este email: sua senha continua a mesma.\n"
        ),
    }
}

#[derive(Deserialize)]
pub(crate) struct RecoverBody {
    email: String,
}

/// `POST /auth/v1/recover` `{email}` → 200 `{}`.
pub(crate) async fn recover(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<RecoverBody>,
) -> Result<Json<Value>, ApiError> {
    let (Some(mailer), Some(recovery_url)) =
        (state.mailer.clone(), state.settings.recovery_url.clone())
    else {
        return Err(disabled());
    };
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("recover:{}", ip_key(ip)))?;
    let email = normalize_email(&body.email).map_err(invalid)?;

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
            // Um link por vez: pedir de novo invalida os anteriores.
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
            outgoing = Some(recovery_email(&email, &recovery_url, &token));
        }
    }
    tx.commit().await.map_err(db_error)?;

    if let Some(message) = outgoing {
        // Em segundo plano: o tempo de resposta não revela se a conta existe,
        // e uma falha do SMTP não vira erro para quem pediu (fica no log).
        tokio::spawn(async move {
            if let Err(err) = mailer.send(message).await {
                tracing::error!(error = %err, "falha ao enviar o email de recuperação de senha");
            }
        });
    }
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub(crate) struct VerifyBody {
    #[serde(rename = "type")]
    kind: String,
    token: String,
    password: String,
}

/// `POST /auth/v1/verify` `{type: "recovery", token, password}` → 200 + sessão.
pub(crate) async fn verify(
    State(state): State<AuthState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    Json(body): Json<VerifyBody>,
) -> Result<Json<Value>, ApiError> {
    let ip = client_ip(&state.settings, &headers, peer);
    limit(&state, &format!("verify:{}", ip_key(ip)))?;
    if body.kind != "recovery" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_type",
            "type deve ser recovery",
        ));
    }
    if body.token.is_empty() || body.token.len() > 128 {
        return Err(expired_link());
    }
    validate_password(&body.password).map_err(invalid)?;
    let hash = Sha256::digest(body.token.as_bytes()).to_vec();

    // Confere o link antes do argon2 (caro): link inválido não gasta CPU.
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
    // Consumo atômico: dois envios do mesmo link não trocam a senha duas vezes.
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
    // Abrir o link prova que a pessoa recebe os emails da conta.
    tx.execute(
        "UPDATE auth.users
            SET encrypted_password = $2, updated_at = now(),
                email_confirmed_at = coalesce(email_confirmed_at, now())
          WHERE id = $1",
        &[&user_id, &phc],
    )
    .await
    .map_err(db_error)?;
    // Quem entrou com a senha antiga perde o acesso (os refresh tokens morrem
    // na hora; JWTs de acesso já emitidos valem até expirar).
    tx.execute(
        "UPDATE auth.sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        &[&user_id],
    )
    .await
    .map_err(db_error)?;
    let session = start_session(&state, &tx, user_id, ip, user_agent(&headers)).await?;
    tx.commit().await.map_err(db_error)?;
    tracing::info!(user_id = %user_id, "senha redefinida pelo link de recuperação");
    Ok(Json(session))
}

#[cfg(test)]
mod tests {
    use super::recovery_email;

    #[test]
    fn link_leva_o_token_no_fragmento() {
        let email = recovery_email("ana@x.com", "https://app.x.com/nova-senha", "abc_-123");
        assert_eq!(email.to, "ana@x.com");
        assert!(
            email
                .text
                .contains("https://app.x.com/nova-senha#type=recovery&token=abc_-123")
        );
        assert!(email.text.contains("ana@x.com"));
    }
}
