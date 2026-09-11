//! Extract the fonts the print was set in from the source PDF and subset them to the
//! characters the book actually uses.
//!
//! **Why this is necessary.** 27 glyphs of the dictionary carry Private Use Area code points
//! (PUA, U+F0000+). PUA code points have no standard meaning — they only point at the right
//! glyph *inside* the font of the print. Without that font a machine shows a square; worse,
//! a machine with another font defining those code points shows **the wrong character** with
//! no warning. For a dictionary that is data corruption at the visual layer.
//!
//! **Why take it from the PDF rather than downloading it.** The fonts are already inside the
//! source PDF, unsubsetted. They are **the very fonts the print was set in**, so there is no
//! question of "is this copy the same as that one". No download to trust.
//!
//! **Two fonts, not one.** Nôm Na Tống carries the book, but it does not have every character
//! the book uses: measured on the real data, 26 code points are missing from it, and the 2026
//! print sets those in BabelStone Han. Both fonts sit in the same PDF, so both are extracted
//! and subsetted here. Without the second file those 26 characters fall through to whatever
//! CJK font the reader happens to have — different on every machine, absent on many.
//!
//! Usage: `dnqatv-font <PDF> <entries.jsonl> <output dir>`

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dnqatv_font_core::{Font, subset};
use serde::Deserialize;

/// The Nôm font inside the PDF — the one the book is set in.
const FONT_NAME: &str = "NomNaTong-Regular";

/// The font the print uses for the characters Nôm Na Tống lacks.
const GAP_FONT_NAME: &str = "BabelStone Han Regular";

/// The file the gap subset is written to. Must match `FontRange::Gap::stem` in
/// `adapter-web::assets`.
const GAP_FILE: &str = "BabelStoneHan-gap.ttf";

/// Three Unicode ranges, split into three files so a page loads only what it needs.
///
/// Must match `FontRange` in `adapter-web::assets` — the CSS `unicode-range` there decides
/// which file a browser fetches.
struct Range {
    file: &'static str,
    label: &'static str,
    contains: fn(u32) -> bool,
}

const RANGES: [Range; 3] = [
    Range {
        file: "NomNaTong-bmp.ttf",
        label: "BMP",
        contains: |c| {
            (0x3400..=0x4DBF).contains(&c)
                || (0x4E00..=0x9FFF).contains(&c)
                || (0xF900..=0xFAFF).contains(&c)
        },
    },
    Range {
        file: "NomNaTong-extb.ttf",
        label: "Ext-B",
        contains: |c| (0x20000..=0x3FFFF).contains(&c),
    },
    Range {
        file: "NomNaTong-pua.ttf",
        label: "PUA",
        contains: |c| (0xE000..=0xF8FF).contains(&c) || (0xF0000..=0xFFFFD).contains(&c),
    },
];

#[derive(Deserialize)]
struct EntryRecord {
    glyph: Option<String>,
    sub_entries: Vec<SubRecord>,
}

