//! The exception dossiers in `review/` — read into typed data.
//!
//! The plan says: *every deviating line must either be fixed, or appear in an exception
//! dossier with a reason written by a human.* Until now the second half was only a promise —
//! each gate had a hand-typed "allowed" threshold, and hand-typed constants drift unnoticed.
//!
//! This module closes the loop: **the allowed threshold IS the number of dossier entries**.
//! Turning a gate green for a new deviation requires writing a dossier entry for it — there
//! is no shortcut, not even editing a number.
//!
//! One consequence worth stating: deleting a dossier entry also turns the gate red. That is
//! intentional — dossier and data must match in both directions.

use std::path::Path;

use serde::Deserialize;

use crate::ReportError;

/// The field shared by every dossier entry: who reviewed it.
///
/// Empty means **nobody has looked yet**. An unreviewed dossier still lets the gate pass —
/// it records that the deviation has been *seen and counted*, not yet that it has been
/// *adjudicated*. Unreviewed entries are counted separately in [`Dossiers::unverified`] and
/// shown publicly on the data-quality page.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Reviewed {
    #[serde(default)]
    pub verified_by: String,
}

/// The check names, used to match the `caught_by` field in a dossier.
pub mod caught_by {
    pub const CHECK_COVERAGE: &str = "check_coverage";
    pub const STRAY_LABEL: &str = "reading_has_stray_label";
}

impl Reviewed {
    pub fn is_verified(&self) -> bool {
        !self.verified_by.trim().is_empty()
    }
}

macro_rules! dossier_file {
    ($(#[$m:meta])* $name:ident, $wrapper:ident, $field:ident, $file:literal) => {
        $(#[$m])*
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
        pub struct $wrapper {
            #[serde(default)]
            pub $field: Vec<$name>,
        }

        impl $wrapper {
            pub const FILE: &'static str = $file;

            pub fn load(review_dir: &Path) -> Result<Self, ReportError> {
                read_toml(&review_dir.join($file))
            }

            pub fn len(&self) -> usize {
                self.$field.len()
            }

            pub fn is_empty(&self) -> bool {
                self.$field.is_empty()
            }

            pub fn unverified(&self) -> usize {
                self.$field.iter().filter(|e| !e.reviewed.is_verified()).count()
            }
        }
    };
}

/// A headword line whose label is missing its dot in the print.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LabelTypo {
    pub pdf_page: u16,
    pub line: String,
    pub issue: String,
    /// Which gate caught it. This field is what lets each gate threshold come from the dossier.
    pub caught_by: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// An entry where the print uses an image instead of a glyph.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImageGlyph {
    pub pdf_page: u16,
    pub reading: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// A sub-entry form still holding a placeholder that could not be derived.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ResidualPlaceholder {
    pub pdf_page: u16,
    pub form: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// A place where the text layer of the 2026 edition is missing a space.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct JammedText {
    pub pdf_page: u16,
    /// The run-together Latin string exactly as the text layer records it.
    pub found: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// A line with a placeholder but no italic — ambiguous between sub-entry and continuation.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AmbiguousLine {
    pub pdf_page: u16,
    pub text: String,
    /// The role the program assigned provisionally, pending a human decision.
    pub role_assumed: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

dossier_file!(
    /// `review/label-typos.toml`
    LabelTypo, LabelTypos, label_typo, "label-typos.toml"
);
dossier_file!(
    /// `review/image-glyphs.toml`
    ImageGlyph, ImageGlyphs, image_glyph, "image-glyphs.toml"
);
dossier_file!(
    /// `review/residual-placeholders.toml`
    ResidualPlaceholder,
    ResidualPlaceholders,
    residual_placeholder,
    "residual-placeholders.toml"
);
/// A glyph the 2026 edition sets as an image, for which the independent 1895 transcription
/// recorded a description of the shape instead of guessing a code point.
///
/// Only a row somebody has SIGNED is ever read into the database. An unsigned row is a
/// suggestion from another edition, and this project does not publish another edition claims
/// as its own.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ShapeNote {
    pub printed_page: u16,
    pub reading: String,
    /// The description, as the other edition wrote it.
    pub theirs: String,
    /// The `Trang:` page it was read from, so the claim can be traced and credited.
    pub witness_page: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

dossier_file!(
    /// `review/witness-shape-note-available.toml`
    ShapeNote,
    ShapeNotes,
    shape_note_available,
    "witness-shape-note-available.toml"
);
dossier_file!(
    /// `review/jammed-text.toml`
    JammedText, JammedTexts, jammed_text, "jammed-text.toml"
);
dossier_file!(
    /// `review/ambiguous-sub-entries.toml`
    AmbiguousLine,
    AmbiguousLines,
    ambiguous_line,
    "ambiguous-sub-entries.toml"
);

/// A glyph whose set of readings from the parser differs from the entry index.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ReadingDiff {
    pub glyph: String,
    /// The cause group. Four groups, explained at the top of the dossier file.
    pub kind: String,
    pub parser: String,
    pub index: String,
    #[serde(default)]
    pub note: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

dossier_file!(
    /// review/gate4-readings.toml
    ReadingDiff,
    ReadingDiffs,
    reading_diff,
    "gate4-readings.toml"
);

/// A glyph present in the entry index that the PDF text layer does not contain at all.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MissingGlyph {
    pub glyph: String,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// A label written with a comma `c,` instead of `c.`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CommaLabel {
    pub pdf_page: u16,
    #[serde(flatten)]
    pub reviewed: Reviewed,
}

/// `review/gate4-index.toml` — two kinds of entry in one file.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Gate4Index {
    #[serde(default)]
    pub missing_glyph: Vec<MissingGlyph>,
    #[serde(default)]
    pub comma_label: Vec<CommaLabel>,
}

