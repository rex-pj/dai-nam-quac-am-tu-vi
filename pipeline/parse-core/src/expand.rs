//! The two derivation rules of the book, taken from the DẤU RIÊNG page.
//!
//! ```text
//! |  = stands for the word being defined.   → replace with the entry GLYPH
//! —  = stands for the Chinese character id. → replace with the entry READING
//! ```
//!
//! The two rules DIFFER and are easy to confuse: `|` appears in the Han column, the dash in the Quốc ngữ column.
//!
//! The result is always [`Derived`] — it must never be presented as the words of the book.
//! The UI shows the verbatim `― gươm` by default, and switches to `Lõm gươm` only when the
//! user toggles it, with the substituted part clearly marked as derived.
//!
//! Measured across 57,892 sub-entry forms: **96.39% have exactly one placeholder**, 3.27%
//! have two, and some have up to six — so **every** occurrence must be replaced, not just the first.

use dnqatv_core::text::{DerivationRule, Derived};

/// The reading placeholders. Counted across the body: `―` 58,706 · `—` 1,065 · `–` 129.
///
/// The DẤU RIÊNG page writes `—` (U+2014) but the typesetter used `―` (U+2015) for most.
/// All three are accepted, but **not** normalised to one character — the print is stored as written.
pub const READING_PLACEHOLDERS: [char; 3] = ['―', '—', '–'];

/// The print also uses two joined hyphens on 24 lines.
pub const READING_PLACEHOLDER_DIGRAPH: &str = "--";

/// The glyph placeholder, in the Han column.
pub const GLYPH_PLACEHOLDER: char = '|';

/// The entry under consideration, used as the replacement source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadwordContext<'a> {
    /// `None` when the print uses an image for the glyph — 29 such entries.
    pub glyph: Option<&'a str>,
    pub reading: &'a str,
}

/// The result of a derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub value: Derived,
    /// How many replacements were made. Zero means the form had no placeholder at all.
    pub substitutions: usize,
    /// How many placeholders REMAIN after replacing — of the kind this rule does not handle.
    ///
    /// Above 0 flags a case for review, and replacing the `|` with the glyph would be a
    /// guess rather than a rule of the book.
    ///
    /// This used to fire on 829 forms. Nearly all of them were not the print being unclear
    /// but the splitter beginning the form inside the Han column; `subentry::quoc_ngu_starts_at`
    /// gives that material back and **3** are left, listed in `review/residual-placeholders.toml`.
    pub residual: usize,
}

impl Expansion {
    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }

    /// It contains a placeholder this rule cannot handle.
    pub const fn needs_review(&self) -> bool {
        self.residual > 0
    }
}

/// Replace the placeholders in a Quốc ngữ form with the entry reading.
///
/// The reading is substituted **verbatim**, capitalisation included: `Chính giữa ―` becomes
/// `Chính giữa Lõm`. Lowercasing it to look "more natural" is interpretation, and it is also
/// unnecessary — search goes through [`dnqatv_core::text::fold`], where case does not matter.
pub fn expand_reading_form(form: &str, ctx: &HeadwordContext<'_>) -> Expansion {
    let mut out = String::with_capacity(form.len());
    let mut substitutions = 0usize;

    let mut rest = form;
    'outer: while !rest.is_empty() {
        // The two-dash sequence must be checked before the single characters.
        if let Some(tail) = rest.strip_prefix(READING_PLACEHOLDER_DIGRAPH) {
            out.push_str(ctx.reading);
            substitutions += 1;
            rest = tail;
            continue;
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            if READING_PLACEHOLDERS.contains(&c) {
                out.push_str(ctx.reading);
                substitutions += 1;
            } else {
                out.push(c);
            }
            rest = chars.as_str();
            continue 'outer;
        }
        break;
    }

    let residual = out.matches(GLYPH_PLACEHOLDER).count();
    Expansion {
        value: Derived::new(out, DerivationRule::DashToReading),
        substitutions,
        residual,
    }
}

/// Replace the placeholders in the Han part with the entry glyph.
///
/// Returns `None` when the entry uses an image and has no Unicode glyph — substituting a
/// "close enough" character is exactly what the 2026 editors deliberately refused on the LƯU Ý page.
pub fn expand_han_form(form: &str, ctx: &HeadwordContext<'_>) -> Option<Expansion> {
    let glyph = ctx.glyph?;
    let mut out = String::with_capacity(form.len());
    let mut substitutions = 0usize;

    for c in form.chars() {
        if c == GLYPH_PLACEHOLDER {
            out.push_str(glyph);
            substitutions += 1;
        } else {
            out.push(c);
        }
    }

    let residual = out
        .chars()
        .filter(|c| READING_PLACEHOLDERS.contains(c))
        .count();
    Some(Expansion {
        value: Derived::new(out, DerivationRule::PipeToGlyph),
        substitutions,
        residual,
    })
}
