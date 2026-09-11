//! The environment variable catalogue — **the one and only list in the project**.
//!
//! Same concern as `Pos::ALL` in the migration: the problem is not "ugly strings" but
//! **drift** — a new key added in code but forgotten in `.env.example`, and the next person
//! loses half a day wondering why the program will not start.
//!
//! How that is prevented: `.env.example` is **generated** from [`EnvVar::ALL`] by
//! [`render_example`], and a test compares it byte for byte with the file on disk. Adding a
//! key without regenerating the file turns the test red.

use crate::error::ConfigError;
use crate::source::EnvSource;

/// A key is either required, or optional with a displayable default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    Required,
    Optional { default: &'static str },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnvVar {
    DatabaseUrl,
    DatabaseMaxConnections,
    DatabaseMinConnections,
    DatabaseConnectTimeoutSecs,
    DatabaseAcquireTimeoutSecs,
    DatabaseIdleTimeoutSecs,
    DatabaseSqlLogging,
    AppEnv,
    HttpAddr,
    PageImageDir,
}

impl EnvVar {
    pub const ALL: [EnvVar; 10] = [
        Self::DatabaseUrl,
        Self::DatabaseMaxConnections,
        Self::DatabaseMinConnections,
        Self::DatabaseConnectTimeoutSecs,
        Self::DatabaseAcquireTimeoutSecs,
        Self::DatabaseIdleTimeoutSecs,
        Self::DatabaseSqlLogging,
        Self::AppEnv,
        Self::HttpAddr,
        Self::PageImageDir,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::DatabaseUrl => "DATABASE_URL",
            Self::DatabaseMaxConnections => "DATABASE_MAX_CONNECTIONS",
            Self::DatabaseMinConnections => "DATABASE_MIN_CONNECTIONS",
            Self::DatabaseConnectTimeoutSecs => "DATABASE_CONNECT_TIMEOUT_SECS",
            Self::DatabaseAcquireTimeoutSecs => "DATABASE_ACQUIRE_TIMEOUT_SECS",
            Self::DatabaseIdleTimeoutSecs => "DATABASE_IDLE_TIMEOUT_SECS",
            Self::DatabaseSqlLogging => "DATABASE_SQL_LOGGING",
            Self::AppEnv => "APP_ENV",
            Self::HttpAddr => "HTTP_ADDR",
            Self::PageImageDir => "PAGE_IMAGE_DIR",
        }
    }

    pub const fn requirement(self) -> Requirement {
        use crate::database::defaults;
        match self {
            Self::DatabaseUrl => Requirement::Required,
            Self::DatabaseMaxConnections => Requirement::Optional {
                default: defaults::MAX_CONNECTIONS,
            },
            Self::DatabaseMinConnections => Requirement::Optional {
                default: defaults::MIN_CONNECTIONS,
            },
            Self::DatabaseConnectTimeoutSecs => Requirement::Optional {
                default: defaults::CONNECT_TIMEOUT_SECS,
            },
            Self::DatabaseAcquireTimeoutSecs => Requirement::Optional {
                default: defaults::ACQUIRE_TIMEOUT_SECS,
            },
            Self::DatabaseIdleTimeoutSecs => Requirement::Optional {
                default: defaults::IDLE_TIMEOUT_SECS,
            },
            Self::DatabaseSqlLogging => Requirement::Optional {
                default: defaults::SQL_LOGGING,
            },
            Self::AppEnv => Requirement::Optional {
                default: crate::app::defaults::APP_ENV,
            },
            Self::HttpAddr => Requirement::Optional {
                default: crate::app::defaults::HTTP_ADDR,
            },
            Self::PageImageDir => Requirement::Optional {
                default: crate::app::defaults::PAGE_IMAGE_DIR,
            },
        }
    }

    /// A one-line explanation, copied straight into `.env.example`.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::DatabaseUrl => "PostgreSQL connection string. Must end with a database name.",
            Self::DatabaseMaxConnections => "Maximum number of connections in the pool.",
            Self::DatabaseMinConnections => "Number of connections kept warm.",
            Self::DatabaseConnectTimeoutSecs => "How long to wait for a new connection to open.",
            Self::DatabaseAcquireTimeoutSecs => {
                "How long to wait to borrow a connection from the pool."
            }
            Self::DatabaseIdleTimeoutSecs => "Close a connection after it has been idle this long.",
            Self::DatabaseSqlLogging => "Log every SQL statement: true or false.",
            Self::AppEnv => "Runtime environment: development or production.",
            Self::HttpAddr => "Address the web server listens on.",
            Self::PageImageDir => "Directory holding the rendered page images.",
        }
    }

    /// Read the raw value, or the default when one exists.
    pub fn read(self, env: &dyn EnvSource) -> Result<String, ConfigError> {
        // An empty string counts as unset: `KEY=` in .env is a typo, not an intention.
        match env.get(self.name()).filter(|v| !v.trim().is_empty()) {
            Some(v) => Ok(v),
            None => match self.requirement() {
                Requirement::Required => Err(ConfigError::Missing(self)),
                Requirement::Optional { default } => Ok(default.to_owned()),
            },
        }
    }

    pub fn read_u32(self, env: &dyn EnvSource) -> Result<u32, ConfigError> {
        let raw = self.read(env)?;
        raw.trim().parse().map_err(|_| ConfigError::NotAnInteger {
            var: self,
            value: raw,
        })
    }

    pub fn read_u64(self, env: &dyn EnvSource) -> Result<u64, ConfigError> {
        let raw = self.read(env)?;
        raw.trim().parse().map_err(|_| ConfigError::NotAnInteger {
            var: self,
            value: raw,
        })
    }
}

/// Generate the contents of `.env.example` from [`EnvVar::ALL`] itself.
///
/// Required keys are left blank (the user must fill them in); optional keys carry their
/// default but stay **commented out**, so the example file cannot accidentally override the
/// program default behaviour.
pub fn render_example() -> String {
    let mut out = String::new();
    out.push_str("# Example file — GENERATED from EnvVar::ALL, do not edit by hand.\n");
    out.push_str("# Regenerate with: cargo run -p dnqatv-cli --bin dnqatv -- config example\n");
    out.push_str("# Copy to .env and fill in the required keys.\n");
    for var in EnvVar::ALL {
        out.push('\n');
        out.push_str("# ");
        out.push_str(var.describe());
        out.push('\n');
        match var.requirement() {
            Requirement::Required => {
                out.push_str("# REQUIRED\n");
                out.push_str(var.name());
                out.push_str("=\n");
            }
            Requirement::Optional { default } => {
                out.push_str("# ");
                out.push_str(var.name());
                out.push('=');
                out.push_str(default);
                out.push('\n');
            }
        }
    }
    out
}
