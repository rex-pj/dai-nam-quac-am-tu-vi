//! An in-memory fake repository implementing **exactly the ports** `core` declares.
//!
//! The purpose is not "fast tests" but the **L** of SOLID made runnable: the same set of
//! assertions runs against this repo and against real PostgreSQL, and both must give the
//! same answer. Wherever they differ, one of the two implementations misreads the contract.
//!
//! This repo does **not** simulate the Postgres full-text ranking — it cannot, and
//! pretending it does would be self-deception. It implements the part of the contract
//! expressible without a search engine: lookup by slug, by letter, by page, neighbours,
//! counts. Ranking can only be checked against a real database, and the conformance suite says so.

use std::collections::BTreeMap;

use async_trait::async_trait;
use dnqatv_core::model::{
    EntryDetail, EntryId, EntrySummary, GlyphChar, GlyphId, Letter, PdfPage, Pos, PosSet, Reading,
    Slug, SlugMinter, SubEntry, SubEntryId,
};
use dnqatv_core::port::{
    DataStats, EntryReader, FrontMatter, FrontMatterReader, GlyphReader, PageReader, PageView,
    RepoError, StatsReader,
};
use dnqatv_core::search::{Paged, Pagination};

/// A compact description of one entry, for building test data.
pub struct EntrySeed {
    pub reading: &'static str,
    pub glyph: Option<&'static str>,
    pub pos: &'static [Pos],
    pub gloss: &'static str,
    pub pdf_page: u16,
    /// The sub-entries: `(verbatim, derived, definition)`.
    pub subs: &'static [(&'static str, &'static str, &'static str)],
}

#[derive(Default)]
pub struct FakeRepository {
    entries: Vec<EntryDetail>,
    by_slug: BTreeMap<String, usize>,
    pages: BTreeMap<u16, PageView>,
    front: Vec<FrontMatter>,
}

impl FakeRepository {
    /// Build from a list of descriptions, **in book order**.
    ///
    /// Slugs are minted with the same [`SlugMinter`] `import` uses, so the duplicate-slug
    /// rule is exercised here too — there is no second copy of that rule to drift.
    pub fn from_seeds(seeds: &[EntrySeed]) -> Result<Self, RepoError> {
        let mut minter = SlugMinter::new();
        let mut entries = Vec::new();
        let mut by_slug = BTreeMap::new();
        let mut pages: BTreeMap<u16, PageView> = BTreeMap::new();
        let mut sub_id = 0i32;

        for (i, s) in seeds.iter().enumerate() {
            let reading = Reading::parse(s.reading)?;
            let slug = minter.mint(&reading)?;
            let pdf_page = PdfPage::new(s.pdf_page)?;
            let letter = reading.letter()?;

            pages.entry(s.pdf_page).or_insert(PageView {
                pdf_page,
                printed_page: pdf_page.printed(),
                letter: Some(letter),
                image_path: None,
                head_first: None,
                head_last: None,
            });

            let summary = EntrySummary {
                id: EntryId::new(i as i32 + 1),
                slug: slug.clone(),
                glyph: match s.glyph {
                    Some(g) => Some(GlyphChar::parse(g)?),
                    None => None,
                },
                reading,
                alternate: None,
                pos: PosSet::new(s.pos.to_vec())?,
                gloss: s.gloss.to_owned(),
                pdf_page,
                needs_review: false,
            };

            let sub_entries = s
                .subs
                .iter()
                .enumerate()
                .map(|(j, (form, expanded, def))| {
                    sub_id += 1;
                    SubEntry {
                        id: SubEntryId::new(sub_id),
                        seq: j as u32 + 1,
                        han_form: None,
                        han_expanded: None,
                        form: (*form).to_owned(),
                        form_expanded: (*expanded).to_owned(),
                        definition: (*def).to_owned(),
                        needs_review: false,
                    }
                })
                .collect();

            by_slug.insert(slug.as_str().to_owned(), entries.len());
            entries.push(EntryDetail {
                summary,
                sub_entries,
                inherits_glyph: false,
                // The fixture builds entries that all have a glyph, so none of them is one of
                // the 29 that need a shape described.
                shape_note: None,
                seq: i as u32 + 1,
            });
        }

        Ok(Self {
            entries,
            by_slug,
            pages,
            front: Vec::new(),
        })
    }

    pub fn with_front_matter(mut self, front: Vec<FrontMatter>) -> Self {
        self.front = front;
        self
    }
}

#[async_trait]
impl EntryReader for FakeRepository {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<EntryDetail>, RepoError> {
        Ok(self
            .by_slug
            .get(slug.as_str())
            .and_then(|i| self.entries.get(*i))
            .cloned())
    }

    async fn neighbours(
        &self,
        slug: &Slug,
    ) -> Result<(Option<EntrySummary>, Option<EntrySummary>), RepoError> {
        let Some(&i) = self.by_slug.get(slug.as_str()) else {
            return Ok((None, None));
        };
        let previous = i
            .checked_sub(1)
            .and_then(|j| self.entries.get(j))
            .map(|e| e.summary.clone());
        let next = self.entries.get(i + 1).map(|e| e.summary.clone());
        Ok((previous, next))
    }

