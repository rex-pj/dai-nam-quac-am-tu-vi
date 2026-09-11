#![allow(clippy::expect_used, clippy::panic)]

//! Templates and view models — they run without a database.
//!
//! A template bug is the kind that only appears at runtime: one mistyped variable breaks that
//! page while every other page stays green, so it survives every other check until someone
//! clicks the wrong link. This test catches both kinds: syntax (at `load`) and missing variables (at render).

use std::sync::Arc;

use dnqatv_adapter_web::assets::{FONT_URL_PREFIX, FontRange};
use dnqatv_adapter_web::template::{Templates, name};
use dnqatv_adapter_web::view::{FormPart, GlyphView, LetterChipView, PagedCountView, split_form};
use dnqatv_adapter_web::{FontSet, SiteMeta, route};
use dnqatv_core::model::page::PDF_PAGE_COUNT;
use dnqatv_core::model::{GlyphChar, Letter, PageAddress, PdfPage};
use dnqatv_core::search::{Paged, Pagination};

fn templates() -> Templates {
    // The font directory points at nothing: exactly the current state of the repo, and also
    // the state most worth testing — the page must render with no font at all.
    Templates::load(Arc::new(SiteMeta::new(
        "https://example.test".to_owned(),
        std::path::PathBuf::from("khong-co-thu-muc-nay"),
    )))
    .expect("every template must compile")
}

#[test]
fn every_template_compiles() {
    // `load` compiles them all; an unbalanced brace fails here rather than while serving.
    let _ = templates();
}

#[test]
fn the_not_found_page_renders_with_the_minimal_context() {
    let t = templates();
    let mut ctx = t.context();
    ctx.insert("letters", &LetterChipView::all(None));
    ctx.insert("modes", &Vec::<serde_json::Value>::new());
    let html = t.render(name::NOT_FOUND, &ctx).expect("rendering the page");
    assert!(html.contains("Không tìm thấy"));
    // The error page must still carry a search box: a reader arriving here is lost.
    assert!(html.contains("form"), "there must be a search box");
}

#[test]
fn the_error_page_leaks_no_technical_detail() {
    let t = templates();
    let html = t
        .render(name::ERROR, &t.context())
        .expect("rendering the page");
    for leak in ["SELECT", "postgres", "sqlx", "panic"] {
        assert!(!html.contains(leak), "the error page leaked {leak:?}");
    }
}

// ── Splitting the form for <abbr> wrapping ───────────────────────────────────

fn texts(parts: &[FormPart]) -> Vec<(&str, bool)> {
    parts
        .iter()
        .map(|p| (p.text.as_str(), p.placeholder))
        .collect()
}

#[test]
fn splitting_a_form_separates_the_placeholder() {
    assert_eq!(
        texts(&split_form("― gươm")),
        vec![("―", true), (" gươm", false)]
    );
    assert_eq!(
        texts(&split_form("Chính giữa ―")),
        vec![("Chính giữa ", false), ("―", true)]
    );
}

#[test]
fn splitting_a_form_accepts_all_four_placeholder_codes() {
    // Four code points, not one: `―` U+2015, `—` U+2014, `–` U+2013, `|`. Hard-coding one
    // loses the 540 lines that open with `—`.
    for mark in ["―", "—", "–", "|"] {
        let parts = split_form(&format!("{mark} x"));
        assert!(
            parts[0].placeholder,
            "{mark:?} not recognised as a placeholder"
        );
    }
}

#[test]
fn the_two_dash_form_is_checked_before_the_single_dash() {
    // Checking single characters first would make `--` two placeholders.
    let parts = split_form("-- gươm");
    assert_eq!(parts[0].text, "--");
    assert!(parts[0].placeholder);
    assert_eq!(parts.len(), 2);
}

#[test]
fn every_occurrence_is_split_not_just_the_first() {
    // 3.27% of forms have two placeholders, some have six.
    let parts = split_form("Nhứt ― trọng, nhứt ― khinh");
    assert_eq!(parts.iter().filter(|p| p.placeholder).count(), 2);
}

#[test]
fn a_form_with_no_placeholder_yields_a_single_piece() {
    let parts = split_form("Lõm gươm");
    assert_eq!(texts(&parts), vec![("Lõm gươm", false)]);
}

// ── Glyphs ───────────────────────────────────────────────────────────────────

#[test]
fn an_ext_b_glyph_is_marked_as_needing_a_shipped_font() {
    let glyph = GlyphChar::parse("𨰲").expect("valid glyph");
    let view = GlyphView::new(&glyph, "Lõm");
    assert_eq!(view.kind, "ext_b");
    assert!(
        view.needs_shipped_font,
        "user machines usually lack an Ext-B font"
    );
    assert!(!view.no_unicode);
    assert!(
        view.aria_label.contains("Lõm"),
        "a screen reader must be able to read it"
    );
}

