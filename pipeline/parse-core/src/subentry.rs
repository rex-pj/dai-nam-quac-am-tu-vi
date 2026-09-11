//! Splitting a sub-entry line into three parts, using **style boundaries** rather than guessing at dots.
//!
//! The print sets the sub-entry form in italic and the definition in regular. Definitions are
//! full of dots too, so splitting on punctuation is guessing; splitting on style is reading
//! what the print recorded.
//!
//! The shapes measured across 57,892 sub-entry lines:
//!
//! ```text
//! Ir      54,950  94.92%   ― gươm. Nạm gươm.
//! HIr        757   1.31%   娑婆世界 Ta ― thế giái. Ngao du khắp chỗ.
//! rHIr       595   1.03%   | 意 ― ý. Dua theo một ý.
//! HrIr       483   0.83%   瘖 | Ám ―. Câm, ngọng.
//! ```
//! (`r` regular · `I` italic · `H` Han font)
//!
//! Style alone is not quite enough, because the typesetter set the `|` placeholder and the
//! spaces between Han glyphs in the ITALIC font. Taking the form from the first italic piece
//! therefore begins inside the HAN column on 827 lines — `女 | 男 婚 Nữ ― nam hôn.` came out
//! as han `女` + form `| 男 婚 Nữ ― nam hôn`. [`quoc_ngu_starts_at`] walks that Han-column
//! material back where it belongs; see its notes for how the boundary was verified.
//!
//! What remains flagged is the form genuinely interrupted by another font — 7 lines, e.g.
//! `Ẩn 微 Ẩn vi` — where no rule of the book says which side the Han belongs to.
//!
//! Style is also not quite enough at the OTHER end of the form: the print sets the full stop
//! that closes the form in the regular font, so style alone leaves it heading the definition.
//! [`separator_stop_end`] gives it back to the form, where the page puts it.

use dnqatv_core::model::TextStyle;

use crate::expand::GLYPH_PLACEHOLDER;
use crate::span::Span;

/// A piece of same-styled text. A light copy of `Segment` from `extract-core`, so that
/// `parse-core` need not depend on the PDF reading layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledSegment<'a> {
    pub style: TextStyle,
    pub text: &'a str,
}

/// One sub-entry line, split into fields.
///
/// Three adjacent ranges covering the whole line: `[Han][form][definition]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubEntryLine {
    /// The Han part before the form, including any `|` placeholder inside it.
    pub han_form: Option<Span>,
    /// The Quốc ngữ form — the italic part, from the first italic piece to the last, plus the
    /// full stop that closes it. See [`separator_stop_end`] for why that stop is not italic.
    pub reading_form: Span,
    /// The definition. `None` when the definition starts on the next line.
    pub definition: Option<Span>,
    /// The italic piece count. Above 1 means another font interrupted the form — needs review.
    pub italic_segments: usize,
}

impl SubEntryLine {
    pub fn spans(&self) -> Vec<Span> {
        let mut out = Vec::with_capacity(3);
        out.extend(self.han_form);
        out.push(self.reading_form);
        out.extend(self.definition);
        out
    }

    /// The form is interrupted by a non-italic piece — 7 of 57,892 lines.
    ///
    /// Counted from the column boundary onwards, so the stray `| ` piece that sits in the
    /// Han column does not make a correctly split line look broken.
    pub const fn needs_review(&self) -> bool {
        self.italic_segments > 1
    }
}

