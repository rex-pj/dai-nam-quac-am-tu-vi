// clippy only relaxes panic/expect inside #[test] functions; the helper here sits outside.
#![allow(clippy::expect_used, clippy::panic)]

//! The character conservation law.
//!
//! This gate replaces metrics like "92% of entries have a definition" — that number once
//! hid a bug that lost 355,833 characters.

use dnqatv_parse_core::span::{Span, check_coverage, visible_len};

fn span(text: &str, sub: &str) -> Span {
    let start = text.find(sub).expect("substring found");
    Span::new(text, start, start + sub.len()).expect("valid span")
}

// ── Span ─────────────────────────────────────────────────────────────────────

#[test]
fn a_span_rejects_a_reversed_or_empty_range() {
    let t = "abc";
    assert!(Span::new(t, 2, 1).is_err());
    assert!(Span::new(t, 1, 1).is_err());
}

#[test]
fn a_span_rejects_a_range_past_the_length() {
    let t = "abc";
    assert!(Span::new(t, 0, 4).is_err());
}

#[test]
fn a_span_rejects_cutting_a_multi_byte_character() {
    // "𨰲" takes 4 bytes. Cutting into the middle would make an invalid string.
    let t = "𨰲Lõm";
    assert!(Span::new(t, 0, 2).is_err(), "cutting inside an Ext-B glyph");
    assert!(Span::new(t, 0, 4).is_ok(), "cutting on a boundary is fine");
}

#[test]
fn a_span_slices_the_right_text() {
    let t = "𨰲  Lõm  n.";
    assert_eq!(span(t, "𨰲").slice(t), "𨰲");
    assert_eq!(span(t, "Lõm").slice(t), "Lõm");
    assert_eq!(span(t, "n.").slice(t), "n.");
}

// ── Counting visible characters ──────────────────────────────────────────────

#[test]
fn visible_len_ignores_whitespace() {
    // 𨰲 · L · õ · m · n · .  -> six visible characters
    assert_eq!(visible_len("𨰲  Lõm  n."), 6);
    assert_eq!(visible_len("   "), 0);
    assert_eq!(visible_len(""), 0);
}

#[test]
fn visible_len_counts_characters_not_bytes() {
    // Ext-B takes 4 bytes but is ONE character.
    assert_eq!(visible_len("𨰲"), 1);
    assert_eq!("𨰲".len(), 4);
}

// ── Coverage ─────────────────────────────────────────────────────────────────

#[test]
fn covering_every_visible_character_passes() {
    let t = "𨰲  Lõm  n.";
    let spans = [span(t, "𨰲"), span(t, "Lõm"), span(t, "n.")];
    let cov = check_coverage(t, &spans);
    assert!(cov.is_exact_partition(), "something is uncovered: {cov:?}");
}

#[test]
fn a_missing_field_is_caught() {
    // Forgetting the label "n." — exactly the bug a coverage ratio cannot see.
    let t = "𨰲  Lõm  n.";
    let spans = [span(t, "𨰲"), span(t, "Lõm")];
    let cov = check_coverage(t, &spans);
    assert!(!cov.is_exact_partition());
    assert_eq!(cov.uncovered.len(), 1);
    let (a, b) = cov.uncovered[0];
    assert_eq!(&t[a..b], "n.");
}

#[test]
fn counting_a_stretch_twice_is_caught_too() {
    // The same stretch filed into two fields — the inverse of loss, and just as dangerous.
    let t = "abc def";
    let s = span(t, "abc");
    let cov = check_coverage(t, &[s, s]);
    assert!(!cov.is_exact_partition());
    assert_eq!(cov.overlapping.len(), 1);
    assert!(!cov.uncovered.is_empty(), "def is still uncovered");
}

#[test]
fn whitespace_need_not_be_covered() {
    // Whitespace is typesetting distance, not content.
    let t = "  a   b  ";
    let spans = [span(t, "a"), span(t, "b")];
    assert!(check_coverage(t, &spans).is_exact_partition());
}

#[test]
fn an_empty_or_whitespace_only_string_always_passes() {
    assert!(check_coverage("", &[]).is_exact_partition());
    assert!(check_coverage("    ", &[]).is_exact_partition());
}

#[test]
fn several_uncovered_stretches_are_grouped_into_several_ranges() {
    let t = "aa bb cc";
    let cov = check_coverage(t, &[span(t, "bb")]);
    assert_eq!(cov.uncovered.len(), 2, "aa and cc are two separate ranges");
}
