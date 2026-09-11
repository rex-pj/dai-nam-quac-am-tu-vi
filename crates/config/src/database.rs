//! Database connection configuration.

use std::time::Duration;

use crate::env_var::EnvVar;
use crate::error::ConfigError;
use crate::secret::DatabaseUrl;
use crate::source::EnvSource;

/// Defaults, declared **as strings** because that is exactly what goes into `.env.example`.
///
/// Keeping a single form is deliberate: adding a numeric copy would create two sources of
/// truth, and they would diverge on the very day one side is edited. The numeric values are
/// produced by parsing these strings, down the same path a user-supplied value travels.
pub mod defaults {
    pub const MAX_CONNECTIONS: &str = "16";
    pub const MIN_CONNECTIONS: &str = "1";
    pub const CONNECT_TIMEOUT_SECS: &str = "8";
    pub const ACQUIRE_TIMEOUT_SECS: &str = "8";
    pub const IDLE_TIMEOUT_SECS: &str = "300";
    pub const SQL_LOGGING: &str = "false";
}

/// Whether to log SQL statements. An enum rather than a `bool` so call sites read clearly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlLogging {
    On,
    Off,
}

impl SqlLogging {
    pub const ALL: [SqlLogging; 2] = [Self::On, Self::Off];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::On => "true",
            Self::Off => "false",
        }
    }

    pub const fn enabled(self) -> bool {
        matches!(self, Self::On)
    }

    /// Strict: an unrecognised value is an **error**, not "treat it as off".
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == raw.trim())
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: DatabaseUrl,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub sql_logging: SqlLogging,
}

impl DatabaseConfig {
    pub fn from_env(env: &dyn EnvSource) -> Result<Self, ConfigError> {
        let url = DatabaseUrl::parse(&EnvVar::DatabaseUrl.read(env)?)?;
        let max_connections = EnvVar::DatabaseMaxConnections.read_u32(env)?;
        let min_connections = EnvVar::DatabaseMinConnections.read_u32(env)?;

        if max_connections == 0 {
            return Err(ConfigError::PoolEmpty);
        }
        if min_connections > max_connections {
            return Err(ConfigError::PoolBoundsInverted {
                min: min_connections,
                max: max_connections,
            });
        }

        let sql_logging_raw = EnvVar::DatabaseSqlLogging.read(env)?;
        let sql_logging =
            SqlLogging::parse(&sql_logging_raw).ok_or_else(|| ConfigError::NotInSet {
                var: EnvVar::DatabaseSqlLogging,
                value: sql_logging_raw.clone(),
                allowed: SqlLogging::ALL.map(SqlLogging::as_str).join(", "),
            })?;

        Ok(Self {
            url,
            max_connections,
            min_connections,
            connect_timeout: Duration::from_secs(EnvVar::DatabaseConnectTimeoutSecs.read_u64(env)?),
            acquire_timeout: Duration::from_secs(EnvVar::DatabaseAcquireTimeoutSecs.read_u64(env)?),
            idle_timeout: Duration::from_secs(EnvVar::DatabaseIdleTimeoutSecs.read_u64(env)?),
            sql_logging,
        })
    }
}
