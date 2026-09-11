//! Application-wide configuration — every piece gathered into **one** value loaded once.

use std::path::PathBuf;

use crate::database::DatabaseConfig;
use crate::env_var::EnvVar;
use crate::error::ConfigError;
use crate::source::EnvSource;

pub mod defaults {
    pub const APP_ENV: &str = "development";
    pub const HTTP_ADDR: &str = "127.0.0.1:8080";
    pub const PAGE_IMAGE_DIR: &str = "frontend/static/trang";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Production,
}

impl AppEnv {
    pub const ALL: [AppEnv; 2] = [Self::Development, Self::Production];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == raw.trim())
    }

    pub const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub env: AppEnv,
    pub database: DatabaseConfig,
    pub http_addr: String,
    pub page_image_dir: PathBuf,
}

impl AppConfig {
    pub fn from_env(env: &dyn EnvSource) -> Result<Self, ConfigError> {
        let app_env_raw = EnvVar::AppEnv.read(env)?;
        let app_env = AppEnv::parse(&app_env_raw).ok_or_else(|| ConfigError::NotInSet {
            var: EnvVar::AppEnv,
            value: app_env_raw.clone(),
            allowed: AppEnv::ALL.map(AppEnv::as_str).join(", "),
        })?;

        Ok(Self {
            env: app_env,
            database: DatabaseConfig::from_env(env)?,
            http_addr: EnvVar::HttpAddr.read(env)?,
            page_image_dir: PathBuf::from(EnvVar::PageImageDir.read(env)?),
        })
    }
}
