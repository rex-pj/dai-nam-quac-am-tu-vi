// clippy only relaxes panic/expect inside #[test] functions; the helper here sits outside.
#![allow(clippy::expect_used, clippy::panic)]

//! Splitting headword lines.
//!
//! Every example line is taken VERBATIM from `data/pages.jsonl`, generated from the PDF in
//! docs/, with the PDF page number so it can be checked against the original page image.

use dnqatv_core::model::Pos;
use dnqatv_parse_core::headword::parse_headword;
use dnqatv_parse_core::span::check_coverage;

fn parsed(text: &str) -> dnqatv_parse_core::HeadwordLine {
    parse_headword(text).unwrap_or_else(|| panic!("could not split: {text:?}"))
}

// ── The common shapes ────────────────────────────────────────────────────────

#[test]
fn a_simple_headword() {
    // p.11 — the first entry of the whole book.
    let t = "阿  A  c.";
    let h = parsed(t);
    assert_eq!(h.glyph.map(|s| s.slice(t)), Some("阿"));
    assert_eq!(h.reading.slice(t), "A");
    assert_eq!(h.alternate, None);
    assert_eq!(h.pos_list(), vec![Pos::ChuNho]);
    assert!(h.has_glyph());
}

#[test]
fn a_headword_with_an_ext_b_glyph() {
    // p.500 — 𨰲 is U+28C32, outside the BMP, so it takes 4 bytes.
    let t = "𨰲  Lõm  n.";
    let h = parsed(t);
    assert_eq!(h.glyph.map(|s| s.slice(t)), Some("𨰲"));
    assert_eq!(h.reading.slice(t), "Lõm");
    assert_eq!(h.pos_list(), vec![Pos::ChuNom]);
}

#[test]
fn a_headword_with_no_glyph_because_the_print_uses_an_image() {
    // p.31 — Bấm is in the list of 22 image glyphs on the LƯU Ý page.
    // The text layer has no character for the glyph, so it must be None, never guessed.
    let t = "Bấm  n.";
    let h = parsed(t);
    assert_eq!(h.glyph, None);
    assert!(!h.has_glyph());
    assert_eq!(h.reading.slice(t), "Bấm");
    assert_eq!(h.pos_list(), vec![Pos::ChuNom]);
}

// ── Parenthesised readings ───────────────────────────────────────────────────

#[test]
fn a_headword_with_a_sino_vietnamese_reading_in_parentheses() {
    // p.11 — 39 headword lines measured carry a parenthesised part.
    let t = "丫  A  (Nha.)  c.";
    let h = parsed(t);
    assert_eq!(h.glyph.map(|s| s.slice(t)), Some("丫"));
    assert_eq!(h.reading.slice(t), "A");
    assert_eq!(h.alternate.map(|s| s.slice(t)), Some("(Nha.)"));
    assert_eq!(h.pos_list(), vec![Pos::ChuNho]);
}

#[test]
fn a_reading_variant_after_a_comma_is_kept_intact() {
    // p.55 — this very line once produced a false mismatch against the entry index,
    // because the spreadsheet splits "Biêu, (tiêu)" on the comma into two readings.
    let t = "標  Biêu, (tiêu)  n.";
    let h = parsed(t);
    assert_eq!(h.reading.slice(t), "Biêu,");
    assert_eq!(h.alternate.map(|s| s.slice(t)), Some("(tiêu)"));
}

// ── Labels ───────────────────────────────────────────────────────────────────

#[test]
fn all_three_label_kinds_are_recognised() {
    assert_eq!(parsed("阿  A  c.").pos_list(), vec![Pos::ChuNho]);
    assert_eq!(parsed("阿  A  n.").pos_list(), vec![Pos::ChuNom]);
    assert_eq!(parsed("盆  Bồn  cn.").pos_list(), vec![Pos::ChuNhoDungNom]);
}

#[test]
fn cn_is_not_misread_as_n() {
    // Trying `n.` before `cn.` would lose the "a Chinese character also used as Nôm" distinction.
    let t = "盆  Bồn  cn.";
    let h = parsed(t);
    assert_eq!(h.pos_list(), vec![Pos::ChuNhoDungNom]);
    assert_eq!(h.labels[0].0.slice(t), "cn.");
}

