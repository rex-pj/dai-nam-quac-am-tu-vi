// clippy only relaxes panic/expect inside #[test] functions.
#![allow(clippy::expect_used, clippy::panic)]

//! Classifying line roles.
//!
//! The two `y` constants here come from measuring all 1028 body pages: the running head and
//! the page number each sit at EXACTLY ONE value, which makes the filtering exact.

use dnqatv_parse_core::line::{
    LineRole, PAGE_NUMBER_Y, RUNNING_HEAD_Y, SubEntrySignal, classify, has_placeholder,
};

/// An arbitrary height in the body area, coinciding with neither head nor page number.
const BODY_Y: f64 = 657.79;

/// An ordinary line: no italic.
fn body(y: f64, text: &str) -> LineRole {
    classify(y, text, true, false)
}

/// A line with an italic part — the print uses italics for sub-entry forms.
fn italic(y: f64, text: &str) -> LineRole {
    classify(y, text, true, true)
}

// ── Running head and page number ─────────────────────────────────────────────

#[test]
fn the_running_head_is_recognised_by_height() {
    // p.500 — the running head names the first entry on the page.
    assert_eq!(body(RUNNING_HEAD_Y, " Lõm - "), LineRole::RunningHead);
    // p.300 — the right half of the running head, at the same height.
    assert_eq!(body(RUNNING_HEAD_Y, "押 Ét"), LineRole::RunningHead);
}

#[test]
fn the_page_number_is_recognised_by_height() {
    assert_eq!(body(PAGE_NUMBER_Y, "499"), LineRole::PageNumber);
    assert_eq!(body(PAGE_NUMBER_Y, "299"), LineRole::PageNumber);
}

#[test]
fn the_front_matter_has_no_running_head() {
    // Pages 1-10 are front matter; there that height is real content and must not be filtered.
    assert_ne!(
        classify(RUNNING_HEAD_Y, "TIỂU TỰ", false, false),
        LineRole::RunningHead
    );
}

#[test]
fn a_height_far_off_is_no_longer_the_running_head() {
    // The margin only absorbs JSON rounding; it is not a clustering tolerance.
    assert_ne!(body(RUNNING_HEAD_Y - 1.0, " Lõm - "), LineRole::RunningHead);
    assert_eq!(
        body(RUNNING_HEAD_Y - 0.001, " Lõm - "),
        LineRole::RunningHead
    );
}

// ── Titles ───────────────────────────────────────────────────────────────────

#[test]
fn the_book_title_and_the_letter_section_title() {
    assert_eq!(
        body(BODY_Y, "ĐẠI NAM QUẤC ÂM TỰ VỊ"),
        LineRole::SectionTitle
    );
    assert_eq!(body(BODY_Y, "CHỮ A"), LineRole::SectionTitle);
    assert_eq!(body(BODY_Y, "CHỮ Đ"), LineRole::SectionTitle);
    assert_eq!(body(BODY_Y, "CHỮ X"), LineRole::SectionTitle);
}

// ── Content ──────────────────────────────────────────────────────────────────

#[test]
fn headword_lines() {
    assert_eq!(body(BODY_Y, "阿  A  c."), LineRole::Headword);
    assert_eq!(body(BODY_Y, "𨰲  Lõm  n."), LineRole::Headword);
    assert_eq!(body(BODY_Y, "Bấm  n."), LineRole::Headword);
    assert_eq!(body(BODY_Y, "丫  A  (Nha.)  c."), LineRole::Headword);
}

#[test]
fn a_second_entry_under_the_same_glyph_opens_with_a_label() {
    // p.26 — after `北 Bấc c.` comes `n. Bấc ; bức tức.` for the same glyph.
    // 55 such lines measured.
    assert_eq!(
        body(BODY_Y, "n. Bấc ; bức tức."),
        LineRole::ContinuedHeadword
    );
    assert_eq!(
        body(BODY_Y, "c. Bài vở ; sắp ra, mở ra."),
        LineRole::ContinuedHeadword
    );
}

