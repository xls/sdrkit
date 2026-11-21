//! PCAP file bytes against the format of draft-ietf-opsawg-pcap (the libpcap file format).

use sdrkit_proto_ipcast::pcap::{LinkType, PcapWriter};

#[test]
fn file_header_bytes() {
    // Magic 0xA1B2C3D4 little-endian, version 2.4, two zero words, snap length 262144, link type 101 (LINKTYPE_RAW).
    let mut f = Vec::new();
    PcapWriter::new(LinkType::Raw).file_header(&mut f);
    assert_eq!(
        f,
        [
            0xD4, 0xC3, 0xB2, 0xA1, 0x02, 0x00, 0x04, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00,
            0x04, 0x00, 0x65, 0x00, 0x00, 0x00
        ]
    );
    let mut n = Vec::new();
    PcapWriter::new(LinkType::Ipv4)
        .with_nanoseconds()
        .file_header(&mut n);
    assert_eq!(&n[..4], &[0x4D, 0x3C, 0xB2, 0xA1]);
    assert_eq!(&n[20..], &[228, 0, 0, 0]);
}

#[test]
fn record_times_come_from_the_caller() {
    // The dvbcs copy wrote the packet count as the time. A record holds seconds and microseconds of the given time.
    let w = PcapWriter::new(LinkType::Raw);
    let mut f = Vec::new();
    w.record(1_700_000_000.25, &[1, 2, 3], &mut f).unwrap();
    assert_eq!(&f[..4], &1_700_000_000u32.to_le_bytes());
    assert_eq!(&f[4..8], &250_000u32.to_le_bytes());
    assert_eq!(&f[8..16], &[3, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(&f[16..], &[1, 2, 3]);
    let mut g = Vec::new();
    w.with_nanoseconds().record(2.5, &[], &mut g).unwrap();
    assert_eq!(&g[4..8], &500_000_000u32.to_le_bytes());
    assert!(w.record(-1.0, &[], &mut g).is_err());
    assert!(w.record(f64::NAN, &[], &mut g).is_err());
}