    async fn by_letter(
        &self,
        letter: Letter,
        page: Pagination,
    ) -> Result<Paged<EntrySummary>, RepoError> {
        let mut all = Vec::new();
        for e in &self.entries {
            if e.summary.reading.letter()? == letter {
                all.push(e.summary.clone());
            }
        }
        let total = all.len() as u64;
        let items = all
            .into_iter()
            .skip(page.offset() as usize)
            .take(page.limit() as usize)
            .collect();
        Ok(Paged { items, total, page })
    }

    async fn by_pdf_page(&self, page: PdfPage) -> Result<Vec<EntrySummary>, RepoError> {
        Ok(self
            .entries
            .iter()
            .filter(|e| e.summary.pdf_page == page)
            .map(|e| e.summary.clone())
            .collect())
    }

    async fn nth(&self, index: u64) -> Result<Option<EntrySummary>, RepoError> {
        Ok(self.entries.get(index as usize).map(|e| e.summary.clone()))
    }

    async fn count(&self) -> Result<u64, RepoError> {
        Ok(self.entries.len() as u64)
    }
}

#[async_trait]
impl PageReader for FakeRepository {
    async fn by_pdf_page(&self, page: PdfPage) -> Result<Option<PageView>, RepoError> {
        Ok(self.pages.get(&page.get()).cloned())
    }
}

#[async_trait]
impl GlyphReader for FakeRepository {
    async fn entries_for(&self, glyph: &GlyphChar) -> Result<Vec<EntrySummary>, RepoError> {
        Ok(self
            .entries
            .iter()
            .filter(|e| e.summary.glyph.as_ref() == Some(glyph))
            .map(|e| e.summary.clone())
            .collect())
    }

    async fn image_path(&self, _glyph_id: GlyphId) -> Result<Option<String>, RepoError> {
        Ok(None)
    }

    async fn count(&self) -> Result<u64, RepoError> {
        let mut chars: Vec<&GlyphChar> = self
            .entries
            .iter()
            .filter_map(|e| e.summary.glyph.as_ref())
            .collect();
        chars.sort();
        chars.dedup();
        Ok(chars.len() as u64)
    }
}

#[async_trait]
impl FrontMatterReader for FakeRepository {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<FrontMatter>, RepoError> {
        Ok(self.front.iter().find(|f| f.slug == *slug).cloned())
    }

    async fn all(&self) -> Result<Vec<FrontMatter>, RepoError> {
        Ok(self.front.clone())
    }
}

#[async_trait]
impl StatsReader for FakeRepository {
    async fn stats(&self) -> Result<DataStats, RepoError> {
        Ok(DataStats {
            entries: self.entries.len() as u64,
            sub_entries: self
                .entries
                .iter()
                .map(|e| e.sub_entries.len() as u64)
                .sum(),
            glyphs: GlyphReader::count(self).await?,
            pages: self.pages.len() as u64,
            entries_needing_review: self
                .entries
                .iter()
                .filter(|e| e.summary.needs_review)
                .count() as u64,
            sub_entries_needing_review: self
                .entries
                .iter()
                .flat_map(|e| &e.sub_entries)
                .filter(|s| s.needs_review)
                .count() as u64,
            image_only_glyphs: self
                .entries
                .iter()
                .filter(|e| e.summary.glyph.is_none())
                .count() as u64,
        })
    }
}

/// A small data set taken from pages read by hand, for tests.
///
/// The entries on page 500 were chosen because it is one of the manual verification
/// landmarks: the order Lõm → Lôm → Lốm is the real printed order, so it also checks sorting.
pub fn page_500() -> Vec<EntrySeed> {
    vec![
        EntrySeed {
            reading: "Lõm",
            glyph: Some("𨰲"),
            pos: &[Pos::ChuNom],
            gloss: "Ruột, trúc mứt, cái ở chính giữa, cái cốt.",
            pdf_page: 500,
            subs: &[
                ("― gươm", "Lõm gươm", ". Nạm gươm."),
                ("― súng", "Lõm súng", ". Lòng súng."),
                ("Chính giữa ―", "Chính giữa Lõm", ". Ở ngảy giữa ruột."),
            ],
        },
        EntrySeed {
            reading: "Lôm",
            glyph: Some("𨇣"),
            pos: &[Pos::ChuNom],
            gloss: "Bộ không vững vàng.",
            pdf_page: 500,
            subs: &[("― chôm", "Lôm chôm", ". Bộ không vững vàng.")],
        },
        EntrySeed {
            reading: "Lốm",
            glyph: Some("𤑸"),
            pos: &[Pos::ChuNom],
            gloss: "Có nhiều đúm, nhiều về.",
            pdf_page: 500,
            subs: &[("― đốm", "Lốm đốm", ". Có nhiều đúm.")],
        },
    ]
}

/// The first entry of the book, page 11 — the second manual verification landmark.
pub fn page_11() -> Vec<EntrySeed> {
    vec![EntrySeed {
        reading: "A",
        glyph: Some("阿"),
        pos: &[Pos::ChuNho],
        gloss: "Đèo, nương dựa, phụ theo.",
        pdf_page: 11,
        subs: &[
            ("― ý", "A ý", ". Dua theo một ý."),
            ("― dua", "A dua", ". Thừa thuận, theo ý."),
            ("Thái ―", "Thái A", ". Gươm báu trong nước."),
        ],
    }]
}
