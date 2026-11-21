//! ATSC 3.0 baseband packets (A/322 clause 5.2.2) and ALP (A/330 clause 5).

use sdrkit_proto_ipcast::alp::{
    write_bbp_header, write_single, write_ts, AlpEvent, AlpReader, BbpExtension, BbpHeader,
    PacketType, NO_POINTER,
};

#[test]
fn baseband_header_examples_of_a322() {
    // A/322 clause 5.2.2.1: pointer 130 is coded as 10000010 00000100 with OFI = 0.
    let mut h = Vec::new();
    write_bbp_header(130, BbpExtension::None, &mut h).unwrap();
    assert_eq!(h, [0x82, 0x04]);
    assert_eq!(BbpHeader::parse(&h).unwrap().pointer, 130);
    // A pointer below 128 with no extension uses the 1-byte base field.
    h.clear();
    write_bbp_header(5, BbpExtension::None, &mut h).unwrap();
    assert_eq!(h, [5]);
    // A/322 clause 5.2.2.3.1: OFI = 01, EXT_LEN = 1, a counter: a header of 4 bytes.
    h.clear();
    write_bbp_header(
        0,
        BbpExtension::Counter {
            value: 200,
            bytes: 1,
        },
        &mut h,
    )
    .unwrap();
    assert_eq!(h, [0x80, 0x01, 0x01, 200]);
    let p = BbpHeader::parse(&h).unwrap();
    assert_eq!((p.pointer, p.ofi, p.counter, p.len), (0, 1, Some(200), 4));
}

#[test]
fn long_and_mixed_extension_modes() {
    // OFI = 10: EXT_TYPE 111 (padding), EXT_LEN 13 bits = 300: LSB 5 bits = 12, MSB = 9.
    let mut b = vec![
        0x80 | (700 & 0x7F) as u8,
        ((700 >> 7) << 2 | 2) as u8,
        0xE0 | 12,
        9,
    ];
    b.extend(std::iter::repeat_n(0, 300));
    let p = BbpHeader::parse(&b).unwrap();
    assert_eq!((p.pointer, p.len), (700, 304));
    let mut h = Vec::new();
    write_bbp_header(700, BbpExtension::Padding(302), &mut h).unwrap();
    assert_eq!(h, b);
    // OFI = 11: NUM_EXT = 1, total 6 bytes: one 2-byte header (counter, length 2), the counter, 2 padding bytes.
    let b = [0x80, 0x03, 0x20 | 6, 0, 0x02, 0x00, 0x12, 0x34, 0, 0];
    let p = BbpHeader::parse(&b).unwrap();
    assert_eq!((p.ofi, p.counter, p.len), (3, Some(0x1234), 10));
    assert!(BbpHeader::parse(&b[..7]).is_err());
}

fn events(r: &mut AlpReader, bbp: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    r.push_bbp(bbp, &mut |e| match e {
        AlpEvent::Packet(p) => v.push(format!("{:?} {} sid={:?}", p.kind, p.payload.len(), p.sid)),
        AlpEvent::Ts { packet, null } => v.push(if null {
            "null".into()
        } else {
            format!("ts{}", packet[3] & 15)
        }),
    });
    v
}

fn bbp(alp: &[u8]) -> Vec<u8> {
    let mut b = vec![0];
    b.extend_from_slice(alp);
    b
}

#[test]
fn single_packet_headers_of_a330() {
    // Test vector T-1: a single IPv4 packet of 100 bytes has the 2-byte header 0x00 0x64.
    let mut a = Vec::new();
    write_single(PacketType::Ipv4, None, &[0x45; 100], &mut a).unwrap();
    assert_eq!(&a[..2], &[0x00, 0x64]);
    // 3000 bytes: HM = 1, length 11 LSB = 3000 & 2047 = 952, length_MSB = 1, the reserved bit 1 (table 5.4).
    let mut b = Vec::new();
    write_single(PacketType::Ipv4, None, &[7; 3000], &mut b).unwrap();
    assert_eq!(
        &b[..3],
        &[0x08 | (952 >> 8) as u8, (952 & 0xFF) as u8, 0x08 | 0x04]
    );
    let mut r = AlpReader::new();
    let mut s = a.clone();
    s.extend_from_slice(&b);
    assert_eq!(
        events(&mut r, &bbp(&s)),
        ["Ipv4 100 sid=None", "Ipv4 3000 sid=None"]
    );
}

