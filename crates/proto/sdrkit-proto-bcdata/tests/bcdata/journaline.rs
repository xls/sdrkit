use sdrkit_proto_bcdata::journaline::*;

#[test]
fn header_layout_and_round_trip() {
    let msg = Object {
        id: 0x0102,
        kind: Kind::Text,
        is_static: true,
        revision: 5,
        title: "Sport".into(),
        body: "Results\ntonight".into(),
        ..Object::default()
    };
    let mut b = Vec::new();
    encode(&msg, false, &mut b);
    // Object id, then type 2, static 1, compressed 0, revision 5.
    assert_eq!(&b[..4], &[0x01, 0x02, 0x55, 0x01]);
    assert_eq!(decode(&b, 0).unwrap(), msg);
    let list = Object {
        id: 3,
        kind: Kind::List,
        title: "Prices".into(),
        items: vec!["Gold\t1900".into(), "Oil\t80".into()],
        ..Object::default()
    };
    for c in [false, true] {
        let mut b = Vec::new();
        encode(&list, c, &mut b);
        assert_eq!(decode(&b, 0).unwrap(), list);
    }
}

#[test]
fn extended_header_zlib_and_data_sections() {
    // A menu with a 2-byte extended header, sent as a zlib stream, with a data section in its title.
    let content = [
        &[0x01u8][..],
        b"Ma",
        &[0x1A, 1, 0xEE, 0xEE],
        b"in",
        &[0x02, 0x00, 0x10],
        b"Sport",
        &[0x00],
    ]
    .concat();
    let mut b = vec![0x00, 0x00, (1 << 5) | 0x08, 0xAA, 0xBB];
    b.extend(miniz_oxide::deflate::compress_to_vec_zlib(&content, 6));
    let o = decode(&b, 2).unwrap();
    assert_eq!(o.title, "Main");
    assert_eq!(o.links, vec![(0x10, "Sport".to_string())]);
}

#[test]
fn bad_objects() {
    assert!(decode(&[0, 1], 0).is_err());
    assert!(decode(&[0, 1, 0x00], 0).is_err()); // type 0
    assert!(decode(&[0, 1, (1 << 5) | 0x08, 1, 2, 3], 0).is_err()); // bad deflate
    assert!(decode(&[0, 1, 1 << 5, 0x02, 0x00], 0).is_err()); // a cut link
    let mut s = Store::default();
    let o = Object {
        id: 1,
        ..Object::default()
    };
    assert!(s.add(o.clone()));
    assert!(!s.add(o));
    assert_eq!(s.received, 2);
}
