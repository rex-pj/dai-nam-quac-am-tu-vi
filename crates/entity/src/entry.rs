//! Table `entry`.
//!
//! **The `search_doc` column is deliberately absent.** It is a generated column computed by
//! Postgres; a `Model` carrying it means an `ActiveModel` that can write to it, and Postgres
//! would reject the `INSERT`. Absence at the type level stops that more reliably than any note.

use sea_orm::entity::prelude::*;

use crate::sea_orm_active_enums::EntryPos;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "entry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    /// `NULL` for the 29 entries where the print uses an image for the glyph.
    pub glyph_id: Option<i32>,
    pub page_id: i32,
    /// Position in the book. Gate ⑤ proved this ordering contains no inversion.
    pub seq: i32,
    #[sea_orm(unique)]
    pub slug: String,
    /// Verbatim, tone marks KEPT — Ả, Á and À are three different entries.
    pub reading: String,
    /// Accent-folded and lowercased, for accent-insensitive lookup.
    pub reading_norm: String,
    /// The Sino-Vietnamese reading in parentheses; 43 entries have one.
    pub alternate: Option<String>,
    /// An array, because 548 entries carry two labels.
    pub pos: Vec<EntryPos>,
    pub gloss: String,
    /// All sub-entry text concatenated — **denormalised, for search only, never displayed**.
    ///
    /// A Postgres generated column cannot reference another table, and reverse lookup is
    /// exactly a search through sub-entry text. The import step fills this column.
    pub sub_text: String,
    /// The entry reuses the preceding entry glyph; 55 entries do.
    pub inherits_glyph: bool,
    pub needs_review: bool,
    /// How the shape of an unencodable glyph was described. `NULL` except for the 29 entries
    /// whose glyph is an image, and only once a person has signed the dossier row it came from.
    pub shape_note: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::glyph::Entity",
        from = "Column::GlyphId",
        to = "super::glyph::Column::Id"
    )]
    Glyph,
    #[sea_orm(
        belongs_to = "super::page::Entity",
        from = "Column::PageId",
        to = "super::page::Column::Id"
    )]
    Page,
    #[sea_orm(has_many = "super::sub_entry::Entity")]
    SubEntry,
}

impl Related<super::glyph::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Glyph.def()
    }
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::sub_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SubEntry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
