//! Tera: loading templates and building contexts.
//!
//! Templates are **embedded into the binary** with `include_str!`. Giving up hot reload buys
//! three better things: the server runs from any working directory, a mistyped template name
//! is a startup error rather than a user click, and a deployment is a single file.

use std::collections::HashMap;
use std::sync::Arc;

use dnqatv_core::search::SearchMode;
use tera::{Context, Tera};

use crate::error::WebError;
use crate::state::SiteMeta;
use crate::view::LetterChipView;

/// Template names — declared once, used both when loading and when rendering.
pub mod name {
    pub const BASE: &str = "base.html";
    pub const HOME: &str = "home.html";
    pub const SEARCH: &str = "search.html";
    pub const ENTRY: &str = "entry.html";
    pub const LETTER: &str = "letter.html";
    pub const GLYPH: &str = "glyph.html";
    pub const PAGE: &str = "page.html";
    pub const ABOUT: &str = "about.html";
    pub const QUALITY: &str = "quality.html";
    pub const NOT_FOUND: &str = "not_found.html";
    pub const ERROR: &str = "error.html";
    pub const API_DOCS: &str = "api_docs.html";
    /// Reusable fragments.
    pub const PARTIAL_ENTRY_ROW: &str = "partial/entry_row.html";
    pub const PARTIAL_SEARCH_BOX: &str = "partial/search_box.html";
    pub const PARTIAL_LETTER_CHIPS: &str = "partial/letter_chips.html";
    /// The left navigation rail — inserted by the page frame, present on every page.
    pub const PARTIAL_RAIL: &str = "partial/rail.html";
}

const FILES: [(&str, &str); 13] = [
    (
        name::BASE,
        include_str!("../../../frontend/templates/base.html"),
    ),
    (
        name::HOME,
        include_str!("../../../frontend/templates/home.html"),
    ),
    (
        name::SEARCH,
        include_str!("../../../frontend/templates/search.html"),
    ),
    (
        name::ENTRY,
        include_str!("../../../frontend/templates/entry.html"),
    ),
    (
        name::LETTER,
        include_str!("../../../frontend/templates/letter.html"),
    ),
    (
        name::GLYPH,
        include_str!("../../../frontend/templates/glyph.html"),
    ),
    (
        name::PAGE,
        include_str!("../../../frontend/templates/page.html"),
    ),
    (
        name::ABOUT,
        include_str!("../../../frontend/templates/about.html"),
    ),
    (
        name::QUALITY,
        include_str!("../../../frontend/templates/quality.html"),
    ),
    (
        name::NOT_FOUND,
        include_str!("../../../frontend/templates/not_found.html"),
    ),
    (
        name::ERROR,
        include_str!("../../../frontend/templates/error.html"),
    ),
    (
        name::API_DOCS,
        include_str!("../../../frontend/templates/api_docs.html"),
    ),
    (
        name::PARTIAL_ENTRY_ROW,
        include_str!("../../../frontend/templates/partial/entry_row.html"),
    ),
];

/// The CSS is embedded too, for the same reason — and because it is small.
pub const STYLESHEET: &str = include_str!("../../../frontend/styles/main.css");
/// The progressive-enhancement JS. The site works fully without it.
pub const SCRIPT: &str = include_str!("../../../frontend/styles/enhance.js");

pub struct Templates {
    tera: Tera,
    site: Arc<SiteMeta>,
}

impl Templates {
    /// Load and **compile immediately**. A template syntax error fires at startup, not while serving.
    pub fn load(site: Arc<SiteMeta>) -> Result<Self, tera::Error> {
        let mut tera = Tera::default();
        tera.add_raw_templates(FILES.to_vec())?;
        // These small fragments are `include`d from several pages; loaded separately for clarity.
        tera.add_raw_template(
            name::PARTIAL_SEARCH_BOX,
            include_str!("../../../frontend/templates/partial/search_box.html"),
        )?;
        tera.add_raw_template(
            name::PARTIAL_LETTER_CHIPS,
            include_str!("../../../frontend/templates/partial/letter_chips.html"),
        )?;
        tera.add_raw_template(
            name::PARTIAL_RAIL,
            include_str!("../../../frontend/templates/partial/rail.html"),
        )?;
        tera.register_filter("so", format_number);
        tera.autoescape_on(vec![".html"]);
        Ok(Self { tera, site })
    }

    /// The base context, present on every page.
    pub fn context(&self) -> Context {
        let mut ctx = Context::new();
        ctx.insert("site_title", self.site.title);
        ctx.insert("site_subtitle", self.site.subtitle);
        ctx.insert("base_url", &self.site.base_url);
        ctx.insert("favicon_path", crate::assets::FAVICON_PATH);
        // `None` when no font is shipped: the template drops the preload tag entirely
        // rather than pointing at nothing.
        // The MIME type follows the REAL format of the file. Hard-coding `font/woff2` while
        // shipping TTF lies to the browser about what it is fetching.
        ctx.insert(
            "font_preload_type",
            &self
                .site
                .fonts
                .file(crate::assets::FontRange::Bmp)
                .map(|f| f.mime()),
        );
        // Whether a Nôm font is shipped. Without one, the 26 PUA glyphs are guaranteed to
        // render as squares on every machine, and the page must say so rather than let the
        // reader think the data is broken.
        ctx.insert("has_nom_font", &!self.site.fonts.is_empty());

        // Navigation lives in the page FRAME, so it belongs to the base context rather than
        // to each handler. Previously every handler injected `modes` itself (ten copies) and
        // three injected `letters`; a new page could easily forget, leaving its left column
        // empty. Here no page can forget.
        ctx.insert("letters", &LetterChipView::all(None));
        ctx.insert("modes", &search_modes());
        ctx
    }

    pub fn render(&self, template: &str, ctx: &Context) -> Result<String, WebError> {
        self.tera
            .render(template, ctx)
            .map_err(|e| WebError::Render(format!("{template}: {}", describe(&e))))
    }
}

/// The `| so` filter: thousands separators in the Vietnamese style.
///
/// `7687` is harder to read than `7.687`, and both the plan and every passage of UI copy
/// write numbers with dots. Letting a template print a raw number would put two conventions
/// on one page. Declared here rather than pre-formatted in the DTO: the DTO is the public
/// API contract, and there a number must stay a NUMBER for machines, not a decorated string.
fn format_number(
    value: &tera::Value,
    _: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let Some(n) = value.as_u64() else {
        // Not an integer: return it untouched. This filter has nothing to do, and guessing
        // at some other formatting is worse than doing nothing.
        return Ok(value.clone());
    };
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    Ok(tera::Value::String(out))
}

/// Flatten the cause chain of a Tera error into one readable line.
///
/// Tera wraps the real error in `source`, while the outermost layer only says
/// `Failed to render 'x.html'` — which names neither the missing variable nor the missing
/// template. Ignoring `source` means guessing on every failure, and guessing is what this
/// project forbids.
fn describe(error: &tera::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut cause: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(error);
    while let Some(e) = cause {
        parts.push(e.to_string());
        cause = e.source();
    }
    parts.join(" ← ")
}

/// The four search modes, in a form Tera can read.
///
/// Generated from [`SearchMode::ALL`], so adding a mode makes it appear in the left column —
/// there is no second list that could drift from the domain layer.
pub fn search_modes() -> Vec<serde_json::Value> {
    SearchMode::ALL
        .iter()
        .map(|m| serde_json::json!({ "value": m.as_param(), "label": m.label() }))
        .collect()
}
