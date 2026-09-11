//! The PDF extraction core — pure functions, no I/O.
//!
//! Everything here is legitimately `pub`: this crate is small and does one job, so the crate
//! boundary is the encapsulation boundary. That lets the tests live in an external tests/
//! directory without reaching into private items.

pub mod cmap;
pub mod content;
pub mod document;
pub mod layout;
pub mod render;
pub mod style;

pub use cmap::{CMapError, DecodedGlyph, ToUnicodeCMap};
pub use content::{ContentError, Token, tokenize};
pub use document::{DocumentError, PdfBook};
pub use layout::{Column, Line, Segment, TextRun, group_into_lines};
pub use render::{FontEncoding, FontMap, PageText, UnmappedGlyph, render_page};
pub use style::TextStyle;
