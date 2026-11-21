use sdrkit_proto_bcdata::packet::*;

fn groups(a: &mut DabAssembler, bytes: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let mut v = Vec::new();
    a.push(bytes, &mut |addr, g| v.push((addr, g.to_vec())));
    v
}

#[test]
fn dab_header_layout_of_clause_5_3_2() {
    let mut w = DabPacketWriter::new(0x123).unwrap();
    let mut p = Vec::new();
    w.write_group(&[0xAA; 10], &mut p).unwrap();
    // 24 bytes: length 00, CI 0, first 1, last 1, address 0x123. Command 0, useful 10. Data padded to 19. CRC.
    assert_eq!(p.len(), 24);
    assert_eq!(&p[..3], &[0x0D, 0x23, 10]);
    assert!(p[13..22].iter().all(|&b| b == 0));
    assert_eq!(u16::from_be_bytes([p[22], p[23]]), crate::genibus(&p[..22]));
    let h = DabHeader::parse(&p).unwrap();
    assert_eq!(
        (h.len, h.first, h.last, h.address, h.useful),
        (24, true, true, 0x123, 10)
    );
    // A 96-byte packet: length code 3.
    let mut p = Vec::new();
    w.write_group(&[1; 91], &mut p).unwrap();
    assert_eq!((p.len(), p[0] >> 6, (p[0] >> 4) & 3), (96, 3, 1));
}

#[test]
fn dab_groups_across_packets_lengths_and_cuts() {
    let mut w = DabPacketWriter::new(17).unwrap();
    let mut w2 = DabPacketWriter::new(18).unwrap();
    let g1: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
    let g2 = vec![5u8; 30];
    let mut s = Vec::new();
    w.write_group(&g1, &mut s).unwrap();
    dab_padding(&mut s);
    w2.write_group(&g2, &mut s).unwrap();
    for cut in [1, 7, 24, 100, s.len()] {
        let mut a = DabAssembler::new();
        let mut got = Vec::new();
        for c in s.chunks(cut) {
            got.extend(groups(&mut a, c));
        }
        assert_eq!(got, vec![(17, g1.clone()), (18, g2.clone())], "cut {cut}");
        assert_eq!(a.stats.padding, 1);
    }
}

#[test]
fn dab_lost_packet_drops_the_group() {
    let mut w = DabPacketWriter::new(17).unwrap();
    let mut s = Vec::new();
    w.write_group(&[3u8; 200], &mut s).unwrap();
    let mut a = DabAssembler::new();
    let mut damaged = s.clone();
    damaged[100] ^= 0x40;
    assert!(groups(&mut a, &damaged).is_empty());
    assert_eq!(a.stats.packets_bad, 1);
    assert_eq!(a.stats.groups_dropped, 1);
    assert_eq!(groups(&mut a, &s).len(), 1);
}

#[test]
fn dab_fec_corrects_bytes() {
    let mut w = DabPacketWriter::new(17).unwrap();
    let group: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
    let mut adt = Vec::new();
    w.write_group(&group, &mut adt).unwrap();
    while adt.len() < FEC_ADT {
        dab_padding(&mut adt);
    }
    adt.truncate(FEC_ADT);
    let fec = DabFec::new();
    let mut frame = adt.clone();
    fec.encode(&adt, &mut frame).unwrap();
    assert_eq!(frame.len(), FEC_FRAME);
    // Two frames: the second carries the first group again.
    let mut stream = frame.clone();
    stream.extend_from_slice(&frame);
    // Bytes in different rows of the table: each row corrects up to 8.
    for i in [100, 500, 501, 900, 2000, 2471, 2472 + 30] {
        stream[i] ^= 0xFF;
    }
    let mut d = DabFec::new();
    let mut got = Vec::new();
    for c in stream.chunks(96) {
        d.push(c, &mut |a, g| got.push((a, g.to_vec())));
    }
    assert_eq!(got, vec![(17, group.clone()), (17, group)]);
    assert!(d.stats().fec_corrected >= 6);
    assert_eq!(d.stats().fec_frames, 2);
    assert!(fec.encode(&adt[1..], &mut Vec::new()).is_err());
}

#[test]
fn drm_header_layout_of_clause_6_6_1() {
    let mut w = DrmPacketWriter::new(1, 10).unwrap();
    let mut s = Vec::new();
    w.write_unit(b"ABCDEFGHIJ", &mut s).unwrap();
    // Data field 7 bytes: first packet full (first 1, last 0, id 1, padded 0, CI 0), then 3 bytes padded (length byte 3).
    assert_eq!(s.len(), 20);
    assert_eq!(s[0], 0x90);
    assert_eq!(&s[1..8], b"ABCDEFG");
    assert_eq!(u16::from_be_bytes([s[8], s[9]]), crate::genibus(&s[..8]));
    assert_eq!(s[10], 0x40 | 0x10 | 0x08 | 1);
    assert_eq!(&s[11..15], &[3, b'H', b'I', b'J']);
}

#[test]
fn drm_units_with_ids_padding_and_loss() {
    let mut w1 = DrmPacketWriter::new(1, 48).unwrap();
    let mut w2 = DrmPacketWriter::new(2, 48).unwrap();
    let unit: Vec<u8> = (0..200u32).map(|i| i as u8).collect();
    let mut s = Vec::new();
    w1.write_unit(&unit, &mut s).unwrap();
    w1.write_padding(&mut s);
    w2.write_unit(b"short", &mut s).unwrap();
    let mut a = DrmAssembler::new(48, None).unwrap();
    let mut got = Vec::new();
    for c in s.chunks(13) {
        a.push(c, &mut |id, u| got.push((id, u.to_vec())));
    }
    assert_eq!(got, vec![(1, unit.clone()), (2, b"short".to_vec())]);
    assert_eq!(a.stats.padding, 1);
    // One id only, and a lost packet.
    let mut a = DrmAssembler::new(48, Some(1)).unwrap();
    let mut s = Vec::new();
    w1.write_unit(&unit, &mut s).unwrap();
    s.drain(48..96);
    w1.write_unit(&unit, &mut s).unwrap();
    let mut got = Vec::new();
    a.push(&s, &mut |id, u| got.push((id, u.to_vec())));
    assert_eq!(got, vec![(1, unit)]);
    assert_eq!(a.stats.groups_dropped, 1);
    assert!(DrmAssembler::new(3, None).is_err());
    assert!(DrmPacketWriter::new(4, 48).is_err());
}

#[test]
fn garbage_does_not_panic() {
    let mut a = DabAssembler::new();
    let mut d = DrmAssembler::new(26, None).unwrap();
    let mut f = DabFec::new();
    let mut x: u32 = 1;
    let junk: Vec<u8> = (0..20_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect();
    a.push(&junk, &mut |_, _| {});
    d.push(&junk, &mut |_, _| {});
    f.push(&junk, &mut |_, _| {});
}
