//! Static assets: the Nôm fonts and the site icon.
//!
//! **Principle: never point at a file that does not exist.** It sounds obvious, but this is
//! exactly where it went wrong: the CSS declared three `@font-face` rules and `base.html`
//! carried a `<link rel="preload">` pointing at `frontend/fonts/`, while that directory was
//! empty. With `@font-face` the browser silently falls back — but `preload` produces a
//! **loud 404 in the console on every page load**.
//! There is no way to render these correctly other than shipping the font of the print itself.
//!
//! The fix is not to drop the fonts, but to have the program **look at what is there before
//! declaring anything**: with no file, nothing points at it; add a file and it appears, with
//! no code change.
//!
//! The fonts are produced by `pipeline/font`: it **extracts the fonts embedded in the source
//! PDF** and subsets them to the characters the book actually uses. See that crate for why.

use std::path::Path;

/// A Unicode range split out into its own font file.
///
/// Split by range so a page downloads only what it needs: a page with only BMP characters
/// never fetches the heavy Ext-B file. The first three match [`dnqatv_core::model::GlyphKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRange {
    Bmp,
    ExtB,
    Pua,
    /// The 26 characters Nôm Na Tống does not have.
    ///
    /// The 2026 print sets those in BabelStone Han, so that is what is shipped for them —
    /// extracted from the same PDF, not downloaded. Without this file they fall through to
    /// whatever CJK font the reader happens to have, which differs machine to machine and is
    /// absent entirely on many.
    Gap,
}

impl FontRange {
    pub const ALL: [FontRange; 4] = [Self::Bmp, Self::ExtB, Self::Pua, Self::Gap];

    /// Accepted formats in priority order, with the name used by `format()` in CSS.
    ///
    /// WOFF2 compresses far better, but `pipeline/font` currently emits TrueType because it
    /// subsets the font with code in this project and pulls in no WOFF2 compressor. Both are
    /// accepted so that adding a compression step later needs no change here.
    pub const FORMATS: [(&'static str, &'static str); 2] =
        [("woff2", "woff2"), ("ttf", "truetype")];

    /// The file name **without extension**; the extension is discovered by [`FontSet::scan`].
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Bmp => "NomNaTong-bmp",
            Self::ExtB => "NomNaTong-extb",
            Self::Pua => "NomNaTong-pua",
            Self::Gap => "BabelStoneHan-gap",
        }
    }

    /// The CSS family this range is declared under.
    ///
    /// The gap file really is BabelStone Han, so it is declared under that name rather than
    /// borrowing "Nom Na Tong". `--nom` in `main.css` already lists BabelStone Han after
    /// Nom Na Tong, so the browser reaches it without any CSS change — and the declaration
    /// does not misrepresent whose glyphs these are.
    pub const fn family(self) -> &'static str {
        match self {
            Self::Bmp | Self::ExtB | Self::Pua => FONT_FAMILY,
            Self::Gap => GAP_FONT_FAMILY,
        }
    }

    /// The fixed `unicode-range` of the three Nôm files, or `None` for the gap file.
    ///
    /// The gap file has no fixed range: its content is whatever Nôm Na Tống happened to lack,
    /// which is a measurement of the data, not a constant. [`FontSet::scan`] reads the exact
    /// list out of the file's own `cmap`, so there is no second list that could drift.
    pub const fn fixed_unicode_range(self) -> Option<&'static str> {
        match self {
            Self::Bmp => Some("U+3400-4DBF, U+4E00-9FFF, U+F900-FAFF"),
            Self::ExtB => Some("U+20000-2FFFF, U+30000-3FFFF"),
            Self::Pua => Some("U+E000-F8FF, U+F0000-FFFFD"),
            Self::Gap => None,
        }
    }
}

/// The public path of the font directory.
pub const FONT_URL_PREFIX: &str = "/tinh/fonts";
/// The family the three Nôm files are declared under. Must match `--nom` in `main.css`.
pub const FONT_FAMILY: &str = "Nom Na Tong";
/// The family the gap file is declared under. Already present in the `--nom` stack.
pub const GAP_FONT_FAMILY: &str = "BabelStone Han";

/// One font file found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFile {
    pub range: FontRange,
    /// The real file name, with extension.
    pub name: String,
    /// The format name for `format()` in CSS.
    pub css_format: &'static str,
    /// The `unicode-range` this file is declared with — fixed for the Nôm files, read from
    /// the file's own `cmap` for the gap file.
    pub unicode_range: String,
}

/// The font files **actually present on disk**.
#[derive(Debug, Clone, Default)]
pub struct FontSet {
    available: Vec<FontFile>,
}

