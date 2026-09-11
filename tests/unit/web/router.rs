#![allow(clippy::expect_used, clippy::panic)]

//! The router itself, driven with real requests against the fake repository.
//!
//! Rendering is covered in `templates.rs`. What this file exists for is the wiring no
//! template test can see: which path is served, what status it answers with, and where a
//! retired path sends the reader.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use dnqatv_adapter_web::route::pattern;
use dnqatv_adapter_web::router;
use dnqatv_adapter_web::state::{AppState, SiteMeta};
use dnqatv_adapter_web::template::Templates;
use dnqatv_app::bridge::Bridge;
use dnqatv_app::service::{BrowseService, DictionaryService};
use dnqatv_testkit::{FakeRepository, page_11, page_500};
use tower::ServiceExt;

/// The fake repository answers no search — this suite never asserts on search results.
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

fn app() -> Router {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seeds = page_11();
    seeds.extend(page_500());
    let repo = Arc::new(FakeRepository::from_seeds(&seeds).expect("building the fake repo"));
    let bridge = Arc::new(Bridge::load(&root.join("review")).expect("reading the bridge file"));
    let site = Arc::new(SiteMeta::new(
        "https://example.test".to_owned(),
        root.join("frontend/static/fonts"),
    ));

    router(AppState {
        dictionary: DictionaryService::new(repo.clone(), Arc::new(NoSearch), repo.clone(), bridge),
        browse: BrowseService::new(repo.clone(), repo.clone(), repo.clone(), repo.clone(), repo),
        templates: Arc::new(Templates::load(site.clone()).expect("loading templates")),
        site,
    })
}

async fn get(path: &str) -> axum::response::Response {
    app()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("building the request"),
        )
        .await
        .expect("the router must answer")
}

fn location(response: &axum::response::Response) -> &str {
    response
        .headers()
        .get(header::LOCATION)
        .expect("a redirect must carry Location")
        .to_str()
        .expect("Location must be printable ASCII")
}

#[tokio::test]
async fn a_retired_path_answers_301_to_its_new_home() {
    for (old, current) in dnqatv_adapter_web::route::retired::REDIRECTS {
        let response = get(old).await;
        assert_eq!(
            response.status(),
            StatusCode::MOVED_PERMANENTLY,
            "{old} must answer 301, not {}",
            response.status()
        );
        assert_eq!(location(&response), current, "{old} sent the reader astray");
    }
}

#[tokio::test]
async fn the_retired_search_path_keeps_the_reader_query() {
    // The whole point of the rename is that nobody loses anything. A 301 to a bare
    // `/tra-tim` would hand the reader an empty search box and look like the site ate
    // their query — the failure the redirect exists to prevent.
    let response = get("/tra-cuu?q=l%C3%B5m&che_do=toan-van").await;

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        location(&response),
        "/tra-tim?q=l%C3%B5m&che_do=toan-van",
        "every parameter must come along, not just the first"
    );
}

#[tokio::test]
async fn robots_disallows_the_retired_search_path_too() {
    // The retired path still answers — with a 301 — so a crawler holding an old
    // `/tra-cuu?q=…` keeps requesting it unless robots names it as well. Disallowing only
    // the new path leaves the old one crawled for ever.
    let response = get(pattern::ROBOTS).await;
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("reading robots.txt");
    let body = String::from_utf8(body.to_vec()).expect("robots.txt must be UTF-8");

    assert!(
        body.contains(&format!(
            "Disallow: {}
",
            pattern::SEARCH
        )),
        "{body}"
    );
    for (old, current) in dnqatv_adapter_web::route::retired::REDIRECTS {
        if current == pattern::SEARCH {
            assert!(
                body.contains(&format!(
                    "Disallow: {old}
"
                )),
                "{old} redirects into the disallowed search page but is itself crawlable:
{body}"
            );
        }
    }
}

#[tokio::test]
async fn the_paths_the_redirects_point_at_are_really_served() {
    // A redirect to a 404 is worse than no redirect: it looks deliberate.
    for path in [pattern::SEARCH, pattern::QUALITY, pattern::API_STATS] {
        let status = get(path).await.status();
        assert_ne!(status, StatusCode::NOT_FOUND, "{path} is not served");
        assert_ne!(
            status,
            StatusCode::MOVED_PERMANENTLY,
            "{path} must be the destination, not another hop"
        );
    }
}