#[derive(Deserialize)]
struct SubRecord {
    han_form: Option<String>,
    han_expanded: Option<String>,
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(pdf), Some(entries), Some(out)) = (args.next(), args.next(), args.next()) else {
        bail!("usage: dnqatv-font <PDF> <entries.jsonl> <output dir>");
    };
    let out = PathBuf::from(out);

    // The PDF is read once: it is 43 MB, and both fonts come out of the same document.
    let pdf_path = PathBuf::from(&pdf);
    let doc = lopdf::Document::load(&pdf_path)
        .with_context(|| format!("opening {}", pdf_path.display()))?;

    let raw = extract_font(&doc, FONT_NAME)?;
    println!("== Font extracted from the PDF ==");
    println!("  name           : {FONT_NAME}");
    println!("  original size  : {} bytes", raw.len());

    let font = Font::parse(&raw).context("parsing the font")?;
    println!("  glyph count    : {}", font.num_glyphs()?);

    let needed = codepoints_used(Path::new(&entries))?;
    println!("  chars in book  : {}", needed.len());
    println!();

    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;

    println!("== Subsetting by range ==");
    let mut gap: BTreeSet<u32> = BTreeSet::new();
    for range in &RANGES {
        let set: BTreeSet<u32> = needed
            .iter()
            .copied()
            .filter(|c| (range.contains)(*c))
            .collect();
        if set.is_empty() {
            println!("  {:<6} no characters — skipped", range.label);
            continue;
        }

        let notice = modification_notice(FONT_NAME, set.len(), &today());
        let (bytes, report) = subset::subset_with_notice(&font, &set, Some(&notice))
            .context("subsetting the font")?;

        // Fail closed where nothing CAN rescue it: a PUA code point only means anything
        // inside this font, so Nôm Na Tống missing one leaves no other route.
        //
        // BMP and Ext-B differ: the print uses several CJK fonts, and a character Nôm Na Tống
        // lacks another font supplies. Collect those rather than stopping — they are handled
        // by the gap pass below, which is the whole reason this list is gathered.
        if !report.codepoints_missing.is_empty() {
            if range.label == "PUA" {
                bail!(
                    "the PUA range is missing {} code points — no other font can rescue them: {:X?}",
                    report.codepoints_missing.len(),
                    &report.codepoints_missing[..report.codepoints_missing.len().min(8)]
                );
            }
            gap.extend(report.codepoints_missing.iter().copied());
            println!(
                "  {:<6} {} characters Nôm Na Tống does NOT have — deferred to the gap pass",
                range.label,
                report.codepoints_missing.len(),
            );
        }

        // Re-read the subset font and compare each outline with the original. Renumbering
        // GIDs is the easiest thing to get wrong, and getting it wrong does NOT break the
        // font — it just draws different characters.
        let path = out.join(range.file);
        std::fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;

        let checked = subset::verify(&raw, &bytes, &set).context("verifying the subset font")?;
        println!(
            "  {:<6} {:>5} chars -> {:>4} KB   ({} glyphs, +{} components, {} outlines compared)",
            range.label,
            set.len(),
            report.bytes / 1024,
            report.glyphs_kept,
            report.glyphs_from_components,
            checked,
        );
    }

    println!();
    subset_gap(&doc, &out, &gap)?;

    println!();
    println!("wrote to {}", out.display());
    println!("The server picks the fonts up on its next start — no code change needed.");
    Ok(())
}

/// Subset the gap font to exactly the characters Nôm Na Tống lacks.
///
/// An empty gap is the good case and writes nothing. A non-empty gap that BabelStone Han
/// cannot close is a **hard error**: those characters would reach readers as squares or, worse,
/// as whatever a local font maps the code point to, and neither is acceptable for a dictionary.
fn subset_gap(doc: &lopdf::Document, out: &Path, gap: &BTreeSet<u32>) -> Result<()> {
    println!("== Gap pass: characters Nôm Na Tống does not have ==");
    let path = out.join(GAP_FILE);
    if gap.is_empty() {
        // Remove a stale file rather than leaving one that claims characters nothing needs.
        if path.is_file() {
            std::fs::remove_file(&path)
                .with_context(|| format!("removing the stale {}", path.display()))?;
            println!("  gap is empty — removed the stale {GAP_FILE}");
        } else {
            println!("  gap is empty — nothing to ship");
        }
        return Ok(());
    }

    let raw = extract_font(doc, GAP_FONT_NAME)?;
    let font = Font::parse(&raw).context("parsing the gap font")?;
    println!("  source         : {GAP_FONT_NAME} ({} bytes)", raw.len());

    let notice = modification_notice(GAP_FONT_NAME, gap.len(), &today());
    let (bytes, report) =
        subset::subset_with_notice(&font, gap, Some(&notice)).context("subsetting the gap font")?;
    if !report.codepoints_missing.is_empty() {
        bail!(
            "{} characters are in neither Nôm Na Tống nor {GAP_FONT_NAME}: {:X?}",
            report.codepoints_missing.len(),
            &report.codepoints_missing[..report.codepoints_missing.len().min(16)]
        );
    }

    std::fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
    let checked = subset::verify(&raw, &bytes, gap).context("verifying the gap font")?;
    println!(
        "  gap      {:>5} chars -> {:>4} KB   ({} glyphs, +{} components, {} outlines compared)",
        gap.len(),
        report.bytes / 1024,
        report.glyphs_kept,
        report.glyphs_from_components,
        checked,
    );
    let listed: Vec<String> = gap
        .iter()
        .map(|cp| match char::from_u32(*cp) {
            Some(ch) => format!("U+{cp:X} {ch}"),
            // A code point that is not a Unicode scalar cannot reach here: it came from a
            // `char` in UTF-8 data. Saying so beats substituting a character — a stand-in
            // glyph in a report about missing glyphs is exactly the wrong thing to invent.
            None => format!("U+{cp:X} (not a scalar value)"),
        })
        .collect();
    println!("  {}", listed.join(" · "));
    Ok(())
}

