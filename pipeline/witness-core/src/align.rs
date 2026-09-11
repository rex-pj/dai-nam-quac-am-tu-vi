//! Lining the two editions up, and naming the ways they differ.
//!
//! Both lists run in the collation order of the book, so a two-pointer walk with a short
//! lookahead is enough — no scoring matrix, no threshold to tune. Where the walk cannot
//! re-synchronise it says so and moves on, rather than forcing a pairing.
//!
//! The kinds of divergence are kept apart on purpose. Most of them are the two editions
//! being two editions and mean nothing is wrong; exactly one, [`DivergenceKind::ColumnBoundary`],
//! says the other edition read the page differently from us, and that one is worth answering.

use crate::record::WitnessEntry;
use crate::text::fold;

/// One sub-entry as this project parsed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OurSub {
    /// The parser could not settle this line on its own and flagged it for a person.
    pub needs_review: bool,
    /// `None` when the line has no Han column — most sub-entries have none. Absent and
    /// empty are kept apart: an empty string would be a value this step invented.
    pub han_form: Option<String>,
    pub form: String,
    pub definition: String,
}

/// One headword as this project parsed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OurEntry {
    /// A dossier row says something on this headword line is unsettled, and a person must
    /// look. Set by the shell from review/, the same way `import` sets it.
    pub needs_review: bool,
    pub seq: u32,
    pub pdf_page: u16,
    pub printed_page: u16,
    /// `None` where the 2026 edition set an image instead of a character.
    pub glyph: Option<char>,
    pub reading: String,
    /// The main definition, kept so a flagged headword line can be shown beside the other
    /// edition reading of the same entry.
    pub gloss: String,
    pub subs: Vec<OurSub>,
}

/// What sort of difference was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DivergenceKind {
    /// The other edition places the Han / Quốc ngữ boundary elsewhere. **The sharp one.**
    ColumnBoundary,
    /// Both editions encode a glyph for this headword, and the characters differ.
    GlyphDiffers,
    /// We assign a character where the other edition could not encode one. Worth a look:
    /// it may mean our source guessed.
    GlyphUnencodableThere,
    /// We have no character (the 2026 edition set an image) and the other edition recorded
    /// a description of the shape. Not a fault — an opportunity.
    ShapeNoteAvailable,
    /// A headword the other edition has and we do not.
    EntryOnlyThere,
    /// A headword we have and the other edition does not.
    EntryOnlyHere,
    /// Our parser flagged this sub-entry for a person, and the other edition has the line
    /// too. Not a difference — a second reading placed beside ours so the reviewer can
    /// confirm rather than go and look it up.
    FlaggedHere,
}

impl DivergenceKind {
    /// Whether this kind is a claim that we misread the page, as opposed to the two
    /// editions simply being different books.
    pub const fn is_contradiction(self) -> bool {
        matches!(self, Self::ColumnBoundary | Self::GlyphDiffers)
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::ColumnBoundary => "column_boundary",
            Self::GlyphDiffers => "glyph_differs",
            Self::GlyphUnencodableThere => "glyph_unencodable_there",
            Self::ShapeNoteAvailable => "shape_note_available",
            Self::EntryOnlyThere => "entry_only_there",
            Self::EntryOnlyHere => "entry_only_here",
            Self::FlaggedHere => "flagged_here",
        }
    }
}

/// One finding, written out for a person to settle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub kind: DivergenceKind,
    pub printed_page: u16,
    pub reading: String,
    /// What this project has.
    pub ours: String,
    /// What the other edition has.
    pub theirs: String,
    /// The `Trang:` page it was seen on, and that page's proofreading level.
    pub witness_page: String,
    pub witness_quality: Option<u8>,
}

/// The tallies of a comparison run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Alignment {
    pub entries_ours: usize,
    pub entries_theirs: usize,
    pub matched: usize,
    pub subs_compared: usize,
    pub subs_no_partner: usize,
    pub divergences: Vec<Divergence>,
}

