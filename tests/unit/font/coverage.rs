#![allow(clippy::expect_used, clippy::panic)]

//! Every Han-Nom character in the data must be covered by a shipped font file.
//!
//! Why this is a test and not a note: the shipped fonts are subsets, and a subset is only
//! correct with respect to the data it was cut for. Reload the data with one new character
//! and the fonts silently stop covering it — the site keeps working, the character quietly
//! falls through to whatever CJK font the reader happens to have, and on many machines it
//! shows as ▯. Nothing else in the system looks at that.
//!
//! Skipped (not failed) when the data or the fonts are absent: `data/` is not in the repo,
//! so CI has neither, and a checkout without them must still run `cargo test`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dnqatv_font_core::Font;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The code points every shipped font file covers, or `None` when nothing is shipped.
fn shipped(dir: &Path) -> Option<BTreeSet<u32>> {
    let mut out = BTreeSet::new();
    let mut found_any = false;
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ttf") {
            continue;
        }
        found_any = true;
        let data = std::fs::read(&path).expect("reading the font");
        let font = Font::parse(&data)
            .unwrap_or_else(|e| panic!("{} is not a readable TrueType font: {e}", path.display()));
        let map = font
            .unicode_map()
            .unwrap_or_else(|e| panic!("{} has no usable cmap: {e}", path.display()));
        out.extend(map.keys().copied());
    }
    found_any.then_some(out)
}

/// Every Han-Nom code point the data uses, with one entry that shows where it came from.
fn used(path: &Path) -> Option<Vec<(u32, String)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {}: {e}", i + 1));
        let reading = v.get("reading").and_then(|r| r.as_str()).unwrap_or("?");
        let page = v.get("printed_page").and_then(|p| p.as_u64()).unwrap_or(0);
        let mut take = |s: &str, out: &mut Vec<(u32, String)>| {
            for c in s.chars().filter(|c| *c as u32 > 0x2E7F) {
                if seen.insert(c as u32) {
                    out.push((c as u32, format!("{reading} tr.{page}")));
                }
            }
        };
        if let Some(g) = v.get("glyph").and_then(|g| g.as_str()) {
            take(g, &mut out);
        }
        if let Some(subs) = v.get("sub_entries").and_then(|s| s.as_array()) {
            for s in subs {
                for key in ["han_form", "han_expanded"] {
                    if let Some(h) = s.get(key).and_then(|h| h.as_str()) {
                        take(h, &mut out);
                    }
                }
            }
        }
    }
    Some(out)
}

