//! Splitting a headword line into fields, with spans tracing back to the original text.
//!
//! The shapes measured across the 7,622 headword lines of the 2026 edition:
//!
//! ```text
//! 阿  A  c.                  glyph + reading + label        (the common form)
//! 丫  A  (Nha.)  c.          plus a Sino-Vietnamese reading in parentheses (39 lines)
//! 標  Biêu, (tiêu)  n.       the reading has a variant after the comma
//! Bấm  n.                    NO glyph — the print uses an image (29 lines)
//! ```
//!
//! Glyph distribution: BMP 6,500 · Ext-B 1,062 · PUA 31 · no glyph 29.
//!
//! Every field returned is a [`Span`], so [`HeadwordLine::spans`] feeds straight into
//! [`crate::span::check_coverage`] to prove the split drops nothing and double-counts nothing.

use dnqatv_core::model::{GlyphKind, Pos};

use crate::span::Span;

/// One headword line, split into fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadwordLine {
    /// The Han-Nom glyph. `None` when the print uses an image and the text layer has nothing.
    pub glyph: Option<Span>,
    /// The `*` the LƯU Ý page defines to mark a glyph presented as an image.
    ///
    /// It only appears on entries without a Unicode glyph. Letting it into the reading would
    /// make the reading `"**  Dạng"` and the ordering invariant would not see the initial.
    pub image_marker: Option<Span>,
    /// The Quốc ngữ reading, including anything after a comma.
    pub reading: Span,
    /// A Sino-Vietnamese or alternative reading, in parentheses.
    pub alternate: Option<Span>,
    /// The part-of-speech labels at the end of the line, in printed order.
    ///
    /// It must be a list, not a scalar: measuring the 7,622 headword lines shows
    /// **548 lines carrying two labels** (`c. n.` 543 times, `n. c.` 3 times, and twice the
    /// print omits a dot). Collapsing `c. n.` into `cn.` would be interpretation, not
    /// recording — the two spellings differ on paper, so they differ here too.
    pub labels: Vec<(Span, Pos)>,
    /// The text remaining AFTER the label, when the print starts the definition on the headword line.
    ///
    /// Exactly 6 such lines measured in the whole book, e.g. p.525 `馬  Mã  c. N` — the `N`
    /// begins "Ngựa" and the rest is on the next line. Requiring the label at the end would
    /// drop these 6 entries; recording them with a warning keeps them without guessing.
    pub trailing_text: Option<Span>,
}

impl HeadwordLine {
    /// Every span of the line, for the conservation check.
    pub fn spans(&self) -> Vec<Span> {
        let mut out = Vec::with_capacity(4);
        out.extend(self.glyph);
        out.extend(self.image_marker);
        out.push(self.reading);
        out.extend(self.alternate);
        out.extend(self.labels.iter().map(|(s, _)| *s));
        out.extend(self.trailing_text);
        out
    }

    /// The line has an anomaly needing review: the print starts the definition on the headword line.
    pub const fn needs_review(&self) -> bool {
        self.trailing_text.is_some()
    }

    /// The labels, without spans.
    pub fn pos_list(&self) -> Vec<Pos> {
        self.labels.iter().map(|(_, p)| *p).collect()
    }

    /// Whether the reading contains something that looks like a missed part-of-speech label.
    ///
    /// A supplementary gate to the conservation law, and its reason is very concrete:
    /// conservation catches LOST characters, not MISFILED ones. The print has two places
    /// where a label lacks its dot (`當 Đương. c  n.` p.295 and `興 Hấng (Hứng) n  c.` p.353);
    /// on the first, the bare `c` lands in the reading yet is still "fully covered", so gate ② misses it.
    ///
    /// This function CHANGES nothing — it only raises the case for a human, per the no-guessing rule.
    pub fn reading_has_stray_label(&self, text: &str) -> bool {
        self.reading
            .slice(text)
            .split_whitespace()
            .any(|token| matches!(token.trim_end_matches('.'), "c" | "n" | "cn"))
    }

    /// Whether the glyph exists in the text layer. False means the print uses an image.
    pub const fn has_glyph(&self) -> bool {
        self.glyph.is_some()
    }