impl Alignment {
    pub fn count(&self, kind: DivergenceKind) -> usize {
        self.divergences.iter().filter(|d| d.kind == kind).count()
    }

    /// The number that has to be answered for: differences that claim we misread the page.
    pub fn contradictions(&self) -> usize {
        self.divergences
            .iter()
            .filter(|d| d.kind.is_contradiction())
            .count()
    }
}

/// How far ahead to look for the lists to fall back into step. The 1895 *Bổ di* section
/// adds entries in runs; twelve covers every run seen without pairing unrelated headwords.
const LOOKAHEAD: usize = 12;

/// Compare the two editions.
pub fn compare(ours: &[OurEntry], theirs: &[WitnessEntry]) -> Alignment {
    let mut out = Alignment {
        entries_ours: ours.len(),
        entries_theirs: theirs.len(),
        ..Alignment::default()
    };

    let (mut i, mut j) = (0usize, 0usize);
    while i < ours.len() && j < theirs.len() {
        if fold(&ours[i].reading) == fold(&theirs[j].reading) {
            // Take the whole RUN of headwords sharing this reading, not just one. The book
            // has several 班/搬 "Ban" in a row and the two editions do not always set them in
            // the same order; pairing position-by-position inside a run reported the same two
            // glyphs swapped as two glyph faults.
            let ni = run_end(ours, i, |e| &e.reading);
            let nj = run_end(theirs, j, |e| &e.reading);
            let run = pair_run(&ours[i..ni], &theirs[j..nj]);
            for (o, t) in run.pairs {
                compare_entry(o, t, &mut out);
                out.matched += 1;
            }
            // A run of unequal length leaves entries over. They are reported, never dropped:
            // an entry that quietly vanished from the comparison is the one failure this
            // whole step exists to prevent.
            for o in run.only_ours {
                out.divergences.push(only_here(o));
            }
            for t in run.only_theirs {
                out.divergences.push(only_there(t));
            }
            i = ni;
            j = nj;
            continue;
        }
        match resync(ours, theirs, i, j) {
            Some((Side::Theirs, d)) => {
                for t in &theirs[j..j + d] {
                    out.divergences.push(only_there(t));
                }
                j += d;
            }
            Some((Side::Ours, d)) => {
                for o in &ours[i..i + d] {
                    out.divergences.push(only_here(o));
                }
                i += d;
            }
            // Neither side re-synchronises nearby: report both and step past.
            None => {
                out.divergences.push(only_here(&ours[i]));
                out.divergences.push(only_there(&theirs[j]));
                i += 1;
                j += 1;
            }
        }
    }
    for o in &ours[i..] {
        out.divergences.push(only_here(o));
    }
    for t in &theirs[j..] {
        out.divergences.push(only_there(t));
    }
    out
}

enum Side {
    Ours,
    Theirs,
}

/// The end of the run of consecutive entries sharing the reading at `start`.
fn run_end<T>(items: &[T], start: usize, reading: impl Fn(&T) -> &String) -> usize {
    let key = fold(reading(&items[start]));
    let mut end = start + 1;
    while end < items.len() && fold(reading(&items[end])) == key {
        end += 1;
    }
    end
}

/// The result of pairing one run: matched entries, plus whatever each side had left over.
struct RunPairing<'a> {
    pairs: Vec<(&'a OurEntry, &'a WitnessEntry)>,
    only_ours: Vec<&'a OurEntry>,
    only_theirs: Vec<&'a WitnessEntry>,
}

