//! Add `entry.shape_note`.
//!
//! For the 29 entries whose glyph has no Unicode code point, the 2026 edition sets a picture
//! cropped from the print. A picture cannot be searched, copied or read aloud, and it tells a
//! reader nothing about what the character is made of.
//!
//! An independent transcription of the 1895 original hit the same wall and answered it better:
//! where it could not encode a shape it **described** it — `⿰口胖`, `⿱壯卵` — in Ideographic
//! Description Sequences, a standard notation. That is not a guess at a code point; it is a
//! record of what the page shows. This column is where such a description lands once a person
//! has checked it against the page image and signed for it.

use sea_orm_migration::prelude::*;

use crate::schema;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(schema::add_entry_shape_note()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(schema::drop_entry_shape_note()).await
    }
}
