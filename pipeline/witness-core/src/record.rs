//! Turning one transcribed page into headwords with their sub-entries attached.
//!
//! The two templates are siblings in the page text, not nested: a `DNQATV/nghĩa` call
//! belongs to the nearest `DNQATV/mục` **above** it. So the two lists are merged by byte
//! offset and walked in document order — the order the page itself is read in.

use crate::template::template_fields;

const ENTRY: &str = "DNQATV/mục";
const SUB: &str = "DNQATV/nghĩa";

/// One sub-entry as the other edition set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessSub {
    /// The Han column, empty when the line has none.
    pub han: String,
    /// The Quốc ngữ form.
    pub form: String,
    pub definition: String,
}

/// One headword as the other edition set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessEntry {
    /// The glyph field **verbatim**. It is not always a character: where the 1895 print used
    /// a shape with no code point, the transcriber wrote `{{?|…}}`, a Private Use code point,
    /// or an Ideographic Description Sequence. [`WitnessEntry::unicode_glyph`] tells them apart.
    pub glyph_field: String,
    pub reading: String,
    pub alternate: String,
    pub label: String,
    pub gloss: String,
    pub subs: Vec<WitnessSub>,
    /// Page title in the `Trang:` namespace, so a finding can be linked back.
    pub page: String,
    /// 1 not proofread · 3 proofread · 4 validated. A level-1 page is a weaker witness.
    pub quality: Option<u8>,
}

impl WitnessEntry {
    /// The glyph as a single Unicode character, or `None` when this edition could not
    /// encode it either.
    ///
    /// Measured over the snapshot: 7,585 of 8,077 headwords carry a plain character; 60 hold
    /// a description, a Private Use code point or an IDS; the rest are empty. Of **our** 29
    /// image-only entries, 28 are unencodable here too — two independent transcriptions
    /// hitting the same wall, which is the strongest evidence available that the 2026
    /// editors were right to set an image rather than pick something close.
    pub fn unicode_glyph(&self) -> Option<char> {
        let g = self.glyph_field.trim();
        if g.contains("{{") {
            return None;
        }
        let mut chars = g.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None; // an IDS or a juxtaposition, not one character
        }
        if is_private_use(c) || is_ids(c) {
            return None;
        }
        Some(c)
    }

    /// How this edition recorded a glyph it could not encode — the `{{?|…}}` note, verbatim.
    /// Worth keeping: it describes the shape by its parts instead of guessing a code point.
    pub fn shape_note(&self) -> Option<&str> {
        let g = self.glyph_field.trim();
        let open = g.find("{{")?;
        let close = g[open..].find("}}")? + open;
        let inner = g.get(open + 2..close)?;
        let note = inner.split_once('|').map_or(inner, |(_, rest)| rest).trim();
        (!note.is_empty()).then_some(note)
    }
}

const fn is_private_use(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
}

const fn is_ids(c: char) -> bool {
    matches!(c as u32, 0x2FF0..=0x2FFF)
}

/// Read one page of wikitext into headwords, each carrying the sub-entries beneath it.
///
/// Sub-entries appearing before the first headword on the page are dropped: they are the
/// tail of an entry that began on the previous page, and attaching them to the wrong
/// headword would invent a relationship the page does not state.
pub fn read_page(wikitext: &str, page: &str, quality: Option<u8>) -> Vec<WitnessEntry> {
    let mut marks: Vec<(usize, bool, Vec<String>)> = Vec::new();
    for t in template_fields(wikitext, ENTRY) {
        marks.push((t.at, true, t.fields));
    }
    for t in template_fields(wikitext, SUB) {
        marks.push((t.at, false, t.fields));
    }
    marks.sort_by_key(|(at, _, _)| *at);

    // An optional template parameter that was not written is an empty field — that is what
    // the wiki template itself means by it, not a value this step is inventing.
    let f = |fields: &[String], n: usize| match fields.get(n) {
        Some(s) => s.trim().to_owned(),
        None => String::new(),
    };

    let mut out: Vec<WitnessEntry> = Vec::new();
    for (_, is_entry, fields) in marks {
        if is_entry {
            out.push(WitnessEntry {
                glyph_field: f(&fields, 0),
                reading: f(&fields, 1),
                alternate: f(&fields, 2),
                label: f(&fields, 3),
                gloss: f(&fields, 4),
                subs: Vec::new(),
                page: page.to_owned(),
                quality,
            });
        } else if let Some(last) = out.last_mut() {
            last.subs.push(WitnessSub {
                han: f(&fields, 0),
                form: f(&fields, 1),
                definition: f(&fields, 2),
            });
        }
    }
    out
}