#[test]
fn a_word_ending_in_a_dot_is_not_mistaken_for_a_label() {
    // Without whitespace before the label it is not a headword.
    assert!(parse_headword("Ruột, trúc mứt, cái cốt.").is_none());
    assert!(parse_headword("nói a dua.").is_none());
}

#[test]
fn a_non_headword_line_returns_none_rather_than_a_guess() {
    for t in [
        "Đèo, nương dựa, phụ theo.",
        "― gươm. Nạm gươm.",
        "CHỮ A",
        "",
        "   ",
    ] {
        assert!(parse_headword(t).is_none(), "{t:?} is not a headword");
    }
}

// ── The conservation law ─────────────────────────────────────────────────────

#[test]
fn the_fields_cover_the_line_with_no_gap_and_no_overlap() {
    // Gate 2: the split must be an exact partition of the source line.
    for t in [
        "阿  A  c.",
        "𨰲  Lõm  n.",
        "Bấm  n.",
        "丫  A  (Nha.)  c.",
        "標  Biêu, (tiêu)  n.",
        "盆  Bồn  cn.",
    ] {
        let h = parsed(t);
        let cov = check_coverage(t, &h.spans());
        assert!(
            cov.is_exact_partition(),
            "line {t:?} is not fully covered: {cov:?}"
        );
    }
}

#[test]
fn spans_point_back_at_the_right_stretch_of_the_source_line() {
    // Provenance: every field must be re-sliceable from the original string.
    let t = "丫  A  (Nha.)  c.";
    let h = parsed(t);
    for s in h.spans() {
        assert!(!s.slice(t).trim().is_empty(), "empty span in {t:?}");
    }
}

// ── Two labels ───────────────────────────────────────────────────────────────

#[test]
fn an_entry_with_two_labels_keeps_both_in_printed_order() {
    // p.14 — 543 lines measured as `c. n.` and 3 as `n. c.`, out of 7,622.
    // Collapsing them into `cn.` would be interpretation, not recording.
    let t = "蔭  Ấm  c. n.";
    let h = parsed(t);
    assert_eq!(
        h.reading.slice(t),
        "Ấm",
        "the reading must not swallow the first label"
    );
    assert_eq!(h.pos_list(), vec![Pos::ChuNho, Pos::ChuNom]);
    assert_eq!(h.labels.len(), 2);

    // p.421 — the reverse order must be preserved too.
    let t = "欺  Khi  n. c.";
    assert_eq!(parsed(t).pos_list(), vec![Pos::ChuNom, Pos::ChuNho]);
}

#[test]
fn two_labels_still_cover_the_line() {
    let t = "蔭  Ấm  c. n.";
    let h = parsed(t);
    let cov = check_coverage(t, &h.spans());
    assert!(cov.is_exact_partition(), "not fully covered: {cov:?}");
}

#[test]
fn a_label_missing_its_dot_is_not_guessed_into_a_label() {
    // p.295 and p.353 — the print omits the dot on the first label. Accepting a bare `c`
    // would open the door to misreading lowercase letters in a reading, so it surfaces at the gate.
    let t = "興  Hấng (Hứng) n  c.";
    let h = parsed(t);
    assert_eq!(
        h.pos_list(),
        vec![Pos::ChuNho],
        "only dotted labels are accepted"
    );
    let cov = check_coverage(t, &h.spans());
    assert!(
        !cov.is_exact_partition(),
        "the gate must catch this anomalous line"
    );
}

#[test]
fn a_label_swallowed_by_the_reading_is_still_reported() {
    // p.295 — the print writes `c` without a dot, so it lands in the reading and is STILL covered.
    // The conservation law is blind to this: it catches lost characters, not misfiled ones.
    let t = "當  Đương. c  n.";
    let h = parsed(t);
    assert!(
        check_coverage(t, &h.spans()).is_exact_partition(),
        "this line is fully covered, so gate 2 sees nothing"
    );
    assert!(
        h.reading_has_stray_label(t),
        "but the stray-label check must report it"
    );

    // A normal entry must not raise a false alarm.
    for good in [
        "阿  A  c.",
        "𨰲  Lõm  n.",
        "蔭  Ấm  c. n.",
        "標  Biêu, (tiêu)  n.",
    ] {
        let h = parsed(good);
        assert!(!h.reading_has_stray_label(good), "false alarm on {good:?}");
    }
}

