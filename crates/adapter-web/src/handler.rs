//! Handlers — deliberately thin.
//!
//! Each function here does exactly three things: parse parameters into domain types, call a
//! service, turn the result into a view or a DTO. No business decision lives here. Any `if`
//! about dictionary content that appears in this file is in the wrong layer.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use dnqatv_app::service::{BrowseService, DictionaryService};
use dnqatv_core::model::{GlyphChar, Letter, PageAddress, PdfPage, Slug};
use dnqatv_core::search::{Pagination, SearchMode};
use serde::Deserialize;

use crate::dto;
use crate::error::WebError;
use crate::route::{self, pattern, url};
use crate::state::SiteMeta;
use crate::template::{self, Templates, name};
use crate::view::{
    EntryDetailView, EntrySummaryView, LetterChipView, PageMetaView, PagedCountView,
    ResultGroupView, SuggestionView,
};

/// The search box parameters.
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub che_do: Option<String>,
    #[serde(default)]
    pub trang: Option<u64>,
}

impl SearchParams {
    fn mode(&self) -> SearchMode {
        self.che_do
            .as_deref()
            .and_then(SearchMode::parse)
            .unwrap_or(SearchMode::Auto)
    }

    fn pagination(&self) -> Pagination {
        let page = self.trang.unwrap_or(1).max(1);
        Pagination::new(
            (page - 1) * Pagination::DEFAULT_LIMIT,
            Pagination::DEFAULT_LIMIT,
        )
    }
}

// ── HTML pages ───────────────────────────────────────────────────────────────

pub async fn home(
    State(dict): State<DictionaryService>,
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
) -> Result<Html<String>, WebError> {
    // Entry of the day: deterministic on the day count since the Unix epoch, so everyone
    // sees one entry per day and the link is shareable. No random number.
    let day = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or_default();
    let featured = dict.entry_of_the_day(day).await?;

    let mut ctx = templates.context();
    ctx.insert("featured", &featured.as_ref().map(EntrySummaryView::from));
    // The search box already sits in the left column, so the home page uses the space to
    // say where the data stands — including what is still awaiting review.
    ctx.insert("stats", &dto::StatsDto::from(&browse.stats().await?));
    Ok(Html(templates.render(name::HOME, &ctx)?))
}