impl FontSet {
    /// Scan the font directory. A missing directory is fine — it means no font is shipped yet.
    ///
    /// A file whose `unicode-range` cannot be determined is **skipped rather than declared**:
    /// an `@font-face` with a wrong range would quietly claim characters it does not have,
    /// which is worse than not shipping it.
    pub fn scan(dir: &Path) -> Self {
        let mut available = Vec::new();
        for range in FontRange::ALL {
            for (ext, css_format) in FontRange::FORMATS {
                let name = format!("{}.{ext}", range.stem());
                let path = dir.join(&name);
                if !path.is_file() {
                    continue;
                }
                let unicode_range = match range.fixed_unicode_range() {
                    Some(fixed) => Some(fixed.to_owned()),
                    None => read_unicode_range(&path),
                };
                if let Some(unicode_range) = unicode_range {
                    available.push(FontFile {
                        range,
                        name,
                        css_format,
                        unicode_range,
                    });
                }
                break;
            }
        }
        Self { available }
    }

    pub fn has(&self, range: FontRange) -> bool {
        self.file(range).is_some()
    }

    pub fn file(&self, range: FontRange) -> Option<&FontFile> {
        self.available.iter().find(|f| f.range == range)
    }

    /// Whether a file name is valid — used by the font-serving route, so the URL only accepts
    /// names the program itself chose, never an arbitrary string from outside.
    pub fn by_name(&self, name: &str) -> Option<&FontFile> {
        self.available.iter().find(|f| f.name == name)
    }

    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }

    pub fn len(&self) -> usize {
        self.available.len()
    }

    /// The `preload` URL, present only when the BMP file really exists.
    ///
    /// Only the BMP range is preloaded: the other two are needed only on pages that really
    /// contain Ext-B or PUA characters, and preloading what most pages never use makes
    /// everyone pay for a minority.
    pub fn preload_url(&self) -> Option<String> {
        self.file(FontRange::Bmp)
            .map(|f| format!("{FONT_URL_PREFIX}/{}", f.name))
    }

    /// The `@font-face` blocks, generated from exactly the files that exist.
    ///
    /// `font-display: block` is deliberate: with Han characters, showing ▯ and then swapping
    /// to the real glyph is worse than a short blank — a reader would believe the square is the glyph.
    pub fn font_face_css(&self) -> String {
        if self.is_empty() {
            return format!(
                "/* No font file shipped under {FONT_URL_PREFIX}/, so no @font-face is declared.\n   \
                 Han-Nom text falls back to the system fonts listed in --nom.\n   \
                 Declaring an @font-face pointing at a missing file only produces a 404. */\n"
            );
        }
        let mut css =
            String::from("/* Generated at runtime from the font files present on disk. */\n");
        for f in &self.available {
            css.push_str(&format!(
                "@font-face {{\n  \
                 font-family: \"{}\";\n  \
                 src: url(\"{FONT_URL_PREFIX}/{}\") format(\"{}\");\n  \
                 unicode-range: {};\n  \
                 font-display: block;\n}}\n",
                f.range.family(),
                f.name,
                f.css_format,
                f.unicode_range
            ));
        }
        css
    }
}

/// The exact `unicode-range` of a font file, read from its own `cmap`.
///
/// Returns `None` when the file cannot be read or parsed. The caller then leaves the file
/// undeclared: a guessed range would claim characters the file does not contain, and the
/// browser would show nothing for them rather than falling through to a font that has them.
fn read_unicode_range(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let font = dnqatv_font_core::Font::parse(&data).ok()?;
    let map = font.unicode_map().ok()?;
    if map.is_empty() {
        return None;
    }
    let parts: Vec<String> = map.keys().map(|cp| format!("U+{cp:X}")).collect();
    Some(parts.join(", "))
}

/// The site icon, as SVG.
///
/// Drawn as a shape rather than text: a `<text>` Han character in SVG depends on the
/// viewer machine fonts, which is exactly what we cannot control. This shape evokes the
/// vermilion seal (印) on a Han-Nom book cover — the role vermilion plays in this whole visual system.
pub const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="3" fill="#FAF6EE"/>
  <rect x="4.5" y="4.5" width="23" height="23" rx="2" fill="none" stroke="#A63A2A" stroke-width="3"/>
  <path d="M16 9.5v13M10.5 16h11" stroke="#A63A2A" stroke-width="3" stroke-linecap="square"/>
</svg>"##;

/// The public path of the icon.
pub const FAVICON_PATH: &str = "/favicon.svg";

impl FontFile {
    /// The MIME type from the real format of the file, never guessed from the range name.
    pub fn mime(&self) -> &'static str {
        if self.css_format == "woff2" {
            "font/woff2"
        } else {
            "font/ttf"
        }
    }
}
