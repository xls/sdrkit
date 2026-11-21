use sdrkit_proto_bcdata::spi::*;

#[test]
fn length_forms_and_tokens() {
    // 0xFE and 16 bits.
    let long = "x".repeat(300);
    let e = write::text(0x12, &long);
    assert_eq!(&e[..2], &[0x12, 0xFE]);
    assert_eq!(decode(&e).unwrap().text, long);
    // A token table at the top level: token 1 is "Radio ".
    let tokens = [&[0x04u8, 8, 1, 6][..], b"Radio "].concat();
    let name = write::element(0x10, &[], &[[&[0x01u8, 4, 1][..], b"One"].concat()]);
    let root = [
        &[0x03u8, (tokens.len() + name.len()) as u8][..],
        &tokens,
        &name,
    ]
    .concat();
    let r = decode(&root).unwrap();
    assert_eq!(r.children[0].text, "Radio One");
}

#[test]
fn timepoint_of_clause_5_4_5() {
    let t = 1_700_000_000;
    let b = write::timepoint(t);
    assert_eq!(timepoint(&b).unwrap(), t);
    // MJD 60000 (2023-02-25), short form, 13:45.
    assert_eq!(
        timepoint(&[0x3A, 0x98, 0x03, 0x6D]).unwrap(),
        1_677_283_200 + 13 * 3600 + 45 * 60
    );
    assert!(timepoint(&[0, 0, 8, 0]).is_err());
}

#[test]
fn service_and_programme_information() {
    let svc = write::element(
        0x28,
        &[],
        &[
            write::text(0x10, "Melody"),
            write::text(0x12, "Test Melody"),
            write::element(0x29, &[(0x80, write::bearer(0xE1, 0xC0DE, 0xC103, 0))], &[]),
        ],
    );
    let si = write::element(
        0x03,
        &[],
        &[write::element(
            0x26,
            &[(0x80, vec![0xE1, 0xC0, 0xDE])],
            &[svc],
        )],
    );
    let mut g = Guide::default();
    g.add(&decode(&si).unwrap());
    assert_eq!(g.services[0].names[2], "Test Melody");
    assert_eq!(g.services[0].bearers, vec![(0xC0DE, 0xC103, 0)]);
    let t = write::element(
        0x2C,
        &[
            (0x80, write::timepoint(1_700_000_000)),
            (0x81, 3600u16.to_be_bytes().to_vec()),
        ],
        &[],
    );
    let prog = write::element(
        0x1C,
        &[(0x80, b"crid://a/1".to_vec())],
        &[
            write::text(0x11, "Morning"),
            write::element(0x19, &[], &[t]),
        ],
    );
    let pi = write::element(0x02, &[], &[write::element(0x21, &[], &[prog])]);
    g.add(&decode(&pi).unwrap());
    g.add(&decode(&pi).unwrap());
    assert_eq!(g.programmes.len(), 1);
    assert_eq!(
        (
            g.programmes[0].names[1].as_str(),
            g.programmes[0].start,
            g.programmes[0].duration_s
        ),
        ("Morning", 1_700_000_000, 3600)
    );
}

#[test]
fn bad_and_deep_input() {
    assert!(decode(&[0x02, 10, 0x10]).is_err());
    let mut deep = vec![0x10, 0];
    for _ in 0..40 {
        let mut e = vec![0x10];
        write::push_len(&mut e, deep.len());
        e.extend(deep);
        deep = e;
    }
    assert!(decode(&deep).is_err());
}

#[test]
fn the_guide_caps_services_and_groups() {
    let svc = |i: u16| {
        write::element(
            0x28,
            &[],
            &[
                write::text(0x12, &format!("Service {i}")),
                write::element(0x29, &[(0x80, write::bearer(0xE1, 0xC0DE, i, 0))], &[]),
            ],
        )
    };
    let list: Vec<Vec<u8>> = (0..300).map(svc).collect();
    let mut g = Guide::default();
    g.add(&decode(&write::element(0x03, &[], &list)).unwrap());
    assert_eq!(g.services.len(), Guide::MAX_SERVICES);
    assert_eq!(g.services[0].names[2], "Service 0");
    for i in 0..300 {
        let group = write::element(0x23, &[], &[write::text(0x12, &format!("Group {i}"))]);
        let pi = write::element(0x02, &[], &[write::element(0x20, &[], &[group])]);
        g.add(&decode(&pi).unwrap());
    }
    assert_eq!(g.groups.len(), Guide::MAX_GROUPS);
    assert_eq!(g.groups.last().map(String::as_str), Some("Group 299"));
    assert_eq!(g.groups[0], "Group 44");
}
