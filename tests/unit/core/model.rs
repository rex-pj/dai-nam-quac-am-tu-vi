//! The domain value objects. Every number here comes from measuring the actual PDF and
//! spreadsheet in docs/, not from an assumption.

use dnqatv_core::model::page::PDF_PAGE_COUNT;
use dnqatv_core::model::{
    GlyphChar, GlyphKind, Letter, PageAddress, PdfPage, Pos, PrintedPage, Reading,
};

// ── Reading ──────────────────────────────────────────────────────────────────

#[test]
fn reading_rejects_an_empty_string() {
    assert!(Reading::parse("").is_err());
    assert!(Reading::parse("   ").is_err());
}

#[test]
fn reading_keeps_tone_distinctions() {
    // Ả, Á and À are THREE different entries in the book — they must not be merged.
    let a = Reading::parse("Ả").expect("Ả");
    let b = Reading::parse("Á").expect("Á");
    let c = Reading::parse("À").expect("À");
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
    // but folding brings them to one form
    assert_eq!(a.normalized(), b.normalized());
    assert_eq!(b.normalized(), c.normalized());
}

#[test]
fn reading_accepts_nfd_input() {
    let nfd: String = [0x004C, 0x006F, 0x0303, 0x006D]
        .iter()
        .filter_map(|c| char::from_u32(*c))
        .collect();
    assert_eq!(Reading::parse(&nfd).expect("nfd").as_str(), "Lõm");
}

// ── Letter ───────────────────────────────────────────────────────────────────

#[test]
fn the_book_has_exactly_22_letter_sections_in_printed_order() {
    let labels: Vec<&str> = Letter::ALL.iter().map(|l| l.label()).collect();
    assert_eq!(
        labels,
        vec![
            "A", "B", "C", "D", "Đ", "E", "G", "H", "Y", "K", "L", "M", "N", "O", "P", "Q", "R",
            "S", "T", "U", "V", "X"
        ],
        "the order must follow the print, not the Latin alphabet"
    );
    assert_eq!(Letter::ALL.len(), 22);
}

#[test]
fn the_book_has_no_f_j_w_or_z_section() {
    for c in ['f', 'j', 'w', 'z', 'F', 'J', 'W', 'Z'] {
        assert!(
            Letter::from_initial(c).is_err(),
            "{c} is not a section of the book"
        );
    }
}

#[test]
fn readings_starting_with_i_live_under_the_y_section() {
    // Measured over all 7,007 entries: Ị, Ich, Im, In, Ít… all live under CHỮ Y.
    for c in ['I', 'Í', 'Ì', 'Ỉ', 'Ĩ', 'Ị', 'i'] {
        assert_eq!(
            Letter::from_initial(c).expect("i"),
            Letter::Y,
            "initial {c}"
        );
    }
    for c in ['Y', 'Ý', 'Ỳ', 'Ỷ', 'Ỹ', 'Ỵ'] {
        assert_eq!(
            Letter::from_initial(c).expect("y"),
            Letter::Y,
            "initial {c}"
        );
    }
}

#[test]
fn d_with_stroke_is_its_own_section_not_folded_into_d() {
    assert_eq!(Letter::from_initial('Đ').expect("D stroke"), Letter::Đ);
    assert_eq!(Letter::from_initial('đ').expect("d stroke"), Letter::Đ);
    assert_eq!(Letter::from_initial('D').expect("D"), Letter::D);
    assert_ne!(Letter::Đ, Letter::D);
}

#[test]
fn accented_vowels_fold_into_the_right_section() {
    let cases = [
        ('Ă', Letter::A),
        ('Â', Letter::A),
        ('Ạ', Letter::A),
        ('Ấ', Letter::A),
        ('Ê', Letter::E),
        ('Ệ', Letter::E),
        ('Ô', Letter::O),
        ('Ơ', Letter::O),
        ('Ợ', Letter::O),
        ('Ư', Letter::U),
        ('Ự', Letter::U),
    ];
    for (c, want) in cases {
        assert_eq!(
            Letter::from_initial(c).expect("initial"),
            want,
            "initial {c}"
        );
    }
}

#[test]
fn collation_rank_increases_in_book_order() {
    for pair in Letter::ALL.windows(2) {
        assert!(
            pair[0].collation_rank() < pair[1].collation_rank(),
            "{:?} must come before {:?}",
            pair[0],
            pair[1]
        );
    }
    // Y comes BEFORE K, exactly as printed — standard Vietnamese collation would reverse it.
    assert!(Letter::Y.collation_rank() < Letter::K.collation_rank());
}

#[test]
fn letter_db_values_round_trip() {
    for l in Letter::ALL {
        assert_eq!(Letter::from_db_value(l.db_value()).expect("db"), l);
    }
}

#[test]
fn a_reading_knows_which_letter_section_it_belongs_to() {
    assert_eq!(
        Reading::parse("Lõm").expect("r").letter().expect("l"),
        Letter::L
    );
    assert_eq!(
        Reading::parse("Ít").expect("r").letter().expect("l"),
        Letter::Y
    );
    assert_eq!(
        Reading::parse("Đèo").expect("r").letter().expect("l"),
        Letter::Đ
    );
}

// ── Pos ──────────────────────────────────────────────────────────────────────

#[test]
fn pos_has_three_variants_and_round_trips() {
    assert_eq!(Pos::ALL.len(), 3);
    for p in Pos::ALL {
        assert_eq!(Pos::from_book_label(p.book_label()).expect("label"), p);
        assert_eq!(Pos::from_db_value(p.db_value()).expect("db"), p);
    }
    assert_eq!(Pos::ChuNho.book_label(), "c.");
    assert_eq!(Pos::ChuNom.book_label(), "n.");
    assert_eq!(Pos::ChuNhoDungNom.book_label(), "cn.");
}

