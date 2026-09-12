//! The results of the five gates, in machine-readable form.
//!
//! Until now the gates only printed to the screen for a human. That is enough to *know*
//! whether the data is clean, but not enough to **stop** a bad load: one person running
//! `import` without running `reconcile` sends unchecked data straight into the database.
//!
//! This file closes that door. `reconcile` writes `data/gates.json`; `import` **refuses to
//! load** if the file is missing, if any gate is red, or if the report was produced from a
//! different `entries.jsonl` than the one about to be loaded.
//!
//! The last detail is the most important one: a green but stale report is **more dangerous**
//! than no report, because it creates false confidence. So the report carries a fingerprint
//! of the very data file it checked.

pub mod dossier;

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The report format version.
pub const REPORT_VERSION: u32 = 1;

/// The gates of the plan. An enum so no gate can be silently forgotten.
///
/// There is no `⑥` here on purpose. Gate ⑥ is the witness step, and it deliberately writes
/// no result: the 1895 and 2026 editions genuinely differ, so a pass/fail number would either
/// cry wolf or invite somebody to soften it. It sorts its findings into `review/` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Gate {
    /// ① Glyph codes not found in the `/ToUnicode` CMap.
    UnmappedGlyphs,
    /// ② Full field coverage of the headword line.
    HeadwordCoverage,
    /// ③ Character conservation.
    CharacterConservation,
    /// ④ Reconciliation against the entry index.
    IndexReconciliation,
    /// ⑤ The ordering invariant under the collation of the book.
    CollationOrder,
    /// ⑦ The corrections the print prints about itself.
    PrintedErrata,
    /// ⑧ Readings settled against the scan still point at something.
    ScanVerified,
}

impl Gate {
    pub const ALL: [Gate; 7] = [
        Self::UnmappedGlyphs,
        Self::HeadwordCoverage,
        Self::CharacterConservation,
        Self::IndexReconciliation,
        Self::CollationOrder,
        Self::PrintedErrata,
        Self::ScanVerified,
    ];

    pub const fn number(self) -> u8 {
        match self {
            Self::UnmappedGlyphs => 1,
            Self::HeadwordCoverage => 2,
            Self::CharacterConservation => 3,
            Self::IndexReconciliation => 4,
            Self::CollationOrder => 5,
            Self::PrintedErrata => 7,
            Self::ScanVerified => 8,
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::UnmappedGlyphs => "Glyph decoding with no fallback",
            Self::HeadwordCoverage => "Headword lines fully covered",
            Self::CharacterConservation => "Character conservation",
            Self::IndexReconciliation => "Reconciliation against the entry index",
            Self::CollationOrder => "Ordering invariant under the book collation",
            Self::PrintedErrata => "The errata the print carries about itself",
            Self::ScanVerified => "Readings settled against the 1895 scan",
        }
    }
}

/// The status of one gate.
///
/// There is no "warning" level. For a dictionary a gate either passes or needs a human —
/// an intermediate level only trains people to get used to yellow and stop looking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    /// Pass: the deviation equals exactly the accepted number.
    Green,
    /// Fail: unexplained deviation remains.
    Red,
}

impl GateStatus {
    pub const fn is_green(self) -> bool {
        matches!(self, Self::Green)
    }
}

/// The result of one gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: Gate,
    pub status: GateStatus,
    /// The measured number — whatever this gate counts.
    pub measured: i64,
    /// The permitted number. Equal to `measured` means green.
    pub allowed: i64,
    /// The explanation, written for a reader.
    pub note: String,
}

impl GateResult {
    /// A gate passes when the measured number equals the **explained and accepted** threshold.
    ///
    /// `<=` is deliberately not used: every unit of deviation must have a reason, so "fewer
    /// than expected" is also a signal that something changed and nobody noticed.
    pub fn judge(gate: Gate, measured: i64, allowed: i64, note: impl Into<String>) -> Self {
        Self {
            gate,
            status: if measured == allowed {
                GateStatus::Green
            } else {
                GateStatus::Red
            },
            measured,
            allowed,
            note: note.into(),
        }
    }
}

