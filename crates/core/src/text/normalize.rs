//! Unicode normalization — the ONLY place in the whole system that performs NFC.
//!
//! Why it exists: accented Vietnamese lives in both NFC (precomposed) and NFD (decomposed),
//! and the two forms are NOT byte-equal. Text copied from macOS is usually NFD. Scattering
//! normalization across the codebase guarantees one site forgets it, and search then misses
//! in ways that are hard to trace.

use unicode_normalization::UnicodeNormalization;

/// Convert to NFC form.
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Whether the string is already in NFC form.
pub fn is_nfc(s: &str) -> bool {
    s.nfc().eq(s.chars())
}

/// Vietnamese diacritics live entirely within the Combining Diacritical Marks block
/// (U+0300..U+036F): grave U+0300, acute U+0301, tilde U+0303, hook above U+0309,
/// dot below U+0323, circumflex U+0302, breve U+0306, horn U+031B.
fn is_combining_diacritic(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F)
}

/// Strip diacritics and lowercase, for accent-insensitive lookup.
///
/// `đ` is NOT `d` plus a combining mark — it is its own character U+0111 that NFD does not
/// decompose, so it needs explicit handling. This is exactly where generic deaccenting
/// libraries get Vietnamese wrong.
pub fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_diacritic(*c))
        .map(|c| match c {
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}