impl Gate4Index {
    pub const FILE: &'static str = "gate4-index.toml";

    pub fn load(review_dir: &Path) -> Result<Self, ReportError> {
        read_toml(&review_dir.join(Self::FILE))
    }

    pub fn unverified(&self) -> usize {
        self.missing_glyph
            .iter()
            .filter(|e| !e.reviewed.is_verified())
            .count()
            + self
                .comma_label
                .iter()
                .filter(|e| !e.reviewed.is_verified())
                .count()
    }
}

/// Every dossier, loaded once.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dossiers {
    pub label_typos: LabelTypos,
    pub image_glyphs: ImageGlyphs,
    pub residual_placeholders: ResidualPlaceholders,
    pub shape_notes: ShapeNotes,
    pub jammed_texts: JammedTexts,
    pub ambiguous_lines: AmbiguousLines,
    pub gate4: Gate4Index,
    pub reading_diffs: ReadingDiffs,
}

impl Dossiers {
    pub fn load(review_dir: &Path) -> Result<Self, ReportError> {
        Ok(Self {
            label_typos: LabelTypos::load(review_dir)?,
            image_glyphs: ImageGlyphs::load(review_dir)?,
            residual_placeholders: ResidualPlaceholders::load(review_dir)?,
            shape_notes: ShapeNotes::load(review_dir)?,
            jammed_texts: JammedTexts::load(review_dir)?,
            ambiguous_lines: AmbiguousLines::load(review_dir)?,
            gate4: Gate4Index::load(review_dir)?,
            reading_diffs: ReadingDiffs::load(review_dir)?,
        })
    }

    /// The total number of entries nobody has signed off on.
    pub fn unverified(&self) -> usize {
        self.label_typos.unverified()
            + self.image_glyphs.unverified()
            + self.residual_placeholders.unverified()
            + self.shape_notes.unverified()
            + self.jammed_texts.unverified()
            + self.ambiguous_lines.unverified()
            + self.gate4.unverified()
            + self.reading_diffs.unverified()
    }

