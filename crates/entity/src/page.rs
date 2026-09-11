//! Table `page` — one row per PDF page.

use sea_orm::entity::prelude::*;

use crate::sea_orm_active_enums::BookLetter;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "page")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub pdf_page: i16,
    /// `NULL` on the three pages with no printed number: the 2026 cover, the original cover, and the LƯU Ý page.
    pub printed_page: Option<i16>,
    pub letter: Option<BookLetter>,
    pub image_path: Option<String>,
    pub head_first: Option<String>,
    pub head_last: Option<String>,
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
