//! Opening the connection. **This is the only place in the project that builds a pool.**

use std::fmt;

use dnqatv_config::{DatabaseConfig, DatabaseUrl};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::error::{DbError, scrub};

/// A handle to the database.
///
/// It wraps `DatabaseConnection` rather than using it directly, for two practical reasons:
/// it carries the redacted connection string so passwords can be scrubbed out of lower-layer
/// errors, and its `Debug` is safe — an `AppState` holding a `Db` will be
/// `#[derive(Debug)]`-ed somewhere soon enough.
#[derive(Clone)]
pub struct Db {
    conn: DatabaseConnection,
    url: DatabaseUrl,
}

impl fmt::Debug for Db {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Db").field("url", &self.url).finish()
    }
}

impl Db {
    /// The raw connection, for sibling adapter-layer crates (migrations).
    ///
    /// Not an architectural hole: a crate that does not declare `sea-orm` in its
    /// `Cargo.toml` **cannot even name the return type**, so `core`, `app` and
    /// `adapter-web` cannot call this. Cargo holds the boundary, not convention.
    pub fn connection(&self) -> &DatabaseConnection {
        &self.conn
    }

    pub fn url(&self) -> &DatabaseUrl {
        &self.url
    }

    /// Turn a `DbErr` into an error with the password scrubbed out.
    pub(crate) fn query_err(&self, err: sea_orm::DbErr) -> DbError {
        DbError::Query(scrub(err.to_string(), &self.url))
    }

    /// A real round trip to the server.
    ///
    /// The statement is built with the builder (`SELECT 1`), not a raw string — the same
    /// discipline as the migrations, so the health check travels the path real queries take.
    pub async fn ping(&self) -> Result<(), DbError> {
        use sea_orm::ConnectionTrait;
        use sea_orm::sea_query::{Expr, Query};

        let select = Query::select().expr(Expr::val(1)).to_owned();
        self.conn
            .query_one(&select)
            .await
            .map_err(|e| self.query_err(e))?;
        Ok(())
    }

    pub async fn close(self) -> Result<(), DbError> {
        let url = self.url.clone();
        self.conn
            .close()
            .await
            .map_err(|e| DbError::Query(scrub(e.to_string(), &url)))
    }
}

/// Build the pool from validated configuration.
pub async fn connect(cfg: &DatabaseConfig) -> Result<Db, DbError> {
    let mut opt = ConnectOptions::new(cfg.url.expose().to_owned());
    opt.max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .connect_timeout(cfg.connect_timeout)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        .sqlx_logging(cfg.sql_logging.enabled());

    let conn = Database::connect(opt).await.map_err(|e| DbError::Connect {
        url: cfg.url.redacted().to_owned(),
        message: scrub(e.to_string(), &cfg.url),
    })?;

    Ok(Db {
        conn,
        url: cfg.url.clone(),
    })
}