    /// Whether the LAST label on this line is really the capital that opens the definition.
    ///
    /// The 2026 edition rebuilt its text layer, and on 210 entries that rebuild turned the
    /// opening capital of the gloss into a second part-of-speech label. `壓 Áp` is the plain
    /// case: the 1895 print sets
    ///
    /// ```text
    /// 壓  Áp. c. Ngăn, giữ, đè, nhận xuống.
    /// ```
    ///
    /// — ONE label — while the 2026 text layer emits `壓  Áp  c. n.` on one line and
    /// `găn, giữ, đè, nhận xuống.` on the next. The `N` became `n.`, and 210 definitions lost
    /// their first letter. The independent 1895 transcription agrees with the print on every
    /// one of the 210, and the scan was read by eye for `壓 Áp` itself.
    ///
    /// The test is deliberately narrow, and neither half of it is a guess:
    ///
    /// * the line carries **more than one** label, so removing the last still leaves the
    ///   entry with a part of speech — a single label is never touched;
    /// * the definition begins with a **lowercase** letter, which this book never does: a
    ///   gloss is a sentence and opens with a capital.
    ///
    /// The restored letter is the letter the label was made of, not a letter chosen to fit.
    /// `cn.` cannot match: it is two letters, and this asks for exactly one.
    pub fn gloss_initial_label(&self, line: &str, definition: &str) -> Option<GlossInitial> {
        if self.labels.len() < 2 {
            return None;
        }
        let opens_lowercase = definition
            .trim_start()
            .chars()
            .next()
            .is_some_and(char::is_lowercase);
        if !opens_lowercase {
            return None;
        }

        let (span, pos) = *self.labels.last()?;
        let mut letters = span.slice(line).trim_end_matches('.').chars();
        let letter = letters.next()?;
        if letters.next().is_some() || !letter.is_ascii_alphabetic() {
            return None;
        }

        // The letter must actually begin the word that follows. Vietnamese spelling settles
        // this without a judgement call: `N` + `găn` is the onset *ng*, but `N` + `tụ` is not
        // a syllable at all. Measured on the 211 candidates: 204 form a legal onset, and the
        // 7 that do not are all places where the print really does set a second label and
        // the sense after it opens in lowercase (`đoàn c. n. tụ; bầy, lũ.`).
        let next = definition.trim_start().chars().next()?;
        if !begins_syllable(letter, next) {
            return None;
        }

        Some(GlossInitial {
            letter: letter.to_ascii_uppercase(),
            dropped: pos,
        })
    }
}

/// Whether `first` followed by `second` can open a Vietnamese syllable.
///
/// Only the onset matters here, so the table is the list of onsets the language has:
/// single consonants, plus the digraphs and trigraphs `ch gh gi kh ng ngh nh ph qu th tr`.
/// Everything else is settled by `second` being a vowel.
///
/// This is a fact about the language, not a threshold: it never needs tuning, and it is what
/// separates `N` + `găn` (the onset *ng*) from `N` + `tụ` (no such syllable).
pub fn begins_syllable(first: char, second: char) -> bool {
    const VOWELS: &str = "aeiouy";
    let head = first.to_ascii_lowercase();
    // `fold` takes the tone and the vowel mark off, so `ố` and `ư` both arrive as a plain
    // letter. It is the same function search uses, and it knows `đ` is not `d` plus a mark.
    let folded = dnqatv_core::text::fold(&second.to_string());
    let Some(bare) = folded.chars().next() else {
        return false;
    };

    if VOWELS.contains(bare) {
        return true;
    }
    matches!(
        (head, bare),
        ('c', 'h')
            | ('g', 'h' | 'i')
            | ('k', 'h')
            | ('n', 'g' | 'h')
            | ('p', 'h')
            | ('q', 'u')
            | ('t', 'h' | 'r')
    )
}

/// A part-of-speech label that is really the first letter of the definition.
///
/// See [`HeadwordLine::gloss_initial_label`] for the evidence and the test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlossInitial {
    /// The letter to put back at the head of the definition, as the book capitalises it.
    pub letter: char,
    /// The label that has to go, because it was never a label.
    pub dropped: Pos,
}

/// Whether this character is a Han-Nom glyph opening a headword line.
///
/// The Private Use Area is included: 31 entries use the private encoding of the Nom Na Tong
/// font, and those are real glyphs, not junk.
pub fn is_glyph_char(c: char) -> bool {
    !matches!(GlyphKind::from_codepoint(c as u32), GlyphKind::Bmp)
        || matches!(c as u32, 0x3400..=0x9FFF | 0xF900..=0xFAFF)
}

