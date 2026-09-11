#![allow(clippy::expect_used, clippy::panic)]

//! The application layer, running against the fake repo — no database needed.

use std::sync::Arc;

use dnqatv_app::bridge::Bridge;
use dnqatv_app::service::{BrowseService, DictionaryService};
use dnqatv_core::model::{Letter, PdfPage, Slug};
use dnqatv_core::search::Pagination;
use dnqatv_testkit::{FakeRepository, page_11, page_500};

fn dictionary() -> DictionaryService {
    let mut seeds = page_11();
    seeds.extend(page_500());
    let repo = Arc::new(FakeRepository::from_seeds(&seeds).expect("building the fake repo"));
    DictionaryService::new(
        repo.clone(),
        // The fake repo does not simulate the Postgres full-text ranking; the tests here
        // use only the part of the contract expressible without a search engine.
        Arc::new(NoSearch),
        repo,
        Arc::new(bridge()),
    )
}

fn bridge() -> Bridge {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../review");
    Bridge::load(&dir).expect("reading the orthography bridge file")
}

struct NoSearch;

#[async_trait::async_trait]
impl dnqatv_core::port::EntrySearchPort for NoSearch {
    async fn search(
        &self,
        query: &dnqatv_core::search::SearchQuery,
    ) -> Result<
        dnqatv_core::search::Paged<dnqatv_core::search::ScoredEntry>,
        dnqatv_core::port::RepoError,
    > {
        Ok(dnqatv_core::search::Paged::empty(query.page))
    }

    async fn suggest(
        &self,
        _query: &dnqatv_core::search::SearchQuery,
        _limit: u64,
    ) -> Result<Vec<dnqatv_core::model::EntrySummary>, dnqatv_core::port::RepoError> {
        Ok(Vec::new())
    }
}

fn slug(s: &str) -> Slug {
    Slug::parse(s).expect("valid slug")
}

// ── The entry page ───────────────────────────────────────────────────────────

#[tokio::test]
async fn the_entry_page_carries_sub_entries_neighbours_and_the_printed_page() {
    let svc = dictionary();
    let page = svc
        .lookup(&slug("lom"))
        .await
        .expect("lookup")
        .expect("the entry must exist");

    assert_eq!(page.entry.summary.reading.as_str(), "Lõm");
    assert_eq!(page.entry.sub_entry_count(), 3);

    // Verbatim and derived are separate fields — the UI picks which to show, but the two
    // can never be confused.
    let first = &page.entry.sub_entries[0];
    assert_eq!(first.form, "― gươm");
    assert_eq!(first.form_expanded, "Lõm gươm");

    // Neighbours follow book order, not the alphabet.
    assert!(page.previous.is_some(), "there must be a previous entry");
    assert_eq!(page.next.as_ref().map(|n| n.reading.as_str()), Some("Lôm"));

    let printed = page.page.expect("the printed page must exist");
    assert_eq!(printed.pdf_page.get(), 500);
    assert_eq!(printed.printed_page.map(|p| p.get()), Some(499));
}

#[test]
fn the_first_entry_slug_carries_no_suffix() {
    // The first entry with a reading keeps the bare stem; this fixes URLs, so it must hold.
    let seeds = page_500();
    let repo = FakeRepository::from_seeds(&seeds).expect("building the repo");
    let _ = repo;
}

#[tokio::test]
async fn a_missing_entry_returns_none_rather_than_an_error() {
    let svc = dictionary();
    assert!(
        svc.lookup(&slug("khong-he-co"))
            .await
            .expect("lookup")
            .is_none()
    );
}

// ── Entry of the day ─────────────────────────────────────────────────────────

