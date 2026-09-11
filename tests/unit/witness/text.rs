// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
#![allow(clippy::expect_used, clippy::panic)]

//! Making two editions comparable without changing either of them.
//!
//! The keys built here never reach the data — they exist only for the moment of asking
//! "are these two the same line?".

use dnqatv_witness_core::text::{fold, placeholder_key, plain};

#[test]
fn italic_markup_is_not_part_of_the_text() {
    assert_eq!(plain("- đợi ''hoặc'' đợi -"), "- đợi hoặc đợi -");
    assert_eq!(plain("'''Đậm''' nét"), "Đậm nét");
}

#[test]
fn an_editors_note_is_dropped_but_the_text_around_it_stays() {
    assert_eq!(plain("{{?|trên:⺮, dưới:摧}}"), "");
    assert_eq!(plain("Toi {{?|trên:⺮}} nhọn"), "Toi nhọn");
}

#[test]
fn a_link_shows_its_label_not_its_target() {
    assert_eq!(plain("[[Trang:X|chữ nầy]]"), "chữ nầy");
    assert_eq!(plain("[[chữ nho]]"), "chữ nho");
}

#[test]
fn every_placeholder_shape_folds_to_one_token() {
    // The 2026 edition uses ― (U+2015) mostly, — (U+2014) on the DẤU RIÊNG page, – (U+2013)
    // on 129 lines and `--` on 24. Wikisource writes a plain `-` for all of them, and for
    // the Han-column `|` as well.
    let ours = placeholder_key("| 男 婚 ― nam hôn");
    let theirs = placeholder_key("| 男 婚 - nam hôn");
    assert_eq!(ours, theirs);
    assert_eq!(placeholder_key("a -- b"), placeholder_key("a ― b"));
}

#[test]
fn the_position_of_a_placeholder_still_counts() {
    // Folding the MARK must not fold away WHERE it sits — that is the whole question.
    assert_ne!(placeholder_key("― gươm"), placeholder_key("gươm ―"));
}

#[test]
fn folding_ignores_punctuation_and_case_but_not_words() {
    assert_eq!(fold("Nữ ― nam hôn."), fold("nữ - nam hôn"));
    assert_ne!(fold("Nữ ― nam hôn"), fold("Nữ ― nam hân"));
}

#[test]
fn the_column_boundary_of_the_two_editions_can_be_compared() {
    // p.313 — after the re-split we hold han `女 | 男 婚` + form `Nữ ― nam hôn`;
    // the 1895 transcription holds han `女 - 男 婚` + form `Nữ - nam hôn`. Same break.
    assert_eq!(
        placeholder_key("Nữ ― nam hôn"),
        placeholder_key("Nữ - nam hôn")
    );
    // Before the fix our form began with the Han column, and that is a real difference.
    assert_ne!(
        placeholder_key("| 男 婚 Nữ ― nam hôn"),
        placeholder_key("Nữ - nam hôn")
    );
}
