//! Sending email (today only the password recovery link).
//!
//! It sits behind the [`Mailer`] trait: SMTP in production ([`SmtpMailer`]);
//! in tests, a mailer that only keeps the messages. Nelcota runs no mail server
//! of its own: it uses whatever SMTP the project configures (Postmark, SES,
//! Resend, Gmail, the hosting provider's...).

use std::time::Duration;

use futures_util::future::BoxFuture;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
};

/// Plain-text message: links work in any client and there is no HTML to
/// escape.
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

/// SMTP configured by URL, in lettre's format:
/// `smtps://user:password@smtp.example.com:465` (implicit TLS) or
/// `smtp://user:password@smtp.example.com:587?tls=required` (STARTTLS).
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn new(url: &str, from: &str) -> Result<Self, MailError> {
        let from = from
            .parse::<Mailbox>()
            .map_err(|e| MailError(format!("invalid NELCOTA_SMTP_FROM: {e}")))?;
        // lettre's error message does not include the URL (which holds the password).
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|e| MailError(format!("invalid NELCOTA_SMTP_URL: {e}")))?
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
                .map_err(|e| MailError(format!("invalid recipient: {e}")))?;
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
    fn valid_and_invalid_settings() {
        assert!(
            SmtpMailer::new(
                "smtps://u:p@smtp.example.com:465",
                "Shop <no-reply@shop.com>"
            )
            .is_ok()
        );
        assert!(
            SmtpMailer::new(
                "smtp://u:p@smtp.example.com:587?tls=required",
                "no-reply@shop.com"
            )
            .is_ok()
        );
        let err = SmtpMailer::new("http://smtp.example.com", "a@b.com")
            .err()
            .unwrap();
        assert!(err.0.contains("NELCOTA_SMTP_URL"));
        let err = SmtpMailer::new("smtps://u:secret@smtp.example.com", "not an email")
            .err()
            .unwrap();
        assert!(err.0.contains("NELCOTA_SMTP_FROM"));
        assert!(!err.0.contains("secret"));
    }
}
