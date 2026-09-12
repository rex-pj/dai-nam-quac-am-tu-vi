//! Gate ④ (reconciliation against the entry index) and gate ⑤ (the ordering invariant).
//!
//! The `Mục Từ` spreadsheet is **not a data source** — it has been shown to be generated
//! from the PDF text layer itself. Its role is as an **independent cross-check at the glyph
//! level**: if the parser drops or invents a glyph, this comparison sees it.
//!
//! The principle of gate ④: **no percentage thresholds.** Every deviating line must either
//! be fixed or appear in an exception dossier with a human-written reason. "99% match" says nothing about the other 1%.
//!
//! Usage: `reconcile <entries.jsonl> <Mục Từ.xlsx>`

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use calamine::{Data, Reader, Xlsx, open_workbook};
use dnqatv_core::model::Letter;
use dnqatv_core::text::nfc;
use dnqatv_gate_report::dossier::{Correction, Dossiers};
use dnqatv_gate_report::{Fingerprint, Gate, GateReport, GateResult};
use serde::Deserialize;

/// Must match `ENTRY_SCHEMA_VERSION` in `parse`.
const EXPECTED_SCHEMA: u32 = 1;

/// The sheet name inside the entry index workbook.
const SHEET_NAME: &str = "Hán-Nôm → Quốc ngữ";

/// The gate ①②③ results recorded by the parse step.
const PARSE_GATES_FILE: &str = "gates-parse.json";
/// The final report, covering all five gates. This is the file `import` demands.
const FINAL_GATES_FILE: &str = "gates.json";

#[derive(Deserialize)]
struct EntryRecord {
    schema_version: u32,
    seq: usize,
    pdf_page: u16,
    glyph: Option<String>,
    reading: String,
    /// Gate ⑦ picks an entry out of its homographs by a fragment of this.
    #[serde(default)]
    gloss: String,
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(entries), Some(xlsx)) = (args.next(), args.next()) else {
        bail!("usage: reconcile <entries.jsonl> <Mục Từ.xlsx>");
    };

    let entries_path = PathBuf::from(entries);
    let entries = read_entries(&entries_path)?;
    let index = read_index(&PathBuf::from(xlsx))?;

    let dossiers = Dossiers::load(&review_dir()).context("reading the dossiers in review/")?;

    let four = gate_four(&entries, &index, &dossiers);
    let five = gate_five(&entries);
    // Gate ⑦ needs to ask whether a superseded wording is still anywhere in the data, so it
    // reads the file as text rather than through the typed record.
    let entries_text = std::fs::read_to_string(&entries_path)
        .with_context(|| format!("reading {}", entries_path.display()))?;
    let seven = gate_seven(&entries, &entries_text, &dossiers);

    // ── Merge with the parse-step results and write the final report ─────────
    //
    // Gates ①②③ are measured in the parse step (they speak about the text layer and the
    // assembly); gates ④⑤ are measured here (they speak about cross-checking and order).
    // The final report must carry ALL FIVE — `import` treats an absent gate as a failure,
    // so no step can be silently skipped.
    let parse_report = GateReport::read(&entries_path.with_file_name(PARSE_GATES_FILE))
        .context("reading the gate results of the parse step — run `parse` first")?;

    let mut results = parse_report.results.clone();
    results.push(four);
    results.push(five);
    results.push(seven);
    // parse writes ①②③⑧ and this step appends ④⑤⑦, so put them back in the order a
    // reader expects rather than the order they were measured in.
    results.sort_by_key(|r| r.gate.number());

    let report = GateReport::new(
        entries_path.display().to_string(),
        Fingerprint::of_file(&entries_path).context("fingerprinting entries.jsonl")?,
        results,
    );
    let out = entries_path.with_file_name(FINAL_GATES_FILE);
    report
        .write(&out)
        .with_context(|| format!("writing {}", out.display()))?;

    println!();
    println!("== Gate report: {} ==", out.display());
    for r in &report.results {
        println!(
            "  gate {} {:<45} measured {:>3} / allowed {:>3}  {}",
            r.gate.number(),
            r.gate.title(),
            r.measured,
            r.allowed,
            if r.status.is_green() { "PASS" } else { "FAIL" }
        );
    }
    println!();
    println!(
        "  exception dossiers: {} entries, {} not yet signed off by a reviewer",
        dossiers.total(),
        dossiers.unverified()
    );
    if !report.all_green() {
        bail!("some gates did not pass — `import` will refuse to load");
    }

    Ok(())
}

/// The dossier directory, derived from the crate location at compile time.
fn review_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../review")
}

