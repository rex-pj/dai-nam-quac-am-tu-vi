//! The I/O shell for gate ⑥: read both editions, call the core, write dossiers, report.
//!
//! No network. `tools/fetch-wikisource.mjs` takes the snapshot once and records the revision
//! id of every page in `data/wikisource/PROVENANCE.json`; this step only ever reads that
//! file, so a run months from now compares against exactly the same text.
//!
//! Usage: `witness <entries.jsonl> <wikisource/pages.jsonl> [review-dir]`

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dnqatv_gate_report::dossier::{DossierRow, Dossiers, write_dossier};
use dnqatv_witness_core::align::{DivergenceKind, OurEntry, OurSub};
use dnqatv_witness_core::{compare, read_page};
use serde::Deserialize;

/// Must match `ENTRY_SCHEMA_VERSION` in `parse`.
const EXPECTED_SCHEMA: u32 = 1;

#[derive(Deserialize)]
struct EntryRecord {
    schema_version: u32,
    seq: u32,
    pdf_page: u16,
    printed_page: u16,
    glyph: Option<String>,
    reading: String,
    gloss: String,
    sub_entries: Vec<SubRecord>,
}

#[derive(Deserialize)]
struct SubRecord {
    needs_review: bool,
    han_form: Option<String>,
    form: String,
    definition: String,
}

#[derive(Deserialize)]
struct WitnessPage {
    title: String,
    quality: Option<u8>,
    wikitext: String,
}

