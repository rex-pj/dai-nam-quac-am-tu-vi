//! SeaORM implementations of the `core` ports.
//!
//! One type implementing several traits is deliberate, and does **not** contradict the I in
//! SOLID: what gets split up is the **port** (what a call site depends on), not the number
//! of structs. A handler that only renders a printed page still sees `Arc<dyn PageReader>`
//! and cannot reach a search function.
use async_trait::async_trait;
use dnqatv_core::model::{EntryDetail, EntrySummary, GlyphChar, GlyphId, Letter, PdfPage, Slug};
use dnqatv_core::port::{
    DataStats, EntryReader, EntrySearchPort, FrontMatter, FrontMatterReader, GlyphReader,
    PageReader, PageView, RepoError, StatsReader,
};
use dnqatv_core::search::{Paged, Pagination, RankTier, ScoredEntry, SearchMode, SearchQuery};
use dnqatv_entity::{entry, front_matter, glyph, page, sub_entry};
use sea_orm::sea_query::{
    CaseStatement, Expr, ExprTrait, IntoCondition, extension::postgres::PgBinOper,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, FromQueryResult, JoinType, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, RelationTrait,
};

use crate::connect::Db;
use crate::mapping::{self, EntryRow};
use crate::sql;

/// The similarity threshold above which an entry counts as a "near match".
///
/// 0.3 is the `pg_trgm` default. Kept as-is rather than invented: it is a widely used
/// threshold, whereas a number we made up would have nothing to justify it.
const FUZZY_THRESHOLD: f64 = 0.3;

#[derive(Clone, Debug)]
pub struct PgRepository {
    db: Db,
}

impl PgRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn err(&self, e: sea_orm::DbErr) -> RepoError {
        RepoError::Unavailable(crate::error::scrub(e.to_string(), self.db.url()))
    }
}

/// One result row: the `entry` columns, the joined glyph character, plus tier and score.
#[derive(FromQueryResult)]
struct ScoredRow {
    id: i32,
    glyph_id: Option<i32>,
    page_id: i32,
    seq: i32,
    slug: String,
    reading: String,
    reading_norm: String,
    alternate: Option<String>,
    pos: Vec<dnqatv_entity::EntryPos>,
    gloss: String,
    inherits_glyph: bool,
    needs_review: bool,
    glyph_char: Option<String>,
    tier: i32,
    score: f32,
}

impl ScoredRow {
    fn into_row(self) -> (EntryRow, i32, f32) {
        let tier = self.tier;
        let score = self.score;
        (
            EntryRow {
                entry: entry::Model {
                    id: self.id,
                    glyph_id: self.glyph_id,
                    page_id: self.page_id,
                    seq: self.seq,
                    slug: self.slug,
                    reading: self.reading,
                    reading_norm: self.reading_norm,
                    alternate: self.alternate,
                    pos: self.pos,
                    gloss: self.gloss,
                    // `sub_text` is a denormalised search-only column; it is never displayed,
                    // so it is not fetched. Empty here is intended, not lost data.
                    sub_text: String::new(),
                    inherits_glyph: self.inherits_glyph,
                    needs_review: self.needs_review,
                    // Like `sub_text`, not fetched for a result row: a shape description
                    // belongs on the entry page, not in a list. Absent here, not lost.
                    shape_note: None,
                },
                glyph_char: self.glyph_char,
            },
            tier,
            score,
        )
    }
}

