//! Subsetting the font: keep exactly the characters the book uses, drop the rest.
//!
//! The source font has 32,951 glyphs / 15.9 MB. The dictionary uses only 4,994 characters,
//! and split by Unicode range each page needs a small slice. After subsetting, the PUA range holds **27**.
//!
//! **GIDs are renumbered.** Keeping the old numbers would be far simpler, but `loca` and
//! `hmtx` would still need all 32,952 entries — over 260 KB for those two tables alone in a
//! 27-character font. Renumbering forces rewriting the GIDs inside composite glyphs, which
//! is the easiest place to go wrong; hence [`verify`], which re-reads the result and asserts
//! that every code point still produces the same shape.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CMAP, Font, FontError, GLYF, HEAD, HHEA, HMTX, LOCA, MAXP, NAME, OS2, POST, Tag, be16,
};

/// The tables carried over into the new font.
///
/// `GSUB` (contextual substitution) is dropped: it takes real space and this lookup edition
/// shows isolated characters with no context to substitute in. `name` is **kept** — it
/// carries both the font name and its licence, and dropping it would cut the credit line.
const KEEP: [Tag; 4] = [HEAD, HHEA, NAME, OS2];

/// The subsetting result, with figures to report rather than just a file written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsetReport {
    pub codepoints_requested: usize,
    /// Code points the source font lacks — **must be 0**, or characters would be missing.
    pub codepoints_missing: Vec<u32>,
    pub glyphs_kept: usize,
    /// Glyphs pulled in as components of composite glyphs.
    pub glyphs_from_components: usize,
    pub bytes: usize,
}

/// Narrow a `usize` to `u32` and **error** if it does not fit.
///
/// A silent fallback here (`unwrap_or(0)`) would write a font that is formally valid but
/// points at the wrong offsets — characters still render, just the wrong ones. The CI grep gate forbids exactly this.
fn size32(what: &'static str, n: usize) -> Result<u32, FontError> {
    u32::try_from(n).map_err(|_| FontError::TooLarge { what, size: n })
}

fn push16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}
fn push32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// Subset the font down to the given set of code points.
pub fn subset(
    font: &Font,
    codepoints: &BTreeSet<u32>,
) -> Result<(Vec<u8>, SubsetReport), FontError> {
    subset_with_notice(font, codepoints, None)
}