/// Pair up two runs of same-reading headwords.
///
/// Glyph agreement comes first: if both sides carry the same character, that is the pairing,
/// whatever order the two editions set them in. Only what is left over is paired by position,
/// and only those leftovers can ever be reported as a glyph difference — so a run the two
/// editions merely ordered differently produces no finding at all.
///
/// Every entry of both runs comes back in exactly one of the three lists.
fn pair_run<'a>(ours: &'a [OurEntry], theirs: &'a [WitnessEntry]) -> RunPairing<'a> {
    let mut taken = vec![false; theirs.len()];
    let mut partner: Vec<Option<usize>> = vec![None; ours.len()];

    for (a, o) in ours.iter().enumerate() {
        let Some(g) = o.glyph else { continue };
        if let Some(b) = theirs
            .iter()
            .enumerate()
            .position(|(b, t)| !taken[b] && t.unicode_glyph() == Some(g))
        {
            taken[b] = true;
            partner[a] = Some(b);
        }
    }

    let mut spare = (0..theirs.len())
        .filter(|b| !taken[*b])
        .collect::<Vec<_>>()
        .into_iter();
    for slot in partner.iter_mut().filter(|p| p.is_none()) {
        let Some(b) = spare.next() else { break };
        taken[b] = true;
        *slot = Some(b);
    }

    let mut out = RunPairing {
        pairs: Vec::new(),
        only_ours: Vec::new(),
        only_theirs: Vec::new(),
    };
    let mut ordered: Vec<(usize, usize)> = Vec::new();
    for (a, p) in partner.iter().enumerate() {
        match p {
            Some(b) => ordered.push((a, *b)),
            None => out.only_ours.push(&ours[a]),
        }
    }
    ordered.sort_unstable();
    out.pairs = ordered
        .into_iter()
        .map(|(a, b)| (&ours[a], &theirs[b]))
        .collect();
    out.only_theirs = theirs
        .iter()
        .enumerate()
        .filter(|(b, _)| !taken[*b])
        .map(|(_, t)| t)
        .collect();
    out
}

/// How many entries to skip, on which side, for the two lists to agree again.
fn resync(ours: &[OurEntry], theirs: &[WitnessEntry], i: usize, j: usize) -> Option<(Side, usize)> {
    let here = fold(&ours[i].reading);
    let there = fold(&theirs[j].reading);
    for d in 1..=LOOKAHEAD {
        if theirs.get(j + d).is_some_and(|t| fold(&t.reading) == here) {
            return Some((Side::Theirs, d));
        }
        if ours.get(i + d).is_some_and(|o| fold(&o.reading) == there) {
            return Some((Side::Ours, d));
        }
    }
    None
}

fn only_here(o: &OurEntry) -> Divergence {
    Divergence {
        kind: DivergenceKind::EntryOnlyHere,
        printed_page: o.printed_page,
        reading: o.reading.clone(),
        // No glyph is not a missing value: the 2026 edition set an image in its place.
        ours: match o.glyph {
            Some(c) => c.to_string(),
            None => String::new(),
        },
        theirs: String::new(),
        witness_page: String::new(),
        witness_quality: None,
    }
}

fn only_there(t: &WitnessEntry) -> Divergence {
    Divergence {
        kind: DivergenceKind::EntryOnlyThere,
        printed_page: 0,
        reading: t.reading.clone(),
        ours: String::new(),
        theirs: t.glyph_field.clone(),
        witness_page: t.page.clone(),
        witness_quality: t.quality,
    }
}