pub async fn search(
    State(dict): State<DictionaryService>,
    State(templates): State<Arc<Templates>>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, WebError> {
    let mut ctx = templates.context();
    ctx.insert("query", &params.q);
    ctx.insert("mode", params.mode().as_param());

    if params.q.trim().is_empty() {
        ctx.insert("groups", &Vec::<ResultGroupView>::new());
        ctx.insert("total", &0u64);
        ctx.insert("orthography", &Vec::<SuggestionView>::new());
        ctx.insert("did_you_mean", &Vec::<EntrySummaryView>::new());
        return Ok(Html(templates.render(name::SEARCH, &ctx)?));
    }

    let outcome = dict
        .search(&params.q, params.mode(), params.pagination())
        .await?;

    let groups: Vec<ResultGroupView> = outcome
        .groups
        .iter()
        .map(|(tier, items)| ResultGroupView::new(*tier, items))
        .collect();

    ctx.insert("groups", &groups);
    ctx.insert("total", &outcome.total);
    ctx.insert(
        "orthography",
        &outcome
            .orthography
            .iter()
            .map(SuggestionView::from)
            .collect::<Vec<_>>(),
    );
    ctx.insert(
        "did_you_mean",
        &outcome
            .did_you_mean
            .iter()
            .map(EntrySummaryView::from)
            .collect::<Vec<_>>(),
    );
    Ok(Html(templates.render(name::SEARCH, &ctx)?))
}

pub async fn entry(
    State(dict): State<DictionaryService>,
    State(templates): State<Arc<Templates>>,
    Path(slug): Path<String>,
) -> Result<Html<String>, WebError> {
    let slug = Slug::parse(&slug).map_err(|_| WebError::NotFound)?;
    let page = dict.lookup(&slug).await?.ok_or(WebError::NotFound)?;

    let mut ctx = templates.context();
    // The letter of this entry lights up in the left column. The 22-letter printed order
    // (Y where I would be) is a core concept of the book; showing the current position
    // teaches it without a lecture. An unrecognised initial simply lights nothing — no guessing.
    ctx.insert(
        "letters",
        &LetterChipView::all(page.entry.summary.reading.letter().ok()),
    );
    ctx.insert("entry", &EntryDetailView::from(&page.entry));
    ctx.insert(
        "previous",
        &page.previous.as_ref().map(EntrySummaryView::from),
    );
    ctx.insert("next", &page.next.as_ref().map(EntrySummaryView::from));
    ctx.insert("page", &page.page.as_ref().map(PageMetaView::from));
    // A relative path; the template joins it with `base_url` for the canonical tag.
    ctx.insert("canonical_path", &url::entry(slug.as_str()));
    Ok(Html(templates.render(name::ENTRY, &ctx)?))
}

pub async fn letter(
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
    Path(letter): Path<String>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, WebError> {
    let letter = Letter::from_db_value(&letter).map_err(|_| WebError::NotFound)?;
    let paged = browse.by_letter(letter, params.pagination()).await?;

    let mut ctx = templates.context();
    // Overrides the base-context list so the letter being browsed lights up on the left.
    ctx.insert("letters", &LetterChipView::all(Some(letter)));
    ctx.insert("letter", letter.label());
    // The display label and the URL segment are TWO different things: the letter Đ shows as
    // "Đ" but is "dd" in a URL. The template used to lowercase the label itself, so every
    // pagination link under Đ pointed at /van/đ and returned 404 — 398 entries lost their pages.
    ctx.insert("letter_slug", letter.db_value());
    // The count in the heading must describe what is on the SCREEN, not what the query
    // matched: "74 chữ đầu" over thirty rows leaves the reader counting and finding thirty.
    ctx.insert("count", &PagedCountView::new(&paged));
    ctx.insert("has_more", &paged.has_more());
    ctx.insert("next_page", &(params.trang.unwrap_or(1) + 1));
    ctx.insert(
        "previous_page",
        &params.trang.unwrap_or(1).saturating_sub(1),
    );
    ctx.insert(
        "entries",
        &paged
            .items
            .iter()
            .map(EntrySummaryView::from)
            .collect::<Vec<_>>(),
    );
    Ok(Html(templates.render(name::LETTER, &ctx)?))
}

pub async fn glyph(
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
    Path(glyph): Path<String>,
) -> Result<Html<String>, WebError> {
    let glyph = GlyphChar::parse(&glyph).map_err(|_| WebError::NotFound)?;
    let entries = browse.glyph_view(&glyph).await?;
    if entries.is_empty() {
        return Err(WebError::NotFound);
    }

    let mut ctx = templates.context();
    ctx.insert(
        "letters",
        &LetterChipView::all(entries[0].reading.letter().ok()),
    );
    ctx.insert(
        "glyph",
        &crate::view::GlyphView::new(&glyph, entries[0].reading.as_str()),
    );
    ctx.insert(
        "entries",
        &entries
            .iter()
            .map(EntrySummaryView::from)
            .collect::<Vec<_>>(),
    );
    Ok(Html(templates.render(name::GLYPH, &ctx)?))
}

/// `/trang/{page}` — the segment is a **printed** page number, or `pdf-N` for the three
/// pages the print left unnumbered. Taken as a string so the domain does the reading:
/// `u16` in this position would have made the path silently mean the PDF numbering.
pub async fn page(
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
    Path(page): Path<String>,
) -> Result<Html<String>, WebError> {
    let pdf_page = PageAddress::parse(&page)
        .map_err(|_| WebError::NotFound)?
        .pdf_page();
    let (view, entries) = browse
        .page_view(pdf_page)
        .await?
        .ok_or(WebError::NotFound)?;

    let mut ctx = templates.context();
    ctx.insert("page", &PageMetaView::from(&view));
    ctx.insert(
        "entries",
        &entries
            .iter()
            .map(EntrySummaryView::from)
            .collect::<Vec<_>>(),
    );
    Ok(Html(templates.render(name::PAGE, &ctx)?))
}

pub async fn about(
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
) -> Result<Html<String>, WebError> {
    let front = browse.front_matter_all().await?;
    let mut ctx = templates.context();
    ctx.insert(
        "sections",
        &front
            .iter()
            .map(|f| {
                serde_json::json!({
                    "slug": f.slug.as_str(),
                    "title": f.title,
                    "body": f.body,
                    "pdf_page": f.pdf_page.get(),
                    "printed_page": f.pdf_page.printed().map(|p| p.get()),
                    "page_url": url::page(f.pdf_page),
                })
            })
            .collect::<Vec<_>>(),
    );
    Ok(Html(templates.render(name::ABOUT, &ctx)?))
}

pub async fn quality(
    State(browse): State<BrowseService>,
    State(templates): State<Arc<Templates>>,
) -> Result<Html<String>, WebError> {
    let stats = browse.stats().await?;
    let mut ctx = templates.context();
    ctx.insert("stats", &dto::StatsDto::from(&stats));
    Ok(Html(templates.render(name::QUALITY, &ctx)?))
}

pub async fn api_docs(State(templates): State<Arc<Templates>>) -> Result<Html<String>, WebError> {
    let mut ctx = templates.context();
    ctx.insert("spec_url", pattern::API_SPEC);
    Ok(Html(templates.render(name::API_DOCS, &ctx)?))
}

// ── Embedded static assets ───────────────────────────────────────────────────

/// The CSS: the runtime-generated `@font-face` blocks followed by the static stylesheet.
///
/// Assembled here rather than declared in the file: only fonts that really exist on disk
/// are declared, so the page never points at a missing file.
pub async fn stylesheet(State(site): State<Arc<SiteMeta>>) -> Response {
    let css = format!(
        "{}
{}",
        site.fonts.font_face_css(),
        template::STYLESHEET
    );
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], css).into_response()
}