#[test]
fn pos_rejects_an_unknown_label_instead_of_guessing() {
    for bad in ["c", "n", "C.", "cn", "", "adj.", "(c.)"] {
        assert!(
            Pos::from_book_label(bad).is_err(),
            "{bad:?} must be rejected"
        );
    }
}

// ── Trang ────────────────────────────────────────────────────────────────────

#[test]
fn three_pages_have_no_printed_number() {
    // Measured over all 1038 pages: the 2026 cover, the original cover, and the LƯU Ý page.
    for n in [1u16, 2, 10] {
        assert_eq!(
            PdfPage::new(n).expect("page").printed(),
            None,
            "trang PDF {n}"
        );
    }
}

#[test]
fn front_matter_printed_numbers_equal_the_pdf_page_numbers() {
    for n in 3u16..=9 {
        let printed = PdfPage::new(n).expect("page").printed().expect("printed");
        assert_eq!(printed.get(), n, "trang PDF {n}");
    }
}

#[test]
fn the_book_body_is_offset_by_exactly_one_page() {
    // The LƯU Ý page (PDF 10) was inserted by the 2026 edition; it causes the offset.
    for (pdf, want) in [
        (11u16, 10u16),
        (12, 11),
        (300, 299),
        (500, 499),
        (1038, 1037),
    ] {
        let printed = PdfPage::new(pdf).expect("page").printed().expect("printed");
        assert_eq!(printed.get(), want, "trang PDF {pdf}");
    }
}

#[test]
fn page_conversion_round_trips() {
    for pdf in [3u16, 9, 11, 500, 1038] {
        let p = PdfPage::new(pdf).expect("page");
        let back = p.printed().expect("printed").pdf();
        assert_eq!(back, p, "trang PDF {pdf}");
    }
}

#[test]
fn a_page_out_of_range_is_rejected() {
    assert!(PdfPage::new(0).is_err());
    assert!(PdfPage::new(1039).is_err());
    assert!(PdfPage::new(u16::MAX).is_err());
}

#[test]
fn only_the_numbers_the_print_carries_are_printed_pages() {
    // The original's first two leaves are unnumbered, so printed 1 and 2 name no page: a
    // reader asking for them is asking for a page the book never numbered.
    for n in [0u16, 1, 2, 1038, u16::MAX] {
        assert!(PrintedPage::new(n).is_err(), "số in {n}");
    }
    for n in [3u16, 9, 10, 500, 1037] {
        assert_eq!(PrintedPage::new(n).expect("printed").get(), n);
    }
}

#[test]
fn every_page_has_exactly_one_address() {
    // The address is what `/trang/{n}` carries. It must round-trip for all 1038 pages …
    for n in 1..=PDF_PAGE_COUNT {
        let page = PdfPage::new(n).expect("page");
        let printed = page.address().to_string();
        assert_eq!(
            PageAddress::parse(&printed).expect("address").pdf_page(),
            page,
            "trang PDF {n}"
        );
    }
    // … and the three unnumbered pages are the only ones spelled with the `pdf-` prefix.
    assert_eq!(
        PdfPage::new(10).expect("page").address().to_string(),
        "pdf-10"
    );
    assert_eq!(PdfPage::new(11).expect("page").address().to_string(), "10");
    assert_eq!(PdfPage::new(3).expect("page").address().to_string(), "3");
}

#[test]
fn a_page_that_has_a_printed_number_has_no_second_address() {
    // Two spellings of one page split its inbound links and its search ranking.
    for segment in ["pdf-3", "pdf-9", "pdf-11", "pdf-1038"] {
        assert!(PageAddress::parse(segment).is_err(), "{segment}");
    }
    // Nor is anything that is not a page number a page.
    for segment in ["", "-1", "pdf-", "pdf-0", "pdf-x", "mười", "10.5", " 10"] {
        assert!(PageAddress::parse(segment).is_err(), "{segment:?}");
    }
}

#[test]
fn body_pages_are_distinguishable() {
    assert!(!PdfPage::new(10).expect("p").is_body());
    assert!(PdfPage::new(11).expect("p").is_body());
}

// ── Glyphs ───────────────────────────────────────────────────────────────────

#[test]
fn a_glyph_must_be_exactly_one_character() {
    assert!(GlyphChar::parse("阿").is_ok());
    assert!(GlyphChar::parse("阿意").is_err());
    assert!(GlyphChar::parse("").is_err());
}

#[test]
fn glyphs_are_classified_by_code_point() {
    let bmp = GlyphChar::parse("阿").expect("bmp");
    assert_eq!(bmp.kind(), GlyphKind::Bmp);
    assert_eq!(bmp.codepoint(), 0x963F);

    // 𨰲 (Lõm) is CJK Ext-B — one of the 850 characters at U+20000 and above.
    let ext = GlyphChar::parse("𨰲").expect("ext");
    assert_eq!(ext.kind(), GlyphKind::ExtB);
    assert!(ext.codepoint() >= 0x20000);

    // U+F0036 is one of the 26 PUA characters measured in the entry index (reading "Bạm").
    let pua_char = char::from_u32(0xF0036).expect("pua");
    let pua = GlyphChar::parse(&pua_char.to_string()).expect("pua");
    assert_eq!(pua.kind(), GlyphKind::Pua);
}

#[test]
fn only_image_glyphs_have_no_unicode() {
    assert!(GlyphKind::Bmp.has_unicode());
    assert!(GlyphKind::ExtB.has_unicode());
    assert!(GlyphKind::Pua.has_unicode());
    assert!(!GlyphKind::ImageOnly.has_unicode());
}
