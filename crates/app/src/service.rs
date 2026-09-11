//! The services — each answers exactly one question the UI asks.

use std::sync::Arc;

use dnqatv_core::model::{EntryDetail, EntrySummary, GlyphChar, Letter, PdfPage, Slug};
use dnqatv_core::port::{
    DataStats, EntryReader, EntrySearchPort, FrontMatter, FrontMatterReader, GlyphReader,
    PageReader, PageView, StatsReader,
};
use dnqatv_core::search::{
    Paged, Pagination, RankTier, ScoredEntry, SearchMode, SearchQuery, group_by_tier,
};

use crate::AppError;
use crate::bridge::{Bridge, Suggestion};

/// Maximum number of "near match" suggestions. More than this and a reader cannot scan the
/// list, and a long list of near-misses looks like the system is guessing.
const MAX_SUGGESTIONS: u64 = 8;

/// A search result, grouped and carrying suggestions.
pub struct SearchOutcome {
    pub query: SearchQuery,
    /// Results grouped by **why they matched** — matching the §6.2 screen layout.
    pub groups: Vec<(RankTier, Vec<ScoredEntry>)>,
    pub total: u64,
    pub page: Pagination,
    /// The 1895 ↔ modern orthography suggestions. **Never** a replacement for the query.
    pub orthography: Vec<Suggestion>,
    /// The "near match" suggestions, shown only when nothing was found.
    pub did_you_mean: Vec<EntrySummary>,
}

impl SearchOutcome {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// One entry page, with everything the screen needs.
pub struct EntryPage {
    pub entry: EntryDetail,
    pub previous: Option<EntrySummary>,
    pub next: Option<EntrySummary>,
    pub page: Option<PageView>,
}

/// The main service: search and view entries.
#[derive(Clone)]
pub struct DictionaryService {
    entries: Arc<dyn EntryReader>,
    search: Arc<dyn EntrySearchPort>,
    pages: Arc<dyn PageReader>,
    bridge: Arc<Bridge>,
}

impl DictionaryService {
    pub fn new(
        entries: Arc<dyn EntryReader>,
        search: Arc<dyn EntrySearchPort>,
        pages: Arc<dyn PageReader>,
        bridge: Arc<Bridge>,
    ) -> Self {
        Self {
            entries,
            search,
            pages,
            bridge,
        }
    }

    pub async fn lookup(&self, slug: &Slug) -> Result<Option<EntryPage>, AppError> {
        let Some(entry) = self.entries.by_slug(slug).await? else {
            return Ok(None);
        };
        let (previous, next) = self.entries.neighbours(slug).await?;
        let page = self.pages.by_pdf_page(entry.summary.pdf_page).await?;
        Ok(Some(EntryPage {
            entry,
            previous,
            next,
            page,
        }))
    }

    /// Search.
    ///
    /// The order of steps is deliberate: orthography suggestions are fetched **always**, even
    /// when there are results. The original plan was to suggest only when nothing matched;
    /// measuring against real data showed that is broken — `nhân` has 14 entries *and* `nhơn`
    /// has 13 different ones, so someone searching `nhân` still needs to know `nhơn` exists.
    /// Suggesting only when empty would hide exactly half the dictionary.
    pub async fn search(
        &self,
        raw: &str,
        mode: SearchMode,
        page: Pagination,
    ) -> Result<SearchOutcome, AppError> {
        let Some(query) = SearchQuery::parse(raw, mode, page) else {
            return Err(AppError::BadQuery);
        };
        let found: Paged<ScoredEntry> = self.search.search(&query).await?;
        let orthography = self.bridge.suggest(query.text());

        let did_you_mean = if found.items.is_empty() {
            self.search.suggest(&query, MAX_SUGGESTIONS).await?
        } else {
            Vec::new()
        };

        Ok(SearchOutcome {
            groups: group_by_tier(found.items),
            total: found.total,
            page: found.page,
            query,
            orthography,
            did_you_mean,
        })
    }

    /// Entry of the day — chosen **deterministically** from the date.
    ///
    /// Deterministic, not random: on a given day everyone sees the same entry, so sharing a
    /// link means something and the response can be cached. `day` is a day count from any epoch.
    pub async fn entry_of_the_day(&self, day: u64) -> Result<Option<EntrySummary>, AppError> {
        let total = self.entries.count().await?;
        if total == 0 {
            return Ok(None);
        }
        Ok(self.entries.nth(day % total).await?)
    }
}

/// Browsing by letter, by printed page, by glyph.
#[derive(Clone)]
pub struct BrowseService {
    entries: Arc<dyn EntryReader>,
    glyphs: Arc<dyn GlyphReader>,
    pages: Arc<dyn PageReader>,
    front: Arc<dyn FrontMatterReader>,
    stats: Arc<dyn StatsReader>,
}

impl BrowseService {
    pub fn new(
        entries: Arc<dyn EntryReader>,
        glyphs: Arc<dyn GlyphReader>,
        pages: Arc<dyn PageReader>,
        front: Arc<dyn FrontMatterReader>,
        stats: Arc<dyn StatsReader>,
    ) -> Self {
        Self {
            entries,
            glyphs,
            pages,
            front,
            stats,
        }
    }

    pub async fn by_letter(
        &self,
        letter: Letter,
        page: Pagination,
    ) -> Result<Paged<EntrySummary>, AppError> {
        Ok(self.entries.by_letter(letter, page).await?)
    }

    /// The comparison screen: the page image beside the entries printed on that page.
    pub async fn page_view(
        &self,
        pdf_page: PdfPage,
    ) -> Result<Option<(PageView, Vec<EntrySummary>)>, AppError> {
        let Some(view) = self.pages.by_pdf_page(pdf_page).await? else {
            return Ok(None);
        };
        let entries = self.entries.by_pdf_page(pdf_page).await?;
        Ok(Some((view, entries)))
    }

    /// One glyph and all its readings — mirroring the structure of the entry index.
    pub async fn glyph_view(&self, glyph: &GlyphChar) -> Result<Vec<EntrySummary>, AppError> {
        Ok(self.glyphs.entries_for(glyph).await?)
    }

    pub async fn front_matter(&self, slug: &Slug) -> Result<Option<FrontMatter>, AppError> {
        Ok(self.front.by_slug(slug).await?)
    }

    pub async fn front_matter_all(&self) -> Result<Vec<FrontMatter>, AppError> {
        Ok(self.front.all().await?)
    }

    pub async fn stats(&self) -> Result<DataStats, AppError> {
        Ok(self.stats.stats().await?)
    }
}

impl DictionaryService {
    /// Entries in **book order**, fetched in batches.
    ///
    /// Used for the sitemap and for sequential browsing. Kept apart from `entry_of_the_day`
    /// because the two only coincidentally share a query; merged, the function name would lie
    /// at one of its two call sites.
    pub async fn entries_in_order(&self, page: Pagination) -> Result<Vec<EntrySummary>, AppError> {
        let mut out = Vec::new();
        for i in 0..page.limit() {
            match self.entries.nth(page.offset() + i).await? {
                Some(e) => out.push(e),
                None => break,
            }
        }
        Ok(out)
    }
}
