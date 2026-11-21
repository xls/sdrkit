//! DVB text through this crate is the text of `sdrkit_util_text::dvb`. Each case failed with the copy that was here.

use sdrkit_proto_mpegts::si::dvb_text;
use sdrkit_proto_mpegts::tables::{self, build_sdt};
use sdrkit_util_text::dvb::{Options, Usage};

fn name(b: &[u8]) -> String {
    dvb_text(b, Usage::Name, &Options::default())
}

#[test]
fn unassigned_iso_6937_positions_are_not_characters() {
    // ISO/IEC 6937 (EN 300 468 Figure A.1): 0xA6, 0xC9 and 0xCC have no character.
    assert_eq!(name(b"A\xA6B\xC9C\xCCD"), "ABCD");
}

#[test]
fn utf8_text_keeps_its_control_codes() {
    // EN 300 468 Table A.2: in UTF-8, U+0086 / U+0087 are emphasis on and off, U+008A is CR/LF.
    let mut b = vec![0x15];
    b.extend_from_slice("line\u{8A}two".as_bytes());
    assert_eq!(name(&b), "line two");
    assert_eq!(
        dvb_text(&b, Usage::Description, &Options::default()),
        "line\ntwo"
    );
}

#[test]
fn a_mark_without_a_precomposed_form_stays() {
    // 0xC8 is the diaeresis. "q" with a diaeresis has no precomposed character: it stays as q and U+0308.
    assert_eq!(name(b"\xC8q"), "q\u{308}");
}

#[test]
fn sdt_names_use_the_same_decoder() {
    let s = build_sdt(1, 1, 0, 0, 0, &[(1, 1, "sdrkit", "TV\u{A6}")]);
    let h = tables::check(&s).unwrap();
    // "TV" followed by U+00A6 written as UTF-8 without a selector: bytes 0xC2 0xA6 read as ISO 6937: 0xC2 is the acute accent,
    // 0xA6 is unassigned.
    let sdt = tables::sdt(&s, &h, &Options::default()).unwrap();
    assert_eq!(sdt.services[0].name, "TV");
}
