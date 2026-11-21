//! The LCT header: RFC 5651 clause 5.1 (figure 1) and clause 5.2.2 (EXT_TIME), RFC 9223 clause 4.4 for ROUTE.

use sdrkit_proto_ipcast::lct::{Lct, LctBuilder};

#[test]
fn default_header_field_by_field() {
    // V = 1, C = 0, PSI = 10, S = 1, O = 01, H = 0, A = 0, B = 1, HDR_LEN 4 words, CP 8, then the 32-bit CCI, TSI and TOI.
    let b = [
        0x12, 0xA1, 0x04, 0x08, 0xCC, 0xCC, 0xCC, 0xCC, 0, 0, 0, 5, 0, 0, 0, 9, 0, 0, 0, 0,
    ];
    let h = Lct::parse(&b).unwrap();
    assert_eq!((h.tsi, h.toi, h.codepoint, h.len), (5, 9, 8, 16));
    assert_eq!(
        h.cci, 0xCCCC_CCCC,
        "the CCI comes before the TSI (the tvterr copy read it as the TSI)"
    );
    assert!(h.source && h.close_object && !h.close_session);
}

#[test]
fn psi_most_significant_bit_marks_source_packets() {
    // RFC 9223 clause 4.4: SPI = 1 is a source packet, whatever the other PSI bit.
    let mut b = [0x10u8, 0xA0, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1];
    for (psi, src) in [(0u8, false), (1, false), (2, true), (3, true)] {
        b[0] = 0x10 | psi;
        assert_eq!(Lct::parse(&b).unwrap().source, src, "PSI {psi}");
    }
}

#[test]
fn extensions_tol_and_time() {
    let mut p = Vec::new();
    LctBuilder {
        tsi: 1,
        toi: 2,
        codepoint: 5,
        transfer_len: Some(5_000_000_000),
        close_object: false,
    }
    .write(&mut p)
    .unwrap();
    assert_eq!(
        Lct::parse(&p).unwrap().transfer_len,
        Some(5_000_000_000),
        "48-bit EXT_TOL (HET 67)"
    );
    let mut p = Vec::new();
    LctBuilder {
        tsi: 1,
        toi: 2,
        codepoint: 5,
        transfer_len: Some(70_000),
        close_object: false,
    }
    .write(&mut p)
    .unwrap();
    assert_eq!(p[16], 194, "24-bit EXT_TOL");
    assert_eq!(Lct::parse(&p).unwrap().transfer_len, Some(70_000));
    // EXT_TIME (HET 2, HEL 3): Use = SCT-High and SCT-Low, then the two words of a 64-bit NTP time.
    let b = [
        0x10, 0xA0, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 2, 3, 0xC0, 0, 0xE6, 0xB0, 0x4E,
        0x81, 0x80, 0, 0, 0,
    ];
    let h = Lct::parse(&b).unwrap();
    assert_eq!(h.sender_time, Some(0xE6B0_4E81_8000_0000));
    // HEL = 0 and a header past the packet are refused.
    let mut bad = b;
    bad[17] = 0;
    assert!(Lct::parse(&bad).is_err());
    assert!(Lct::parse(&b[..20]).is_err());
    let mut v2 = b;
    v2[0] = 0x20;
    assert!(Lct::parse(&v2).is_err());
}
