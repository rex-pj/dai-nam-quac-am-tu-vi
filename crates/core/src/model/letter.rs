//! The book's 22 CHỮ sections, in the EXACT order printed — not the Latin alphabet.
//!
//! ```text
//! A  B  C  D  Đ  E  G  H  Y  K  L  M  N  O  P  Q  R  S  T  U  V  X
//!                         ^ Y sits where I would be
//! ```
//!
//! The book has no I, F, J, W or Z section. Measured across all 7,007 entries: every reading
//! starting with I/Í/Ỉ/Ị lives under CHỮ Y. The book folds Ă/Â into A, Ê into E, Ô/Ơ into O
//! and Ư into U, while Đ gets its own section.
//!
//! Consequence: the book's collation order is NOT standard Vietnamese collation.

use crate::error::DomainError;
use crate::text::normalize::fold;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Letter {
    A,
    B,
    C,
    D,
    Đ,
    E,
    G,
    H,
    Y,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    X,
}

impl Letter {
    pub const ALL: [Letter; 22] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::Đ,
        Self::E,
        Self::G,
        Self::H,
        Self::Y,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::X,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::Đ => "Đ",
            Self::E => "E",
            Self::G => "G",
            Self::H => "H",
            Self::Y => "Y",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::X => "X",
        }
    }

    pub const fn db_value(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::Đ => "dd",
            Self::E => "e",
            Self::G => "g",
            Self::H => "h",
            Self::Y => "y",
            Self::K => "k",
            Self::L => "l",
            Self::M => "m",
            Self::N => "n",
            Self::O => "o",
            Self::P => "p",
            Self::Q => "q",
            Self::R => "r",
            Self::S => "s",
            Self::T => "t",
            Self::U => "u",
            Self::V => "v",
            Self::X => "x",
        }
    }

    /// Collation rank as printed. Used by the ordering invariant check.
    ///
    /// The variants are declared in the book's own order, so the discriminant IS the rank —
    /// correct by construction, with no lookup table and no fallback value.
    pub const fn collation_rank(self) -> u8 {
        self as u8
    }

    /// The CHỮ section holding readings that start with this character.
    ///
    /// Rule: fold the diacritics, then map the base letter — verified against 70+ real
    /// initials with no exception. `Đ` must be checked BEFORE folding, since `fold` turns
    /// `đ` into `d`.
    pub fn from_initial(c: char) -> Result<Self, DomainError> {
        if c == 'Đ' || c == 'đ' {
            return Ok(Self::Đ);
        }
        let base = fold(&c.to_string()).chars().next();
        match base {
            Some('a') => Ok(Self::A),
            Some('b') => Ok(Self::B),
            Some('c') => Ok(Self::C),
            Some('d') => Ok(Self::D),
            Some('e') => Ok(Self::E),
            Some('g') => Ok(Self::G),
            Some('h') => Ok(Self::H),
            // The book files I under the CHỮ Y section — measured, no exceptions.
            Some('i') | Some('y') => Ok(Self::Y),
            Some('k') => Ok(Self::K),
            Some('l') => Ok(Self::L),
            Some('m') => Ok(Self::M),
            Some('n') => Ok(Self::N),
            Some('o') => Ok(Self::O),
            Some('p') => Ok(Self::P),
            Some('q') => Ok(Self::Q),
            Some('r') => Ok(Self::R),
            Some('s') => Ok(Self::S),
            Some('t') => Ok(Self::T),
            Some('u') => Ok(Self::U),
            Some('v') => Ok(Self::V),
            Some('x') => Ok(Self::X),
            _ => Err(DomainError::UnknownInitial(c.to_string())),
        }
    }

    pub fn from_db_value(s: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|l| l.db_value() == s)
            .ok_or_else(|| DomainError::UnknownInitial(s.to_owned()))
    }
}
