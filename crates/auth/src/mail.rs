//! Envio de email (hoje só o link de recuperação de senha).
//!
//! Fica atrás da trait [`Mailer`]: em produção, SMTP ([`SmtpMailer`]); nos
//! testes, um carteiro que só guarda as mensagens. O Nelcota não tem servidor
//! de email próprio: usa o SMTP que o projeto configurar (Postmark, SES,
//! Resend, Gmail, o do provedor de hospedagem...).

use std::time::Duration;

use futures_util::future::BoxFuture;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
};

/// Mensagem em texto puro: links funcionam em qualquer cliente e não há HTML
/// para escapar.
#[derive(Clone, Debug)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MailError(pub String);

pub trait Mailer: Send + Sync + 'static {
    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), MailError>>;
}

/// SMTP configurado por URL, no formato do lettre:
/// `smtps://usuario:senha@smtp.exemplo.com:465` (TLS direto) ou
/// `smtp://usuario:senha@smtp.exemplo.com:587?tls=required` (STARTTLS).
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn new(url: &str, from: &str) -> Result<Self, MailError> {
        let from = from
            .parse::<Mailbox>()
            .map_err(|e| MailError(format!("NELCOTA_SMTP_FROM inválido: {e}")))?;
        // A mensagem de erro do lettre não inclui a URL (que tem a senha).
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|e| MailError(format!("NELCOTA_SMTP_URL inválida: {e}")))?
            .timeout(Some(Duration::from_secs(15)))
            .build();
        Ok(SmtpMailer { transport, from })
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, email: Email) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            let to = email
                .to
                .parse::<Mailbox>()
                .map_err(|e| MailError(format!("destinatário inválido: {e}")))?;
            let message = Message::builder()
                .from(self.from.clone())
                .to(to)
                .subject(email.subject)
                .header(ContentType::TEXT_PLAIN)
                .body(email.text)
                .map_err(|e| MailError(e.to_string()))?;
            self.transport
                .send(message)
                .await
                .map(|_| ())
                .map_err(|e| MailError(e.to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SmtpMailer;

    #[test]
    fn configuracao_valida_e_invalida() {
        assert!(
            SmtpMailer::new(
                "smtps://u:p@smtp.exemplo.com:465",
                "Loja <nao-responda@loja.com>"
            )
            .is_ok()
        );
        assert!(
            SmtpMailer::new(
                "smtp://u:p@smtp.exemplo.com:587?tls=required",
                "nao-responda@loja.com"
            )
            .is_ok()
        );
        let err = SmtpMailer::new("http://smtp.exemplo.com", "a@b.com")
            .err()
            .unwrap();
        assert!(err.0.contains("NELCOTA_SMTP_URL"));
        let err = SmtpMailer::new("smtps://u:segredo@smtp.exemplo.com", "não é email")
            .err()
            .unwrap();
        assert!(err.0.contains("NELCOTA_SMTP_FROM"));
        assert!(!err.0.contains("segredo"));
    }
}
