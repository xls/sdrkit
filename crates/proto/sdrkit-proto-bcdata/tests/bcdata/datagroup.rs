use sdrkit_proto_bcdata::datagroup::{self, Header};
use sdrkit_proto_bcdata::Error;

#[test]
fn crc_definition_check_value() {
    // The CRC-16/GENIBUS value of "123456789" (CRC RevEng catalogue). The test helper is the CRC of EN 300 401 clause 5.2.1.
    assert_eq!(crate::genibus(b"123456789"), 0xD64E);
}

#[test]
fn header_layout_of_clause_5_3_3() {
    let h = Header {
        group_type: datagroup::MOT_HEADER,
        continuity: 2,
        repetition: 1,
        segment: Some((true, 5)),
        transport_id: Some(0x1234),
        ..Header::default()
    };
    let mut g = Vec::new();
    h.write(b"AB", &mut g).unwrap();
    // Flags: extension 0, CRC 1, segment 1, user access 1, type 3. Then CI 2, RI 1. Segment: last 1, number 5.
    // User access: transport id flag 1, length 2, transport id.
    assert_eq!(
        &g[..9],
        &[0x73, 0x21, 0x80, 0x05, 0x12, 0x12, 0x34, b'A', b'B']
    );
    assert_eq!(u16::from_be_bytes([g[9], g[10]]), crate::genibus(&g[..9]));
    let d = datagroup::parse(&g).unwrap();
    assert_eq!(d.header, h);
    assert_eq!(d.data, b"AB");
    assert!(d.is_last());
    assert_eq!(d.segment_number(), 5);
}

#[test]
fn extension_and_end_user_address() {
    let h = Header {
        extension: Some(0xBEEF),
        transport_id: Some(7),
        crc: false,
        ..Header::default()
    };
    let mut g = Vec::new();
    h.write_with_address(&[9, 8, 7], b"x", &mut g).unwrap();
    assert_eq!(
        &g[..10],
        &[0x90, 0x00, 0xBE, 0xEF, 0x15, 0x00, 0x07, 9, 8, 7]
    );
    let d = datagroup::parse(&g).unwrap();
    assert_eq!(
        (d.header.extension, d.end_user_address, d.data),
        (Some(0xBEEF), &[9u8, 8, 7][..], &b"x"[..])
    );
}

#[test]
fn bad_input_returns_errors() {
    let mut g = Vec::new();
    Header {
        transport_id: Some(1),
        ..Header::default()
    }
    .write(b"data", &mut g)
    .unwrap();
    let mut bad = g.clone();
    bad[5] ^= 1;
    assert_eq!(datagroup::parse(&bad), Err(Error::Crc));
    assert_eq!(datagroup::parse(&[0x00]), Err(Error::Truncated));
    // A user access field that claims 9 bytes in a 4-byte group.
    assert_eq!(
        datagroup::parse(&[0x10, 0x00, 0x09, 0x00]),
        Err(Error::Truncated)
    );
    // A transport id flag with a length of 1.
    assert!(matches!(
        datagroup::parse(&[0x10, 0x00, 0x11, 0x00]),
        Err(Error::Invalid(_))
    ));
    assert!(Header {
        group_type: 16,
        ..Header::default()
    }
    .write(b"", &mut g)
    .is_err());
    for n in 0..g.len() {
        let _ = datagroup::parse(&g[..n]);
    }
}