fn compare_entry(o: &OurEntry, t: &WitnessEntry, out: &mut Alignment) {
    let note = |kind, ours: String, theirs: String| Divergence {
        kind,
        printed_page: o.printed_page,
        reading: o.reading.clone(),
        ours,
        theirs,
        witness_page: t.page.clone(),
        witness_quality: t.quality,
    };

    // The headword line itself is flagged and the other edition has this entry. Put the two
    // glosses side by side: where they read the same, the reviewer can see at a glance that
    // the line our parser could not place is set the same way in the 1895 print too.
    if o.needs_review && !t.gloss.trim().is_empty() {
        out.divergences.push(note(
            DivergenceKind::FlaggedHere,
            o.gloss.clone(),
            crate::text::plain(&t.gloss),
        ));
    }

    match (o.glyph, t.unicode_glyph()) {
        (Some(a), Some(b)) if a != b => {
            out.divergences
                .push(note(DivergenceKind::GlyphDiffers, a.into(), b.into()));
        }
        (Some(a), None) if !t.glyph_field.is_empty() => {
            let theirs = match t.shape_note() {
                Some(note) => note.to_owned(),
                None => t.glyph_field.clone(),
            };
            out.divergences.push(note(
                DivergenceKind::GlyphUnencodableThere,
                a.into(),
                theirs,
            ));
        }
        (None, _) => {
            if let Some(n) = t.shape_note() {
                out.divergences.push(note(
                    DivergenceKind::ShapeNoteAvailable,
                    String::new(),
                    n.to_owned(),
                ));
            } else if let Some(b) = t.unicode_glyph() {
                out.divergences.push(note(
                    DivergenceKind::ShapeNoteAvailable,
                    String::new(),
                    b.into(),
                ));
            }
        }
        _ => {}
    }

    for s in &o.subs {
        let Some(partner) = find_partner(s, &o.subs, &t.subs) else {
            out.subs_no_partner += 1;
            continue;
        };
        out.subs_compared += 1;
        // Compare with the LOOSE key. The question is which column a piece of text sits in,
        // and that survives the two editions disagreeing about a closing full stop or about
        // capitalising the first word — `fold` drops both, `placeholder_key` would report
        // 220 differences of which none was a column break at all.
        if fold(&s.form) != fold(&partner.form) {
            // `¦` marks where the Han column ends and the Quốc ngữ form begins — the very
            // thing in dispute — so a line with no Han column shows nothing before it.
            let ours = match &s.han_form {
                Some(h) => format!("{h} ¦ {}", s.form),
                None => format!("¦ {}", s.form),
            };
            let theirs = if partner.han.is_empty() {
                format!("¦ {}", partner.form)
            } else {
                format!("{} ¦ {}", partner.han, partner.form)
            };
            out.divergences
                .push(note(DivergenceKind::ColumnBoundary, ours, theirs));
        } else if s.needs_review {
            // The parser could not settle this line, and the other edition has it. Recording
            // both readings side by side turns a reviewer's errand — find the 1895 page, find
            // the line — into a glance. It is not a verdict: the other edition agreeing is an
            // editor's judgement, not a rule of the book, so the flag stays until a person
            // here signs for it.
            let ours = match &s.han_form {
                Some(h) => format!("{h} ¦ {}", s.form),
                None => format!("¦ {}", s.form),
            };
            let theirs = if partner.han.is_empty() {
                format!("¦ {}", partner.form)
            } else {
                format!("{} ¦ {}", partner.han, partner.form)
            };
            out.divergences
                .push(note(DivergenceKind::FlaggedHere, ours, theirs));
        }
    }
}

/// A definition shorter than this is not distinctive enough to identify a line. The book is
/// full of one-word definitions — `id.`, `Chậu.` — and pairing on those matched two different
/// sub-entries of ours to one of theirs, then reported the mismatch as a column break.
const DISTINCTIVE: usize = 12;

/// The same sub-entry in the other edition: by definition first, since a definition is long
/// and distinctive, then by form.
///
/// The definition must be unique on **both** sides. Unique only on theirs is not enough: if
/// two of our lines share a definition, either could claim the same partner and at most one
/// of them would be right.
///
/// No fuzzy scoring anywhere. An uncertain pairing is reported as no pairing, counted in
/// `subs_no_partner` and shown in the run — not quietly resolved to the nearest thing.
fn find_partner<'a>(
    s: &OurSub,
    mine: &[OurSub],
    theirs: &'a [crate::record::WitnessSub],
) -> Option<&'a crate::record::WitnessSub> {
    let def = fold(&s.definition);
    if def.chars().count() >= DISTINCTIVE
        && mine.iter().filter(|m| fold(&m.definition) == def).count() == 1
    {
        let mut hits = theirs.iter().filter(|t| fold(&t.definition) == def);
        if let Some(first) = hits.next()
            && hits.next().is_none()
        {
            return Some(first);
        }
    }
    let form = fold(&s.form);
    if mine.iter().filter(|m| fold(&m.form) == form).count() != 1 {
        return None;
    }
    let mut hits = theirs.iter().filter(|t| fold(&t.form) == form);
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}
