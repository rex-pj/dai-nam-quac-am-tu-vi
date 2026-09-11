//! Reconstructing the **real printed lines** from coordinates, rather than cutting at
//! typesetting-command boundaries.
//!
//! Why it is needed: the book is set in two columns, and one printed line is split across
//! several `Tm` commands every time the font changes. Measured on the real file: **1.95–2.63
//! `Tm` commands per printed line**. The original exploratory prototype treated each `Tm` as
//! a line, so an entry like `𨰲 Lõm n.` was cut into three separate lines — and that cut is
//! exactly what broke line assembly and dropped 355,833 characters.
//!
//! The line key here is the pair **(column, y)**, needing no tolerance threshold: measured on
//! the sample pages, the two columns almost never share a y value (1–3 out of some 40 lines
//! per column). The column boundary comes from the page `MediaBox` itself, not a hand-typed constant.

use crate::style::TextStyle;

/// A column of the book two-column layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Column {
    Left,
    Right,
}

impl Column {
    pub const ALL: [Column; 2] = [Self::Left, Self::Right];

    /// The column holding an x coordinate, splitting at half the page width.
    pub fn of(x: f64, page_width: f64) -> Self {
        if x < page_width / 2.0 {
            Self::Left
        } else {
            Self::Right
        }
    }
}

/// One decoded text run, with its position and style.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub style: TextStyle,
}

/// A piece of same-styled text inside one printed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub style: TextStyle,
    pub text: String,
}

/// One printed line, assembled from every run at the same column and height.
///
/// The style-separated pieces are kept rather than concatenated: the boundary between a
/// sub-entry form and its definition is exactly where the style changes, so merging early
/// throws information away.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub column: Column,
    pub y: f64,
    pub segments: Vec<Segment>,
}

impl Line {
    /// The full text of the line.
    pub fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }

    /// The style of the first piece with visible content.
    ///
    /// Whitespace-only pieces are skipped, because the print often inserts a regular space
    /// before an italic run and it says nothing about the role of the line.
    pub fn leading_style(&self) -> Option<TextStyle> {
        self.segments
            .iter()
            .find(|s| !s.text.trim().is_empty())
            .map(|s| s.style)
    }

    pub fn has_style(&self, style: TextStyle) -> bool {
        self.segments
            .iter()
            .any(|s| s.style == style && !s.text.trim().is_empty())
    }
}

/// Group runs into printed lines, in reading order: left column top to bottom, then right.
///
/// Within a line, runs are joined by increasing x and **no character is inserted** —
/// whatever whitespace the print has is already in the source string.
pub fn group_into_lines(runs: &[TextRun], page_width: f64) -> Vec<Line> {
    let mut keyed: Vec<(Column, f64, &TextRun)> = runs
        .iter()
        .map(|r| (Column::of(r.x, page_width), r.y, r))
        .collect();

    // `total_cmp` is a TOTAL order on f64, so no fallback value is needed.
    // `partial_cmp(..).unwrap_or(Equal)` would silently treat two NaN coordinates as equal,
    // i.e. merge two printed lines — exactly the kind of silent bug the plan forbids.
    keyed.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.total_cmp(&a.1))
            .then_with(|| a.2.x.total_cmp(&b.2.x))
    });

    let mut lines: Vec<Line> = Vec::new();
    for (column, y, run) in keyed {
        match lines.last_mut() {
            Some(last) if last.column == column && last.y.to_bits() == y.to_bits() => {
                match last.segments.last_mut() {
                    // Adjacent runs of the same style merge; a style change opens a new piece.
                    Some(seg) if seg.style == run.style => seg.text.push_str(&run.text),
                    _ => last.segments.push(Segment {
                        style: run.style,
                        text: run.text.clone(),
                    }),
                }
            }
            _ => lines.push(Line {
                column,
                y,
                segments: vec![Segment {
                    style: run.style,
                    text: run.text.clone(),
                }],
            }),
        }
    }
    lines
}
