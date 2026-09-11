//! Table `glyph` — one row per Han-Nom glyph.

use sea_orm::entity::prelude::*;

use crate::sea_orm_active_enums::GlyphKind;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "glyph")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    #[sea_orm(column_name = "char", unique)]
    pub char: String,
    pub codepoint: i32,
    pub kind: GlyphKind,
    /// An image cropped from the print, for glyphs with no Unicode code point.
    pub image_path: Option<String>,
    /// Whether this glyph appears in the entry index — the trace left by gate ④.
    pub in_index: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::entry::Entity")]
    Entry,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
