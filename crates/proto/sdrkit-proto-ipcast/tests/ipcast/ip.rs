//! IPv4 (RFC 791), IPv6 (RFC 8200), UDP (RFC 768), reassembly (RFC 791 clause 3.2, RFC 815) and flow statistics.

use sdrkit_proto_ipcast::flow::FlowTable;
use sdrkit_proto_ipcast::ip::{
    udp_checksum, write_ipv4_udp, write_udp, Ipv4, Ipv6, Reassembler, Udp, UDP,
};
use sdrkit_proto_ipcast::Check;
use sdrkit_proto_ipcast::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The IPv4 header of the worked example of the Internet checksum on Wikipedia ("IPv4 header checksum"): checksum 0xB861.
const HEADER: [u8; 20] = [
    0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xB8, 0x61, 0xC0, 0xA8, 0x00, 0x01,
    0xC0, 0xA8, 0x00, 0xC7,
];

#[test]
fn header_checksum_known_answer() {
    let mut p = HEADER.to_vec();
    p.resize(0x73, 0);
    let ip = Ipv4::parse(&p).unwrap();
    assert_eq!(ip.src, Ipv4Addr::new(192, 168, 0, 1));
    assert_eq!(ip.dst, Ipv4Addr::new(192, 168, 0, 199));
    assert_eq!((ip.protocol, ip.ttl, ip.payload.len()), (17, 64, 0x73 - 20));
    // The checksum the writer makes is the published one.
    let mut out = Vec::new();
    sdrkit_proto_ipcast::ip::write_ipv4_header(
        &mut out,
        ip.src,
        ip.dst,
        17,
        0,
        false,
        0,
        0x73 - 20,
    )
    .unwrap();
    out[1] = 0;
    out[6] = 0x40; // DF, as in the example
    out[10] = 0;
    out[11] = 0;
    let c = sdrkit_util_bits::checksum::internet_checksum(&[&out]);
    assert_eq!(c, 0xB861);
    // A wrong checksum is refused.
    p[11] ^= 1;
    assert_eq!(
        Ipv4::parse(&p),
        Err(Error::Check(Check::Ipv4HeaderChecksum))
    );
    assert_eq!(Ipv4::parse(&p[..19]), Err(Error::Short));
}

/// RFC 1071 one's complement sum written out word by word, as a second calculation of the UDP checksum.
fn ones(words: &[u8]) -> u16 {
    let mut s = 0u32;
    for c in words.chunks(2) {
        s += u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]) as u32;
    }
    while s > 0xFFFF {
        s = (s & 0xFFFF) + (s >> 16);
    }
    !(s as u16)
}

#[test]
fn udp_checksum_with_pseudo_headers() {
    let src = Ipv4Addr::new(10, 0, 0, 1);
    let dst = Ipv4Addr::new(239, 1, 2, 3);
    let mut seg = Vec::new();
    write_udp(&mut seg, src.into(), dst.into(), 5000, 5004, b"abc").unwrap();
    // RFC 768: pseudo-header source, destination, zero, protocol, UDP length.
    let mut v = Vec::new();
    v.extend_from_slice(&src.octets());
    v.extend_from_slice(&dst.octets());
    v.extend_from_slice(&[0, 17, 0, 11]);
    let mut zeroed = seg.clone();
    zeroed[6] = 0;
    zeroed[7] = 0;
    v.extend_from_slice(&zeroed);
    assert_eq!(u16::from_be_bytes([seg[6], seg[7]]), ones(&v));
    let u = Udp::parse(src.into(), dst.into(), &seg).unwrap();
    assert_eq!(
        (u.src_port, u.dst_port, u.checksum_ok, u.payload),
        (5000, 5004, Some(true), &b"abc"[..])
    );
    // Over IPv4 a zero checksum means "not sent". Over IPv6 it is not allowed (RFC 8200 clause 8.1).
    let u = Udp::parse(src.into(), dst.into(), &zeroed).unwrap();
    assert_eq!(u.checksum_ok, None);
    let s6: IpAddr = Ipv6Addr::LOCALHOST.into();
    assert_eq!(
        Udp::parse(s6, s6, &zeroed).unwrap().checksum_ok,
        Some(false)
    );
    let mut seg6 = Vec::new();
    write_udp(&mut seg6, s6, s6, 1, 2, b"x").unwrap();
    assert_eq!(udp_checksum(s6, s6, &seg6), 0);
}

#[test]
fn ipv6_header_and_extension_headers() {
    let s = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
    let d = Ipv6Addr::new(0xff0e, 0, 0, 0, 0, 0, 0, 0x101);
    let mut udp = Vec::new();
    write_udp(&mut udp, s.into(), d.into(), 1234, 5678, b"hello").unwrap();
    let mut p = vec![0x60, 0, 0, 0];
    p.extend_from_slice(&((8 + udp.len()) as u16).to_be_bytes());
    p.extend_from_slice(&[0, 64]); // next header: hop-by-hop options
    p.extend_from_slice(&s.octets());
    p.extend_from_slice(&d.octets());
    p.extend_from_slice(&[17, 0, 1, 4, 0, 0, 0, 0]); // hop-by-hop: next UDP, length 8
    p.extend_from_slice(&udp);
    let v6 = Ipv6::parse(&p).unwrap();
    assert_eq!(
        (v6.protocol, v6.src, v6.dst, v6.payload),
        (UDP, s, d, &udp[..])
    );
    assert!(!v6.fragment);
}

