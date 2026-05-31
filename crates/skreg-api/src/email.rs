//! Thin async SMTP send helper used by auth and rotate handlers.

use lettre::message::{header::ContentType, Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use log::error;

/// SMTP relay configuration.
#[derive(Clone, Debug)]
pub struct SmtpConfig {
    /// Hostname of the SMTP relay.
    pub host: String,
    /// TCP port of the SMTP relay.
    pub port: u16,
    /// When `Some`, uses STARTTLS + PLAIN auth (e.g. Amazon SES SMTP on port 587).
    /// When `None`, connects anonymously (e.g. in-cluster Postfix on port 25).
    pub username: Option<String>,
    /// SMTP password paired with `username`.
    pub password: Option<String>,
}

/// Send a plain-text email.
///
/// # Errors
///
/// Returns a human-readable error string if the message cannot be built or sent.
pub async fn send_email(
    cfg: &SmtpConfig,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    let from_mb: Mailbox = from
        .parse()
        .map_err(|e| format!("invalid from address: {e}"))?;
    let to_mb: Mailbox = to.parse().map_err(|e| format!("invalid to address: {e}"))?;
    let email = Message::builder()
        .from(from_mb)
        .to(to_mb)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_owned())
        .map_err(|e| format!("build email: {e}"))?;

    match (&cfg.username, &cfg.password) {
        (Some(user), Some(pass)) => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
                .map_err(|e| format!("smtp relay: {e}"))?
                .port(cfg.port)
                .credentials(Credentials::new(user.clone(), pass.clone()))
                .build()
                .send(email)
                .await
        }
        _ => {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
                .port(cfg.port)
                .build()
                .send(email)
                .await
        }
    }
    .map(|_| ())
    .map_err(|e| {
        error!("smtp send error: {e}");
        format!("smtp: {e}")
    })
}