fn read_entries(path: &PathBuf) -> Result<Vec<EntryRecord>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let r: EntryRecord =
            serde_json::from_str(line).with_context(|| format!("parsing line {}", i + 1))?;
        if r.schema_version != EXPECTED_SCHEMA {
            bail!(
                "entry {} has schema_version {} but {EXPECTED_SCHEMA} is required",
                r.seq,
                r.schema_version
            );
        }
        out.push(r);
    }
    Ok(out)
}

/// The entry index: glyph → the reading string verbatim (commas not yet split).
///
/// **Do not split on commas here.** Splitting early is exactly the guess that once produced
/// four false "mismatches" — e.g. `掃 → "Tảo, Táo"` is one entry with two readings, not two entries.
fn read_index(path: &PathBuf) -> Result<BTreeMap<String, String>> {
    let mut book: Xlsx<_> =
        open_workbook(path).with_context(|| format!("opening {}", path.display()))?;
    let range = book
        .worksheet_range(SHEET_NAME)
        .with_context(|| format!("no sheet named {SHEET_NAME:?}"))?;

    let mut out = BTreeMap::new();
    for row in range.rows() {
        let (Some(Data::String(glyph)), Some(Data::String(readings))) = (row.first(), row.get(1))
        else {
            continue;
        };
        let glyph = nfc(glyph.trim());
        // Skip the header row of the table.
        if glyph.is_empty() || glyph == "Hán/Nôm" {
            continue;
        }
        out.insert(glyph, nfc(readings.trim()));
    }
    Ok(out)
}

/// Gate ④ — reconciliation against the entry index.
fn gate_four(
    entries: &[EntryRecord],
    index: &BTreeMap<String, String>,
    dossiers: &Dossiers,
) -> GateResult {
    // Group readings by glyph, from the parsed data.
    let mut parsed: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut image_only = 0usize;
    for e in entries {
        match e.glyph.as_deref() {
            Some(g) => {
                parsed.entry(g).or_default().insert(e.reading.as_str());
            }
            None => image_only += 1,
        }
    }

    let index_glyphs: BTreeSet<&str> = index.keys().map(String::as_str).collect();
    let parsed_glyphs: BTreeSet<&str> = parsed.keys().copied().collect();

    let invented: Vec<&str> = parsed_glyphs.difference(&index_glyphs).copied().collect();
    let missing: Vec<&str> = index_glyphs.difference(&parsed_glyphs).copied().collect();

    println!("== Gate 4: reconciliation against the entry index ==");
    println!("  glyphs in the index       : {}", index_glyphs.len());
    println!("  glyphs found by the parser: {}", parsed_glyphs.len());
    println!("  (image-only entries with no glyph: {image_only})");
    println!();
    println!(
        "  INVENTED glyphs (parser has, index does not): {}",
        invented.len()
    );
    for g in invented.iter().take(12) {
        println!(
            "    {g:?}  U+{:04X}",
            g.chars().next().map_or(0, |c| c as u32)
        );
    }
    println!(
        "  MISSED glyphs (index has, parser does not): {}",
        missing.len()
    );
    for g in missing.iter().take(12) {
        println!(
            "    {g:?}  U+{:04X}",
            g.chars().next().map_or(0, |c| c as u32)
        );
    }

    // Compare reading sets over the intersection.
    let mut exact = 0usize;
    let mut differing: Vec<(&str, String, String)> = Vec::new();
    for g in parsed_glyphs.intersection(&index_glyphs) {
        let Some(from_index) = index.get(*g) else {
            continue;
        };
        // Compare as SETS, not as sorted strings: the spreadsheet orders readings by
        // Vietnamese collation while Rust sorts by byte, so string comparison would produce
        // hundreds of false mismatches.
        let want: BTreeSet<&str> = from_index.split(',').map(str::trim).collect();
        let got: BTreeSet<&str> = parsed[*g].iter().copied().collect();
        if got == want {
            exact += 1;
        } else {
            let mut gv: Vec<&str> = got.into_iter().collect();
            let mut wv: Vec<&str> = want.into_iter().collect();
            gv.sort_unstable();
            wv.sort_unstable();
            differing.push((g, gv.join(" | "), wv.join(" | ")));
        }
    }
    println!();
    println!("  reading sets matching verbatim: {exact}");
    println!("  reading sets differing        : {}", differing.len());
    for (g, got, want) in differing.iter() {
        println!("    {g}  parser={got:?}  index={want:?}");
    }

    // ── Verdict ──────────────────────────────────────────────────────────────
    //
    // What is counted is not "the number of deviations" but **the number of deviations
    // WITHOUT A DOSSIER**. The distinction matters: a book printed in 1895 and an index
    // rebuilt in 2026 will of course differ somewhere; what must hold is that a human has
    // looked at each place and written down why.
    //
    // Consequence: a new deviation nobody has documented turns the gate red and `import` refuses.
    let documented_missing: BTreeSet<&str> = dossiers
        .gate4
        .missing_glyph
        .iter()
        .map(|m| m.glyph.as_str())
        .collect();
    let documented_readings: BTreeSet<&str> = dossiers
        .reading_diffs
        .reading_diff
        .iter()
        .map(|d| d.glyph.as_str())
        .collect();

    let undocumented_missing: Vec<&str> = missing
        .iter()
        .filter(|g| !documented_missing.contains(*g))
        .copied()
        .collect();
    let undocumented_readings: Vec<&str> = differing
        .iter()
        .map(|(g, _, _)| *g)
        .filter(|g| !documented_readings.contains(g))
        .collect();
    // A dossier naming a glyph that no longer deviates is also a deviation — it means the
    // dossier is stale, and a stale dossier is no longer evidence of anything.
    let stale_missing: Vec<&str> = documented_missing
        .iter()
        .filter(|g| !missing.contains(*g))
        .copied()
        .collect();
    let differing_glyphs: BTreeSet<&str> = differing.iter().map(|(g, _, _)| *g).collect();
    let stale_readings: Vec<&str> = documented_readings
        .iter()
        .filter(|g| !differing_glyphs.contains(*g))
        .copied()
        .collect();

    println!();
    println!("  INVENTED glyphs                     : {}", invented.len());
    println!(
        "  MISSED with NO DOSSIER              : {}",
        undocumented_missing.len()
    );
    for g in &undocumented_missing {
        println!("    {g}");
    }
    println!(
        "  READING MISMATCH with NO DOSSIER    : {}",
        undocumented_readings.len()
    );
    for g in &undocumented_readings {
        println!("    {g}");
    }
    println!(
        "  STALE DOSSIERS (recorded, no longer deviating): {}",
        stale_missing.len() + stale_readings.len()
    );
    for g in stale_missing.iter().chain(stale_readings.iter()) {
        println!("    {g}");
    }

    let unexplained = invented.len()
        + undocumented_missing.len()
        + undocumented_readings.len()
        + stale_missing.len()
        + stale_readings.len();

    GateResult::judge(
        Gate::IndexReconciliation,
        unexplained as i64,
        0,
        format!(
            "{} glyphs cross-checked; 0 invented; {} missed and {} reading sets differing, all documented in review/",
            parsed_glyphs.len(),
            missing.len(),
            differing.len()
        ),
    )
}

