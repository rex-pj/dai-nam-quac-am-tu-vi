//! The I/O shell for parsing: read the `extract` JSONL, classify lines, assemble entries, report gates.
//!
//! All logic lives in `dnqatv-parse-core`; this file only reads files, gathers a stream and counts.
//!
//! The crucial point: all 1038 pages are gathered into **one continuous line stream** before
//! assembly. Assembling page by page leaves 25,087 orphan lines, because entries spill over page boundaries.
//!
//! Usage: `parse <pages.jsonl> [entries.jsonl]`

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use dnqatv_core::model::{GlyphChar, GlyphKind, PdfPage, TextStyle};
use dnqatv_gate_report::dossier::{
    DossierRow, GlossInitialsLost, GlossInitialsRestored, LabelTypos, ResidualPlaceholders,
    ScanReading, ScanReadings, caught_by, write_dossier,
};
use dnqatv_gate_report::{Fingerprint, Gate, GateReport, GateResult};
use dnqatv_parse_core::assemble::{assemble, check_partition};
use dnqatv_parse_core::expand::{HeadwordContext, expand_han_form, expand_reading_form};
use dnqatv_parse_core::line::{LineRole, SubEntrySignal, classify, has_placeholder};
use dnqatv_parse_core::span::{Span, check_coverage, visible_len as span_visible};
use dnqatv_parse_core::subentry::{
    StyledSegment, SubEntryParts, merge_wrapped_forms, parse_sub_entry,
};
use dnqatv_parse_core::{parse_continued_headword, parse_headword, visible_len};
use serde::{Deserialize, Serialize};

/// Must match `SCHEMA_VERSION` in `extract`; a mismatch is refused, never read anyway.
const EXPECTED_SCHEMA: u32 = 2;

#[derive(Deserialize)]
struct PageRecord {
    schema_version: u32,
    pdf_page: u16,
    lines: Vec<LineRecord>,
    /// Glyph codes not found in the CMap — the raw material of gate 1.
    #[serde(default)]
    unmapped: Vec<UnmappedRecord>,
}

#[derive(Deserialize)]
struct UnmappedRecord {
    code: u32,
}

#[derive(Deserialize)]
struct LineRecord {
    y: f64,
    segments: Vec<SegmentRecord>,
}

#[derive(Deserialize)]
struct SegmentRecord {
    style: String,
    text: String,
}

impl LineRecord {
    fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }

    /// Whether the line has an italic piece carrying content.
    ///
    /// The print uses italics for sub-entry forms — one of the two sub-entry signals.
    fn has_italic(&self) -> bool {
        self.segments
            .iter()
            .any(|s| s.style == "italic" && !s.text.trim().is_empty())
    }
}

/// The entry format version. The next step refuses to read on a mismatch.
const ENTRY_SCHEMA_VERSION: u32 = 1;

/// One definition whose opening capital the 2026 text layer mislaid.
///
/// Used for both dossiers: `letter` carries the restored capital, or `'\0'` when the letter
/// is gone from the text layer altogether and only the scan can supply it.
struct GlossInitialFix {
    page: u16,
    reading: String,
    letter: char,
    line: String,
    gloss: String,
}

/// The prose at the top of `review/residual-placeholders.toml`. Vietnamese, like every
/// dossier: these files are read by the people doing the checking, not by the program.
const RESIDUAL_DOSSIER_HEADER: &str = "\
# Hình thái mục con còn chứa dấu thế chỗ `|` sau khi suy diễn.
#
# SINH TỰ ĐỘNG bởi `parse` — đừng sửa tay phần thân. Chữ ký ở `verified_by` thì được giữ
# nguyên qua mỗi lần sinh lại, khớp theo `key`.
#
# Hồ sơ nầy từng có 829 dòng. Nguyên do không phải bản in mờ ám, mà là bộ tách đọc sai ranh
# giới cột: thợ sắp chữ đặt dấu `|` và các khoảng trắng giữa chữ Hán bằng phông NGHIÊNG, nên
# quy tắc \"lấy từ mẩu nghiêng đầu tới mẩu nghiêng cuối\" khởi sự ngay giữa cột Hán rồi kéo
# luôn phần Hán qua cột Quốc ngữ.
#
# `quoc_ngu_starts_at` trong pipeline/parse-core/src/subentry.rs trả phần ấy về chỗ của nó.
# Ranh giới mới đã đối chiếu với bản chép Wikisource của bản in 1895-96: 679 dòng khớp,
# KHÔNG dòng nào nghịch.
#
# Mấy dòng còn lại đều có đáp án sẵn trong review/witness-column-boundary.toml — bản 1895
# ngắt cột ở đâu thì chép ở đó. Vẫn để người xem quyết, vì hai bản in không phải một.
";

