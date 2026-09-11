// clippy only relaxes panic/expect inside #[test] functions.
#![allow(clippy::expect_used, clippy::panic)]

//! Snapshots of the SQL strings the migration generates.
//!
//! This is how the "no magic strings in the query builder" promise is verified **without a
//! running PostgreSQL**: seeing the final SQL string matters more than executing it.
//!
//! This test is also where the real boundary of that promise is recorded — exactly three DDL
//! statements no Rust builder can express, all confined to `pg_ddl`.

use dnqatv_migration::{pg_ddl, schema};
use sea_orm_migration::prelude::*;

// ── Enums generated from the domain layer ────────────────────────────────────

#[test]
fn the_pos_enum_is_generated_from_pos_all() {
    // Adding a variant to `Pos` makes it appear here automatically — no second list to drift.
    assert_eq!(
        schema::create_pos_enum().to_string(PostgresQueryBuilder),
        r#"CREATE TYPE "entry_pos" AS ENUM ('chu_nho', 'chu_nom', 'chu_nho_dung_nom')"#
    );
}

#[test]
fn the_letter_enum_keeps_the_printed_order() {
    // The crucial point: `'h', 'y', 'k'` — Y sits where I would be, exactly as printed.
    // Postgres compares enums by declaration order, so the Latin range would break sorting.
    let sql = schema::create_letter_enum().to_string(PostgresQueryBuilder);
    assert_eq!(
        sql,
        r#"CREATE TYPE "book_letter" AS ENUM ('a', 'b', 'c', 'd', 'dd', 'e', 'g', 'h', 'y', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'x')"#
    );
    assert!(sql.contains("'h', 'y', 'k'"), "Y must come before K");
    assert!(!sql.contains("'i'"), "the book has no CHỮ I section");
    for missing in ["'f'", "'j'", "'w'", "'z'"] {
        assert!(!sql.contains(missing), "the book has no {missing} section");
    }
}

#[test]
fn the_glyph_kind_enum_has_all_four_values() {
    assert_eq!(
        schema::create_glyph_kind_enum().to_string(PostgresQueryBuilder),
        r#"CREATE TYPE "glyph_kind" AS ENUM ('bmp', 'ext_b', 'pua', 'image_only')"#
    );
}

// ── The generated column ─────────────────────────────────────────────────────

#[test]
fn the_search_doc_column_concatenates_three_weight_ranks() {
    let sql = schema::create_entry_table().to_string(PostgresQueryBuilder);

    // A the reading (forward lookup) · B the definition · C sub-entry text (reverse lookup).
    for (col, weight) in [("reading", "'A'"), ("gloss", "'B'"), ("sub_text", "'C'")] {
        let want = format!(
            r#"setweight(to_tsvector(CAST('vi_simple' AS regconfig), coalesce("{col}", '')), {weight})"#
        );
        assert!(
            sql.contains(&want),
            "rank {weight} missing for column {col}"
        );
    }
    assert!(
        sql.contains("GENERATED ALWAYS AS"),
        "it must be a generated column"
    );
    assert!(
        sql.contains("STORED"),
        "it must be stored, not recomputed on every read"
    );
}

#[test]
fn regconfig_uses_a_cast_rather_than_an_oid() {
    // `PgFunc::to_tsvector` takes regconfig as a u32 OID, and the OID of `vi_simple` only
    // exists once the configuration has been created — unknowable while writing the migration.
    // The cast lets Postgres resolve the OID at parse time.
    let sql = schema::create_entry_table().to_string(PostgresQueryBuilder);
    assert!(sql.contains("CAST('vi_simple' AS regconfig)"));
}

#[test]
fn the_pos_column_is_an_array_because_548_entries_carry_two_labels() {
    let sql = schema::create_entry_table().to_string(PostgresQueryBuilder);
    assert!(sql.contains(r#""pos" entry_pos[] NOT NULL"#));
}

#[test]
fn the_printed_page_number_may_be_null() {
    // Three pages have no printed number: the 2026 cover, the original cover, the LƯU Ý page.
    let sql = schema::create_page_table().to_string(PostgresQueryBuilder);
    assert!(
        sql.contains(r#""printed_page" smallint,"#),
        "it must not be NOT NULL"
    );
    assert!(sql.contains(r#""pdf_page" smallint NOT NULL"#));
}

#[test]
fn the_glyph_may_be_null_because_29_entries_use_an_image() {
    let sql = schema::create_entry_table().to_string(PostgresQueryBuilder);
    assert!(
        sql.contains(r#""glyph_id" integer,"#),
        "it must not be NOT NULL"
    );
}

// ── Index ────────────────────────────────────────────────────────────────────

#[test]
fn the_full_text_index_uses_gin() {
    assert_eq!(
        schema::create_search_doc_index().to_string(PostgresQueryBuilder),
        r#"CREATE INDEX "idx_entry_search_doc" ON "entry" USING gin ("search_doc")"#
    );
}

#[test]
fn the_unaccented_index_uses_the_trigram_operator_class() {
    // `gin_trgm_ops` is an operator class, declared as an Iden rather than a string.
    assert_eq!(
        schema::create_reading_norm_trgm_index().to_string(PostgresQueryBuilder),
        r#"CREATE INDEX "idx_entry_reading_norm_trgm" ON "entry" USING gin ("reading_norm" gin_trgm_ops)"#
    );
}

// ── The boundary of the "no magic strings" promise ───────────────────────────

#[test]
fn exactly_three_ddl_statements_no_builder_can_express() {
    let stmts = pg_ddl::all_statements();
    assert_eq!(
        stmts.len(),
        3,
        "more than three means the promise has slipped"
    );
}

#[test]
fn the_unaccent_wrapper_must_be_immutable_and_pin_its_dictionary() {
    let sql = pg_ddl::create_immutable_unaccent();
    // `unaccent(text)` is STABLE, so it CANNOT be used in a generated column or an
    // expression index. Only the wrapper can.
    assert!(sql.contains("IMMUTABLE"), "it must declare IMMUTABLE");
    // The two-argument form pins the dictionary — only then is IMMUTABLE honest.
    assert!(
        sql.contains("unaccent('unaccent', $1)"),
        "it must use the two-argument form"
    );
    assert!(sql.contains("immutable_unaccent(text)"));
}

#[test]
fn the_search_configuration_copies_simple_and_uses_no_stemmer() {
    let sql = pg_ddl::create_text_search_config();
    assert_eq!(
        sql,
        "CREATE TEXT SEARCH CONFIGURATION vi_simple (COPY = simple)"
    );
    // English or French style stemming is meaningless for Vietnamese and corrupts meaning.
    assert!(!sql.contains("english"));
    assert!(!sql.contains("french"));
}

#[test]
fn the_unaccent_dictionary_is_chained_ahead_of_simple() {
    let sql = pg_ddl::alter_text_search_config_mapping();
    assert!(
        sql.contains("WITH unaccent, simple"),
        "fold the accents first, then the raw form"
    );
    assert!(sql.contains("vi_simple"));
}

#[test]
fn the_drop_statements_cover_everything_created() {
    let drops = pg_ddl::all_drop_statements();
    assert_eq!(drops.len(), 2);
    assert!(drops[0].contains("DROP TEXT SEARCH CONFIGURATION"));
    assert!(drops[1].contains("DROP FUNCTION"));
}

#[test]
fn the_two_required_extensions() {
    use dnqatv_migration::pg_ddl::PgExtension;
    let names: Vec<&str> = PgExtension::ALL.iter().map(|e| e.name()).collect();
    assert_eq!(names, vec!["unaccent", "pg_trgm"]);
}
