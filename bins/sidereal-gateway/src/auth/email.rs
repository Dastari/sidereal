use async_trait::async_trait;
use lettre::message::{Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use tokio::sync::RwLock;
use tracing::info;

use crate::auth::error::AuthError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailTemplate {
    PasswordReset,
    EmailLogin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub body_text: String,
    pub template: EmailTemplate,
}

#[async_trait]
pub trait EmailDelivery: Send + Sync {
    async fn send(&self, message: EmailMessage) -> Result<(), AuthError>;
}

#[derive(Debug, Default)]
pub struct NoopEmailDelivery;

#[async_trait]
impl EmailDelivery for NoopEmailDelivery {
    async fn send(&self, _message: EmailMessage) -> Result<(), AuthError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct LogEmailDelivery;

#[async_trait]
impl EmailDelivery for LogEmailDelivery {
    async fn send(&self, message: EmailMessage) -> Result<(), AuthError> {
        info!(
            "gateway local-dev email delivery template={:?} to={} subject={} body={}",
            message.template, message.to, message.subject, message.body_text
        );
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SmtpEmailDelivery {
    mailer: SmtpTransport,
    from: Mailbox,
}

impl SmtpEmailDelivery {
    pub fn from_env() -> Result<Option<Self>, AuthError> {
        let mode = std::env::var("GATEWAY_EMAIL_DELIVERY")
            .unwrap_or_else(|_| "noop".to_string())
            .trim()
            .to_ascii_lowercase();
        if mode != "smtp" {
            return Ok(None);
        }

        let relay = required_env("GATEWAY_SMTP_RELAY")?;
        let username = required_env("GATEWAY_SMTP_USERNAME")?;
        let password = required_env("GATEWAY_SMTP_PASSWORD")?;
        let from = required_env("GATEWAY_SMTP_FROM")?
            .parse::<Mailbox>()
            .map_err(|err| AuthError::Config(format!("GATEWAY_SMTP_FROM is invalid: {err}")))?;
        let mailer = SmtpTransport::relay(&relay)
            .map_err(|err| AuthError::Config(format!("GATEWAY_SMTP_RELAY is invalid: {err}")))?
            .credentials(Credentials::new(username, password))
            .build();

        Ok(Some(Self { mailer, from }))
    }
}

#[async_trait]
impl EmailDelivery for SmtpEmailDelivery {
    async fn send(&self, message: EmailMessage) -> Result<(), AuthError> {
        let mailer = self.mailer.clone();
        let from = self.from.clone();
        tokio::task::spawn_blocking(move || {
            let to = message.to.parse::<Mailbox>().map_err(|err| {
                AuthError::Validation(format!("email recipient is invalid: {err}"))
            })?;
            let email = Message::builder()
                .from(from)
                .to(to)
                .subject(message.subject)
                .singlepart(SinglePart::plain(message.body_text))
                .map_err(|err| AuthError::Internal(format!("build email failed: {err}")))?;
            mailer
                .send(&email)
                .map_err(|err| AuthError::Internal(format!("smtp send failed: {err}")))?;
            Ok(())
        })
        .await
        .map_err(|err| AuthError::Internal(format!("smtp send task failed: {err}")))?
    }
}

#[derive(Debug, Default)]
pub struct RecordingEmailDelivery {
    messages: RwLock<Vec<EmailMessage>>,
}

impl RecordingEmailDelivery {
    pub async fn messages(&self) -> Vec<EmailMessage> {
        self.messages.read().await.clone()
    }
}

#[async_trait]
impl EmailDelivery for RecordingEmailDelivery {
    async fn send(&self, message: EmailMessage) -> Result<(), AuthError> {
        self.messages.write().await.push(message);
        Ok(())
    }
}

fn required_env(name: &str) -> Result<String, AuthError> {
    let value = std::env::var(name)
        .map_err(|_| AuthError::Config(format!("{name} is required when SMTP email is enabled")))?;
    if value.trim().is_empty() {
        return Err(AuthError::Config(format!(
            "{name} must not be empty when SMTP email is enabled"
        )));
    }
    Ok(value)
}
