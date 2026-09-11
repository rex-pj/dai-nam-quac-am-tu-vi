//! Tokenizing a PDF content stream.
//!
//! Works on **bytes**, not `str`: strings in a content stream carry arbitrary binary data
//! (2-byte glyph codes of Type0 fonts) and are not guaranteed to be UTF-8.
//!
//! Measured on the real file: five sample pages contain no inline images (`BI`/`ID`/`EI`).
//! Even so, meeting `BI` is a **hard error** rather than a skip — the binary data of an
//! inline image would be misread as operators and silently produce garbage text.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContentError {
    #[error("literal string has no closing parenthesis, starting at byte {0}")]
    UnterminatedLiteralString(usize),

    #[error("hex string has no closing '>', starting at byte {0}")]
    UnterminatedHexString(usize),

    #[error("hex string at byte {offset} contains a non-hex character: {byte:?}")]
    InvalidHexDigit { offset: usize, byte: u8 },

    #[error("unreadable number at byte {offset}: {raw:?}")]
    InvalidNumber { offset: usize, raw: String },

    #[error("inline image (BI) at byte {0}; the tokenizer does not support it and must not guess")]
    InlineImageUnsupported(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A literal or hex string, unescaped, bytes preserved.
    Str(Vec<u8>),
    Name(String),
    Number(f64),
    ArrayOpen,
    ArrayClose,
    DictOpen,
    DictClose,
    Operator(String),
}

impl Token {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_operator(&self) -> Option<&str> {
        match self {
            Self::Operator(s) => Some(s),
            _ => None,
        }
    }
}

const fn is_pdf_whitespace(b: u8) -> bool {
    matches!(b, b'\0' | b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

const fn is_pdf_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

const fn is_regular(b: u8) -> bool {
    !is_pdf_whitespace(b) && !is_pdf_delimiter(b)
}

pub fn tokenize(src: &[u8]) -> Result<Vec<Token>, ContentError> {
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < src.len() {
        let b = src[i];

        if is_pdf_whitespace(b) {
            i += 1;
            continue;
        }

        match b {
            b'%' => {
                while i < src.len() && src[i] != b'\n' && src[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let (bytes, next) = read_literal_string(src, i)?;
                out.push(Token::Str(bytes));
                i = next;
            }
            b'<' if src.get(i + 1) == Some(&b'<') => {
                out.push(Token::DictOpen);
                i += 2;
            }
            b'<' => {
                let (bytes, next) = read_hex_string(src, i)?;
                out.push(Token::Str(bytes));
                i = next;
            }
            b'>' if src.get(i + 1) == Some(&b'>') => {
                out.push(Token::DictClose);
                i += 2;
            }
            b'>' => i += 1,
            b'[' => {
                out.push(Token::ArrayOpen);
                i += 1;
            }
            b']' => {
                out.push(Token::ArrayClose);
                i += 1;
            }
            b'/' => {
                let start = i + 1;
                let mut j = start;
                while j < src.len() && is_regular(src[j]) {
                    j += 1;
                }
                out.push(Token::Name(
                    String::from_utf8_lossy(&src[start..j]).into_owned(),
                ));
                i = j;
            }
            b'{' | b'}' => i += 1,
            _ => {
                let start = i;
                let mut j = i;
                while j < src.len() && is_regular(src[j]) {
                    j += 1;
                }
                if j == start {
                    i += 1;
                    continue;
                }
                let raw = &src[start..j];
                if raw[0].is_ascii_digit() || matches!(raw[0], b'+' | b'-' | b'.') {
                    let text = String::from_utf8_lossy(raw);
                    let n = parse_pdf_number(&text).ok_or_else(|| ContentError::InvalidNumber {
                        offset: start,
                        raw: text.into_owned(),
                    })?;
                    out.push(Token::Number(n));
                } else {
                    let op = String::from_utf8_lossy(raw).into_owned();
                    if op == "BI" {
                        return Err(ContentError::InlineImageUnsupported(start));
                    }
                    out.push(Token::Operator(op));
                }
                i = j;
            }
        }
    }

    Ok(out)
}

/// PDF allows numbers like `4.`, `.5`, `-.002`, `--5` (a known defect). Only valid forms are accepted.
fn parse_pdf_number(text: &str) -> Option<f64> {
    if text == "." || text == "-" || text == "+" {
        return None;
    }
    text.parse::<f64>().ok().or_else(|| {
        // Rust accepts `4.` and `.5`, so only the leading-sign forms need patching.
        text.strip_prefix('+').and_then(|t| t.parse::<f64>().ok())
    })
}

/// Read a literal string `( … )`.
///
/// Nested parentheses must balance, and `\` opens the escapes of PDF 32000-1 §7.3.4.2:
/// `\n \r \t \b \f \( \) \`, octal codes `\ddd`, and line continuation `\<newline>`.
fn read_literal_string(src: &[u8], start: usize) -> Result<(Vec<u8>, usize), ContentError> {
    let mut out = Vec::new();
    let mut depth = 1usize;
    let mut i = start + 1;

    while i < src.len() {
        match src[i] {
            b'\\' => {
                i += 1;
                let Some(&esc) = src.get(i) else {
                    return Err(ContentError::UnterminatedLiteralString(start));
                };
                match esc {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0C),
                    b'(' => out.push(b'('),
                    b')' => out.push(b')'),
                    b'\\' => out.push(b'\\'),
                    // Line continuation: a backslash right before a newline is dropped, emitting no byte.
                    b'\n' => {}
                    b'\r' => {
                        if src.get(i + 1) == Some(&b'\n') {
                            i += 1;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut val = u16::from(esc - b'0');
                        let mut taken = 1;
                        while taken < 3 {
                            match src.get(i + 1) {
                                Some(&d @ b'0'..=b'7') => {
                                    val = val * 8 + u16::from(d - b'0');
                                    i += 1;
                                    taken += 1;
                                }
                                _ => break,
                            }
                        }
                        // §7.3.4.2: values above 255 drop the high digit; this is spec behaviour.
                        out.push((val & 0xFF) as u8);
                    }
                    other => out.push(other),
                }
                i += 1;
            }
            b'(' => {
                depth += 1;
                out.push(b'(');
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((out, i + 1));
                }
                out.push(b')');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }

    Err(ContentError::UnterminatedLiteralString(start))
}

/// Read a hex string `< … >`. An odd digit count pads the last digit with `0` — per §7.3.4.3.
fn read_hex_string(src: &[u8], start: usize) -> Result<(Vec<u8>, usize), ContentError> {
    let mut digits: Vec<u8> = Vec::new();
    let mut i = start + 1;

    while i < src.len() {
        let b = src[i];
        if b == b'>' {
            if digits.len() % 2 == 1 {
                digits.push(b'0');
            }
            let bytes = digits
                .chunks(2)
                .map(|c| hex_val(c[0]) * 16 + hex_val(c[1]))
                .collect();
            return Ok((bytes, i + 1));
        }
        if is_pdf_whitespace(b) {
            i += 1;
            continue;
        }
        if !b.is_ascii_hexdigit() {
            return Err(ContentError::InvalidHexDigit { offset: i, byte: b });
        }
        digits.push(b);
        i += 1;
    }

    Err(ContentError::UnterminatedHexString(start))
}

/// Only called after `is_ascii_hexdigit` has passed, so the other branches cannot occur.
const fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}
