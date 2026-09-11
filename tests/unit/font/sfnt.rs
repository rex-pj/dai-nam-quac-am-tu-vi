#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

//! Subsetting a TrueType font.
//!
//! A **synthetic** font is built rather than using the real one: the real font lives inside
//! a 43 MB PDF, and a unit test should read no file. The font here is tiny but contains the
//! hardest case — a **composite glyph** pointing at two others. That is where GID
//! renumbering goes wrong, and going wrong there **does not break the font**: it just draws

use std::collections::BTreeSet;

use dnqatv_font_core::{Font, FontError, subset};

// ── Building the synthetic font ──────────────────────────────────────────────

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}
fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// A simple glyph: one contour, one point. The content does not matter — the subsetter copies
/// bytes verbatim; only `numberOfContours >= 0` is meaningful (it separates simple from composite).
fn simple_glyph(marker: u8) -> Vec<u8> {
    let mut g = Vec::new();
    g.extend_from_slice(&be16(1)); // numberOfContours
    g.extend_from_slice(&be16(0)); // xMin
    g.extend_from_slice(&be16(0)); // yMin
    g.extend_from_slice(&be16(100)); // xMax
    g.extend_from_slice(&be16(100)); // yMax
    g.extend_from_slice(&be16(0)); // endPtsOfContours[0]
    g.extend_from_slice(&be16(0)); // instructionLength
    g.push(0x01); // flags: on-curve
    g.extend_from_slice(&be16(u16::from(marker))); // x
    g.extend_from_slice(&be16(u16::from(marker))); // y
    g
}

/// A composite glyph pointing at two others — exactly what the subsetter must pull in and renumber.
fn composite_glyph(first: u16, second: u16) -> Vec<u8> {
    const ARGS_ARE_WORDS: u16 = 0x0001;
    const MORE_COMPONENTS: u16 = 0x0020;

    let mut g = Vec::new();
    g.extend_from_slice(&be16(u16::MAX)); // numberOfContours = -1
    g.extend_from_slice(&be16(0));
    g.extend_from_slice(&be16(0));
    g.extend_from_slice(&be16(200));
    g.extend_from_slice(&be16(200));

    g.extend_from_slice(&be16(ARGS_ARE_WORDS | MORE_COMPONENTS));
    g.extend_from_slice(&be16(first));
    g.extend_from_slice(&be16(0));
    g.extend_from_slice(&be16(0));

    g.extend_from_slice(&be16(ARGS_ARE_WORDS));
    g.extend_from_slice(&be16(second));
    g.extend_from_slice(&be16(50));
    g.extend_from_slice(&be16(50));
    g
}