#[test]
fn a_bmp_glyph_needs_no_shipped_font() {
    let glyph = GlyphChar::parse("阿").expect("valid glyph");
    let view = GlyphView::new(&glyph, "A");
    assert_eq!(view.kind, "bmp");
    assert!(!view.needs_shipped_font);
}

#[test]
fn an_image_entry_is_never_replaced_by_a_lookalike() {
    let view = GlyphView::image_only("Chày");
    assert_eq!(view.kind, "image_only");
    assert!(view.no_unicode);
    assert!(view.char.is_empty(), "no character may be filled in here");
    assert!(view.aria_label.contains("chưa có mã Unicode"));
}

// ── Letter chips ─────────────────────────────────────────────────────────────

#[test]
fn twenty_two_chips_in_the_printed_order() {
    let chips = LetterChipView::all(Some(Letter::Đ));
    assert_eq!(chips.len(), 22);
    let letters: Vec<&str> = chips.iter().map(|c| c.letter.as_str()).collect();
    assert_eq!(
        letters,
        vec![
            "A", "B", "C", "D", "Đ", "E", "G", "H", "Y", "K", "L", "M", "N", "O", "P", "Q", "R",
            "S", "T", "U", "V", "X"
        ]
    );
    assert_eq!(chips.iter().filter(|c| c.active).count(), 1);
    assert!(
        chips
            .iter()
            .find(|c| c.letter == "Đ")
            .expect("the letter Đ is present")
            .active
    );
}

// ── Paths ────────────────────────────────────────────────────────────────────

fn pdf(n: u16) -> PdfPage {
    PdfPage::new(n).expect("trang PDF trong khoảng")
}

/// Every page has an address, and reading it back gives the same page.
///
/// This is the pair the screenshot bug lived in: the heading printed one number and the
/// URL another, so a reader who typed the number they saw landed one page off.
#[test]
fn the_url_of_a_page_reads_back_as_that_page() {
    for n in 1..=PDF_PAGE_COUNT {
        let page = pdf(n);
        let url = route::url::page(page);
        let segment = url
            .strip_prefix("/trang/")
            .unwrap_or_else(|| panic!("{url} phải bắt đầu bằng /trang/"));
        assert_eq!(
            PageAddress::parse(segment).expect(segment).pdf_page(),
            page,
            "trang PDF {n}"
        );
    }
}

#[test]
fn the_url_carries_the_number_printed_on_the_page() {
    // The three unnumbered pages — both covers and the inserted LƯU Ý page — are the only
    // ones addressed by PDF number, and they say so in the URL.
    assert_eq!(route::url::page(pdf(1)), "/trang/pdf-1");
    assert_eq!(route::url::page(pdf(2)), "/trang/pdf-2");
    assert_eq!(route::url::page(pdf(10)), "/trang/pdf-10");
    // Front matter: the two numberings coincide.
    assert_eq!(route::url::page(pdf(3)), "/trang/3");
    assert_eq!(route::url::page(pdf(9)), "/trang/9");
    // The body is shifted by one — PDF 11 is the page that prints "10".
    assert_eq!(route::url::page(pdf(11)), "/trang/10");
    assert_eq!(route::url::page(pdf(PDF_PAGE_COUNT)), "/trang/1037");
}

#[test]
fn han_nom_is_correctly_encoded_in_a_url() {
    // 𨰲 is outside the BMP, four UTF-8 bytes.
    assert_eq!(route::url::glyph('\u{28C32}'), "/chu/%F0%A8%B0%B2");
    assert_eq!(route::url::entry("lom-4"), "/muc-tu/lom-4");
    // PDF 500 is printed 499: the URL carries the number on the paper, not the PDF index.
    assert_eq!(route::url::page(pdf(500)), "/trang/499");
}

#[test]
fn a_query_with_special_characters_still_encodes() {
    let url = route::url::search("nạm gươm");
    assert!(!url.contains(' '), "spaces must be encoded: {url}");
    assert!(url.starts_with("/tra-tim?q="));
}

#[test]
fn the_router_paths_and_the_generated_paths_agree() {
    // The two sets must say the same thing; this is where they are compared.
    assert!(route::pattern::ENTRY.starts_with("/muc-tu/"));
    assert!(route::url::entry("x").starts_with("/muc-tu/"));
    assert!(route::pattern::PAGE.starts_with("/trang/"));
    assert!(route::url::page(pdf(1)).starts_with("/trang/"));
    assert!(route::pattern::GLYPH.starts_with("/chu/"));
    assert!(route::url::glyph('阿').starts_with("/chu/"));
}

// ── The API specification ────────────────────────────────────────────────────

