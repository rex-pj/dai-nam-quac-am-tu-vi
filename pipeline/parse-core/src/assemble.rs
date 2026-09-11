//! Assembling printed lines into complete entries.
//!
//! This is the step that dropped **355,833 characters** in the first exploratory pass, so it
//! is designed to make loss **impossible in a checkable way**: an entry holds no text, it
//! holds the **line indices** it consumed. Gate ③ then reduces to a claim that is hard to
//! argue with — *every content line is consumed exactly once, no more and no less*.
//!
//! Precondition: the line stream must be **continuous across pages**. Assembling page by
//! page leaves 25,087 orphan lines, because entries routinely spill over a page boundary —
//! page 500, for instance, opens with a sub-entry of an entry that started on page 499.

use crate::line::LineRole;

/// One sub-entry block: the opening line plus the definition lines flowing after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubEntryBlock {
    pub first_line: usize,
    pub continuation_lines: Vec<usize>,
}

impl SubEntryBlock {
    pub fn line_indices(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(self.first_line).chain(self.continuation_lines.iter().copied())
    }
}

/// One entry, represented entirely by line indices into the source stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The line opening the entry.
    pub headword_line: usize,
    /// This entry reuses the glyph of the preceding entry (its opening line has labels only).
    ///
    /// 55 such lines measured, e.g. p.26: after `北 Bấc c.` comes `n. Bấc ; bức tức.`
    pub inherits_glyph: bool,
    /// The lines of the main definition.
    pub gloss_lines: Vec<usize>,
    pub sub_entries: Vec<SubEntryBlock>,
}

impl Entry {
    /// Every line index this entry consumed, in source order.
    pub fn line_indices(&self) -> Vec<usize> {
        let mut out = vec![self.headword_line];
        out.extend(&self.gloss_lines);
        for block in &self.sub_entries {
            out.extend(block.line_indices());
        }
        out.sort_unstable();
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assembly {
    pub entries: Vec<Entry>,
    /// Content lines belonging to no entry — usually lines preceding the first entry.
    ///
    /// They are allowed to exist but must be **countable and nameable**, never silently dropped.
    pub orphan_lines: Vec<usize>,
}

/// Assemble a stream of line roles into entries.
///
/// It takes roles rather than text: that keeps the function pure and makes the tests read
/// like a specification.
pub fn assemble(roles: &[LineRole]) -> Assembly {
    let mut out = Assembly::default();
    let mut current: Option<Entry> = None;

    for (index, role) in roles.iter().enumerate() {
        match role {
            // Not dictionary content: running head, page number, section title.
            r if !r.is_content() => {}

            LineRole::Headword | LineRole::ContinuedHeadword => {
                if let Some(entry) = current.take() {
                    out.entries.push(entry);
                }
                current = Some(Entry {
                    headword_line: index,
                    inherits_glyph: matches!(role, LineRole::ContinuedHeadword),
                    gloss_lines: Vec::new(),
                    sub_entries: Vec::new(),
                });
            }

            LineRole::SubEntry => match current.as_mut() {
                Some(entry) => entry.sub_entries.push(SubEntryBlock {
                    first_line: index,
                    continuation_lines: Vec::new(),
                }),
                None => out.orphan_lines.push(index),
            },

            LineRole::Continuation => match current.as_mut() {
                // A continuation belongs to the nearest sub-entry if there is one, else to the main definition.
                Some(entry) => match entry.sub_entries.last_mut() {
                    Some(block) => block.continuation_lines.push(index),
                    None => entry.gloss_lines.push(index),
                },
                None => out.orphan_lines.push(index),
            },

            _ => {}
        }
    }

    if let Some(entry) = current.take() {
        out.entries.push(entry);
    }
    out
}

/// The result of the line-level gate ③ check.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Partition {
    /// Content lines no entry consumed.
    pub missing: Vec<usize>,
    /// Lines consumed by more than one entry.
    pub duplicated: Vec<usize>,
    /// Lines that were consumed but are not dictionary content.
    pub unexpected: Vec<usize>,
}

impl Partition {
    pub fn is_exact(&self) -> bool {
        self.missing.is_empty() && self.duplicated.is_empty() && self.unexpected.is_empty()
    }
}

/// Gate ③ at the line level: the assembly must be a **partition** of the content line stream.
///
/// Stronger than "no line is lost": double consumption is also an error, because it means a
/// passage was filed under two different entries.
pub fn check_partition(roles: &[LineRole], assembly: &Assembly) -> Partition {
    let mut used = vec![0u16; roles.len()];
    for entry in &assembly.entries {
        for index in entry.line_indices() {
            if let Some(slot) = used.get_mut(index) {
                *slot = slot.saturating_add(1);
            }
        }
    }
    for index in &assembly.orphan_lines {
        if let Some(slot) = used.get_mut(*index) {
            *slot = slot.saturating_add(1);
        }
    }

    let mut out = Partition::default();
    for (index, role) in roles.iter().enumerate() {
        match (role.is_content(), used[index]) {
            (true, 0) => out.missing.push(index),
            (true, n) if n > 1 => out.duplicated.push(index),
            (false, n) if n > 0 => out.unexpected.push(index),
            _ => {}
        }
    }
    out
}