/// The fingerprint of the data file that was checked.
///
/// FNV-1a rather than `DefaultHasher`: `DefaultHasher` promises no stability across Rust
/// releases, and this report must be comparable with one produced on another machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub bytes: u64,
    pub lines: u64,
    pub fnv1a: u64,
}

impl Fingerprint {
    pub fn of(data: &[u8]) -> Self {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET;
        let mut lines = 0u64;
        for &b in data {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(PRIME);
            if b == b'\n' {
                lines += 1;
            }
        }
        Self {
            bytes: data.len() as u64,
            lines,
            fnv1a: hash,
        }
    }

    pub fn of_file(path: &Path) -> Result<Self, ReportError> {
        let data = std::fs::read(path).map_err(|e| ReportError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        Ok(Self::of(&data))
    }
}

/// The complete report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub version: u32,
    /// The data file checked, as a path relative to the workspace root.
    pub source: String,
    pub fingerprint: Fingerprint,
    pub results: Vec<GateResult>,
}

impl GateReport {
    pub fn new(
        source: impl Into<String>,
        fingerprint: Fingerprint,
        results: Vec<GateResult>,
    ) -> Self {
        Self {
            version: REPORT_VERSION,
            source: source.into(),
            fingerprint,
            results,
        }
    }

    pub fn all_green(&self) -> bool {
        self.missing_gates().is_empty() && self.results.iter().all(|r| r.status.is_green())
    }

    /// Gates present in [`Gate::ALL`] but absent from the report.
    ///
    /// Absence must count as **failure**, not as "not applicable": a gate silently dropped is
    /// exactly the failure mode this whole mechanism exists to prevent.
    pub fn missing_gates(&self) -> Vec<Gate> {
        Gate::ALL
            .into_iter()
            .filter(|g| !self.results.iter().any(|r| r.gate == *g))
            .collect()
    }

    pub fn red_gates(&self) -> Vec<&GateResult> {
        self.results
            .iter()
            .filter(|r| !r.status.is_green())
            .collect()
    }

    pub fn write(&self, path: &Path) -> Result<(), ReportError> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| ReportError::Encode(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| ReportError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    }

    pub fn read(path: &Path) -> Result<Self, ReportError> {
        let text = std::fs::read_to_string(path).map_err(|e| ReportError::Io {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        let report: Self =
            serde_json::from_str(&text).map_err(|e| ReportError::Decode(e.to_string()))?;
        if report.version != REPORT_VERSION {
            return Err(ReportError::WrongVersion {
                found: report.version,
                expected: REPORT_VERSION,
            });
        }
        Ok(report)
    }

    /// The single entrance for `import`: the report must exist, cover every gate, be all
    /// green, **and** be the report of the very data file about to be loaded.
    pub fn authorize(&self, data_fingerprint: Fingerprint) -> Result<(), ReportError> {
        let missing = self.missing_gates();
        if !missing.is_empty() {
            return Err(ReportError::GatesMissing(
                missing.iter().map(|g| g.number()).collect(),
            ));
        }
        if self.fingerprint != data_fingerprint {
            return Err(ReportError::StaleReport);
        }
        let red: Vec<u8> = self.red_gates().iter().map(|r| r.gate.number()).collect();
        if !red.is_empty() {
            return Err(ReportError::GatesRed(red));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReportError {
    #[error("could not read or write {path}: {message}")]
    Io { path: String, message: String },

    #[error("corrupt report: {0}")]
    Decode(String),

    #[error("could not encode the report: {0}")]
    Encode(String),

    #[error("report is version {found}, the program needs {expected}")]
    WrongVersion { found: u32, expected: u32 },

    #[error("report is missing gate {0:?} — an absent gate counts as a failure")]
    GatesMissing(Vec<u8>),

    #[error("gate {0:?} is still red — refusing to load")]
    GatesRed(Vec<u8>),

    #[error(
        "the report was produced from a different data file. Re-run reconcile before loading — \
         a green but stale report is more dangerous than no report."
    )]
    StaleReport,
}
