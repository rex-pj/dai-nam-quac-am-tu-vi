//! Reading the fields out of one wiki template call.
//!
//! A naive `split('|')` tears the data apart, because templates nest. Where the print used
//! a character with no Unicode code point, the transcriber wrote a description **inside**
//! the glyph field:
//!
//! ```text
//! {{DNQATV/mục|{{?|trên:壯, dưới:卵}}|Trấng||n|…}}
//! ```
//!
//! Splitting that on `|` yields `{{?` and `trên:壯, dưới:卵}}` as two fields and shifts every
//! later field by one — silently turning the reading into the label. So the scanner tracks
//! brace depth and only treats `|` as a separator at depth 1.

/// One template call found in the page text: its fields, and where it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCall {
    /// The fields after the template name, in order, untrimmed.
    pub fields: Vec<String>,
    /// Byte offset of the opening `{{` — used to keep sub-entries with the headword above them.
    pub at: usize,
}

impl TemplateCall {
    /// Field `n`, trimmed. `None` when the call has no such field.
    pub fn field(&self, n: usize) -> Option<&str> {
        self.fields.get(n).map(|f| f.trim())
    }
}

/// Every call of `{{name|…}}` in `text`, in document order.
///
/// An unterminated call at the end of the page is returned with the fields read so far
/// rather than dropped: losing a headword silently is worse than reporting a short one.
pub fn template_fields(text: &str, name: &str) -> Vec<TemplateCall> {
    let open = format!("{{{{{name}|");
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut search = 0usize;

    while let Some(rel) = text.get(search..).and_then(|t| t.find(&open)) {
        let start = search + rel;
        let mut depth = 0usize;
        let mut fields: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut i = start;

        // A piped wikilink brings its own `|`, and it is not a field separator. Eight glyph
        // fields in the snapshot are written `[[wikt:阿|阿]]`, and splitting there shifted
        // every later field along: the reading came out `阿]]` and eight rows of
        // `witness-entry-only-there.toml` were an artefact of this parser, not a difference
        // between the two editions.
        let mut link_depth = 0usize;

        while i < bytes.len() {
            let rest = &text[i..];
            if rest.starts_with("[[") {
                link_depth += 1;
                cur.push_str("[[");
                i += 2;
                continue;
            }
            if rest.starts_with("]]") && link_depth > 0 {
                link_depth -= 1;
                cur.push_str("]]");
                i += 2;
                continue;
            }
            if rest.starts_with("{{") {
                depth += 1;
                if depth > 1 {
                    cur.push_str("{{");
                }
                i += 2;
                continue;
            }
            if rest.starts_with("}}") {
                depth -= 1;
                if depth == 0 {
                    fields.push(std::mem::take(&mut cur));
                    i += 2;
                    break;
                }
                cur.push_str("}}");
                i += 2;
                continue;
            }
            // Only a separator at the top level; inside a nested call or a piped wikilink it
            // belongs to that construct.
            if rest.starts_with('|') && depth == 1 && link_depth == 0 {
                fields.push(std::mem::take(&mut cur));
                i += 1;
                continue;
            }
            let Some(c) = rest.chars().next() else { break };
            cur.push(c);
            i += c.len_utf8();
        }
        if !cur.is_empty() {
            fields.push(cur);
        }
        if !fields.is_empty() {
            fields.remove(0); // the template name itself
        }
        out.push(TemplateCall { fields, at: start });
        search = i.max(start + open.len());
    }
    out
}
