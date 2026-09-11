#![allow(clippy::expect_used, clippy::panic)]

//! The fail-closed gate between `reconcile` and `import`.

use dnqatv_gate_report::{Fingerprint, Gate, GateReport, GateResult, GateStatus, ReportError};

fn green(gate: Gate) -> GateResult {
    GateResult::judge(gate, 0, 0, "pass")
}

fn all_gates() -> Vec<GateResult> {
    Gate::ALL.into_iter().map(green).collect()
}

#[test]
fn passes_when_the_measurement_equals_the_explained_threshold() {
    // Gate 3 on the real book: 1 unexplained character, and it is a documented print defect.
    let r = GateResult::judge(Gate::CharacterConservation, 1, 1, "print defect on p.353");
    assert_eq!(r.status, GateStatus::Green);
}

#[test]
fn fewer_than_expected_is_also_red() {
    // `<=` is deliberately avoided: a smaller deviation means something changed unnoticed.
    let r = GateResult::judge(Gate::CharacterConservation, 0, 1, "");
    assert_eq!(r.status, GateStatus::Red);
}

#[test]
fn an_absent_gate_counts_as_a_failure() {
    let incomplete: Vec<GateResult> = all_gates().into_iter().take(3).collect();
    let report = GateReport::new("data/entries.jsonl", Fingerprint::of(b"x"), incomplete);
    assert!(!report.all_green());
    assert_eq!(
        report.authorize(Fingerprint::of(b"x")),
        Err(ReportError::GatesMissing(vec![4, 5]))
    );
}

#[test]
fn a_stale_report_is_rejected() {
    // The most dangerous case: every gate green, but green for a DIFFERENT data file.
    let report = GateReport::new(
        "data/entries.jsonl",
        Fingerprint::of(b"old data"),
        all_gates(),
    );
    assert!(report.all_green());
    assert_eq!(
        report.authorize(Fingerprint::of(b"new data")),
        Err(ReportError::StaleReport)
    );
}

#[test]
fn a_red_gate_refuses_the_load() {
    let mut results = all_gates();
    results[3] = GateResult::judge(Gate::IndexReconciliation, 6, 0, "6 glyphs differ");
    let fp = Fingerprint::of(b"x");
    let report = GateReport::new("data/entries.jsonl", fp, results);
    assert_eq!(report.authorize(fp), Err(ReportError::GatesRed(vec![4])));
}

#[test]
fn complete_and_correct_is_authorized() {
    let fp = Fingerprint::of(b"x");
    let report = GateReport::new("data/entries.jsonl", fp, all_gates());
    assert_eq!(report.authorize(fp), Ok(()));
}

#[test]
fn the_fingerprint_changes_when_the_content_changes_by_one_byte() {
    let a = Fingerprint::of(b"line one\nline two\n");
    let b = Fingerprint::of(b"line one\nline twx\n");
    assert_ne!(a, b);
    assert_eq!(a.lines, 2);
    assert_eq!(a.bytes, 18);
}

#[test]
fn a_json_round_trip_returns_the_same_report() {
    let fp = Fingerprint::of(b"x");
    let report = GateReport::new("data/entries.jsonl", fp, all_gates());
    let dir = std::env::temp_dir().join("dnqatv-gate-test");
    std::fs::create_dir_all(&dir).expect("creating the directory");
    let path = dir.join("gates.json");
    report.write(&path).expect("write");
    let doc = GateReport::read(&path).expect("read");
    assert_eq!(doc, report);
}

#[test]
fn every_gate_has_its_own_number_and_title() {
    let mut numbers: Vec<u8> = Gate::ALL.iter().map(|g| g.number()).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5]);
    for g in Gate::ALL {
        assert!(!g.title().is_empty());
    }
}
