//! Decoding a PDF `/ToUnicode` CMap.
//!
//! This is gate ① of the plan: **there is no fallback**.
//!
//! The original exploratory prototype, on meeting a glyph code absent from the CMap,
//! silently used `String::from_char_code(code)` as a fallback. For a dictionary that is
//! fabricating data — it could turn 惡 into an arbitrary character with nobody noticing.
//! So `decode` returns [`DecodedGlyph::Unmapped`] and never guesses, and every syntax error
//! is a hard error rather than something skipped.
//!
//! A number worth remembering: the Nom Na Tong CMap in the 2026 edition alone has **17,701**
//! targets that are surrogate pairs (CJK Extension B and beyond). Getting UTF-16 wrong here
//! corrupts most of the Nôm script in the book.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CMapError {
    #[error("hex string {0:?} is not a multiple of 4 digits and cannot be read as UTF-16BE")]
    HexNotUtf16Aligned(String),

    #[error("hex string {0:?} contains an invalid character")]
    InvalidHex(String),

    #[error("hex string {0:?} produces invalid UTF-16 (lone surrogate)")]
    LoneSurrogate(String),

    #[error("hex string {0:?} is not a valid Unicode code point")]
    InvalidCodePoint(String),

    #[error("source code {0:?} is longer than 4 hex digits; only 1-2 byte codes are supported")]
    SourceCodeTooWide(String),

    #[error("bfrange {lo:#06X}..={hi:#06X} has a lower bound above its upper bound")]
    RangeInverted { lo: u32, hi: u32 },

    #[error("bfrange {lo:#06X}..={hi:#06X} overflows the width of its destination {dst:?}")]
    RangeOverflowsDestination { lo: u32, hi: u32, dst: String },

    #[error("bfrange {lo:#06X}..={hi:#06X} needs {need} elements but the array has {got}")]
    ArrayLengthMismatch {
        lo: u32,
        hi: u32,
        need: usize,
        got: usize,
    },

    #[error("block {0} is missing its terminating keyword")]
    UnterminatedBlock(&'static str),

    #[error("block {block} has extra or missing data near {near:?}")]
    MalformedBlock { block: &'static str, near: String },
}

/// The result of decoding one glyph code.
///
/// There is deliberately NO "guess" variant. An unmapped code must surface to the caller for a human.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedGlyph {
    Mapped(String),
    Unmapped { code: u16 },
}

impl DecodedGlyph {
    pub const fn is_mapped(&self) -> bool {
        matches!(self, Self::Mapped(_))
    }

    /// The text, if it could be mapped. No `unwrap_or_default` — call sites must handle it explicitly.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Mapped(s) => Some(s),
            Self::Unmapped { .. } => None,
        }
    }
}

/// The glyph-code → Unicode-string map, built from a `/ToUnicode` stream.
#[derive(Debug, Clone, Default)]
pub struct ToUnicodeCMap {
    map: HashMap<u16, String>,
}

impl ToUnicodeCMap {
    pub fn parse(src: &str) -> Result<Self, CMapError> {
        let toks = lex(src);
        let mut map = HashMap::new();
        parse_bfchar_blocks(&toks, &mut map)?;
        parse_bfrange_blocks(&toks, &mut map)?;
        Ok(Self { map })
    }

    /// Decode one glyph code.
    ///
    /// **A destination of U+0000 counts as UNMAPPED, not as a character.** The font in the
    /// 2026 edition has `/ToUnicode` entries pointing at U+0000 — that is the font saying
    /// "this glyph has no Unicode code point", exactly `.notdef`, except it says so with a
    /// present entry rather than an absent one. Accepting it as a character lets a NUL byte
    /// into the data: PostgreSQL cannot store it, and every character count goes wrong.
    ///
    /// Measured **28 times** across all 1,038 pages, all on pages 3 and 7 — the TIỂU TỰ and
    /// its French translation, where the author quotes Nôm examples. Book body: 0.
    pub fn decode(&self, code: u16) -> DecodedGlyph {
        match self.map.get(&code) {
            Some(s) if !s.contains('\0') => DecodedGlyph::Mapped(s.clone()),
            _ => DecodedGlyph::Unmapped { code },
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ── Tokens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok<'a> {
    /// The content between `<` and `>`, not yet validated.
    Hex(&'a str),
    OpenArray,
    CloseArray,
    Word(&'a str),
}

/// Tokenize the CMap. Only three things matter: hex strings, brackets and keywords.
/// Everything else (dicts, PostScript names, numbers) is skipped as irrelevant to the mapping.
fn lex(src: &str) -> Vec<Tok<'_>> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => {
                // `<<` opens a dictionary, not a hex string.
                if bytes.get(i + 1) == Some(&b'<') {
                    i += 2;
                    continue;
                }
                match src[i + 1..].find('>') {
                    Some(rel) => {
                        out.push(Tok::Hex(&src[i + 1..i + 1 + rel]));
                        i += rel + 2;
                    }
                    None => break,
                }
            }
            b'>' => {
                i += 1;
            }
            b'[' => {
                out.push(Tok::OpenArray);
                i += 1;
            }
            b']' => {
                out.push(Tok::CloseArray);
                i += 1;
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push(Tok::Word(&src[start..i]));
            }
            _ => {
                i += 1;
            }
        }
    }
    out
}

