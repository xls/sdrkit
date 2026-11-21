//! GSE: ETSI TS 102 606-1 table 2, clauses 4.2.1, 4.2.2 and annex A. RFC 4326 clause 5 and RFC 5163 for extension headers.

use sdrkit_proto_ipcast::gse::{parse_extensions, GseReader, GseWriter, Label};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;

fn read(r: &mut GseReader, df: &[u8]) -> Vec<(u16, Label, Vec<u8>)> {
    let mut v = Vec::new();
    r.push(df, &mut |p| v.push((p.protocol, p.label, p.pdu.to_vec())));
    v
}

#[test]
fn complete_packet_field_by_field() {
    // Table 2: S = 1, E = 1, LT = 00 (6-byte label), GSE_Length = 2 + 6 + 4 = 12, Protocol_Type 0x0800, label, PDU.
    let df = [
        0xC0, 0x0C, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00,
    ];
    let mut r = GseReader::default();
    let got = read(&mut r, &df);
    assert_eq!(
        got,
        [(
            0x0800,
            Label::Six([1, 2, 3, 4, 5, 6]),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        )]
    );
    // A 3-byte label (LT = 01) and broadcast (LT = 10).
    let df = [
        0xD0, 0x07, 0x86, 0xDD, 7, 8, 9, 0xAA, 0xBB, 0xE0, 0x03, 0x08, 0x00, 0x42,
    ];
    let got = read(&mut r, &df);
    assert_eq!(got[0], (0x86DD, Label::Three([7, 8, 9]), vec![0xAA, 0xBB]));
    assert_eq!(got[1], (0x0800, Label::Broadcast, vec![0x42]));
}

/// Builds a PDU in three fragments by table 2, the CRC-32 by clause 4.2.2: start, intermediate (LT = 11) and end.
fn three_fragments(pdu: &[u8], fid: u8) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let label = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    let total = (2 + 6 + pdu.len()) as u16;
    let mut covered = total.to_be_bytes().to_vec();
    covered.extend_from_slice(&0x0800u16.to_be_bytes());
    covered.extend_from_slice(&label);
    covered.extend_from_slice(pdu);
    let crc = CRC32_MPEG2.checksum(&covered);
    let (a, rest) = pdu.split_at(pdu.len() / 3);
    let (b, c) = rest.split_at(rest.len() / 2);
    let mut start = vec![0, 0, fid];
    start.extend_from_slice(&covered[..10]);
    start.extend_from_slice(a);
    let gl = (start.len() - 2) as u16;
    start[..2].copy_from_slice(&(0x8000 | gl).to_be_bytes()); // S = 1, E = 0, LT = 00
    let mut mid = vec![0, 0, fid];
    mid.extend_from_slice(b);
    let gl = (mid.len() - 2) as u16;
    mid[..2].copy_from_slice(&(0x3000 | gl).to_be_bytes()); // S = 0, E = 0, LT = 11
    let mut end = vec![0, 0, fid];
    end.extend_from_slice(c);
    end.extend_from_slice(&crc.to_be_bytes());
    let gl = (end.len() - 2) as u16;
    end[..2].copy_from_slice(&(0x7000 | gl).to_be_bytes()); // S = 0, E = 1, LT = 11
    (start, mid, end)
}

#[test]
fn fragments_reassemble_across_frames_with_lt_11_middles() {
    let pdu: Vec<u8> = (0..900u32).map(|i| (i * 7) as u8).collect();
    let (s, m, e) = three_fragments(&pdu, 9);
    let mut r = GseReader::default();
    assert!(read(&mut r, &s).is_empty());
    assert!(
        read(&mut r, &m).is_empty(),
        "an intermediate packet is not padding"
    );
    let got = read(&mut r, &e);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].2, pdu);
    assert_eq!(r.stats.crc_errors, 0);
    // A wrong CRC-32 drops the PDU.
    let mut bad = e.clone();
    let k = bad.len() - 1;
    bad[k] ^= 1;
    read(&mut r, &s);
    read(&mut r, &m);
    assert!(read(&mut r, &bad).is_empty());
    assert_eq!(r.stats.crc_errors, 1);
    // A lost middle fragment makes the length differ from Total_Length (annex A.2).
    read(&mut r, &s);
    assert!(read(&mut r, &e).is_empty());
    assert_eq!(r.stats.length_errors, 1);
    // An end fragment with no start is an orphan.
    assert!(read(&mut r, &e).is_empty());
    assert_eq!(r.stats.orphans, 1);
}