/// Join the pieces into the full text of the line.
pub fn line_text(segments: &[StyledSegment<'_>]) -> String {
    segments.iter().map(|s| s.text).collect()
}

/// Split a sub-entry line. Returns `None` when no italic piece carries content.
///
/// No italic means no form, so this is not a sub-entry line — and inventing a form from a
/// dot is exactly what the plan forbids.
pub fn parse_sub_entry(segments: &[StyledSegment<'_>]) -> Option<SubEntryLine> {
    // The starting byte offset of each piece in the joined string.
    let mut offsets = Vec::with_capacity(segments.len() + 1);
    let mut at = 0usize;
    for s in segments {
        offsets.push(at);
        at += s.text.len();
    }
    offsets.push(at);
    let total = at;

    let is_italic =
        |s: &StyledSegment<'_>| s.style == TextStyle::Italic && !s.text.trim().is_empty();
    let first = segments.iter().position(is_italic)?;
    let last = segments.iter().rposition(is_italic)?;

    let text = line_text(segments);
    let form_start = quoc_ngu_starts_at(&text, offsets[first], offsets[last + 1]);
    let after_italic = offsets[last + 1];
    let form_end = separator_stop_end(&text, after_italic).unwrap_or(after_italic);

    // Count only the italic pieces at or after the column boundary. The stray `| ` piece
    // skipped above is set in italic but sits in the HAN column, so counting it would keep
    // 827 correctly split lines flagged for a fault they do not have.
    let italic_segments = segments
        .iter()
        .enumerate()
        .filter(|(k, s)| is_italic(s) && offsets[k + 1] > form_start)
        .count();

    let han_form = trimmed_span(&text, 0, form_start);
    let reading_form = trimmed_span(&text, form_start, form_end)?;
    let definition = trimmed_span(&text, form_end, total);

    Some(SubEntryLine {
        han_form,
        reading_form,
        definition,
        italic_segments,
    })
}

/// One sub-entry after its line and any continuation lines have been read, ready to be
/// written out. Kept here so [`merge_wrapped_forms`] can be tested without a file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubEntryParts {
    pub han_form: Option<String>,
    pub han_expanded: Option<String>,
    pub form: String,
    pub form_expanded: String,
    pub definition: String,
    pub needs_review: bool,
}

/// Join two pieces of a line with a single space, the way continuation lines are joined.
fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_owned();
    }
    if b.is_empty() {
        return a.to_owned();
    }
    if a.ends_with(char::is_whitespace) {
        format!("{a}{b}")
    } else {
        format!("{a} {b}")
    }
}

/// Re-join a Quốc ngữ form that the print wrapped onto the next line.
///
/// A long Hán-Việt proverb does not fit one column, so the print sets its reading across two
/// lines. Both lines are italic and both may carry a placeholder, so the line classifier sees
/// **two** sub-entries — the first holding the whole Han column and no definition at all, the
/// second holding the tail of the form and the definition that belongs to both:
///
/// ```text
/// 君 子 以 財 | 身 小 人 以 身 | 財   Quân tử dỉ tài― thân, tiểu      <- no definition anywhere
///                                    nhơn dĩ thân― tài. Người khôn…
/// ```
///
/// A sub-entry with no definition on its own line **and none on any continuation line** is
/// not a sub-entry: the book does not print a phrase and then say nothing about it. That is
/// the signal used here, and it is exact — all 14 such sub-entries in the body are this
/// shape, every one a proverb split across two lines.
///
/// Only the record is merged; the line indices are untouched, so gate ③ still sees every
/// content line consumed exactly once. A trailing empty definition with nothing to merge into
/// is left alone rather than dropped.
pub fn merge_wrapped_forms(subs: Vec<SubEntryParts>) -> (Vec<SubEntryParts>, usize) {
    let mut out: Vec<SubEntryParts> = Vec::with_capacity(subs.len());
    let mut merged = 0usize;
    let mut carry: Option<SubEntryParts> = None;

    for mut s in subs {
        if let Some(head) = carry.take() {
            s.han_form = match (head.han_form, s.han_form) {
                (Some(a), Some(b)) => Some(join(&a, &b)),
                (a, b) => a.or(b),
            };
            s.han_expanded = match (head.han_expanded, s.han_expanded) {
                (Some(a), Some(b)) => Some(join(&a, &b)),
                (a, b) => a.or(b),
            };
            s.form = join(&head.form, &s.form);
            s.form_expanded = join(&head.form_expanded, &s.form_expanded);
            s.needs_review = head.needs_review || s.needs_review;
            merged += 1;
        }
        if s.definition.trim().is_empty() {
            carry = Some(s);
            continue;
        }
        out.push(s);
    }
    // Nothing followed it: keep it as it stands rather than losing the text.
    if let Some(last) = carry {
        out.push(last);
    }
    (out, merged)
}

