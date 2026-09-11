//! The web adapter: Axum + Tera + a JSON API.
//!
//! This crate **does not know PostgreSQL exists** — it only sees the `app` services, which
//! in turn only see the `core` ports. `Cargo.toml` holds the boundary: there is no `sea-orm` here.

pub mod assets;
pub mod dto;
pub mod error;
pub mod handler;
pub mod openapi;
pub mod route;
pub mod state;
pub mod template;
pub mod view;

use axum::Router;
use axum::extract::OriginalUri;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;

pub use assets::{FAVICON_SVG, FontSet};
pub use state::{AppState, SiteMeta};
pub use template::Templates;

/// The complete router.
///
/// Paths come from [`route::pattern`], alongside the constants the sitemap and templates
/// use. No path string is written inline here.
pub fn router(state: AppState) -> Router {
    use route::pattern as p;

    let mut router = Router::new()
        // ── Pages ────────────────────────────────────────────────────────────
        .route(p::HOME, get(handler::home))
        .route(p::SEARCH, get(handler::search))
        .route(p::ENTRY, get(handler::entry))
        .route(p::LETTER, get(handler::letter))
        .route(p::GLYPH, get(handler::glyph))
        .route(p::PAGE, get(handler::page))
        .route(p::ABOUT, get(handler::about))
        .route(p::QUALITY, get(handler::quality))
        // ── API ──────────────────────────────────────────────────────────────
        .route(p::API_SEARCH, get(handler::api_search))
        .route(p::API_ENTRY, get(handler::api_entry))
        .route(p::API_GLYPH, get(handler::api_glyph))
        .route(p::API_PAGE, get(handler::api_page))
        .route(p::API_STATS, get(handler::api_stats))
        .route(p::API_SPEC, get(handler::api_spec))
        .route(p::API_DOCS, get(handler::api_docs))
        // ── Assets ───────────────────────────────────────────────────────────
        .route(STYLESHEET_PATH, get(handler::stylesheet))
        .route(SCRIPT_PATH, get(handler::script))
        .route(assets::FAVICON_PATH, get(handler::favicon))
        .route(FONT_PATH, get(handler::font))
        .route(p::SITEMAP, get(handler::sitemap))
        .route(p::ROBOTS, get(handler::robots))
        .route(p::HEALTH, get(handler::health))
        .fallback(handler::not_found)
        .with_state(state);

    // ── Retired paths ────────────────────────────────────────────────────────
    // Added last, and never with state: a redirect needs nothing from the app.
    for (old, current) in route::retired::REDIRECTS {
        router = router.route(
            old,
            get(move |uri: OriginalUri| moved_permanently(current, uri)),
        );
    }

    router
}

/// `301` to `target`, carrying the query string over — see [`route::retired::location`].
///
/// Not `Redirect::permanent`, which is `308`. Both are permanent, but `301` is what the old
/// links in the wild and the search engines already understand for a renamed GET page.
async fn moved_permanently(
    target: &'static str,
    OriginalUri(uri): OriginalUri,
) -> impl IntoResponse {
    let location = route::retired::location(target, uri.query());
    match header::HeaderValue::try_from(location) {
        Ok(value) => (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, value)]).into_response(),
        // A query that cannot go in a header is not worth a 500: drop it, keep the path.
        Err(_) => (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, target)]).into_response(),
    }
}

/// Embedded asset paths. Templates use exactly these two constants.
pub const STYLESHEET_PATH: &str = "/tinh/main.css";
pub const SCRIPT_PATH: &str = "/tinh/enhance.js";
/// The Nôm font. Only files that actually live in the font directory are served.
pub const FONT_PATH: &str = "/tinh/fonts/{file}";
