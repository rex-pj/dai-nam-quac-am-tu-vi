//! The Quốc ngữ reading of an entry.
//!
//! A `NormalizedReading` can only be produced from a `Reading`, and a `Reading` can only be
//! built through `Reading::parse` — the single place that calls NFC. That makes NFC/NFD bugs
//! structurally impossible rather than something to remember to avoid.

use crate::error::DomainError;
use crate::model::letter::Letter;
use crate::text::normalize::{fold, nfc};

/// A reading as printed: NFC-normalized, trimmed, tone marks LEFT INTACT.
///
/// Tone marks are meaning-bearing here — Ả, Á and À are three different entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reading(String);

/// A reading with diacritics stripped and lowercased, for accent-insensitive lookup.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormalizedReading(String);

impl Reading {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let t = nfc(raw.trim());
        if t.is_empty() {
            return Err(DomainError::EmptyReading);
        }
        Ok(Self(t))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn normalized(&self) -> NormalizedReading {
        NormalizedReading(fold(&self.0))
    }

    pub fn initial(&self) -> Option<char> {
        self.0.chars().next()
    }

    /// The CHỮ section this reading belongs to.
    pub fn letter(&self) -> Result<Letter, DomainError> {
        match self.initial() {
            Some(c) => Letter::from_initial(c),
            None => Err(DomainError::EmptyReading),
        }
    }
}

impl NormalizedReading {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
