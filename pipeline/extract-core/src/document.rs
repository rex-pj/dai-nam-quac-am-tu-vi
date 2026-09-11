//! Reading the structure of a PDF file: the page tree, font tables, content streams.
//!
//! Still a pure core in the sense that it **never touches the filesystem** — the input is
//! bytes, read by the `extract` shell and passed in. That makes all of this testable
//! without building a fake directory tree.

use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object, ObjectId};
use thiserror::Error;

use crate::cmap::{CMapError, ToUnicodeCMap};
use crate::content::ContentError;
use crate::render::{FontEncoding, FontMap, PageText, render_page};
use crate::style::TextStyle;

/// The decompression limit for one content stream. The largest page measured is around
/// 30 KB, so 64 MB is generous enough never to reject real data while still stopping a
/// decompression bomb.
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("could not read the PDF file: {0}")]
    Pdf(#[from] lopdf::Error),

    #[error("page {page} is out of range; the document has {count} pages")]
    PageOutOfRange { page: u32, count: usize },

    #[error("page {page} has no MediaBox, even walking up the page tree")]
    MissingMediaBox { page: u32 },

    #[error("page {page} has a malformed MediaBox: {detail}")]
    MalformedMediaBox { page: u32, detail: String },

    #[error("page {page} has a corrupt content stream: {source}")]
    Content { page: u32, source: ContentError },

    #[error("font {font} on page {page} has a corrupt /ToUnicode table: {source}")]
    CMap {
        page: u32,
        font: String,
        source: CMapError,
    },
}

/// A loaded PDF book, with its pages in order.
pub struct PdfBook {
    doc: Document,
    /// Page number (1-based) → page object id.
    pages: Vec<(u32, ObjectId)>,
}

impl PdfBook {
    pub fn open(bytes: &[u8]) -> Result<Self, DocumentError> {
        let doc = Document::load_mem(bytes)?;
        let mut pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
        pages.sort_by_key(|(n, _)| *n);
        Ok(Self { doc, pages })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The numbers of every page, in order.
    pub fn page_numbers(&self) -> Vec<u32> {
        self.pages.iter().map(|(n, _)| *n).collect()
    }

    fn page_id(&self, page: u32) -> Result<ObjectId, DocumentError> {
        self.pages
            .iter()
            .find(|(n, _)| *n == page)
            .map(|(_, id)| *id)
            .ok_or(DocumentError::PageOutOfRange {
                page,
                count: self.pages.len(),
            })
    }

    /// The page width, taken from `MediaBox`.
    ///
    /// `MediaBox` is an inheritable attribute: a page may not declare it and inherit from a
    /// parent node, so we must walk up via `Parent`. lopdf has no built-in for this.
    pub fn page_width(&self, page: u32) -> Result<f64, DocumentError> {
        let mut node = self.doc.get_dictionary(self.page_id(page)?)?;
        loop {
            if let Ok(mb) = node.get(b"MediaBox") {
                return media_box_width(page, mb);
            }
            let Ok(parent) = node.get(b"Parent").and_then(Object::as_reference) else {
                return Err(DocumentError::MissingMediaBox { page });
            };
            node = self.doc.get_dictionary(parent)?;
        }
    }

    /// The font table of a page.
    ///
    /// A font without `/ToUnicode` gets an empty table — meaning every one of its codes
    /// becomes [`crate::DecodedGlyph::Unmapped`] and surfaces at the gate, never a guess.
    pub fn page_fonts(&self, page: u32) -> Result<FontMap, DocumentError> {
        let mut out: FontMap = HashMap::new();
        for (name, dict) in self.doc.get_page_fonts(self.page_id(page)?)? {
            let name = String::from_utf8_lossy(&name).into_owned();
            let two_byte = matches!(dict.get(b"Subtype").and_then(Object::as_name), Ok(b"Type0"));
            // A font missing /BaseFont is abnormal, but the text style only classifies line
            // roles rather than producing content, so state the default here instead of staying silent.
            let style = match dict.get(b"BaseFont").and_then(Object::as_name) {
                Ok(name) => TextStyle::from_base_font(&String::from_utf8_lossy(name)),
                Err(_) => TextStyle::Regular,
            };
            let cmap = self.to_unicode(page, &name, dict)?;
            out.insert(
                name,
                FontEncoding {
                    cmap,
                    two_byte,
                    style,
                },
            );
        }
        Ok(out)
    }

    fn to_unicode(
        &self,
        page: u32,
        font_name: &str,
        dict: &Dictionary,
    ) -> Result<ToUnicodeCMap, DocumentError> {
        let Ok(reference) = dict.get(b"ToUnicode").and_then(Object::as_reference) else {
            return Ok(ToUnicodeCMap::default());
        };
        let stream = self.doc.get_object(reference)?.as_stream()?;
        let raw = stream.decompressed_content_with_limit(MAX_CONTENT_BYTES)?;
        // A CMap is ASCII PostScript text; latin-1 preserves every byte so the hex parts read correctly.
        let text: String = raw.iter().map(|b| char::from(*b)).collect();
        ToUnicodeCMap::parse(&text).map_err(|source| DocumentError::CMap {
            page,
            font: font_name.to_owned(),
            source,
        })
    }

    pub fn page_content(&self, page: u32) -> Result<Vec<u8>, DocumentError> {
        let id = self.page_id(page)?;
        Ok(self
            .doc
            .get_page_content_with_limit(id, MAX_CONTENT_BYTES)?)
    }

    /// The text of one page, reassembled into printed lines.
    pub fn page_text(&self, page: u32) -> Result<PageText, DocumentError> {
        let content = self.page_content(page)?;
        let fonts = self.page_fonts(page)?;
        let width = self.page_width(page)?;
        render_page(&content, &fonts, width)
            .map_err(|source| DocumentError::Content { page, source })
    }
}

/// `MediaBox` is an array of four numbers `[x0 y0 x1 y1]`, each an integer or a real.
fn media_box_width(page: u32, obj: &Object) -> Result<f64, DocumentError> {
    let array = obj
        .as_array()
        .map_err(|e| DocumentError::MalformedMediaBox {
            page,
            detail: e.to_string(),
        })?;
    if array.len() != 4 {
        return Err(DocumentError::MalformedMediaBox {
            page,
            detail: format!("expected 4 elements, got {}", array.len()),
        });
    }
    let x0 = number(page, &array[0])?;
    let x1 = number(page, &array[2])?;
    Ok((x1 - x0).abs())
}

fn number(page: u32, obj: &Object) -> Result<f64, DocumentError> {
    match obj {
        Object::Integer(i) => Ok(*i as f64),
        Object::Real(r) => Ok(f64::from(*r)),
        other => Err(DocumentError::MalformedMediaBox {
            page,
            detail: format!("not a number: {other:?}"),
        }),
    }
}
