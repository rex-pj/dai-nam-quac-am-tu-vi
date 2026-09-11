//! Outbound ports — they speak the dictionary's language, not SQL.
//!
//! Split fine-grained after the **I** in SOLID: a handler that only renders a printed page
//! depends on [`PageReader`], not on a god repository. Fake repos in tests then only have to
//! implement the part they actually use.
//!
//! There is no `save`, `insert` or `delete` here. This web app is **read-only**: data enters
//! the database through exactly one door, `pipeline/import`, and that door refuses to load
//! while any gate is red. Having no write path from the web layer is an integrity guarantee,
//! not an omission.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::model::entry::{EntryDetail, EntrySummary};
use crate::model::glyph::GlyphChar;
use crate::model::letter::Letter;
use crate::model::page::PdfPage;
use crate::model::slug::Slug;
use crate::search::{Paged, Pagination, ScoredEntry, SearchQuery};

/// A storage-layer failure as the domain sees it.
///
/// Deliberately does **not** leak SeaORM or sqlx error types: leaking them would force `app`
/// to know about them, and the boundary would be gone. The adapter translates its own errors
/// into these.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("stored data violates a domain invariant: {0}")]
    Corrupt(#[from] DomainError),

    #[error("data store unreachable: {0}")]
    Unavailable(String),
}

/// Reading entries.
#[async_trait]
pub trait EntryReader: Send + Sync {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<EntryDetail>, RepoError>;

    /// The immediately preceding and following entries in book order — for the
    /// "‹ previous · next ›" navigation.
    async fn neighbours(
        &self,
        slug: &Slug,
    ) -> Result<(Option<EntrySummary>, Option<EntrySummary>), RepoError>;

    /// Browse by letter section.
    async fn by_letter(
        &self,
        letter: Letter,
        page: Pagination,
    ) -> Result<Paged<EntrySummary>, RepoError>;

    /// The entries on one printed page — the side-by-side comparison screen.
    async fn by_pdf_page(&self, page: PdfPage) -> Result<Vec<EntrySummary>, RepoError>;

    /// Entry of the day: chosen deterministically from the date, never randomly.
    ///
    /// Determinism is deliberate — everyone sees the same entry on a given day, sharing a
    /// link means something, and the response can be cached.
    async fn nth(&self, index: u64) -> Result<Option<EntrySummary>, RepoError>;

    async fn count(&self) -> Result<u64, RepoError>;
}

/// Searching.
#[async_trait]
pub trait EntrySearchPort: Send + Sync {
    async fn search(&self, query: &SearchQuery) -> Result<Paged<ScoredEntry>, RepoError>;

    /// Suggestions when nothing matched — based on trigram similarity.
    ///
    /// **Suggestions only.** The user's query is never rewritten: 1895 spelling and today's
    /// spelling are often two different entries, and merging them destroys meaning.
    async fn suggest(
        &self,
        query: &SearchQuery,
        limit: u64,
    ) -> Result<Vec<EntrySummary>, RepoError>;
}

/// Lookup by glyph.
#[async_trait]
pub trait GlyphReader: Send + Sync {
    /// Every entry that uses this glyph — mirroring the structure of the entry table.
    async fn entries_for(&self, glyph: &GlyphChar) -> Result<Vec<EntrySummary>, RepoError>;

    /// Image path for a glyph that has no Unicode code point.
    async fn image_path(
        &self,
        glyph_id: crate::model::ids::GlyphId,
    ) -> Result<Option<String>, RepoError>;

    async fn count(&self) -> Result<u64, RepoError>;
}

/// A printed page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageView {
    pub pdf_page: PdfPage,
    pub printed_page: Option<crate::model::page::PrintedPage>,
    pub letter: Option<Letter>,
    pub image_path: Option<String>,
    /// The two running-head entries at the top of the page, as printed.
    pub head_first: Option<String>,
    pub head_last: Option<String>,
}

#[async_trait]
pub trait PageReader: Send + Sync {
    async fn by_pdf_page(&self, page: PdfPage) -> Result<Option<PageView>, RepoError>;
}

/// The book's front matter: TIỂU TỰ, DẤU RIÊNG, PRÉFACE, LỜI DẶN, LƯU Ý.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatter {
    pub slug: Slug,
    pub title: String,
    pub body: String,
    pub pdf_page: PdfPage,
}

#[async_trait]
pub trait FrontMatterReader: Send + Sync {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<FrontMatter>, RepoError>;
    async fn all(&self) -> Result<Vec<FrontMatter>, RepoError>;
}

/// Figures for the data-quality page.
///
/// That page publishes where the data is still weak. For a dictionary, **being open about
/// defects is a feature**; hiding them is what would be shameful.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataStats {
    pub entries: u64,
    pub sub_entries: u64,
    pub glyphs: u64,
    pub pages: u64,
    pub entries_needing_review: u64,
    pub sub_entries_needing_review: u64,
    pub image_only_glyphs: u64,
}

#[async_trait]
pub trait StatsReader: Send + Sync {
    async fn stats(&self) -> Result<DataStats, RepoError>;
}