#[test]
fn sub_stream_id_and_header_extension_are_not_payload() {
    // HM = 1 with SIF and HEF (table 5.4), then SID, then header_extension (table 5.8): type 0xF0, length_minus1 1.
    let mut a = vec![
        0x08,
        0x03,
        0x04 | 0x02 | 0x01,
        0x77,
        0xF0,
        0x01,
        0xAA,
        0xBB,
        1,
        2,
        3,
    ];
    let mut r = AlpReader::new();
    let mut got = Vec::new();
    r.push_bbp(&bbp(&a), &mut |e| {
        if let AlpEvent::Packet(p) = e {
            got.push((
                p.sid,
                p.header_extension.map(|(t, b)| (t, b.to_vec())),
                p.payload.to_vec(),
            ));
        }
    });
    assert_eq!(
        got,
        [(Some(0x77), Some((0xF0, vec![0xAA, 0xBB])), vec![1, 2, 3])]
    );
    a.clear();
}

#[test]
fn concatenation_header_of_table_5_6() {
    // Three IPv4 packets of 5, 7 and 4 bytes: PC = 1, S/C = 1, length 16, length_MSB 0, count = 3 - 2 = 1, SIF 0,
    // count + 1 = 2 lengths of 12 bits (5 and 7), no stuffing (count odd).
    let mut a = vec![0x18, 16, 0x02, 0x00, 0x50, 0x07];
    a.extend_from_slice(&[1; 5]);
    a.extend_from_slice(&[2; 7]);
    a.extend_from_slice(&[3; 4]);
    let mut r = AlpReader::new();
    assert_eq!(
        events(&mut r, &bbp(&a)),
        ["Ipv4 5 sid=None", "Ipv4 7 sid=None", "Ipv4 4 sid=None"]
    );
    // Two packets (count 0): one length and 4 stuffing bits.
    let mut a = vec![0x18, 9, 0x00, 0x00, 0x60];
    a.extend_from_slice(&[1; 6]);
    a.extend_from_slice(&[2; 3]);
    assert_eq!(
        events(&mut r, &bbp(&a)),
        ["Ipv4 6 sid=None", "Ipv4 3 sid=None"]
    );
}

#[test]
fn segments_are_joined_in_order() {
    // Segmentation (table 5.5): seq 0, 1, 2. The last has LSI.
    let mk = |seq: u8, last: bool, part: &[u8]| {
        let mut v = vec![
            0x10 | ((part.len() >> 8) as u8 & 7),
            part.len() as u8,
            seq << 3 | (last as u8) << 2,
        ];
        v.extend_from_slice(part);
        v
    };
    let mut s = mk(0, false, &[1; 100]);
    s.extend(mk(1, false, &[2; 100]));
    s.extend(mk(2, true, &[3; 50]));
    let mut r = AlpReader::new();
    assert_eq!(events(&mut r, &bbp(&s)), ["Ipv4 250 sid=None"]);
    // A missing segment drops the packet.
    let mut s = mk(0, false, &[1; 10]);
    s.extend(mk(2, true, &[3; 10]));
    assert!(events(&mut r, &bbp(&s)).is_empty());
    assert_eq!(r.stats.dropped, 1);
}

fn ts(cc: u8) -> [u8; 188] {
    let mut p = [0u8; 188];
    p[0] = 0x47;
    p[1] = 0x01;
    p[2] = 0x00;
    p[3] = 0x10 | cc;
    p[4] = cc;
    p
}

#[test]
fn ts_packets_with_null_deletion_figure_5_9() {
    // A/330 figure 5.9: NUMTS = 2, DNP = 1, then NUMTS = 4, DNP = 2.
    let mut s = Vec::new();
    write_ts(&[ts(0), ts(1)], 1, false, &mut s).unwrap();
    write_ts(&[ts(2), ts(3), ts(4), ts(5)], 2, false, &mut s).unwrap();
    assert_eq!(&s[..2], &[7 << 5 | 2 << 1 | 1, 1]);
    let mut r = AlpReader::new();
    assert_eq!(
        events(&mut r, &bbp(&s)),
        ["null", "ts0", "ts1", "null", "null", "ts2", "ts3", "ts4", "ts5"]
    );
}