// ── Continued headwords (a label opening the line) ───────────────────────────

#[test]
fn a_continued_headword_splits_into_labels_and_definition() {
    // p.26 — after `北 Bấc c.` comes this line, same glyph 北 read the Nôm way.
    // 55 such lines measured.
    let t = "n. Bấc ; bức tức.";
    let c = dnqatv_parse_core::parse_continued_headword(t).expect("splits");
    assert_eq!(c.pos_list(), vec![Pos::ChuNom]);
    assert_eq!(c.labels[0].0.slice(t), "n.");
    assert_eq!(c.definition.map(|d| d.slice(t)), Some("Bấc ; bức tức."));
}

#[test]
fn a_continued_headword_covers_the_line() {
    for t in [
        "n. Bấc ; bức tức.",
        "c. Bài vở ; sắp ra, mở ra.",
        "n. Nôm là bán chác, đổi vật mà lấy tiền.",
    ] {
        let c = dnqatv_parse_core::parse_continued_headword(t).expect("splits");
        let cov = check_coverage(t, &c.spans());
        assert!(
            cov.is_exact_partition(),
            "line {t:?} is not fully covered: {cov:?}"
        );
    }
}

#[test]
fn a_line_not_opening_with_a_label_returns_none() {
    for t in [
        "Đèo, nương dựa, phụ theo.",
        "阿  A  c.",
        "nói a dua.",
        "cá. Thứ cá dẹp mà dài",
        "",
    ] {
        assert!(
            dnqatv_parse_core::parse_continued_headword(t).is_none(),
            "{t:?} is not a continued headword"
        );
    }
}

#[test]
fn the_asterisk_of_an_image_glyph_is_its_own_field() {
    // p.198 — the print uses `*` to mark a glyph presented as an image (the LƯU Ý convention).
    // Letting it into the reading would make the reading "**  Dạng" and the ordering
    // invariant would not see the initial — exactly the bug gate 5 caught.
    // The whitespace here is U+00A0 (non-breaking), exactly as printed.
    let t = "**\u{a0}\u{a0}Dạng  c.";
    let h = parsed(t);
    assert_eq!(
        h.glyph, None,
        "the glyph is an image, so there is no Unicode character"
    );
    assert_eq!(h.image_marker.map(|s| s.slice(t)), Some("**"));
    assert_eq!(h.reading.slice(t), "Dạng");
    assert_eq!(h.pos_list(), vec![Pos::ChuNho]);

    let cov = check_coverage(t, &h.spans());
    assert!(cov.is_exact_partition(), "not fully covered: {cov:?}");
}

#[test]
fn a_normal_headword_has_no_asterisk() {
    for t in ["阿  A  c.", "𨰲  Lõm  n.", "Bấm  n."] {
        assert_eq!(parsed(t).image_marker, None, "{t:?}");
    }
}

#[test]
fn a_label_not_at_the_end_of_the_line_still_yields_a_headword() {
    // p.525 — the print starts the definition on the headword line; the `N` begins "Ngựa"
    // and the rest is on the next line. Exactly 6 such lines measured in the whole book.
    // Requiring the label at the end would lose those 6 real entries.
    let t = "馬  Mã  c. N";
    let h = parsed(t);
    assert_eq!(h.glyph.map(|s| s.slice(t)), Some("馬"));
    assert_eq!(h.reading.slice(t), "Mã");
    assert_eq!(h.pos_list(), vec![Pos::ChuNho]);
    assert_eq!(h.trailing_text.map(|s| s.slice(t)), Some("N"));
    assert!(h.needs_review(), "it must be flagged for review");

    let cov = check_coverage(t, &h.spans());
    assert!(cov.is_exact_partition(), "not fully covered: {cov:?}");
}

