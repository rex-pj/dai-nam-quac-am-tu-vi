//! Loading checked data into PostgreSQL — **the only door** data enters the store through.
//!
//! The web layer has no write path; everything in the database came through here. So this is
//! where the promise "data must not be guessed" is enforced rather than merely stated:
//!
//! **The program refuses to load if `data/gates.json` is missing, incomplete, has a red
//! gate, or was produced from a different `entries.jsonl` than the one being loaded.**
//!
//! The last condition is the easiest to forget and the most dangerous: a green but stale
//! report creates false confidence, which is worse than having no report at all.
//!
//! Usage: `import <entries.jsonl> [pages.jsonl]`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dnqatv_adapter_db::{Db, connect, migrate};
use dnqatv_core::model::{
    GlyphChar, GlyphKind, Letter, PdfPage, Pos, Reading, Slug, SlugMinter, page::PDF_PAGE_COUNT,
};
use dnqatv_entity::{entry, front_matter, glyph, page, sub_entry};
use dnqatv_gate_report::dossier::Dossiers;
use dnqatv_gate_report::{Fingerprint, GateReport};
use sea_orm::{ActiveValue::Set, EntityTrait, TransactionTrait};
use serde::Deserialize;

/// Must match `ENTRY_SCHEMA_VERSION` in `parse`.
const EXPECTED_SCHEMA: u32 = 1;
/// The gate report written by `reconcile`.
const GATES_FILE: &str = "gates.json";
/// Rows per `INSERT`. Postgres caps a statement at 65,535 parameters, and the `sub_entry`
/// table has 9 columns — 2,000 rows is 18,000 parameters, safe and still fast.
const CHUNK: usize = 2_000;
/// The y coordinate of the running head. Exactly one value across all 1,028 body pages.
const RUNNING_HEAD_Y: f64 = 797.8898;
const Y_EPSILON: f64 = 0.01;

// ── Intermediate records ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EntryRecord {
    schema_version: u32,
    seq: i32,
    pdf_page: u16,
    printed_page: Option<u16>,
    glyph: Option<String>,
    inherits_glyph: bool,
    reading: String,
    alternate: Option<String>,
    pos: Vec<String>,
    gloss: String,
    sub_entries: Vec<SubEntryRecord>,
}

#[derive(Deserialize)]
struct SubEntryRecord {
    han_form: Option<String>,
    han_expanded: Option<String>,
    form: String,
    form_expanded: String,
    definition: String,
    needs_review: bool,
}

#[derive(Deserialize)]
struct PageRecord {
    pdf_page: u16,
    lines: Vec<PageLine>,
}

#[derive(Deserialize)]
struct PageLine {
    y: f64,
    column: String,
    segments: Vec<PageSegment>,
}

#[derive(Deserialize)]
struct PageSegment {
    text: String,
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let Some(entries_arg) = args.next() else {
        bail!("usage: import <entries.jsonl> [pages.jsonl]");
    };
    let entries_path = PathBuf::from(entries_arg);
    let pages_path = args.next().map(PathBuf::from);

    // 1. The gate. Placed BEFORE opening a connection: there is no reason to touch the
    //    database while the data is not cleared for loading.
    let report = authorize(&entries_path)?;

    let records = read_entries(&entries_path)?;
    let heads = match &pages_path {
        Some(p) => read_running_heads(p)?,
        None => BTreeMap::new(),
    };
    let dossiers = Dossiers::load(&review_dir()).context("reading the dossiers in review/")?;

    let front_matter = match &pages_path {
        Some(p) => read_front_matter(p)?,
        None => Vec::new(),
    };
    let plan = build(&records, &heads, front_matter, &dossiers)?;
    plan.report();

    // 2. Load.
    let root = workspace_root();
    let config = dnqatv_config::load_from_dotenv_and_env(&root.join(".env"))
        .context("loading configuration")?;
    let db = connect(&config.database)
        .await
        .with_context(|| format!("connecting to {}", config.database.url))?;
    migrate::up(&db).await.context("running migrations")?;