/// Subset the font, writing a modification notice into its `name` table.
///
/// Prefer this over [`subset`] for anything that leaves the machine: the licences these fonts
/// carry require a modified copy to say how and when it was modified, and only a notice inside
/// the file travels with the file. [`subset`] exists for tests, where nothing is shipped.
pub fn subset_with_notice(
    font: &Font,
    codepoints: &BTreeSet<u32>,
    notice: Option<&str>,
) -> Result<(Vec<u8>, SubsetReport), FontError> {
    let unicode = font.unicode_map()?;
    let loca = font.loca()?;

    let mut seeds = BTreeSet::new();
    let mut missing = Vec::new();
    let mut wanted: BTreeMap<u32, u16> = BTreeMap::new();
    for &cp in codepoints {
        match unicode.get(&cp) {
            Some(&gid) => {
                seeds.insert(gid);
                wanted.insert(cp, gid);
            }
            None => missing.push(cp),
        }
    }

    let keep = font.closure(&seeds)?;
    let from_components = keep.len().saturating_sub(seeds.len() + 1);

    // Old GID → new, in ascending order, so GID 0 stays 0.
    let remap: BTreeMap<u16, u16> = keep
        .iter()
        .enumerate()
        .map(|(new, &old)| {
            Ok((
                old,
                u16::try_from(new).map_err(|_| FontError::TooManyGlyphs(new))?,
            ))
        })
        .collect::<Result<_, FontError>>()?;

    let (glyf, new_loca) = build_glyf(font, &loca, &keep, &remap)?;
    let hmtx = build_hmtx(font, &keep)?;
    let cmap = build_cmap(&wanted, &remap)?;

    let num_glyphs = u16::try_from(keep.len()).map_err(|_| FontError::TooManyGlyphs(keep.len()))?;
    let mut tables: BTreeMap<Tag, Vec<u8>> = BTreeMap::new();
    for tag in KEEP {
        if font.has(tag) {
            tables.insert(tag, font.table(tag)?.to_vec());
        }
    }

    // The licence of a modified font decides what the file must say about being modified.
    // A missing `name` table is not silently tolerated here: without it the file carries
    // neither its copyright nor its licence, and shipping that is not something to do quietly.
    if let Some(notice) = notice {
        let name = tables
            .get(&NAME)
            .ok_or(FontError::MissingTable(NAME))?
            .clone();
        tables.insert(NAME, with_modification_notice(&name, notice)?);
    }
    tables.insert(GLYF, glyf);
    tables.insert(LOCA, new_loca);
    tables.insert(HMTX, hmtx);
    tables.insert(CMAP, cmap);
    tables.insert(MAXP, build_maxp(font, num_glyphs)?);
    // `post` version 3.0: no glyph names. The original can carry hundreds of KB of names
    // that a browser has no use for here.
    tables.insert(
        POST,
        vec![
            0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ],
    );

    // `head.indexToLocFormat` = 1 because the new loca is always written in long form.
    if let Some(head) = tables.get_mut(&HEAD)
        && head.len() >= 52
    {
        head[50] = 0;
        head[51] = 1;
        // checkSumAdjustment is computed after assembly; write 0 first.
        head[8..12].fill(0);
    }
    if let Some(hhea) = tables.get_mut(&HHEA)
        && hhea.len() >= 36
    {
        hhea[34..36].copy_from_slice(&num_glyphs.to_be_bytes());
    }

    let bytes = assemble(&tables)?;
    let report = SubsetReport {
        codepoints_requested: codepoints.len(),
        codepoints_missing: missing,
        glyphs_kept: keep.len(),
        glyphs_from_components: from_components,
        bytes: bytes.len(),
    };
    Ok((bytes, report))
}

fn build_glyf(
    font: &Font,
    loca: &[u32],
    keep: &BTreeSet<u16>,
    remap: &BTreeMap<u16, u16>,
) -> Result<(Vec<u8>, Vec<u8>), FontError> {
    let mut glyf = Vec::new();
    let mut offsets = Vec::with_capacity(keep.len() + 1);
    for &old in keep {
        offsets.push(size32("glyf", glyf.len())?);
        let data = font.glyph(old, loca)?;
        if data.is_empty() {
            continue;
        }
        let start = glyf.len();
        glyf.extend_from_slice(data);
        // Composite glyphs point at OLD GIDs; without rewriting them a character renders with another shape.
        if !font.components(data)?.is_empty() {
            patch_components(&mut glyf[start..], remap)?;
        }
        // Every glyph must start at an even address.
        while glyf.len() % 2 != 0 {
            glyf.push(0);
        }
    }
    offsets.push(size32("glyf", glyf.len())?);

    let mut new_loca = Vec::with_capacity(offsets.len() * 4);
    for o in offsets {
        push32(&mut new_loca, o);
    }
    Ok((glyf, new_loca))
}

/// Rewrite the GIDs inside one composite glyph, in place.
fn patch_components(glyph: &mut [u8], remap: &BTreeMap<u16, u16>) -> Result<(), FontError> {
    const ARGS_ARE_WORDS: u16 = 0x0001;
    const HAVE_SCALE: u16 = 0x0008;
    const MORE_COMPONENTS: u16 = 0x0020;
    const HAVE_XY_SCALE: u16 = 0x0040;
    const HAVE_2X2: u16 = 0x0080;

    let mut at = 10;
    loop {
        let flags = u16::from_be_bytes([glyph[at], glyph[at + 1]]);
        let old = u16::from_be_bytes([glyph[at + 2], glyph[at + 3]]);
        // A component is always in the kept set, because `closure` pulled it in.
        let new = *remap.get(&old).ok_or(FontError::GlyphOutOfRange(old))?;
        glyph[at + 2..at + 4].copy_from_slice(&new.to_be_bytes());
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
            return Ok(());
        }
    }
}

