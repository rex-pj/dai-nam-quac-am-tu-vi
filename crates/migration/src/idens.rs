//! Every schema identifier, declared as an `Iden` rather than a string.
//!
//! Why: the real problem is not "ugly strings" but **drift** — the same name written in
//! several places, then diverging silently. With enums, a rename makes the compiler point at
//! every use site.
//!
//! The Postgres enum *values* are NOT declared here: they are generated from the domain layer
//! `::ALL` arrays (`Pos`, `Letter`, `GlyphKind`, `TextStyle`), so adding a variant carries it
//! into the migration automatically.

use sea_orm_migration::prelude::*;

// ── Tables and columns ───────────────────────────────────────────────────────

#[derive(DeriveIden)]
pub enum Page {
    Table,
    Id,
    PdfPage,
    /// The original printed page number. `NULL` on the three pages without one (the 2026
    /// cover, the original cover, the LƯU Ý page) — the two numbering systems do not line up
    /// uniformly; see `core::model::page`.
    PrintedPage,
    Letter,
    ImagePath,
    HeadFirst,
    HeadLast,
}

#[derive(DeriveIden)]
pub enum Glyph {
    Table,
    Id,
    Char,
    Codepoint,
    Kind,
    ImagePath,
    /// Whether this glyph appears in the entry index.
    InIndex,
}

#[derive(DeriveIden)]
pub enum Entry {
    Table,
    Id,
    GlyphId,
    PageId,
    Seq,
    Slug,
    /// The reading verbatim, tone marks kept — Ả, Á and À are three different entries.
    Reading,
    /// The reading accent-folded and lowercased, for accent-insensitive lookup.
    ReadingNorm,
    /// The Sino-Vietnamese reading in parentheses; 39 entries have one.
    Alternate,
    /// The label list — 548 entries carry two labels, so this is an array, not a scalar.
    Pos,
    Gloss,
    /// All sub-entry text of this entry, concatenated — **a denormalised, derived column**.
    ///
    /// Why it must exist: a Postgres generated column **cannot reference another table**, so
    /// `sub_entry` cannot feed `search_doc` directly. Yet reverse lookup ("nạm gươm" → the
    /// entry Lõm) is exactly a search through sub-entry text. The import step fills this
    /// column, and it is used for search only — never displayed as the words of the book.
    SubText,
    /// The entry reuses the preceding entry glyph; 55 entries do.
    InheritsGlyph,
    NeedsReview,
    /// How the shape of an unencodable glyph was described, for the 29 entries where the
    /// print sets an image. `NULL` until a person has signed the dossier row it comes from.
    ShapeNote,
    SearchDoc,
}

#[derive(DeriveIden)]
pub enum SubEntry {
    Table,
    Id,
    EntryId,
    Seq,
    /// The Han part verbatim.
    HanForm,
    /// Derived: `|` replaced by the entry glyph.
    HanExpanded,
    /// The form verbatim — what the UI shows by default.
    Form,
    /// Derived: the dash replaced by the reading — used for search.
    FormExpanded,
    Definition,
    NeedsReview,
}

#[derive(DeriveIden)]
pub enum FrontMatter {
    Table,
    Id,
    Slug,
    Title,
    Body,
    PdfPage,
}

// ── Postgres enum types ──────────────────────────────────────────────────────

#[derive(DeriveIden)]
pub enum PgEnum {
    #[sea_orm(iden = "entry_pos")]
    EntryPos,
    #[sea_orm(iden = "book_letter")]
    BookLetter,
    #[sea_orm(iden = "glyph_kind")]
    GlyphKind,
}

// ── Index names ──────────────────────────────────────────────────────────────

#[derive(DeriveIden)]
pub enum IdxName {
    #[sea_orm(iden = "idx_entry_search_doc")]
    EntrySearchDoc,
    #[sea_orm(iden = "idx_entry_reading_norm_trgm")]
    EntryReadingNormTrgm,
    #[sea_orm(iden = "idx_entry_reading")]
    EntryReading,
    #[sea_orm(iden = "idx_entry_letter")]
    EntryLetter,
    #[sea_orm(iden = "idx_sub_entry_entry_id")]
    SubEntryEntryId,
    #[sea_orm(iden = "idx_glyph_char")]
    GlyphChar,
}

// ── Index methods and operator classes ───────────────────────────────────────

#[derive(DeriveIden)]
pub enum IdxMethod {
    #[sea_orm(iden = "gin")]
    Gin,
}

#[derive(DeriveIden)]
pub enum OpClass {
    #[sea_orm(iden = "gin_trgm_ops")]
    GinTrgm,
}

// ── Postgres functions and types ─────────────────────────────────────────────

#[derive(DeriveIden)]
pub enum PgFn {
    #[sea_orm(iden = "to_tsvector")]
    ToTsvector,
    #[sea_orm(iden = "setweight")]
    Setweight,
    #[sea_orm(iden = "coalesce")]
    Coalesce,
    /// The IMMUTABLE `unaccent` wrapper — see `pg_ddl`.
    #[sea_orm(iden = "immutable_unaccent")]
    ImmutableUnaccent,
}

#[derive(DeriveIden)]
pub enum PgType {
    #[sea_orm(iden = "regconfig")]
    RegConfig,
    #[sea_orm(iden = "tsvector")]
    TsVector,
    #[sea_orm(iden = "text")]
    Text,
}

/// The full-text search configuration for Vietnamese.
///
/// Postgres ships no `vietnamese` configuration; this one is created in `pg_ddl` by copying
/// `simple` and chaining the `unaccent` dictionary onto it.
#[derive(DeriveIden)]
pub enum TsConfig {
    #[sea_orm(iden = "vi_simple")]
    ViSimple,
}

/// The configuration name as a literal, for the `'vi_simple'::regconfig` cast.
///
/// This is the only place that string appears as a value; everywhere else uses [`TsConfig`].
pub fn ts_config_literal() -> String {
    TsConfig::ViSimple.into_iden().to_string()
}