    pub fn total(&self) -> usize {
        self.label_typos.len()
            + self.image_glyphs.len()
            + self.residual_placeholders.len()
            + self.shape_notes.len()
            + self.jammed_texts.len()
            + self.ambiguous_lines.len()
            + self.gate4.missing_glyph.len()
            + self.gate4.comma_label.len()
            + self.reading_diffs.len()
    }
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ReportError> {
    let text = std::fs::read_to_string(path).map_err(|e| ReportError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    toml::from_str(&text).map_err(|e| ReportError::Decode(format!("{}: {e}", path.display())))
}

impl LabelTypos {
    /// The number of dossier entries the gate `which` caught.
    ///
    /// This is where each gate threshold is **taken from the dossier** rather than typed:
    /// gate ③ counts characters not yet in a field, and that count equals the number of
    /// dossier entries caught by `check_coverage`.
    pub fn caught_by(&self, which: &str) -> usize {
        self.label_typo
            .iter()
            .filter(|t| t.caught_by == which)
            .count()
    }
}

// ── Writing a dossier back out ───────────────────────────────────────────────

/// A dossier field value. The two cases are kept apart rather than guessed at from the text:
/// a form that happens to read as digits must still be written as a string, or the file will
/// no longer load back into the typed struct that reads it.
pub enum DossierValue {
    Text(String),
    Int(i64),
}

impl DossierValue {
    fn render(&self) -> String {
        match self {
            Self::Text(s) => toml_string(s),
            Self::Int(n) => n.to_string(),
        }
    }
}

impl From<String> for DossierValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for DossierValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<u16> for DossierValue {
    fn from(n: u16) -> Self {
        Self::Int(i64::from(n))
    }
}

/// One finding about to be written to a dossier.
pub struct DossierRow {
    /// A stable identity for this finding, written into the file as `key`.
    ///
    /// It is what lets a regenerated dossier keep the signatures already on it. Build it
    /// from what identifies the finding in the book — a page and the text — not from a row
    /// number, which shifts the moment anything above it changes.
    pub key: String,
    /// Field name and value, written in the given order. Values are written as strings.
    pub fields: Vec<(&'static str, DossierValue)>,
}

/// Write a dossier, **keeping the signatures already on it**.
///
/// A generated dossier that is rewritten wholesale on every run will sooner or later throw
/// away a `verified_by` somebody typed — the work of the one person the whole review process
/// depends on. So the old file is read first and every signature whose `key` comes round
/// again is carried over.
///
/// Returns how many signatures were carried. A signature whose finding has gone is dropped
/// along with it: the finding it vouched for no longer exists, and keeping it would let it
/// later attach to something nobody looked at.
pub fn write_dossier(
    path: &Path,
    table: &str,
    header: &str,
    rows: &[DossierRow],
) -> Result<usize, ReportError> {
    let mut signatures: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(path)
        && let Ok(parsed) = text.parse::<toml::Table>()
        && let Some(items) = parsed.get(table).and_then(toml::Value::as_array)
    {
        for item in items {
            let key = item.get("key").and_then(toml::Value::as_str);
            let by = item.get("verified_by").and_then(toml::Value::as_str);
            if let (Some(k), Some(b)) = (key, by)
                && !b.trim().is_empty()
            {
                signatures.insert(k.to_owned(), b.to_owned());
            }
        }
    }

    let mut out = String::with_capacity(rows.len() * 160 + header.len());
    out.push_str(header);
    if !header.ends_with('\n') {
        out.push('\n');
    }
    let mut kept = 0usize;
    for row in rows {
        out.push_str(&format!("\n[[{table}]]\n"));
        out.push_str(&format!("key          = {}\n", toml_string(&row.key)));
        for (name, value) in &row.fields {
            out.push_str(&format!("{name:<12} = {}\n", value.render()));
        }
        let by = signatures.get(&row.key).map_or("", String::as_str);
        if !by.is_empty() {
            kept += 1;
        }
        out.push_str(&format!("verified_by  = {}\n", toml_string(by)));
    }
    std::fs::write(path, out).map_err(|e| ReportError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    Ok(kept)
}

/// A TOML basic string. Escapes what the format requires, so a definition full of quotes and
/// backslashes cannot break the dossier it is written into.
pub fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
