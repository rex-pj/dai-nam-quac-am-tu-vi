//! Classifying the role of each printed line.
//!
//! Every threshold here is **measured**, not estimated. Across all 1028 body pages: the
//! running head sits at exactly ONE `y` value, and the printed page number at exactly one too.
//! That makes head/foot filtering exact rather than a guess.
//!
//! Role distribution measured over 89,625 body lines:
//! sub-entry 63.49% · continuation 25.76% · headword 7.26% (7,622 lines)
//! · running head 2.26% · page number 1.15% · continued headword 0.06% · section title 0.03%.

/// The height of the running head. Measured across all 1028 body pages: exactly one value.
pub const RUNNING_HEAD_Y: f64 = 797.89;

/// The height of the printed page number. Also exactly one value across all 1028 pages.
pub const PAGE_NUMBER_Y: f64 = 25.89;

/// The tolerance when comparing `y`.
///
/// NOT a clustering threshold — each position has exactly one `y` value, so this margin only
/// absorbs rounding as floats pass through JSON. Widening it would destroy the exactness.
pub const Y_EPSILON: f64 = 0.005;

/// The running title at the top of every body page.
pub const BOOK_TITLE: &str = "ĐẠI NAM QUẤC ÂM TỰ VỊ";

/// The prefix of the line opening each letter section, e.g. `CHỮ A`.
pub const SECTION_PREFIX: &str = "CHỮ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineRole {
    /// The running head, naming the first and last entry on the page. Not dictionary content.
    RunningHead,
    /// The printed page number.
    PageNumber,
    /// `ĐẠI NAM QUẤC ÂM TỰ VỊ` or `CHỮ A`.
    SectionTitle,
    /// The start of an entry: `𨰲  Lõm  n.`
    Headword,
    /// A second entry sharing one glyph, opening directly with a label: `n. Bấc ; bức tức.`
    /// 55 such lines measured.
    ContinuedHeadword,
    /// A sub-entry, recognised by the placeholder `―` (Quốc ngữ column) or `|` (Han column).
    SubEntry,
    /// The continuation of the definition on the line above.
    Continuation,
}

impl LineRole {
    pub const ALL: [LineRole; 7] = [
        Self::RunningHead,
        Self::PageNumber,
        Self::SectionTitle,
        Self::Headword,
        Self::ContinuedHeadword,
        Self::SubEntry,
        Self::Continuation,
    ];

    /// Whether this role carries dictionary content. Heads, page numbers and titles do not.
    pub const fn is_content(self) -> bool {
        matches!(
            self,
            Self::Headword | Self::ContinuedHeadword | Self::SubEntry | Self::Continuation
        )
    }
}

fn is_at(y: f64, target: f64) -> bool {
    (y - target).abs() < Y_EPSILON
}

/// The three label spellings of the print. Trying `cn.` before `c.` is mandatory.
pub const POS_LABELS: [&str; 3] = ["cn.", "c.", "n."];

/// A part-of-speech label at the END of the line — the mark of a headword line.
///
/// Also accepts a **run-together** label sequence: the print has 4 lines writing `c.n.` with
/// no space (辰 Thìn p.864 · 對 Tụi p.965 · 焠 Tui · 樣 Dạng). Requiring a space between the
/// two labels would lose four real entries, and gate ④ caught exactly those four against the index.
///
/// This is not a guess: the dot after `c` ends the first label decisively, so `c.n.` and
/// `c. n.` differ only in the typesetter whitespace. The LEFT boundary of the whole run must
/// still be whitespace, so `abc.` does not slip through.
pub fn ends_with_pos_label(text: &str) -> bool {
    strip_trailing_label_run(text).is_some()
}

/// Strip the label run at the end of a line and return the remaining head.
///
/// `None` when there is no label, or when the label-looking text is really a word ending.
pub fn strip_trailing_label_run(text: &str) -> Option<&str> {
    let mut t = text.trim_end();
    let mut found = 0usize;

    while let Some(head) = POS_LABELS.iter().find_map(|l| t.strip_suffix(l)) {
        if head.ends_with(char::is_whitespace) {
            // A clean left boundary: this is definitely a label.
            found += 1;
            t = head.trim_end();
            continue;
        }
        if POS_LABELS.iter().any(|l| head.ends_with(l)) {
            // Run together right after another label — `c.n.`. The left boundary of the whole
            // run is checked on the next iteration, when the leftmost label is stripped.
            found += 1;
            t = head;
            continue;
        }
        // `abc.` — this dot belongs to a word, not to a label.
        return None;
    }

    if found == 0 { None } else { Some(t) }
}

/// A part-of-speech label at the START of the line — a second entry under the same glyph.
pub fn starts_with_pos_label(text: &str) -> bool {
    let t = text.trim_start();
    for label in ["cn.", "c.", "n."] {
        if let Some(rest) = t.strip_prefix(label)
            && rest.starts_with(char::is_whitespace)
        {
            return true;
        }
    }
    false
}

