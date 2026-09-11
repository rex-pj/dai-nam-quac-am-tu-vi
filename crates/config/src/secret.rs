//! The connection string — its own type, because it carries a password.
//!
//! `DATABASE_URL` is the single most leak-prone value in the program: it ends up in connection
//! error messages, in `dbg!`, in startup logs, in panic reports. The defence is not "remember
//! not to print it" but **making a verbatim print impossible unless it is deliberate**: both
//! `Debug` and `Display` return the masked form, and the real string requires
//! [`DatabaseUrl::expose`].

use crate::env_var::EnvVar;
use crate::error::ConfigError;

/// Accepted schemes. Both are standard PostgreSQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbScheme {
    Postgres,
    Postgresql,
}

impl DbScheme {
    pub const ALL: [DbScheme; 2] = [Self::Postgres, Self::Postgresql];

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Postgres => "postgres://",
            Self::Postgresql => "postgresql://",
        }
    }
}

/// A validated connection string that masks its password when printed.
#[derive(Clone, PartialEq, Eq)]
pub struct DatabaseUrl {
    raw: String,
    redacted: String,
    scheme: DbScheme,
    password: Option<String>,
    database: String,
}

impl DatabaseUrl {
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        const VAR: EnvVar = EnvVar::DatabaseUrl;
        let raw = raw.trim();

        let Some(scheme) = DbScheme::ALL
            .into_iter()
            .find(|s| raw.starts_with(s.prefix()))
        else {
            // Report only the part before "://" — it cannot contain a password.
            let found = raw.split_once("://").map_or("", |(s, _)| s).to_owned();
            return Err(ConfigError::UnsupportedScheme { found });
        };

        let rest = &raw[scheme.prefix().len()..];
        // The authority ends at the first character of the path, query or fragment.
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at(authority_end);

        // A password may contain '@', so the LAST '@' is the userinfo/host boundary.
        let (userinfo, host) = match authority.rsplit_once('@') {
            Some((u, h)) => (Some(u), h),
            None => (None, authority),
        };
        if host.is_empty() {
            return Err(ConfigError::MalformedUrl {
                var: VAR,
                missing: "a host name",
            });
        }

        let password = userinfo
            .and_then(|u| u.split_once(':'))
            .map(|(_, p)| p.to_owned())
            .filter(|p| !p.is_empty());

        let database = tail
            .trim_start_matches('/')
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .to_owned();
        if database.is_empty() {
            // Omitting the database name is a deadly trap: sea-orm would connect to the
            // default database named after the user, and the migration would silently run
            // in the wrong place.
            return Err(ConfigError::MalformedUrl {
                var: VAR,
                missing: "a database name",
            });
        }

        let redacted = match (userinfo, &password) {
            (Some(u), Some(p)) => {
                let user = u.split_once(':').map_or(u, |(user, _)| user);
                let _ = p;
                format!("{}{user}:***@{host}{tail}", scheme.prefix())
            }
            _ => raw.to_owned(),
        };

        Ok(Self {
            raw: raw.to_owned(),
            redacted,
            scheme,
            password,
            database,
        })
    }

    /// The real string. The name is deliberately jarring so `grep expose` finds every use.
    pub fn expose(&self) -> &str {
        &self.raw
    }

    pub fn redacted(&self) -> &str {
        &self.redacted
    }

    pub const fn scheme(&self) -> DbScheme {
        self.scheme
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    /// The password, if any — so the adapter can scrub it from lower-layer error messages.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl std::fmt::Debug for DatabaseUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DatabaseUrl({})", self.redacted)
    }
}

impl std::fmt::Display for DatabaseUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.redacted)
    }
}
