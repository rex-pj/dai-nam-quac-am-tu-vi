//! Build the schema.
//!
//! This part only *runs* the statements; their shape lives in [`crate::schema`] and
//! [`crate::pg_ddl`] as pure functions, so it is snapshot-tested separately with no DB.

use sea_orm_migration::prelude::extension::postgres::Type;
use sea_orm_migration::prelude::*;

use crate::pg_ddl;
use crate::schema;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Extensions, the unaccent wrapper and the search configuration — all raw SQL is
        // confined to pg_ddl, and the CI grep gate allows `execute_unprepared` only there.
        pg_ddl::apply(manager).await?;

        manager.create_type(schema::create_pos_enum()).await?;
        manager.create_type(schema::create_letter_enum()).await?;
        manager
            .create_type(schema::create_glyph_kind_enum())
            .await?;

        // Table order follows the foreign-key dependency chain.
        manager.create_table(schema::create_page_table()).await?;
        manager.create_table(schema::create_glyph_table()).await?;
        manager.create_table(schema::create_entry_table()).await?;
        manager
            .create_table(schema::create_sub_entry_table())
            .await?;
        manager
            .create_table(schema::create_front_matter_table())
            .await?;

        manager
            .create_index(schema::create_search_doc_index())
            .await?;
        manager
            .create_index(schema::create_reading_norm_trgm_index())
            .await?;
        manager.create_index(schema::create_reading_index()).await?;
        manager
            .create_index(schema::create_sub_entry_index())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in schema::all_tables_in_drop_order() {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }
        for ty in schema::all_types() {
            manager
                .drop_type(Type::drop().name(ty).if_exists().to_owned())
                .await?;
        }
        pg_ddl::revert(manager).await?;
        Ok(())
    }
}