/// The prose at the top of `review/gloss-initial-restored.toml`.
const GLOSS_RESTORED_DOSSIER_HEADER: &str = "\
# Chữ hoa mở đầu lời chú giải, lấy lại được từ một nhãn `dấu riêng` giả.
#
# SINH TỰ ĐỘNG bởi `parse` — đừng sửa tay phần thân. Chữ ký ở `verified_by` thì được giữ
# nguyên qua mỗi lần sinh lại, khớp theo `key`.
#
# Bản in 1895 đặt `壓  Áp. c. Ngăn, giữ, đè, nhận xuống.` — MỘT nhãn. Lớp chữ bản 2026 lại
# ghi `壓  Áp  c. n.` rồi xuống hàng `găn, giữ, đè, nhận xuống.`: chữ `N` của \"Ngăn\" bị
# xếp thành nhãn `n.`, lời chú giải mất chữ đầu.
#
# Phép thử hẹp, không có chỗ nào đoán: dòng phải mang HƠN MỘT nhãn (bỏ cái chót đi thì mục
# vẫn còn dấu riêng), và lời chú giải phải mở đầu bằng chữ THƯỜNG — sách nầy không hề vậy,
# chú giải là một câu, mở đầu bằng chữ hoa. Chữ trả lại chính là chữ của cái nhãn ấy.
#
# Đã dò: cả 210 chỗ đều khớp với bản chép độc lập của bản in 1895, và riêng 壓 Áp thì đã mở
# ảnh trang in ra coi tận mắt (cuốn 1, ảnh 29).
#
#   node tools/scan-page.mjs 1 29 ap.png 0.50 0.52 0.96 0.70
";

/// The prose at the top of `review/gloss-initial-lost.toml`.
const GLOSS_LOST_DOSSIER_HEADER: &str = "\
# Lời chú giải mở đầu bằng chữ THƯỜNG, mà chương trình không trả lại được chữ hoa.
#
# SINH TỰ ĐỘNG bởi `parse` — đừng sửa tay phần thân. Chữ ký ở `verified_by` thì được giữ
# nguyên qua mỗi lần sinh lại, khớp theo `key`.
#
# Khác với gloss-initial-restored.toml: ở đó cái nhãn thừa còn giữ được chữ bị mất, nên trả
# về chỗ cũ là xong. Còn đây thì chữ ấy không còn trong lớp chữ nữa — mục chỉ có một nhãn,
# hoặc nhãn không phải là chữ bị mất. Bản chép Wikisource cũng hỏng y như vậy (hai bản ấy
# chung một gốc, coi README), nên KHÔNG có nhân chứng nào ngoài mực trên giấy.
#
# Cách tra: lấy số `pdf_page`, trừ 1 ra số trang bản in 2026, rồi tìm mục ấy trong bản 1895
# mà mở ảnh ra coi:
#
#   node tools/scan-page.mjs <cuốn> <ảnh> ra.png [x0 y0 x1 y1]
#
# Ký vào `verified_by` nghĩa là: đã mở ảnh coi, và chữ ghi ở `gloss` đúng như bản in.
";

/// The file name for this step gate results.
const PARSE_GATES_FILE: &str = "gates-parse.json";
/// The dossier directory. Taken from the crate location at compile time, not the cwd:
/// gate thresholds depend on the files there, so they must not vanish when run from elsewhere.
fn review_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../review")
}

#[derive(Serialize)]
struct EntryRecord {
    schema_version: u32,
    seq: usize,
    pdf_page: u16,
    printed_page: Option<u16>,
    /// `None` when the print uses an image for the glyph — 29 such entries.
    glyph: Option<String>,
    glyph_kind: &'static str,
    /// This entry reuses the glyph of the preceding one — 55 entries.
    inherits_glyph: bool,
    reading: String,
    /// The Sino-Vietnamese reading in parentheses; 39 entries have one.
    alternate: Option<String>,
    /// The labels in printed order; 548 entries carry two.
    pos: Vec<&'static str>,
    gloss: String,
    sub_entries: Vec<SubEntryRecord>,
}

#[derive(Serialize)]
struct SubEntryRecord {
    /// The Han part verbatim.
    han_form: Option<String>,
    /// Derived: `|` replaced by the entry glyph.
    han_expanded: Option<String>,
    /// The Quốc ngữ form verbatim — what the UI shows by default.
    form: String,
    /// Derived: the dash replaced by the reading — for search, never shown as the book words.
    form_expanded: String,
    definition: String,
    /// The form is interrupted by another font, or still holds an unresolved placeholder.
    needs_review: bool,
}

