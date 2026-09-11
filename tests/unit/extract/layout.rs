// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
// This whole file is test code, where panicking on a broken fixture is correct.
#![allow(clippy::expect_used, clippy::panic)]

//! Reconstructing printed lines from coordinates.
//!
//! The background number: measured on the real file, each printed line corresponds to
//! **1.95–2.63 `Tm` commands** because font changes cut it into pieces. Regrouping them

use dnqatv_extract_core::layout::{Column, TextRun, group_into_lines};
use dnqatv_extract_core::style::TextStyle;

/// The real page width of the 2026 edition (MediaBox 0 0 595.30398 841.8898).
const PAGE_WIDTH: f64 = 595.304;

fn run(x: f64, y: f64, text: &str) -> TextRun {
    styled(x, y, text, TextStyle::Regular)
}

fn styled(x: f64, y: f64, text: &str, style: TextStyle) -> TextRun {
    TextRun {
        x,
        y,
        text: text.to_owned(),
        style,
    }
}

#[test]
fn the_column_split_follows_the_page_width_not_a_hand_typed_constant() {
    // Measured on page 500: the left column starts near 44, the right near 300. Half is 297.65.
    assert_eq!(Column::of(44.0, PAGE_WIDTH), Column::Left);
    assert_eq!(Column::of(100.0, PAGE_WIDTH), Column::Left);
    assert_eq!(Column::of(300.0, PAGE_WIDTH), Column::Right);
    assert_eq!(Column::of(540.4, PAGE_WIDTH), Column::Right);
}

#[test]
fn runs_in_the_same_column_at_the_same_y_join_into_one_line() {
    // The real shape of an entry: glyph, reading, label — three Tm commands, one printed line.
    let runs = vec![
        run(44.0, 700.0, "𨰲  "),
        run(70.0, 700.0, "Lõm"),
        run(95.0, 700.0, "  n."),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines.len(), 1, "must be ONE printed line, not three");
    assert_eq!(lines[0].text(), "𨰲  Lõm  n.");
    assert_eq!(lines[0].column, Column::Left);
}

#[test]
fn runs_within_a_line_are_joined_by_increasing_x() {
    // The content-stream order can be arbitrary; reading order is decided by coordinates.
    let runs = vec![
        run(95.0, 700.0, "  n."),
        run(44.0, 700.0, "𨰲  "),
        run(70.0, 700.0, "Lõm"),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines[0].text(), "𨰲  Lõm  n.");
}

#[test]
fn a_different_y_means_a_different_line() {
    let runs = vec![
        run(44.0, 700.0, "upper line"),
        run(44.0, 684.6, "lower line"),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text(), "upper line");
    assert_eq!(lines[1].text(), "lower line");
}

#[test]
fn the_whole_left_column_is_read_before_the_right() {
    // The easiest mistake in a two-column layout: reading across mixes two entries together.
    let runs = vec![
        run(300.0, 700.0, "right-1"),
        run(44.0, 700.0, "left-1"),
        run(300.0, 684.0, "right-2"),
        run(44.0, 684.0, "left-2"),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    let texts: Vec<String> = lines
        .iter()
        .map(dnqatv_extract_core::layout::Line::text)
        .collect();
    assert_eq!(texts, vec!["left-1", "left-2", "right-1", "right-2"]);
}

#[test]
fn the_same_y_in_different_columns_does_not_merge() {
    // Measured: the two columns occasionally share a y value (1–3 out of ~40 lines).
    // Keying on y alone would glue those lines together.
    let runs = vec![run(44.0, 700.0, "left"), run(300.0, 700.0, "right")];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(
        lines.len(),
        2,
        "same y, different columns must be two lines"
    );
    assert_eq!(lines[0].column, Column::Left);
    assert_eq!(lines[1].column, Column::Right);
}

#[test]
fn an_upper_line_sorts_before_a_lower_one() {
    // PDF coordinates originate at the bottom of the page, so a larger y is HIGHER.
    let runs = vec![
        run(44.0, 100.0, "page bottom"),
        run(44.0, 800.0, "page top"),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines[0].text(), "page top");
    assert_eq!(lines[1].text(), "page bottom");
}

#[test]
fn no_runs_means_no_lines() {
    assert!(group_into_lines(&[], PAGE_WIDTH).is_empty());
}

// ── Text style ───────────────────────────────────────────────────────────────

#[test]
fn a_style_change_opens_a_new_piece_within_one_line() {
    // The real shape of a printed sub-entry: the form ITALIC, the definition REGULAR.
    // The boundary between them is the font change, not a guessed full stop.
    let runs = vec![
        styled(44.0, 700.0, "― gươm", TextStyle::Italic),
        styled(80.0, 700.0, ". Nạm gươm.", TextStyle::Regular),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].segments.len(), 2);
    assert_eq!(lines[0].segments[0].style, TextStyle::Italic);
    assert_eq!(lines[0].segments[0].text, "― gươm");
    assert_eq!(lines[0].segments[1].style, TextStyle::Regular);
    assert_eq!(lines[0].text(), "― gươm. Nạm gươm.");
}

#[test]
fn adjacent_runs_of_the_same_style_merge() {
    let runs = vec![
        styled(44.0, 700.0, "Ruột, ", TextStyle::Regular),
        styled(70.0, 700.0, "trúc mứt.", TextStyle::Regular),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(
        lines[0].segments.len(),
        1,
        "no fragmentation when the style is the same"
    );
}

#[test]
fn the_leading_style_skips_whitespace_only_pieces() {
    // The print often inserts a regular space before an italic run.
    let runs = vec![
        styled(44.0, 700.0, "  ", TextStyle::Regular),
        styled(50.0, 700.0, "― gươm", TextStyle::Italic),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines[0].leading_style(), Some(TextStyle::Italic));
}

#[test]
fn a_headword_line_leads_with_the_nom_font() {
    let runs = vec![
        styled(44.0, 700.0, "𨰲  ", TextStyle::Han),
        styled(70.0, 700.0, "Lõm", TextStyle::Bold),
        styled(95.0, 700.0, "  n.", TextStyle::Regular),
    ];
    let lines = group_into_lines(&runs, PAGE_WIDTH);
    assert_eq!(lines[0].leading_style(), Some(TextStyle::Han));
    assert!(lines[0].has_style(TextStyle::Bold));
    assert!(!lines[0].has_style(TextStyle::Italic));
}