#[test]
fn numts_zero_means_16_and_dnp_zero_means_128() {
    // A/330 clause 5.4.1: NUMTS = 0 is 16 packets. With HDM = 0, DNP = 0 is 128 deleted null packets.
    let pk: Vec<[u8; 188]> = (0..16).map(ts).collect();
    let mut s = Vec::new();
    write_ts(&pk, 128, false, &mut s).unwrap();
    assert_eq!(&s[..2], &[7 << 5 | 1, 0]);
    let mut r = AlpReader::new();
    let ev = events(&mut r, &bbp(&s));
    assert_eq!(ev.iter().filter(|e| *e == "null").count(), 128);
    assert_eq!(ev.len(), 128 + 16);
}

#[test]
fn ts_header_deletion_restores_headers() {
    // A/330 clause 5.4.4: HDM = 1, the headers after the first are deleted. The counter counts up from the first.
    let pk: Vec<[u8; 188]> = (3..7).map(ts).collect();
    let mut s = Vec::new();
    write_ts(&pk, 0, true, &mut s).unwrap();
    assert_eq!(s.len(), 2 + 187 + 3 * 184);
    let mut r = AlpReader::new();
    let mut got = Vec::new();
    r.push_bbp(&bbp(&s), &mut |e| {
        if let AlpEvent::Ts { packet, null } = e {
            assert!(!null);
            got.push(*packet);
        }
    });
    assert_eq!(got, pk);
}

#[test]
fn a_stream_cut_into_baseband_packets_comes_back() {
    let mut stream = Vec::new();
    write_single(PacketType::Ipv4, None, &[0x45; 100], &mut stream).unwrap();
    write_single(PacketType::Ipv4, None, &vec![7; 3000], &mut stream).unwrap();
    write_ts(&[ts(0), ts(1), ts(2)], 0, false, &mut stream).unwrap();
    let mut sig = vec![1, 0, 0, 3, 0x40 | 0x0F];
    sig.extend_from_slice(b"<LMT/>");
    write_single(PacketType::Signalling, None, &sig, &mut stream).unwrap();
    let mut r = AlpReader::new();
    let mut out = Vec::new();
    let mut first = true;
    for chunk in stream.chunks(997) {
        let mut b = Vec::new();
        write_bbp_header(
            if first { 0 } else { NO_POINTER },
            BbpExtension::None,
            &mut b,
        )
        .unwrap();
        first = false;
        b.extend_from_slice(chunk);
        out.extend(events(&mut r, &b));
    }
    assert_eq!(
        out,
        [
            "Ipv4 100 sid=None",
            "Ipv4 3000 sid=None",
            "ts0",
            "ts1",
            "ts2",
            "Signalling 6 sid=None"
        ]
    );
}

#[test]
fn the_reader_does_not_allocate() {
    let mut stream = Vec::new();
    for i in 0..20 {
        write_single(
            PacketType::Ipv4,
            None,
            &vec![i as u8; 200 + 50 * i],
            &mut stream,
        )
        .unwrap();
        write_ts(&[ts(0), ts(1)], 1, false, &mut stream).unwrap();
    }
    let mut packets = Vec::new();
    let mut first = true;
    for chunk in stream.chunks(1200) {
        let mut b = Vec::new();
        write_bbp_header(
            if first { 0 } else { NO_POINTER },
            BbpExtension::None,
            &mut b,
        )
        .unwrap();
        first = false;
        b.extend_from_slice(chunk);
        packets.push(b);
    }
    let mut r = AlpReader::new();
    let mut k = 0;
    let mut sink = 0usize;
    let a = crate::alloc::count(|| {
        r.push_bbp(&packets[k % packets.len()], &mut |_| sink += 1);
        k += 1;
    });
    assert_eq!(a, 0);
    assert!(sink > 0);
}