/// One line of the continuous stream, keeping its origin for traceability.
struct StreamLine {
    page: u16,
    text: String,
    role: LineRole,
    signal: SubEntrySignal,
    /// The style-separated pieces are kept so sub-entry fields split on font boundaries.
    segments: Vec<(TextStyle, String)>,
}

fn style_of(name: &str) -> TextStyle {
    match name {
        "italic" => TextStyle::Italic,
        "bold" => TextStyle::Bold,
        "han" => TextStyle::Han,
        _ => TextStyle::Regular,
    }
}

const fn role_name(r: LineRole) -> &'static str {
    match r {
        LineRole::RunningHead => "running head",
        LineRole::PageNumber => "page number",
        LineRole::SectionTitle => "section title",
        LineRole::Headword => "headword",
        LineRole::ContinuedHeadword => "continued headword",
        LineRole::SubEntry => "sub-entry",
        LineRole::Continuation => "continuation",
    }
}

const fn kind_name(k: GlyphKind) -> &'static str {
    match k {
        GlyphKind::Bmp => "BMP",
        GlyphKind::ExtB => "Ext-B",
        GlyphKind::Pua => "PUA",
        GlyphKind::ImageOnly => "image only",
    }
}

/// The `.notdef` glyph code the 2026 edition uses to mark where an IMAGE replaces a glyph,
/// plus one extra code on page 137. Both were checked by hand and explained; any other code is unexplained.
const ACCEPTED_UNMAPPED: [u32; 2] = [0x0000, 0x002A];

struct Stream {
    lines: Vec<StreamLine>,
    pages: usize,
    /// Occurrences of an unmapped AND unexplained glyph code — gate 1.
    unexplained_unmapped: usize,
    unmapped_total: usize,
}

fn read_stream(path: &PathBuf) -> Result<Stream> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut stream = Vec::new();
    let mut pages = 0usize;
    let mut unexplained_unmapped = 0usize;
    let mut unmapped_total = 0usize;

    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("reading line {}", index + 1))?;
        let record: PageRecord =
            serde_json::from_str(&line).with_context(|| format!("parsing line {}", index + 1))?;
        if record.schema_version != EXPECTED_SCHEMA {
            bail!(
                "page {}: schema_version {} but {EXPECTED_SCHEMA} is required",
                record.pdf_page,
                record.schema_version
            );
        }
        pages += 1;
        unmapped_total += record.unmapped.len();
        unexplained_unmapped += record
            .unmapped
            .iter()
            .filter(|u| !ACCEPTED_UNMAPPED.contains(&u.code))
            .count();

        let page = PdfPage::new(record.pdf_page)
            .with_context(|| format!("page number {}", record.pdf_page))?;

        for l in record.lines {
            let text = l.text();
            let has_italic = l.has_italic();
            let role = classify(l.y, &text, page.is_body(), has_italic);
            let signal = SubEntrySignal::of(has_italic, has_placeholder(text.trim()));
            stream.push(StreamLine {
                page: record.pdf_page,
                text,
                role,
                signal,
                segments: l
                    .segments
                    .iter()
                    .map(|seg| (style_of(&seg.style), seg.text.clone()))
                    .collect(),
            });
        }
    }
    Ok(Stream {
        lines: stream,
        pages,
        unexplained_unmapped,
        unmapped_total,
    })
}

/// Append a spill-over line to the text being built.
///
/// The print breaks lines mid-sentence, so a space must be inserted — but NOT when the
/// preceding part already ends in whitespace, to avoid a double space the print does not have.
fn join_text(target: &mut String, line: &str) {
    if line.is_empty() {
        return;
    }
    if !target.is_empty() && !target.ends_with(char::is_whitespace) {
        target.push(' ');
    }
    target.push_str(line);
}

