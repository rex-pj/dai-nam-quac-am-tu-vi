//! The Postgres enum types, on the SeaORM side.
//!
//! **Why redeclare them instead of using `core::Pos` directly:** `ActiveEnum` is a SeaORM
//! trait and `Pos` belongs to another crate, so Rust orphan rules forbid the impl. This is
//! the price of keeping `core` free of infrastructure — and it is worth paying.
//!
//! That price is guarded by a parity test (`tests/unit/entity/enum_parity.rs`): variant
//! count, stored DB values and both conversion directions must match the domain layer.
//! Drift between the two lists is the only thing that can go wrong here, so it is checked head-on.

use dnqatv_core::model::{GlyphKind as CoreGlyphKind, Letter as CoreLetter, Pos as CorePos};
use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "entry_pos")]
pub enum EntryPos {
    #[sea_orm(string_value = "chu_nho")]
    ChuNho,
    #[sea_orm(string_value = "chu_nom")]
    ChuNom,
    #[sea_orm(string_value = "chu_nho_dung_nom")]
    ChuNhoDungNom,
}

impl From<CorePos> for EntryPos {
    fn from(p: CorePos) -> Self {
        match p {
            CorePos::ChuNho => Self::ChuNho,
            CorePos::ChuNom => Self::ChuNom,
            CorePos::ChuNhoDungNom => Self::ChuNhoDungNom,
        }
    }
}

impl From<EntryPos> for CorePos {
    fn from(p: EntryPos) -> Self {
        match p {
            EntryPos::ChuNho => Self::ChuNho,
            EntryPos::ChuNom => Self::ChuNom,
            EntryPos::ChuNhoDungNom => Self::ChuNhoDungNom,
        }
    }
}

/// The 22 letters, **in printed order** — Y sits where I would be.
///
/// The declaration order here must match `Letter::ALL`: Postgres compares enums by
/// declaration order, so a mismatch makes `ORDER BY letter` sort wrongly with no warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "book_letter")]
pub enum BookLetter {
    #[sea_orm(string_value = "a")]
    A,
    #[sea_orm(string_value = "b")]
    B,
    #[sea_orm(string_value = "c")]
    C,
    #[sea_orm(string_value = "d")]
    D,
    #[sea_orm(string_value = "dd")]
    Dd,
    #[sea_orm(string_value = "e")]
    E,
    #[sea_orm(string_value = "g")]
    G,
    #[sea_orm(string_value = "h")]
    H,
    #[sea_orm(string_value = "y")]
    Y,
    #[sea_orm(string_value = "k")]
    K,
    #[sea_orm(string_value = "l")]
    L,
    #[sea_orm(string_value = "m")]
    M,
    #[sea_orm(string_value = "n")]
    N,
    #[sea_orm(string_value = "o")]
    O,
    #[sea_orm(string_value = "p")]
    P,
    #[sea_orm(string_value = "q")]
    Q,
    #[sea_orm(string_value = "r")]
    R,
    #[sea_orm(string_value = "s")]
    S,
    #[sea_orm(string_value = "t")]
    T,
    #[sea_orm(string_value = "u")]
    U,
    #[sea_orm(string_value = "v")]
    V,
    #[sea_orm(string_value = "x")]
    X,
}

impl From<CoreLetter> for BookLetter {
    fn from(l: CoreLetter) -> Self {
        match l {
            CoreLetter::A => Self::A,
            CoreLetter::B => Self::B,
            CoreLetter::C => Self::C,
            CoreLetter::D => Self::D,
            CoreLetter::Đ => Self::Dd,
            CoreLetter::E => Self::E,
            CoreLetter::G => Self::G,
            CoreLetter::H => Self::H,
            CoreLetter::Y => Self::Y,
            CoreLetter::K => Self::K,
            CoreLetter::L => Self::L,
            CoreLetter::M => Self::M,
            CoreLetter::N => Self::N,
            CoreLetter::O => Self::O,
            CoreLetter::P => Self::P,
            CoreLetter::Q => Self::Q,
            CoreLetter::R => Self::R,
            CoreLetter::S => Self::S,
            CoreLetter::T => Self::T,
            CoreLetter::U => Self::U,
            CoreLetter::V => Self::V,
            CoreLetter::X => Self::X,
        }
    }
}

impl From<BookLetter> for CoreLetter {
    fn from(l: BookLetter) -> Self {
        match l {
            BookLetter::A => Self::A,
            BookLetter::B => Self::B,
            BookLetter::C => Self::C,
            BookLetter::D => Self::D,
            BookLetter::Dd => Self::Đ,
            BookLetter::E => Self::E,
            BookLetter::G => Self::G,
            BookLetter::H => Self::H,
            BookLetter::Y => Self::Y,
            BookLetter::K => Self::K,
            BookLetter::L => Self::L,
            BookLetter::M => Self::M,
            BookLetter::N => Self::N,
            BookLetter::O => Self::O,
            BookLetter::P => Self::P,
            BookLetter::Q => Self::Q,
            BookLetter::R => Self::R,
            BookLetter::S => Self::S,
            BookLetter::T => Self::T,
            BookLetter::U => Self::U,
            BookLetter::V => Self::V,
            BookLetter::X => Self::X,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "glyph_kind")]
pub enum GlyphKind {
    #[sea_orm(string_value = "bmp")]
    Bmp,
    #[sea_orm(string_value = "ext_b")]
    ExtB,
    #[sea_orm(string_value = "pua")]
    Pua,
    #[sea_orm(string_value = "image_only")]
    ImageOnly,
}

impl From<CoreGlyphKind> for GlyphKind {
    fn from(k: CoreGlyphKind) -> Self {
        match k {
            CoreGlyphKind::Bmp => Self::Bmp,
            CoreGlyphKind::ExtB => Self::ExtB,
            CoreGlyphKind::Pua => Self::Pua,
            CoreGlyphKind::ImageOnly => Self::ImageOnly,
        }
    }
}

impl From<GlyphKind> for CoreGlyphKind {
    fn from(k: GlyphKind) -> Self {
        match k {
            GlyphKind::Bmp => Self::Bmp,
            GlyphKind::ExtB => Self::ExtB,
            GlyphKind::Pua => Self::Pua,
            GlyphKind::ImageOnly => Self::ImageOnly,
        }
    }
}
