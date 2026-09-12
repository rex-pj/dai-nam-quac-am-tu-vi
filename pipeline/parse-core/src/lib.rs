//! The parsing core: from reconstructed printed lines to structured entries.
//!
//! Pure functions, no I/O, everything `pub` — so the tests live in an external tests/ directory.

pub mod assemble;
pub mod expand;
pub mod headword;
pub mod line;
pub mod span;
pub mod subentry;

pub use assemble::{Assembly, Entry, SubEntryBlock, assemble, check_partition};
pub use expand::{Expansion, HeadwordContext, expand_han_form, expand_reading_form};
pub use headword::{
    ContinuedHeadwordLine, GlossInitial, HeadwordLine, begins_syllable, parse_continued_headword,
    parse_headword,
};
pub use line::{LineRole, PAGE_NUMBER_Y, PLACEHOLDERS, RUNNING_HEAD_Y, SubEntrySignal, classify};
pub use span::{Coverage, Span, SpanError, check_coverage, visible_len};
pub use subentry::{StyledSegment, SubEntryLine, parse_sub_entry};
