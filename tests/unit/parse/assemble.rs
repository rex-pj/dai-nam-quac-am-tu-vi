// clippy only relaxes panic/expect inside #[test] functions.
#![allow(clippy::expect_used, clippy::panic)]

//! Assembling lines into entries.
//!
//! This is the step that once dropped 355,833 characters, so most tests here check
//! **gate 3**: the assembly must be a partition of the content line stream.

use dnqatv_parse_core::assemble::{assemble, check_partition};
use dnqatv_parse_core::line::LineRole;

use LineRole::{
    Continuation as Cont, ContinuedHeadword as Cont2, Headword as Head, PageNumber as Num,
    RunningHead as Run, SectionTitle as Title, SubEntry as Sub,
};

// ── Basic shapes ─────────────────────────────────────────────────────────────

#[test]
fn one_entry_holds_its_gloss_and_its_sub_entries() {
    // The real shape of the entry 𨰲 Lõm on page 500.
    let roles = [Head, Cont, Sub, Sub, Cont, Sub];
    let a = assemble(&roles);

    assert_eq!(a.entries.len(), 1);
    let e = &a.entries[0];
    assert_eq!(e.headword_line, 0);
    assert_eq!(e.gloss_lines, vec![1]);
    assert_eq!(e.sub_entries.len(), 3);
    assert!(!e.inherits_glyph);
}

#[test]
fn a_continuation_belongs_to_the_nearest_sub_entry() {
    // "― chôm. Bộ không vững vàng, không tê tỉnh.Giò giám, không" + "trơn liền."
    let roles = [Head, Sub, Cont, Cont];
    let a = assemble(&roles);
    let e = &a.entries[0];
    assert!(
        e.gloss_lines.is_empty(),
        "before any sub-entry it goes to the gloss"
    );
    assert_eq!(e.sub_entries[0].continuation_lines, vec![2, 3]);
}

#[test]
fn a_continuation_before_the_first_sub_entry_belongs_to_the_main_gloss() {
    let roles = [Head, Cont, Cont, Sub];
    let e = &assemble(&roles).entries[0];
    assert_eq!(e.gloss_lines, vec![1, 2]);
    assert!(e.sub_entries[0].continuation_lines.is_empty());
}

#[test]
fn a_new_headword_opens_a_new_entry() {
    let roles = [Head, Cont, Head, Cont, Head];
    let a = assemble(&roles);
    assert_eq!(a.entries.len(), 3);
    assert_eq!(a.entries[2].headword_line, 4);
}

#[test]
fn an_entry_reusing_the_previous_glyph_is_marked() {
    // p.26 — `北 Bấc c.` then `n. Bấc ; bức tức.` share a glyph. 55 such lines measured.
    let roles = [Head, Cont, Cont2, Cont];
    let a = assemble(&roles);
    assert_eq!(a.entries.len(), 2);
    assert!(!a.entries[0].inherits_glyph);
    assert!(
        a.entries[1].inherits_glyph,
        "it must be recorded as reusing the glyph, not treated as independent"
    );
}

// ── Non-content lines ────────────────────────────────────────────────────────

#[test]
fn running_heads_page_numbers_and_titles_never_enter_an_entry() {
    let roles = [Run, Title, Head, Cont, Num];
    let a = assemble(&roles);
    assert_eq!(a.entries.len(), 1);
    assert_eq!(a.entries[0].line_indices(), vec![2, 3]);
    assert!(a.orphan_lines.is_empty());
}

#[test]
fn a_running_head_in_the_middle_does_not_split_an_entry() {
    // An entry spilling to the next page: a running head and page number sit between its lines.
    let roles = [Head, Sub, Num, Run, Sub, Cont];
    let a = assemble(&roles);
    assert_eq!(a.entries.len(), 1, "it must still be ONE entry");
    assert_eq!(a.entries[0].sub_entries.len(), 2);
    assert!(check_partition(&roles, &a).is_exact());
}

// ── Gate 3: partition ────────────────────────────────────────────────────────

#[test]
fn every_content_line_is_consumed_exactly_once() {
    let roles = [Run, Head, Cont, Sub, Cont, Cont2, Cont, Sub, Num];
    let a = assemble(&roles);
    let p = check_partition(&roles, &a);
    assert!(p.is_exact(), "not a partition: {p:?}");
}

#[test]
fn content_lines_before_the_first_entry_become_orphans_rather_than_vanishing() {
    // They must not be dropped silently: orphans must be countable and nameable.
    let roles = [Cont, Sub, Head, Cont];
    let a = assemble(&roles);
    assert_eq!(a.orphan_lines, vec![0, 1]);
    assert_eq!(a.entries.len(), 1);

    // Orphans still count as consumed, so the partition stays complete.
    assert!(check_partition(&roles, &a).is_exact());
}

#[test]
fn an_empty_stream_produces_no_entry() {
    let a = assemble(&[]);
    assert!(a.entries.is_empty());
    assert!(a.orphan_lines.is_empty());
    assert!(check_partition(&[], &a).is_exact());
}

#[test]
fn a_stream_of_only_non_content_lines() {
    let roles = [Run, Title, Num];
    let a = assemble(&roles);
    assert!(a.entries.is_empty());
    assert!(check_partition(&roles, &a).is_exact());
}

#[test]
fn the_gate_catches_a_line_consumed_twice() {
    // A broken assembly is built by hand to prove the gate is not a formality.
    let roles = [Head, Cont];
    let mut a = assemble(&roles);
    let dup = a.entries[0].clone();
    a.entries.push(dup);

    let p = check_partition(&roles, &a);
    assert!(!p.is_exact());
    assert_eq!(p.duplicated, vec![0, 1]);
}

#[test]
fn the_gate_catches_a_missing_line() {
    let roles = [Head, Cont, Cont];
    let mut a = assemble(&roles);
    a.entries[0].gloss_lines.pop();

    let p = check_partition(&roles, &a);
    assert!(!p.is_exact());
    assert_eq!(p.missing, vec![2]);
}