#[test]
fn padding_ends_the_frame() {
    // Annex A.5: S = E = 0 and LT = 00 is padding to the end of the frame.
    let df = [
        0xE0, 0x03, 0x08, 0x00, 0x42, 0x00, 0x05, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    let mut r = GseReader::default();
    assert_eq!(read(&mut r, &df).len(), 1);
    assert_eq!(r.stats.packets, 1);
}

#[test]
fn label_reuse_does_not_cross_frames() {
    // LT = 11 takes the label of the previous start or complete packet of the same frame (table 3, annex A.4).
    let first = [
        0xC0, 0x09, 0x08, 0x00, 1, 2, 3, 4, 5, 6, 0xAA, 0xF0, 0x03, 0x08, 0x00, 0xBB,
    ];
    let mut r = GseReader::default();
    let got = read(&mut r, &first);
    assert_eq!(got[1].1, Label::Six([1, 2, 3, 4, 5, 6]));
    // The next frame starts with label re-use: the packet is discarded.
    let second = [0xF0, 0x03, 0x08, 0x00, 0xCC];
    assert!(read(&mut r, &second).is_empty());
    assert_eq!(r.stats.label_errors, 1);
}

#[test]
fn extension_headers() {
    // An optional Timestamp (type 0x0301: H-LEN 3, H-Type 1, RFC 5163 clause 3.2) then IPv4.
    let data = [0x12, 0x34, 0x56, 0x78, 0x08, 0x00, 0x45, 0x00];
    let (p, ext, pdu) = parse_extensions(0x0301, &data).unwrap();
    assert_eq!(
        (p, ext.timestamp, ext.optional, pdu),
        (0x0800, Some(0x1234_5678), 1, &data[6..])
    );
    // Extension padding (0x0100: H-LEN 1, only the next type).
    let (p, _, pdu) = parse_extensions(0x0100, &[0x86, 0xDD, 1]).unwrap();
    assert_eq!((p, pdu), (0x86DD, &[1u8][..]));
    // Bridged frame (0x0001): an Ethernet frame. The protocol is its EtherType.
    let mut eth = vec![0xFFu8; 12];
    eth.extend_from_slice(&[0x08, 0x06, 0, 1]);
    let (p, ext, pdu) = parse_extensions(0x0001, &eth).unwrap();
    assert!(ext.bridged);
    assert_eq!((p, pdu.len()), (0x0806, 16));
    // A mandatory type this reader does not know, and the Test SNDU, are discarded (annex A.3).
    assert!(parse_extensions(0x0042, &data).is_err());
    assert!(parse_extensions(0x0000, &data).is_err());
    // In a GSE packet: counted, never delivered.
    let df = [0xE0, 0x04, 0x00, 0x42, 1, 2];
    let mut r = GseReader::default();
    assert!(read(&mut r, &df).is_empty());
    assert_eq!(r.stats.ext_errors, 1);
}

#[test]
fn writer_round_trip_with_start_middle_and_end_fragments() {
    let mut w = GseWriter::default();
    let mut sent = Vec::new();
    for i in 0..30usize {
        let pdu: Vec<u8> = (0..(50 + 397 * i) % 5000).map(|k| (k ^ i) as u8).collect();
        let label = match i % 3 {
            0 => Label::Six([1, 2, 3, 4, 5, i as u8]),
            1 => Label::Three([9, 9, i as u8]),
            _ => Label::Broadcast,
        };
        w.push_pdu(0x0800, label, &pdu).unwrap();
        sent.push((0x0800u16, label, pdu));
    }
    let mut r = GseReader::default();
    let mut got = Vec::new();
    // Data fields of 1500 bytes: PDUs of up to 5000 bytes need middle fragments.
    for _ in 0..200 {
        let mut df = vec![0u8; 1500];
        w.fill(&mut df);
        got.extend(read(&mut r, &df));
    }
    assert_eq!(got, sent);
    assert_eq!(w.backlog(), 0);
}

#[test]
fn sliced_mode_joins_packets_cut_across_fields() {
    let mut w = GseWriter::default();
    let mut sent = Vec::new();
    for i in 0..10u8 {
        let pdu = vec![i; 300];
        w.push_pdu(0x0800, Label::Broadcast, &pdu).unwrap();
        sent.push(pdu);
    }
    let mut stream = vec![0u8; 0];
    let mut df = vec![0u8; 4000];
    let n = w.fill(&mut df);
    stream.extend_from_slice(&df[..n]);
    // Cut the packet stream into fields of 700 bytes. SYNCD is the first packet start in each field.
    let mut starts = Vec::new();
    let mut i = 0;
    while i < stream.len() {
        starts.push(i);
        let gl = (u16::from_be_bytes([stream[i], stream[i + 1]]) & 0x0FFF) as usize;
        i += 2 + gl;
    }
    let mut r = GseReader::default();
    let mut got = Vec::new();
    for (k, field) in stream.chunks(700).enumerate() {
        let a = k * 700;
        let syncd = starts
            .iter()
            .find(|&&s| s >= a && s < a + field.len())
            .map_or(0xFFFF, |&s| ((s - a) * 8) as u16);
        r.push_sliced(field, syncd, &mut |p| got.push(p.pdu.to_vec()));
    }
    assert_eq!(got, sent);
}

#[test]
fn the_reader_does_not_allocate() {
    let mut w = GseWriter::default();
    let mut fields = Vec::new();
    for i in 0..40usize {
        w.push_pdu(
            0x0800,
            Label::Six([0, 1, 2, 3, 4, 5]),
            &vec![i as u8; 100 + 113 * i],
        )
        .unwrap();
    }
    for _ in 0..40 {
        let mut df = vec![0u8; 2000];
        w.fill(&mut df);
        fields.push(df);
    }
    let mut r = GseReader::default();
    let mut k = 0;
    let mut sink = 0usize;
    let a = crate::alloc::count(|| {
        r.push(&fields[k % fields.len()], &mut |p| sink += p.pdu.len());
        k += 1;
    });
    assert_eq!(a, 0);
    assert!(sink > 0);
}

#[test]
fn fill_returns_the_bytes_of_every_packet_written() {
    // A generator sets DFL from the length fill returns: the fragments must be inside it.
    let mut w = GseWriter::default();
    let pdu: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
    w.push_pdu(0x0800, Label::Broadcast, &pdu).unwrap();
    let mut r = GseReader::default();
    let mut got = Vec::new();
    for _ in 0..4 {
        let mut df = vec![0u8; 1000];
        let n = w.fill(&mut df);
        r.push(&df[..n], &mut |p| got.push(p.pdu.to_vec()));
    }
    assert_eq!(got, [pdu]);
}

#[test]
fn sliced_mode_finds_the_chain_again_after_a_lost_field() {
    let mut w = GseWriter::default();
    let mut sent = Vec::new();
    for i in 0..20u8 {
        let pdu = vec![i; 250 + i as usize * 7];
        w.push_pdu(0x0800, Label::Broadcast, &pdu).unwrap();
        sent.push(pdu);
    }
    let mut stream = Vec::new();
    let mut df = vec![0u8; 8000];
    let n = w.fill(&mut df);
    stream.extend_from_slice(&df[..n]);
    let mut starts = Vec::new();
    let mut i = 0;
    while i < stream.len() {
        starts.push(i);
        i += 2 + (u16::from_be_bytes([stream[i], stream[i + 1]]) & 0x0FFF) as usize;
    }
    let mut r = GseReader::default();
    let mut got = Vec::new();
    for (k, field) in stream.chunks(700).enumerate() {
        if k == 3 {
            continue; // lost
        }
        let a = k * 700;
        let syncd = starts
            .iter()
            .find(|&&s| s >= a && s < a + field.len())
            .map_or(0xFFFF, |&s| ((s - a) * 8) as u16);
        r.push_sliced(field, syncd, &mut |p| got.push(p.pdu.to_vec()));
    }
    assert!(r.stats.resyncs >= 1);
    // Every PDU out is one that was sent: no head joined to a foreign tail.
    assert!(
        got.iter().all(|p| sent.contains(p)),
        "a PDU that was not sent came out"
    );
    assert!(got.len() >= sent.len() - 3);
}
