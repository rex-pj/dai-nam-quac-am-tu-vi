//! Reading and subsetting TrueType fonts.
//!
//! **Why this project needs its own font subsetter.** The print uses the Nôm Na Tống font,
//! and 27 glyphs of the dictionary carry **Private Use Area** code points (PUA, U+F0000+).
//! PUA code points have no standard meaning: they only point at the right glyph *inside*
//! that font. A machine without it shows a square — and worse, a machine with a different
//! font that also defines those code points shows **the wrong character** with no warning.
//! There is no way to render these correctly other than shipping the font of the print itself.
//!
//! Fortunately the font **sits intact inside the source PDF**: 15,917,208 bytes, not
//! subsetted, with a complete `cmap` format 12. But 15.9 MB cannot be pushed onto readers,
//! so it must be cut down to exactly the characters the book uses.
//!
//! This module deliberately **depends on nothing but `thiserror`**: it is pure functions over
//! byte slices, so its tests run in milliseconds and touch no file.

pub mod subset;

use std::collections::{BTreeMap, BTreeSet};

/// An sfnt table tag, e.g. `glyf`.
pub type Tag = [u8; 4];

pub const HEAD: Tag = *b"head";
pub const HHEA: Tag = *b"hhea";
pub const MAXP: Tag = *b"maxp";
pub const HMTX: Tag = *b"hmtx";
pub const CMAP: Tag = *b"cmap";
pub const LOCA: Tag = *b"loca";
pub const GLYF: Tag = *b"glyf";
pub const NAME: Tag = *b"name";
pub const OS2: Tag = *b"OS/2";
pub const POST: Tag = *b"post";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
    #[error("font data too short: needs {need} bytes at {at}, only {have} available")]
    Truncated { at: usize, need: usize, have: usize },

    #[error("not TrueType: sfnt tag 0x{0:08X}")]
    NotTrueType(u32),

    #[error("missing table {}", String::from_utf8_lossy(.0))]
    MissingTable(Tag),

    #[error("cmap has no subtable usable beyond the BMP (format 12 required)")]
    NoUnicodeCmap,

    #[error("cmap format {0} is not supported")]
    UnsupportedCmap(u16),

    #[error("glyph {0} lies outside the loca table")]
    GlyphOutOfRange(u16),

    #[error("the font has {0} glyphs, past the format limit of 65535")]
    TooManyGlyphs(usize),

    #[error("{what} is {size} bytes, past the sfnt format limit")]
    TooLarge { what: &'static str, size: usize },

    #[error("the subset font dropped code point U+{0:04X}")]
    CodepointDropped(u32),

    #[error("U+{codepoint:04X} draws a different shape after subsetting (glyph {gid})")]
    OutlineMismatch { codepoint: u32, gid: u16 },
}