    load(&db, &plan).await?;

    println!();
    println!(
        "loaded into {} — gate report: {} gates, all PASS",
        config.database.url.database(),
        report.results.len()
    );
    db.close().await?;
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn review_dir() -> PathBuf {
    workspace_root().join("review")
}

/// The fail-closed gate. Returns the report if — and only if — loading is permitted.
fn authorize(entries_path: &Path) -> Result<GateReport> {
    let gates_path = entries_path.with_file_name(GATES_FILE);
    let report = GateReport::read(&gates_path).with_context(|| {
        format!(
            "reading {} — run `parse` then `reconcile` before loading",
            gates_path.display()
        )
    })?;
    let fingerprint = Fingerprint::of_file(entries_path)
        .with_context(|| format!("fingerprinting {}", entries_path.display()))?;
    report
        .authorize(fingerprint)
        .context("REFUSING TO LOAD: the data has not passed every gate")?;

    println!("== Gate ==");
    for r in &report.results {
        println!(
            "  gate {} {:<45} measured {:>3} / allowed {:>3}  PASS",
            r.gate.number(),
            r.gate.title(),
            r.measured,
            r.allowed
        );
    }
    Ok(report)
}

fn read_entries(path: &Path) -> Result<Vec<EntryRecord>> {
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

/// Front-matter pages: every page before the book body.
const FRONT_MATTER_LAST_PAGE: u16 = 10;
/// A line longer than this is certainly body text, not a title.
const HEADING_MAX_CHARS: usize = 40;

/// The front matter, read **verbatim, page by page**.
///
/// Deliberately makes NO attempt to merge pages into "chapters". Pages 4, 7 and 8 continue
/// TIỂU TỰ and PRÉFACE, but that boundary must be confirmed by someone reading the print,
/// not by a rule guessing from line shapes. One page per item, linked back to the source
/// page, is enough and invents nothing.
/// The running head of a page: left column and right column.
type RunningHead = (Option<String>, Option<String>);

fn read_front_matter(path: &Path) -> Result<Vec<front_matter::ActiveModel>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let p: PageRecord = serde_json::from_str(line).context("parsing pages.jsonl")?;
        if p.pdf_page > FRONT_MATTER_LAST_PAGE {
            continue;
        }
        let lines: Vec<String> = p
            .lines
            .iter()
            .filter(|l| (l.y - RUNNING_HEAD_Y).abs() >= Y_EPSILON)
            .map(|l| {
                l.segments
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|t| !t.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }

        // The title: the first short all-caps line — the print sets titles in capitals.
        // Without one, use the page number; a dull title beats an invented one.
        let title = lines
            .iter()
            .find(|t| is_heading(t))
            .cloned()
            .unwrap_or_else(|| format!("Trang {}", p.pdf_page));

        out.push(front_matter::ActiveModel {
            id: Set(i32::from(p.pdf_page)),
            slug: Set(format!("trang-{}", p.pdf_page)),
            title: Set(title),
            body: Set(lines.join("\n\n")),
            pdf_page: Set(p.pdf_page as i16),
        });
    }
    Ok(out)
}

/// Whether this line is a title: short, contains letters, and has **no lowercase**.
fn is_heading(text: &str) -> bool {
    let count = text.chars().count();
    count <= HEADING_MAX_CHARS
        && text.chars().any(char::is_alphabetic)
        && !text.chars().any(char::is_lowercase)
}

/// The running head, split into left and right column.
///
/// The print records the first and last entry of each page at its top. This is data of
/// **the print**, so it is stored verbatim rather than re-derived from the parsed entries:
/// if the two disagree, that is a finding, not a defect to hide.
fn read_running_heads(path: &Path) -> Result<BTreeMap<u16, RunningHead>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let p: PageRecord = serde_json::from_str(line).context("parsing pages.jsonl")?;
        let mut left = None;
        let mut right = None;
        for l in &p.lines {
            if (l.y - RUNNING_HEAD_Y).abs() >= Y_EPSILON {
                continue;
            }
            let t: String = l.segments.iter().map(|s| s.text.as_str()).collect();
            let t = t.trim().to_owned();
            if t.is_empty() {
                continue;
            }
            match l.column.as_str() {
                "left" => left = Some(t),
                _ => right = Some(t),
            }
        }
        out.insert(p.pdf_page, (left, right));
    }
    Ok(out)
}

