//! The boundary between the book's own words and words we derived.
//!
//! Every text field in the system is exactly one of two kinds: `Verbatim` (taken from the
//! PDF text layer, not a byte altered) or `Derived` (produced by a named, tested rule).
//! There is no third kind. Interfaces that promise "verbatim" accept only `Verbatim`.

use crate::text::normalize::nfc;

/// Text taken straight from the printed page; NFC-normalized only, never content-edited.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verbatim(String);

impl Verbatim {
    pub fn new(raw: &str) -> Self {
        Self(nfc(raw))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A derivation rule, taken from the book's own DẤU RIÊNG (special marks) page.
///
/// The two rules DIFFER and are easy to confuse: the book states that `|` stands for the
/// *word* being defined and `—` for the corresponding *Chinese character* — in practice
/// `|` appears in the Han column and `―` in the Quốc ngữ column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivationRule {
    /// `|` inside a Han string, replaced by the entry's glyph. E.g. `| 意` under entry 阿 becomes `阿意`.
    PipeToGlyph,
    /// `―` inside a Quốc ngữ string, replaced by the entry's reading. E.g. `― gươm` under entry Lõm becomes `Lõm gươm`.
    DashToReading,
}

impl DerivationRule {
    pub const ALL: [DerivationRule; 2] = [Self::PipeToGlyph, Self::DashToReading];

    /// The placeholder character this rule replaces.
    pub const fn placeholder(self) -> char {
        match self {
            Self::PipeToGlyph => '|',
            Self::DashToReading => '―',
        }
    }
}

/// Text we computed, carrying the rule that produced it. Never to be presented as the book's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived {
    value: String,
    rule: DerivationRule,
}

impl Derived {
    pub fn new(value: String, rule: DerivationRule) -> Self {
        Self { value, rule }
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
    pub const fn rule(&self) -> DerivationRule {
        self.rule
    }
}