/// Read a big-endian integer, erroring instead of panicking when bytes are missing.
fn be16(data: &[u8], at: usize) -> Result<u16, FontError> {
    let bytes = data.get(at..at + 2).ok_or(FontError::Truncated {
        at,
        need: 2,
        have: data.len(),
    })?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be32(data: &[u8], at: usize) -> Result<u32, FontError> {
    let bytes = data.get(at..at + 4).ok_or(FontError::Truncated {
        at,
        need: 4,
        have: data.len(),
    })?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// A TrueType font with its table directory parsed.
pub struct Font<'a> {
    data: &'a [u8],
    tables: BTreeMap<Tag, (usize, usize)>,
}

impl<'a> Font<'a> {
    /// The sfnt tag of plain TrueType. `OTTO` (CFF) is deliberately **not** accepted:
    /// subsetting CFF is a different problem, and the font we need is TrueType anyway.
    const SFNT_TRUETYPE: u32 = 0x0001_0000;

    pub fn parse(data: &'a [u8]) -> Result<Self, FontError> {
        let tag = be32(data, 0)?;
        if tag != Self::SFNT_TRUETYPE {
            return Err(FontError::NotTrueType(tag));
        }
        let count = be16(data, 4)? as usize;
        let mut tables = BTreeMap::new();
        for i in 0..count {
            let rec = 12 + i * 16;
            let mut name: Tag = [0; 4];
            let raw = data.get(rec..rec + 4).ok_or(FontError::Truncated {
                at: rec,
                need: 4,
                have: data.len(),
            })?;
            name.copy_from_slice(raw);
            let off = be32(data, rec + 8)? as usize;
            let len = be32(data, rec + 12)? as usize;
            tables.insert(name, (off, len));
        }
        Ok(Self { data, tables })
    }

    pub fn table(&self, tag: Tag) -> Result<&'a [u8], FontError> {
        let (off, len) = *self.tables.get(&tag).ok_or(FontError::MissingTable(tag))?;
        self.data.get(off..off + len).ok_or(FontError::Truncated {
            at: off,
            need: len,
            have: self.data.len(),
        })
    }

    pub fn has(&self, tag: Tag) -> bool {
        self.tables.contains_key(&tag)
    }

    pub fn num_glyphs(&self) -> Result<u16, FontError> {
        be16(self.table(MAXP)?, 4)
    }

    /// `head.indexToLocFormat`: 0 = short loca (u16, in 2-byte units), 1 = long (u32).
    pub fn long_loca(&self) -> Result<bool, FontError> {
        Ok(be16(self.table(HEAD)?, 50)? == 1)
    }

    /// The `loca` table, always returned as **byte offsets** whichever form the font stores.
    ///
    /// The short form stores halved offsets; forgetting to double them is the classic bug,
    /// and it does not break the font outright, it just draws a few characters wrong — the kind
    /// of bug that must be written down rather than remembered.
    pub fn loca(&self) -> Result<Vec<u32>, FontError> {
        let raw = self.table(LOCA)?;
        let n = self.num_glyphs()? as usize + 1;
        let long = self.long_loca()?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(if long {
                be32(raw, i * 4)?
            } else {
                u32::from(be16(raw, i * 2)?) * 2
            });
        }
        Ok(out)
    }

    /// The outline bytes of one glyph. Empty means an empty glyph (a space), not an error.
    pub fn glyph(&self, gid: u16, loca: &[u32]) -> Result<&'a [u8], FontError> {
        let i = gid as usize;
        let (start, end) = match (loca.get(i), loca.get(i + 1)) {
            (Some(a), Some(b)) => (*a as usize, *b as usize),
            _ => return Err(FontError::GlyphOutOfRange(gid)),
        };
        if end <= start {
            return Ok(&[]);
        }
        let glyf = self.table(GLYF)?;
        glyf.get(start..end).ok_or(FontError::Truncated {
            at: start,
            need: end - start,
            have: glyf.len(),
        })
    }

    /// The code point → GID map, read from a format 12 subtable (full Unicode coverage).
    ///
    /// **Only** format 12 is accepted, deliberately: format 4 reaches only U+FFFF, while the
    /// 855 Ext-B and 27 PUA glyphs of this book all live beyond it. A font without format 12
    /// cannot serve this purpose, and saying so beats silently subsetting too little.
    pub fn unicode_map(&self) -> Result<BTreeMap<u32, u16>, FontError> {
        let cmap = self.table(CMAP)?;
        let count = be16(cmap, 2)? as usize;
        let mut best: Option<usize> = None;
        for i in 0..count {
            let rec = 4 + i * 8;
            let sub = be32(cmap, rec + 4)? as usize;
            if be16(cmap, sub)? == 12 {
                best = Some(sub);
            }
        }
        let sub = best.ok_or(FontError::NoUnicodeCmap)?;

        let groups = be32(cmap, sub + 12)? as usize;
        let mut out = BTreeMap::new();
        for g in 0..groups {
            let rec = sub + 16 + g * 12;
            let lo = be32(cmap, rec)?;
            let hi = be32(cmap, rec + 4)?;
            let first_gid = be32(cmap, rec + 8)?;
            for (n, cp) in (lo..=hi).enumerate() {
                let gid = first_gid + n as u32;
                if let Ok(gid) = u16::try_from(gid) {
                    out.insert(cp, gid);
                }
            }
        }
        Ok(out)
    }

    /// The GIDs a composite glyph refers to.
    ///
    /// A composite glyph (`numberOfContours < 0`) is drawn by calling other glyphs.
    /// Subsetting without pulling those in renders the character **with strokes missing** —
    /// still a character, just the wrong one. Recursion is needed: components can be composite too.
    pub fn components(&self, glyph: &[u8]) -> Result<Vec<u16>, FontError> {
        const ARGS_ARE_WORDS: u16 = 0x0001;
        const HAVE_SCALE: u16 = 0x0008;
        const MORE_COMPONENTS: u16 = 0x0020;
        const HAVE_XY_SCALE: u16 = 0x0040;
        const HAVE_2X2: u16 = 0x0080;

        if glyph.len() < 10 || be16(glyph, 0)? as i16 >= 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut at = 10;
        loop {
            let flags = be16(glyph, at)?;
            out.push(be16(glyph, at + 2)?);
            at += 4;
            at += if flags & ARGS_ARE_WORDS != 0 { 4 } else { 2 };
            at += if flags & HAVE_2X2 != 0 {
                8
            } else if flags & HAVE_XY_SCALE != 0 {
                4
            } else if flags & HAVE_SCALE != 0 {
                2
            } else {
                0
            };
            if flags & MORE_COMPONENTS == 0 {
                return Ok(out);
            }
        }
    }

    /// Expand a GID set to include every component those glyphs need.
    pub fn closure(&self, seeds: &BTreeSet<u16>) -> Result<BTreeSet<u16>, FontError> {
        let loca = self.loca()?;
        // GID 0 (.notdef) must always be present: the spec requires it, and it is what a failed lookup shows.
        let mut keep: BTreeSet<u16> = BTreeSet::from([0]);
        let mut queue: Vec<u16> = seeds.iter().copied().collect();
        while let Some(gid) = queue.pop() {
            if !keep.insert(gid) {
                continue;
            }
            for c in self.components(self.glyph(gid, &loca)?)? {
                if !keep.contains(&c) {
                    queue.push(c);
                }
            }
        }
        Ok(keep)
    }
}

impl std::fmt::Debug for Font<'_> {
    /// Print the table list rather than 15.9 MB of bytes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tags: Vec<String> = self
            .tables
            .keys()
            .map(|t| String::from_utf8_lossy(t).into_owned())
            .collect();
        f.debug_struct("Font")
            .field("bytes", &self.data.len())
            .field("tables", &tags)
            .finish()
    }
}
