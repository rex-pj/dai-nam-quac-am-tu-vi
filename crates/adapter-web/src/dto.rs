//! The public JSON contract, version **v1**.
//!
//! Keeping it apart from the domain model is what makes it possible to refactor the domain
//! **without breaking other people's contract**. If a handler returned `core::EntrySummary`
//! directly, renaming a domain field would silently change the public API, and API users
//! would find out by breaking.
//!
//! v1 conventions:
//! * Field names are English `snake_case` — the API is machine-read, and researchers abroad
//!   use this dictionary too.
//! * `form` and `form_expanded` always travel together: one is **the words of the book**,
//!   the other is **what we derived**. The API must not let users confuse them.
//! * Every item carries `pdf_page` and `printed_page` — provenance is part of the data.
use dnqatv_core::model::{EntryDetail, EntrySummary, SubEntry};
use dnqatv_core::port::{DataStats, PageView};
use dnqatv_core::search::{RankTier, ScoredEntry};
use serde::Serialize;
use utoipa::ToSchema;

/// The contract version. Changing the shape means bumping this and keeping the old path alive.
pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GlyphDto {
    /// The Han-Nom character; absent when the print uses an image.
    pub char: Option<String>,
    /// `bmp` · `ext_b` · `pua` · `image_only`
    pub kind: String,
    pub codepoint: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EntryDto {
    pub slug: String,
    /// The reading verbatim, **tone marks kept** — Ả, Á and À are three different entries.
    pub reading: String,
    /// The Sino-Vietnamese reading in parentheses, when the print records one.
    pub alternate: Option<String>,
    pub glyph: Option<GlyphDto>,
    /// The labels as printed, e.g. `["c.", "n."]`. Order preserved.
    pub pos: Vec<String>,
    pub gloss: String,
    pub pdf_page: u16,
    pub printed_page: Option<u16>,
    /// The print has something questionable on this entry line, awaiting a human check.
    pub needs_review: bool,
}

impl From<&EntrySummary> for EntryDto {
    fn from(e: &EntrySummary) -> Self {
        Self {
            slug: e.slug.as_str().to_owned(),
            reading: e.reading.as_str().to_owned(),
            alternate: e.alternate.as_ref().map(|a| a.as_str().to_owned()),
            glyph: e.glyph.as_ref().map(|g| GlyphDto {
                char: Some(g.ch().to_string()),
                kind: g.kind().db_value().to_owned(),
                codepoint: Some(g.codepoint()),
            }),
            pos: e
                .pos
                .as_slice()
                .iter()
                .map(|p| p.book_label().to_owned())
                .collect(),
            gloss: e.gloss.clone(),
            pdf_page: e.pdf_page.get(),
            printed_page: e.pdf_page.printed().map(|p| p.get()),
            needs_review: e.needs_review,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SubEntryDto {
    /// **The print verbatim**, placeholder intact, e.g. `― gươm`.
    pub form: String,
    /// **Derived**: the placeholder replaced by the reading, e.g. `Lõm gươm`.
    pub form_expanded: String,
    /// The Han part verbatim, `|` intact.
    pub han_form: Option<String>,
    /// Derived: `|` replaced by the entry glyph.
    pub han_expanded: Option<String>,
    pub definition: String,
    pub needs_review: bool,
}

impl From<&SubEntry> for SubEntryDto {
    fn from(s: &SubEntry) -> Self {
        Self {
            form: s.form.clone(),
            form_expanded: s.form_expanded.clone(),
            han_form: s.han_form.clone(),
            han_expanded: s.han_expanded.clone(),
            definition: s.definition.clone(),
            needs_review: s.needs_review,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EntryDetailDto {
    #[serde(flatten)]
    pub entry: EntryDto,
    pub sub_entries: Vec<SubEntryDto>,
    /// The entry reuses the preceding entry glyph.
    pub inherits_glyph: bool,
}

impl From<&EntryDetail> for EntryDetailDto {
    fn from(e: &EntryDetail) -> Self {
        Self {
            entry: EntryDto::from(&e.summary),
            sub_entries: e.sub_entries.iter().map(SubEntryDto::from).collect(),
            inherits_glyph: e.inherits_glyph,
        }
    }
}

/// A result together with **the reason it appeared**.
///
/// `tier` is exposed rather than hidden: for a scholarly tool, knowing why a result is
/// present matters as much as knowing that it is.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScoredEntryDto {
    #[serde(flatten)]
    pub entry: EntryDto,
    /// `trung-khit` · `trung-tu-dang` · `bat-dau-bang` · `bo-dau` · `trong-loi-giai-nghia` · `gan-giong`
    pub tier: String,
    pub tier_label: String,
}

impl From<&ScoredEntry> for ScoredEntryDto {
    fn from(s: &ScoredEntry) -> Self {
        Self {
            entry: EntryDto::from(&s.entry),
            tier: s.tier.as_param().to_owned(),
            tier_label: s.tier.group_label().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SuggestionDto {
    pub text: String,
    pub reason: String,
    /// Whether a human has checked it against the print.
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchResponseDto {
    /// The query **exactly as the user typed it**. The API never returns a corrected query.
    pub query: String,
    pub mode: String,
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
    pub results: Vec<ScoredEntryDto>,
    /// The 1895 ↔ modern orthography suggestions. **Suggestions only**; the query is not rewritten.
    pub orthography: Vec<SuggestionDto>,
    /// Near-match suggestions, present only when nothing was found.
    pub did_you_mean: Vec<EntryDto>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PageDto {
    pub pdf_page: u16,
    pub printed_page: Option<u16>,
    pub letter: Option<String>,
    /// The first entry on the page, as the running head records it.
    pub head_first: Option<String>,
    pub head_last: Option<String>,
    pub entries: Vec<EntryDto>,
}

impl PageDto {
    pub fn new(view: &PageView, entries: &[EntrySummary]) -> Self {
        Self {
            pdf_page: view.pdf_page.get(),
            printed_page: view.printed_page.map(|p| p.get()),
            letter: view.letter.map(|l| l.label().to_owned()),
            head_first: view.head_first.clone(),
            head_last: view.head_last.clone(),
            entries: entries.iter().map(EntryDto::from).collect(),
        }
    }
}

/// Data-quality figures — **publishing where the data is still weak**.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StatsDto {
    pub entries: u64,
    pub sub_entries: u64,
    pub glyphs: u64,
    pub pages: u64,
    pub entries_needing_review: u64,
    pub sub_entries_needing_review: u64,
    /// Entries whose glyph the print shows as an image because it has no Unicode code point.
    pub image_only_glyphs: u64,
}

impl From<&DataStats> for StatsDto {
    fn from(s: &DataStats) -> Self {
        Self {
            entries: s.entries,
            sub_entries: s.sub_entries,
            glyphs: s.glyphs,
            pages: s.pages,
            entries_needing_review: s.entries_needing_review,
            sub_entries_needing_review: s.sub_entries_needing_review,
            image_only_glyphs: s.image_only_glyphs,
        }
    }
}

/// The list of rank tiers, so API users know what values `tier` can take.
pub fn rank_tiers() -> Vec<SuggestionDto> {
    RankTier::ALL
        .iter()
        .map(|t| SuggestionDto {
            text: t.as_param().to_owned(),
            reason: t.group_label().to_owned(),
            verified: true,
        })
        .collect()
}