/// The `entry` columns needed for one result row, plus the glyph character.
///
/// Listed explicitly rather than `SELECT *`: the `search_doc` column is a `tsvector`,
/// pointless and heavy to fetch, and `sub_text` is never displayed.
fn select_summary<E: EntityTrait>(q: sea_orm::Select<E>) -> sea_orm::Select<E> {
    q.select_only()
        .column(entry::Column::Id)
        .column(entry::Column::GlyphId)
        .column(entry::Column::PageId)
        .column(entry::Column::Seq)
        .column(entry::Column::Slug)
        .column(entry::Column::Reading)
        .column(entry::Column::ReadingNorm)
        .column(entry::Column::Alternate)
        .column(entry::Column::Pos)
        .column(entry::Column::Gloss)
        .column(entry::Column::InheritsGlyph)
        .column(entry::Column::NeedsReview)
        .column_as(
            Expr::col((glyph::Entity, glyph::Column::Char)),
            sql::Extra::GlyphChar.as_str(),
        )
}

fn summary_query() -> sea_orm::Select<entry::Entity> {
    select_summary(entry::Entity::find().join(JoinType::LeftJoin, entry::Relation::Glyph.def()))
}

/// A row without a tier — used by the queries that are not searches.
fn plain(rows: Vec<ScoredRow>) -> Result<Vec<EntrySummary>, RepoError> {
    rows.into_iter()
        .map(|r| EntrySummary::try_from(r.into_row().0))
        .collect()
}

// ── Reading entries ──────────────────────────────────────────────────────────

#[async_trait]
impl EntryReader for PgRepository {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<EntryDetail>, RepoError> {
        let Some(row) = summary_query()
            .filter(entry::Column::Slug.eq(slug.as_str()))
            .into_model::<ScoredRowNoScore>()
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?
        else {
            return Ok(None);
        };
        let (row, _, _) = row.0.into_row();
        let subs = sub_entry::Entity::find()
            .filter(sub_entry::Column::EntryId.eq(row.entry.id))
            .order_by_asc(sub_entry::Column::Seq)
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        Ok(Some(mapping::to_detail(row, subs)?))
    }

    async fn neighbours(
        &self,
        slug: &Slug,
    ) -> Result<(Option<EntrySummary>, Option<EntrySummary>), RepoError> {
        let Some(current) = entry::Entity::find()
            .filter(entry::Column::Slug.eq(slug.as_str()))
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?
        else {
            return Ok((None, None));
        };

        let previous = summary_query()
            .filter(entry::Column::Seq.lt(current.seq))
            .order_by_desc(entry::Column::Seq)
            .into_model::<ScoredRowNoScore>()
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        let next = summary_query()
            .filter(entry::Column::Seq.gt(current.seq))
            .order_by_asc(entry::Column::Seq)
            .into_model::<ScoredRowNoScore>()
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;

        Ok((
            previous
                .map(|r| EntrySummary::try_from(r.0.into_row().0))
                .transpose()?,
            next.map(|r| EntrySummary::try_from(r.0.into_row().0))
                .transpose()?,
        ))
    }

