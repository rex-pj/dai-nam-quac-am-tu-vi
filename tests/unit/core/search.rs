#![allow(clippy::expect_used, clippy::panic)]

//! Search policy and slugs — pure functions, milliseconds, no database.

use dnqatv_core::model::{Pos, PosSet, Reading, Slug, SlugMinter};
use dnqatv_core::search::{Pagination, RankTier, SearchMode, SearchQuery, is_han_nom};

fn reading(s: &str) -> Reading {
    Reading::parse(s).expect("valid reading")
}

// ── Ranking ──────────────────────────────────────────────────────────────────

#[test]
fn an_accented_match_always_beats_an_unaccented_one() {
    // Ả, Á and À are three different entries, so an accented match must outrank a folded one.
    assert!(RankTier::ExactReading.rank() < RankTier::UnaccentedReading.rank());
    assert!(RankTier::PrefixReading.rank() < RankTier::UnaccentedReading.rank());
    assert!(RankTier::UnaccentedReading.rank() < RankTier::FullText.rank());
    assert!(RankTier::FullText.rank() < RankTier::Fuzzy.rank());
}

#[test]
fn the_rank_and_the_display_label_are_one_set() {
    // If the two were separate, the display groups would eventually drift from the ranking.
    for tier in RankTier::ALL {
        assert_eq!(RankTier::from_rank(tier.rank()), Some(tier));
        assert!(!tier.group_label().is_empty());
        assert!(!tier.as_param().is_empty());
    }
    let ranks: Vec<i32> = RankTier::ALL.iter().map(|t| t.rank()).collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(ranks, sorted, "declaration order must be ranking order");
}

// ── Mode detection ───────────────────────────────────────────────────────────

#[test]
fn pasting_han_nom_switches_to_script_lookup() {
    for q in ["阿", "𨰲", "惡"] {
        assert_eq!(SearchMode::Auto.resolve(q), SearchMode::HanNom, "{q}");
    }
}

#[test]
fn typing_quoc_ngu_stays_in_reading_lookup() {
    for q in ["lõm", "lom", "nạm gươm"] {
        assert_eq!(SearchMode::Auto.resolve(q), SearchMode::QuocNgu, "{q}");
    }
}

#[test]
fn an_explicit_mode_is_not_second_guessed() {
    assert_eq!(
        SearchMode::ToanVan.resolve("阿"),
        SearchMode::ToanVan,
        "an explicit choice must beat the inference"
    );
}

#[test]
fn every_script_range_the_book_uses_is_recognised() {
    assert!(is_han_nom('阿')); // CJK Unified
    assert!(is_han_nom('\u{28C32}')); // Ext B — 𨰲
    assert!(is_han_nom('\u{F0000}')); // PUA plane 15 — font Nom Na Tong
    assert!(!is_han_nom('L'));
    assert!(!is_han_nom('õ'));
}

// ── Queries ─────────────────────────────────────────────────────────────────

#[test]
fn a_query_is_nfc_normalized_exactly_once() {
    // Pasting from macOS yields NFD; it must match the NFC data in the store.
    let nfd = "Lo\u{303}m";
    let q = SearchQuery::parse(nfd, SearchMode::Auto, Pagination::first_page()).expect("valid");
    assert_eq!(q.text(), "Lõm");
    assert_eq!(q.folded(), "lom");
}

#[test]
fn an_empty_or_overlong_query_is_rejected() {
    assert!(SearchQuery::parse("   ", SearchMode::Auto, Pagination::first_page()).is_none());
    let dai = "a".repeat(SearchQuery::MAX_LEN + 1);
    assert!(SearchQuery::parse(&dai, SearchMode::Auto, Pagination::first_page()).is_none());
}

#[test]
fn pagination_has_a_hard_ceiling() {
    let p = Pagination::new(0, 10_000);
    assert_eq!(p.limit(), Pagination::MAX_LIMIT);
    // A limit of 0 is meaningless; raise it to 1 rather than silently returning an empty page.
    assert_eq!(Pagination::new(0, 0).limit(), 1);
}

// ── Slug ─────────────────────────────────────────────────────────────────────

#[test]
fn a_slug_follows_the_accent_folded_reading() {
    assert_eq!(Slug::stem(&reading("Lõm")).expect("valid").as_str(), "lom");
    assert_eq!(
        Slug::stem(&reading("Đại Nam")).expect("valid").as_str(),
        "dai-nam"
    );
    // đ is not d plus a combining mark, yet it must still yield "d" in a slug.
    assert_eq!(Slug::stem(&reading("Đèo")).expect("valid").as_str(), "deo");
}

#[test]
fn a_slug_has_no_stray_hyphens_at_either_end() {
    let s = Slug::stem(&reading("  A  di   đà  ")).expect("valid");
    assert_eq!(s.as_str(), "a-di-da");
}

#[test]
fn duplicate_slugs_are_disambiguated_in_book_order() {
    // Many entries share a reading. The rule must be DETERMINISTIC: the same input order
    // gives the same slugs, otherwise every data reload rewrites every URL.
    let mut minter = SlugMinter::new();
    let got: Vec<String> = ["A", "A", "Lõm", "A"]
        .iter()
        .map(|r| {
            minter
                .mint(&reading(r))
                .expect("minting a slug")
                .as_str()
                .to_owned()
        })
        .collect();
    assert_eq!(got, vec!["a", "a-2", "lom", "a-3"]);

    let mut again = SlugMinter::new();
    let second_run: Vec<String> = ["A", "A", "Lõm", "A"]
        .iter()
        .map(|r| {
            again
                .mint(&reading(r))
                .expect("minting a slug")
                .as_str()
                .to_owned()
        })
        .collect();
    assert_eq!(got, second_run, "must be deterministic");
}

#[test]
fn a_slug_from_a_url_is_checked_strictly() {
    assert!(Slug::parse("lom").is_ok());
    assert!(Slug::parse("a-2").is_ok());
    for bad in ["Lõm", "lom!", "lom/../etc", "", "  "] {
        assert!(Slug::parse(bad).is_err(), "must reject {bad:?}");
    }
}

// ── Part-of-speech labels ────────────────────────────────────────────────────

#[test]
fn a_double_label_keeps_the_printed_order() {
    // The book prints both `c. n.` (543 entries) and `n. c.` (3). Merging them is interpretation.
    let cn = PosSet::new(vec![Pos::ChuNho, Pos::ChuNom]).expect("valid");
    let nc = PosSet::new(vec![Pos::ChuNom, Pos::ChuNho]).expect("valid");
    assert_eq!(cn.book_label(), "c. n.");
    assert_eq!(nc.book_label(), "n. c.");
    assert_ne!(cn, nc);
}

#[test]
fn an_entry_with_no_label_is_corrupt_data() {
    assert!(PosSet::new(vec![]).is_err());
}

#[test]
fn the_spelled_out_name_is_readable() {
    let cn = PosSet::new(vec![Pos::ChuNho, Pos::ChuNom]).expect("valid");
    assert_eq!(cn.display_name(), "chữ nho · chữ nôm");
}