/// The site icon. SVG, drawn as a shape rather than text — see `assets`.
pub async fn favicon() -> Response {
    (
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        crate::assets::FAVICON_SVG,
    )
        .into_response()
}

/// One font file.
///
/// Only files [`FontSet::scan`] found on disk are served — names chosen by the program,
/// never strings from the URL. That leaves no path traversal (`../`) route.
pub async fn font(
    State(site): State<Arc<SiteMeta>>,
    Path(file): Path<String>,
) -> Result<Response, WebError> {
    // Serve only the names the program ITSELF chose and found on disk. A URL cannot inject
    // an arbitrary name here, so there is no path traversal route.
    let font = site.fonts.by_name(&file).ok_or(WebError::NotFound)?;
    let bytes = std::fs::read(site.font_dir.join(&font.name))
        .map_err(|e| WebError::Render(format!("reading font {file}: {e}")))?;
    Ok((
        [
            (header::CONTENT_TYPE, font.mime()),
            // Fonts are immutable and uniquely named; a long cache is safe.
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response())
}

// ── Not found ────────────────────────────────────────────────────────────────

pub async fn not_found(State(templates): State<Arc<Templates>>) -> Response {
    // Nothing more is needed: the base context already carries the left column, so the 404
    // page still offers the search box and all 22 letters to a lost reader.
    match templates.render(name::NOT_FOUND, &templates.context()) {
        Ok(html) => (StatusCode::NOT_FOUND, Html(html)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn script() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        template::SCRIPT,
    )
        .into_response()
}

// ── Search engines and liveness ──────────────────────────────────────────────

/// `sitemap.xml` — every entry, every letter, every printed page.
///
/// Paths come from [`url`], never hand-written: a wrong sitemap is the kind of bug nobody
/// notices until a search engine drops half the site.
pub async fn sitemap(
    State(dict): State<DictionaryService>,
    State(site): State<Arc<SiteMeta>>,
) -> Result<Response, WebError> {
    let base = &site.base_url;
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let mut add = |path: &str| {
        body.push_str(&format!("  <url><loc>{base}{path}</loc></url>\n"));
    };

    add("/");
    add(pattern::ABOUT);
    add(pattern::QUALITY);
    for letter in Letter::ALL {
        add(&url::letter(letter.db_value()));
    }
    for page in PdfPage::all() {
        add(&url::page(page));
    }

    // Entries are read in book order, in batches, so one query never builds all 7,687 rows.
    let mut offset = 0u64;
    loop {
        let batch = dict
            .entries_in_order(Pagination::new(offset, Pagination::MAX_LIMIT))
            .await?;
        if batch.is_empty() {
            break;
        }
        for entry in &batch {
            add(&url::entry(entry.slug.as_str()));
        }
        offset += batch.len() as u64;
    }

    body.push_str("</urlset>\n");
    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response())
}

pub async fn robots(State(site): State<Arc<SiteMeta>>) -> Response {
    // The search results page is disallowed: it is unbounded in the query and has no content of its own.
    let mut body = String::from(
        "User-agent: *
",
    );
    body.push_str(&format!(
        "Disallow: {}
",
        pattern::SEARCH
    ));

    // And the retired paths that lead there. They still answer — with a 301 — so a crawler
    // holding an old `/tra-cuu?q=…` would go on requesting it unless it is disallowed too.
    for (old, current) in route::retired::REDIRECTS {
        if current == pattern::SEARCH {
            body.push_str(&format!(
                "Disallow: {old}
"
            ));
        }
    }

    body.push_str(&format!(
        "Sitemap: {}{}
",
        site.base_url,
        pattern::SITEMAP
    ));
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

pub async fn health() -> Response {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok").into_response()
}

// ── API JSON ─────────────────────────────────────────────────────────────────

pub async fn api_search(
    State(dict): State<DictionaryService>,
    Query(params): Query<SearchParams>,
) -> Result<Json<dto::SearchResponseDto>, WebError> {
    let page = params.pagination();
    if params.q.trim().is_empty() {
        return Ok(Json(dto::SearchResponseDto {
            query: params.q.clone(),
            mode: params.mode().as_param().to_owned(),
            total: 0,
            offset: page.offset(),
            limit: page.limit(),
            results: Vec::new(),
            orthography: Vec::new(),
            did_you_mean: Vec::new(),
        }));
    }

    let outcome = dict.search(&params.q, params.mode(), page).await?;
    Ok(Json(dto::SearchResponseDto {
        // The query comes back VERBATIM as the user typed it — the API never returns a corrected version.
        query: params.q.clone(),
        mode: params.mode().as_param().to_owned(),
        total: outcome.total,
        offset: outcome.page.offset(),
        limit: outcome.page.limit(),
        results: outcome
            .groups
            .iter()
            .flat_map(|(_, items)| items.iter())
            .map(dto::ScoredEntryDto::from)
            .collect(),
        orthography: outcome
            .orthography
            .iter()
            .map(|s| dto::SuggestionDto {
                text: s.alternative.clone(),
                reason: s.reason.to_owned(),
                verified: s.verified,
            })
            .collect(),
        did_you_mean: outcome
            .did_you_mean
            .iter()
            .map(dto::EntryDto::from)
            .collect(),
    }))
}

pub async fn api_entry(
    State(dict): State<DictionaryService>,
    Path(slug): Path<String>,
) -> Result<Json<dto::EntryDetailDto>, WebError> {
    let slug = Slug::parse(&slug).map_err(|_| WebError::NotFound)?;
    let page = dict.lookup(&slug).await?.ok_or(WebError::NotFound)?;
    Ok(Json(dto::EntryDetailDto::from(&page.entry)))
}

pub async fn api_glyph(
    State(browse): State<BrowseService>,
    Path(glyph): Path<String>,
) -> Result<Json<Vec<dto::EntryDto>>, WebError> {
    let glyph = GlyphChar::parse(&glyph).map_err(|_| WebError::NotFound)?;
    let entries = browse.glyph_view(&glyph).await?;
    if entries.is_empty() {
        return Err(WebError::NotFound);
    }
    Ok(Json(entries.iter().map(dto::EntryDto::from).collect()))
}

pub async fn api_page(
    State(browse): State<BrowseService>,
    Path(page): Path<String>,
) -> Result<Json<dto::PageDto>, WebError> {
    let pdf_page = PageAddress::parse(&page)
        .map_err(|_| WebError::NotFound)?
        .pdf_page();
    let (view, entries) = browse
        .page_view(pdf_page)
        .await?
        .ok_or(WebError::NotFound)?;
    Ok(Json(dto::PageDto::new(&view, &entries)))
}

pub async fn api_stats(
    State(browse): State<BrowseService>,
) -> Result<Json<dto::StatsDto>, WebError> {
    Ok(Json(dto::StatsDto::from(&browse.stats().await?)))
}

pub async fn api_spec() -> Response {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        crate::openapi::document(),
    )
        .into_response()
}