    /// Browse by letter, filtered through the **letter of the PAGE**.
    ///
    /// Filtering indirectly like this is only correct if no page holds entries from two
    /// different letters. That is a **measurement, not an assumption**: checked across all
    /// 1,038 pages with mismatch = 0 — the print starts each letter on a fresh page. The
    /// invariant is re-asserted in `tests/integration/db/repository.rs`, so a reload that
    /// breaks it turns red instead of silently filing entries under the wrong letter.
    async fn by_letter(
        &self,
        letter: Letter,
        page: Pagination,
    ) -> Result<Paged<EntrySummary>, RepoError> {
        let db_letter: dnqatv_entity::BookLetter = letter.into();
        // Compare with `Column::eq` rather than `Expr::col(..).eq(Expr::val(..))`: the latter
        // binds the enum value as text, and Postgres rejects it outright —
        // `operator does not exist: book_letter = text`. `Column` knows the column type.
        let condition = Condition::all().add(page::Column::Letter.eq(db_letter));

        let base = || {
            entry::Entity::find()
                .join(JoinType::InnerJoin, entry::Relation::Page.def())
                .filter(condition.clone())
        };

        let total = base()
            .count(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;

        let rows = select_summary(base().join(JoinType::LeftJoin, entry::Relation::Glyph.def()))
            .order_by_asc(entry::Column::Seq)
            .offset(page.offset())
            .limit(page.limit())
            .into_model::<ScoredRowNoScore>()
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;

        Ok(Paged {
            items: plain(rows.into_iter().map(|r| r.0).collect())?,
            total,
            page,
        })
    }

    async fn by_pdf_page(&self, pdf_page: PdfPage) -> Result<Vec<EntrySummary>, RepoError> {
        let rows = summary_query()
            .filter(entry::Column::PageId.eq(i32::from(pdf_page.get())))
            .order_by_asc(entry::Column::Seq)
            .into_model::<ScoredRowNoScore>()
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        plain(rows.into_iter().map(|r| r.0).collect())
    }

    async fn nth(&self, index: u64) -> Result<Option<EntrySummary>, RepoError> {
        let row = summary_query()
            .order_by_asc(entry::Column::Seq)
            .offset(index)
            .limit(1)
            .into_model::<ScoredRowNoScore>()
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        row.map(|r| EntrySummary::try_from(r.0.into_row().0))
            .transpose()
    }

    async fn count(&self) -> Result<u64, RepoError> {
        entry::Entity::find()
            .count(self.db.connection())
            .await
            .map_err(|e| self.err(e))
    }
}

/// A wrapper around [`ScoredRow`] for queries that have no `tier`/`score` column.
///
/// SeaORM requires every field of a `FromQueryResult` to be present in the result, so
/// ordinary queries must pad two constant columns. Wrapping it spares call sites that detail.
struct ScoredRowNoScore(ScoredRow);

impl FromQueryResult for ScoredRowNoScore {
    fn from_query_result(res: &sea_orm::QueryResult, pre: &str) -> Result<Self, sea_orm::DbErr> {
        Ok(Self(ScoredRow {
            id: res.try_get(pre, "id")?,
            glyph_id: res.try_get(pre, "glyph_id")?,
            page_id: res.try_get(pre, "page_id")?,
            seq: res.try_get(pre, "seq")?,
            slug: res.try_get(pre, "slug")?,
            reading: res.try_get(pre, "reading")?,
            reading_norm: res.try_get(pre, "reading_norm")?,
            alternate: res.try_get(pre, "alternate")?,
            pos: res.try_get(pre, "pos")?,
            gloss: res.try_get(pre, "gloss")?,
            inherits_glyph: res.try_get(pre, "inherits_glyph")?,
            needs_review: res.try_get(pre, "needs_review")?,
            glyph_char: res.try_get(pre, "glyph_char")?,
            tier: RankTier::ExactReading.rank(),
            score: 0.0,
        }))
    }
}

// ── Search ───────────────────────────────────────────────────────────────────

/// The condition for each tier, in the priority order of [`RankTier`].
///
/// Returns (tier, condition) pairs so that **a single list** builds both the `CASE`
/// expression and the `WHERE` clause. Splitting them in two would eventually let a tier
/// reach `CASE` but not `WHERE`, dropping every result into the last tier.
fn tier_conditions(query: &SearchQuery) -> Vec<(RankTier, sea_orm::sea_query::SimpleExpr)> {
    let text = query.text();
    let folded = query.folded();
    let mut out = Vec::new();

    match query.mode {
        SearchMode::HanNom => {
            out.push((
                RankTier::ExactGlyph,
                Expr::col((glyph::Entity, glyph::Column::Char)).eq(text),
            ));
            // Han characters also appear in the Han part of sub-entries — that is the
            // reverse-lookup-by-character path, and script searchers almost always want it.
            out.push((RankTier::FullText, full_text_condition(text)));
        }
        SearchMode::ToanVan => {
            out.push((RankTier::FullText, full_text_condition(text)));
        }
        SearchMode::Auto | SearchMode::QuocNgu => {
            out.push((RankTier::ExactReading, entry::Column::Reading.eq(text)));
            out.push((
                RankTier::PrefixReading,
                entry::Column::Reading.like(sql::prefix_pattern(text)),
            ));
            out.push((
                RankTier::UnaccentedReading,
                entry::Column::ReadingNorm.eq(folded.clone()),
            ));
            out.push((RankTier::FullText, full_text_condition(text)));
            out.push((
                RankTier::Fuzzy,
                sql::similarity(Expr::col(entry::Column::ReadingNorm), &folded).gt(FUZZY_THRESHOLD),
            ));
        }
    }
    out
}

fn full_text_condition(text: &str) -> sea_orm::sea_query::SimpleExpr {
    Expr::col((sql::EntryCol::Table, sql::EntryCol::SearchDoc))
        .binary(PgBinOper::Matches, sql::websearch(text))
}

fn tier_case(query: &SearchQuery) -> sea_orm::sea_query::SimpleExpr {
    let conditions = tier_conditions(query);
    let mut case = CaseStatement::new();
    for (tier, cond) in &conditions {
        case = case.case(cond.clone().into_condition(), sql::int(tier.rank()));
    }
    // Last arm: the lowest tier in the list. No invented "infinity" value — every row that
    // passes `WHERE` matches at least one condition, so this arm never actually runs; it
    // exists because `CASE` needs a default.
    let last = conditions.last().map_or(RankTier::Fuzzy, |(t, _)| *t);
    case.finally(sql::int(last.rank())).into()
}

fn where_any(query: &SearchQuery) -> Condition {
    tier_conditions(query)
        .into_iter()
        .fold(Condition::any(), |acc, (_, cond)| acc.add(cond))
}

#[async_trait]
impl EntrySearchPort for PgRepository {
    async fn search(&self, query: &SearchQuery) -> Result<Paged<ScoredEntry>, RepoError> {
        let condition = where_any(query);

        let total = entry::Entity::find()
            .join(JoinType::LeftJoin, entry::Relation::Glyph.def())
            .filter(condition.clone())
            .count(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;

        let rows = select_summary(
            entry::Entity::find().join(JoinType::LeftJoin, entry::Relation::Glyph.def()),
        )
        .column_as(tier_case(query), sql::Extra::Tier.as_str())
        .column_as(
            sql::ts_rank(
                Expr::col((sql::EntryCol::Table, sql::EntryCol::SearchDoc)),
                query.text(),
            ),
            sql::Extra::Score.as_str(),
        )
        .filter(condition)
        // Tier first, then score, then book order. That last term is what makes results
        // STABLE: without it, two entries of equal tier and score swap places between loads.
        // Ordering by the EXPRESSION rather than the alias: naming an alias needs
        // `Alias::new`, which the CI grep gate forbids in `crates/` so nobody smuggles in a
        // string column name.
        .order_by(tier_case(query), Order::Asc)
        .order_by(
            sql::ts_rank(
                Expr::col((sql::EntryCol::Table, sql::EntryCol::SearchDoc)),
                query.text(),
            ),
            Order::Desc,
        )
        .order_by_asc(entry::Column::Seq)
        .offset(query.page.offset())
        .limit(query.page.limit())
        .into_model::<ScoredRow>()
        .all(self.db.connection())
        .await
        .map_err(|e| self.err(e))?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let (entry_row, tier, score) = row.into_row();
            items.push(ScoredEntry {
                entry: EntrySummary::try_from(entry_row)?,
                // An unknown tier means the `CASE` expression and `RankTier` have drifted
                // apart — corrupt data, not something to silently fold into a default tier.
                tier: RankTier::from_rank(tier).ok_or_else(|| {
                    RepoError::Unavailable(format!("tier {tier} is not in RankTier"))
                })?,
                score,
            });
        }

        Ok(Paged {
            items,
            total,
            page: query.page,
        })
    }

