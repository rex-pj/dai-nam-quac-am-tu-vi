//! Migrations for the dictionary schema.
//!
//! All DDL is written with the typed builder, except the three statements in [`pg_ddl`] that
//! no Rust builder can express. The schema itself lives in [`schema`] as pure functions, so
//! it can be snapshot-tested without a running PostgreSQL.

mod m20260101_000001_create_schema;
mod m20260101_000002_add_entry_shape_note;

pub mod idens;
pub mod pg_ddl;
pub mod schema;

use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260101_000001_create_schema::Migration),
            Box::new(m20260101_000002_add_entry_shape_note::Migration),
        ]
    }
}