/// A four-glyph font: `.notdef`, A, B, and C composed from A + B.
///
/// The cmap uses three **widely separated** code points so that the format 12 grouping
/// cannot pass by accident — adjacent ones would let a grouping bug still give the right answer.
fn synthetic_font() -> Vec<u8> {
    const A: u32 = 0x4E00; // BMP
    const B: u32 = 0x28C32; // Ext-B
    const C: u32 = 0xF15A4; // PUA

    let glyphs: [Vec<u8>; 4] = [
        Vec::new(), // .notdef is empty
        simple_glyph(11),
        simple_glyph(22),
        composite_glyph(1, 2),
    ];

    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for g in &glyphs {
        loca.extend_from_slice(&be32(u32::try_from(glyf.len()).expect("small")));
        glyf.extend_from_slice(g);
        while glyf.len() % 2 != 0 {
            glyf.push(0);
        }
    }
    loca.extend_from_slice(&be32(u32::try_from(glyf.len()).expect("small")));

    let mut head = vec![0u8; 54];
    head[50..52].copy_from_slice(&be16(1)); // indexToLocFormat = long

    let mut hhea = vec![0u8; 36];
    hhea[34..36].copy_from_slice(&be16(4)); // numberOfHMetrics

    let mut maxp = vec![0u8; 32];
    maxp[4..6].copy_from_slice(&be16(4)); // numGlyphs

    let mut hmtx = Vec::new();
    for i in 0..4u16 {
        hmtx.extend_from_slice(&be16(500 + i)); // advance
        hmtx.extend_from_slice(&be16(i)); // lsb
    }

    // cmap format 12, one group per code point.
    let mut sub = Vec::new();
    sub.extend_from_slice(&be16(12));
    sub.extend_from_slice(&be16(0));
    sub.extend_from_slice(&be32(0)); // length, filled in later
    sub.extend_from_slice(&be32(0)); // language
    sub.extend_from_slice(&be32(3));
    for (cp, gid) in [(A, 1u32), (B, 2), (C, 3)] {
        sub.extend_from_slice(&be32(cp));
        sub.extend_from_slice(&be32(cp));
        sub.extend_from_slice(&be32(gid));
    }
    let sub_len = u32::try_from(sub.len()).expect("small");
    sub[4..8].copy_from_slice(&be32(sub_len));

    let mut cmap = Vec::new();
    cmap.extend_from_slice(&be16(0));
    cmap.extend_from_slice(&be16(1));
    cmap.extend_from_slice(&be16(3));
    cmap.extend_from_slice(&be16(10));
    cmap.extend_from_slice(&be32(12));
    cmap.extend_from_slice(&sub);

    let tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
    ];

    let count = u16::try_from(tables.len()).expect("small");
    let mut out = Vec::new();
    out.extend_from_slice(&be32(0x0001_0000));
    out.extend_from_slice(&be16(count));
    out.extend_from_slice(&be16(0));
    out.extend_from_slice(&be16(0));
    out.extend_from_slice(&be16(0));

    let mut offset = 12 + tables.len() * 16;
    for (tag, body) in &tables {
        out.extend_from_slice(*tag);
        out.extend_from_slice(&be32(0));
        out.extend_from_slice(&be32(u32::try_from(offset).expect("small")));
        out.extend_from_slice(&be32(u32::try_from(body.len()).expect("small")));
        offset += (body.len() + 3) & !3;
    }
    for (_, body) in &tables {
        out.extend_from_slice(body);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    out
}

const CP_BMP: u32 = 0x4E00;
const CP_EXT_B: u32 = 0x28C32;
const CP_PUA: u32 = 0xF15A4;

// ── Reading a font ───────────────────────────────────────────────────────────

#[test]
fn reads_the_table_directory_and_the_glyph_count() {
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    assert_eq!(font.num_glyphs().expect("maxp"), 4);
    assert!(font.long_loca().expect("head"));
    assert_eq!(font.loca().expect("loca").len(), 5);
}

#[test]
fn rejects_anything_that_is_not_truetype() {
    let err = Font::parse(b"OTTO\0\0\0\0\0\0\0\0").expect_err("must be rejected");
    assert!(matches!(err, FontError::NotTrueType(_)));
}

#[test]
fn looks_up_code_points_beyond_the_bmp() {
    // This is why the module accepts only cmap format 12: the 855 Ext-B and 27 PUA glyphs of
    // the book all live past U+FFFF, which format 4 cannot reach.
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let map = font.unicode_map().expect("cmap");
    assert_eq!(map.get(&CP_BMP), Some(&1));
    assert_eq!(map.get(&CP_EXT_B), Some(&2));
    assert_eq!(map.get(&CP_PUA), Some(&3));
}

#[test]
fn finds_the_components_of_a_composite_glyph() {
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let loca = font.loca().expect("loca");
    assert_eq!(
        font.components(font.glyph(3, &loca).expect("glyph"))
            .expect("components"),
        vec![1, 2]
    );
    // A simple glyph has no components.
    assert!(
        font.components(font.glyph(1, &loca).expect("glyph"))
            .expect("components")
            .is_empty()
    );
}

// ── Subsetting ───────────────────────────────────────────────────────────────

