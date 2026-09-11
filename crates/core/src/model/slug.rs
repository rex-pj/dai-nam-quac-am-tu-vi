//! An entry's URL identifier.
//!
//! A slug is **derived data**, not the book's words — it exists only to give each entry a
//! stable web address. Its generation rule must therefore be a pure, named, testable and
//! **deterministic** function: the same list of entries must yield the same set of slugs on
//! every run, otherwise every data reload rewrites every URL on the site.
//!
//! Han-Nom characters stay out of slugs: 𨰲 cannot be typed, and most clients cannot even
//! render it in the address bar. Slugs follow the accent-folded Quốc ngữ reading.

use crate::error::DomainError;
use crate::model::reading::Reading;

/// Separator character inside a slug.
const SEP: char = '-';

/// A URL-safe string: only `a-z`, `0-9` and `-`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    /// Accept an existing slug (from a URL or from the DB), rejecting anything outside the
    /// allowed character set.
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let t = raw.trim();
        if t.is_empty() {
            return Err(DomainError::EmptySlug);
        }
        if !t
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == SEP)
        {
            return Err(DomainError::InvalidSlug(t.to_owned()));
        }
        Ok(Self(t.to_owned()))
    }

    /// The slug stem, derived from the accent-folded reading.
    ///
    /// Not unique yet — many entries share a reading (`Lõm` occurs once, but `A` many
    /// times). Disambiguation is [`SlugMinter`]'s job.
    pub fn stem(reading: &Reading) -> Result<Self, DomainError> {
        let folded = reading.normalized();
        let mut out = String::with_capacity(folded.as_str().len());
        let mut last_was_sep = true; // blocks a leading '-'
        for c in folded.as_str().chars() {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                out.push(c);
                last_was_sep = false;
            } else if !last_was_sep {
                out.push(SEP);
                last_was_sep = true;
            }
        }
        while out.ends_with(SEP) {
            out.pop();
        }
        if out.is_empty() {
            return Err(DomainError::InvalidSlug(reading.as_str().to_owned()));
        }
        Ok(Self(out))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Slug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Mints unique slugs in book order.
///
/// Rule: the first entry carrying a given reading keeps the bare stem; later ones get
/// `-2`, `-3`, … appended **in the order they appear in the book**. Book order is invariant
/// (checked by gate ⑤), so slugs are invariant too — the precondition for URLs surviving
/// a data reload unchanged.
#[derive(Debug, Default)]
pub struct SlugMinter {
    seen: std::collections::BTreeMap<String, u32>,
}

impl SlugMinter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mint(&mut self, reading: &Reading) -> Result<Slug, DomainError> {
        let stem = Slug::stem(reading)?;
        let count = self.seen.entry(stem.0.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            Ok(stem)
        } else {
            Slug::parse(&format!("{}{SEP}{}", stem.0, count))
        }
    }
}
