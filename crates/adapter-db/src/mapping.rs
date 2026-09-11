//! The seam between the shape of the tables and the invariants of the domain.
//!
//! The **DB → domain** direction uses `TryFrom` and **is allowed to fail**. That is
//! intentional, not excess caution: a row violating an invariant (an empty reading, an empty
//! label list, a two-character glyph) must blow up right where it is read. If we "rescued" it
//! with a default here, a broken entry would travel on into the UI and a reader would see
//! something that is not in the book.
//!
//! The **domain → DB** direction does not exist in this crate: the web app is read-only, and
//! data reaches the store only through `pipeline/import`.

use dnqatv_core::model::{
    EntryDetail, EntryId, EntrySummary, GlyphChar, PdfPage, Pos, PosSet, Reading, Slug, SubEntry,
    SubEntryId,
};
use dnqatv_core::port::{FrontMatter, PageView, RepoError};
use dnqatv_entity::{entry, front_matter, glyph, page, sub_entry};

/// One `entry` row with the glyph character already joined in.
///
/// A dedicated type rather than a `(Model, Option<String>)` pair, so call sites cannot swap
/// the two by accident — with two values of the same `Option<String>` type the compiler cannot help.
pub struct EntryRow {
    pub entry: entry::Model,
    pub glyph_char: Option<String>,
}

impl TryFrom<EntryRow> for EntrySummary {
    type Error = RepoError;

    fn try_from(row: EntryRow) -> Result<Self, Self::Error> {
        let e = row.entry;
        Ok(Self {
            id: EntryId::new(e.id),
            slug: Slug::parse(&e.slug)?,
            glyph: match row.glyph_char {
                Some(c) => Some(GlyphChar::parse(&c)?),
                None => None,
            },
            reading: Reading::parse(&e.reading)?,
            alternate: match e.alternate {
                Some(a) => Some(Reading::parse(&a)?),
                None => None,
            },
            pos: PosSet::new(e.pos.into_iter().map(Pos::from).collect())?,
            gloss: e.gloss,
            pdf_page: PdfPage::new(u16::try_from(e.page_id).unwrap_or_default())?,
            needs_review: e.needs_review,
        })
    }
}

/// Why a function and not `TryFrom`: Rust orphan rules forbid implementing a foreign trait
/// for a foreign type. Only [`EntryRow`] — a type belonging to THIS crate — opens the
/// `TryFrom` door. The plan once promised `TryFrom<entity::Model> for core::Entry`; it turns
/// out that only works with a local intermediate type, and `EntryRow` is that type.
pub fn to_sub_entry(m: sub_entry::Model) -> Result<SubEntry, RepoError> {
    {
        Ok(SubEntry {
            id: SubEntryId::new(m.id),
            seq: u32::try_from(m.seq).unwrap_or_default(),
            han_form: m.han_form,
            han_expanded: m.han_expanded,
            form: m.form,
            form_expanded: m.form_expanded,
            definition: m.definition,
            needs_review: m.needs_review,
        })
    }
}

/// Join the head of an entry with its list of sub-entries.
pub fn to_detail(row: EntryRow, subs: Vec<sub_entry::Model>) -> Result<EntryDetail, RepoError> {
    let inherits_glyph = row.entry.inherits_glyph;
    let seq = u32::try_from(row.entry.seq).unwrap_or_default();
    let shape_note = row.entry.shape_note.clone();
    let summary = EntrySummary::try_from(row)?;
    let sub_entries = subs
        .into_iter()
        .map(to_sub_entry)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EntryDetail {
        summary,
        sub_entries,
        inherits_glyph,
        shape_note,
        seq,
    })
}

pub fn to_page_view(m: page::Model) -> Result<PageView, RepoError> {
    {
        let pdf_page = PdfPage::new(u16::try_from(m.pdf_page).unwrap_or_default())?;
        Ok(PageView {
            pdf_page,
            // `printed_page` is not read from the column: the relation between the two page
            // numbering systems is a pure domain function, and letting the domain compute it
            // keeps column and function from ever diverging.
            printed_page: pdf_page.printed(),
            letter: m.letter.map(Into::into),
            image_path: m.image_path,
            head_first: m.head_first,
            head_last: m.head_last,
        })
    }
}

pub fn to_front_matter(m: front_matter::Model) -> Result<FrontMatter, RepoError> {
    {
        Ok(FrontMatter {
            slug: Slug::parse(&m.slug)?,
            title: m.title,
            body: m.body,
            pdf_page: PdfPage::new(u16::try_from(m.pdf_page).unwrap_or_default())?,
        })
    }
}

/// The glyph character, verified to be exactly one character.
pub fn glyph_char(m: &glyph::Model) -> Result<GlyphChar, RepoError> {
    Ok(GlyphChar::parse(&m.char)?)
}
