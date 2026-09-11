//! View models — **the only thing Tera ever sees**.
//!
//! Templates never receive domain models. The reason is specific to this book: every glyph
//! needs to know *how it should be rendered* (BMP font, Ext-B font, or an image), every
//! sub-entry needs both the verbatim and the derived form, and every label needs both the
//! printed symbol and its spelled-out name. Computing all that in a template means putting
//! logic where it cannot be tested.
//!
//! Principle: **a template may only loop and print.** Every decision is already made here.

use dnqatv_core::model::{
    EntryDetail, EntrySummary, GlyphChar, GlyphKind, Letter, PdfPage, SubEntry,
};
use dnqatv_core::port::PageView;
use dnqatv_core::search::{Paged, RankTier, ScoredEntry};
use serde::Serialize;

use crate::route::url;

/// One glyph, with everything needed to display it correctly.
#[derive(Debug, Clone, Serialize)]
pub struct GlyphView {
    pub char: String,
    /// `bmp` · `ext_b` · `pua` · `image_only` — used as a CSS class to pick the font range.
    pub kind: &'static str,
    /// The user machine may have no font for this character; we ship one for those ranges.
    pub needs_shipped_font: bool,
    /// A character with no Unicode code point — show an explanatory chip, **never** substitute
    /// a lookalike.
    pub no_unicode: bool,
    /// No system font can render this character — **certain**, not a guess.
    ///
    /// True for the 26 Private Use Area glyphs: their code points only mean anything INSIDE
    /// the Nom Na Tong font the print uses. A machine without that font sees ▯, and there is
    /// no way around it. Unlike Ext-B: many machines ship a font covering that range, so
    /// assuming they are broken would stick a false warning on 847 entries.
    pub no_system_font: bool,
    /// The code point in hex, e.g. `F15A4`. Shown so the reader can tell this is a private-use
    /// code point rather than an ordinary Han character that failed to render.
    pub codepoint_hex: String,
    pub url: String,
    /// The label for screen readers. Nôm script is a total blind spot for them.
    pub aria_label: String,
    /// The BCP-47 language tag for the glyph.
    ///
    /// `vi-Hani` = Vietnamese written in Han script. Not `zh`: this is Vietnamese Nôm, and a
    /// screen reader guessing Chinese would read it with Mandarin pronunciation.
    pub lang: &'static str,
}

/// The language tag for every glyph in this book.
pub const GLYPH_LANG: &str = "vi-Hani";

impl GlyphView {
    pub fn new(glyph: &GlyphChar, reading: &str) -> Self {
        let kind = glyph.kind();
        Self {
            char: glyph.ch().to_string(),
            kind: kind.db_value(),
            needs_shipped_font: matches!(kind, GlyphKind::ExtB | GlyphKind::Pua),
            no_unicode: !kind.has_unicode(),
            no_system_font: matches!(kind, GlyphKind::Pua),
            codepoint_hex: format!("{:04X}", glyph.codepoint()),
            url: url::glyph(glyph.ch()),
            aria_label: format!("chữ Hán-Nôm đọc là {reading}"),
            lang: GLYPH_LANG,
        }
    }

