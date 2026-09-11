//! The typographic style of a text run.
//!
//! The printed book encodes structure through **type style**, and the 2026 digital edition
//! preserves that:
//!
//! ```text
//! 𨰲  Lõm  n.              the glyph set in a Nôm font
//! ― gươm. Nạm gươm.        "― gươm" ITALIC, ". Nạm gươm." REGULAR
//! Ruột, trúc mứt, …        the whole line in regular
//! ```
//!
//! Why this matters: the boundary between a sub-entry's **form** and its **definition** is a
//! **font change**, not a full stop to be guessed at. Definitions are full of full stops, so
//! splitting on punctuation is guessing; splitting on font is reading what the print recorded.
//!
//! Before styles were available, sub-entry detection keyed on the placeholder character and
//! was wrong in both directions: it missed sub-entries written with `—` (U+2014, 1,065 times)
//! instead of `―` (U+2015), and it falsely caught folk verses quoted inside definitions that
//! happen to contain a placeholder character.

/// A text style, inferred from the font name embedded in the PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TextStyle {
    #[default]
    Regular,
    /// Italic — the print uses it for a sub-entry's **form**.
    Italic,
    Bold,
    /// A Han-Nom font — used for glyphs and Han compounds.
    Han,
}

impl TextStyle {
    pub const ALL: [TextStyle; 4] = [Self::Regular, Self::Italic, Self::Bold, Self::Han];

    pub const fn db_value(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Italic => "italic",
            Self::Bold => "bold",
            Self::Han => "han",
        }
    }

    /// Infer the style from the PDF's base font name.
    ///
    /// The Han-Nom font list comes from the file itself: `Nom Na Tong`, `BabelStone Han`,
    /// `ZenKai`, `UnBatang`. PDF font names usually carry a subset prefix of the form
    /// `AAAAAA+` and encode spaces as `#20`, so both are stripped before comparing.
    pub fn from_base_font(name: &str) -> Self {
        let cleaned = name.replace("#20", " ");
        let bare = cleaned
            .split_once('+')
            .map_or(cleaned.as_str(), |(_, rest)| rest);

        // Check Han fonts first: `UnBatang Bold` contains "Bold" yet is a Han font.
        for han in [
            "Nom Na Tong",
            "NomNaTong",
            "BabelStone",
            "ZenKai",
            "UnBatang",
        ] {
            if bare.contains(han) {
                return Self::Han;
            }
        }
        if bare.contains("Italic") || bare.contains("Oblique") {
            return Self::Italic;
        }
        if bare.contains("Bold") {
            return Self::Bold;
        }
        Self::Regular
    }
}