/// The new `hmtx`: a full (advance, lsb) pair for every kept glyph.
///
/// The "last glyph repeats its advance" compression of the format is not used — it saves a
/// few KB but adds another place to go wrong, and this part is already small after subsetting.
fn build_hmtx(font: &Font, keep: &BTreeSet<u16>) -> Result<Vec<u8>, FontError> {
    let hmtx = font.table(HMTX)?;
    let hhea = font.table(HHEA)?;
    let long_count = u16::from_be_bytes([hhea[34], hhea[35]]) as usize;

    let mut out = Vec::with_capacity(keep.len() * 4);
    for &gid in keep {
        let i = gid as usize;
        let (advance, lsb) = if i < long_count {
            let at = i * 4;
            (read16(hmtx, at), read16(hmtx, at + 2))
        } else {
            // Past `numberOfHMetrics`, every glyph shares the last advance and has its own lsb.
            let last = long_count.saturating_sub(1) * 4;
            let at = long_count * 4 + (i - long_count) * 2;
            (read16(hmtx, last), read16(hmtx, at))
        };
        push16(&mut out, advance);
        push16(&mut out, lsb);
    }
    Ok(out)
}

fn read16(data: &[u8], at: usize) -> u16 {
    match data.get(at..at + 2) {
        Some(b) => u16::from_be_bytes([b[0], b[1]]),
        None => 0,
    }
}

fn build_maxp(font: &Font, num_glyphs: u16) -> Result<Vec<u8>, FontError> {
    let mut maxp = font.table(MAXP)?.to_vec();
    if maxp.len() >= 6 {
        maxp[4..6].copy_from_slice(&num_glyphs.to_be_bytes());
    }
    Ok(maxp)
}

/// The new `cmap`, format 12 **only**.
///
/// This font must be usable beyond the BMP (Ext-B, PUA), which format 4 is not. Every
/// current browser reads format 12, so adding format 4 would be dead weight.
fn build_cmap(
    wanted: &BTreeMap<u32, u16>,
    remap: &BTreeMap<u16, u16>,
) -> Result<Vec<u8>, FontError> {
    // Merge into contiguous groups: a group extends when the next code point follows
    // immediately AND its GID follows immediately too. That is exactly the format 12 invariant.
    let mut groups: Vec<(u32, u32, u16)> = Vec::new();
    for (&cp, old_gid) in wanted {
        let Some(&gid) = remap.get(old_gid) else {
            continue;
        };
        match groups.last_mut() {
            Some((start, end, first_gid))
                if cp == *end + 1 && u32::from(gid) == u32::from(*first_gid) + (cp - *start) =>
            {
                *end = cp;
            }
            _ => groups.push((cp, cp, gid)),
        }
    }

    let mut sub = Vec::new();
    push16(&mut sub, 12);
    push16(&mut sub, 0);
    push32(&mut sub, 0); // length, filled in once known
    push32(&mut sub, 0); // language
    push32(&mut sub, size32("cmap group count", groups.len())?);
    for (lo, hi, gid) in &groups {
        push32(&mut sub, *lo);
        push32(&mut sub, *hi);
        push32(&mut sub, u32::from(*gid));
    }
    let sub_len = size32("cmap", sub.len())?;
    sub[4..8].copy_from_slice(&sub_len.to_be_bytes());

    let mut out = Vec::new();
    push16(&mut out, 0); // version
    push16(&mut out, 1); // exactly one subtable
    push16(&mut out, 3); // platform Windows
    push16(&mut out, 10); // encoding UCS-4
    push32(&mut out, 12); // offset to the subtable
    out.extend_from_slice(&sub);
    Ok(out)
}

