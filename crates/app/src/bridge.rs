//! The 1895 ↔ modern orthography bridge — **it suggests, it never rewrites**.
//!
//! This is the easiest place in the whole application to corrupt data, so the rule is
//! encoded in the types: the one function this module offers returns [`Suggestion`], and no
//! function returns a corrected query. Rewriting the query would require changing the API,
//! and changing the API means reading this passage again.
//!
//! The reason is a measurement on the book itself: `quấc` and `quốc`, `sanh` and `sinh`,
//! `chánh` and `chính` **are all separate entries, each with its own definition**. Merging
//! them would blend the meanings of two different entries.

use std::path::Path;

use dnqatv_core::text::fold;
use serde::Deserialize;

/// One documented variant pair.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Pair {
    /// The 1895 spelling.
    pub old: String,
    /// The modern spelling.
    pub new: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub verified_by: String,
}

impl Pair {
    pub fn is_verified(&self) -> bool {
        !self.verified_by.trim().is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Bridge {
    #[serde(default)]
    pub pair: Vec<Pair>,
}

/// A suggestion for the user to **click themselves**.
///
/// No field carries a "corrected query": this type deliberately cannot express a rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The spelling to offer as a further lookup.
    pub alternative: String,
    /// A short explanation, shown next to the link.
    pub reason: &'static str,
    /// Whether a human has checked it against the print — the UI says so honestly.
    pub verified: bool,
}

/// The reasons, written once so no strings are scattered through the handlers.
const REASON_OLD_TO_NEW: &str = "sách in năm 1895 theo chính tả Nam Kỳ đương thời";
const REASON_NEW_TO_OLD: &str = "dạng chính tả 1895 của cùng âm";

impl Bridge {
    pub const FILE: &'static str = "orthography-bridge.toml";

    pub fn load(review_dir: &Path) -> Result<Self, BridgeError> {
        let path = review_dir.join(Self::FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| BridgeError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        toml::from_str(&text).map_err(|e| BridgeError::Decode(e.to_string()))
    }

    pub fn len(&self) -> usize {
        self.pair.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pair.is_empty()
    }

    pub fn unverified(&self) -> usize {
        self.pair.iter().filter(|p| !p.is_verified()).count()
    }

    /// Suggestions for one query.
    ///
    /// Matching is done on the accent-folded form so `Quấc`, `quấc` and `QUẤC` all find the
    /// pair; but what comes back is the **form as recorded**, not the folded one — a user
    /// needs a readable word, not a lookup key.
    ///
    /// Both directions match: a reader may type the modern spelling and need the 1895 one, or the reverse.
    pub fn suggest(&self, query: &str) -> Vec<Suggestion> {
        let key = fold(query.trim());
        if key.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for p in &self.pair {
            if fold(&p.old) == key {
                out.push(Suggestion {
                    alternative: p.new.clone(),
                    reason: REASON_OLD_TO_NEW,
                    verified: p.is_verified(),
                });
            } else if fold(&p.new) == key {
                out.push(Suggestion {
                    alternative: p.old.clone(),
                    reason: REASON_NEW_TO_OLD,
                    verified: p.is_verified(),
                });
            }
        }
        out
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("could not read {path}: {message}")]
    Io { path: String, message: String },
    #[error("the orthography bridge file is malformed: {0}")]
    Decode(String),
}
