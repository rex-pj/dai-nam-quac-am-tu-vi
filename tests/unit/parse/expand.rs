// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
#![allow(clippy::expect_used, clippy::panic)]

//! The two derivation rules of the DẤU RIÊNG page.
//!
//! Every example is taken verbatim from `data/pages.jsonl`, generated from the PDF in docs/.

use dnqatv_core::text::DerivationRule;
use dnqatv_parse_core::expand::{HeadwordContext, expand_han_form, expand_reading_form};

/// The entry 𨰲 Lõm, printed page 499.
fn lom() -> HeadwordContext<'static> {
    HeadwordContext {
        glyph: Some("𨰲"),
        reading: "Lõm",
    }
}

/// The entry 阿 A, printed page 10 — the first entry of the whole book.
fn a() -> HeadwordContext<'static> {
    HeadwordContext {
        glyph: Some("阿"),
        reading: "A",
    }
}

// ── The dash -> reading rule ─────────────────────────────────────────────────

#[test]
fn substitutes_the_dash_with_the_reading() {
    // p.500 — `― gươm. Nạm gươm.`
    let e = expand_reading_form("― gươm", &lom());
    assert_eq!(e.as_str(), "Lõm gươm");
    assert_eq!(e.substitutions, 1);
    assert_eq!(e.value.rule(), DerivationRule::DashToReading);
    assert!(!e.needs_review());
}

#[test]
fn a_placeholder_at_the_end_of_the_form() {
    // p.500 — `Chính giữa ―. Ở ngảy giữa ruột.`
    let e = expand_reading_form("Chính giữa ―", &lom());
    assert_eq!(e.as_str(), "Chính giữa Lõm");
}

#[test]
fn the_capitalisation_of_the_reading_is_kept() {
    // Lowercasing it to look "more natural" is interpretation. Search goes through fold so
    // it is unnecessary, and keeping it makes tracing back to the print easier.
    let e = expand_reading_form("Con mắt thom ―", &lom());
    assert_eq!(e.as_str(), "Con mắt thom Lõm");
}

#[test]
fn every_occurrence_is_substituted_not_just_the_first() {
    // p.53 — `| 輕 Nhứt ― trọng, nhứt ― khinh`. 3.27% of forms have two placeholders,
    // and some have six. Substituting only the first would miss nearly 2,000 forms.
    let e = expand_reading_form("Nhứt ― trọng, nhứt ― khinh", &lom());
    assert_eq!(e.as_str(), "Nhứt Lõm trọng, nhứt Lõm khinh");
    assert_eq!(e.substitutions, 2);
}

#[test]
fn all_three_dash_characters_are_accepted() {
    // The DẤU RIÊNG page writes `—` (U+2014), the typesetter used `―` (U+2015), and `–`
    // (U+2013) opens 13 lines. Missing `—` loses 540 lines.
    for ph in ["―", "—", "–"] {
        let e = expand_reading_form(&format!("{ph} thánh"), &a());
        assert_eq!(e.as_str(), "A thánh", "character {ph:?}");
        assert_eq!(e.substitutions, 1);
    }
}

#[test]
fn the_two_dash_sequence_is_accepted_too() {
    // tr.11 — `--. id.`
    let e = expand_reading_form("--", &a());
    assert_eq!(e.as_str(), "A");
    assert_eq!(e.substitutions, 1);
}

#[test]
fn the_two_dash_form_is_checked_before_the_single_dash() {
    // Checking single characters first would turn `--` into two substitutions, changing the meaning.
    let e = expand_reading_form("--", &lom());
    assert_eq!(e.substitutions, 1, "must be ONE substitution");
    assert_eq!(e.as_str(), "Lõm");
}

#[test]
fn a_form_with_no_placeholder_is_left_alone() {
    // 130 forms measured contain no placeholder at all.
    let e = expand_reading_form("Anh vủ năng ngôn", &lom());
    assert_eq!(e.as_str(), "Anh vủ năng ngôn");
    assert_eq!(e.substitutions, 0);
}