/// Split a headword line. Returns `None` when the line is not shaped like a headword.
///
/// Deliberately makes NO attempt to rescue near-misses: a non-matching line must surface at
/// the gate for a human, never be guessed into an entry.
pub fn parse_headword(text: &str) -> Option<HeadwordLine> {
    let mut labels = find_trailing_labels(text);
    let mut trailing_text = None;
    if labels.is_empty() {
        let (found, tail) = find_labels_before_text(text)?;
        labels = found;
        trailing_text = Some(tail);
    }
    let (first_label, _) = *labels.first()?;
    let label_start = first_label.start();

    let head = &text[..label_start];

    // The glyph: the first Han-Nom character, which must sit at the very start of the rest.
    let trimmed_start = head.len() - head.trim_start().len();
    let first = head[trimmed_start..].chars().next()?;
    let glyph = if is_glyph_char(first) {
        Some(Span::new(text, trimmed_start, trimmed_start + first.len_utf8()).ok()?)
    } else {
        None
    };

    // Cut inside `head`, not inside `text`: taking to the end of the string would let the
    // reading swallow the label, and the parenthesis could be caught from the definition after it.
    let mut rest_start = glyph.map_or(trimmed_start, Span::end);

    // The `*` of an image glyph comes first, before the reading.
    let marker_from = rest_start + count_leading_space(&head[rest_start..]);
    let stars = head[marker_from..]
        .chars()
        .take_while(|c| *c == '*')
        .count();
    let image_marker = if stars > 0 {
        let span = Span::new(text, marker_from, marker_from + stars).ok()?;
        rest_start = span.end();
        Some(span)
    } else {
        None
    };

    let rest = &head[rest_start..];

    // The parenthesised part, when present, always follows the reading.
    let alternate = find_parenthesised(text, rest_start, rest);

    let reading_end = alternate.map_or(rest_start + rest.len(), Span::start);
    let reading_slice = &text[rest_start..reading_end];
    let lead = reading_slice.len() - reading_slice.trim_start().len();
    let trail = reading_slice.len() - reading_slice.trim_end().len();
    let reading = Span::new(text, rest_start + lead, reading_end - trail).ok()?;

    Some(HeadwordLine {
        glyph,
        image_marker,
        reading,
        alternate,
        labels,
        trailing_text,
    })
}

/// Find the label when it is NOT at the end of the line, returning the remaining text with it.
///
/// A strict condition prevents false matches: the line must **start with a Han-Nom glyph**.
/// A sub-entry starts with an italic form and so cannot get in. Measured: this rule admits
/// exactly 6 lines out of all 89,690, and no others.
fn find_labels_before_text(text: &str) -> Option<(Vec<(Span, Pos)>, Span)> {
    let first = text.trim_start().chars().next()?;
    if !is_glyph_char(first) {
        return None;
    }
    // The end position of the last label found.
    let mut best: Option<(usize, Vec<(Span, Pos)>)> = None;
    for pos in [Pos::ChuNhoDungNom, Pos::ChuNho, Pos::ChuNom] {
        let label = pos.book_label();
        let mut from = 0usize;
        while let Some(rel) = text[from..].find(label) {
            let start = from + rel;
            let end = start + label.len();
            from = end;
            if !text[..start].ends_with(char::is_whitespace) {
                continue;
            }
            if !text[end..].starts_with(char::is_whitespace) {
                continue;
            }
            let Ok(span) = Span::new(text, start, end) else {
                continue;
            };
            if best.as_ref().is_none_or(|(e, _)| end > *e) {
                best = Some((end, vec![(span, pos)]));
            }
        }
    }
    let (end, labels) = best?;
    let tail = &text[end..];
    let lead = tail.len() - tail.trim_start().len();
    let trail = tail.len() - tail.trim_end().len();
    let span = Span::new(text, end + lead, text.len() - trail).ok()?;
    Some((labels, span))
}

