//! The PostgreSQL adapter.
//!
//! **This is the only crate allowed to know PostgreSQL exists.** `core` declares its ports in
//! the language of the domain; this crate implements them with SeaORM. That boundary is held
//! by the Cargo dependency graph: `core/Cargo.toml` does not declare `sea-orm`, so leaking
//! persistence into the domain is a compile error, not a review finding.

pub mod connect;
pub mod error;
pub mod health;
pub mod mapping;
pub mod migrate;
pub mod repository;
pub mod sql;

pub use connect::{Db, connect};
pub use error::DbError;
pub use repository::PgRepository;