#[test]
fn a_single_hyphen_is_not_treated_as_a_placeholder() {
    // `-` occurs 4,039 times but is the hyphen inside compounds.
    let e = expand_reading_form("Di-đà", &a());
    assert_eq!(e.as_str(), "Di-đà");
    assert_eq!(e.substitutions, 0);
}

// ── The | -> glyph rule ──────────────────────────────────────────────────────

#[test]
fn substitutes_the_pipe_with_the_glyph() {
    // p.11 — `| 意 ― ý.` The Han part `| 意` under entry 阿 becomes `阿 意`.
    let e = expand_han_form("| 意", &a()).expect("the entry has a glyph");
    assert_eq!(e.as_str(), "阿 意");
    assert_eq!(e.substitutions, 1);
    assert_eq!(e.value.rule(), DerivationRule::PipeToGlyph);
}

#[test]
fn the_pipe_may_follow_a_han_character() {
    // p.11 — `瘖 | Ám ―.`
    let e = expand_han_form("瘖 |", &a()).expect("the entry has a glyph");
    assert_eq!(e.as_str(), "瘖 阿");
}

#[test]
fn several_pipes_in_one_compound_are_substituted() {
    // p.315 — `先 | | 後 |`
    let e = expand_han_form("先 | | 後 |", &a()).expect("the entry has a glyph");
    assert_eq!(e.as_str(), "先 阿 阿 後 阿");
    assert_eq!(e.substitutions, 3);
}

#[test]
fn an_image_entry_cannot_have_its_han_part_derived() {
    // 29 entries have no Unicode glyph. Substituting a "close enough" character is exactly
    // what the 2026 editors deliberately refused on the LƯU Ý page.
    let no_glyph = HeadwordContext {
        glyph: None,
        reading: "Bấm",
    };
    assert!(expand_han_form("| 意", &no_glyph).is_none());

    // But the Quốc ngữ part still derives, because it only needs the reading.
    let e = expand_reading_form("― tay", &no_glyph);
    assert_eq!(e.as_str(), "Bấm tay");
}

// ── The two rules do not bleed into each other ───────────────────────────────

#[test]
fn the_reading_rule_does_not_touch_the_pipe() {
    // p.53 — `| 輕 Nhứt ― trọng`: the Han part is interleaved into the form.
    // Substituting `|` here with the glyph would be a guess, so it is left and reported.
    //
    // A form shaped like this rarely reaches here any more — `quoc_ngu_starts_at` now hands
    // the leading Han column back before expansion, which took 829 such forms down to 3.
    // The rule tested here is unchanged: this function never touches a `|`, whoever sends
    // one in. That is what keeps the two derivation rules of the DẤU RIÊNG page apart.
    let e = expand_reading_form("| 輕 Nhứt ― trọng", &lom());
    assert_eq!(e.as_str(), "| 輕 Nhứt Lõm trọng");
    assert_eq!(e.substitutions, 1);
    assert_eq!(e.residual, 1);
    assert!(
        e.needs_review(),
        "an unresolved placeholder remains — it must be reported"
    );
}

#[test]
fn the_glyph_rule_does_not_touch_the_dash() {
    let e = expand_han_form("| 意 ―", &a()).expect("the entry has a glyph");
    assert_eq!(e.as_str(), "阿 意 ―");
    assert_eq!(e.substitutions, 1);
    assert_eq!(e.residual, 1);
    assert!(e.needs_review());
}

#[test]
fn the_result_is_always_derived_never_the_words_of_the_book() {
    // The Verbatim/Derived boundary underpins the whole plan: the UI shows the verbatim
    // form by default and switches to the full form only when the user toggles it.
    let r = expand_reading_form("― gươm", &lom());
    let h = expand_han_form("| 意", &a()).expect("the entry has a glyph");
    assert_eq!(r.value.rule(), DerivationRule::DashToReading);
    assert_eq!(h.value.rule(), DerivationRule::PipeToGlyph);
    assert_ne!(r.value.rule(), h.value.rule());
}

#[test]
fn an_empty_form_does_not_crash() {
    let e = expand_reading_form("", &lom());
    assert_eq!(e.as_str(), "");
    assert_eq!(e.substitutions, 0);
}