// ── Hex and UTF-16 ───────────────────────────────────────────────────────────

fn clean_hex(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The source code (left-hand side) is an integer of at most 4 hex digits.
fn parse_source_code(raw: &str) -> Result<u32, CMapError> {
    let h = clean_hex(raw);
    if h.is_empty() || h.len() > 4 {
        return Err(CMapError::SourceCodeTooWide(h));
    }
    u32::from_str_radix(&h, 16).map_err(|_| CMapError::InvalidHex(h))
}

/// Decode the destination string of a bfchar/bfrange.
///
/// The spec says the destination is a UTF-16BE string, but this file uses BOTH conventions:
///   * the Nom Na Tong font writes 8-digit surrogate pairs, e.g. `<D863DC32>` = U+28C32;
///   * the DejaVu Serif Bold font writes 5-digit code points directly, e.g. `<1d400>` = U+1D400.
///
/// They can be told apart without guessing: a length not divisible by 4 CANNOT be UTF-16BE,
/// so the only remaining reading is a code point. Both are checked strictly, with no fallback.
fn decode_destination(raw: &str) -> Result<String, CMapError> {
    let h = clean_hex(raw);
    if h.is_empty() {
        return Ok(String::new());
    }
    if h.len().is_multiple_of(4) {
        utf16be_hex_to_string(&h)
    } else {
        scalar_hex_to_string(&h)
    }
}

/// Uses [`String::from_utf16`] rather than assembling characters by hand: it handles
/// surrogate pairs correctly and REJECTS lone surrogates. Corrupt data must error, never
/// produce a replacement character.
fn utf16be_hex_to_string(h: &str) -> Result<String, CMapError> {
    let mut units = Vec::with_capacity(h.len() / 4);
    for chunk in h.as_bytes().chunks(4) {
        let s = std::str::from_utf8(chunk).map_err(|_| CMapError::InvalidHex(h.to_owned()))?;
        let u = u16::from_str_radix(s, 16).map_err(|_| CMapError::InvalidHex(h.to_owned()))?;
        units.push(u);
    }
    String::from_utf16(&units).map_err(|_| CMapError::LoneSurrogate(h.to_owned()))
}

/// A directly written code point. `char::from_u32` rejects surrogates and values above U+10FFFF.
fn scalar_hex_to_string(h: &str) -> Result<String, CMapError> {
    let v = u32::from_str_radix(h, 16).map_err(|_| CMapError::InvalidHex(h.to_owned()))?;
    char::from_u32(v)
        .map(|c| c.to_string())
        .ok_or_else(|| CMapError::InvalidCodePoint(h.to_owned()))
}

/// The destination of a bfrange, plus `offset`.
///
/// §9.10.3 describes this as "increment the last byte of the destination string". Taken
/// literally — forbidding a carry into the previous byte — it would wrongly reject valid
/// data: this very file has `<0062> <00ff> <00a0>`, a 158-code range mapping to
/// U+00A0..U+013D, which requires a carry into the high byte.
///
/// The real invariant to preserve is not "no carry" but **the result is still valid UTF-16**.
/// So the addition happens over the whole destination string as a big-endian integer, keeping
/// its width, and `String::from_utf16` decides. Overflowing the width is still a hard error.
fn offset_destination(raw: &str, offset: u32, lo: u32, hi: u32) -> Result<String, CMapError> {
    let h = clean_hex(raw);
    if h.is_empty() {
        return Err(CMapError::HexNotUtf16Aligned(h));
    }
    // u128 holds 32 hex digits, i.e. 8 UTF-16 code units — anything longer is abnormal data.
    if h.len() > 32 {
        return Err(CMapError::RangeOverflowsDestination { lo, hi, dst: h });
    }
    let width = h.len();
    let base = u128::from_str_radix(&h, 16).map_err(|_| CMapError::InvalidHex(h.clone()))?;
    let bumped = base + u128::from(offset);
    let rebuilt = format!("{bumped:0width$X}");
    if rebuilt.len() != width {
        return Err(CMapError::RangeOverflowsDestination { lo, hi, dst: h });
    }
    decode_destination(&rebuilt)
}

// ── bfchar / bfrange blocks ──────────────────────────────────────────────────

fn find_blocks<'a>(toks: &'a [Tok<'a>], begin: &str, end: &str) -> Vec<&'a [Tok<'a>]> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        if toks[i] == Tok::Word(begin) {
            let start = i + 1;
            let mut j = start;
            while j < toks.len() && toks[j] != Tok::Word(end) {
                j += 1;
            }
            if j <= toks.len() {
                out.push(&toks[start..j.min(toks.len())]);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn parse_bfchar_blocks(toks: &[Tok<'_>], map: &mut HashMap<u16, String>) -> Result<(), CMapError> {
    for block in find_blocks(toks, "beginbfchar", "endbfchar") {
        let mut i = 0usize;
        while i < block.len() {
            let (src, dst) = match (&block[i], block.get(i + 1)) {
                (Tok::Hex(s), Some(Tok::Hex(d))) => (*s, *d),
                (other, _) => {
                    return Err(CMapError::MalformedBlock {
                        block: "bfchar",
                        near: format!("{other:?}"),
                    });
                }
            };
            let code = parse_source_code(src)?;
            let text = decode_destination(dst)?;
            insert_code(map, code, text);
            i += 2;
        }
    }
    Ok(())
}

fn parse_bfrange_blocks(toks: &[Tok<'_>], map: &mut HashMap<u16, String>) -> Result<(), CMapError> {
    for block in find_blocks(toks, "beginbfrange", "endbfrange") {
        let mut i = 0usize;
        while i < block.len() {
            let (lo_raw, hi_raw) = match (&block[i], block.get(i + 1)) {
                (Tok::Hex(a), Some(Tok::Hex(b))) => (*a, *b),
                (other, _) => {
                    return Err(CMapError::MalformedBlock {
                        block: "bfrange",
                        near: format!("{other:?}"),
                    });
                }
            };
            let lo = parse_source_code(lo_raw)?;
            let hi = parse_source_code(hi_raw)?;
            if lo > hi {
                return Err(CMapError::RangeInverted { lo, hi });
            }

            match block.get(i + 2) {
                // Form 1: a single destination, incrementing across the range.
                Some(Tok::Hex(dst)) => {
                    for code in lo..=hi {
                        let text = offset_destination(dst, code - lo, lo, hi)?;
                        insert_code(map, code, text);
                    }
                    i += 3;
                }
                // Form 2: an array of destinations, one element per code.
                Some(Tok::OpenArray) => {
                    let mut items = Vec::new();
                    let mut j = i + 3;
                    while j < block.len() && block[j] != Tok::CloseArray {
                        match &block[j] {
                            Tok::Hex(h) => items.push(*h),
                            other => {
                                return Err(CMapError::MalformedBlock {
                                    block: "bfrange",
                                    near: format!("{other:?}"),
                                });
                            }
                        }
                        j += 1;
                    }
                    let need = (hi - lo + 1) as usize;
                    if items.len() != need {
                        return Err(CMapError::ArrayLengthMismatch {
                            lo,
                            hi,
                            need,
                            got: items.len(),
                        });
                    }
                    for (k, item) in items.iter().enumerate() {
                        let text = decode_destination(item)?;
                        insert_code(map, lo + k as u32, text);
                    }
                    i = j + 1;
                }
                _ => return Err(CMapError::UnterminatedBlock("bfrange")),
            }
        }
    }
    Ok(())
}

/// The source code is limited to 4 hex digits by `parse_source_code`, so it always fits `u16`.
fn insert_code(map: &mut HashMap<u16, String>, code: u32, text: String) {
    if let Ok(c) = u16::try_from(code) {
        map.insert(c, text);
    }
}
