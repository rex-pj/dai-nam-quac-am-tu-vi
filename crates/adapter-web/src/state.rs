//! Shared state of the web layer.
//!
//! `FromRef` is how the **I** in SOLID surfaces in Axum: a handler that only renders a
//! printed page declares `State<Arc<dyn PageReader>>` and **cannot call** a search function.
//! If everything took `State<AppState>`, every handler could reach everything and the
//! boundary would be a promise again.

use std::sync::Arc;

use axum::extract::FromRef;
use dnqatv_app::service::{BrowseService, DictionaryService};

use crate::assets::FontSet;
use crate::template::Templates;

#[derive(Clone)]
pub struct AppState {
    pub dictionary: DictionaryService,
    pub browse: BrowseService,
    pub templates: Arc<Templates>,
    pub site: Arc<SiteMeta>,
}

/// Immutable facts about the site, used by every template and by the sitemap.
pub struct SiteMeta {
    pub title: &'static str,
    pub subtitle: &'static str,
    /// The origin, for the sitemap and canonical tags. Taken from configuration, never
    /// guessed from the `Host` header — that header is caller-supplied and untrustworthy.
    pub base_url: String,
    /// The font files actually present on disk. The page declares `@font-face` and `preload`
    /// only for these, so it never points at a file that does not exist.
    pub fonts: FontSet,
    /// The directory holding those font files.
    pub font_dir: std::path::PathBuf,
}

impl SiteMeta {
    pub fn new(base_url: String, font_dir: std::path::PathBuf) -> Self {
        Self {
            title: "Đại Nam Quấc Âm Tự Vị",
            subtitle: "Huình-Tịnh Paulus Của · 1895–1896",
            base_url,
            fonts: FontSet::scan(&font_dir),
            font_dir,
        }
    }
}

impl FromRef<AppState> for DictionaryService {
    fn from_ref(s: &AppState) -> Self {
        s.dictionary.clone()
    }
}

impl FromRef<AppState> for BrowseService {
    fn from_ref(s: &AppState) -> Self {
        s.browse.clone()
    }
}

impl FromRef<AppState> for Arc<Templates> {
    fn from_ref(s: &AppState) -> Self {
        s.templates.clone()
    }
}

impl FromRef<AppState> for Arc<SiteMeta> {
    fn from_ref(s: &AppState) -> Self {
        s.site.clone()
    }
}