#[test]
fn the_openapi_spec_is_valid_and_points_at_endpoints_that_exist() {
    let spec = dnqatv_adapter_web::openapi::document();
    let parsed: serde_json::Value = serde_json::from_str(&spec).expect("must be valid JSON");
    let paths = parsed.get("paths").expect("a paths section exists");
    for p in [
        route::pattern::API_SEARCH,
        route::pattern::API_ENTRY,
        route::pattern::API_GLYPH,
        route::pattern::API_PAGE,
        route::pattern::API_STATS,
    ] {
        assert!(paths.get(p).is_some(), "the spec is missing {p}");
    }
    // The contract must name both the verbatim and derived fields, the easiest pair to misuse.
    assert!(spec.contains("form_expanded"));
}

#[test]
fn no_template_still_links_to_a_retired_path() {
    // The quality page was renamed `chat-luong` → `pham-chat` when the site's wording was
    // moved to the vocabulary of the book. A template left pointing at the old string would
    // send every reader through a redirect, and would 404 the day the redirect is dropped.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");

    let mut stale: Vec<String> = Vec::new();
    for dir in [root.clone(), root.join("partial")] {
        for entry in std::fs::read_dir(&dir).expect("reading the template directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|e| e == "html") {
                let html = std::fs::read_to_string(&path).expect("reading the template");
                for (old, _) in route::retired::REDIRECTS {
                    if html.contains(old) {
                        stale.push(format!("{}: {old}", path.display()));
                    }
                }
            }
        }
    }
    assert!(
        stale.is_empty(),
        "templates still linking to a retired path: {stale:#?}"
    );

    let served = [
        route::pattern::QUALITY,
        route::pattern::API_STATS,
        route::pattern::SEARCH,
        route::pattern::API_SEARCH,
    ];
    for (old, current) in route::retired::REDIRECTS {
        assert_ne!(old, current, "a path cannot redirect to itself");
        assert!(
            served.contains(&current),
            "{old} redirects to {current}, which no route serves"
        );
        assert!(
            !current.contains('{'),
            "a redirect target must be a concrete path, not a router pattern: {current}"
        );
    }
}

#[test]
fn a_retired_path_carries_the_query_string_to_its_new_home() {
    // The bug this guards: `/tra-cuu?q=lõm` redirected to a bare `/tra-tim` drops the reader's
    // search. They land on an empty box and read it as the site losing their query — the one
    // failure a rename is supposed to avoid, and one no compiler and no static scan can catch.
    use dnqatv_adapter_web::route::retired::location;

    assert_eq!(
        location(route::pattern::SEARCH, Some("q=l%C3%B5m")),
        "/tra-tim?q=l%C3%B5m"
    );
    assert_eq!(
        location(
            route::pattern::SEARCH,
            Some("q=n%E1%BA%A1m&che_do=toan-van")
        ),
        "/tra-tim?q=n%E1%BA%A1m&che_do=toan-van",
        "every parameter comes along, not just the first"
    );
    // No query, and the empty query, both give a clean path — never a dangling `?`.
    assert_eq!(location(route::pattern::SEARCH, None), "/tra-tim");
    assert_eq!(location(route::pattern::SEARCH, Some("")), "/tra-tim");
}

// ── Two real bugs, kept as gates so they cannot return ───────────────────────

