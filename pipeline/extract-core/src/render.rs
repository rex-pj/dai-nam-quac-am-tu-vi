//! Walking the content-stream tokens and building the text of one page.
//!
//! Gate ① of the plan lives here: an unmapped glyph code is **not** guessed and **not**
//! silently dropped — it is recorded in [`PageText::unmapped`] with its font name and code,
//! so the caller can decide to stop. The 2026 edition uses 2-byte Type0 fonts for Han-Nom,
//! so one byte out of step corrupts a whole page.

use std::collections::HashMap;

use crate::cmap::{DecodedGlyph, ToUnicodeCMap};
use crate::content::{ContentError, Token, tokenize};
use crate::layout::{Line, TextRun, group_into_lines};
use crate::style::TextStyle;

/// How a font encodes glyph codes inside content-stream strings.
#[derive(Debug, Clone)]
pub struct FontEncoding {
    pub cmap: ToUnicodeCMap,
    /// Type0 fonts (`Identity-H`) use 2-byte codes; simple fonts use 1 byte.
    pub two_byte: bool,
    /// The style inferred from the font name — the print uses it to encode structure.
    pub style: TextStyle,
}

/// The font table of a page, keyed by resource name (`/F1`, `/TT2`…).
pub type FontMap = HashMap<String, FontEncoding>;

/// A glyph code absent from the font `/ToUnicode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedGlyph {
    pub font: String,
    pub code: u16,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageText {
    pub lines: Vec<Line>,
    /// Every code that could not be decoded, in encounter order. Empty is the precondition for loading.
    pub unmapped: Vec<UnmappedGlyph>,
}

impl PageText {
    /// The text of the page, one printed line per line.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(Line::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_clean(&self) -> bool {
        self.unmapped.is_empty()
    }
}

/// Build the text of one page from a decompressed content stream.
pub fn render_page(
    content: &[u8],
    fonts: &FontMap,
    page_width: f64,
) -> Result<PageText, ContentError> {
    let tokens = tokenize(content)?;

    let mut runs: Vec<TextRun> = Vec::new();
    let mut unmapped: Vec<UnmappedGlyph> = Vec::new();

    let mut current_font: Option<&str> = None;
    let mut x = 0.0f64;
    let mut y = 0.0f64;
    // The operands preceding the current operator; PDF is postfix.
    let mut operands: Vec<&Token> = Vec::new();

    for token in &tokens {
        let Some(op) = token.as_operator() else {
            operands.push(token);
            continue;
        };

        match op {
            "Tf" => {
                if let Some(Token::Name(name)) = operands.iter().rev().find_map(|t| match t {
                    Token::Name(_) => Some(*t),
                    _ => None,
                }) {
                    current_font = Some(name);
                }
            }
            // Tm resets the whole text matrix; the last two numbers are the translation.
            "Tm" => {
                let nums: Vec<f64> = operands.iter().filter_map(|t| t.as_number()).collect();
                if nums.len() >= 6 {
                    x = nums[nums.len() - 2];
                    y = nums[nums.len() - 1];
                }
            }
            "Td" | "TD" => {
                let nums: Vec<f64> = operands.iter().filter_map(|t| t.as_number()).collect();
                if nums.len() >= 2 {
                    x += nums[nums.len() - 2];
                    y += nums[nums.len() - 1];
                }
            }
            "Tj" | "'" | "\"" => {
                for t in &operands {
                    if let Token::Str(bytes) = t {
                        push_run(&mut runs, &mut unmapped, x, y, bytes, current_font, fonts);
                    }
                }
            }
            "TJ" => {
                // An array alternating strings and spacing adjustments. The numbers only
                // affect spacing, never produce characters, so they are skipped — real
                // whitespace is already in the strings.
                for t in &operands {
                    if let Token::Str(bytes) = t {
                        push_run(&mut runs, &mut unmapped, x, y, bytes, current_font, fonts);
                    }
                }
            }
            _ => {}
        }

        operands.clear();
    }

    Ok(PageText {
        lines: group_into_lines(&runs, page_width),
        unmapped,
    })
}

fn push_run(
    runs: &mut Vec<TextRun>,
    unmapped: &mut Vec<UnmappedGlyph>,
    x: f64,
    y: f64,
    bytes: &[u8],
    font_name: Option<&str>,
    fonts: &FontMap,
) {
    let Some(name) = font_name else { return };
    let Some(enc) = fonts.get(name) else { return };

    let mut text = String::new();
    if enc.two_byte {
        for pair in bytes.chunks_exact(2) {
            let code = u16::from_be_bytes([pair[0], pair[1]]);
            append_code(&mut text, unmapped, &enc.cmap, name, code);
        }
    } else {
        for &b in bytes {
            append_code(&mut text, unmapped, &enc.cmap, name, u16::from(b));
        }
    }

    if !text.is_empty() {
        runs.push(TextRun {
            x,
            y,
            text,
            style: enc.style,
        });
    }
}

fn append_code(
    text: &mut String,
    unmapped: &mut Vec<UnmappedGlyph>,
    cmap: &ToUnicodeCMap,
    font: &str,
    code: u16,
) {
    match cmap.decode(code) {
        DecodedGlyph::Mapped(s) => text.push_str(&s),
        DecodedGlyph::Unmapped { code } => unmapped.push(UnmappedGlyph {
            font: font.to_owned(),
            code,
        }),
    }
}
