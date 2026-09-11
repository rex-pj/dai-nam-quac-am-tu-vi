// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
#![allow(clippy::expect_used, clippy::panic)]

//! Lining the two editions up and naming the ways they differ.

use dnqatv_witness_core::align::{DivergenceKind, OurEntry, OurSub, compare};
use dnqatv_witness_core::record::read_page;

fn ours(reading: &str, glyph: Option<char>, subs: &[(&str, &str, &str)]) -> OurEntry {
    OurEntry {
        needs_review: false,
        gloss: String::new(),
        seq: 1,
        pdf_page: 1,
        printed_page: 1,
        glyph,
        reading: reading.to_owned(),
        subs: subs
            .iter()
            .map(|(h, f, d)| OurSub {
                needs_review: false,
                han_form: (!h.is_empty()).then(|| (*h).to_owned()),
                form: (*f).to_owned(),
                definition: (*d).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn two_editions_that_agree_produce_no_finding() {
    let a = [ours(
        "Giá",
        Some('嫁'),
        &[("女 | 男 婚", "Nữ ― nam hôn", "Gái thì gả trai thì cưới.")],
    )];
    let b = read_page(
        "{{DNQATV/mục|嫁|Giá||n|Gả chồng.}}{{DNQATV/nghĩa|女 - 男 婚|Nữ - nam hôn|Gái thì gả trai thì cưới.}}",
        "Trang:test/1",
        Some(3),
    );
    let r = compare(&a, &b);
    assert_eq!(r.matched, 1);
    assert_eq!(r.subs_compared, 1);
    assert_eq!(
        r.contradictions(),
        0,
        "same break, different mark — not a difference"
    );
}

#[test]
fn a_different_column_break_is_a_contradiction() {
    // The pre-fix split: the Han column leaked into the Quốc ngữ form.
    let a = [ours(
        "Giá",
        Some('嫁'),
        &[("女", "| 男 婚 Nữ ― nam hôn", "Gái thì gả trai thì cưới.")],
    )];
    let b = read_page(
        "{{DNQATV/mục|嫁|Giá||n|Gả chồng.}}{{DNQATV/nghĩa|女 - 男 婚|Nữ - nam hôn|Gái thì gả trai thì cưới.}}",
        "Trang:test/1",
        Some(3),
    );
    let r = compare(&a, &b);
    assert_eq!(r.count(DivergenceKind::ColumnBoundary), 1);
    assert_eq!(r.contradictions(), 1);
}

#[test]
fn two_glyphs_that_differ_are_a_contradiction() {
    let a = [ours("Choản", Some('篡'), &[])];
    let b = read_page(
        "{{DNQATV/mục|𣑕|Choản||n|(Coi chữ chủn).}}",
        "Trang:test/1",
        Some(3),
    );
    let r = compare(&a, &b);
    assert_eq!(r.count(DivergenceKind::GlyphDiffers), 1);
    assert!(DivergenceKind::GlyphDiffers.is_contradiction());
}

#[test]
fn a_shape_note_where_we_have_an_image_is_an_opportunity_not_a_fault() {
    // Our side has no character because the 2026 edition set an image; the other edition
    // could not encode it either and described the parts instead.
    let a = [ours("Trấng", None, &[])];
    let b = read_page(
        "{{DNQATV/mục|{{?|trên:壯, dưới:卵}}|Trấng||n|Cái vỏ chứa vật nôi sinh.}}",
        "Trang:test/1",
        Some(1),
    );
    let r = compare(&a, &b);
    assert_eq!(r.count(DivergenceKind::ShapeNoteAvailable), 1);
    assert_eq!(r.contradictions(), 0);
    let d = &r.divergences[0];
    assert_eq!(d.theirs, "trên:壯, dưới:卵");
    assert_eq!(
        d.witness_quality,
        Some(1),
        "a level-1 page is a weaker witness; say so"
    );
}

#[test]
fn an_entry_only_one_edition_has_is_reported_on_the_right_side() {
    let a = [ours("Giáp", Some('甲'), &[]), ours("Ất", Some('乙'), &[])];
    let b = read_page(
        "{{DNQATV/mục|甲|Giáp||c|A.}}{{DNQATV/mục|丙|Bính||c|C.}}{{DNQATV/mục|乙|Ất||c|B.}}",
        "Trang:test/1",
        Some(3),
    );
    let r = compare(&a, &b);
    assert_eq!(
        r.matched, 2,
        "the walk steps over the extra entry and falls back into line"
    );
    assert_eq!(r.count(DivergenceKind::EntryOnlyThere), 1);
    assert_eq!(r.count(DivergenceKind::EntryOnlyHere), 0);
    assert_eq!(
        r.contradictions(),
        0,
        "the 1895 print simply has an entry the 2026 one folds in"
    );
}

#[test]
fn an_uncertain_pairing_is_counted_not_guessed() {
    // Two sub-entries with the same definition and no distinguishing form: the partner
    // cannot be told apart, so none is chosen.
    let a = [ours("A", Some('阿'), &[("", "― x", "id.")])];
    let b = read_page(
        "{{DNQATV/mục|阿|A||c|Đèo.}}{{DNQATV/nghĩa||- y|id.}}{{DNQATV/nghĩa||- z|id.}}",
        "Trang:test/1",
        Some(3),
    );
    let r = compare(&a, &b);
    assert_eq!(r.subs_compared, 0);
    assert_eq!(r.subs_no_partner, 1);
    assert_eq!(r.contradictions(), 0);
}