// ── Building the load plan ───────────────────────────────────────────────────

struct Plan {
    pages: Vec<page::ActiveModel>,
    front_matter: Vec<front_matter::ActiveModel>,
    glyphs: Vec<glyph::ActiveModel>,
    entries: Vec<entry::ActiveModel>,
    sub_entries: Vec<sub_entry::ActiveModel>,
    needing_review: usize,
    image_only: usize,
}

impl Plan {
    fn report(&self) {
        println!();
        println!("== To be loaded ==");
        println!("  trang        : {}", self.pages.len());
        println!("  glyphs        : {}", self.glyphs.len());
        println!("  entries       : {}", self.entries.len());
        println!("    image-only  : {}", self.image_only);
        println!("    need review : {}", self.needing_review);
        println!("  sub-entries   : {}", self.sub_entries.len());
        println!("  front matter  : {} pages", self.front_matter.len());
    }
}

fn build(
    records: &[EntryRecord],
    heads: &BTreeMap<u16, RunningHead>,
    front_matter: Vec<front_matter::ActiveModel>,
    dossiers: &Dossiers,
) -> Result<Plan> {
    // Glyphs: keyed by the character itself, ids in code-point order so every load matches.
    let mut glyph_chars: BTreeSet<String> = BTreeSet::new();
    for r in records {
        if let Some(g) = &r.glyph {
            glyph_chars.insert(g.clone());
        }
    }
    let mut by_codepoint: Vec<GlyphChar> = glyph_chars
        .iter()
        .map(|g| GlyphChar::parse(g).with_context(|| format!("glyph {g:?}")))
        .collect::<Result<_>>()?;
    by_codepoint.sort_by_key(GlyphChar::codepoint);

    let mut glyph_id: BTreeMap<char, i32> = BTreeMap::new();
    let mut glyphs = Vec::with_capacity(by_codepoint.len());
    for (i, g) in by_codepoint.iter().enumerate() {
        let id = i as i32 + 1;
        glyph_id.insert(g.ch(), id);
        glyphs.push(glyph::ActiveModel {
            id: Set(id),
            char: Set(g.ch().to_string()),
            codepoint: Set(g.codepoint() as i32),
            kind: Set(dnqatv_entity::GlyphKind::from(g.kind())),
            image_path: Set(None),
            // Gate 4 proved 0 "invented" glyphs: every glyph the parser found is in the
            // entry index. That gate is a precondition for reaching here, so this flag follows.
            in_index: Set(true),
        });
    }

    // Pages: one row for EVERY PDF page, including pages with no entries.
    // Front-matter pages must exist too, so `/trang/{n}` has no holes.
    let mut letter_of_page: BTreeMap<u16, Letter> = BTreeMap::new();
    for r in records {
        let reading =
            Reading::parse(&r.reading).with_context(|| format!("reading {:?}", r.reading))?;
        let letter = reading
            .letter()
            .with_context(|| format!("letter of {:?}", r.reading))?;
        letter_of_page.entry(r.pdf_page).or_insert(letter);
    }

    let mut pages = Vec::with_capacity(PDF_PAGE_COUNT as usize);
    for n in 1..=PDF_PAGE_COUNT {
        let pdf = PdfPage::new(n).with_context(|| format!("trang {n}"))?;
        let (head_first, head_last) = match heads.get(&n) {
            Some(h) => h.clone(),
            // The first ten pages have no running head. Its absence here is a FACT about the
            // print, not a fallback value — so it is written as a branch, not hidden behind
            // `unwrap_or`. The CI grep gate watches exactly this spot.
            None => (None, None),
        };
        pages.push(page::ActiveModel {
            id: Set(i32::from(n)),
            pdf_page: Set(n as i16),
            printed_page: Set(pdf.printed().map(|p| p.get() as i16)),
            letter: Set(letter_of_page.get(&n).map(|l| (*l).into())),
            image_path: Set(None),
            head_first: Set(head_first),
            head_last: Set(head_last),
        });
    }

    // Entries whose label lacks a dot in the print — flagged so the UI shows a warning strip.
    //
    // Matched by the (page, line) PAIR rather than by page alone: flagging a whole page
    // would mark 534 entries while the print has only 2 broken lines, and a warning spread
    // that widely teaches readers to ignore it.
    let typo_lines: Vec<(u16, &str)> = dossiers
        .label_typos
        .label_typo
        .iter()
        .map(|t| (t.pdf_page, t.line.as_str()))
        .collect();

    // Ambiguous lines: they carry a placeholder but are NOT italic, so the program cannot
    // decide by itself whether they are a sub-entry or a continuation. Each was swallowed
    // into the gloss of some entry.
    //
    // Why they must be flagged: those 39 lines have dossiers in review/, but a dossier lives
    // in the repo while readers read the website. Not flagging them means a reader sees a
    // passage that looks broken with nothing saying it is awaiting review.
    let ambiguous_lines: Vec<(u16, &str)> = dossiers
        .ambiguous_lines
        .ambiguous_line
        .iter()
        .map(|a| (a.pdf_page, a.text.trim()))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    let mut ambiguous_matched = 0usize;

    // Which ambiguous lines found an entry to sit next to. A line that finds none is shown
    // to nobody: it was absorbed into a continuation and reads as ordinary text.
    let mut ambiguous_owned = vec![false; ambiguous_lines.len()];

    // Shape descriptions, keyed by the entry they belong to. Unsigned rows are dropped here
    // rather than further down, so there is exactly one place where the rule lives.
    let signed_shape_notes: BTreeMap<(u16, String), String> = dossiers
        .shape_notes
        .shape_note_available
        .iter()
        .filter(|s| s.reviewed.is_verified())
        .map(|s| ((s.printed_page, s.reading.clone()), s.theirs.clone()))
        .collect();
    let mut shape_notes_applied = 0usize;

    let mut minter = SlugMinter::new();
    let mut entries = Vec::with_capacity(records.len());
    let mut sub_entries = Vec::with_capacity(64_000);
    let mut sub_id = 0i32;
    let mut needing_review = 0usize;
    let mut image_only = 0usize;

    for r in records {
        let reading = Reading::parse(&r.reading)?;
        let slug: Slug = minter.mint(&reading)?;
        let pos: Vec<dnqatv_entity::EntryPos> = r
            .pos
            .iter()
            .map(|p| Pos::from_db_value(p).map(dnqatv_entity::EntryPos::from))
            .collect::<std::result::Result<_, _>>()
            .with_context(|| format!("labels of entry {}", r.seq))?;
        if pos.is_empty() {
            bail!("entry {} has no part-of-speech label", r.seq);
        }

        let glyph_ref = match &r.glyph {
            Some(g) => {
                let ch = GlyphChar::parse(g)?;
                Some(
                    *glyph_id
                        .get(&ch.ch())
                        .context("glyph has not been assigned an id")?,
                )
            }
            None => {
                image_only += 1;
                None
            }
        };

        // `sub_text` is the denormalised concatenation, for search ONLY. It uses the derived
        // form (`form_expanded`) because a reader types "nạm gươm", not "― gươm".
        let mut sub_text = String::new();
        let sub_review = r.sub_entries.iter().any(|s| s.needs_review);
        // 32 of the 39 ambiguous lines land inside a sub-entry rather than in the gloss of
        // the entry. Flag that sub-entry so the mark sits next to the suspect passage rather
        // than on the whole entry.
        //
        // The page window is ±1, not an exact match. An entry begins on one page and runs on
        // to the next, so a line printed on page N can belong to an entry recorded as page
        // N−1. Pinning the search to one page left 12 of the 39 with no owner at all: they
        // were absorbed into a continuation and the reader saw no mark on them.
        let sub_ambiguous: Vec<bool> = r
            .sub_entries
            .iter()
            .map(|s| {
                let mut hit = false;
                for (k, (page, text)) in ambiguous_lines.iter().enumerate() {
                    if page.abs_diff(r.pdf_page) <= 1
                        && (s.definition.contains(text) || s.form.contains(text))
                    {
                        ambiguous_owned[k] = true;
                        hit = true;
                    }
                }
                hit
            })
            .collect();
        for (i, s) in r.sub_entries.iter().enumerate() {
            sub_id += 1;
            if !sub_text.is_empty() {
                sub_text.push(' ');
            }
            sub_text.push_str(&s.form_expanded);
            sub_text.push(' ');
            sub_text.push_str(&s.definition);
            sub_entries.push(sub_entry::ActiveModel {
                id: Set(sub_id),
                entry_id: Set(r.seq),
                seq: Set(i as i32 + 1),
                han_form: Set(s.han_form.clone()),
                han_expanded: Set(s.han_expanded.clone()),
                form: Set(s.form.clone()),
                form_expanded: Set(s.form_expanded.clone()),
                definition: Set(s.definition.clone()),
                needs_review: Set(s.needs_review || sub_ambiguous[i]),
            });
        }

        let has_typo = typo_lines
            .iter()
            .any(|(page, line)| *page == r.pdf_page && line.contains(&r.reading));
        // Matched by CONTENT rather than by page: a page holds many entries, and only the
        // one that actually swallowed the ambiguous line deserves the flag.
        let mut has_ambiguous = false;
        for (k, (page, text)) in ambiguous_lines.iter().enumerate() {
            if page.abs_diff(r.pdf_page) <= 1 && r.gloss.contains(text) {
                ambiguous_owned[k] = true;
                has_ambiguous = true;
            }
        }
        if has_ambiguous {
            ambiguous_matched += 1;
        }
        // This flag is about the HEADWORD LINE itself, not about sub-entries. A sub-entry
        // needing review carries its own flag and the UI marks that line. Merging the two
        // would warn on 517 entries, most with a perfectly sound head.
        let _ = sub_review;
        let needs_review = has_typo || has_ambiguous;
        if needs_review {
            needing_review += 1;
        }

        // A shape description for an entry whose glyph is an image. Only a SIGNED dossier row
        // is taken: the description is another edition's reading of the page, and it becomes
        // this edition's statement only once a person here has checked it and put their name
        // to it. An unsigned row stays in review/ and reaches no reader.
        let shape_note = match (glyph_ref, r.printed_page) {
            (None, Some(printed)) => signed_shape_notes
                .get(&(printed, r.reading.clone()))
                .cloned(),
            _ => None,
        };
        if shape_note.is_some() {
            shape_notes_applied += 1;
        }

        entries.push(entry::ActiveModel {
            id: Set(r.seq),
            glyph_id: Set(glyph_ref),
            page_id: Set(i32::from(r.pdf_page)),
            seq: Set(r.seq),
            slug: Set(slug.as_str().to_owned()),
            reading: Set(reading.as_str().to_owned()),
            reading_norm: Set(reading.normalized().as_str().to_owned()),
            alternate: Set(r.alternate.clone()),
            pos: Set(pos),
            gloss: Set(r.gloss.clone()),
            sub_text: Set(sub_text),
            inherits_glyph: Set(r.inherits_glyph),
            needs_review: Set(needs_review),
            shape_note: Set(shape_note),
        });
    }

    // Report how many ambiguous lines FOUND an owner. A gap against the total means the
    // rest sit elsewhere (in a sub-entry, or in a continuation of one) and are not flagged
    // yet — stated out loud rather than passed over.
    println!(
        "  shape descriptions taken from a SIGNED dossier row: {shape_notes_applied}/{}",
        dossiers.shape_notes.len()
    );
    println!(
        "  ambiguous lines matched into a gloss: {ambiguous_matched}/{}",
        ambiguous_lines.len()
    );
    let orphans: Vec<&str> = ambiguous_lines
        .iter()
        .zip(&ambiguous_owned)
        .filter(|(_, owned)| !**owned)
        .map(|((_, text), _)| *text)
        .collect();
    println!(
        "  ambiguous lines that found an owner: {}/{}",
        ambiguous_lines.len() - orphans.len(),
        ambiguous_lines.len()
    );
    if !orphans.is_empty() {
        // A line nobody owns carries no mark on the website, so a reader meets a passage the
        // dossier calls doubtful with nothing to say so. That is the one outcome this dossier
        // exists to prevent, so it stops the load rather than being noted and passed over.
        for t in &orphans {
            println!("    no owner: {t}");
        }
        bail!(
            "{} of {} lines in review/ambiguous-sub-entries.toml match no entry — either the \
             text drifted from the data, or they are being absorbed into a continuation and \
             shown to the reader unmarked",
            orphans.len(),
            ambiguous_lines.len()
        );
    }

    let _ = GlyphKind::ALL; // keep the link to the domain layer explicit.

    Ok(Plan {
        pages,
        front_matter,
        glyphs,
        entries,
        sub_entries,
        needing_review,
        image_only,
    })
}

