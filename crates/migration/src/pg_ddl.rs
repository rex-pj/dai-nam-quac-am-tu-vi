//! The three DDL statements **no Rust builder can express** — and only these three.
//!
//! The plan promised "no magic strings in the query builder". This is where that promise meets
//! reality: SeaQuery (like Diesel, like every other builder) has no API for `CREATE FUNCTION`
//! or `CREATE TEXT SEARCH CONFIGURATION`. Claiming otherwise would be an empty promise.
//!
//! The honest handling: confine all three to a single module, wrap them in clearly named
//! functions, and **snapshot-test the generated strings**. Each string appears exactly once in
//! the whole codebase. CI has a grep gate blocking `execute_unprepared` anywhere else.

use crate::idens::{PgFn, PgType, ts_config_literal};
use sea_orm_migration::prelude::*;

/// The Postgres extensions the schema requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgExtension {
    /// Accent folding — the foundation of accent-insensitive lookup.
    Unaccent,
    /// Trigram matching — used for "did you mean…" suggestions.
    PgTrgm,
}

impl PgExtension {
    pub const ALL: [PgExtension; 2] = [Self::Unaccent, Self::PgTrgm];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Unaccent => "unaccent",
            Self::PgTrgm => "pg_trgm",
        }
    }
}

/// Wrap `unaccent` in an IMMUTABLE function.
///
/// **The classic Postgres trap:** `unaccent(text)` is declared STABLE, not IMMUTABLE, because
/// it depends on the dictionary currently loaded. Consequence: it **cannot be used** in a
/// generated column or an expression index — Postgres rejects it outright.
///
/// The wrapper must use the **two-argument** form `unaccent('unaccent', $1)`: pinning the
/// dictionary is what makes the result genuinely immutable, and the IMMUTABLE declaration honest.
pub fn create_immutable_unaccent() -> String {
    let f = PgFn::ImmutableUnaccent.into_iden().to_string();
    let t = PgType::Text.into_iden().to_string();
    format!(
        "CREATE OR REPLACE FUNCTION {f}({t}) RETURNS {t} \
         LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE \
         AS $$ SELECT unaccent('unaccent', $1) $$"
    )
}

/// Create the full-text search configuration for Vietnamese.
///
/// Postgres has no `vietnamese` configuration. Copy from `simple` — deliberately **not** a
/// stemmed configuration: English or French style stemming is meaningless for Vietnamese and
/// would corrupt meaning.
pub fn create_text_search_config() -> String {
    let c = ts_config_literal();
    format!("CREATE TEXT SEARCH CONFIGURATION {c} (COPY = simple)")
}

/// Chain the `unaccent` dictionary ahead of `simple` so search ignores diacritics.
///
/// The order `unaccent, simple` is deliberate: fold the accents first, then take the raw form.
pub fn alter_text_search_config_mapping() -> String {
    let c = ts_config_literal();
    format!(
        "ALTER TEXT SEARCH CONFIGURATION {c} \
         ALTER MAPPING FOR asciiword, asciihword, hword_asciipart, \
         word, hword, hword_part \
         WITH unaccent, simple"
    )
}

/// The three DDL statements, in the order they must run.
pub fn all_statements() -> Vec<String> {
    vec![
        create_immutable_unaccent(),
        create_text_search_config(),
        alter_text_search_config_mapping(),
    ]
}

/// The matching drop statements, in reverse order.
pub fn all_drop_statements() -> Vec<String> {
    let c = ts_config_literal();
    let f = PgFn::ImmutableUnaccent.into_iden().to_string();
    let t = PgType::Text.into_iden().to_string();
    vec![
        format!("DROP TEXT SEARCH CONFIGURATION IF EXISTS {c}"),
        format!("DROP FUNCTION IF EXISTS {f}({t})"),
    ]
}

// ── The ONLY place raw SQL is executed ───────────────────────────────────────

/// Run the raw DDL: extensions, the wrapper function, the search configuration.
///
/// Executing here rather than in the migration file is deliberate: the CI grep gate allows
/// `execute_unprepared` only inside this module. That makes "no raw SQL outside pg_ddl" a
/// machine-checkable fact rather than a convention people must remember.
pub async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    for ext in PgExtension::ALL {
        conn.execute_unprepared(&format!("CREATE EXTENSION IF NOT EXISTS {}", ext.name()))
            .await?;
    }
    for stmt in all_statements() {
        conn.execute_unprepared(&stmt).await?;
    }
    Ok(())
}

/// Revert the raw DDL. Extensions are not dropped: another schema may be using them.
pub async fn revert(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    for stmt in all_drop_statements() {
        conn.execute_unprepared(&stmt).await?;
    }
    Ok(())
}
