//! Entries and sub-entries — the domain's aggregates.
//!
//! Part I's `Verbatim` / `Derived` boundary shows up here as **two separate fields**, not one
//! field plus a flag: `form` is the book's words, `form_expanded` is what we derived. The UI
//! picks which one to show, but the two can never be mistaken for each other.

use crate::error::DomainError;
use crate::model::glyph::GlyphChar;
use crate::model::ids::{EntryId, SubEntryId};
use crate::model::page::PdfPage;
use crate::model::pos::Pos;
use crate::model::reading::Reading;
use crate::model::slug::Slug;

/// An entry's part-of-speech labels — **non-empty, in printed order**.
///
/// Why a list: 548 entries carry two labels (`c. n.`). Why ordered: the book prints both
/// `c. n.` (543 entries) and `n. c.` (3 entries); collapsing them is interpretation, and
/// interpretation belongs to the reader, not to the data model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosSet(Vec<Pos>);

impl PosSet {
    pub fn new(labels: Vec<Pos>) -> Result<Self, DomainError> {
        if labels.is_empty() {
            return Err(DomainError::NoPosLabel);
        }
        Ok(Self(labels))
    }

    pub fn as_slice(&self) -> &[Pos] {
        &self.0
    }

    /// The labels written exactly as printed, e.g. `"c. n."`.
    pub fn book_label(&self) -> String {
        self.0
            .iter()
            .map(|p| p.book_label())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The spelled-out names for display, e.g. `"chữ nho · chữ nôm"`.
    pub fn display_name(&self) -> String {
        self.0
            .iter()
            .map(|p| p.display_name())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// A sub-entry: a form plus its definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubEntry {
    pub id: SubEntryId,
    pub seq: u32,
    /// The Han part verbatim; may contain `|` standing for the entry's glyph.
    pub han_form: Option<String>,
    /// Derived: `|` replaced by the glyph.
    pub han_expanded: Option<String>,
    /// The form **verbatim**, e.g. `― gươm`. This is what the UI shows by default.
    pub form: String,
    /// **Derived**: the placeholder replaced by the reading, e.g. `Lõm gươm`.
    pub form_expanded: String,
    pub definition: String,
    pub needs_review: bool,
}

/// The head of an entry — enough to render one search result row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: EntryId,
    pub slug: Slug,
    /// `None` when the print uses an image for the glyph — 29 entries are like this.
    pub glyph: Option<GlyphChar>,
    pub reading: Reading,
    /// The Sino-Vietnamese reading in parentheses; 43 entries have one.
    pub alternate: Option<Reading>,
    pub pos: PosSet,
    pub gloss: String,
    pub pdf_page: PdfPage,
    pub needs_review: bool,
}

impl EntrySummary {
    /// Whether this entry's glyph can be rendered with a font.
    pub fn has_renderable_glyph(&self) -> bool {
        self.glyph.as_ref().is_some_and(|g| g.kind().has_unicode())
    }
}

/// A full entry with its sub-entries — what the entry page needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDetail {
    pub summary: EntrySummary,
    pub sub_entries: Vec<SubEntry>,
    /// How the shape of a glyph with no code point was described, for the 29 entries where
    /// the print sets an image. `None` until a person has signed for it.
    pub shape_note: Option<String>,
    /// The entry reuses the preceding entry's glyph; 55 entries do.
    pub inherits_glyph: bool,
    pub seq: u32,
}

impl EntryDetail {
    pub fn sub_entry_count(&self) -> usize {
        self.sub_entries.len()
    }
}
