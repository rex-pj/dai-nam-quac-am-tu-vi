use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("empty reading")]
    EmptyReading,

    #[error("reading {0:?} does not start with any letter of the book")]
    UnknownInitial(String),

    #[error("part-of-speech label {0:?} is not c. / n. / cn.")]
    UnknownPosLabel(String),

    #[error("a glyph must be exactly one character, got {0:?}")]
    NotSingleGlyph(String),

    #[error("PDF page {0} falls outside the 2026 edition (1..=1038)")]
    PageOutOfRange(u16),

    #[error("no page of the print carries the number {0} (3..=1037)")]
    PrintedPageOutOfRange(u16),

    #[error("{0:?} names no page: expected a printed page number, or pdf-1 / pdf-2 / pdf-10")]
    UnknownPageAddress(String),

    #[error("an entry must carry at least one part-of-speech label")]
    NoPosLabel,

    #[error("empty slug")]
    EmptySlug,

    #[error("slug {0:?} contains characters outside a-z, 0-9 and hyphen")]
    InvalidSlug(String),
}
