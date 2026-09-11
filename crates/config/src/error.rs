//! Configuration errors.
//!
//! **Invariant of this module: no variant may carry the raw value of a secret key.**
//! `DATABASE_URL` carries a password, so an error parsing it may mention only the *key name*
//! and the non-sensitive part (e.g. the scheme). A test asserts this.

use crate::env_var::EnvVar;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("required environment variable {} is missing", .0.name())]
    Missing(EnvVar),

    #[error("variable {} is not valid UTF-8", .0.name())]
    NotUtf8(EnvVar),

    #[error("variable {} expects an integer, got {value:?}", .var.name())]
    NotAnInteger { var: EnvVar, value: String },

    #[error("variable {} accepts only one of [{allowed}], got {value:?}", .var.name())]
    NotInSet {
        var: EnvVar,
        value: String,
        allowed: String,
    },

    /// Mentions the scheme only — that part cannot contain a password.
    #[error("connection string must start with postgres:// or postgresql://, got {found:?}")]
    UnsupportedScheme { found: String },

    /// Deliberately does NOT include the connection string.
    #[error("connection string in {} is missing {missing}", .var.name())]
    MalformedUrl { var: EnvVar, missing: &'static str },

    #[error("minimum connection count ({min}) exceeds the maximum ({max})")]
    PoolBoundsInverted { min: u32, max: u32 },

    #[error("maximum connection count must be at least 1")]
    PoolEmpty,
}
