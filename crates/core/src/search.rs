//! Search policy — it belongs to the **domain, not the adapter**.
//!
//! The split is deliberate: *what beats what* is a dictionary decision, so [`RankTier`] lives
//! here; *how Postgres computes it* is an infrastructure capability, so the SQL expression
//! lives in the adapter. Ranking policy is therefore testable against a fake repo, with no
//! database.
//!
//! [`RankTier`] doubles as the **group heading** on the results screen. One enum, two uses —
//! there is no second set of labels that could drift from the ranking.

use crate::model::entry::EntrySummary;

/// What kind of lookup the user is performing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Inferred from the shape of the query — the default mode.
    Auto,
    /// The query is Han-Nom script.
    HanNom,
    /// The query is a Quốc ngữ reading.
    QuocNgu,
    /// Search the full text of definitions and sub-entries (reverse lookup).
    ToanVan,
}

impl SearchMode {
    pub const ALL: [SearchMode; 4] = [Self::Auto, Self::HanNom, Self::QuocNgu, Self::ToanVan];

    pub const fn as_param(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::HanNom => "han-nom",
            Self::QuocNgu => "quoc-ngu",
            Self::ToanVan => "toan-van",
        }
    }

    /// The label shown on the mode chips. **Vietnamese: this is read by the user, not by a
    /// developer.** Code around it stays English; text on screen stays in the language of the book.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Tùy chữ",
            Self::HanNom => "Chữ Hán-Nôm",
            Self::QuocNgu => "Âm Quốc ngữ",
            Self::ToanVan => "Toàn văn",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_param() == raw.trim())
    }

    /// The mode inferred from the query itself, used when the user leaves
    /// [`SearchMode::Auto`] selected.
    ///
    /// The rule looks **only at character shape**, never at the dictionary: any CJK character
    /// means a script lookup. This is an inference about *user intent*, not about the book
    /// content — so it is allowed to guess, unlike data.
    pub fn resolve(self, query: &str) -> SearchMode {
        match self {
            Self::Auto => {
                if query.chars().any(is_han_nom) {
                    Self::HanNom
                } else {
                    Self::QuocNgu
                }
            }
            other => other,
        }
    }
}

/// Whether a character falls in the Han-Nom ranges this book uses.
pub fn is_han_nom(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF      // CJK Ext A
        | 0x4E00..=0x9FFF    // CJK Unified
        | 0xF900..=0xFAFF    // compatibility forms
        | 0x20000..=0x3FFFF  // Ext B and beyond
        | 0xE000..=0xF8FF    // PUA — used by the Nom Na Tong font
        | 0xF0000..=0x10FFFD // PUA planes 15/16
    )
}

/// Why a result showed up — and, at the same time, its priority order.
///
/// Declaration order **is** ranking order: an earlier variant beats a later one. Tone marks
/// are meaning-bearing (Ả, Á and À are three different entries), so an accented match must
/// always beat an unaccented one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RankTier {
    /// Exact match including tone marks.
    ExactReading,
    /// Han-Nom glyph match.
    ExactGlyph,
    /// The reading starts with the query.
    PrefixReading,
    /// Match after folding diacritics.
    UnaccentedReading,
    /// Match inside a definition or sub-entry — this is the **reverse lookup** path.
    FullText,
    /// Near match, by trigram similarity.
    Fuzzy,
}

impl RankTier {
    pub const ALL: [RankTier; 6] = [
        Self::ExactReading,
        Self::ExactGlyph,
        Self::PrefixReading,
        Self::UnaccentedReading,
        Self::FullText,
        Self::Fuzzy,
    ];

    /// The rank used in the CASE expression of the adapter. Lower is better.
    pub const fn rank(self) -> i32 {
        self as i32
    }

    /// The group heading shown on the results screen. **Vietnamese: it is read by the user.**
    pub const fn group_label(self) -> &'static str {
        match self {
            Self::ExactReading => "Trùng khít",
            Self::ExactGlyph => "Trùng tự dạng",
            Self::PrefixReading => "Bắt đầu bằng",
            Self::UnaccentedReading => "Bỏ dấu",
            Self::FullText => "Trong lời giải nghĩa",
            Self::Fuzzy => "Gần giống",
        }
    }

    pub const fn as_param(self) -> &'static str {
        match self {
            Self::ExactReading => "trung-khit",
            Self::ExactGlyph => "trung-tu-dang",
            Self::PrefixReading => "bat-dau-bang",
            Self::UnaccentedReading => "bo-dau",
            Self::FullText => "trong-loi-giai-nghia",
            Self::Fuzzy => "gan-giong",
        }
    }

    pub fn from_rank(rank: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.rank() == rank)
    }
}

/// Pagination. The limit has a hard ceiling so one query cannot pull down the whole book.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    offset: u64,
    limit: u64,
}

impl Pagination {
    pub const DEFAULT_LIMIT: u64 = 30;
    pub const MAX_LIMIT: u64 = 200;

    pub fn new(offset: u64, limit: u64) -> Self {
        Self {
            offset,
            limit: limit.clamp(1, Self::MAX_LIMIT),
        }
    }

    pub fn first_page() -> Self {
        Self::new(0, Self::DEFAULT_LIMIT)
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }
    pub const fn limit(self) -> u64 {
        self.limit
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self::first_page()
    }
}

/// One page of results plus the total count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: Pagination,
}

impl<T> Paged<T> {
    pub fn empty(page: Pagination) -> Self {
        Self {
            items: Vec::new(),
            total: 0,
            page,
        }
    }

    pub fn has_more(&self) -> bool {
        self.page.offset() + (self.items.len() as u64) < self.total
    }
}

/// A validated search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    text: String,
    pub mode: SearchMode,
    pub page: Pagination,
}

impl SearchQuery {
    /// Maximum length — a guard against junk queries, not against real users.
    pub const MAX_LEN: usize = 200;

    pub fn parse(raw: &str, mode: SearchMode, page: Pagination) -> Option<Self> {
        let text = crate::text::nfc(raw.trim());
        if text.is_empty() || text.chars().count() > Self::MAX_LEN {
            return None;
        }
        let mode = mode.resolve(&text);
        Some(Self { text, mode, page })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The accent-folded form, for the unaccented branch.
    pub fn folded(&self) -> String {
        crate::text::fold(&self.text)
    }
}

/// A result together with the reason it appeared.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredEntry {
    pub entry: EntrySummary,
    pub tier: RankTier,
    /// The Postgres score within one tier; used for ordering only, never displayed.
    pub score: f32,
}

/// Group results by tier, preserving rank order — matching the section 6.2 screen layout.
pub fn group_by_tier(items: Vec<ScoredEntry>) -> Vec<(RankTier, Vec<ScoredEntry>)> {
    let mut out: Vec<(RankTier, Vec<ScoredEntry>)> = Vec::new();
    for tier in RankTier::ALL {
        let group: Vec<ScoredEntry> = items.iter().filter(|s| s.tier == tier).cloned().collect();
        if !group.is_empty() {
            out.push((tier, group));
        }
    }
    out
}
