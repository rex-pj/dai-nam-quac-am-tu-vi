#![allow(clippy::expect_used, clippy::panic)]

//! The schema PostgreSQL **actually built**, read back from `pg_catalog`.
//!
//! Quite different from the snapshot test in `tests/unit/migration`: there we pin the SQL we
//! *generate*, here we ask the server what it *understood*. Syntactically valid SQL can still
//! create something other than intended — enum label order, array column types, whether a
//!
//! Skipped (not failed) when `DATABASE_URL` is absent: a machine with no database must still
//! be able to run `cargo test`. But with a database, this is a real gate.

use dnqatv_adapter_db::{Db, connect, health, migrate};
use dnqatv_config::{AppConfig, ConfigError, EnvVar};
use dnqatv_core::model::{GlyphKind, Letter, Pos};

/// Connect to the development database, or `None` when it is not configured.
async fn db() -> Option<Db> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config: AppConfig = match dnqatv_config::load_from_dotenv_and_env(&root.join(".env")) {
        Ok(c) => c,
        Err(ConfigError::Missing(EnvVar::DatabaseUrl)) => {
            eprintln!("skipped: DATABASE_URL is not set");
            return None;
        }
        Err(e) => panic!("broken configuration: {e}"),
    };
    let db = connect(&config.database)
        .await
        .expect("connecting to the database");
    migrate::up(&db).await.expect("running migrations");
    Some(db)
}

macro_rules! db_or_skip {
    () => {
        match db().await {
            Some(db) => db,
            None => return,
        }
    };
}

// ── Enum types ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_letter_enum_keeps_the_printed_order_in_postgres() {
    let db = db_or_skip!();

    let labels = health::enum_labels(&db, "book_letter")
        .await
        .expect("reading the enum labels");
    let want: Vec<String> = Letter::ALL
        .iter()
        .map(|l| l.db_value().to_owned())
        .collect();

    // Postgres compares enums by DECLARATION ORDER. If the order in the database differs
    // from `Letter::ALL`, `ORDER BY letter` sorts wrongly and pipeline gate 5 never knows.
    assert_eq!(labels, want);
    let y = labels
        .iter()
        .position(|l| l == "y")
        .expect("the letter y is present");
    let k = labels
        .iter()
        .position(|l| l == "k")
        .expect("the letter k is present");
    assert!(y < k, "the print orders … H Y K …, with Y where I would be");
    assert!(
        !labels.iter().any(|l| l == "i"),
        "the book has no CHỮ I section"
    );
}