/// Gate ⑤ — the ordering invariant, under the 22-letter collation OF THE BOOK.
///
/// The book orders by Quốc ngữ reading, so the entry sequence must be **non-decreasing** by
/// letter rank. It uses `Letter::collation_rank` rather than standard Vietnamese collation:
/// the print orders `… H Y K …`, i.e. Y sits where I would be.
///
/// Every inversion signals the parser mixing up the two columns of the layout — or the book
/// being like that. Either way a human must look.
fn gate_five(entries: &[EntryRecord]) -> GateResult {
    let mut inversions: Vec<(usize, u16, &str, &str)> = Vec::new();
    let mut unknown: Vec<(u16, &str)> = Vec::new();
    let mut previous: Option<(Letter, &EntryRecord)> = None;

    for e in entries {
        let Some(initial) = e.reading.chars().next() else {
            continue;
        };
        let Ok(letter) = Letter::from_initial(initial) else {
            unknown.push((e.pdf_page, e.reading.as_str()));
            continue;
        };
        if let Some((prev_letter, prev)) = previous
            && letter.collation_rank() < prev_letter.collation_rank()
        {
            inversions.push((e.seq, e.pdf_page, prev.reading.as_str(), e.reading.as_str()));
        }
        previous = Some((letter, e));
    }

    println!();
    println!("== Gate 5: ordering invariant under the book collation ==");
    println!("  entries considered   : {}", entries.len());
    println!("  unrecognised initials: {}", unknown.len());
    for (p, r) in unknown.iter().take(8) {
        println!("    tr{p}: {r:?}");
    }
    println!("  inversions           : {}", inversions.len());
    for (seq, page, prev, cur) in inversions.iter().take(12) {
        println!("    #{seq} p{page}: {prev:?} then {cur:?}");
    }

    // An unrecognised initial also counts as a failure: it means a reading falls outside the
    // 22 letters of the book, which means either the parser is wrong or our alphabet is.
    GateResult::judge(
        Gate::CollationOrder,
        (inversions.len() + unknown.len()) as i64,
        0,
        format!(
            "{} entries, no inversion under the 22-letter collation of the print",
            entries.len()
        ),
    )
}

