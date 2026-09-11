//! Distinctly typed keys, so a GlyphId can never be passed where an EntryId is expected.

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i32);

        impl $name {
            pub const fn new(v: i32) -> Self { Self(v) }
            pub const fn get(self) -> i32 { self.0 }
        }
    };
}

id_type!(/// Key of an entry.
    EntryId);
id_type!(/// Key of a Han-Nom glyph.
    GlyphId);
id_type!(/// Key of a page.
    PageId);
id_type!(/// Key of a sub-entry.
    SubEntryId);
