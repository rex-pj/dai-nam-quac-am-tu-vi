//! A Han-Nom glyph and how it must be displayed.

use crate::error::DomainError;
use crate::text::normalize::nfc;

/// How a glyph exists in the digital edition — this decides how the UI renders it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlyphKind {
    /// Inside the Basic Multilingual Plane; common fonts usually cover it.
    Bmp,
    /// CJK Extension B and beyond (U+20000+). 850 characters measured. User machines
    /// rarely have a font for these, so shipping Nom Na Tong is mandatory.
    ExtB,
    /// Private Use Area. 26 characters measured in Plane 15, using the private encoding
    /// of the Nom Na Tong font embedded in the PDF.
    Pua,
    /// No Unicode code point at all — the 2026 edition shows an image cropped from the print.
    /// 22 glyphs measured, listed on the LƯU Ý page.
    ///
    /// The 2026 editors deliberately refused to assign "close enough" code points; we
    /// respect that decision and never substitute another character.
    ImageOnly,
}

impl GlyphKind {
    pub const ALL: [GlyphKind; 4] = [Self::Bmp, Self::ExtB, Self::Pua, Self::ImageOnly];

    pub const fn db_value(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::ExtB => "ext_b",
            Self::Pua => "pua",
            Self::ImageOnly => "image_only",
        }
    }

    /// Whether this glyph has a usable Unicode code point.
    pub const fn has_unicode(self) -> bool {
        !matches!(self, Self::ImageOnly)
    }

    /// Classify by code point. Does not apply to image-only glyphs — those have no code
    /// point, so they must be marked from the LƯU Ý page listing.
    pub const fn from_codepoint(cp: u32) -> Self {
        match cp {
            0xE000..=0xF8FF => Self::Pua,
            0xF0000..=0x10FFFD => Self::Pua,
            0x20000..=0x3FFFF => Self::ExtB,
            _ => Self::Bmp,
        }
    }
}

/// A glyph: exactly ONE character.
///
/// Measured across all 4,678 glyphs of the entry table: not one glyph is longer than a
/// single character.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphChar {
    ch: char,
    kind: GlyphKind,
}

impl GlyphChar {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let t = nfc(raw.trim());
        let mut it = t.chars();
        match (it.next(), it.next()) {
            (Some(ch), None) => Ok(Self {
                ch,
                kind: GlyphKind::from_codepoint(ch as u32),
            }),
            _ => Err(DomainError::NotSingleGlyph(t)),
        }
    }

    pub const fn ch(&self) -> char {
        self.ch
    }
    pub const fn kind(&self) -> GlyphKind {
        self.kind
    }
    pub const fn codepoint(&self) -> u32 {
        self.ch as u32
    }
}