#[test]
fn a_sub_entry_is_not_mistaken_for_a_headword_with_an_inline_definition() {
    // The strict condition: the line must START with a Han-Nom glyph. A sub-entry opens with
    // an italic form, so it cannot enter this branch even if its definition looks like a label.
    assert!(parse_headword("― gươm. Nạm gươm.").is_none());
    assert!(parse_headword("Chính giữa ―. Ở ngảy giữa ruột.").is_none());
    assert!(parse_headword("Đèo, nương dựa, phụ theo.").is_none());
}

#[test]
fn a_normal_headword_has_no_trailing_remainder() {
    for t in ["阿  A  c.", "𨰲  Lõm  n.", "蔭  Ấm  c. n."] {
        let h = parsed(t);
        assert_eq!(h.trailing_text, None, "{t:?}");
        assert!(!h.needs_review(), "{t:?}");
    }
}

// ── The capital the 2026 rebuild filed as a label ────────────────────────────

#[test]
fn the_last_label_gives_back_the_capital_that_opens_the_definition() {
    // pdf_page 21, verbatim. The 1895 print sets ONE label and the gloss "Ngăn, giữ, đè,
    // nhận xuống." — read off the scan, volume 1, image 29. The 2026 text layer turned that
    // capital N into a second label and left "găn" on the following line.
    let t = "壓  Áp  c. n.";
    let h = parsed(t);
    assert_eq!(h.pos_list(), vec![Pos::ChuNho, Pos::ChuNom]);

    let fix = h
        .gloss_initial_label(t, "găn, giữ, đè, nhận xuống.")
        .expect("the trailing label is the N of Ngăn");
    assert_eq!(fix.letter, 'N');
    assert_eq!(fix.dropped, Pos::ChuNom);
}

#[test]
fn a_definition_already_opening_with_a_capital_is_left_alone() {
    // 蔭 Ấm really does carry two labels, and its gloss is a sentence like any other.
    let t = "蔭  Ấm  c. n.";
    assert_eq!(
        parsed(t).gloss_initial_label(t, "Đồ đúc bằng đồng thau."),
        None
    );
}

#[test]
fn a_single_label_is_never_taken_apart() {
    // Removing the only label would leave the entry with no part of speech at all.
    let t = "阿  A  c.";
    assert_eq!(parsed(t).gloss_initial_label(t, "đèo, nương dựa."), None);
}

#[test]
fn a_letter_that_cannot_open_the_word_is_refused() {
    // `đoàn c. n. tụ; bầy, lũ.` — here the print really does set a second label, and the
    // sense after it opens in lowercase. `N` + `tụ` is not a Vietnamese syllable, so the
    // restoration must not fire. Measured: 7 of 211 candidates are of this shape.
    let t = "團  Đoàn  c. n.";
    assert_eq!(parsed(t).gloss_initial_label(t, "tụ; bầy, lũ."), None);
    assert_eq!(
        parsed(t).gloss_initial_label(t, "loại sắt cứng mà giòn."),
        None
    );
}

#[test]
fn the_onset_rule_knows_vietnamese_spelling() {
    use dnqatv_parse_core::begins_syllable;
    // The onsets that actually exist: ng, ngh, nh, ch, kh, ph, th, tr, gh, gi, qu.
    for (a, b) in [
        ('n', 'g'),
        ('n', 'h'),
        ('c', 'h'),
        ('t', 'r'),
        ('g', 'i'),
        ('q', 'u'),
    ] {
        assert!(begins_syllable(a, b), "{a}{b} is a real onset");
    }
    // A vowel always closes the question, tone mark and vowel mark included.
    for c in ['a', 'ó', 'ố', 'ư', 'ề', 'ắ', 'ị'] {
        assert!(begins_syllable('n', c), "n{c}");
    }
    // And the combinations Vietnamese does not have.
    for (a, b) in [
        ('n', 't'),
        ('n', 'l'),
        ('n', 'b'),
        ('n', 'd'),
        ('n', 'k'),
        ('c', 'g'),
    ] {
        assert!(!begins_syllable(a, b), "{a}{b} is not an onset");
    }
}