#[test]
fn pulls_in_the_components_of_a_composite_glyph() {
    // Only ONE character is requested, and it is composed of two others. Missing them makes
    // the character render **with strokes missing** — still a character, just wrong, silently.
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let (_, report) = subset::subset(&font, &BTreeSet::from([CP_PUA])).expect("subsets");

    assert!(report.codepoints_missing.is_empty());
    // .notdef + C + the two components A and B.
    assert_eq!(report.glyphs_kept, 4);
    assert_eq!(report.glyphs_from_components, 2);
}

#[test]
fn renumbers_gids_including_those_inside_composite_glyphs() {
    // Two characters are requested: the composite (GID 3) and the Ext-B one (GID 2). The new
    // font renumbers from 0, so the GIDs inside the composite MUST follow, or they point at
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let want = BTreeSet::from([CP_EXT_B, CP_PUA]);
    let (cut, _) = subset::subset(&font, &want).expect("subsets");

    let new_font = Font::parse(&cut).expect("the new font is valid");
    let map = new_font.unicode_map().expect("new cmap");
    let loca = new_font.loca().expect("new loca");
    let num = new_font.num_glyphs().expect("new maxp");

    let gid = *map
        .get(&CP_PUA)
        .expect("the composite is still in the cmap");
    let parts = new_font
        .components(new_font.glyph(gid, &loca).expect("glyph"))
        .expect("components");
    assert_eq!(parts.len(), 2);
    for p in parts {
        assert!(
            p < num,
            "a component points at GID {p}, outside the new font ({num} glyphs)"
        );
    }
}

#[test]
fn verification_compares_every_outline_with_the_original() {
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let want = BTreeSet::from([CP_BMP, CP_EXT_B, CP_PUA]);
    let (cut, _) = subset::subset(&font, &want).expect("subsets");

    assert_eq!(subset::verify(&data, &cut, &want).expect("matches"), 3);
}

#[test]
fn verification_catches_a_tampered_font() {
    // The gate must NOT be vacuous: flipping one byte of an outline must make it complain.
    // Without this check, a green `verify` would prove nothing.
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let want = BTreeSet::from([CP_BMP]);
    let (mut cut, _) = subset::subset(&font, &want).expect("subsets");

    let new_font = Font::parse(&cut).expect("new font");
    let loca = new_font.loca().expect("loca");
    let gid = *new_font
        .unicode_map()
        .expect("cmap")
        .get(&CP_BMP)
        .expect("present");
    // Find the first outline byte and flip it.
    let glyf_at = cut
        .windows(4)
        .position(|w| w == b"glyf")
        .expect("the glyf table is present");
    let table_off = u32::from_be_bytes([
        cut[glyf_at + 8],
        cut[glyf_at + 9],
        cut[glyf_at + 10],
        cut[glyf_at + 11],
    ]) as usize;
    let at = table_off + loca[gid as usize] as usize;
    cut[at] ^= 0xFF;

    assert!(
        subset::verify(&data, &cut, &want).is_err(),
        "if a tampered outline still verifies green, the check is useless"
    );
}

#[test]
fn reports_code_points_the_font_lacks_instead_of_skipping_them() {
    // The print uses several CJK fonts; Nôm Na Tống does not cover them all. A character it
    // lacks must be NAMED so the caller can decide — never vanish silently.
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let absent = 0x9FFF;
    let (_, report) =
        subset::subset(&font, &BTreeSet::from([CP_BMP, absent])).expect("still subsets");

    assert_eq!(report.codepoints_missing, vec![absent]);
}

#[test]
fn the_subset_font_is_much_smaller_than_the_original() {
    let data = synthetic_font();
    let font = Font::parse(&data).expect("valid font");
    let (cut, report) = subset::subset(&font, &BTreeSet::from([CP_BMP])).expect("subsets");

    // Only .notdef plus one character is kept.
    assert_eq!(report.glyphs_kept, 2);
    assert_eq!(
        Font::parse(&cut)
            .expect("valid")
            .num_glyphs()
            .expect("maxp"),
        2
    );
}