// ── Loading ──────────────────────────────────────────────────────────────────

/// Load everything in **one transaction**.
///
/// Either the whole book goes in, or nothing changes. A half load that then fails is the
/// worst state: the site still runs, still returns results, and nobody knows part of it vanished.
async fn load(db: &Db, plan: &Plan) -> Result<()> {
    let txn = db
        .connection()
        .begin()
        .await
        .context("opening the transaction")?;

    // Delete in reverse foreign-key order. An unconditional `delete_many` builds
    // `DELETE FROM …` through the builder — still no raw SQL here.
    sub_entry::Entity::delete_many().exec(&txn).await?;
    entry::Entity::delete_many().exec(&txn).await?;
    front_matter::Entity::delete_many().exec(&txn).await?;
    glyph::Entity::delete_many().exec(&txn).await?;
    page::Entity::delete_many().exec(&txn).await?;

    // Batched because Postgres caps a statement at 65,535 parameters.
    macro_rules! insert_all {
        ($entity:path, $rows:expr, $what:literal) => {{
            let rows = $rows;
            let total = rows.len();
            for chunk in rows.chunks(CHUNK) {
                <$entity>::insert_many(chunk.to_vec())
                    .exec(&txn)
                    .await
                    .with_context(|| format!("loading {}", $what))?;
            }
            println!("  {:<10} {total:>6}", $what);
        }};
    }

    println!();
    println!("== Loading ==");
    insert_all!(page::Entity, plan.pages.clone(), "trang");
    insert_all!(glyph::Entity, plan.glyphs.clone(), "glyphs");
    insert_all!(entry::Entity, plan.entries.clone(), "entries");
    insert_all!(sub_entry::Entity, plan.sub_entries.clone(), "sub-entries");
    insert_all!(
        front_matter::Entity,
        plan.front_matter.clone(),
        "front matter"
    );

    txn.commit().await.context("committing the transaction")?;
    Ok(())
}
