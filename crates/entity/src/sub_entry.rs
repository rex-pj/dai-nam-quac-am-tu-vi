//! Table `sub_entry`.
//!
//! Two pairs of fields, not two fields plus a flag: `han_form`/`han_expanded` and
//! `form`/`form_expanded`. The left side is the print verbatim, the right side is derived by
//! the two rules from the DẤU RIÊNG page. The UI picks which side to show, but the two can
//! never be conflated.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "sub_entry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub entry_id: i32,
    pub seq: i32,
    /// The Han part verbatim; `|` not yet substituted.
    pub han_form: Option<String>,
    /// Derived: `|` replaced by the entry glyph.
    pub han_expanded: Option<String>,
    /// The form verbatim, e.g. `― gươm`.
    pub form: String,
    /// Derived: the placeholder replaced by the reading, e.g. `Lõm gươm`.
    pub form_expanded: String,
    pub definition: String,
    pub needs_review: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::EntryId",
        to = "super::entry::Column::Id",
        on_delete = "Cascade"
    )]
    Entry,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
