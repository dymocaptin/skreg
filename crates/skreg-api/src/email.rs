//! Thin async SMTP send helper used by auth and rotate handlers.

use lettre::message::{header::ContentType, Mailbox, Message};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use log::error;

/// Send a plain-text email via an unauthenticated SMTP relay.
///
/// # Errors
///
/// Returns a human-readable error string if the message cannot be built or sent.
pub async fn send_email(
    smtp_host: &str,
    smtp_port: u16,
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
    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
        .port(smtp_port)
        .build()
        .send(email)
        .await
        .map(|_| ())
        .map_err(|e| {
            error!("smtp send error: {e}");
            format!("smtp: {e}")
        })
}