    /// Entries whose glyph the print shows as an image — 29 of them.
    pub fn image_only(reading: &str) -> Self {
        Self {
            char: String::new(),
            kind: GlyphKind::ImageOnly.db_value(),
            needs_shipped_font: false,
            no_unicode: true,
            no_system_font: false,
            codepoint_hex: String::new(),
            url: String::new(),
            aria_label: format!("chữ Nôm đọc là {reading}, chưa có mã Unicode"),
            lang: GLYPH_LANG,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntrySummaryView {
    pub slug: String,
    pub url: String,
    pub reading: String,
    pub alternate: Option<String>,
    pub glyph: GlyphView,
    /// The label as printed: `c.` · `n.` · `c. n.`
    pub pos_label: String,
    /// The spelled-out name: `chữ nho · chữ nôm`
    pub pos_name: String,
    pub gloss: String,
    pub pdf_page: u16,
    pub printed_page: Option<u16>,
    pub page_url: String,
    pub needs_review: bool,
}

impl From<&EntrySummary> for EntrySummaryView {
    fn from(e: &EntrySummary) -> Self {
        let reading = e.reading.as_str().to_owned();
        Self {
            slug: e.slug.as_str().to_owned(),
            url: url::entry(e.slug.as_str()),
            glyph: match &e.glyph {
                Some(g) => GlyphView::new(g, &reading),
                None => GlyphView::image_only(&reading),
            },
            alternate: e.alternate.as_ref().map(|a| a.as_str().to_owned()),
            pos_label: e.pos.book_label(),
            pos_name: e.pos.display_name(),
            gloss: e.gloss.clone(),
            pdf_page: e.pdf_page.get(),
            printed_page: e.pdf_page.printed().map(|p| p.get()),
            page_url: url::page(e.pdf_page),
            needs_review: e.needs_review,
            reading,
        }
    }
}

/// One piece of a sub-entry form: ordinary text, or a placeholder mark.
///
/// Split here rather than generating HTML in Rust: the template only loops and prints, so
/// Tera still auto-escapes, and no raw HTML string passes through any layer.
#[derive(Debug, Clone, Serialize)]
pub struct FormPart {
    pub text: String,
    /// This piece is a placeholder mark (`―`, `—`, `–`, `|`, `--`).
    pub placeholder: bool,
}

/// The characters the print uses as placeholders.
///
/// Four code points, not one. Measured across the book body: `―` U+2015 58,706 times ·
/// `|` 2,197 · **`—` U+2014 1,065** · `–` U+2013 129. The DẤU RIÊNG page writes `—` while
/// the typesetter used `―`; hard-coding one code point loses 540 lines.
const PLACEHOLDERS: [char; 4] = ['―', '—', '–', '|'];
/// The two-dash form, which must be checked BEFORE the single dash, or it becomes two placeholders.
const PLACEHOLDER_DIGRAPH: &str = "--";

/// Split a form into pieces, marking which ones are placeholders.
pub fn split_form(form: &str) -> Vec<FormPart> {
    let mut parts: Vec<FormPart> = Vec::new();
    let mut buffer = String::new();
    let mut rest = form;

    let push_text = |buffer: &mut String, parts: &mut Vec<FormPart>| {
        if !buffer.is_empty() {
            parts.push(FormPart {
                text: std::mem::take(buffer),
                placeholder: false,
            });
        }
    };

    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix(PLACEHOLDER_DIGRAPH) {
            push_text(&mut buffer, &mut parts);
            parts.push(FormPart {
                text: PLACEHOLDER_DIGRAPH.to_owned(),
                placeholder: true,
            });
            rest = tail;
            continue;
        }
        let mut chars = rest.chars();
        match chars.next() {
            Some(c) if PLACEHOLDERS.contains(&c) => {
                push_text(&mut buffer, &mut parts);
                parts.push(FormPart {
                    text: c.to_string(),
                    placeholder: true,
                });
            }
            Some(c) => buffer.push(c),
            None => break,
        }
        rest = chars.as_str();
    }
    push_text(&mut buffer, &mut parts);
    parts
}

/// One sub-entry: **verbatim** and **derived** are two separate fields, not one field plus
/// a flag. The "Show full form" toggle changes only what is displayed, never the data.
#[derive(Debug, Clone, Serialize)]
pub struct SubEntryView {
    pub form: String,
    /// The form split into pieces, marking placeholders — so the template can wrap <abbr>.
    pub form_parts: Vec<FormPart>,
    pub form_expanded: String,
    /// Whether a placeholder is present — decides whether the toggle means anything here.
    pub has_placeholder: bool,
    /// The text standing in for `―`, placed in `<abbr title>` so a screen reader does not
    /// read out "dash gươm".
    pub placeholder_title: String,
    pub han_form: Option<String>,
    pub han_expanded: Option<String>,
    pub definition: String,
    pub needs_review: bool,
}

impl SubEntryView {
    pub fn new(sub: &SubEntry, reading: &str) -> Self {
        Self {
            has_placeholder: sub.form != sub.form_expanded,
            form_parts: split_form(&sub.form),
            form: sub.form.clone(),
            form_expanded: sub.form_expanded.clone(),
            placeholder_title: reading.to_owned(),
            han_form: sub.han_form.clone(),
            han_expanded: sub.han_expanded.clone(),
            definition: sub.definition.clone(),
            needs_review: sub.needs_review,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryDetailView {
    #[serde(flatten)]
    pub summary: EntrySummaryView,
    pub sub_entries: Vec<SubEntryView>,
    pub sub_entry_count: usize,
    pub inherits_glyph: bool,
    /// Whether any sub-entry needs review — decides whether the warning strip is shown.
    pub any_sub_needs_review: bool,
    /// How the shape of an unencodable glyph was described. `None` unless a person has signed
    /// the dossier row it comes from, so the page shows nothing here on anyone's say-so.
    pub shape_note: Option<String>,
}

impl From<&EntryDetail> for EntryDetailView {
    fn from(e: &EntryDetail) -> Self {
        let reading = e.summary.reading.as_str();
        Self {
            sub_entries: e
                .sub_entries
                .iter()
                .map(|s| SubEntryView::new(s, reading))
                .collect(),
            sub_entry_count: e.sub_entries.len(),
            inherits_glyph: e.inherits_glyph,
            any_sub_needs_review: e.sub_entries.iter().any(|s| s.needs_review),
            shape_note: e.shape_note.clone(),
            summary: EntrySummaryView::from(&e.summary),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PageMetaView {
    pub pdf_page: u16,
    pub printed_page: Option<u16>,
    pub letter: Option<String>,
    pub image_path: Option<String>,
    pub head_first: Option<String>,
    pub head_last: Option<String>,
    pub url: String,
    pub previous_url: Option<String>,
    pub next_url: Option<String>,
}

impl From<&PageView> for PageMetaView {
    fn from(p: &PageView) -> Self {
        let n = p.pdf_page.get();
        // Neighbours are walked in PDF order — physical adjacency in the book — and only then
        // turned into addresses: over the LƯU Ý page the printed numbers are not adjacent.
        let neighbour = |n: u16| PdfPage::new(n).ok().map(url::page);
        Self {
            pdf_page: n,
            printed_page: p.printed_page.map(|x| x.get()),
            letter: p.letter.map(|l| l.label().to_owned()),
            image_path: p.image_path.clone(),
            head_first: p.head_first.clone(),
            head_last: p.head_last.clone(),
            url: url::page(p.pdf_page),
            previous_url: n.checked_sub(1).and_then(neighbour),
            next_url: neighbour(n + 1),
        }
    }
}

/// How many rows this page holds, against how many there are in all.
///
/// A heading reading "74 chữ đầu" above thirty rows contradicts itself: the reader counts the
/// rows and finds thirty.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PagedCountView {
    /// Rows on this page.
    pub shown: u64,
    pub total: u64,
    /// Whether this page holds fewer rows than the whole list. A single-page list just states
    /// its total — "7 / 7" is noise.
    pub partial: bool,
}

impl PagedCountView {
    pub fn new<T>(paged: &Paged<T>) -> Self {
        let shown = paged.items.len() as u64;
        Self {
            shown,
            total: paged.total,
            partial: shown < paged.total,
        }
    }
}

/// One result group, labelled by **why it matched**.
///
/// The label comes straight from [`RankTier`] — the UI and the ranking layer share one
/// enum, so there is no second label set to drift from the priority order.
#[derive(Debug, Clone, Serialize)]
pub struct ResultGroupView {
    pub tier: &'static str,
    pub label: &'static str,
    pub count: usize,
    pub entries: Vec<EntrySummaryView>,
}

impl ResultGroupView {
    pub fn new(tier: RankTier, entries: &[ScoredEntry]) -> Self {
        Self {
            tier: tier.as_param(),
            label: tier.group_label(),
            count: entries.len(),
            entries: entries.iter().map(|s| (&s.entry).into()).collect(),
        }
    }
}

/// One letter chip on the home and browse pages.
#[derive(Debug, Clone, Serialize)]
pub struct LetterChipView {
    pub letter: String,
    pub url: String,
    pub active: bool,
}

impl LetterChipView {
    /// The 22 letters **in the exact printed order** — Y where I would be, no I/F/J/W/Z.
    pub fn all(active: Option<Letter>) -> Vec<Self> {
        Letter::ALL
            .iter()
            .map(|l| Self {
                letter: l.label().to_owned(),
                url: url::letter(l.db_value()),
                active: Some(*l) == active,
            })
            .collect()
    }
}

/// The 1895 ↔ modern orthography suggestions.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestionView {
    pub text: String,
    pub url: String,
    pub reason: &'static str,
    /// Whether a human has checked it against the print. The UI says so, it does not stay silent.
    pub verified: bool,
}

impl From<&dnqatv_app::Suggestion> for SuggestionView {
    fn from(s: &dnqatv_app::Suggestion) -> Self {
        Self {
            text: s.alternative.clone(),
            url: url::search(&s.alternative),
            reason: s.reason,
            verified: s.verified,
        }
    }
}
