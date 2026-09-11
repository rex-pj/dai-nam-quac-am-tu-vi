//! The I/O shell for extraction: read the PDF, call the core, write JSONL, print a report.
//!
//! All the real logic lives in `dnqatv-extract-core` — this file only reads files, writes
//! files and counts. That is what keeps the core testable without a fake directory tree.
//!
//! Usage: `extract <input.pdf> <output.jsonl>`

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use dnqatv_extract_core::PdfBook;
use dnqatv_extract_core::layout::Column;
use dnqatv_extract_core::style::TextStyle;
use serde::Serialize;

/// The version of the intermediate record format. Changing the structure means bumping this,
/// so the `parse` step rejects stale data instead of misreading it.
const SCHEMA_VERSION: u32 = 2;

#[derive(Serialize)]
struct PageRecord {
    schema_version: u32,
    pdf_page: u32,
    width: f64,
    lines: Vec<LineRecord>,
    unmapped: Vec<UnmappedRecord>,
}

#[derive(Serialize)]
struct LineRecord {
    column: &'static str,
    y: f64,
    /// The style-separated pieces. Kept apart because the form/definition boundary of a
    /// sub-entry is exactly where the style changes — joining them throws structure away.
    segments: Vec<SegmentRecord>,
}

#[derive(Serialize)]
struct SegmentRecord {
    style: &'static str,
    text: String,
}

#[derive(Serialize)]
struct UnmappedRecord {
    font: String,
    code: u16,
}

const fn style_name(s: TextStyle) -> &'static str {
    s.db_value()
}

const fn column_name(c: Column) -> &'static str {
    match c {
        Column::Left => "left",
        Column::Right => "right",
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(pdf), Some(out)) = (args.next(), args.next()) else {
        bail!("usage: extract <input.pdf> <output.jsonl>");
    };
    let pdf = PathBuf::from(pdf);
    let out = PathBuf::from(out);

    let bytes = std::fs::read(&pdf).with_context(|| format!("reading {}", pdf.display()))?;
    eprintln!("read {} ({} bytes)", pdf.display(), bytes.len());

    let book = PdfBook::open(&bytes).context("loading the PDF structure")?;
    let pages = book.page_numbers();
    eprintln!("pages: {}", pages.len());

    let mut writer =
        BufWriter::new(File::create(&out).with_context(|| format!("creating {}", out.display()))?);

    let mut total_lines = 0usize;
    let mut total_chars = 0usize;
    let mut unmapped_total = 0usize;
    let mut unmapped_by_code: BTreeMap<u16, usize> = BTreeMap::new();
    let mut pages_with_unmapped = 0usize;
    let mut empty_pages: Vec<u32> = Vec::new();

    for page in pages {
        let width = book
            .page_width(page)
            .with_context(|| format!("trang {page}"))?;
        let text = book
            .page_text(page)
            .with_context(|| format!("trang {page}"))?;

        total_lines += text.lines.len();
        total_chars += text
            .lines
            .iter()
            .map(|l| l.text().chars().filter(|c| !c.is_whitespace()).count())
            .sum::<usize>();

        if text.lines.is_empty() {
            empty_pages.push(page);
        }
        if !text.unmapped.is_empty() {
            pages_with_unmapped += 1;
            unmapped_total += text.unmapped.len();
            for u in &text.unmapped {
                *unmapped_by_code.entry(u.code).or_default() += 1;
            }
        }

        let record = PageRecord {
            schema_version: SCHEMA_VERSION,
            pdf_page: page,
            width,
            lines: text
                .lines
                .iter()
                .map(|l| LineRecord {
                    column: column_name(l.column),
                    y: l.y,
                    segments: l
                        .segments
                        .iter()
                        .map(|seg| SegmentRecord {
                            style: style_name(seg.style),
                            text: seg.text.clone(),
                        })
                        .collect(),
                })
                .collect(),
            unmapped: text
                .unmapped
                .iter()
                .map(|u| UnmappedRecord {
                    font: u.font.clone(),
                    code: u.code,
                })
                .collect(),
        };
        serde_json::to_writer(&mut writer, &record).context("ghi JSONL")?;
        writer.write_all(b"\n")?;
    }

    writer.flush()?;

    eprintln!();
    eprintln!("== Extraction complete ==");
    eprintln!("  printed lines rebuilt : {total_lines}");
    eprintln!("  characters (no spaces): {total_chars}");
    eprintln!("  pages with no text    : {empty_pages:?}");
    eprintln!();
    eprintln!("== Gate 1: unmapped glyph codes ==");
    eprintln!("  total occurrences     : {unmapped_total}");
    eprintln!("  distinct codes        : {}", unmapped_by_code.len());
    eprintln!("  pages affected        : {pages_with_unmapped}");
    for (code, count) in &unmapped_by_code {
        eprintln!("    code {code:#06X} x {count}");
    }
    eprintln!();
    eprintln!("wrote {}", out.display());

    Ok(())
}