/// The placeholder characters, taken from COUNTING the whole body, not from the DẤU RIÊNG page.
///
/// The DẤU RIÊNG page writes the placeholder as `—` (U+2014), but the 2026 typesetter used
/// `―` (U+2015) for most of them. Counted across the body:
///
/// | character | count | line-initial |
/// |---|---|---|
/// | `―` U+2015 | 58,706 | 27,166 |
/// | `|` U+007C | 2.197 | 629 |
/// | `—` U+2014 | 1.065 | 540 |
/// | `–` U+2013 | 129 | 13 |
///
/// Hard-coding only `―`, as the first version did, misses over 1,100 lines. All four are
/// accepted here, but **not normalised** to one character — whatever the print wrote is stored.
pub const PLACEHOLDERS: [char; 4] = ['―', '—', '–', '|'];

/// The print also uses two joined hyphens on 24 lines.
pub const PLACEHOLDER_DIGRAPH: &str = "--";

/// A single `-` (U+002D, 4,039 times) is NOT a placeholder — it is the hyphen inside compounds
/// such as "Di-đà", "Thiên-trúc". Verified: no line begins with a single `-`.
pub fn has_placeholder(text: &str) -> bool {
    text.contains(PLACEHOLDER_DIGRAPH) || text.contains(PLACEHOLDERS)
}

/// The two sub-entry signals and how far they agree.
///
/// Measured over 78,870 content lines of the body:
///
/// | signal | lines | what it really is |
/// |---|---|---|
/// | both | 57,892 (73.4%) | a sub-entry |
/// | neither | 18,188 (23.1%) | a continuation |
/// | italic only | 2,751 (3.5%) | **an italic quotation**, not a sub-entry |
/// | placeholder only | 39 (0.05%) | ambiguous, needs a human |
///
/// Why the UNION of both signals is needed: the print uses italics for sub-entry forms AND
/// for verses and Han phrases quoted inside definitions; the placeholder characters also
/// appear inside those same quotations. Either signal alone is wrong by thousands of lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubEntrySignal {
    /// Both italic and a placeholder — certainly a sub-entry.
    Both,
    /// Italic only — almost always a quotation, not a sub-entry.
    ItalicOnly,
    /// Placeholder only — rare (39 lines) and needs human review.
    PlaceholderOnly,
    Neither,
}

impl SubEntrySignal {
    pub fn of(has_italic: bool, has_placeholder: bool) -> Self {
        match (has_italic, has_placeholder) {
            (true, true) => Self::Both,
            (true, false) => Self::ItalicOnly,
            (false, true) => Self::PlaceholderOnly,
            (false, false) => Self::Neither,
        }
    }

    /// Whether this signal is contradictory — used to collect the review list.
    pub const fn is_ambiguous(self) -> bool {
        matches!(self, Self::PlaceholderOnly)
    }
}

/// Determine the role of a line.
///
/// `is_body_page` separates the body from the front matter: the front matter has no running
/// head, so `y` filtering must not apply there.
pub fn classify(y: f64, text: &str, is_body_page: bool, has_italic: bool) -> LineRole {
    if is_body_page && is_at(y, RUNNING_HEAD_Y) {
        return LineRole::RunningHead;
    }
    if is_at(y, PAGE_NUMBER_Y) {
        return LineRole::PageNumber;
    }

    let t = text.trim();
    if t == BOOK_TITLE
        || (t.starts_with(SECTION_PREFIX)
            && t.chars().count() <= SECTION_PREFIX.chars().count() + 1)
    {
        return LineRole::SectionTitle;
    }
    if ends_with_pos_label(t) || starts_with_glyph_and_has_label(t) {
        return LineRole::Headword;
    }
    if starts_with_pos_label(t) {
        return LineRole::ContinuedHeadword;
    }
    match SubEntrySignal::of(has_italic, has_placeholder(t)) {
        SubEntrySignal::Both => LineRole::SubEntry,
        _ => LineRole::Continuation,
    }
}

/// A line starting with a Han-Nom glyph and carrying a label, even one not at the end.
///
/// Exactly 6 such lines measured (e.g. p.525 `馬  Mã  c. N`): the print lets the definition
/// start on the headword line, the `N` beginning "Ngựa". Requiring the label at the end loses 6 real entries.
///
/// The "starts with a glyph" condition keeps sub-entries out — a sub-entry opens with an
/// italic form. Verified across all 89,690 lines: this rule admits no other line.
pub fn starts_with_glyph_and_has_label(text: &str) -> bool {
    let t = text.trim_start();
    let Some(first) = t.chars().next() else {
        return false;
    };
    if !crate::headword::is_glyph_char(first) {
        return false;
    }
    ["cn.", "c.", "n."].iter().any(|label| {
        t.match_indices(label).any(|(at, _)| {
            t[..at].ends_with(char::is_whitespace)
                && t[at + label.len()..]
                    .chars()
                    .next()
                    .is_none_or(char::is_whitespace)
        })
    })
}