/// Han ideographs, by block. Used only to tell Han-column material from a Quốc ngữ form.
const fn is_han(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF        // radicals, Kangxi radicals
        | 0x3400..=0x4DBF      // Extension A
        | 0x4E00..=0x9FFF      // unified ideographs
        | 0xF900..=0xFAFF      // compatibility ideographs
        | 0x20000..=0x3134F    // Extension B and beyond
    )
}

/// Where the Quốc ngữ form really begins inside the italic run.
///
/// The typesetter set the `|` placeholder — and the spaces between Han glyphs — in the
/// ITALIC font. So "the first italic piece" lands inside the HAN column on 827 lines and
/// drags the rest of the Han across with it:
///
/// ```text
/// 女 | 男 婚  Nữ ― nam hôn.      split as  han `女`  +  form `| 男 婚 Nữ ― nam hôn`
///                                   should be  han `女 | 男 婚`  +  form `Nữ ― nam hôn`
/// ```
///
/// Only `|`, Han characters, spaces and commas are given back: none of those can begin a Quốc
/// ngữ form. A reading placeholder `―` stops the walk, because that mark belongs to THIS
/// column — the two rules differ and confusing them is exactly what the DẤU RIÊNG page warns of.
///
/// The comma earns its place: the Han column often holds two parallel phrases separated by
/// one, as in `董 | , 照 | , | 諒, | 原`. Stopping there left the rest of the Han in the form on
/// two lines. No Quốc ngữ form opens with a comma, so nothing else can be swept up by it.
///
/// Nothing moves unless a `|` was actually stranded, and nothing moves if that would leave
/// the form empty: a line is never dropped to make a boundary look tidy.
///
/// Checked against the Wikisource transcription of the 1895 print: of the lines this
/// touches, 679 agree with where that edition puts the boundary and **none** contradict it.
/// In all 58,076 sub-entries of that transcription, not one leaves a `|` in the Quốc ngữ column.
fn quoc_ngu_starts_at(text: &str, from: usize, to: usize) -> usize {
    let Some(slice) = text.get(from..to) else {
        return from;
    };
    let mut seen_placeholder = false;
    let mut at = from;
    for c in slice.chars() {
        if c == GLYPH_PLACEHOLDER {
            seen_placeholder = true;
        } else if !(c.is_whitespace() || c == ',' || is_han(c)) {
            break;
        }
        at += c.len_utf8();
    }
    if seen_placeholder && at < to {
        at
    } else {
        from
    }
}

/// Where the full stop between the form and its definition ends, if the print sets one.
///
/// The typesetter sets that stop in the REGULAR font, not the italic of the form:
///
/// ```text
/// [italic "Cây ―"][regular ". id."]
/// ```
///
/// So a split on style alone hands it to the definition, and the fields come back out as
/// form `Cây ―` + definition `. id.` — a dot floating at the head of the definition column,
/// reading as `Cây Róng . id.` with a space the print never sets. The stop closes the form:
/// the page says `Cây ―. id.` and so does the form span now.
///
/// Measured over the 57,878 sub-entries of the body: 57,780 definitions open with `. ` and
/// no other shape of leading dot occurs anywhere. The remaining 98 open with a lowercase
/// letter — wrapped tails, which carry no stop because their form sits on the line above.
///
/// Nothing moves unless visible text follows the stop. A sub-entry whose definition begins
/// on the NEXT line must keep an empty definition, because that emptiness is the signal
/// [`merge_wrapped_forms`] reads; swallowing the last character of such a line would invent
/// a sub-entry the book does not print.
fn separator_stop_end(text: &str, from: usize) -> Option<usize> {
    let rest = text.get(from..)?;
    let lead = rest.len() - rest.trim_start().len();
    let after = rest.get(lead..)?.strip_prefix('.')?;
    if after.trim().is_empty() {
        return None;
    }
    Some(from + lead + '.'.len_utf8())
}

/// The byte range with whitespace trimmed from both ends; `None` if it is all whitespace.
fn trimmed_span(text: &str, start: usize, end: usize) -> Option<Span> {
    let slice = text.get(start..end)?;
    let lead = slice.len() - slice.trim_start().len();
    let trail = slice.len() - slice.trim_end().len();
    if lead + trail >= slice.len() {
        return None;
    }
    Span::new(text, start + lead, end - trail).ok()
}