/// Gate ⑦ — the corrections the print prints about itself.
///
/// `SAI SÓT` (volume 1, scan 14) and `ĐÍNH NGOA 訂訛` (volume 2, scans 2-3) are the author's
/// own errata. They matter more than any other cross-check here, for a reason that only
/// became clear once the sources were measured: the 2026 edition and the Wikisource
/// transcription are **not independent** — they agree on 4,662 of 4,673 distinct glyphs and
/// on 716 of 721 rare characters that occur exactly once, including 31 identical Private-Use
/// code points. Two people reading a blurred 1895 print do not agree like that. So where
/// both carry the same error, gate ⑥ is blind to it, and this is the only witness left.
///
/// Three kinds of row, and each is tested only as far as it can be:
///
/// * `glyph` — the entry is found by reading plus a fragment of our own gloss, and its glyph
///   is compared. Not finding exactly one entry counts as a failure: an untestable claim
///   about a character is not the same as a satisfied one.
/// * `text` — the superseded wording is searched for in the whole file. Finding it proves
///   the correction was not applied; NOT finding it proves nothing, so it stays silent. This
///   asymmetry is on purpose — the test can accuse, never acquit.
/// * `note` — "move this entry", "add a comma". No machine test exists; counted, never measured.
///
/// The threshold is the number of rows carrying a written `reason`, so admitting a deviation
/// costs somebody a sentence explaining it.
fn gate_seven(entries: &[EntryRecord], entries_text: &str, dossiers: &Dossiers) -> GateResult {
    let rows = &dossiers.corrections.correction;
    let mut unhonoured: Vec<(&str, u16, String)> = Vec::new();
    let mut honoured = 0usize;
    let mut silent = 0usize;
    let mut untestable = 0usize;

    for c in rows {
        match c.kind.as_str() {
            Correction::GLYPH => {
                let found: Vec<&EntryRecord> = entries
                    .iter()
                    .filter(|e| e.reading == c.reading && e.gloss.contains(c.anchor.as_str()))
                    .collect();
                match found.as_slice() {
                    [one] if one.glyph.as_deref() == Some(c.corrected.as_str()) => honoured += 1,
                    [one] => unhonoured.push((
                        c.source.as_str(),
                        c.printed_page_1895,
                        format!(
                            "{} reads {:?}, the print says {:?}",
                            c.reading,
                            // Printed with Debug: 29 entries carry no code point because
                            // the print sets an image there, and `None` says that plainly.
                            one.glyph.as_deref(),
                            c.corrected
                        ),
                    )),
                    other => unhonoured.push((
                        c.source.as_str(),
                        c.printed_page_1895,
                        format!(
                            "{} + {:?} matches {} entries, so the claim cannot be tested",
                            c.reading,
                            c.anchor,
                            other.len()
                        ),
                    )),
                }
            }
            Correction::TEXT => {
                if !c.printed.is_empty() && entries_text.contains(c.printed.as_str()) {
                    unhonoured.push((
                        c.source.as_str(),
                        c.printed_page_1895,
                        format!(
                            "the superseded wording {:?} is still in the data",
                            c.printed
                        ),
                    ));
                } else {
                    silent += 1;
                }
            }
            _ => untestable += 1,
        }
    }

    // A reason attached to a correction the data DOES honour is stale, and a stale dossier
    // is no longer evidence of anything — same rule as gate ④.
    let unhonoured_keys: std::collections::BTreeSet<(&str, u16)> =
        unhonoured.iter().map(|(s, p, _)| (*s, *p)).collect();
    let mut unexplained = 0usize;
    let mut stale: Vec<(&str, u16)> = Vec::new();
    for c in rows {
        let fails = unhonoured_keys.contains(&(c.source.as_str(), c.printed_page_1895));
        match (fails, c.has_reason()) {
            (true, false) => unexplained += 1,
            (false, true) => stale.push((c.source.as_str(), c.printed_page_1895)),
            _ => {}
        }
    }

    println!();
    println!("== Gate 7: the errata the print carries about itself ==");
    println!("  corrections transcribed: {}", rows.len());
    println!("  honoured by the data   : {honoured}");
    println!("  superseded wording gone (no claim either way): {silent}");
    println!("  no machine test possible: {untestable}");
    println!("  NOT honoured           : {}", unhonoured.len());
    for (source, page, what) in unhonoured.iter().take(40) {
        println!("    {source} tr.{page}: {what}");
    }
    println!("  NOT honoured and not explained: {unexplained}");
    println!(
        "  reasons written for a correction the data already honours: {}",
        stale.len()
    );
    for (source, page) in stale.iter().take(8) {
        println!("    {source} tr.{page}");
    }

    GateResult::judge(
        Gate::PrintedErrata,
        (unexplained + stale.len()) as i64,
        0,
        format!(
            "{} corrections read off the scan of SAI SÓT and ĐÍNH NGOA; {honoured} honoured, \
             {} not, every one of those explained in review/errata-ban-in.toml",
            rows.len(),
            unhonoured.len()
        ),
    )
}