/// The label run at the end of the line, returned left to right.
///
/// Scans right to left picking up consecutive labels, so `c. n.` yields two elements.
///
/// Trying `cn.` before `c.` is mandatory: the other order reads `cn.` as `n.` and loses the
/// "a Chinese character also used as Nôm" distinction.
///
/// Labels missing their dot are deliberately NOT accepted (the print has 2 such places).
/// Guessing that a bare `c` is a label would open the door to mistaking a lowercase letter
/// in the reading; those two lines must surface at the gate for a human.
///
/// Run-together label sequences, `c.n.`, **are** accepted — the print has 4 of them, and
/// gate ④ caught it: four real entries (辰 Thìn, 對 Tụi, 焠 Tui, 樣 Dạng) were dropped entirely.
/// This is not a guess: the dot after `c` ends the first label decisively, so `c.n.` and
/// `c. n.` differ only in the typesetter whitespace. The relaxation stays tight — the next
/// label must follow ANOTHER LABEL directly, not just any dot.
fn find_trailing_labels(text: &str) -> Vec<(Span, Pos)> {
    let mut out: Vec<(Span, Pos)> = Vec::new();
    let mut end = text.trim_end().len();

    loop {
        let mut matched = false;
        for pos in [Pos::ChuNhoDungNom, Pos::ChuNho, Pos::ChuNom] {
            let label = pos.book_label();
            let Some(start) = end.checked_sub(label.len()) else {
                continue;
            };
            if text.get(start..end) != Some(label) {
                continue;
            }
            if !preceded_by_label_boundary(text, start) {
                continue;
            }
            let Ok(span) = Span::new(text, start, end) else {
                continue;
            };
            out.push((span, pos));
            end = text[..start].trim_end().len();
            matched = true;
            break;
        }
        if !matched {
            break;
        }
    }

    out.reverse();
    out
}

fn find_parenthesised(text: &str, offset: usize, rest: &str) -> Option<Span> {
    let open = rest.rfind('(')?;
    let close = rest[open..].find(')')? + open;
    Span::new(text, offset + open, offset + close + 1).ok()
}

/// A second headword line under the same glyph, opening directly with a label.
///
/// 55 such lines measured, e.g. p.26: after `北 Bấc c.` comes `n. Bấc ; bức tức.` — the same
/// glyph 北 read the Nôm way. Glyph and reading are inherited from the preceding entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuedHeadwordLine {
    /// The labels at the START of the line, in printed order.
    pub labels: Vec<(Span, Pos)>,
    /// The rest of the line. `None` when the line holds only labels.
    pub definition: Option<Span>,
}

impl ContinuedHeadwordLine {
    pub fn spans(&self) -> Vec<Span> {
        let mut out: Vec<Span> = self.labels.iter().map(|(s, _)| *s).collect();
        out.extend(self.definition);
        out
    }

    pub fn pos_list(&self) -> Vec<Pos> {
        self.labels.iter().map(|(_, p)| *p).collect()
    }
}

/// Parse a continued headword line. Returns `None` when the line does not open with a label.
pub fn parse_continued_headword(text: &str) -> Option<ContinuedHeadwordLine> {
    let mut labels: Vec<(Span, Pos)> = Vec::new();
    let mut at = text.len() - text.trim_start().len();

    loop {
        let mut matched = false;
        for pos in [Pos::ChuNhoDungNom, Pos::ChuNho, Pos::ChuNom] {
            let label = pos.book_label();
            let end = at + label.len();
            if text.get(at..end) != Some(label) {
                continue;
            }
            // A space must follow, otherwise `cá.` would be misread as a label.
            if !text[end..].starts_with(char::is_whitespace) {
                continue;
            }
            let Ok(span) = Span::new(text, at, end) else {
                continue;
            };
            labels.push((span, pos));
            let rest = &text[end..];
            at = end + (rest.len() - rest.trim_start().len());
            matched = true;
            break;
        }
        if !matched {
            break;
        }
    }

    if labels.is_empty() {
        return None;
    }
    let definition = {
        let tail = &text[at..];
        let trail = tail.len() - tail.trim_end().len();
        Span::new(text, at, text.len() - trail).ok()
    };
    Some(ContinuedHeadwordLine { labels, definition })
}

/// The number of leading whitespace bytes.
///
/// Uses `trim_start`, so it includes the non-breaking space (U+00A0) — the print uses
/// one between the `*` and the reading on image-glyph entries.
fn count_leading_space(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

/// Whether a label at position `start` sits on a valid boundary.
///
/// Valid when it follows whitespace (the common form), or **runs together right after
/// another label** — the print has 4 lines writing `c.n.`. The second condition is
/// deliberately tight: it must be exactly one more label, not any dot, so a lowercase letter in a reading cannot slip in.
pub fn preceded_by_label_boundary(text: &str, start: usize) -> bool {
    let before = &text[..start];
    if before.ends_with(char::is_whitespace) {
        return true;
    }
    Pos::ALL.iter().any(|p| before.ends_with(p.book_label()))
}
