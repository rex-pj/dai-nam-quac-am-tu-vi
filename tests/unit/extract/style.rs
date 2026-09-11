//! Inferring the text style from the font name.
//!
//! The font list is taken straight from the PDF in docs/ — every name below really occurs
//! in the document.

use dnqatv_extract_core::style::TextStyle;

#[test]
fn han_nom_fonts_are_recognised() {
    for name in [
        "AAAAAA+NomNaTong-Regular",
        "Nom#20Na#20Tong#20Regular",
        "BabelStone#20Han#20Regular",
        "AAAAAA+ZenKai-Medium",
        "UnBatang#20Bold",
    ] {
        assert_eq!(TextStyle::from_base_font(name), TextStyle::Han, "{name}");
    }
}

#[test]
fn han_fonts_are_checked_before_bold() {
    // `UnBatang Bold` contains "Bold" yet is a Han font. The check order decides the result.
    assert_eq!(TextStyle::from_base_font("UnBatang-Bold"), TextStyle::Han);
}

#[test]
fn italic_and_bold_fonts() {
    assert_eq!(
        TextStyle::from_base_font("Liberation#20Serif#20Italic"),
        TextStyle::Italic
    );
    assert_eq!(
        TextStyle::from_base_font("AAAAAA+NotoSerif-Italic"),
        TextStyle::Italic
    );
    assert_eq!(
        TextStyle::from_base_font("Liberation#20Serif#20Bold"),
        TextStyle::Bold
    );
    assert_eq!(
        TextStyle::from_base_font("AAAAAA+DejaVuSerif-Bold"),
        TextStyle::Bold
    );
}

#[test]
fn regular_is_the_default() {
    for name in [
        "Liberation#20Serif#20Regular",
        "AAAAAA+DejaVuSerif",
        "Noto#20Serif#20Regular",
        "Helvetica",
        "",
    ] {
        assert_eq!(
            TextStyle::from_base_font(name),
            TextStyle::Regular,
            "{name}"
        );
    }
}

#[test]
fn subset_prefixes_and_encoded_spaces_are_stripped() {
    // PDF names subset fonts `AAAAAA+Name` and encodes spaces as `#20`.
    assert_eq!(
        TextStyle::from_base_font("BCDEFG+Liberation#20Serif#20Italic"),
        TextStyle::Italic
    );
}

#[test]
fn every_style_has_a_round_trippable_db_value() {
    let values: Vec<&str> = TextStyle::ALL.iter().map(|s| s.db_value()).collect();
    assert_eq!(values, vec!["regular", "italic", "bold", "han"]);
    assert_eq!(TextStyle::ALL.len(), 4);
}
