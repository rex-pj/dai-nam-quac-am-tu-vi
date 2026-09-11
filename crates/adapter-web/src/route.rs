//! Paths — declared **once**, used by the router, the templates and the sitemap alike.
//!
//! All three must speak the same string. Spread across three places, the day a path changes
//! exactly two of them get updated and the third breaks silently — a sitemap pointing at a
//! 404 kills SEO with no test turning red.
//!
//! Paths are written in **unaccented Vietnamese**: a URL is something people read and retype.

/// The patterns for `Router::route` — axum 0.8 `{param}` syntax.
pub mod pattern {
    pub const HOME: &str = "/";
    pub const SEARCH: &str = "/tra-tim";
    pub const ENTRY: &str = "/muc-tu/{slug}";
    pub const LETTER: &str = "/van/{letter}";
    pub const GLYPH: &str = "/chu/{glyph}";
    pub const PAGE: &str = "/trang/{page}";
    pub const ABOUT: &str = "/gioi-thieu";
    pub const FRONT_MATTER: &str = "/gioi-thieu/{slug}";
    pub const QUALITY: &str = "/pham-chat-du-lieu";
    pub const SITEMAP: &str = "/sitemap.xml";
    pub const ROBOTS: &str = "/robots.txt";
    pub const HEALTH: &str = "/health";

    pub const API_SEARCH: &str = "/api/v1/tra-tim";
    pub const API_ENTRY: &str = "/api/v1/muc-tu/{slug}";
    pub const API_GLYPH: &str = "/api/v1/chu/{glyph}";
    pub const API_PAGE: &str = "/api/v1/trang/{page}";
    pub const API_STATS: &str = "/api/v1/pham-chat-du-lieu";
    pub const API_SPEC: &str = "/api/v1/openapi.json";
    pub const API_DOCS: &str = "/api/v1";
}

/// Paths that used to be live, kept only to redirect.
///
/// Two pages were renamed when the site's wording was moved to the vocabulary of the book
/// itself: `chat-luong` → `pham-chat` (the author never writes *chất lượng*), and `tra-cuu`
/// → `tra-tim`, the word he uses in TIỂU TỰ — *"sắp đặt theo thứ lớp cho dễ việc tra tìm."*
///
/// Old links must not 404: `301` sends them to the new path, query string included, and the
/// sitemap lists the new path only.
pub mod retired {
    /// `(old path, current path)` — every entry gets a `301` route.
    pub const REDIRECTS: [(&str, &str); 4] = [
        ("/chat-luong-du-lieu", super::pattern::QUALITY),
        ("/api/v1/chat-luong-du-lieu", super::pattern::API_STATS),
        ("/tra-cuu", super::pattern::SEARCH),
        ("/api/v1/tra-cuu", super::pattern::API_SEARCH),
    ];

    /// The `Location` a retired path redirects to.
    ///
    /// The query has to come along. `/tra-cuu?q=lõm` sent to a bare `/tra-tim` would drop the
    /// reader's search and land them on an empty box — which reads as the site losing their
    /// query, not as a path that moved. Split out from the handler so it can be tested
    /// without standing up a router.
    pub fn location(current: &str, query: Option<&str>) -> String {
        match query {
            Some(q) if !q.is_empty() => format!("{current}?{q}"),
            _ => current.to_owned(),
        }
    }
}

/// Build concrete URLs. Templates call these instead of concatenating strings.
pub mod url {
    use dnqatv_core::model::PdfPage;

    pub fn entry(slug: &str) -> String {
        format!("/muc-tu/{slug}")
    }

    pub fn letter(letter: &str) -> String {
        format!("/van/{}", urlencode(letter))
    }

    pub fn glyph(glyph: char) -> String {
        format!("/chu/{}", urlencode(&glyph.to_string()))
    }

    /// The one URL of a page.
    ///
    /// Takes a [`PdfPage`] and asks it for its address, so no call site can pass a bare
    /// number and silently mean the wrong numbering system — the two differ by one over
    /// 1,028 of the 1,038 pages, which is exactly the kind of off-by-one no test catches.
    pub fn page(page: PdfPage) -> String {
        format!("/trang/{}", page.address())
    }

    pub fn front_matter(slug: &str) -> String {
        format!("/gioi-thieu/{slug}")
    }

    pub fn search(query: &str) -> String {
        format!("{}?q={}", super::pattern::SEARCH, urlencode(query))
    }

    /// Percent-encode a path segment.
    ///
    /// Hand-written rather than pulling in a crate: the URL-safe character set is small and
    /// fixed, and Han-Nom always needs encoding, so the multi-byte branch is the main path,
    /// not the exception.
    pub fn urlencode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.as_bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(*b as char);
                }
                b' ' => out.push_str("%20"),
                other => out.push_str(&format!("%{other:02X}")),
            }
        }
        out
    }
}
