//! Table `front_matter` — TIỂU TỰ, DẤU RIÊNG, PRÉFACE, LỜI DẶN, LƯU Ý.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "front_matter")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    #[sea_orm(unique)]
    pub slug: String,
    pub title: String,
    pub body: String,
    pub pdf_page: i16,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