fn main() -> Result<()> {
    let Some(path) = std::env::args_os().nth(1) else {
        bail!("usage: parse <pages.jsonl>");
    };
    let path = PathBuf::from(path);
    let out_path = std::env::args_os().nth(2).map(PathBuf::from);
    let loaded = read_stream(&path)?;
    let pages = loaded.pages;
    let unexplained_unmapped = loaded.unexplained_unmapped;
    let unmapped_total = loaded.unmapped_total;
    let stream = loaded.lines;

    // ── Classification ───────────────────────────────────────────────────────
    let mut by_role: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut content_chars = 0usize;
    for l in &stream {
        *by_role.entry(role_name(l.role)).or_default() += 1;
        if l.role.is_content() {
            content_chars += visible_len(&l.text);
        }
    }

    // ── Assembly over the CONTINUOUS stream ──────────────────────────────────
    let roles: Vec<LineRole> = stream.iter().map(|l| l.role).collect();
    let asm = assemble(&roles);
    let partition = check_partition(&roles, &asm);

    let mut consumed_chars = 0usize;
    let mut sub_entry_count = 0usize;
    let mut inherited = 0usize;
    for e in &asm.entries {
        if e.inherits_glyph {
            inherited += 1;
        }
        sub_entry_count += e.sub_entries.len();
        for i in e.line_indices() {
            consumed_chars += visible_len(&stream[i].text);
        }
    }
    for i in &asm.orphan_lines {
        consumed_chars += visible_len(&stream[*i].text);
    }

    // ── Gates on the headword line ───────────────────────────────────────────
    let mut glyph_kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut label_patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut unparsed: Vec<(u16, String)> = Vec::new();
    let mut coverage_failures: Vec<(u16, String)> = Vec::new();
    let mut stray_labels: Vec<(u16, String)> = Vec::new();

    for e in &asm.entries {
        if e.inherits_glyph {
            continue; // labels only, not a complete headword line
        }
        let l = &stream[e.headword_line];
        let Some(h) = parse_headword(&l.text) else {
            unparsed.push((l.page, l.text.clone()));
            continue;
        };
        *label_patterns
            .entry(
                h.pos_list()
                    .iter()
                    .map(|p| p.book_label())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .or_default() += 1;
        if !check_coverage(&l.text, &h.spans()).is_exact_partition() {
            coverage_failures.push((l.page, l.text.clone()));
        }
        if h.reading_has_stray_label(&l.text) {
            stray_labels.push((l.page, l.text.clone()));
        }
        let kind = h
            .glyph
            .map(|s| s.slice(&l.text))
            .and_then(|g| GlyphChar::parse(g).ok())
            .map_or(GlyphKind::ImageOnly, |c| c.kind());
        *glyph_kinds.entry(kind_name(kind)).or_default() += 1;
    }

    // ── Report ───────────────────────────────────────────────────────────────
    println!("== Line classification ==");
    println!("  trang            : {pages}");
    let total: usize = by_role.values().sum();
    println!("  total lines      : {total}");
    for (role, count) in &by_role {
        println!(
            "    {role:<14} {count:>6}  {:>5.2}%",
            *count as f64 / total as f64 * 100.0
        );
    }

    println!();
    println!("== Entry assembly (continuous stream over 1038 pages) ==");
    println!("  entries          : {}", asm.entries.len());
    println!("    reusing a glyph: {inherited}");
    println!("  sub-entries      : {sub_entry_count}");
    for (kind, count) in &glyph_kinds {
        println!("    glyph {kind:<10} {count:>5}");
    }
    for (pattern, count) in &label_patterns {
        println!("    label {pattern:<10} {count:>5}");
    }

    println!();
    println!("== Gate 3: the assembly must partition the line stream ==");
    println!("  content chars    : {content_chars}");
    println!("  consumed chars   : {consumed_chars}");
    println!(
        "  DIFFERENCE       : {}",
        content_chars as i64 - consumed_chars as i64
    );
    println!("  orphan lines     : {}", asm.orphan_lines.len());
    println!("  missing lines    : {}", partition.missing.len());
    println!("  lines consumed twice: {}", partition.duplicated.len());
    println!(
        "  non-content lines swallowed: {}",
        partition.unexpected.len()
    );

    // ── Sub-entry field splitting and derivation, entry by entry ─────────────
    // It must walk entry by entry rather than scan flat: derivation needs the glyph and
    // reading of the entry that owns the sub-entry.
    let mut sub_total = 0usize;
    let mut sub_with_han = 0usize;
    let mut sub_without_def = 0usize;
    let mut wrapped_forms_merged = 0usize;
    let mut sub_needs_review = 0usize;
    let mut sub_unsplit: Vec<(u16, String)> = Vec::new();
    let mut sub_coverage_fail: Vec<(u16, String)> = Vec::new();
    let mut expanded_reading = 0usize;
    let mut expanded_han = 0usize;
    let mut no_substitution = 0usize;
    let mut residual: Vec<(u16, String)> = Vec::new();
    let mut han_blocked_by_image: usize = 0;
    let scan_rows = ScanReadings::load(&review_dir())
        .context("reading review/scan-verified.toml")?
        .reading;
    let mut scan_applied: usize = 0;
    let mut scan_found: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut gloss_initials_restored: Vec<GlossInitialFix> = Vec::new();
    let mut gloss_initials_lost: Vec<GlossInitialFix> = Vec::new();

    // An entry reusing the previous glyph inherits its derivation context too.
    let mut context: Option<(Option<String>, String)> = None;
    let mut records: Vec<EntryRecord> = Vec::new();

    for (seq, e) in asm.entries.iter().enumerate() {
        let head = &stream[e.headword_line];

        let mut alternate: Option<String> = None;
        let mut pos: Vec<&'static str> = Vec::new();
        let mut gloss_prefix = String::new();
        let mut headword = None;

        if e.inherits_glyph {
            if let Some(c) = parse_continued_headword(&head.text) {
                pos = c.pos_list().iter().map(|p| p.db_value()).collect();
                if let Some(d) = c.definition {
                    gloss_prefix.push_str(d.slice(&head.text));
                }
            }
        } else if let Some(h) = parse_headword(&head.text) {
            context = Some((
                h.glyph.map(|g| g.slice(&head.text).to_owned()),
                h.reading.slice(&head.text).to_owned(),
            ));
            alternate = h.alternate.map(|a| a.slice(&head.text).to_owned());
            pos = h.pos_list().iter().map(|p| p.db_value()).collect();
            headword = Some(h);
        }

        let Some((glyph, reading)) = context.as_ref() else {
            continue;
        };
        let ctx = HeadwordContext {
            glyph: glyph.as_deref(),
            reading,
        };

        // The main gloss: the continuation lines before the first sub-entry, joined.
        let mut gloss = gloss_prefix;
        for i in &e.gloss_lines {
            join_text(&mut gloss, stream[*i].text.trim());
        }

        // Put back the capital the 2026 rebuild filed as a part-of-speech label. The test
        // lives in `parse-core` with the evidence; here we only apply what it decides, and
        // record every single one so the repair can be audited against the scan.
        if let Some(h) = headword.as_ref()
            && let Some(fix) = h.gloss_initial_label(&head.text, &gloss)
        {
            pos.retain(|p| *p != fix.dropped.db_value());
            gloss.insert(0, fix.letter);
            gloss_initials_restored.push(GlossInitialFix {
                page: head.page,
                reading: reading.clone(),
                letter: fix.letter,
                line: head.text.trim().to_owned(),
                gloss: gloss.clone(),
            });
        }

        // What is left: a definition still opening in lowercase, which this book never does.
        // Nothing here is repaired — the label was the entry's only one, or there was no
        // label to give the letter back. Both editions are damaged in the same way on these,
        // so only the 1895 scan can settle them.
        if gloss
            .trim_start()
            .chars()
            .next()
            .is_some_and(char::is_lowercase)
        {
            gloss_initials_lost.push(GlossInitialFix {
                page: head.page,
                reading: reading.clone(),
                letter: '\0',
                line: head.text.trim().to_owned(),
                gloss: gloss.clone(),
            });
        }

        let mut subs: Vec<SubEntryParts> = Vec::new();
        for block in &e.sub_entries {
            let l = &stream[block.first_line];
            sub_total += 1;
            let segs: Vec<StyledSegment<'_>> = l
                .segments
                .iter()
                .map(|(style, text)| StyledSegment {
                    style: *style,
                    text,
                })
                .collect();
            let Some(sub) = parse_sub_entry(&segs) else {
                sub_unsplit.push((l.page, l.text.trim().to_owned()));
                continue;
            };
            if sub.han_form.is_some() {
                sub_with_han += 1;
            }
            if sub.definition.is_none() {
                sub_without_def += 1;
            }
            if sub.needs_review() {
                sub_needs_review += 1;
            }
            if !check_coverage(&l.text, &sub.spans()).is_exact_partition() {
                sub_coverage_fail.push((l.page, l.text.trim().to_owned()));
            }

            let form = sub.reading_form.slice(&l.text);
            let exp = expand_reading_form(form, &ctx);
            if exp.substitutions > 0 {
                expanded_reading += 1;
            } else {
                no_substitution += 1;
            }
            if exp.needs_review() && residual.len() < 2000 {
                residual.push((l.page, form.to_owned()));
            }

            let han_raw = sub.han_form.map(|sp| sp.slice(&l.text).to_owned());
            let mut han_expanded = None;
            if let Some(h) = han_raw.as_deref() {
                match expand_han_form(h, &ctx) {
                    Some(he) => {
                        if he.substitutions > 0 {
                            expanded_han += 1;
                        }
                        han_expanded = Some(he.as_str().to_owned());
                    }
                    None => han_blocked_by_image += 1,
                }
            }

            // The sub-entry definition, including the lines flowing after it.
            // A missing definition is valid: 111 sub-entries start theirs on the next line.
            let mut definition = sub
                .definition
                .map_or_else(String::new, |d| d.slice(&l.text).trim().to_owned());
            for i in &block.continuation_lines {
                join_text(&mut definition, stream[*i].text.trim());
            }

            subs.push(SubEntryParts {
                han_form: han_raw,
                han_expanded,
                form: form.to_owned(),
                form_expanded: exp.as_str().to_owned(),
                definition,
                needs_review: sub.needs_review() || exp.needs_review(),
            });
        }

        // Re-join the forms the print wrapped onto a second line. Done on the records, not
        // on the line indices, so gate ③ still sees each content line consumed exactly once.
        let (subs, merged) = merge_wrapped_forms(subs);
        wrapped_forms_merged += merged;
        let subs: Vec<SubEntryRecord> = subs
            .into_iter()
            .map(|s| SubEntryRecord {
                han_form: s.han_form,
                han_expanded: s.han_expanded,
                form: s.form,
                form_expanded: s.form_expanded,
                definition: s.definition,
                needs_review: s.needs_review,
            })
            .collect();

        let kind = glyph
            .as_deref()
            .and_then(|g| GlyphChar::parse(g).ok())
            .map_or(GlyphKind::ImageOnly, |c| c.kind());

        // A reading somebody settled against the 1895 scan. Only a SIGNED row is applied:
        // an unsigned one is a claim about a page, not a decision yet. Every row is looked
        // for whether signed or not, so gate ⑧ can tell a stale dossier from a live one.
        let mut subs = subs;
        for (i, r) in scan_rows.iter().enumerate() {
            if r.pdf_page != head.page {
                continue;
            }
            let mut seen = gloss.contains(r.was.as_str());
            for sub in &subs {
                seen |= sub.form.contains(r.was.as_str())
                    || sub.form_expanded.contains(r.was.as_str())
                    || sub.definition.contains(r.was.as_str());
            }
            if seen {
                scan_found.insert(i);
            }
            if !r.reviewed.is_verified() {
                continue;
            }
            if seen {
                scan_applied += 1;
            }
            gloss = gloss.replace(r.was.as_str(), r.now.as_str());
            for sub in &mut subs {
                sub.form = sub.form.replace(r.was.as_str(), r.now.as_str());
                sub.form_expanded = sub.form_expanded.replace(r.was.as_str(), r.now.as_str());
                sub.definition = sub.definition.replace(r.was.as_str(), r.now.as_str());
            }
        }

        records.push(EntryRecord {
            schema_version: ENTRY_SCHEMA_VERSION,
            seq: seq + 1,
            pdf_page: head.page,
            printed_page: PdfPage::new(head.page)
                .ok()
                .and_then(|p| p.printed())
                .map(|p| p.get()),
            glyph: glyph.clone(),
            glyph_kind: kind_name(kind),
            inherits_glyph: e.inherits_glyph,
            reading: reading.clone(),
            alternate,
            pos,
            gloss,
            sub_entries: subs,
        });
    }

    // ── Gate 3 at the CHARACTER level ────────────────────────────────────────
    // The line level only proves no line is lost. This is stronger: every character of
    // every content line must sit inside a split field, exactly once.
    let mut char_total = 0usize;
    let mut char_covered = 0usize;
    let mut uncovered_lines: Vec<(u16, String)> = Vec::new();

    for l in stream.iter().filter(|l| l.role.is_content()) {
        let n = visible_len(&l.text);
        char_total += n;

        // No `unwrap_or_default` here: a line that fails to split has an empty span list,
        // and the character difference will surface it in the report — writing `map_or_else`
        // makes clear that an empty list is a meaningful RESULT, not a fallback value.
        let spans: Vec<Span> = match l.role {
            LineRole::Headword => parse_headword(&l.text).map_or_else(Vec::new, |h| h.spans()),
            LineRole::ContinuedHeadword => {
                parse_continued_headword(&l.text).map_or_else(Vec::new, |c| c.spans())
            }
            LineRole::SubEntry => {
                let segs: Vec<StyledSegment<'_>> = l
                    .segments
                    .iter()
                    .map(|(style, text)| StyledSegment {
                        style: *style,
                        text,
                    })
                    .collect();
                parse_sub_entry(&segs).map_or_else(Vec::new, |sub| sub.spans())
            }
            // A continuation is the spill-over of a definition: the whole line is one field.
            LineRole::Continuation => {
                Span::new(&l.text, 0, l.text.len()).map_or_else(|_| Vec::new(), |s| vec![s])
            }
            _ => Vec::new(),
        };

        let covered: usize = spans.iter().map(|s| span_visible(s.slice(&l.text))).sum();
        char_covered += covered;
        if covered != n {
            uncovered_lines.push((l.page, l.text.trim().to_owned()));
        }
    }

    let ambiguous: Vec<&StreamLine> = stream
        .iter()
        .filter(|l| l.role.is_content() && l.signal.is_ambiguous())
        .collect();
    println!();
    println!("== Sub-entry field splitting (style boundaries) ==");
    println!("  sub-entry lines  : {sub_total}");
    println!("  with a Han part  : {sub_with_han}");
    println!("  definition on the next line: {sub_without_def}");
    println!("  wrapped forms re-joined: {wrapped_forms_merged}");
    println!("  form interrupted (needs review): {sub_needs_review}");
    println!("  COULD NOT split  : {}", sub_unsplit.len());
    for (p, t) in sub_unsplit.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }
    println!("  incomplete coverage: {}", sub_coverage_fail.len());
    for (p, t) in sub_coverage_fail.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }

    println!();
    println!("== Derivation by the two rules of the DẤU RIÊNG page ==");
    println!("  forms with dash -> reading substituted : {expanded_reading}");
    println!("  Han parts with | -> glyph substituted  : {expanded_han}");
    println!("  forms with no placeholder at all       : {no_substitution}");
    println!("  not derivable because the glyph is an image: {han_blocked_by_image}");
    println!(
        "  placeholders still unresolved (needs review): {}",
        residual.len()
    );
    for (p, t) in residual.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }

    // Write the dossier rather than leaving it to be kept by hand. It held 829 rows for a
    // fault that turned out to be the splitter's, and stayed that way after the fix: a
    // dossier nobody regenerates describes the data as it once was, which is exactly the
    // false confidence `gates.json` carries a fingerprint to prevent.
    let residual_rows: Vec<DossierRow> = residual
        .iter()
        .map(|(page, form)| DossierRow {
            key: format!("{page}|{form}"),
            fields: vec![("pdf_page", (*page).into()), ("form", form.clone().into())],
        })
        .collect();
    let residual_path = review_dir().join(ResidualPlaceholders::FILE);
    let kept = write_dossier(
        &residual_path,
        "residual_placeholder",
        RESIDUAL_DOSSIER_HEADER,
        &residual_rows,
    )?;
    println!(
        "  wrote {} ({} rows, {kept} signature(s) carried over)",
        residual_path.display(),
        residual_rows.len()
    );

    println!();
    println!("== The opening capital of the definition ==");
    println!(
        "  restored from a spurious label: {}",
        gloss_initials_restored.len()
    );
    for f in gloss_initials_restored.iter().take(5) {
        println!(
            "    tr{} {}: {:?}",
            f.page,
            f.reading,
            f.gloss.chars().take(40).collect::<String>()
        );
    }
    println!(
        "  still missing (only the scan can say): {}",
        gloss_initials_lost.len()
    );
    for f in gloss_initials_lost.iter().take(5) {
        println!(
            "    tr{} {}: {:?}",
            f.page,
            f.reading,
            f.gloss.chars().take(40).collect::<String>()
        );
    }

    let restored_rows: Vec<DossierRow> = gloss_initials_restored
        .iter()
        .map(|f| DossierRow {
            key: format!("{}|{}|{}", f.page, f.reading, f.letter),
            fields: vec![
                ("pdf_page", f.page.into()),
                ("reading", f.reading.clone().into()),
                ("line", f.line.clone().into()),
                ("restored", f.letter.to_string().into()),
                ("gloss", f.gloss.clone().into()),
            ],
        })
        .collect();
    let restored_path = review_dir().join(GlossInitialsRestored::FILE);
    let kept = write_dossier(
        &restored_path,
        "gloss_initial_restored",
        GLOSS_RESTORED_DOSSIER_HEADER,
        &restored_rows,
    )?;
    println!(
        "  wrote {} ({} rows, {kept} signature(s) carried over)",
        restored_path.display(),
        restored_rows.len()
    );

    let lost_rows: Vec<DossierRow> = gloss_initials_lost
        .iter()
        .map(|f| DossierRow {
            key: format!("{}|{}", f.page, f.reading),
            fields: vec![
                ("pdf_page", f.page.into()),
                ("reading", f.reading.clone().into()),
                ("line", f.line.clone().into()),
                ("gloss", f.gloss.clone().into()),
            ],
        })
        .collect();
    let lost_path = review_dir().join(GlossInitialsLost::FILE);
    let kept = write_dossier(
        &lost_path,
        "gloss_initial_lost",
        GLOSS_LOST_DOSSIER_HEADER,
        &lost_rows,
    )?;
    println!(
        "  wrote {} ({} rows, {kept} signature(s) carried over)",
        lost_path.display(),
        lost_rows.len()
    );

    println!();
    println!("== Gate 8: readings settled against the 1895 scan ==");
    println!("  rows in review/scan-verified.toml: {}", scan_rows.len());
    println!("  signed, and applied to an entry  : {scan_applied}");
    let stale: Vec<&ScanReading> = scan_rows
        .iter()
        .enumerate()
        .filter(|(i, _)| !scan_found.contains(i))
        .map(|(_, r)| r)
        .collect();
    println!("  rows pointing at nothing        : {}", stale.len());
    for r in stale.iter().take(8) {
        println!("    tr{}: {:?} not found on that page", r.pdf_page, r.was);
    }
    for r in scan_rows.iter().filter(|r| !r.reviewed.is_verified()) {
        println!(
            "    tr{} {:?} -> {:?}  NOT SIGNED, so not applied — node tools/scan-page.mjs {} {} ra.png {}",
            r.pdf_page, r.was, r.now, r.volume, r.scan_page, r.crop
        );
    }

    println!();
    println!("== Gate 3 at the CHARACTER level: every character must sit in a field ==");
    println!("  content chars    : {char_total}");
    println!("  chars placed in a field: {char_covered}");
    println!(
        "  DIFFERENCE       : {}",
        char_total as i64 - char_covered as i64
    );
    println!("  lines not fully covered: {}", uncovered_lines.len());
    for (p, t) in uncovered_lines.iter().take(8) {
        println!("    tr{p}: {t:?}");
    }

    println!();
    println!("== Ambiguous lines: a placeholder but no italic ==");
    println!("  count            : {}", ambiguous.len());
    println!("  (needs a human — sub-entry or continuation cannot be decided automatically)");
    for l in ambiguous.iter().take(8) {
        println!("    tr{}: {:?}", l.page, l.text.trim());
    }

    println!();
    println!("== Gate 2 and the stray-label check on headword lines ==");
    println!("  could not split  : {}", unparsed.len());
    for (p, t) in unparsed.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }
    println!("  incomplete coverage: {}", coverage_failures.len());
    for (p, t) in coverage_failures.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }
    println!("  stray label in the reading: {}", stray_labels.len());
    for (p, t) in stray_labels.iter().take(5) {
        println!("    tr{p}: {t:?}");
    }

    // ── The gates, in machine-readable form ──────────────────────────────────
    //
    // Printing to the screen is enough for a HUMAN to know whether the data is clean, but
    // it stops no bad load. The three gates of this step are written to a file so `import` can refuse.
    let dossiers = LabelTypos::load(&review_dir()).context("reading review/label-typos.toml")?;
    let gate_path = path.with_file_name(PARSE_GATES_FILE);
    let gates = vec![
        GateResult::judge(
            Gate::UnmappedGlyphs,
            unexplained_unmapped as i64,
            0,
            format!(
                "{unmapped_total} unmapped occurrences, all belonging to the two hand-checked codes \n                 (0x0000 = where the 2026 edition places an image, 0x002A on page 137)"
            ),
        ),
        GateResult::judge(
            Gate::HeadwordCoverage,
            (unparsed.len() + coverage_failures.len() + stray_labels.len()) as i64,
            dossiers.len() as i64,
            format!(
                "threshold taken straight from review/{}: {} labels missing a dot in the print",
                LabelTypos::FILE,
                dossiers.len()
            ),
        ),
        GateResult::judge(
            Gate::CharacterConservation,
            char_total as i64 - char_covered as i64,
            dossiers.caught_by(caught_by::CHECK_COVERAGE) as i64,
            format!(
                "threshold taken straight from review/{}: {} entries caught by check_coverage",
                LabelTypos::FILE,
                dossiers.caught_by(caught_by::CHECK_COVERAGE)
            ),
        ),
        // Gate ⑧. A row here changes the published text, so the one thing that must never
        // happen is a row quietly pointing at nothing: the 2026 text layer changed, or the
        // row names the wrong page, and either way it has stopped being evidence. Signing
        // is a separate question — an unsigned row is reported every run, never applied.
        GateResult::judge(
            Gate::ScanVerified,
            stale.len() as i64,
            0,
            format!(
                "{} reading(s) settled against the scan, {scan_applied} applied; \
                 every row still finds its wording on the page it names",
                scan_rows.len()
            ),
        ),
    ];
    let report = GateReport::new(
        path.display().to_string(),
        Fingerprint::of_file(&path).context("fingerprinting pages.jsonl")?,
        gates,
    );
    report
        .write(&gate_path)
        .with_context(|| format!("writing {}", gate_path.display()))?;
    println!();
    println!("wrote the gate 1/2/3/8 results to {}", gate_path.display());
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

    if let Some(out) = out_path {
        let mut w = BufWriter::new(
            File::create(&out).with_context(|| format!("creating {}", out.display()))?,
        );
        for r in &records {
            serde_json::to_writer(&mut w, r).context("writing entries.jsonl")?;
            w.write_all(
                b"
",
            )?;
        }
        w.flush()?;
        println!();
        println!("wrote {} entries to {}", records.len(), out.display());
    }

    Ok(())
}