#[test]
fn a_sub_entry_is_recognised_by_its_placeholder() {
    // The two placeholders are defined by the DẤU RIÊNG page, and they DIFFER:
    // `―` stands for the reading (Quốc ngữ column), `|` for the glyph (Han column).
    assert_eq!(italic(BODY_Y, "― gươm. Nạm gươm."), LineRole::SubEntry);
    assert_eq!(
        italic(BODY_Y, "Chính giữa ―. Ở ngảy giữa ruột."),
        LineRole::SubEntry
    );
    assert_eq!(
        italic(BODY_Y, "| 意 ― ý. Dua theo một ý."),
        LineRole::SubEntry
    );
    assert_eq!(
        italic(BODY_Y, "太 | Thái ―. Gươm báu trong nước."),
        LineRole::SubEntry
    );
    // p.11 — em dash U+2014; 540 lines measured open with it. The first version hard-coded
    // only U+2015 and so missed this whole group.
    assert_eq!(
        italic(BODY_Y, "— thánh mẫu. Tiếng xưng tụng Đức thánh mẫu."),
        LineRole::SubEntry
    );
    assert_eq!(italic(BODY_Y, "--. id."), LineRole::SubEntry);
}

#[test]
fn a_continuation_is_the_remaining_text() {
    assert_eq!(
        body(BODY_Y, "Đèo, nương dựa, phụ theo."),
        LineRole::Continuation
    );
    assert_eq!(body(BODY_Y, "nói a dua."), LineRole::Continuation);
    assert_eq!(body(BODY_Y, "đều được đều mất."), LineRole::Continuation);
}

// ── Coverage ─────────────────────────────────────────────────────────────────

#[test]
fn only_four_roles_carry_dictionary_content() {
    let content: Vec<LineRole> = LineRole::ALL
        .into_iter()
        .filter(|r| r.is_content())
        .collect();
    assert_eq!(content.len(), 4);
    assert!(!LineRole::RunningHead.is_content());
    assert!(!LineRole::PageNumber.is_content());
    assert!(!LineRole::SectionTitle.is_content());
}

#[test]
fn every_line_gets_exactly_one_role() {
    // No branch returns "unknown" — that is the precondition for the conservation law
    // above it to mean anything.
    for t in ["", "   ", "x", "― a", "n. b", "阿  A  c.", "CHỮ A"] {
        let role = body(BODY_Y, t);
        assert!(LineRole::ALL.contains(&role), "{t:?} has no role");
    }
}

// ── The two sub-entry signals ────────────────────────────────────────────────

#[test]
fn all_four_measured_placeholder_characters_are_recognised() {
    // The DẤU RIÊNG page writes `—` but the typesetter used `―`. Both must be accepted.
    for ph in ["―", "—", "–", "|", "--"] {
        assert!(has_placeholder(&format!("x {ph} y")), "{ph:?}");
    }
}

#[test]
fn a_single_hyphen_is_not_a_placeholder() {
    // `-` occurs 4,039 times but is the hyphen inside compounds, not a placeholder.
    assert!(!has_placeholder("Di-đà"));
    assert!(!has_placeholder("Thiên-trúc"));
}

#[test]
fn an_italic_quotation_is_not_mistaken_for_a_sub_entry() {
    // p.13 — a Han phrase quoted inside a definition, italic but with NO placeholder.
    // 2,751 such lines measured; taking the italic signal alone would misread them all.
    assert_eq!(
        italic(
            BODY_Y,
            "trí huệ thông minh khước thọ bần. Ngây, điếc, câm, ngọng"
        ),
        LineRole::Continuation
    );
}

#[test]
fn a_folk_verse_with_a_placeholder_but_no_italic_is_not_mistaken_either() {
    // p.110 — a verse quoted in a definition, carrying `―` because it repeats the entry.
    // Taking the placeholder signal alone would misread it as a sub-entry.
    assert_eq!(
        body(
            BODY_Y,
            "Tay bưng dĩa muối ― gầng, gầng cay muối mặn, xin đừng"
        ),
        LineRole::Continuation
    );
}

#[test]
fn the_four_signal_combinations() {
    assert_eq!(SubEntrySignal::of(true, true), SubEntrySignal::Both);
    assert_eq!(SubEntrySignal::of(true, false), SubEntrySignal::ItalicOnly);
    assert_eq!(
        SubEntrySignal::of(false, true),
        SubEntrySignal::PlaceholderOnly
    );
    assert_eq!(SubEntrySignal::of(false, false), SubEntrySignal::Neither);

    // Only "placeholder without italic" needs a human (39 lines in the whole book).
    assert!(SubEntrySignal::PlaceholderOnly.is_ambiguous());
    assert!(!SubEntrySignal::Both.is_ambiguous());
    assert!(!SubEntrySignal::ItalicOnly.is_ambiguous());
    assert!(!SubEntrySignal::Neither.is_ambiguous());
}