#[test]
fn every_class_used_in_a_template_exists_in_the_css() {
    // The bug: `.visually-hidden` was used for the search box label but never defined, so
    // the word "Tra cứu" appeared in the middle of the page instead of being screen-reader
    // only. No compiler catches this, so it needs a test.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend");
    let css = std::fs::read_to_string(root.join("styles/main.css")).expect("reading the CSS");

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for dir in ["templates", "templates/partial"] {
        for entry in std::fs::read_dir(root.join(dir)).expect("reading the template directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|e| e == "html") {
                files.push(path);
            }
        }
    }
    assert!(files.len() >= 12, "every template must be scanned");

    let mut missing: Vec<String> = Vec::new();
    for file in &files {
        let html = std::fs::read_to_string(file).expect("reading the template");
        for chunk in html.split("class=\"").skip(1) {
            let Some((value, _)) = chunk.split_once('"') else {
                continue;
            };
            // Skip values Tera builds at runtime — a static test cannot know what they become.
            if value.contains("{{") || value.contains("{%") {
                continue;
            }
            for class in value.split_whitespace() {
                if !css.contains(&format!(".{class}")) {
                    missing.push(format!("{}: .{class}", file.display()));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "classes used with no CSS rule: {missing:#?}"
    );
}

#[test]
fn pagination_links_use_the_url_value_not_the_display_label() {
    // The bug: the pagination template built links from the LOWERCASED LABEL. The letter Đ
    // displays as "Đ" but is "dd" in a URL, so every pagination link for Đ pointed at
    // /van/đ and returned 404 — 398 entries lost their pages.
    for letter in Letter::ALL {
        assert_eq!(
            Letter::from_db_value(letter.db_value()).expect("round trips"),
            letter
        );
        let url = route::url::letter(letter.db_value());
        assert!(url.starts_with("/van/"), "{url}");
        // The lowercased label is NOT always the path segment — that is the trap.
        if letter == Letter::Đ {
            assert_ne!(letter.label().to_lowercase(), letter.db_value());
        }
    }
}

#[test]
fn the_pagination_template_does_not_lowercase_the_label_itself() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/templates/letter.html");
    let html = std::fs::read_to_string(path).expect("reading the template");
    assert!(
        !html.contains("letter | lower"),
        "it must use letter_slug, not the lowercased label"
    );
    assert!(html.contains("letter_slug"));
}

fn letter_page(shown: usize, total: u64, offset: u64) -> String {
    let paged = Paged {
        items: vec![(); shown],
        total,
        page: Pagination::new(offset, 30),
    };
    let t = templates();
    let mut ctx = t.context();
    ctx.insert("letter", "A");
    ctx.insert("letter_slug", "a");
    ctx.insert("count", &PagedCountView::new(&paged));
    ctx.insert("entries", &Vec::<serde_json::Value>::new());
    ctx.insert("has_more", &paged.has_more());
    ctx.insert("next_page", &2u64);
    ctx.insert("previous_page", &0u64);
    t.render(name::LETTER, &ctx).expect("rendering the page")
}

#[test]
fn the_letter_heading_counts_the_rows_on_screen_against_the_whole_letter() {
    // "74 chữ đầu" over the thirty rows a page actually holds reads as a contradiction: the
    // reader counts the rows, finds thirty, and has no way to tell the other forty-four exist.
    let html = letter_page(30, 74, 0);
    assert!(
        html.contains("30 / 74 chữ đầu"),
        "the heading must count what is on screen: {html}"
    );
}

#[test]
fn a_letter_that_fits_on_one_page_states_its_total_alone() {
    // "7 / 7" is noise: nothing is off screen to account for.
    let html = letter_page(7, 7, 0);
    assert!(
        html.contains(r#"<span class="muted">7 chữ đầu</span>"#),
        "the heading must state the total on its own: {html}"
    );
}

#[test]
fn the_counts_in_the_letter_heading_carry_thousands_separators() {
    // Vần C runs to thousands of entries, and every other number in the UI is written 7.687.
    let html = letter_page(30, 1_234, 0);
    assert!(
        html.contains("30 / 1.234 chữ đầu"),
        "the total must be grouped like every other number: {html}"
    );
}

#[test]
fn every_glyph_carries_a_language_tag() {
    // Nôm script is not Chinese. Without `lang`, a screen reader guesses Chinese and reads
    // it with Mandarin pronunciation.
    let glyph = GlyphChar::parse("𨰲").expect("valid glyph");
    assert_eq!(GlyphView::new(&glyph, "Lõm").lang, "vi-Hani");
    assert_eq!(GlyphView::image_only("Chày").lang, "vi-Hani");
}

#[test]
fn a_section_heading_and_its_body_share_one_width() {
    // The bug: the rule under `.section-head` ran the full 52rem while the paragraph below
    // it was only 34rem wide, leaving an 18rem gap that looked like missing content. The fix
    // is wrapping both in `.section--prose`; this test keeps new pages from forgetting.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");
    let mut offenders: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&dir).expect("reading the template directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "html") {
            continue;
        }
        let html = std::fs::read_to_string(&path).expect("reading the template");

        let mut wrapped = false;
        let mut heading_open = false;
        for (i, line) in html.lines().enumerate() {
            // Match the attribute as well as the name: a COMMENT mentioning `section--prose`
            // once made this test believe the wrapper was there, so it went green while the
            // real page stayed broken.
            if line.contains("class=\"section--prose\"") {
                wrapped = true;
            } else if line.contains("</section>") {
                wrapped = false;
            }
            if line.contains("class=\"section-head\"") {
                heading_open = true;
                continue;
            }
            // Data tables and entry lists may span the full column; only prose is width
            // limited, so only a heading/prose pair can mismatch.
            if heading_open && line.contains("class=\"prose\"") && !wrapped {
                offenders.push(format!("{}:{}", path.display(), i + 1));
            }
            if line.contains("class=\"prose\"") || line.contains("<table") {
                heading_open = false;
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a section heading is wider than its prose, missing the .section--prose wrapper: {offenders:#?}"
    );
}

// ── Never point at a file that does not exist ────────────────────────────────

/// Strip CSS comments before asserting.
///
/// The two tests below once failed wrongly by matching `@font-face` inside a COMMENT that
/// explained no `@font-face` is declared here. Earlier, another test passed wrongly for the
/// same reason. A comment is not a rule — strip it first, then judge.
fn without_comments(css: &str) -> String {
    without_spans(css, "/*", "*/")
}

/// The same, for the `{# … #}` comments of a template.
///
/// Stripping them line by line does not work: a comment runs over several lines, and only its
/// first one starts with `{#`. A test that filtered the continuation by matching a phrase from
/// the comment went red the day that comment was rewritten.
fn without_template_comments(html: &str) -> String {
    without_spans(html, "{#", "#}")
}

fn without_spans(text: &str, open_mark: &str, close_mark: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(open_mark) {
        out.push_str(&rest[..open]);
        match rest[open..].find(close_mark) {
            Some(close) => rest = &rest[open + close + close_mark.len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The declarations of one rule, by its exact selector.
fn rule_body<'a>(css: &'a str, selector: &str) -> &'a str {
    let at = css
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("no rule for {selector}"));
    let open = css[at..].find('{').expect("an opening brace") + at;
    let close = css[open..].find('}').expect("a closing brace") + open;
    &css[open + 1..close]
}

fn rem(body: &str, property: &str) -> f32 {
    let at = body
        .find(&format!("{property}:"))
        .unwrap_or_else(|| panic!("no {property} in {body}"));
    let value = &body[at + property.len() + 1..];
    let value = value[..value.find(';').expect("a terminating semicolon")].trim();
    value
        .trim_end_matches("rem")
        .parse()
        .unwrap_or_else(|_| panic!("{property} is {value:?}, not a rem value"))
}

#[test]
fn the_note_beside_a_section_heading_speaks_quieter_than_the_heading() {
    // The bug: the slot to the right of a section heading was styled by nothing at all, so it
    // inherited the body serif at 1rem while the heading beside it is sans 0.75rem uppercase.
    // The afterthought — "1–30 trong 74 chữ đầu" — printed BIGGER than the title it belongs
    // to, and the two ends of one line spoke in two different voices.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);
    let note = rule_body(&css, ".section-head > span");
    let heading = rule_body(&css, ".section-head :is(h1, h2)");

    assert!(
        note.contains("font-family: var(--sans)"),
        "the note must share the heading typeface: {note}"
    );
    assert!(
        rem(note, "font-size") < rem(heading, "font-size"),
        "the note must be smaller than the heading it annotates"
    );
    // Counts change under the reader as they page; same-width digits keep them from shifting.
    assert!(note.contains("tabular-nums"), "{note}");
}

#[test]
fn no_font_is_declared_while_no_file_is_shipped() {
    // The bug: the CSS declared three @font-face rules and base.html preloaded a file while
    // frontend/fonts/ was empty. A broken @font-face fails silently, but a preload 404s
    // loudly in the console on EVERY page load.
    let none = FontSet::scan(std::path::Path::new("khong-co-thu-muc-nay"));
    assert!(none.is_empty());
    assert_eq!(
        none.preload_url(),
        None,
        "with no file there must be no preload"
    );

    let css = without_comments(&none.font_face_css());
    assert!(
        !css.contains("@font-face"),
        "no font is shipped yet an @font-face is declared: {css}"
    );
    assert!(!css.contains(".woff2"), "it must point at no file: {css}");
}

#[test]
fn the_static_stylesheet_no_longer_points_at_any_font_file() {
    // @font-face must be generated at runtime by `assets`; a static declaration points blind.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);
    assert!(
        !css.contains("@font-face"),
        "main.css must not declare @font-face itself"
    );
    assert!(
        !css.contains(".woff2"),
        "main.css must not point at a font file"
    );
}

#[test]
fn every_page_declares_an_icon_so_the_browser_does_not_ask_for_favicon_ico() {
    let t = templates();
    let mut ctx = t.context();
    ctx.insert("modes", &Vec::<serde_json::Value>::new());
    ctx.insert("letters", &LetterChipView::all(None));
    let html = t.render(name::NOT_FOUND, &ctx).expect("rendering the page");
    assert!(html.contains("rel=\"icon\""), "the icon tag is missing");
    assert!(html.contains("image/svg+xml"));
    // And there must be no preload tag while no font is shipped.
    assert!(!html.contains("rel=\"preload\""), "preloading nothing");
}

#[test]
fn the_icon_is_valid_svg_and_independent_of_user_machine_fonts() {
    let svg = dnqatv_adapter_web::FAVICON_SVG;
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("</svg>"));
    // Drawn as a shape, not text: a Han character in <text> depends on the viewer machine font.
    assert!(
        !svg.contains("<text"),
        "the icon must not be drawn with text"
    );
}

#[test]
fn fonts_are_declared_when_a_real_file_exists() {
    // Build a fake directory with exactly one file to be sure the "font present" branch works too.
    let dir = std::env::temp_dir().join("dnqatv-font-test");
    std::fs::create_dir_all(&dir).expect("creating the directory");
    let file = dir.join("NomNaTong-bmp.woff2");
    std::fs::write(&file, b"khong-phai-font-that").expect("ghi file");

    let set = FontSet::scan(&dir);
    assert_eq!(set.len(), 1, "only one range has a file");
    assert!(set.preload_url().is_some());
    let css = without_comments(&set.font_face_css());
    assert_eq!(
        css.matches("@font-face").count(),
        1,
        "only the range with a file is declared"
    );
    assert!(css.contains("NomNaTong-bmp.woff2"));
    assert!(
        !css.contains("NomNaTong-extb.woff2"),
        "a range with no file must not be declared"
    );

    std::fs::remove_file(&file).expect("cleanup");
}

// ── The left navigation rail ─────────────────────────────────────────────────

#[test]
fn navigation_is_declared_once_in_the_page_frame() {
    // Before the rail existed, 7 of 12 templates inserted their own search box and 4 their
    // own letter chips. Centralising it only means anything if NO page inserts it again —
    // otherwise there are two search boxes, two `id="q"`, and the reader must pick one.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");
    let mut offenders: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&dir).expect("reading the template directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "html") {
            continue;
        }
        let html = std::fs::read_to_string(&path).expect("reading the template");
        if html.contains("partial/search_box.html") || html.contains("partial/letter_chips.html") {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "a body template re-inserts navigation: {offenders:#?}"
    );

    // And exactly one place IS allowed to insert it: the rail.
    let rail = std::fs::read_to_string(dir.join("partial/rail.html")).expect("reading the rail");
    assert!(rail.contains("partial/search_box.html"));
    assert!(rail.contains("partial/letter_chips.html"));
}

#[test]
fn the_page_frame_always_includes_the_rail() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");
    let base = std::fs::read_to_string(dir.join("base.html")).expect("reading base");
    assert!(
        base.contains("partial/rail.html"),
        "the page frame must include the rail"
    );
    // The skip link must come BEFORE the rail: this layout puts navigation ahead of content
    // in the DOM, so without it every page entry means tabbing past 22 letter chips.
    let skip = base.find("skip-link").expect("a skip link must exist");
    let rail = base
        .find("partial/rail.html")
        .expect("the rail must be included");
    assert!(skip < rail, "the skip link must precede the rail");
}

#[test]
fn every_page_has_full_navigation_without_a_handler_injecting_it() {
    // The base context already carries `letters` and `modes`, so a new page cannot forget them.
    let t = templates();
    let html = t
        .render(name::NOT_FOUND, &t.context())
        .expect("renders with the base context alone");

    assert!(
        html.contains("rail-search"),
        "the rail search box is missing"
    );
    assert!(
        html.contains("rail-browse"),
        "the rail browse section is missing"
    );
    // All 22 chips, in printed order.
    for letter in Letter::ALL {
        assert!(
            html.contains(&format!(">{}</a>", letter.label())),
            "letter chip {} is missing",
            letter.label()
        );
    }
    // All four search modes.
    for mode in dnqatv_core::search::SearchMode::ALL {
        assert!(
            html.contains(mode.as_param()),
            "mode {} is missing",
            mode.label()
        );
    }
}

#[test]
fn every_page_has_exactly_one_search_box() {
    // Two search boxes on a page means two `id="q"` — invalid HTML, and `<label for="q">`
    // would point at nobody knows which.
    let t = templates();
    let html = t
        .render(name::NOT_FOUND, &t.context())
        .expect("rendering the page");
    assert_eq!(
        html.matches("id=\"q\"").count(),
        1,
        "there must be exactly one search box per page"
    );
}

#[test]
fn the_mode_list_is_generated_from_the_domain_layer() {
    // Adding a `SearchMode` makes it appear in the rail; there is no second list to drift.
    let modes = dnqatv_adapter_web::template::search_modes();
    assert_eq!(modes.len(), dnqatv_core::search::SearchMode::ALL.len());
}

#[test]
fn the_ui_does_not_add_parentheses_around_the_sino_vietnamese_reading() {
    // The bug: all 43 Sino-Vietnamese readings in the book already carry parentheses — the
    // data stores "(Nha.)" exactly as printed. The result row wrapped them again into "A ((Nha.))".
    //
    // Small, but exactly the class of bug this project fights: the display layer adding
    // characters the book never printed. Gated here because no data check looks at templates.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");
    let row =
        std::fs::read_to_string(dir.join("partial/entry_row.html")).expect("reading the template");
    assert!(
        !row.contains("({{ e.alternate }})"),
        "the result row wraps parentheses around an already parenthesised reading"
    );
    assert!(
        row.contains("{{ e.alternate }}"),
        "the reading must still be shown"
    );

    let entry = std::fs::read_to_string(dir.join("entry.html")).expect("reading entry.html");
    assert!(!entry.contains("({{ entry.alternate }})"));
}

#[test]
fn every_page_has_one_h1_and_it_describes_the_page_content() {
    // The masthead repeats on every page, so it must NOT be an <h1>: otherwise the entry
    // page has two <h1> and screen readers lose the clue to the real content.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/templates");
    let base = std::fs::read_to_string(dir.join("base.html")).expect("reading base");
    let base_code = without_template_comments(&base);
    assert!(
        !base_code.contains("<h1"),
        "the page frame must carry no <h1>; each page has its own"
    );
    assert!(base_code.contains("<p class=\"site-title\">"));

    // The page renders with no page-specific context: count the <h1> in the real HTML.
    let t = templates();
    let html = t
        .render(name::NOT_FOUND, &t.context())
        .expect("rendering the page");
    assert_eq!(
        html.matches("<h1").count(),
        1,
        "there must be exactly one <h1>"
    );
}

#[test]
fn printed_numbers_carry_thousands_separators() {
    // `7687` is harder to read than `7.687`, and every other passage of UI copy writes
    // numbers with dots. Letting a template print a raw number puts two conventions on one page.
    let t = templates();
    let mut ctx = t.context();
    ctx.insert(
        "stats",
        &serde_json::json!({
            "entries": 7687u64, "sub_entries": 57878u64, "glyphs": 4673u64, "pages": 1038u64,
            "entries_needing_review": 9u64, "sub_entries_needing_review": 40u64,
            "image_only_glyphs": 29u64
        }),
    );
    let html = t.render(name::QUALITY, &ctx).expect("rendering the page");
    assert!(html.contains("7.687"), "the thousands separator is missing");
    assert!(html.contains("57.878"));
    assert!(html.contains("1.038"));
    // Numbers below one thousand must be left alone.
    assert!(
        html.contains(">29<") || html.contains("29</"),
        "a small number got a separator"
    );
    assert!(!html.contains("7687"), "a raw number is still present");
}

#[test]
fn the_rail_sticks_as_one_block_not_piece_by_piece() {
    // The search box used to stick BY ITSELF against the opaque background; on scroll it
    // slid down and covered the letter chips right below it — the reader saw "VẦN", a
    // floating search box, then chips cut in half. Adding spacing does NOT fix it: a sticky
    //
    // The right invariant: on desktop only `.rail` (the wrapper) is sticky; neither inner
    // block is, because any of them sticking alone would cover the other.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);
    // Cut at the first NARROW breakpoint, not at any `@media`: the `prefers-color-scheme`
    // block sits at the top of the file, and cutting there would leave only the palette.
    let desktop = css
        .split("@media (max-width:")
        .next()
        .expect("the part outside the narrow breakpoint");

    assert!(
        desktop.contains(".rail {") && desktop.contains("position: sticky"),
        "the rail must stick as one block"
    );
    for block in [".rail-search {", ".rail-browse {"] {
        let Some(at) = desktop.find(block) else {
            continue;
        };
        let tail = &desktop[at..];
        let rule = tail.split('}').next().unwrap_or("");
        assert!(
            !rule.contains("position: sticky"),
            "{block} must not stick on its own — it would cover the other"
        );
    }
}

#[test]
fn the_rail_splits_in_two_on_mobile() {
    // Content must be able to sit BETWEEN the search box and the browse section. The `.rail`
    // wrapper prevents that unless it removes itself from the grid with `display: contents`.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);
    let mobile = css
        .split("@media (max-width: 899px)")
        .nth(1)
        .expect("a stacking breakpoint must exist");
    assert!(
        mobile.contains("display: contents"),
        "the wrapper must yield its grid cell to its two children"
    );
    assert!(mobile.contains("grid-area: search"));
    assert!(mobile.contains("grid-area: browse"));
    // And on mobile the search box is the sticky one — browsing already sits below the content.
    assert!(
        mobile.contains("position: sticky"),
        "the search box must stick on mobile"
    );
}

#[test]
fn every_grid_area_name_points_at_a_real_area() {
    // The bug: `.content` kept `grid-area: content` after the desktop grid stopped declaring
    // `grid-template-areas`. That name pointed nowhere, and the browser reported NOTHING —
    // it silently pushed the element into an implicit row, dropping the content below the rail.
    //
    // No compiler catches this; CSS wrong in this way only shows when someone looks at the screen.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);

    // Every area name ever declared, in any block.
    let mut declared: Vec<String> = Vec::new();
    for chunk in css.split("grid-template-areas:").skip(1) {
        let block = chunk.split(';').next().unwrap_or("");
        for word in block.split(|c: char| !c.is_ascii_alphanumeric() && c != '-') {
            if !word.is_empty() {
                declared.push(word.to_owned());
            }
        }
    }

    let mut dangling: Vec<String> = Vec::new();
    for chunk in css.split("grid-area:").skip(1) {
        let value = chunk.split(';').next().unwrap_or("").trim().to_owned();
        // Skip the coordinate form (`1 / 2 / 3 / 4`); only the NAMED form matters here.
        if value.is_empty() || value.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        if !declared.contains(&value) {
            dangling.push(value);
        }
    }

    assert!(
        dangling.is_empty(),
        "grid-area points at an area never declared: {dangling:?} (declared: {declared:?})"
    );
}

#[test]
fn the_missing_font_warning_only_covers_glyphs_that_certainly_cannot_render() {
    // Private Use Area glyphs carry code points meaningful only INSIDE the Nôm Na Tống font
    // of the print — 26 characters, and without the font every machine shows ▯. That is certain.
    let pua = GlyphChar::parse("󱖤").expect("valid glyph");
    let view = GlyphView::new(&pua, "Kè");
    assert_eq!(view.kind, "pua");
    assert!(view.no_system_font, "PUA certainly has no system font");
    assert_eq!(view.codepoint_hex, "F15A4");

    // Ext-B does NOT: many machines ship a font covering that range. Assuming they are broken
    // would stick a false warning on 847 entries.
    let ext_b = GlyphChar::parse("𨰲").expect("valid glyph");
    assert!(
        !GlyphView::new(&ext_b, "Lõm").no_system_font,
        "Ext-B must not be warned about: that is a guess, not a fact"
    );

    // Nor image glyphs: they have no code point that could be missing a font.
    assert!(!GlyphView::image_only("Chày").no_system_font);
}

#[test]
fn the_page_frame_tells_the_template_whether_a_font_is_shipped() {
    // Without a font the warning shows; with one it disappears, with no template change.
    let t = templates(); // the font directory points at nothing
    let html = t
        .render(name::NOT_FOUND, &t.context())
        .expect("rendering the page");
    let _ = html;
    let ctx = t.context();
    assert_eq!(
        ctx.get("has_nom_font").and_then(|v| v.as_bool()),
        Some(false),
        "no font is shipped yet the context reports one"
    );
}

#[test]
fn every_page_starts_at_the_same_height_even_with_a_hidden_heading() {
    // The home page opens with an <h1> meant only for screen readers. It occupies no space,
    // so the "first child has no top margin" rule lands on it while the REAL block after it
    // keeps its 32px — the home page alone sits lower and the layout loses consistency.
    let css = without_comments(dnqatv_adapter_web::template::STYLESHEET);
    assert!(
        css.contains(".visually-hidden:first-child + *"),
        "the rule for a block following a hidden heading is missing"
    );
}

#[test]
fn the_shipped_fonts_are_declared_from_what_is_really_on_disk() {
    // The gap file is BabelStone Han, not Nôm Na Tống, so it must be declared under its own
    // family — the `--nom` stack already lists it, and claiming it is Nôm Na Tống would
    // misrepresent whose glyphs those are.
    //
    // Its `unicode-range` must be the explicit list read from the file's own cmap, never a
    // broad range: a broad range would let it claim characters Nôm Na Tống does have, and
    // the later declaration would win.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../frontend/fonts");
    let set = FontSet::scan(&dir);
    if set.is_empty() {
        eprintln!("skipped: no font shipped under frontend/fonts");
        return;
    }
    let css = set.font_face_css();

    if let Some(gap) = set.file(FontRange::Gap) {
        assert!(
            css.contains("font-family: \"BabelStone Han\""),
            "the gap file must be declared under its own family:\n{css}"
        );
        assert!(
            gap.unicode_range.starts_with("U+") && gap.unicode_range.contains(", U+"),
            "the gap range must be an explicit code point list, got {:?}",
            gap.unicode_range
        );
        assert!(
            !gap.unicode_range.contains('-'),
            "the gap range must list code points, not spans: {:?}",
            gap.unicode_range
        );
    }

    // Whatever is declared must point at a file that exists — the whole point of scanning.
    for line in css.lines().filter(|l| l.contains("src: url(")) {
        let name = line
            .split(&format!("{FONT_URL_PREFIX}/"))
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("a file name in the src url");
        assert!(
            dir.join(name).is_file(),
            "@font-face points at {name}, which is not on disk"
        );
    }
}