/// Pull the `FontFile2` font program of one named font out of the PDF.
fn extract_font(doc: &lopdf::Document, want: &str) -> Result<Vec<u8>> {
    for object in doc.objects.values() {
        let Ok(dict) = object.as_dict() else { continue };
        // The `FontDescriptor` is what points at the font program; the `Font` object itself
        // does not. Find the one carrying the name we want.
        if dict.get(b"Type").and_then(|t| t.as_name()).ok() != Some(b"FontDescriptor") {
            continue;
        }
        // PDFs write the font name several ways: `NomNaTong-Regular`, `Nom#20Na#20Tong#20Regular`
        // (lopdf decodes `#20` to a space). Comparing after stripping every non-alphanumeric
        // character collapses all three spellings to the same string.
        let Ok(raw_name) = dict.get(b"FontName").and_then(|n| n.as_name()) else {
            // A descriptor with no name cannot be the font we want; move on rather than
            // treating it as an empty name and comparing — that would be a silent fallback.
            continue;
        };
        if squash(&String::from_utf8_lossy(raw_name)) != squash(want) {
            continue;
        }
        let file_ref = dict
            .get(b"FontFile2")
            .with_context(|| format!("the descriptor for {want} has no FontFile2 — the PDF only references the font, it does not embed it"))?;
        let stream = match file_ref {
            lopdf::Object::Reference(id) => doc.get_object(*id)?.as_stream()?,
            other => other.as_stream()?,
        };
        return stream
            .decompressed_content()
            .context("decompressing the font program");
    }
    bail!("font {want} not found in the PDF")
}

/// Strip every non-alphanumeric character and lowercase, so font names compare independently of spelling.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Every Han-Nom code point the book actually uses — taken from the parsed data itself.
///
/// No guessing by range: subsetting "all of Ext-B" would carry tens of thousands of unused
/// characters. The single source is `entries.jsonl`.
fn codepoints_used(path: &Path) -> Result<BTreeSet<u32>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = BTreeSet::new();
    for (i, line) in text.lines().enumerate() {
        let record: EntryRecord =
            serde_json::from_str(line).with_context(|| format!("parsing line {}", i + 1))?;
        let mut take = |s: &Option<String>| {
            if let Some(s) = s {
                out.extend(s.chars().map(|c| c as u32).filter(|c| *c > 0x2E7F));
            }
        };
        take(&record.glyph);
        for sub in &record.sub_entries {
            take(&sub.han_form);
            take(&sub.han_expanded);
        }
    }
    Ok(out)
}

/// Today's date as `YYYY-MM-DD`, in UTC.
///
/// Written out rather than pulled from a date crate: the only thing needed is the civil date,
/// the algorithm is a well-known closed form, and a subsetter that writes its own `cmap` can
/// manage a calendar. `days_to_civil` is Howard Hinnant's `civil_from_days`, valid for any
/// date in the proleptic Gregorian calendar.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = days_to_civil((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 → (year, month, day).
fn days_to_civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe as i64 + era * 400 + i64::from(m <= 2), m, d)
}

/// The modification notice written into each subset font's `name` table.
///
/// It has to say **how and when** the file was changed — that is the wording of the Arphic
/// Public License §2(a), which BabelStone Han is under, and it is plain good manners for the
/// MIT-licensed Nôm Na Tống too. Everything in it is a fact about what this program did.
fn modification_notice(source: &str, chars: usize, date: &str) -> String {
    format!(
        "Subset of {source}. Modified {date} for the Đại Nam Quấc Âm Tự Vị digital edition: \
         the glyph table was reduced to the {chars} character(s) that edition uses, glyph ids \
         were renumbered accordingly, and the GSUB and post tables were dropped. No glyph \
         outline was altered — every retained outline is compared byte for byte against the \
         source font before the file is written. The source font was extracted from the font \
         program embedded in the 2026 edition PDF; it was not downloaded. The program that \
         performs this subsetting is `pipeline/font` in the dnqatv repository."
    )
}
