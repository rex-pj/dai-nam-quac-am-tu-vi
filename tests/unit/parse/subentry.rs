// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
#![allow(clippy::expect_used, clippy::panic)]

//! Splitting a sub-entry line on style boundaries.
//!
//! Every example is taken verbatim from `data/pages.jsonl`, generated from the PDF in docs/,
//! with the page number so it can be checked against the original page image.

use dnqatv_core::model::TextStyle;
use dnqatv_parse_core::span::check_coverage;
use dnqatv_parse_core::subentry::{
    StyledSegment, SubEntryParts, line_text, merge_wrapped_forms, parse_sub_entry,
};

fn seg(style: TextStyle, text: &str) -> StyledSegment<'_> {
    StyledSegment { style, text }
}

fn italic(text: &str) -> StyledSegment<'_> {
    seg(TextStyle::Italic, text)
}
fn regular(text: &str) -> StyledSegment<'_> {
    seg(TextStyle::Regular, text)
}
fn han(text: &str) -> StyledSegment<'_> {
    seg(TextStyle::Han, text)
}

// ── The commonest shape ──────────────────────────────────────────────────────

#[test]
fn the_ir_shape_covers_94_percent() {
    // p.500 — `― gươm` italic, `. Nạm gươm.` regular.
    // The definition has dots too, so splitting on dots is guessing; splitting on font is reading.
    let segs = [italic("― gươm"), regular(". Nạm gươm.")];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");

    assert_eq!(s.han_form, None);
    assert_eq!(s.reading_form.slice(&t), "― gươm");
    assert_eq!(s.definition.map(|d| d.slice(&t)), Some(". Nạm gươm."));
    assert_eq!(s.italic_segments, 1);
    assert!(!s.needs_review());
}

#[test]
fn many_dots_in_the_definition_do_not_move_the_boundary() {
    // p.500 — `― chôm. Bộ không vững vàng, không tê tỉnh.Giò giám, không`
    // Splitting on the first dot happens to work here but fails elsewhere; font always works.
    let segs = [
        italic("― chôm"),
        regular(". Bộ không vững vàng, không tê tỉnh.Giò giám, không "),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.reading_form.slice(&t), "― chôm");
    assert!(
        s.definition
            .map(|d| d.slice(&t))
            .is_some_and(|d| d.contains("Giò giám"))
    );
}

