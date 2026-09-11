#![allow(clippy::expect_used, clippy::panic)]

//! The ports running against **real PostgreSQL, loaded with the whole book**.
//!
//! This is the only place to check the part of the contract the fake repo cannot express:
//! full-text ranking, unaccented lookup, reverse lookup. The fake could pretend to do these,
//! but pretending here is self-deception — Postgres alone decides the result order.
//!
//! Skipped when `DATABASE_URL` is absent, and also when the database is still empty: a
//! machine that has not run `import` must still be able to run `cargo test`.

use dnqatv_adapter_db::{PgRepository, connect};
use dnqatv_config::{AppConfig, ConfigError, EnvVar};
use dnqatv_core::model::{GlyphChar, Letter, PdfPage, Slug};
use dnqatv_core::port::{EntryReader, EntrySearchPort, GlyphReader, PageReader, StatsReader};
use dnqatv_core::search::{Pagination, RankTier, SearchMode, SearchQuery};

async fn repo() -> Option<PgRepository> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config: AppConfig = match dnqatv_config::load_from_dotenv_and_env(&root.join(".env")) {
        Ok(c) => c,
        Err(ConfigError::Missing(EnvVar::DatabaseUrl)) => {
            eprintln!("skipped: DATABASE_URL is not set");
            return None;
        }
        Err(e) => panic!("broken configuration: {e}"),
    };
    let db = connect(&config.database).await.expect("connecting");
    let repo = PgRepository::new(db);
    if EntryReader::count(&repo).await.expect("counting entries") == 0 {
        eprintln!("skipped: the database is still empty, run `import` first");
        return None;
    }
    Some(repo)
}

macro_rules! repo_or_skip {
    () => {
        match repo().await {
            Some(r) => r,
            None => return,
        }
    };
}

fn query(text: &str, mode: SearchMode) -> SearchQuery {
    SearchQuery::parse(text, mode, Pagination::first_page()).expect("valid query")
}

// ── The six checks of §20, run against real data ─────────────────────────────

#[tokio::test]
async fn an_accented_search_returns_the_right_entry() {
    let repo = repo_or_skip!();
    let found = repo
        .search(&query("Lõm", SearchMode::QuocNgu))
        .await
        .expect("search");
    let first = found.items.first().expect("there must be a result");
    assert_eq!(first.entry.reading.as_str(), "Lõm");
    assert_eq!(
        first.tier,
        RankTier::ExactReading,
        "an exact match must rank first"
    );
    assert_eq!(
        first.entry.glyph.as_ref().map(|g| g.ch()),
        Some('\u{28C32}'),
        "the glyph must be 𨰲, not a square"
    );
}

#[tokio::test]
async fn an_unaccented_search_returns_every_tone_variant_without_penalising_the_accented_one() {
    let repo = repo_or_skip!();
    let found = repo
        .search(&query("lom", SearchMode::QuocNgu))
        .await
        .expect("search");

    let readings: Vec<&str> = found
        .items
        .iter()
        .map(|s| s.entry.reading.as_str())
        .collect();
    for want in ["Lõm", "Lôm", "Lốm", "Lồm", "Lỗm"] {
        assert!(readings.contains(&want), "{want} missing from {readings:?}");
    }

    // Tone marks are meaning-bearing: every folded result must rank after every accented one.
    let tiers: Vec<RankTier> = found.items.iter().map(|s| s.tier).collect();
    let mut sorted = tiers.clone();
    sorted.sort();
    assert_eq!(tiers, sorted, "results must be ordered by tier");
}

#[tokio::test]
async fn a_script_search_returns_every_reading_of_the_glyph() {
    let repo = repo_or_skip!();
    let glyph = GlyphChar::parse("惡").expect("valid glyph");
    let entries = repo.entries_for(&glyph).await.expect("glyph lookup");
    let readings: Vec<&str> = entries.iter().map(|e| e.reading.as_str()).collect();
    assert!(readings.contains(&"Ác"), "{readings:?}");
    assert!(readings.contains(&"Ố"), "{readings:?}");
}

#[tokio::test]
async fn reverse_lookup_from_a_sub_entry_back_to_the_entry() {
    // The hardest and most valuable feature: the print has "― gươm. Nạm gươm." under Lõm,
    // and someone typing "nạm gươm" must find Lõm.
    let repo = repo_or_skip!();
    let found = repo
        .search(&query("nạm gươm", SearchMode::ToanVan))
        .await
        .expect("search");
    let first = found.items.first().expect("there must be a result");
    assert_eq!(first.entry.reading.as_str(), "Lõm");
    assert_eq!(first.tier, RankTier::FullText);
}

#[tokio::test]
async fn a_sub_entry_keeps_both_the_verbatim_and_the_derived_form() {
    let repo = repo_or_skip!();
    let found = repo
        .search(&query("Lõm", SearchMode::QuocNgu))
        .await
        .expect("search");
    let slug = &found.items.first().expect("there is a result").entry.slug;
    let detail = repo
        .by_slug(slug)
        .await
        .expect("entry lookup")
        .expect("the entry must exist");

    let first = detail
        .sub_entries
        .first()
        .expect("there must be a sub-entry");
    assert_eq!(
        first.form, "― gươm",
        "the verbatim form keeps the placeholder"
    );
    assert_eq!(
        first.form_expanded, "Lõm gươm",
        "the derived form substitutes the reading"
    );
    assert_ne!(
        first.form, first.form_expanded,
        "the two fields must differ"
    );
}

