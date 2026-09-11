//! PostgreSQL-specific SQL identifiers, declared as `Iden`.
//!
//! The same discipline as `migration::idens`: Postgres functions and configurations are
//! *names*, and a name is declared once. The string `"vi_simple"` appears exactly once here.

use sea_orm::DeriveIden;
use sea_orm::sea_query::{Expr, Func, FunctionCall, IntoIden, SimpleExpr};

#[derive(DeriveIden)]
pub enum PgFn {
    #[sea_orm(iden = "websearch_to_tsquery")]
    WebsearchToTsquery,
    #[sea_orm(iden = "ts_rank")]
    TsRank,
    #[sea_orm(iden = "similarity")]
    Similarity,
    #[sea_orm(iden = "unaccent")]
    Unaccent,
}

#[derive(DeriveIden)]
pub enum PgType {
    #[sea_orm(iden = "regconfig")]
    RegConfig,
}

/// The Vietnamese full-text search configuration — created by the migration.
#[derive(DeriveIden)]
pub enum TsConfig {
    #[sea_orm(iden = "vi_simple")]
    ViSimple,
}

/// `'vi_simple'::regconfig`
///
/// `PgFunc::to_tsquery` is avoided because it takes `regconfig` as a `u32` OID, and that OID
/// differs per database. Casting from the name lets Postgres resolve it at parse time.
pub fn ts_config() -> FunctionCall {
    Func::cast_as(
        TsConfig::ViSimple.into_iden().to_string(),
        PgType::RegConfig,
    )
}

/// `websearch_to_tsquery('vi_simple', $q)`
///
/// `websearch_` rather than `plainto_`: it accepts the syntax users already know from search
/// engines (`"exact phrase"`, `-exclude`, `or`) and **never raises a syntax error** — a hard
/// requirement when the query comes straight from an input box.
pub fn websearch(query: &str) -> SimpleExpr {
    Func::cust(PgFn::WebsearchToTsquery)
        .arg(ts_config())
        .arg(query)
        .into()
}

/// `ts_rank(<vector>, websearch_to_tsquery('vi_simple', $q))`
pub fn ts_rank(vector: impl Into<SimpleExpr>, query: &str) -> SimpleExpr {
    Func::cust(PgFn::TsRank)
        .arg(vector.into())
        .arg(websearch(query))
        .into()
}

/// `similarity(<column>, $q)` — trigram similarity, for "did you mean…" suggestions.
pub fn similarity(column: impl Into<SimpleExpr>, query: &str) -> SimpleExpr {
    Func::cust(PgFn::Similarity)
        .arg(column.into())
        .arg(query)
        .into()
}

/// Characters that must be escaped inside a `LIKE` pattern.
///
/// A reader can type `%` and `_`; left alone, `%` turns the query into "match everything".
/// This is not an SQL injection hole (parameters are still bound) — it is a wrong-results bug.
pub fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// An escaped prefix pattern for `LIKE`.
pub fn prefix_pattern(query: &str) -> String {
    format!("{}%", escape_like(query))
}

/// The `entry.search_doc` column.
///
/// It exists in the database but is DELIBERATELY absent from `entity::entry::Model`: it is a
/// generated column, a `Model` holding it means an `ActiveModel` that can write to it, and
/// Postgres would reject the `INSERT`. Queries still need to name it, so the name is declared
/// here — still an Iden, still a single place.
#[derive(DeriveIden)]
pub enum EntryCol {
    #[sea_orm(iden = "entry")]
    Table,
    #[sea_orm(iden = "search_doc")]
    SearchDoc,
}

/// Aliases for the extra columns in the search query.
///
/// Declared via `as_str` instead of `DeriveIden` because SeaORM wants `IntoIdentity` in
/// `column_as`, which `DeriveIden` does not provide. Still one declaration site per name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    GlyphChar,
    Tier,
    Score,
}

impl Extra {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GlyphChar => "glyph_char",
            Self::Tier => "tier",
            Self::Score => "score",
        }
    }
}

/// An integer literal expression, convenient inside a `CASE`.
pub fn int(v: i32) -> SimpleExpr {
    Expr::val(v)
}