#[test]
fn the_form_may_end_with_the_placeholder() {
    // p.500 — `Chính giữa ―. Ở ngảy giữa ruột, giữa cái cốt.`
    let segs = [
        italic("Chính giữa ―"),
        regular(". Ở ngảy giữa ruột, giữa cái cốt."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.reading_form.slice(&t), "Chính giữa ―");
}

// ── With a Han part ──────────────────────────────────────────────────────────

#[test]
fn the_hir_shape_has_a_han_compound_in_front() {
    // p.24 — `娑婆世界 Ta ― thế giái. Ngao du khắp chỗ.`
    let segs = [
        han("娑婆世界"),
        italic(" Ta ― thế giái"),
        regular(". Ngao du khắp chỗ."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("娑婆世界"));
    assert_eq!(s.reading_form.slice(&t), "Ta ― thế giái");
    assert_eq!(
        s.definition.map(|d| d.slice(&t)),
        Some(". Ngao du khắp chỗ.")
    );
}

#[test]
fn the_rhir_shape_opens_with_the_han_column_placeholder() {
    // p.11 — `| 意 ― ý. Dua theo một ý.`
    // `|` stands for the entry glyph, so the full Han part is 阿意.
    let segs = [
        regular("| "),
        han("意"),
        italic(" ― ý"),
        regular(". Dua theo một ý."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("| 意"));
    assert_eq!(s.reading_form.slice(&t), "― ý");
}

#[test]
fn the_hrir_shape_has_the_placeholder_after_a_han_character() {
    // p.11 — `瘖 | Ám ―. Câm, ngọng.`
    let segs = [
        han("瘖"),
        regular(" | "),
        italic("Ám ―"),
        regular(". Câm, ngọng."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("瘖 |"));
    assert_eq!(s.reading_form.slice(&t), "Ám ―");
}

// ── The placeholder is set in italic but belongs to the Han column ───────────

#[test]
fn an_italic_placeholder_piece_does_not_start_the_form() {
    // p.313 — `女 | 男 婚 Nữ ― nam hôn. Gái thì gả trai thì cưới…`
    // The ` | ` piece is set in italic, but a `|` only ever stands for the Han character,
    // so the form starts after it and the Han it separates stays in the Han column.
    let segs = [
        han("女"),
        italic(" | "),
        han("男 婚"),
        italic(" Nữ ― nam hôn"),
        regular(". Gái thì gả trai thì cưới."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("女 | 男 婚"));
    assert_eq!(s.reading_form.slice(&t), "Nữ ― nam hôn");
    assert_eq!(s.italic_segments, 1);
    assert!(
        !s.needs_review(),
        "the line is split on a boundary the print itself drew — nothing is in doubt"
    );
}

#[test]
fn a_placeholder_glued_to_the_front_of_the_form_is_split_off() {
    // p.22 — `| 吾 以 及 人 之 | — ngô — dĩ cập nhơn chi —. Nghĩa là mến…`
    // Here the closing `|` of the Han column opens the SAME italic piece that carries the
    // form, so the boundary falls inside one piece and cannot be found by style alone.
    let segs = [
        regular("| "),
        han("吾"),
        regular(" "),
        han("以"),
        italic(" "),
        han("及"),
        italic(" "),
        han("人"),
        italic(" "),
        han("之"),
        italic(" | — ngô — dĩ cập nhơn chi —"),
        regular(". Nghĩa là mến "),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("| 吾 以 及 人 之 |"));
    assert_eq!(s.reading_form.slice(&t), "— ngô — dĩ cập nhơn chi —");
    assert!(!s.needs_review());
}

// ── A form genuinely interrupted in the middle ───────────────────────────────

#[test]
fn a_form_interrupted_by_a_han_font_is_taken_whole_and_flagged() {
    // p.470 — `(癆) ― tổn. Mắc chứng phế hủy phế ung…`
    // No `|` anywhere, so nothing moves: the form runs from the FIRST italic piece to the
    // LAST and is not cut in half. 7 of 57,892 lines are like this.
    let segs = [
        italic("("),
        han("癆"),
        italic(") ― tổn"),
        regular(". Mắc chứng phế hủy phế ung, phải ho hen, một "),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form, None);
    assert_eq!(s.reading_form.slice(&t), "(癆) ― tổn");
    assert_eq!(s.italic_segments, 2);
    assert!(
        s.needs_review(),
        "no rule of the book says which column the Han belongs to — a person must look"
    );
}

#[test]
fn the_form_is_never_emptied_to_tidy_the_boundary() {
    // A line whose italic run is nothing but Han-column material. Moving all of it would
    // leave no form at all, so the split is left alone rather than dropping the line.
    let segs = [han("女"), italic(" | 男 ")];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.reading_form.slice(&t), "| 男");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("女"));
}

// ── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn a_line_with_a_form_but_no_definition() {
    // p.43 — the definition starts on the next line. 57 lines measured with only an italic piece.
    let segs = [italic("Lật ―")];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.reading_form.slice(&t), "Lật ―");
    assert_eq!(s.definition, None);
    assert_eq!(s.han_form, None);
}

#[test]
fn no_italic_means_it_is_not_a_sub_entry() {
    // With no form, one must not be invented from a dot.
    assert!(parse_sub_entry(&[regular("Đèo, nương dựa, phụ theo.")]).is_none());
    assert!(parse_sub_entry(&[han("阿"), regular(" A c.")]).is_none());
    assert!(parse_sub_entry(&[]).is_none());
}

#[test]
fn a_whitespace_only_italic_piece_does_not_count_as_a_form() {
    assert!(parse_sub_entry(&[regular("abc"), italic("   ")]).is_none());
}

// ── The conservation law ─────────────────────────────────────────────────────

#[test]
fn the_three_parts_cover_the_line_with_no_gap_and_no_overlap() {
    let cases: Vec<Vec<StyledSegment<'_>>> = vec![
        vec![italic("― gươm"), regular(". Nạm gươm.")],
        vec![
            han("娑婆世界"),
            italic(" Ta ― thế giái"),
            regular(". Ngao du khắp chỗ."),
        ],
        vec![
            regular("| "),
            han("意"),
            italic(" ― ý"),
            regular(". Dua theo một ý."),
        ],
        vec![italic("Lật ―")],
        vec![
            han("女"),
            italic(" | "),
            han("男 婚"),
            italic(" Nữ ― nam hôn"),
            regular(". Gái thì gả."),
        ],
    ];
    for segs in cases {
        let t = line_text(&segs);
        let s = parse_sub_entry(&segs).expect("splits");
        let cov = check_coverage(&t, &s.spans());
        assert!(
            cov.is_exact_partition(),
            "line {t:?} is not fully covered: {cov:?}"
        );
    }
}

