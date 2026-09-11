//! A 1-to-1 mapping onto the tables — **and nothing more**.
//!
//! This is deliberately NOT the domain model. A `Model` has the shape of the database (`i32`
//! keys, bare strings, `Option` exactly where a column is nullable); the domain model carries
//! invariants (NFC readings, a non-empty part-of-speech list, typed page numbers). The seam
//! between them is `TryFrom` in `adapter-db`, and it **is allowed to fail** — a row violating
//! an invariant must blow up loudly rather than quietly produce a broken entry.
//!
//! The `search_doc` column appears in no `Model`: it is a generated column, computed by
//! Postgres, and nobody may write to it. Absence at the type level is the surest guarantee.

pub mod entry;
pub mod front_matter;
pub mod glyph;
pub mod page;
pub mod sea_orm_active_enums;
pub mod sub_entry;

pub use sea_orm_active_enums::{BookLetter, EntryPos, GlyphKind};
