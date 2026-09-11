//! Storage-layer errors — **with passwords scrubbed**.
//!
//! Why the message is kept as a `String` instead of `#[source] DbErr`: an sqlx connection
//! error can contain the whole connection string, and that string carries a password.
//! Scrubbing it means touching the text, so the scrubbed string is what gets stored. Nothing
//! is truncated — only the password is replaced with `***`.

use dnqatv_config::DatabaseUrl;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("could not open a connection to {url}: {message}")]
    Connect { url: String, message: String },

    #[error("query failed: {0}")]
    Query(String),

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("data in the database violates a domain invariant: {0}")]
    Domain(#[from] dnqatv_core::DomainError),

    #[error("column {column:?} could not be decoded as {expected}: {message}")]
    Decode {
        column: &'static str,
        expected: &'static str,
        message: String,
    },
}

/// Replace every occurrence of the password with `***`.
///
/// This is the second line of defence. The first is that [`DatabaseUrl`] never prints itself
/// verbatim; this one covers strings built by the **underlying library**, where we do not
/// control the formatting.
pub fn scrub(message: impl Into<String>, url: &DatabaseUrl) -> String {
    let message = message.into();
    match url.password() {
        Some(secret) if !secret.is_empty() => message.replace(secret, "***"),
        _ => message,
    }
}