fn fragments(id: u16, dst: Ipv4Addr, payload: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut ranges = Vec::new();
    write_ipv4_udp(
        &mut out,
        Ipv4Addr::new(10, 0, 0, 1),
        dst,
        id,
        576,
        1234,
        4937,
        payload,
        |r| ranges.push(r),
    )
    .unwrap();
    ranges.into_iter().map(|r| out[r].to_vec()).collect()
}

#[test]
fn reassembly_in_any_order_with_duplicates() {
    let data: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
    let pk = fragments(7, Ipv4Addr::new(224, 0, 23, 60), &data);
    assert!(pk.len() >= 6);
    let mut r = Reassembler::default();
    let mut got = Vec::new();
    // Last first, a duplicate of the first, the rest reversed.
    let mut order = vec![pk.len() - 1, 0, 0];
    order.extend((1..pk.len() - 1).rev());
    for &i in &order {
        r.push(&pk[i], 0.0, &mut |d| {
            let u = d.udp().unwrap().unwrap();
            assert_eq!(u.checksum_ok, Some(true));
            got.push(u.payload.to_vec());
        })
        .unwrap();
    }
    assert_eq!(got, [data]);
    assert_eq!(r.stats.reassembled, 1);
    assert_eq!(r.open(), 0);
}

#[test]
fn fragments_of_different_destinations_do_not_mix() {
    // RFC 791 clause 3.2: source, destination, protocol and identification name a datagram. A key of only
    // (identification, source) would join two datagrams to different groups that have the same id.
    let a: Vec<u8> = vec![0xAA; 1500];
    let b: Vec<u8> = vec![0xBB; 1500];
    let pa = fragments(42, Ipv4Addr::new(239, 0, 0, 1), &a);
    let pb = fragments(42, Ipv4Addr::new(239, 0, 0, 2), &b);
    let mut r = Reassembler::default();
    let mut got = Vec::new();
    for (x, y) in pa.iter().zip(&pb) {
        r.push(x, 0.0, &mut |d| {
            got.push(d.udp().unwrap().unwrap().payload.to_vec())
        })
        .unwrap();
        r.push(y, 0.0, &mut |d| {
            got.push(d.udp().unwrap().unwrap().payload.to_vec())
        })
        .unwrap();
    }
    assert_eq!(got, [a, b]);
    assert_eq!(r.stats.overlaps, 0);
}

#[test]
fn reassembly_times_out_and_is_bounded() {
    let pk = fragments(1, Ipv4Addr::new(239, 0, 0, 1), &[1; 2000]);
    let mut r = Reassembler::new(2, 60.0);
    r.push(&pk[0], 0.0, &mut |_| panic!()).unwrap();
    // 61 s later the first fragment is gone: the rest cannot complete it.
    for p in &pk[1..] {
        r.push(p, 61.0, &mut |_| {
            panic!("completed from a timed-out reassembly")
        })
        .unwrap();
    }
    assert_eq!(r.stats.dropped, 1);
    // More datagrams than buffers: the oldest goes.
    for id in 10..20 {
        let pk = fragments(id, Ipv4Addr::new(239, 0, 0, 1), &[1; 2000]);
        r.push(&pk[0], 100.0 + id as f64, &mut |_| {}).unwrap();
    }
    assert_eq!(r.open(), 2);
}

#[test]
fn flows_count_after_reassembly_and_find_encryption() {
    let mut r = Reassembler::default();
    let mut flows = FlowTable::new(8);
    // A fragmented datagram: its later fragments carry no UDP header, so the checksum is calculated only on the whole datagram.
    for p in fragments(5, Ipv4Addr::new(239, 0, 0, 9), &[3; 3000]) {
        r.push(&p, 1.0, &mut |d| flows.datagram(&d, 1.0)).unwrap();
    }
    assert_eq!(flows.flows().len(), 1);
    assert_eq!(
        flows.encrypted_flows(),
        0,
        "a fragmented datagram is not an encrypted flow"
    );
    // A flow whose UDP payload changed after the checksum was made.
    for i in 0..4u8 {
        let mut p = fragments(100 + i as u16, Ipv4Addr::new(239, 0, 0, 7), &[i; 100]).remove(0);
        let n = p.len();
        p[n - 1] ^= 0x5A;
        r.push(&p, 2.0, &mut |d| flows.datagram(&d, 2.0)).unwrap();
    }
    assert_eq!(flows.encrypted_flows(), 1);
    let f = flows
        .flows()
        .iter()
        .find(|f| f.key.dst_port == 4937 && f.packets == 4)
        .unwrap();
    assert!(f.encrypted());
    assert_eq!(flows.udp_ports(), &[(4937, 5)]);
}

#[test]
fn flow_table_and_reassembly_do_not_allocate() {
    let mut pk = Vec::new();
    for id in 0..30u16 {
        pk.extend(fragments(
            id,
            Ipv4Addr::new(239, 0, 0, (id % 40) as u8),
            &vec![id as u8; 1800],
        ));
    }
    let mut r = Reassembler::default();
    let mut flows = FlowTable::new(16);
    let mut t = 0.0;
    // Every buffer is used once before counting.
    for p in &pk {
        r.push(p, t, &mut |d| flows.datagram(&d, t)).unwrap();
    }
    let mut k = 0;
    let a = crate::alloc::count(|| {
        t += 0.001;
        r.push(&pk[k % pk.len()], t, &mut |d| flows.datagram(&d, t))
            .unwrap();
        k += 1;
    });
    assert_eq!(a, 0);
    assert!(flows.evicted > 0);
}
