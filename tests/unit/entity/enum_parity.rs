#![allow(clippy::expect_used, clippy::panic)]

//! Enum parity between the domain layer and the storage layer.
//!
//! Two lists exist because Rust orphan rules forbid implementing `ActiveEnum` for a type
//! from another crate. That is the only thing that can drift at the entity layer, so it is
//! checked head-on — variant count, stored value, and both conversion directions.

use dnqatv_core::model::{GlyphKind as CoreGlyphKind, Letter as CoreLetter, Pos as CorePos};
use dnqatv_entity::{BookLetter, EntryPos, GlyphKind};
use sea_orm::ActiveEnum;
use sea_orm::strum::IntoEnumIterator;

#[test]
fn pos_labels_match_in_count_value_and_both_directions() {
    assert_eq!(EntryPos::iter().count(), CorePos::ALL.len());
    for core in CorePos::ALL {
        let db: EntryPos = core.into();
        assert_eq!(
            db.to_value(),
            core.db_value(),
            "the stored value must match"
        );
        assert_eq!(
            CorePos::from(db),
            core,
            "a round trip must return the same value"
        );
    }
}

#[test]
fn letters_match_in_count_value_and_order() {
    assert_eq!(BookLetter::iter().count(), CoreLetter::ALL.len());
    for core in CoreLetter::ALL {
        let db: BookLetter = core.into();
        assert_eq!(db.to_value(), core.db_value());
        assert_eq!(CoreLetter::from(db), core);
    }

    // The declaration order must match: Postgres compares enums by declaration order, so a
    // mismatch makes `ORDER BY letter` sort wrongly with no warning.
    let entity_order: Vec<String> = BookLetter::iter().map(|l| l.to_value()).collect();
    let core_order: Vec<String> = CoreLetter::ALL
        .iter()
        .map(|l| l.db_value().to_owned())
        .collect();
    assert_eq!(entity_order, core_order);

    let y = entity_order
        .iter()
        .position(|l| l == "y")
        .expect("y is present");
    let k = entity_order
        .iter()
        .position(|l| l == "k")
        .expect("k is present");
    assert!(y < k, "the print orders … H Y K …");
}

#[test]
fn glyph_kinds_match() {
    assert_eq!(GlyphKind::iter().count(), CoreGlyphKind::ALL.len());
    for core in CoreGlyphKind::ALL {
        let db: GlyphKind = core.into();
        assert_eq!(db.to_value(), core.db_value());
        assert_eq!(CoreGlyphKind::from(db), core);
    }
}

#[test]
fn postgres_type_names_match_the_migration() {
    // These three names also appear in `migration::idens::PgEnum`; a mismatch is a runtime
    // error, and a runtime error in the storage layer is very hard to trace.
    assert_eq!(EntryPos::name().to_string(), "entry_pos");
    assert_eq!(BookLetter::name().to_string(), "book_letter");
    assert_eq!(GlyphKind::name().to_string(), "glyph_kind");
}