    async fn suggest(
        &self,
        query: &SearchQuery,
        limit: u64,
    ) -> Result<Vec<EntrySummary>, RepoError> {
        let folded = query.folded();
        let rows = summary_query()
            .filter(
                sql::similarity(Expr::col(entry::Column::ReadingNorm), &folded).gt(FUZZY_THRESHOLD),
            )
            .order_by(
                sql::similarity(Expr::col(entry::Column::ReadingNorm), &folded),
                Order::Desc,
            )
            .order_by_asc(entry::Column::Seq)
            .limit(limit)
            .into_model::<ScoredRowNoScore>()
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        plain(rows.into_iter().map(|r| r.0).collect())
    }
}

// ── Glyphs ───────────────────────────────────────────────────────────────────

#[async_trait]
impl GlyphReader for PgRepository {
    async fn entries_for(&self, glyph_char: &GlyphChar) -> Result<Vec<EntrySummary>, RepoError> {
        let rows = summary_query()
            .filter(Expr::col((glyph::Entity, glyph::Column::Char)).eq(glyph_char.ch().to_string()))
            .order_by_asc(entry::Column::Seq)
            .into_model::<ScoredRowNoScore>()
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        plain(rows.into_iter().map(|r| r.0).collect())
    }

    async fn image_path(&self, glyph_id: GlyphId) -> Result<Option<String>, RepoError> {
        Ok(glyph::Entity::find_by_id(glyph_id.get())
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?
            .and_then(|g| g.image_path))
    }

