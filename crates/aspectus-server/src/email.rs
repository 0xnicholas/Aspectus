//! Email delivery abstraction for Aspectus.
//!
//! The MVP only needs password-reset emails. Production deployments should
//! configure SMTP; development and tests can use the logging transport.

use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::{AsyncTransport, Message};

/// Sends transactional emails to users.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Send a password-reset email containing the one-time reset URL.
    /// Implementations must NOT log or expose the URL in error messages
    /// unless they are the explicit logging transport.
    async fn send_password_reset(&self, email: &str, reset_url: &str) -> Result<(), String>;
}

/// Development/test transport: logs the reset URL instead of sending email.
/// This is the default when no SMTP is configured.
#[derive(Clone)]
pub struct LoggingEmailSender;

#[async_trait]
impl EmailSender for LoggingEmailSender {
    async fn send_password_reset(&self, email: &str, _reset_url: &str) -> Result<(), String> {
        // NEVER log _reset_url — it contains a one-time secret token.
        // Logging transport records that a reset was dispatched, not the link.
        tracing::info!(
            email = %email,
            "Password reset email dispatched (logging transport; URL omitted)"
        );
        Ok(())
    }
}

/// SMTP transport using Lettre.
#[derive(Clone)]
pub struct SmtpEmailSender {
    transport: lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    from: Mailbox,
}

impl SmtpEmailSender {
    /// Build an SMTP sender from environment variables:
    ///   ASPECTUS_SMTP_HOST, ASPECTUS_SMTP_PORT (default 587),
    ///   ASPECTUS_SMTP_USERNAME, ASPECTUS_SMTP_PASSWORD,
    ///   ASPECTUS_EMAIL_FROM (default noreply@aspectus.local).
    pub fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var("ASPECTUS_SMTP_HOST")
            .map_err(|_| anyhow::anyhow!("ASPECTUS_SMTP_HOST is required for SMTP transport"))?;
        let port: u16 = std::env::var("ASPECTUS_SMTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(587);
        let username = std::env::var("ASPECTUS_SMTP_USERNAME").map_err(|_| {
            anyhow::anyhow!("ASPECTUS_SMTP_USERNAME is required for SMTP transport")
        })?;
        let password = std::env::var("ASPECTUS_SMTP_PASSWORD").map_err(|_| {
            anyhow::anyhow!("ASPECTUS_SMTP_PASSWORD is required for SMTP transport")
        })?;
        let from: Mailbox = std::env::var("ASPECTUS_EMAIL_FROM")
            .unwrap_or_else(|_| "Aspectus <noreply@aspectus.local>".into())
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid ASPECTUS_EMAIL_FROM: {e}"))?;

        let credentials =
            lettre::transport::smtp::authentication::Credentials::new(username, password);
        let transport = lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(&host)
            .map_err(|e| anyhow::anyhow!("Invalid SMTP relay: {e}"))?
            .port(port)
            .credentials(credentials)
            .build();

        Ok(Self { transport, from })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send_password_reset(&self, email: &str, reset_url: &str) -> Result<(), String> {
        let to: Mailbox = email
            .parse()
            .map_err(|e| format!("Invalid recipient email: {e}"))?;

        let body = format!(
            "You requested a password reset for your Aspectus account.\n\n\
             Click the link below to reset your password (expires in 1 hour):\n\n\
             {reset_url}\n\n\
             If you did not request this, you can safely ignore this email."
        );

        let email_msg = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject("Reset your Aspectus password")
            .body(body)
            .map_err(|e| format!("Failed to build email: {e}"))?;

        self.transport
            .send(email_msg)
            .await
            .map_err(|e| format!("SMTP send failed: {e}"))?;

        Ok(())
    }
}