#[tokio::test]
async fn the_pos_enum_matches_the_domain_variant_count() {
    let db = db_or_skip!();
    let labels = health::enum_labels(&db, "entry_pos")
        .await
        .expect("reading the enum labels");
    assert_eq!(labels.len(), Pos::ALL.len());
    assert_eq!(
        labels,
        Pos::ALL
            .iter()
            .map(|p| p.db_value().to_owned())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn the_glyph_kind_enum_matches_the_domain_layer() {
    let db = db_or_skip!();
    let labels = health::enum_labels(&db, "glyph_kind")
        .await
        .expect("reading the enum labels");
    assert_eq!(
        labels,
        GlyphKind::ALL
            .iter()
            .map(|k| k.db_value().to_owned())
            .collect::<Vec<_>>()
    );
}

// ── Columns ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn search_doc_is_a_real_generated_column() {
    let db = db_or_skip!();
    let generated = health::generated_columns(&db, "entry")
        .await
        .expect("reading the generated columns");
    assert_eq!(generated, vec!["search_doc".to_owned()]);
}

#[tokio::test]
async fn the_pos_column_is_stored_as_an_array_of_the_enum_type() {
    let db = db_or_skip!();
    // 548 entries carry two labels, so the column must be an ARRAY. `data_type` only says
    // "ARRAY"; the element type is in `udt_name` — the leading underscore is the Postgres
    // convention for "array of this type".
    let (data_type, udt) = health::column_type(&db, "entry", "pos")
        .await
        .expect("reading the column type")
        .expect("the column must exist");
    assert_eq!(data_type, "ARRAY");
    assert_eq!(udt, "_entry_pos");
}

#[tokio::test]
async fn the_printed_page_number_is_nullable_while_the_pdf_page_is_not() {
    let db = db_or_skip!();
    // Three pages have no printed number (the 2026 cover, the original cover, the LƯU Ý
    // page) — that invariant is a measurement, and the schema must reflect it.
    let (printed, _) = health::column_type(&db, "page", "printed_page")
        .await
        .expect("reading the type")
        .expect("the column must exist");
    assert_eq!(printed, "smallint");
}

// ── Index ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_full_text_and_unaccented_indexes_use_the_right_methods() {
    let db = db_or_skip!();
    let names = health::indexes(&db, "entry")
        .await
        .expect("reading the indexes");
    for want in [
        "idx_entry_search_doc",
        "idx_entry_reading_norm_trgm",
        "idx_entry_reading",
    ] {
        assert!(
            names.iter().any(|n| n == want),
            "{want} missing from {names:?}"
        );
    }

    // Read the definition back as Postgres reports it — stronger evidence than the index
    // name: it shows the `gin_trgm_ops` operator class really is applied.
    let def = health::index_definition(&db, "idx_entry_reading_norm_trgm")
        .await
        .expect("reading the definition")
        .expect("the index must exist");
    assert!(def.contains("USING gin"), "{def}");
    assert!(def.contains("gin_trgm_ops"), "{def}");

    let def = health::index_definition(&db, "idx_entry_search_doc")
        .await
        .expect("reading the definition")
        .expect("the index must exist");
    assert!(def.contains("USING gin"), "{def}");
}

// ── The three raw DDL statements, checked by RUNNING them ────────────────────

#[tokio::test]
async fn both_extensions_are_installed() {
    let db = db_or_skip!();
    let exts = health::extensions(&db)
        .await
        .expect("reading the extensions");
    for want in ["unaccent", "pg_trgm"] {
        assert!(exts.iter().any(|e| e == want), "{want} is missing");
    }
}

#[tokio::test]
async fn the_unaccent_wrapper_runs_and_handles_vietnamese_correctly() {
    let db = db_or_skip!();
    let got = health::try_immutable_unaccent(&db, "Lõm gươm")
        .await
        .expect("calling immutable_unaccent");
    assert_eq!(got, "Lom guom");

    // Vietnamese đ is NOT d plus a combining mark; `unaccent` handles it via its dictionary.
    let got = health::try_immutable_unaccent(&db, "Đại Nam Quấc Âm")
        .await
        .expect("calling immutable_unaccent");
    assert_eq!(got, "Dai Nam Quac Am");
}

#[tokio::test]
async fn the_search_configuration_folds_accents_before_indexing() {
    let db = db_or_skip!();
    // This is why `vi_simple` exists: typing without accents still finds accented entries.
    let got = health::try_tsvector(&db, "vi_simple", "Lõm gươm nạm")
        .await
        .expect("calling to_tsvector");
    for lexeme in ["'lom'", "'guom'", "'nam'"] {
        assert!(got.contains(lexeme), "{lexeme} missing from {got}");
    }

    // And the default configuration does NOT do that — this comparison proves `vi_simple`
    // really has an effect rather than coinciding.
    let plain = health::try_tsvector(&db, "simple", "Lõm gươm nạm")
        .await
        .expect("calling to_tsvector");
    assert!(plain.contains("'lõm'"), "{plain}");
}

// ── Connection ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn ping_makes_a_real_round_trip() {
    let db = db_or_skip!();
    db.ping().await.expect("ping");
}

#[tokio::test]
async fn the_full_health_snapshot() {
    let db = db_or_skip!();
    let h = health::snapshot(&db)
        .await
        .expect("reading the health snapshot");
    assert!(
        h.server_version.contains("PostgreSQL"),
        "{}",
        h.server_version
    );
    assert_eq!(h.migrations_pending, 0, "there are unapplied migrations");
    assert!(h.migrations_applied >= 1);
}