#[tokio::test]
async fn the_printed_page_is_offset_from_the_pdf_page_by_exactly_one() {
    let repo = repo_or_skip!();
    let view = PageReader::by_pdf_page(&repo, PdfPage::new(500).expect("valid page"))
        .await
        .expect("reading the page")
        .expect("the page must exist");
    assert_eq!(view.printed_page.map(|p| p.get()), Some(499));
    assert_eq!(view.letter, Some(Letter::L));

    // Three pages have no printed number — a measured invariant that must hold in the store too.
    for n in [1u16, 2, 10] {
        let v = PageReader::by_pdf_page(&repo, PdfPage::new(n).expect("valid"))
            .await
            .expect("reading the page")
            .expect("the page must exist");
        assert_eq!(v.printed_page, None, "PDF page {n} has no printed number");
    }
}

// ── Browsing ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn browsing_by_letter_keeps_book_order() {
    let repo = repo_or_skip!();
    let paged = repo
        .by_letter(Letter::A, Pagination::new(0, 10))
        .await
        .expect("browse by letter");
    // The exact number is a canary: if a reload changes it, something changed unnoticed.
    assert_eq!(paged.total, 74, "the A section of the print");
    assert_eq!(
        paged.items.first().map(|e| e.reading.as_str()),
        Some("A"),
        "the first entry of the book"
    );
}

#[tokio::test]
async fn neighbouring_entries_follow_book_order_not_the_alphabet() {
    let repo = repo_or_skip!();
    let found = repo
        .search(&query("Lõm", SearchMode::QuocNgu))
        .await
        .expect("search");
    let slug = &found.items.first().expect("there is a result").entry.slug;
    let (previous, next) = repo.neighbours(slug).await.expect("neighbours");
    assert!(previous.is_some());
    assert_eq!(
        next.map(|n| n.reading.as_str().to_owned()),
        Some("Lôm".to_owned())
    );
}

#[tokio::test]
async fn slugs_are_stable_and_resolve_back() {
    let repo = repo_or_skip!();
    let first = repo
        .nth(0)
        .await
        .expect("the first entry")
        .expect("it must exist");
    let again = repo
        .by_slug(&first.slug)
        .await
        .expect("tra")
        .expect("it must exist");
    assert_eq!(again.summary.reading, first.reading);

    // An unknown slug returns None, not an error.
    let missing = Slug::parse("no-such-entry").expect("valid slug");
    assert!(repo.by_slug(&missing).await.expect("tra").is_none());
}

// ── Statistics ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_statistics_match_the_pipeline_results() {
    let repo = repo_or_skip!();
    let s = repo.stats().await.expect("statistics");
    assert_eq!(s.entries, 7_687, "entries assembled from 1,038 pages");
    // 57,892 sub-entry LINES were split, but 14 of them are halves of a form the print wrapped
    // onto a second line; `merge_wrapped_forms` re-joins each pair into the one sub-entry the
    // book actually prints. Gate ③ is unmoved by this — the lines are regrouped, never dropped.
    assert_eq!(s.sub_entries, 57_878);
    assert_eq!(s.glyphs, 4_673);
    assert_eq!(s.pages, 1_038);
    assert_eq!(
        s.image_only_glyphs, 29,
        "entries whose glyph the print shows as an image"
    );
}

#[tokio::test]
async fn near_match_suggestions_do_not_return_junk() {
    let repo = repo_or_skip!();
    let suggestions = repo
        .suggest(&query("lomm", SearchMode::QuocNgu), 5)
        .await
        .expect("suggestions");
    assert!(
        !suggestions.is_empty(),
        "a light typo must yield suggestions"
    );
    assert!(suggestions.len() <= 5);
}

#[tokio::test]
async fn no_page_holds_entries_from_two_different_letters() {
    // `by_letter` filters through the letter of the PAGE, so it is only correct while this
    // invariant holds. Measured across all 1,038 pages: the print starts each letter on a
    // fresh page. This test keeps that indirect filter from silently breaking after a reload.
    let repo = repo_or_skip!();
    let mut total = 0u64;
    for letter in Letter::ALL {
        let paged = repo
            .by_letter(letter, Pagination::new(0, Pagination::MAX_LIMIT))
            .await
            .expect("browse by letter");
        total += paged.total;

        // Every entry returned must really belong to that letter, by its own reading.
        for e in &paged.items {
            assert_eq!(
                e.reading.letter().expect("valid initial"),
                letter,
                "entry {:?} was filed under letter {:?}",
                e.reading.as_str(),
                letter.label()
            );
        }
    }
    let all = EntryReader::count(&repo).await.expect("count");
    assert_eq!(
        total, all,
        "the 22 letters must sum to the entry total — no entry may fall outside"
    );
}