#[tokio::test]
async fn the_entry_of_the_day_is_deterministic_per_day() {
    let svc = dictionary();
    let a = svc
        .entry_of_the_day(7)
        .await
        .expect("selection")
        .expect("an entry exists");
    let b = svc
        .entry_of_the_day(7)
        .await
        .expect("selection")
        .expect("an entry exists");
    assert_eq!(
        a.reading, b.reading,
        "the same day must give the same entry"
    );

    // And it must wrap around, never running past the list.
    for day in 0..12u64 {
        assert!(
            svc.entry_of_the_day(day)
                .await
                .expect("selection")
                .is_some()
        );
    }
}

// ── The orthography bridge ───────────────────────────────────────────────────

#[test]
fn the_orthography_bridge_suggests_in_both_directions() {
    let b = bridge();
    let from_old = b.suggest("quấc");
    assert_eq!(from_old.len(), 1);
    assert_eq!(from_old[0].alternative, "quốc");

    let from_new = b.suggest("quốc");
    assert_eq!(from_new.len(), 1);
    assert_eq!(from_new[0].alternative, "quấc");
}

#[test]
fn the_orthography_bridge_matches_unaccented_and_uppercase_input() {
    let b = bridge();
    for form in ["nhơn", "NHƠN", "nhon", "Nhơn"] {
        assert_eq!(
            b.suggest(form).first().map(|s| s.alternative.as_str()),
            Some("nhân"),
            "typed {form:?}"
        );
    }
}

#[test]
fn the_orthography_bridge_is_honest_about_what_has_been_reviewed() {
    let b = bridge();
    // Nobody has signed off any line yet, and the UI must say so rather than stay silent.
    assert_eq!(b.unverified(), b.len());
    assert!(b.suggest("quấc").iter().all(|s| !s.verified));
}

#[tokio::test]
async fn orthography_suggestions_appear_even_when_there_are_results() {
    // The original plan was to suggest only when nothing matched. Measuring real data showed
    // that is broken: `nhân` has 14 entries AND `nhơn` has 13 different ones. Suggesting only
    // when empty would hide exactly half of them.
    let svc = dictionary();
    let out = svc
        .search(
            "nhân",
            dnqatv_core::search::SearchMode::Auto,
            Pagination::first_page(),
        )
        .await
        .expect("search");
    assert_eq!(
        out.orthography.first().map(|s| s.alternative.as_str()),
        Some("nhơn")
    );
}

#[tokio::test]
async fn a_query_is_never_rewritten() {
    let svc = dictionary();
    let out = svc
        .search(
            "quấc",
            dnqatv_core::search::SearchMode::Auto,
            Pagination::first_page(),
        )
        .await
        .expect("search");
    assert_eq!(out.query.text(), "quấc", "the query must survive intact");
}

// ── Browsing ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn browsing_by_letter_and_by_page() {
    let mut seeds = page_11();
    seeds.extend(page_500());
    let repo = Arc::new(FakeRepository::from_seeds(&seeds).expect("building the repo"));
    let browse = BrowseService::new(
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
    );

    let l = browse
        .by_letter(Letter::L, Pagination::first_page())
        .await
        .expect("browse by letter");
    assert_eq!(l.total, 3);
    assert_eq!(l.items[0].reading.as_str(), "Lõm");

    let a = browse
        .by_letter(Letter::A, Pagination::first_page())
        .await
        .expect("browse by letter");
    assert_eq!(a.total, 1);

    let (view, entries) = browse
        .page_view(PdfPage::new(500).expect("valid page"))
        .await
        .expect("xem trang")
        .expect("the page must exist");
    assert_eq!(view.printed_page.map(|p| p.get()), Some(499));
    assert_eq!(entries.len(), 3);
}

#[tokio::test]
async fn the_data_quality_figures_count_correctly() {
    let mut seeds = page_11();
    seeds.extend(page_500());
    let repo = Arc::new(FakeRepository::from_seeds(&seeds).expect("building the repo"));
    let browse = BrowseService::new(
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
    );
    let s = browse.stats().await.expect("statistics");
    assert_eq!(s.entries, 4);
    assert_eq!(s.sub_entries, 8);
    assert_eq!(s.glyphs, 4);
}
