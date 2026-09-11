//! Making two editions comparable **without changing either of them.**
//!
//! Everything here produces a comparison key. None of it is ever stored: the rule of the
//! project is that the print goes in as written, so normalising is something done to a copy
//! at the moment of asking "are these two the same line?", never to the data.
//!
//! Three differences have to be folded away before the question can be asked at all:
//!
//! 1. **Wiki markup.** `''italic''`, `[[links]]`, stray inline templates.
//! 2. **The placeholder mark.** The 2026 edition distinguishes `|` (stands for the glyph)
//!    from `―` (stands for the reading), and uses three dash shapes for the latter. The
//!    Wikisource transcribers wrote a single `-` for both. So the *mark* cannot be compared —
//!    only its **position** can, and [`placeholder_key`] reduces every mark to one token.
//! 3. **Punctuation and case**, for the looser [`fold`] used when hunting for a partner line.

/// Strip wiki markup and collapse whitespace. Keeps letters, Han and placeholders.
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        if let Some(open) = rest.find("{{")
            && let Some(close) = rest[open..].find("}}")
        {
            out.push_str(&rest[..open]);
            out.push_str(&inline_template(&rest[open + 2..open + close]));
            rest = &rest[open + close + 2..];
            continue;
        }
        out.push_str(rest);
        break;
    }

    let no_links = strip_links(&out);
    let mut cleaned = String::with_capacity(no_links.len());
    let mut chars = no_links.chars().peekable();
    let mut in_tag = false;
    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            // '' italic, ''' bold — any run of two or more apostrophes is markup.
            '\'' if chars.peek() == Some(&'\'') => {
                while chars.peek() == Some(&'\'') {
                    chars.next();
                }
            }
            _ => cleaned.push(c),
        }
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What an inline template contributes to the compared text.
///
/// `{{SIC|as printed|what was meant}}` marks a misprint in the **1895 original**: the
/// transcriber kept the wrong word and recorded the correction beside it. Used 154 times.
/// This project stores the print as written, so the first parameter is the one that
/// corresponds to our text — dropping the whole call would delete the word and report a
/// column break that is not there.
///
/// Everything else — `{{?|…}}` shape notes, running heads, rules, column markup — is an
/// editor's apparatus and contributes nothing.
fn inline_template(inner: &str) -> String {
    let mut parts = inner.split('|');
    let name = match parts.next() {
        Some(n) => n.trim(),
        None => return String::new(),
    };
    if name.eq_ignore_ascii_case("sic") {
        return match parts.next() {
            Some(printed) => printed.trim().to_owned(),
            None => String::new(),
        };
    }
    String::new()
}

fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find("[[") {
        let Some(close) = rest[open..].find("]]") else {
            break;
        };
        out.push_str(&rest[..open]);
        let inner = &rest[open + 2..open + close];
        // `[[target|shown]]` displays the part after the bar.
        // rsplit always yields at least one piece, so the fallback is unreachable, not silent.
        match inner.rsplit('|').next() {
            Some(shown) => out.push_str(shown),
            None => out.push_str(inner),
        }
        rest = &rest[open + close + 2..];
    }
    out.push_str(rest);
    out
}

/// Every placeholder mark, of either column and any shape, reduced to one token.
///
/// The two editions do not agree on the mark, so comparing marks would report a difference
/// on nearly every line. Comparing *positions* asks the question that actually matters:
/// does the other edition break the Han column and the Quốc ngữ column in the same place?
pub fn placeholder_key(s: &str) -> String {
    let flat = plain(s);
    let mut out = String::with_capacity(flat.len());
    let mut in_run = false;
    for c in flat.chars() {
        // A RUN of marks collapses to one token. The 2026 print writes the reading
        // placeholder as `―`, `—`, `–`, and on 24 lines as two joined hyphens standing for a
        // SINGLE placeholder; Wikisource writes a bare `-` for all of them and for the
        // Han-column `|` as well. With the shapes this far apart, `―-` against `--` cannot be
        // told from one mark against two, so counting marks in a run would report a column
        // break on lines where both editions in fact break in the same place.
        if matches!(c, '|' | '\u{2015}' | '\u{2014}' | '\u{2013}' | '-') {
            if !in_run {
                out.push('@');
                in_run = true;
            }
            continue;
        }
        in_run = false;
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A looser key for finding the same line in the other edition: placeholders folded,
/// punctuation dropped, case ignored.
pub fn fold(s: &str) -> String {
    let k = placeholder_key(s);
    let mut out = String::with_capacity(k.len());
    for c in k.chars() {
        if matches!(
            c,
            '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '"' | '\'' | '«' | '»'
        ) {
            out.push(' ');
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
