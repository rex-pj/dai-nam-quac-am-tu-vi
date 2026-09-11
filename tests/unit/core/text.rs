//! Text normalization. This is the NFC/NFD guard — a bug hit for real during the survey:
//! filenames under docs/ are NFD, hand-typed strings are NFC, and the two are not equal.

use dnqatv_core::text::{fold, is_nfc, nfc};

/// Build an NFD string explicitly, without the library, so the test does not prove itself.
fn nfd_chars(parts: &[u32]) -> String {
    parts.iter().filter_map(|c| char::from_u32(*c)).collect()
}

#[test]
fn nfd_and_nfc_of_the_same_word_are_equal_after_normalizing() {
    // "Lõm" in NFD: L + o + combining tilde (U+0303) + m
    let nfd = nfd_chars(&[0x004C, 0x006F, 0x0303, 0x006D]);
    let nfc_form = "Lõm";

    assert_ne!(nfd, nfc_form, "the two forms must differ byte-wise");
    assert_eq!(nfc(&nfd), nfc_form);
    assert!(!is_nfc(&nfd));
    assert!(is_nfc(nfc_form));
}

#[test]
fn fold_strips_tone_marks_and_lowercases() {
    for (input, want) in [
        ("Lõm", "lom"),
        ("Lôm", "lom"),
        ("Lốm", "lom"),
        ("Lồm", "lom"),
        ("Lỗm", "lom"),
        ("Ẩm", "am"),
        ("Ượt", "uot"),
    ] {
        assert_eq!(fold(input), want, "fold({input:?})");
    }
}

#[test]
fn fold_handles_the_letter_d_with_stroke() {
    // đ is its own character U+0111, NOT d plus a combining mark, so NFD does not split it.
    // This is where generic deaccenting libraries get Vietnamese wrong.
    assert_eq!(char::from_u32(0x0111), Some('đ'));
    assert_eq!(fold("Đại"), "dai");
    assert_eq!(fold("đèo"), "deo");
    assert_eq!(fold("Đ"), "d");
}

#[test]
fn fold_leaves_unaccented_letters_alone() {
    assert_eq!(fold("Ba"), "ba");
    assert_eq!(fold("gươm"), "guom");
}

#[test]
fn nfc_does_not_damage_han_nom() {
    // Ext-B and PUA must pass through untouched.
    for s in ["阿", "惡", "𨰲"] {
        assert_eq!(nfc(s), s, "NFC changed {s:?}");
    }
}
