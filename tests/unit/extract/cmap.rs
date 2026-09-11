// clippy only relaxes panics inside #[test] functions; the fixture helper sits outside
// one, so it must be declared explicitly. This whole file is test code, where panicking is correct.
#![allow(clippy::panic)]

//! Decoding a `/ToUnicode` CMap.
//!
//! The fixtures are of two deliberately separated kinds:
//!   * `cmap_all_forms.txt` — HAND-WRITTEN from PDF 32000-1 §9.10.3, covering every syntax form.
//!   * `cmap_*_slice.txt`   — CUT FROM the actual PDF in docs/, used as behaviour specification.
//!
//! That boundary matters: the cut fixtures prove we can read the real file, while the
//! hand-written one proves we follow the spec even for forms the real file never uses.

use dnqatv_extract_core::{CMapError, DecodedGlyph, ToUnicodeCMap};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/");
    let full = format!("{path}{name}");
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("reading fixture {full}: {e}"))
}

// ── Syntax forms ─────────────────────────────────────────────────────────────

#[test]
fn reads_a_simple_bfchar() {
    let m = ToUnicodeCMap::parse(&fixture("cmap_all_forms.txt")).expect("parse");
    assert_eq!(m.decode(0x0003), DecodedGlyph::Mapped("A".to_owned()));
    assert_eq!(m.decode(0x0004), DecodedGlyph::Mapped("阿".to_owned()));
}

#[test]
fn reads_a_bfrange_with_a_single_destination() {
    // <0010> <0012> <0061>  →  0x10=a, 0x11=b, 0x12=c
    let m = ToUnicodeCMap::parse(&fixture("cmap_all_forms.txt")).expect("parse");
    assert_eq!(m.decode(0x0010), DecodedGlyph::Mapped("a".to_owned()));
    assert_eq!(m.decode(0x0011), DecodedGlyph::Mapped("b".to_owned()));
    assert_eq!(m.decode(0x0012), DecodedGlyph::Mapped("c".to_owned()));
}

#[test]
fn reads_a_bfrange_with_an_array_of_destinations() {
    // <0020> <0022> [<0301> <0302> <D840DC01>]
    let m = ToUnicodeCMap::parse(&fixture("cmap_all_forms.txt")).expect("parse");
    assert_eq!(
        m.decode(0x0020).text().and_then(|s| s.chars().next()),
        char::from_u32(0x0301)
    );
    assert_eq!(
        m.decode(0x0022).text().and_then(|s| s.chars().next()),
        char::from_u32(0x20001),
        "an array element that is a surrogate pair must join into one Ext-B character"
    );
}

#[test]
fn lowercase_hex_is_read_too() {
    // The UnBatang font in this very PDF writes its bfrange in lowercase hex.
    let m = ToUnicodeCMap::parse(&fixture("cmap_all_forms.txt")).expect("parse");
    assert_eq!(m.decode(0x0030), DecodedGlyph::Mapped("à".to_owned()));
    assert_eq!(m.decode(0x0031), DecodedGlyph::Mapped("á".to_owned()));
}

// ── Fixtures cut from the real file ──────────────────────────────────────────

#[test]
fn decodes_ext_b_nom_from_the_nom_na_tong_font() {
    // Taken straight from the CMap of the Nom Na Tong font embedded in the 2026 edition:
    //     <51B6> <D863DC32>
    // D863 DC32 is the surrogate pair for U+28C32 = 𨰲, the glyph of the entry "Lõm" (printed page 499).
    // Note: surrogate pairs must be MEASURED, not computed in your head — the first attempt
    // read D862DC32, which is 𨠲 (U+28832), a completely different character. This test caught it.
    let m = ToUnicodeCMap::parse(&fixture("cmap_nomnatong_slice.txt")).expect("parse");

    let g = m.decode(0x51B6);
    assert_eq!(g, DecodedGlyph::Mapped("𨰲".to_owned()));

    let ch = g
        .text()
        .and_then(|s| s.chars().next())
        .expect("one character");
    assert_eq!(ch as u32, 0x28C32);
    assert_eq!(
        g.text().map(str::chars).map(Iterator::count),
        Some(1),
        "a surrogate pair must become ONE character, not two"
    );
}

#[test]
fn the_real_fixtures_load_the_expected_number_of_entries() {
    let m = ToUnicodeCMap::parse(&fixture("cmap_nomnatong_slice.txt")).expect("parse");
    assert_eq!(m.len(), 10, "the bfchar block in the slice has 10 entries");

    let un = ToUnicodeCMap::parse(&fixture("cmap_unbatang_bfrange_slice.txt")).expect("parse");
    assert!(!un.is_empty(), "the bfrange slice must load");
}

// ── No fallback ──────────────────────────────────────────────────────────────

#[test]
fn a_code_absent_from_the_table_returns_unmapped_rather_than_a_guess() {
    let m = ToUnicodeCMap::parse(&fixture("cmap_nomnatong_slice.txt")).expect("parse");

    // 0x0041 is not in the slice. The old prototype would return "A" — that is fabricated data.
    assert_eq!(m.decode(0x0041), DecodedGlyph::Unmapped { code: 0x0041 });
    assert_eq!(m.decode(0x0041).text(), None);
    assert!(!m.decode(0x0041).is_mapped());
}