fn read_entries(path: &Path) -> Result<Vec<OurEntry>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut out = Vec::with_capacity(8000);
    for (n, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let r: EntryRecord =
            serde_json::from_str(&line).with_context(|| format!("{}:{}", path.display(), n + 1))?;
        if r.schema_version != EXPECTED_SCHEMA {
            bail!(
                "{} line {} has schema {} but this build expects {EXPECTED_SCHEMA} — re-run `parse`",
                path.display(),
                n + 1,
                r.schema_version
            );
        }
        out.push(OurEntry {
            seq: r.seq,
            pdf_page: r.pdf_page,
            printed_page: r.printed_page,
            // One character by construction; anything else is a parser fault, not ours to paper over.
            glyph: r.glyph.as_deref().and_then(|g| {
                let mut it = g.chars();
                it.next().filter(|_| it.next().is_none())
            }),
            reading: r.reading,
            gloss: r.gloss,
            // Filled in below from review/: the shell owns the dossiers, the core stays pure.
            needs_review: false,
            subs: r
                .sub_entries
                .into_iter()
                .map(|s| OurSub {
                    needs_review: s.needs_review,
                    han_form: s.han_form,
                    form: s.form,
                    definition: s.definition,
                })
                .collect(),
        });
    }
    Ok(out)
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(entries_path), Some(witness_path)) = (args.next(), args.next()) else {
        bail!("usage: witness <entries.jsonl> <wikisource/pages.jsonl> [review-dir]");
    };
    let entries_path = PathBuf::from(entries_path);
    let witness_path = PathBuf::from(witness_path);
    let review_dir = args
        .next()
        .map_or_else(|| PathBuf::from("review"), PathBuf::from);

    let mut ours = read_entries(&entries_path)?;

    // Which headword lines a dossier already calls unsettled. The rule is the one `import`
    // uses, page window included: an entry beginning on page N−1 runs on to page N, so a line
    // printed on N can belong to it.
    let dossiers = Dossiers::load(&review_dir).context("reading the dossiers in review/")?;
    let mut flagged = 0usize;
    for e in &mut ours {
        let by_line = dossiers.ambiguous_lines.ambiguous_line.iter().any(|a| {
            a.pdf_page.abs_diff(e.pdf_page) <= 1
                && !a.text.trim().is_empty()
                && e.gloss.contains(a.text.trim())
        });
        let by_label = dossiers
            .label_typos
            .label_typo
            .iter()
            .any(|t| t.pdf_page == e.pdf_page && t.line.contains(&e.reading));
        e.needs_review = by_line || by_label;
        if e.needs_review {
            flagged += 1;
        }
    }
    eprintln!("  headword lines a dossier already flags: {flagged}");

    let wf = File::open(&witness_path).with_context(|| {
        format!(
            "opening {} — run tools/fetch-wikisource.mjs first",
            witness_path.display()
        )
    })?;
    let mut theirs = Vec::with_capacity(8000);
    let mut pages = 0usize;
    for line in BufReader::new(wf).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let p: WitnessPage = serde_json::from_str(&line)?;
        pages += 1;
        theirs.extend(read_page(&p.wikitext, &p.title, p.quality));
    }

    eprintln!("== Gate 6: an independent transcription of the 1895 print ==");
    eprintln!("  witness pages read : {pages}");
    eprintln!("  headwords  ours    : {}", ours.len());
    eprintln!("  headwords  theirs  : {}", theirs.len());

    let a = compare(&ours, &theirs);

    eprintln!("\n  aligned by reading : {}", a.matched);
    eprintln!("  sub-entries compared: {}", a.subs_compared);
    eprintln!("  sub-entries with no partner line: {}", a.subs_no_partner);

    eprintln!("\n  findings by kind:");
    let kinds = [
        (
            DivergenceKind::ColumnBoundary,
            "the other edition breaks the columns elsewhere",
        ),
        (
            DivergenceKind::GlyphDiffers,
            "both encode a glyph, and they differ",
        ),
        (
            DivergenceKind::GlyphUnencodableThere,
            "we encode one, the other edition could not",
        ),
        (
            DivergenceKind::ShapeNoteAvailable,
            "we have an image, they describe the shape",
        ),
        (
            DivergenceKind::EntryOnlyThere,
            "headword only in the 1895 print",
        ),
        (
            DivergenceKind::EntryOnlyHere,
            "headword only in the 2026 edition",
        ),
        (
            DivergenceKind::FlaggedHere,
            "already flagged here; the 1895 reading is recorded beside ours",
        ),
    ];
    for (k, what) in kinds {
        let mark = if k.is_contradiction() { "!" } else { " " };
        eprintln!("  {mark} {:<26} {:>6}   {what}", k.slug(), a.count(k));
    }
    eprintln!(
        "\n  contradictions (differences that claim we misread the page): {}",
        a.contradictions()
    );

    // Every headword on each side must be either paired or reported. A comparison that
    // quietly drops entries would read as clean exactly when it had looked at least.
    let ours_seen = a.matched + a.count(DivergenceKind::EntryOnlyHere);
    let theirs_seen = a.matched + a.count(DivergenceKind::EntryOnlyThere);
    if ours_seen != a.entries_ours || theirs_seen != a.entries_theirs {
        bail!(
            "the comparison lost entries: {ours_seen} of {} accounted for on our side, \
             {theirs_seen} of {} on theirs — this is a fault in `witness`, not in the data",
            a.entries_ours,
            a.entries_theirs
        );
    }
    eprintln!("  accounting: every headword on both sides is paired or reported");

    // ── Dossiers, one file per kind ──────────────────────────────────────────
    std::fs::create_dir_all(&review_dir)?;
    let mut by_kind: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    for d in &a.divergences {
        by_kind.entry(d.kind.slug()).or_default().push(d);
    }
    for (slug, items) in &by_kind {
        let path = review_dir.join(format!("witness-{}.toml", slug.replace('_', "-")));
        let header = format!(
            "\
# Chốt ⑥ — {slug}: {} chỗ.
#
# SINH TỰ ĐỘNG bởi `witness` — đừng sửa tay phần thân. Chữ ký ở `verified_by` được giữ
# nguyên qua mỗi lần sinh lại, khớp theo `key`.
#
# Nhân chứng: bản chép Wikisource của bản in 1895-96, chụp lại trong data/wikisource/
# (số hiệu bản sửa từng trang ghi ở PROVENANCE.json). Bản chép ấy mang giấy phép CC BY-SA —
# dùng để ĐỐI CHIẾU, không bưng vào dữ liệu.
#
# Hai bản in khác nhau thì lệch nhau là thường. Chỉ `column_boundary` và `glyph_differs`
# mới là lời tố rằng mình đọc sai trang; còn lại là tư liệu.
#
# quality: 1 chưa hiệu đính · 3 đã hiệu đính · 4 đã kiểm chứng
",
            items.len()
        );
        // The key must identify the FINDING, not its position: the list is re-sorted and
        // re-numbered on every run, and a signature that followed a row number would end up
        // vouching for whatever landed in that slot next.
        let rows: Vec<DossierRow> = items
            .iter()
            .map(|d| DossierRow {
                key: format!("{}|{}|{}|{}", d.printed_page, d.reading, d.ours, d.theirs),
                fields: vec![
                    ("printed_page", d.printed_page.into()),
                    ("reading", d.reading.clone().into()),
                    ("ours", d.ours.clone().into()),
                    ("theirs", d.theirs.clone().into()),
                    ("witness_page", d.witness_page.clone().into()),
                    ("quality", d.witness_quality.map_or(0u16, u16::from).into()),
                ],
            })
            .collect();
        let kept = write_dossier(&path, slug, &header, &rows)?;
        eprintln!(
            "  wrote {} ({} entries, {kept} signature(s) carried over)",
            path.display(),
            items.len()
        );
    }
    Ok(())
}