#[test]
fn every_character_in_the_data_has_a_shipped_font() {
    let root = root();
    let Some(covered) = shipped(&root.join("frontend/fonts")) else {
        eprintln!("skipped: no font shipped under frontend/fonts");
        return;
    };
    let Some(used) = used(&root.join("data/entries.jsonl")) else {
        eprintln!("skipped: data/entries.jsonl is absent, run `parse` first");
        return;
    };

    let missing: Vec<String> = used
        .iter()
        .filter(|(cp, _)| !covered.contains(cp))
        .map(|(cp, where_)| {
            format!(
                "U+{cp:05X} {} ({where_})",
                char::from_u32(*cp).unwrap_or('?')
            )
        })
        .collect();

    assert!(
        missing.is_empty(),
        "{} character(s) in the data have no shipped font — they will render as ▯ on machines \
         without a matching CJK font. Re-run `dnqatv-font <PDF> data/entries.jsonl frontend/fonts`.\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

#[test]
fn the_shipped_fonts_carry_no_character_the_data_does_not_use() {
    // The other direction. A subset holding characters nothing needs means the subset was cut
    // for different data, which makes its size and its provenance both wrong.
    let root = root();
    let Some(covered) = shipped(&root.join("frontend/fonts")) else {
        eprintln!("skipped: no font shipped under frontend/fonts");
        return;
    };
    let Some(used) = used(&root.join("data/entries.jsonl")) else {
        eprintln!("skipped: data/entries.jsonl is absent, run `parse` first");
        return;
    };
    let needed: BTreeSet<u32> = used.iter().map(|(cp, _)| *cp).collect();

    let extra: Vec<String> = covered
        .difference(&needed)
        .map(|cp| format!("U+{cp:05X} {}", char::from_u32(*cp).unwrap_or('?')))
        .collect();

    assert!(
        extra.is_empty(),
        "{} character(s) are shipped but unused — the fonts were cut for different data.\n  {}",
        extra.len(),
        extra
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// One `name` table record, decoded.
fn name_records(data: &[u8]) -> Vec<(u16, String)> {
    fn be16(b: &[u8], at: usize) -> u16 {
        u16::from_be_bytes([b[at], b[at + 1]])
    }
    let font = Font::parse(data).expect("parsing the font");
    let name = font
        .table(*b"name")
        .expect("the font must keep its name table");
    let count = be16(name, 2) as usize;
    let storage = be16(name, 4) as usize;
    (0..count)
        .map(|i| {
            let rec = 6 + i * 12;
            let platform = be16(name, rec);
            let name_id = be16(name, rec + 6);
            let len = be16(name, rec + 8) as usize;
            let off = be16(name, rec + 10) as usize;
            let raw = &name[storage + off..storage + off + len];
            let text: String = if platform == 1 {
                raw.iter().map(|b| *b as char).collect()
            } else {
                raw.chunks(2)
                    .filter_map(|c| char::from_u32(u32::from(be16(c, 0))))
                    .collect()
            };
            (name_id, text)
        })
        .collect()
}

#[test]
fn every_shipped_font_carries_its_copyright_licence_and_modification_notice() {
    // These files are modified copies of someone else's work, and both licences say what a
    // modified copy must carry. The Arphic Public License, which BabelStone Han is under, is
    // explicit — §2(a): a prominent notice **in each modified file** stating how and when it
    // was changed. A note in the repository does not travel with the file; this does.
    //
    // The check exists because the subsetter rebuilds the file table by table. Dropping `name`
    // would save a few KB and break the licence, and nothing else would notice.
    let dir = root().join("frontend/fonts");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("skipped: no font shipped under frontend/fonts");
        return;
    };
    let mut checked = 0;
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ttf") {
            continue;
        }
        let data = std::fs::read(&path).expect("reading the font");
        let records = name_records(&data);
        let text = |id: u16| {
            records
                .iter()
                .find(|(rec_id, _)| *rec_id == id)
                .map(|(_, t)| t.as_str())
                .unwrap_or("")
        };
        let name = path.file_name().expect("a file name").to_string_lossy();

        assert!(
            text(0).contains("Copyright") || text(0).contains('©'),
            "{name} has no copyright notice (nameID 0)"
        );
        assert!(
            text(13).len() > 200,
            "{name} has no licence text (nameID 13)"
        );
        assert!(
            text(10).contains("Subset of") && text(10).contains("Modified"),
            "{name} has no modification notice (nameID 10): {:?}",
            text(10)
        );
        checked += 1;
    }
    assert!(checked > 0, "no .ttf found under {}", dir.display());
}

#[test]
fn the_licence_text_of_each_shipped_font_sits_beside_it_as_a_file() {
    // The Arphic Public License §1 asks for the licence file itself to be retained in every
    // copy. It rides inside the font too, but a reader of the repository should not have to
    // open a binary to find out what they are allowed to do.
    let dir = root().join("frontend/fonts");
    if !dir.join("BabelStoneHan-gap.ttf").is_file() {
        eprintln!("skipped: no font shipped under frontend/fonts");
        return;
    }
    for (file, marker) in [
        ("ARPHICPL.txt", "ARPHIC PUBLIC LICENSE"),
        ("LICENSE-NomNaTong-MIT.txt", "MIT License"),
    ] {
        let text = std::fs::read_to_string(dir.join(file))
            .unwrap_or_else(|e| panic!("frontend/fonts/{file} must be present: {e}"));
        assert!(
            text.contains(marker),
            "frontend/fonts/{file} is not {marker}"
        );
    }
}
