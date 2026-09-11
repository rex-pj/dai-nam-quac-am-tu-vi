//! The book's part-of-speech labels. THE SINGLE SOURCE OF TRUTH for migration, entity,
//! DTO and parser alike.
//!
//! Adding a variant here breaks every `match` below until it is handled — `match`
//! exhaustiveness is the drift detector across the five places this concept appears.

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pos {
    /// `c.` — chữ nho. 2,565 labels counted in the book body.
    ChuNho,
    /// `n.` — chữ nôm. 4,472 labels counted.
    ChuNom,
    /// `cn.` — a Chinese character also used as Nôm.
    ///
    /// Occurs EXACTLY ONCE in the whole book (PDF page 67: `盆 Bồn. cn. Chậu Trồng…`),
    /// and there it sits on the same line as the reading instead of on its own line
    /// like the other 7,037 labels.
    ChuNhoDungNom,
}

impl Pos {
    pub const ALL: [Pos; 3] = [Self::ChuNho, Self::ChuNom, Self::ChuNhoDungNom];

    /// The label exactly as printed in the book.
    pub const fn book_label(self) -> &'static str {
        match self {
            Self::ChuNho => "c.",
            Self::ChuNom => "n.",
            Self::ChuNhoDungNom => "cn.",
        }
    }

    /// The value stored in the Postgres enum. The migration generates `CREATE TYPE`
    /// by iterating `ALL`.
    pub const fn db_value(self) -> &'static str {
        match self {
            Self::ChuNho => "chu_nho",
            Self::ChuNom => "chu_nom",
            Self::ChuNhoDungNom => "chu_nho_dung_nom",
        }
    }

    /// The book's own full name for the label, spelled out for display.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ChuNho => "chữ nho",
            Self::ChuNom => "chữ nôm",
            Self::ChuNhoDungNom => "chữ nho dùng nôm",
        }
    }

    /// Read a label from the printed page. Deliberately STRICT: only the three measured
    /// forms are accepted. Anything else must surface as an error for a human to look at,
    /// never be guessed.
    pub fn from_book_label(s: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|p| p.book_label() == s)
            .ok_or_else(|| DomainError::UnknownPosLabel(s.to_owned()))
    }

    pub fn from_db_value(s: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|p| p.db_value() == s)
            .ok_or_else(|| DomainError::UnknownPosLabel(s.to_owned()))
    }
}