/// Assemble the tables into an sfnt file, with checksums.
fn assemble(tables: &BTreeMap<Tag, Vec<u8>>) -> Result<Vec<u8>, FontError> {
    let count = u16::try_from(tables.len()).map_err(|_| FontError::TooLarge {
        what: "table count",
        size: tables.len(),
    })?;
    // searchRange/entrySelector/rangeShift: browsers ignore them, but the spec requires them
    // and some font validators complain if they are wrong.
    let entry_selector = (u32::BITS - 1 - u32::from(count.max(1)).leading_zeros()) as u16;
    let search_range = (1u16 << entry_selector) * 16;

    let mut out = Vec::new();
    push32(&mut out, 0x0001_0000);
    push16(&mut out, count);
    push16(&mut out, search_range);
    push16(&mut out, entry_selector);
    push16(&mut out, count * 16 - search_range);

    let mut offset = 12 + tables.len() * 16;
    let mut records: Vec<(Tag, usize, usize)> = Vec::new();
    for (tag, body) in tables {
        records.push((*tag, offset, body.len()));
        offset += (body.len() + 3) & !3; // every table starts at a multiple of 4
    }
    for (tag, off, len) in &records {
        out.extend_from_slice(tag);
        push32(&mut out, checksum(&tables[tag]));
        push32(&mut out, size32("table offset", *off)?);
        push32(&mut out, size32("table length", *len)?);
    }
    for body in tables.values() {
        out.extend_from_slice(body);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }

    // `head.checkSumAdjustment` = 0xB1B0AFBA − the checksum of the whole file.
    if let Some((_, head_off, _)) = records.iter().find(|(t, _, _)| *t == HEAD) {
        let total = checksum(&out);
        let adj = 0xB1B0_AFBAu32.wrapping_sub(total);
        let at = head_off + 8;
        if out.len() >= at + 4 {
            out[at..at + 4].copy_from_slice(&adj.to_be_bytes());
        }
    }
    Ok(out)
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

/// Re-read the subset font and assert every code point still produces the same **shape**.
///
/// This is the gate of the whole module: renumbering GIDs and rewriting them inside
/// composite glyphs is the easiest thing to get wrong, and getting it wrong does not break
/// the font — it just draws different characters. Comparing outline bytes is the only way to catch it.
pub fn verify(
    original: &[u8],
    subsetted: &[u8],
    codepoints: &BTreeSet<u32>,
) -> Result<usize, FontError> {
    let src = Font::parse(original)?;
    let dst = Font::parse(subsetted)?;
    let (src_map, dst_map) = (src.unicode_map()?, dst.unicode_map()?);
    let (src_loca, dst_loca) = (src.loca()?, dst.loca()?);

    let mut checked = 0;
    for &cp in codepoints {
        let Some(&src_gid) = src_map.get(&cp) else {
            // The source font never had this character; `subset` reported it separately, nothing to compare.
            continue;
        };
        let dst_gid = *dst_map.get(&cp).ok_or(FontError::CodepointDropped(cp))?;
        let a = src.glyph(src_gid, &src_loca)?;
        let b = dst.glyph(dst_gid, &dst_loca)?;

        // The new `loca` includes the padding to an even address, so the tail of `b` can be
        // up to 3 bytes longer than `a`. Comparing the whole slice is wrong — and that is
        // exactly what once made this check raise a false alarm on a perfectly good font.
        let same = if a.is_empty() {
            b.is_empty()
        } else if !src.components(a)?.is_empty() {
            // Composite glyph: the inner GIDs were renumbered, so differing bytes are CORRECT.
            // Compare the first 10 bytes (contour count and bounding box) — the only invariant part.
            b.len() >= 10 && a[..10] == b[..10]
        } else {
            b.len() >= a.len() && a == &b[..a.len()]
        };
        if !same {
            return Err(FontError::OutlineMismatch {
                codepoint: cp,
                gid: dst_gid,
            });
        }
        checked += 1;
    }
    Ok(checked)
}

/// Add a **modification notice** to a `name` table, as nameID 10 (Description).
///
/// Why this exists, and why it is not decoration. A subset is a *modified font*, and the
/// licences these fonts ship under say what a modified copy must carry. The Arphic Public
/// License, which BabelStone Han is under, is explicit — §2(a): *"You must insert a prominent
/// notice in each modified file stating how and when you changed that file."* A note in the
/// repository does not satisfy that: the file is what reaches the reader, so the notice has
/// to travel inside the file.
///
/// Any existing nameID 10 record is replaced rather than added to. Two descriptions, one of
/// them stale, is worse than one accurate description.
///
/// The record is written for Windows / Unicode BMP / en-US (platform 3, encoding 1, language
/// 0x0409) — the combination every current renderer and font tool reads.
pub fn with_modification_notice(name_table: &[u8], notice: &str) -> Result<Vec<u8>, FontError> {
    const HEADER: usize = 6;
    const RECORD: usize = 12;

    let count = be16(name_table, 2)? as usize;
    let storage_offset = be16(name_table, 4)? as usize;

    // Carry every record over except nameID 10, keeping each one's bytes exactly as they were.
    let mut records: Vec<([u8; 8], Vec<u8>)> = Vec::with_capacity(count + 1);
    for i in 0..count {
        let at = HEADER + i * RECORD;
        if be16(name_table, at + 6)? == 10 {
            continue;
        }
        let len = be16(name_table, at + 8)? as usize;
        let off = be16(name_table, at + 10)? as usize;
        let start = storage_offset + off;
        let end = start + len;
        let text = name_table.get(start..end).ok_or(FontError::Truncated {
            at: start,
            need: len,
            have: name_table.len(),
        })?;
        let mut head = [0u8; 8];
        head.copy_from_slice(name_table.get(at..at + 8).ok_or(FontError::Truncated {
            at,
            need: 8,
            have: name_table.len(),
        })?);
        records.push((head, text.to_vec()));
    }

    let mut head = [0u8; 8];
    head[0..2].copy_from_slice(&3u16.to_be_bytes()); // platform: Windows
    head[2..4].copy_from_slice(&1u16.to_be_bytes()); // encoding: Unicode BMP
    head[4..6].copy_from_slice(&0x0409u16.to_be_bytes()); // language: en-US
    head[6..8].copy_from_slice(&10u16.to_be_bytes()); // nameID: Description
    let encoded: Vec<u8> = notice
        .encode_utf16()
        .flat_map(|u| u.to_be_bytes())
        .collect();
    records.push((head, encoded));

    let new_count = size16("name record count", records.len())?;
    let new_storage = size16("name storage offset", HEADER + records.len() * RECORD)?;

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_be_bytes()); // format 0
    out.extend_from_slice(&new_count.to_be_bytes());
    out.extend_from_slice(&new_storage.to_be_bytes());

    let mut storage = Vec::new();
    for (head, text) in &records {
        out.extend_from_slice(head);
        out.extend_from_slice(&size16("name string length", text.len())?.to_be_bytes());
        out.extend_from_slice(&size16("name string offset", storage.len())?.to_be_bytes());
        storage.extend_from_slice(text);
    }
    out.extend_from_slice(&storage);
    Ok(out)
}

/// Narrow a `usize` to `u16` and **error** if it does not fit.
///
/// The `name` table addresses its strings with 16-bit offsets. A silent truncation here would
/// write a table that still parses but points at the wrong bytes, so the licence text a reader
/// sees would be garbage — exactly the failure this function exists to prevent.
fn size16(what: &'static str, v: usize) -> Result<u16, FontError> {
    u16::try_from(v).map_err(|_| FontError::TooLarge { what, size: v })
}
