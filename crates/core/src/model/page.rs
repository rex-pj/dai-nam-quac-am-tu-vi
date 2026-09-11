//! PDF page number and printed page number are TWO DIFFERENT TYPES.
//!
//! Their relationship is not uniform — measured across all 1038 pages:
//!
//! | PDF page  | printed no.    | note                                          |
//! |-----------|----------------|-----------------------------------------------|
//! | 1, 2, 10  | none           | 2026 cover, original cover, the LƯU Ý page    |
//! | 3..=9     | same as PDF    | front matter of the original print            |
//! | 11..=1038 | PDF page − 1   | book body                                     |
//!
//! The LƯU Ý page is a note inserted by the 2026 editors; it alone shifts everything after
//! it by one. The conversion is therefore *partial* — some pages have no printed number.
//!
//! Which of the two a reader sees is not a free choice either: the number on the paper is the
//! one they cite and retype, so it is the number in the URL. See [`PageAddress`].

use crate::error::DomainError;
use std::fmt;

/// Total page count of the 2026 digital edition.
pub const PDF_PAGE_COUNT: u16 = 1038;
/// First PDF page of the book body (the CHỮ A section).
pub const BODY_FIRST_PDF_PAGE: u16 = 11;
/// Lowest number the print puts on a page — pages 1 and 2 of the original are unnumbered covers.
pub const PRINTED_PAGE_FIRST: u16 = 3;
/// Highest number the print puts on a page: the last PDF page, less the inserted LƯU Ý page.
pub const PRINTED_PAGE_LAST: u16 = PDF_PAGE_COUNT - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PdfPage(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrintedPage(u16);

impl PdfPage {
    pub fn new(n: u16) -> Result<Self, DomainError> {
        if n == 0 || n > PDF_PAGE_COUNT {
            return Err(DomainError::PageOutOfRange(n));
        }
        Ok(Self(n))
    }

    /// Every page of the edition, in book order.
    ///
    /// Handed out by the domain so no caller has to rebuild `1..=PDF_PAGE_COUNT` and then
    /// prove to itself that each number is in range.
    pub fn all() -> impl Iterator<Item = Self> {
        (1..=PDF_PAGE_COUNT).map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    /// Whether this page belongs to the book body (i.e. carries entries).
    pub const fn is_body(self) -> bool {
        self.0 >= BODY_FIRST_PDF_PAGE
    }

    /// The original printed page number, if this page has one.
    pub const fn printed(self) -> Option<PrintedPage> {
        match self.0 {
            3..=9 => Some(PrintedPage(self.0)),
            n if n >= BODY_FIRST_PDF_PAGE => Some(PrintedPage(n - 1)),
            _ => None,
        }
    }

    /// The one address this page is reachable at — see [`PageAddress`].
    pub const fn address(self) -> PageAddress {
        match self.printed() {
            Some(p) => PageAddress::Printed(p),
            None => PageAddress::Unnumbered(self),
        }
    }
}

impl PrintedPage {
    /// Only numbers the print actually carries, `3..=1037`.
    ///
    /// 1 and 2 are rejected on purpose: the original's first two leaves are unnumbered, so a
    /// reader asking for printed page 1 is asking for a page that does not exist.
    pub fn new(n: u16) -> Result<Self, DomainError> {
        if !(PRINTED_PAGE_FIRST..=PRINTED_PAGE_LAST).contains(&n) {
            return Err(DomainError::PrintedPageOutOfRange(n));
        }
        Ok(Self(n))
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    /// The matching PDF page. Over 3..=9 the two numbering systems coincide, so the body
    /// range must be checked first: printed numbers 3..=9 only ever occur in the front matter.
    ///
    /// Infallible: `3..=1037` maps onto `3..=9` and `11..=1038`, both inside range, and
    /// [`PrintedPage::new`] admits nothing else.
    pub const fn pdf(self) -> PdfPage {
        match self.0 {
            3..=9 => PdfPage(self.0),
            n => PdfPage(n + 1),
        }
    }
}

/// How a page is **addressed in a URL**.
///
/// The number a reader sees on the page, cites in a footnote and retypes into the address
/// bar is the one printed on the paper — so `/trang/10` is printed page 10, not PDF page 10.
/// Printed numbers run 3..=1037 with no hole, which covers 1,035 of the 1,038 pages.
///
/// The remaining three have no printed number at all, and dropping them would lose the LƯU Ý
/// page and both covers, so they keep a second form: `pdf-1`, `pdf-2`, `pdf-10`. That form is
/// rejected for every page that *does* carry a printed number, so each page has exactly one
/// address — two spellings of one page would split its inbound links and its search ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAddress {
    /// Addressed by the number printed on the paper.
    Printed(PrintedPage),
    /// Addressed by PDF page, for the three pages the print left unnumbered.
    Unnumbered(PdfPage),
}

impl PageAddress {
    /// Prefix marking the PDF-numbered form, e.g. `pdf-10`.
    const UNNUMBERED_PREFIX: &'static str = "pdf-";

    /// Read the `{page}` segment of a URL.
    pub fn parse(segment: &str) -> Result<Self, DomainError> {
        let bad = || DomainError::UnknownPageAddress(segment.to_owned());
        match segment.strip_prefix(Self::UNNUMBERED_PREFIX) {
            Some(rest) => {
                let pdf = PdfPage::new(rest.parse::<u16>().map_err(|_| bad())?)?;
                // A page with a printed number is addressed by that number, full stop.
                if pdf.printed().is_some() {
                    return Err(bad());
                }
                Ok(Self::Unnumbered(pdf))
            }
            None => Ok(Self::Printed(PrintedPage::new(
                segment.parse::<u16>().map_err(|_| bad())?,
            )?)),
        }
    }

    /// The page this address names.
    pub const fn pdf_page(self) -> PdfPage {
        match self {
            Self::Printed(p) => p.pdf(),
            Self::Unnumbered(p) => p,
        }
    }
}

impl fmt::Display for PageAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Printed(p) => write!(f, "{}", p.get()),
            Self::Unnumbered(p) => write!(f, "{}{}", Self::UNNUMBERED_PREFIX, p.get()),
        }
    }
}
