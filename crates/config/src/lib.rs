//! Typed configuration, loaded from the environment **exactly once** at startup.
//!
//! Three principles, all direct consequences of the discipline set out in Part I:
//!
//! 1. **No scattered `std::env` reads.** Every key is declared in [`EnvVar`]; it is read in one
//!    place, after which the program only ever works with a validated struct.
//! 2. **No silent fallbacks.** A missing required key, or a malformed value, is a **startup
//!    error**. Defaults do exist — but a default is a named constant, not an
//!    `unwrap_or_default()` buried in an expression.
//! 3. **Passwords never reach the log.** [`DatabaseUrl`] masks the password in both `Debug` and
//!    `Display`; getting the real string requires [`DatabaseUrl::expose`] — a deliberately
//!    awkward name so every use site can be found with `grep`.
//!
//! This crate **does not depend on sea-orm**: it only produces values; opening a connection is
//! the adapter's job.

pub mod app;
pub mod database;
pub mod dotenv;
pub mod env_var;
pub mod error;
pub mod secret;
pub mod source;

pub use app::{AppConfig, AppEnv};
pub use database::{DatabaseConfig, SqlLogging, defaults};
pub use env_var::{EnvVar, Requirement};
pub use error::ConfigError;
pub use secret::{DatabaseUrl, DbScheme};
pub use source::{EnvSource, Layered, MapEnv, SystemEnv};

/// Load configuration in the standard precedence order: **the real environment beats `.env`**.
///
/// A `.env` file is a convenience on developer machines; in deployment the environment
/// variables are the source of truth, and a stray file inside an image must not override them.
pub fn load_from_dotenv_and_env(dotenv_path: &std::path::Path) -> Result<AppConfig, ConfigError> {
    let layered = Layered(SystemEnv, dotenv::load(dotenv_path));
    AppConfig::from_env(&layered)
}
