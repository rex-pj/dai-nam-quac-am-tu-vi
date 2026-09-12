// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
#![allow(clippy::expect_used, clippy::panic)]

//! Reading fields out of a nested wiki template.
//!
//! Every example is taken verbatim from `data/wikisource/pages.jsonl`, with the `Trang:`
//! page it came from so it can be checked against the scan behind it.

use dnqatv_witness_core::record::read_page;
use dnqatv_witness_core::template::template_fields;

#[test]
fn a_plain_call_splits_into_its_fields() {
    // v1/162 — the headword 篡 Choán.
    let t = template_fields(
        "{{DNQATV/mục|篡|Choán||n|Chiếm cứ, giành lấy.}}",
        "DNQATV/mục",
    );
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].field(0), Some("篡"));
    assert_eq!(t[0].field(1), Some("Choán"));
    assert_eq!(t[0].field(2), Some(""));
    assert_eq!(t[0].field(3), Some("n"));
    assert_eq!(t[0].field(4), Some("Chiếm cứ, giành lấy."));
}

#[test]
fn a_nested_call_does_not_shift_the_later_fields() {
    // v2/477 — the shape has no code point, so the transcriber described it IN the glyph
    // field. Splitting on a bare `|` would read "trên:壯, dưới:卵}}" as the reading.
    let src = "{{DNQATV/mục|{{?|trên:壯, dưới:卵}}|Trấng||n|Cái vỏ chứa vật nôi sinh.}}";
    let t = template_fields(src, "DNQATV/mục");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].field(0), Some("{{?|trên:壯, dưới:卵}}"));
    assert_eq!(t[0].field(1), Some("Trấng"));
    assert_eq!(t[0].field(3), Some("n"));
}

#[test]
fn several_calls_are_returned_in_document_order() {
    let src = "{{DNQATV/mục|甲|Giáp||c|A.}}\n{{DNQATV/mục|乙|Ất||c|B.}}";
    let t = template_fields(src, "DNQATV/mục");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].field(1), Some("Giáp"));
    assert_eq!(t[1].field(1), Some("Ất"));
    assert!(
        t[0].at < t[1].at,
        "offsets keep the reading order of the page"
    );
}

#[test]
fn a_missing_field_reads_as_absent_not_as_a_panic() {
    // A short call must not panic, and must not pretend the field was written as empty:
    // absent and empty are different things, and only the caller knows which it can accept.
    let t = template_fields("{{DNQATV/mục|甲|Giáp}}", "DNQATV/mục");
    assert_eq!(t[0].field(1), Some("Giáp"));
    assert_eq!(t[0].field(4), None);
}

#[test]
fn a_sub_entry_belongs_to_the_headword_above_it() {
    let src = "{{DNQATV/mục|阿|A||c|Đèo, nương dựa.}}\
               {{DNQATV/nghĩa||- ý|Dua theo một ý.}}\
               {{DNQATV/nghĩa|太 -|Thái -|Gươm báu trong nước.}}";
    let e = read_page(src, "Trang:test/1", Some(3));
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].reading, "A");
    assert_eq!(e[0].subs.len(), 2);
    assert_eq!(e[0].subs[1].han, "太 -");
    assert_eq!(e[0].subs[1].form, "Thái -");
    assert_eq!(e[0].quality, Some(3));
}

#[test]
fn a_sub_entry_before_any_headword_is_dropped() {
    // The tail of an entry that began on the previous page. Attaching it to the next
    // headword would invent a relationship the page does not state.
    let src = "{{DNQATV/nghĩa||- rơi|còn lại từ trang trước}}{{DNQATV/mục|甲|Giáp||c|A.}}";
    let e = read_page(src, "Trang:test/2", Some(1));
    assert_eq!(e.len(), 1);
    assert!(e[0].subs.is_empty());
}

#[test]
fn a_piped_wikilink_does_not_shift_the_later_fields() {
    // v1/23 — the transcriber linked the glyph to Wiktionary. The `|` inside `[[…|…]]` is
    // the link's own separator, not a field separator: splitting there read the reading as
    // "阿]]" and put eight rows into witness-entry-only-there.toml that were this parser's
    // doing, not a difference between the two editions.
    let t = template_fields(
        "{{DNQATV/mục|[[wikt:阿|阿]]|A||c|Đèo, nương dựa, phụ theo.}}",
        "DNQATV/mục",
    );
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].field(0), Some("[[wikt:阿|阿]]"));
    assert_eq!(t[0].field(1), Some("A"));
    assert_eq!(t[0].field(3), Some("c"));
    assert_eq!(t[0].field(4), Some("Đèo, nương dựa, phụ theo."));
}

#[test]
fn a_separator_after_a_closed_wikilink_still_separates() {
    // The link must not swallow the rest of the call: once it closes, `|` divides again.
    let t = template_fields("{{DNQATV/mục|[[a|b]]|R||n|G.}}", "DNQATV/mục");
    assert_eq!(t[0].field(0), Some("[[a|b]]"));
    assert_eq!(t[0].field(1), Some("R"));
    assert_eq!(t[0].field(4), Some("G."));
}