    async fn count(&self) -> Result<u64, RepoError> {
        glyph::Entity::find()
            .count(self.db.connection())
            .await
            .map_err(|e| self.err(e))
    }
}

// ── Printed pages ─────────────────────────────────────────────────────────────────

#[async_trait]
impl PageReader for PgRepository {
    async fn by_pdf_page(&self, pdf_page: PdfPage) -> Result<Option<PageView>, RepoError> {
        let found = page::Entity::find_by_id(i32::from(pdf_page.get()))
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        found.map(mapping::to_page_view).transpose()
    }
}

// ── Front matter ─────────────────────────────────────────────────────────────

#[async_trait]
impl FrontMatterReader for PgRepository {
    async fn by_slug(&self, slug: &Slug) -> Result<Option<FrontMatter>, RepoError> {
        let found = front_matter::Entity::find()
            .filter(front_matter::Column::Slug.eq(slug.as_str()))
            .one(self.db.connection())
            .await
            .map_err(|e| self.err(e))?;
        found.map(mapping::to_front_matter).transpose()
    }

    async fn all(&self) -> Result<Vec<FrontMatter>, RepoError> {
        front_matter::Entity::find()
            .order_by_asc(front_matter::Column::PdfPage)
            .all(self.db.connection())
            .await
            .map_err(|e| self.err(e))?
            .into_iter()
            .map(mapping::to_front_matter)
            .collect()
    }
}

// ── Statistics ───────────────────────────────────────────────────────────────

#[async_trait]
impl StatsReader for PgRepository {
    async fn stats(&self) -> Result<DataStats, RepoError> {
        let conn = self.db.connection();
        Ok(DataStats {
            entries: entry::Entity::find()
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            sub_entries: sub_entry::Entity::find()
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            glyphs: glyph::Entity::find()
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            pages: page::Entity::find()
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            entries_needing_review: entry::Entity::find()
                .filter(entry::Column::NeedsReview.eq(true))
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            sub_entries_needing_review: sub_entry::Entity::find()
                .filter(sub_entry::Column::NeedsReview.eq(true))
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
            image_only_glyphs: entry::Entity::find()
                .filter(entry::Column::GlyphId.is_null())
                .count(conn)
                .await
                .map_err(|e| self.err(e))?,
        })
    }
}

/// Keep `QueryTrait` in scope so `.into_query()` is available when inspecting SQL while debugging.
const _: fn() = || {
    fn assert_query_trait<T: QueryTrait>() {}
    assert_query_trait::<sea_orm::Select<entry::Entity>>();
};
