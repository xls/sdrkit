//! MPE: the computed example of a datagram section in a transport packet, the reader's rules, and the CRC check value.

use sdrkit_proto_ipcast::filecast::tail_packet;
use sdrkit_proto_ipcast::mpe::{
    parse_section, section_packets, write_ipv4_udp_plain, write_section, MpeReader, SectionFault,
};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;
use std::net::Ipv4Addr;

const MAC: [u8; 6] = [0x01, 0x00, 0x5E, 0x00, 0x00, 0x01];

fn example_datagram() -> Vec<u8> {
    let mut d = Vec::new();
    write_ipv4_udp_plain(
        &mut d,
        Ipv4Addr::new(10, 0, 0, 1),
        Ipv4Addr::new(239, 0, 0, 1),
        5000,
        8001,
        &tail_packet(0x1000),
    )
    .unwrap();
    d
}

#[test]
fn the_computed_example_is_reproduced() {
    let d = example_datagram();
    assert_eq!(
        d[..28],
        [
            0x45, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x40, 0x11, 0x81, 0xBF, 0x0A, 0x00,
            0x00, 0x01, 0xEF, 0x00, 0x00, 0x01, 0x13, 0x88, 0x1F, 0x41, 0x00, 0x18, 0x00, 0x00
        ]
    );
    assert_eq!(
        d[28..],
        [0x00, 0xFF, 0x10, 0x00, 0x00, 0x10, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let mut s = Vec::new();
    write_section(MAC, &d, &mut s).unwrap();
    assert_eq!(
        s[..12],
        [0x3E, 0xB0, 0x39, 0x01, 0x00, 0xC1, 0x00, 0x00, 0x00, 0x5E, 0x00, 0x01]
    );
    assert_eq!(s[s.len() - 4..], [0x02, 0x83, 0x3D, 0x85]);
    assert_eq!(CRC32_MPEG2.checksum(&s), 0);
    assert_eq!(CRC32_MPEG2.checksum(b"123456789"), 0x0376_E6E7);
    let mut packets = Vec::new();
    let mut cc = 0;
    section_packets(&s, 1001, &mut cc, |p| packets.push(p));
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][..8], [0x47, 0x43, 0xE9, 0x10, 0x00, 0x3E, 0xB0, 0x39]);
    assert!(packets[0][5 + s.len()..].iter().all(|&b| b == 0xFF));
    let sec = parse_section(&s).unwrap();
    assert_eq!(sec.mac, MAC);
    assert_eq!(sec.datagram, &d[..]);
    let mut r = MpeReader::new(1001);
    let mut got = Vec::new();
    r.push(&packets[0], |d| got.push(d.to_vec()));
    assert_eq!(got, [d]);
}

#[test]
fn the_reader_keeps_its_rules() {
    let d = example_datagram();
    let mut s = Vec::new();
    write_section(MAC, &d, &mut s).unwrap();
    // A bad CRC.
    let mut bad = s.clone();
    bad[20] ^= 1;
    assert_eq!(parse_section(&bad), Err(SectionFault::Crc));
    // Section number 1: skipped.
    let mut skip = s.clone();
    skip[6] = 1;
    let n = skip.len() - 4;
    let c = CRC32_MPEG2.checksum(&skip[..n]);
    skip[n..].copy_from_slice(&c.to_be_bytes());
    assert_eq!(parse_section(&skip), Err(SectionFault::Skipped));
    // Too short, or another table.
    assert_eq!(parse_section(&s[..12]), Err(SectionFault::NotDatagram));
    let mut other = s.clone();
    other[0] = 0x3F;
    assert_eq!(parse_section(&other), Err(SectionFault::NotDatagram));
    // A PID that starts with a PES is never read.
    let mut pes = [0xFFu8; 188];
    pes[..4].copy_from_slice(&[0x47, 0x43, 0xE9, 0x10]);
    pes[4..7].copy_from_slice(&[0, 0, 1]);
    let mut r = MpeReader::new(1001);
    r.push(&pes, |_| panic!("no datagram from a PES"));
    let mut cc = 1;
    let mut n = 0;
    section_packets(&s, 1001, &mut cc, |p| r.push(&p, |_| n += 1));
    assert_eq!(n, 0);
    // The wrong PID gives nothing; the right one, across two packets, gives the datagram.
    let mut big = Vec::new();
    write_ipv4_udp_plain(
        &mut big,
        Ipv4Addr::new(10, 0, 0, 1),
        Ipv4Addr::new(239, 0, 0, 1),
        5000,
        8001,
        &[7u8; 500],
    )
    .unwrap();
    let mut s2 = Vec::new();
    write_section(MAC, &big, &mut s2).unwrap();
    let mut wrong = MpeReader::new(1002);
    let mut right = MpeReader::new(1001);
    let mut got = Vec::new();
    let mut cc = 0;
    section_packets(&s2, 1001, &mut cc, |p| {
        wrong.push(&p, |_| panic!("wrong PID"));
        right.push(&p, |d| got.push(d.len()));
    });
    assert_eq!(got, [big.len()]);
    assert_eq!(right.stats.datagrams, 1);
    assert_eq!(wrong.stats.packets, 0);
}
