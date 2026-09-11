//! The character conservation law — gate ② of the plan, in machine-checkable form.
//!
//! The lesson behind this module: the exploratory prototype once reported "92% of entries
//! have a definition", a number that sounds fine. Replacing it with a conservation count
//! revealed **355,833 characters** vanishing silently. Coverage says nothing about the rest; a conservation law says everything.
//!
//! The constraint here is stronger than "no character is lost": the spans must form a
//! **partition** — every visible character is covered **exactly once**. Double coverage is
//! also an error, because it means a passage was counted into two different fields.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpanError {
    #[error("empty or reversed span: {start}..{end}")]
    Inverted { start: usize, end: usize },

    #[error("span {start}..{end} runs past the string length ({len} bytes)")]
    OutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },

    #[error("span {start}..{end} does not fall on character boundaries")]
    NotCharBoundary { start: usize, end: usize },
}

/// A byte range inside a source line. Used to trace every field back to its original text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn new(text: &str, start: usize, end: usize) -> Result<Self, SpanError> {
        if end <= start {
            return Err(SpanError::Inverted { start, end });
        }
        if end > text.len() {
            return Err(SpanError::OutOfBounds {
                start,
                end,
                len: text.len(),
            });
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return Err(SpanError::NotCharBoundary { start, end });
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }

    pub fn slice<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }
}

/// The visible character count — whitespace excluded.
///
/// Whitespace is not counted because the typesetting of the print produces spaces in places
/// that carry no information; counting them would add noise without adding certainty.
pub fn visible_len(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

/// The coverage check result. It passes when both lists are empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Coverage {
    /// Stretches with visible characters that no span covers.
    pub uncovered: Vec<(usize, usize)>,
    /// Stretches covered by more than one span.
    pub overlapping: Vec<(usize, usize)>,
}

impl Coverage {
    pub fn is_exact_partition(&self) -> bool {
        self.uncovered.is_empty() && self.overlapping.is_empty()
    }
}

/// Check whether the spans partition every visible character of `text`.
///
/// Whitespace may fall outside every span — it is typesetting distance, not content.
pub fn check_coverage(text: &str, spans: &[Span]) -> Coverage {
    let mut hits = vec![0u16; text.len()];
    for s in spans {
        for slot in hits.iter_mut().take(s.end).skip(s.start) {
            *slot = slot.saturating_add(1);
        }
    }

    let mut coverage = Coverage::default();
    let mut run_uncovered: Option<(usize, usize)> = None;
    let mut run_overlap: Option<(usize, usize)> = None;

    for (offset, ch) in text.char_indices() {
        let count = hits[offset];
        let visible = !ch.is_whitespace();

        push_run(
            &mut run_uncovered,
            &mut coverage.uncovered,
            offset,
            ch,
            visible && count == 0,
        );
        push_run(
            &mut run_overlap,
            &mut coverage.overlapping,
            offset,
            ch,
            count > 1,
        );
    }
    flush(&mut run_uncovered, &mut coverage.uncovered);
    flush(&mut run_overlap, &mut coverage.overlapping);

    coverage
}

fn push_run(
    run: &mut Option<(usize, usize)>,
    out: &mut Vec<(usize, usize)>,
    offset: usize,
    ch: char,
    active: bool,
) {
    match (active, run.as_mut()) {
        (true, Some(r)) => r.1 = offset + ch.len_utf8(),
        (true, None) => *run = Some((offset, offset + ch.len_utf8())),
        (false, Some(_)) => flush(run, out),
        (false, None) => {}
    }
}

fn flush(run: &mut Option<(usize, usize)>, out: &mut Vec<(usize, usize)>) {
    if let Some(r) = run.take() {
        out.push(r);
    }
}
