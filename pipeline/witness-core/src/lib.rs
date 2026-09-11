//! Gate ⑥ — comparing the parsed data against an **independent transcription**.
//!
//! Gates ①–⑤ all read the same source: the 2026 digital edition. They can prove the parser
//! is self-consistent; they cannot prove it read the page right. The Wikisource
//! transcription of the **1895–96 original** is a second pair of eyes on the same book,
//! typed by people, carrying its own structure:
//!
//! ```text
//! {{DNQATV/mục|<glyph>|<reading>|<alternate>|<label>|<gloss>}}
//! {{DNQATV/nghĩa|<Han part>|<Quốc ngữ form>|<definition>}}
//! ```
//!
//! Measured on the snapshot in `data/wikisource/`: 1,222 pages, 8,077 headwords, 58,076
//! sub-entries — very nearly the whole book.
//!
//! **This is deliberately not a pass/fail gate.** The 1895 print and the 2026 edition are
//! different editions: the original carries a *Bổ di* (addenda) section the later one folds
//! in, and the two set some entries differently. Demanding an exact match would either cry
//! wolf on every run or, worse, invite someone to soften the threshold until it went quiet —
//! which is the habit `review/gate4-index.toml` forbids in writing. So the comparison
//! sorts its findings into kinds and writes them out for a person, and only the sharp,
//! genuinely contradictory kind is counted as something to answer for.

pub mod align;
pub mod record;
pub mod template;
pub mod text;

pub use align::{Alignment, Divergence, DivergenceKind, compare};
pub use record::{WitnessEntry, WitnessSub, read_page};
pub use template::template_fields;
pub use text::{fold, placeholder_key, plain};
