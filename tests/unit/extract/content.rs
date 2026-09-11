// clippy only relaxes panic/expect inside #[test] functions; the helpers here sit outside.
// This whole file is test code, where panicking on a broken fixture is correct.
#![allow(clippy::expect_used, clippy::panic)]

//! Content-stream tokenizing. The escape cases are taken straight from PDF 32000-1 §7.3.4.

use dnqatv_extract_core::content::{ContentError, Token, tokenize};

fn toks(src: &str) -> Vec<Token> {
    tokenize(src.as_bytes()).expect("tokenize")
}

fn only_strings(src: &str) -> Vec<Vec<u8>> {
    toks(src)
        .into_iter()
        .filter_map(|t| match t {
            Token::Str(b) => Some(b),
            _ => None,
        })
        .collect()
}

// ── Literal strings ──────────────────────────────────────────────────────────

#[test]
fn a_simple_literal_string() {
    assert_eq!(only_strings("(Lom) Tj"), vec![b"Lom".to_vec()]);
}

#[test]
fn control_character_escapes() {
    let got = only_strings(r"(a\nb\tc\rd) Tj");
    assert_eq!(got, vec![b"a\nb\tc\rd".to_vec()]);
}

#[test]
fn parenthesis_and_backslash_escapes() {
    let got = only_strings(r"(a\(b\)c\\d) Tj");
    assert_eq!(got, vec![br"a(b)c\d".to_vec()]);
}

#[test]
fn balanced_nested_parentheses_need_no_escape() {
    // §7.3.4.2 allows nested parentheses when balanced.
    assert_eq!(only_strings("((x)) Tj"), vec![b"(x)".to_vec()]);
    assert_eq!(only_strings("(a(b(c))d) Tj"), vec![b"a(b(c))d".to_vec()]);
}

#[test]
fn octal_codes() {
    assert_eq!(only_strings(r"(\101\102\103) Tj"), vec![b"ABC".to_vec()]);
    // At most three digits: \1010 is \101 followed by the character '0'.
    assert_eq!(only_strings(r"(\1010) Tj"), vec![b"A0".to_vec()]);
    // Fewer than three digits is valid too.
    assert_eq!(only_strings(r"(\7) Tj"), vec![vec![7u8]]);
}

#[test]
fn an_octal_code_above_255_drops_its_high_digit_per_the_spec() {
    // \400 = 256 -> §7.3.4.2 says drop the high digit, leaving 0.
    assert_eq!(only_strings(r"(\400) Tj"), vec![vec![0u8]]);
}

#[test]
fn a_backslash_line_continuation_emits_no_character() {
    let src = "(abc\\\ndef) Tj";
    assert_eq!(only_strings(src), vec![b"abcdef".to_vec()]);
}

#[test]
fn an_unterminated_literal_string_is_an_error() {
    let err = tokenize(b"(chua dong").expect_err("phai loi");
    assert!(
        matches!(err, ContentError::UnterminatedLiteralString(0)),
        "got {err:?}"
    );
}

#[test]
fn binary_bytes_inside_a_string_are_preserved() {
    // Type0 font strings hold arbitrary 2-byte glyph codes, not UTF-8.
    // This is why the tokenizer works on bytes rather than str.
    let src: Vec<u8> = [
        b"(".to_vec(),
        vec![0x51, 0xB6, 0x00, 0x41],
        b") Tj".to_vec(),
    ]
    .concat();
    let got = tokenize(&src).expect("tokenize");
    assert_eq!(got[0], Token::Str(vec![0x51, 0xB6, 0x00, 0x41]));
}

// ── Hex strings ──────────────────────────────────────────────────────────────

#[test]
fn a_basic_hex_string() {
    assert_eq!(only_strings("<48656C6C6F> Tj"), vec![b"Hello".to_vec()]);
}

#[test]
fn an_odd_digit_hex_string_is_padded_per_the_spec() {
    // §7.3.4.3: an odd hex digit count pads a '0' at the end. <414> means <4140>.
    assert_eq!(only_strings("<414> Tj"), vec![vec![0x41, 0x40]]);
}

#[test]
fn a_hex_string_ignores_whitespace() {
    assert_eq!(
        only_strings("<48 65 6C\n6C 6F> Tj"),
        vec![b"Hello".to_vec()]
    );
}

#[test]
fn a_hex_string_with_a_stray_character_is_an_error() {
    let err = tokenize(b"<48ZZ> Tj").expect_err("phai loi");
    assert!(
        matches!(err, ContentError::InvalidHexDigit { byte: b'Z', .. }),
        "got {err:?}"
    );
}

#[test]
fn an_unterminated_hex_string_is_an_error() {
    let err = tokenize(b"<4865").expect_err("phai loi");
    assert!(
        matches!(err, ContentError::UnterminatedHexString(0)),
        "got {err:?}"
    );
}

#[test]
fn a_dictionary_is_not_mistaken_for_a_hex_string() {
    let got = toks("<</Type /Page>> BDC");
    assert_eq!(got[0], Token::DictOpen);
    assert!(
        !got.iter().any(|t| matches!(t, Token::Str(_))),
        "a dictionary must produce no string"
    );
}

// ── Numbers, names, operators ────────────────────────────────────────────────

#[test]
fn the_pdf_number_forms() {
    let got = toks("1 -2 3.5 -.002 4. +7 Tm");
    let nums: Vec<f64> = got.iter().filter_map(Token::as_number).collect();
    assert_eq!(nums, vec![1.0, -2.0, 3.5, -0.002, 4.0, 7.0]);
}

#[test]
fn names_and_operators() {
    let got = toks("/F1 12 Tf");
    assert_eq!(got[0], Token::Name("F1".to_owned()));
    assert_eq!(got[1], Token::Number(12.0));
    assert_eq!(got[2], Token::Operator("Tf".to_owned()));
}

#[test]
fn a_tj_array_keeps_its_order() {
    let got = toks("[(a) -250 (b)] TJ");
    assert_eq!(got[0], Token::ArrayOpen);
    assert_eq!(got[1], Token::Str(b"a".to_vec()));
    assert_eq!(got[2], Token::Number(-250.0));
    assert_eq!(got[3], Token::Str(b"b".to_vec()));
    assert_eq!(got[4], Token::ArrayClose);
    assert_eq!(got[5], Token::Operator("TJ".to_owned()));
}

#[test]
fn comments_are_skipped() {
    let got = toks("% day la chu thich (khong phai chuoi)\n/F1 Tf");
    assert_eq!(got[0], Token::Name("F1".to_owned()));
}

// ── No guessing ──────────────────────────────────────────────────────────────

#[test]
fn an_inline_image_is_a_hard_error_rather_than_a_skip() {
    // Measured: this file has no inline images. But if one appeared, the binary data between
    // ID and EI would be misread as operators and silently produce garbage text.
    let err = tokenize(b"BI /W 16 /H 16 ID xxxx EI").expect_err("phai loi");
    assert!(
        matches!(err, ContentError::InlineImageUnsupported(0)),
        "got {err:?}"
    );
}
