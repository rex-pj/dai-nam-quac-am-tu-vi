//! Running migrations.
//!
//! Living here rather than in `server` is a deliberate operational choice: **a migration is a
//! deployment step, not a startup side effect**. A server that migrates itself on boot sounds
//! convenient, but with several replicas they race each other, and a broken deploy rewrites
//! the schema before anyone can look. Here it is a separate command, run by a person, with a
//! record.

use dnqatv_migration::Migrator;
use sea_orm_migration::MigratorTrait;

use crate::connect::Db;
use crate::error::{DbError, scrub};

/// The state of one migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationState {
    pub name: String,
    pub applied: bool,
}

fn err(db: &Db, e: sea_orm::DbErr) -> DbError {
    DbError::Migration(scrub(e.to_string(), db.url()))
}

/// Apply every pending migration.
pub async fn up(db: &Db) -> Result<(), DbError> {
    Migrator::up(db.connection(), None)
        .await
        .map_err(|e| err(db, e))
}

/// Roll back `steps` migrations; `None` rolls back all of them.
pub async fn down(db: &Db, steps: Option<u32>) -> Result<(), DbError> {
    Migrator::down(db.connection(), steps)
        .await
        .map_err(|e| err(db, e))
}

/// Wipe everything and rebuild. Development machines and tests only.
pub async fn fresh(db: &Db) -> Result<(), DbError> {
    Migrator::fresh(db.connection())
        .await
        .map_err(|e| err(db, e))
}

pub async fn status(db: &Db) -> Result<Vec<MigrationState>, DbError> {
    use sea_orm_migration::migrator::MigrationStatus;

    let applied = Migrator::get_applied_migrations(db.connection())
        .await
        .map_err(|e| err(db, e))?;
    let pending = Migrator::get_pending_migrations(db.connection())
        .await
        .map_err(|e| err(db, e))?;

    let mut out: Vec<MigrationState> = applied
        .iter()
        .chain(pending.iter())
        .map(|m| MigrationState {
            name: m.name().to_owned(),
            applied: m.status() == MigrationStatus::Applied,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}