// ── Forms the print wrapped onto a second line ───────────────────────────────

fn part(han: Option<&str>, form: &str, def: &str) -> SubEntryParts {
    SubEntryParts {
        han_form: han.map(str::to_owned),
        han_expanded: None,
        form: form.to_owned(),
        form_expanded: form.to_owned(),
        definition: def.to_owned(),
        needs_review: false,
    }
}

#[test]
fn a_form_split_across_two_lines_is_re_joined() {
    // p.688 — the proverb 君子以財發身… does not fit one column, so the print sets its
    // reading across two lines. Both lines are italic, so the classifier sees two
    // sub-entries: the first holds the whole Han column and says nothing at all.
    let subs = vec![
        part(
            Some("君 子 以 財 | 身 小 人 以 身 | 財"),
            "Quân tử dỉ tài― thân, tiểu",
            "",
        ),
        part(None, "nhơn dĩ thân― tài", ". Người khôn vì mình."),
    ];
    let (out, merged) = merge_wrapped_forms(subs);
    assert_eq!(merged, 1);
    assert_eq!(out.len(), 1, "one printed sub-entry, one record");
    assert_eq!(out[0].form, "Quân tử dỉ tài― thân, tiểu nhơn dĩ thân― tài");
    assert_eq!(
        out[0].han_form.as_deref(),
        Some("君 子 以 財 | 身 小 人 以 身 | 財")
    );
    assert_eq!(out[0].definition, ". Người khôn vì mình.");
}

#[test]
fn an_ordinary_pair_of_sub_entries_is_left_alone() {
    let subs = vec![
        part(None, "― gươm", ". Nạm gươm."),
        part(None, "― chuôi", ". id."),
    ];
    let (out, merged) = merge_wrapped_forms(subs);
    assert_eq!(merged, 0);
    assert_eq!(out.len(), 2);
}

#[test]
fn a_trailing_empty_definition_is_kept_not_dropped() {
    // Nothing follows it, so there is nothing to join it to. Losing the text would be worse
    // than leaving a sub-entry that says nothing.
    let subs = vec![
        part(None, "― gươm", ". Nạm gươm."),
        part(None, "Trơ trọi ―", ""),
    ];
    let (out, merged) = merge_wrapped_forms(subs);
    assert_eq!(merged, 0);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].form, "Trơ trọi ―");
}

#[test]
fn a_review_flag_survives_the_join() {
    let mut head = part(Some("甲"), "Nửa đầu", "");
    head.needs_review = true;
    let (out, merged) = merge_wrapped_forms(vec![head, part(None, "nửa sau", ". Nghĩa.")]);
    assert_eq!(merged, 1);
    assert!(
        out[0].needs_review,
        "a doubt about either half is a doubt about the whole"
    );
}

#[test]
fn a_comma_between_two_han_phrases_does_not_end_the_han_column() {
    // p.599 — `| 人 莫 用, 用 人 莫 |  ― nhơn mạc dụng, dụng nhơn mạc ―.`
    // Two parallel Han phrases share one line, separated by a comma. Stopping the walk at the
    // comma left the second phrase stranded in the Quốc ngữ form.
    let segs = [
        han("人 莫 用"),
        italic(", "),
        han("用 人 莫"),
        italic(" | ― nhơn mạc dụng, dụng nhơn mạc ―"),
        regular(". Nghi người thì đừng dùng."),
    ];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(
        s.han_form.map(|h| h.slice(&t)),
        Some("人 莫 用, 用 人 莫 |")
    );
    assert_eq!(s.reading_form.slice(&t), "― nhơn mạc dụng, dụng nhơn mạc ―");
    assert!(!s.needs_review());
}

#[test]
fn a_comma_cannot_drag_a_quoc_ngu_form_into_the_han_column() {
    // The comma is only swept up while the walk is still inside the Han column. A form that
    // begins with a letter stops it on the first character, comma or no comma.
    let segs = [han("甲"), italic(" | Tối ―, gia ―"), regular(". Nghĩa.")];
    let t = line_text(&segs);
    let s = parse_sub_entry(&segs).expect("splits");
    assert_eq!(s.han_form.map(|h| h.slice(&t)), Some("甲 |"));
    assert_eq!(s.reading_form.slice(&t), "Tối ―, gia ―");
}