#[test]
fn a_lone_surrogate_is_rejected_rather_than_replaced() {
    // D862 alone is a high surrogate with no pair — a broken UTF-16 string.
    // Many libraries would silently turn it into U+FFFD; here it must be a hard error.
    let src = "1 beginbfchar\n<0001> <D862>\nendbfchar\n";
    let err = ToUnicodeCMap::parse(src).expect_err("must fail");
    assert!(matches!(err, CMapError::LoneSurrogate(_)), "got {err:?}");
}

#[test]
fn a_destination_not_divisible_by_four_is_read_as_a_code_point() {
    // Taken from the real file (DejaVu Serif Bold font, page 11): <0cfc> <0cff> <1d400>.
    // The PDF producer writes characters beyond the BMP as DIRECT CODE POINTS here, while
    // the Nom Na Tong font writes 8-digit SURROGATE PAIRS. Both conventions in one file.
    //
    // They are distinguishable without guessing: a length not divisible by 4 cannot be UTF-16BE.
    let src = "1 beginbfrange\n<0cfc> <0cff> <1d400>\nendbfrange\n";
    let m = ToUnicodeCMap::parse(src).expect("valid range");
    assert_eq!(m.decode(0x0CFC).text(), Some("\u{1D400}"));
    assert_eq!(m.decode(0x0CFF).text(), Some("\u{1D403}"));

    // Three digits is also a code point, not an error.
    let one = "1 beginbfchar\n<0001> <ABC>\nendbfchar\n";
    let m = ToUnicodeCMap::parse(one).expect("valid code point");
    assert_eq!(m.decode(0x0001).text(), Some("\u{0ABC}"));
}

#[test]
fn an_invalid_code_point_is_rejected() {
    // 110000 exceeds U+10FFFF; D800 is inside the surrogate area — neither is a character.
    for bad in ["110000", "0D800"] {
        let src = format!("1 beginbfchar\n<0001> <{bad}>\nendbfchar\n");
        let err = ToUnicodeCMap::parse(&src).expect_err("must fail");
        assert!(
            matches!(err, CMapError::InvalidCodePoint(_)),
            "{bad}: got {err:?}"
        );
    }
}

#[test]
fn a_reversed_range_is_rejected() {
    let src = "1 beginbfrange\n<0020> <0010> <0061>\nendbfrange\n";
    let err = ToUnicodeCMap::parse(src).expect_err("must fail");
    assert!(
        matches!(err, CMapError::RangeInverted { lo: 0x20, hi: 0x10 }),
        "got {err:?}"
    );
}

#[test]
fn a_destination_array_with_too_few_elements_is_rejected() {
    // The range needs 3 elements but only 2 are given — the rest must not be silently skipped.
    let src = "1 beginbfrange\n<0020> <0022> [<0061> <0062>]\nendbfrange\n";
    let err = ToUnicodeCMap::parse(src).expect_err("must fail");
    assert!(
        matches!(
            err,
            CMapError::ArrayLengthMismatch {
                need: 3,
                got: 2,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn a_range_may_carry_into_the_high_byte() {
    // Taken from the real file (CIDX0 font, page 11): <0062> <00ff> <00a0>.
    // §9.10.3 describes the addition as "increment the last byte"; taken literally it would
    // wrongly reject this valid range — and it did, on the very first page of the real PDF.
    let src = "1 beginbfrange\n<0062> <00ff> <00a0>\nendbfrange\n";
    let m = ToUnicodeCMap::parse(src).expect("a valid range must not be rejected");
    assert_eq!(m.len(), 158);
    assert_eq!(m.decode(0x0062).text(), Some("\u{00A0}"));
    // 0xFF is exactly 157 steps from 0x62 -> U+00A0 + 157 = U+013D
    assert_eq!(m.decode(0x00FF).text(), Some("\u{013D}"));
}

#[test]
fn a_range_overflowing_the_destination_width_is_rejected_rather_than_wrapped() {
    // Destination FFFE with a range of 4 -> past four hex digits.
    // Wrapping would silently produce the wrong character, so it must stay a hard error.
    let src = "1 beginbfrange\n<0001> <0004> <FFFE>\nendbfrange\n";
    let err = ToUnicodeCMap::parse(src).expect_err("must fail");
    assert!(
        matches!(err, CMapError::RangeOverflowsDestination { .. }),
        "got {err:?}"
    );
}

#[test]
fn an_empty_cmap_is_not_an_error_but_returns_nothing() {
    let m = ToUnicodeCMap::parse("begincmap\nendcmap\n").expect("parse");
    assert!(m.is_empty());
    assert_eq!(m.decode(0x0001), DecodedGlyph::Unmapped { code: 0x0001 });
}

#[test]
fn a_dictionary_in_the_header_is_not_mistaken_for_a_hex_string() {
    // The real header has `<</Registry(Adobe)/Ordering(UCS)/Supplement 0>>` — if the lexer
    // treated `<<` as opening a hex string it would swallow the rest and load too little.
    let m = ToUnicodeCMap::parse(&fixture("cmap_nomnatong_slice.txt")).expect("parse");
    assert_eq!(m.len(), 10);
}
