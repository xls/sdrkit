//! The header reader on the worked frames of the Thread receiver's specification (T1 to T8), and on frames built from the
//! rules of RFC 4944 section 5, RFC 6282 sections 3 and 4 and RFC 7252 section 3.

use sdrkit_proto_6lowpan::coap::{self, Kind};
use sdrkit_proto_6lowpan::{read, DstAddress, LinkAddress, NextHeader, SrcAddress};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[test]
fn t1_iphc_with_a_multicast_destination_and_compressed_udp() {
    let b = hex("7F 3B 02 F0 4D 4C 4D 4C 12 34 FF 10 0B 02 20 00");
    let d = read(&b);
    let ip = d.iphc.unwrap();
    assert_eq!(ip.traffic_len, 0);
    assert_eq!(ip.next_header, None);
    assert_eq!(ip.hop_limit, 255);
    assert_eq!(ip.source, SrcAddress::Elided { context: false });
    let DstAddress::Multicast {
        bytes,
        len,
        address,
    } = ip.destination
    else {
        panic!("a multicast destination");
    };
    assert_eq!((bytes[0], len), (0x02, 1));
    let mut all_routers = [0u8; 16];
    all_routers[0] = 0xFF;
    all_routers[1] = 0x02;
    all_routers[15] = 0x02;
    assert_eq!(address, Some(all_routers));
    let u = d.udp.unwrap();
    assert_eq!(
        (u.source, u.destination, u.checksum),
        (19788, 19788, Some(0x1234))
    );
    assert!(u.compressed);
    assert_eq!(
        (d.next, d.payload, d.truncated),
        (NextHeader::Udp, 10, false)
    );
    // The payload is an MLE message: no security, command 16.
    assert_eq!(b[d.payload..d.payload + 2], [0xFF, 0x10]);
}

#[test]
fn t3_mesh_header_first_fragment_and_udp_ports_in_one_byte() {
    let d = read(&hex("B5 44 00 10 00 C1 2C 00 07 7F 3B 02 F7 13 99"));
    let m = d.mesh.unwrap();
    assert_eq!(m.originator, LinkAddress::Short(0x4400));
    assert_eq!(m.final_destination, LinkAddress::Short(0x1000));
    assert_eq!(m.hops_left, 5);
    let f = d.fragment.unwrap();
    assert_eq!(
        (f.datagram_size, f.datagram_tag, f.offset, f.first),
        (300, 7, 0, true)
    );
    assert!(d.iphc.is_some());
    let u = d.udp.unwrap();
    assert_eq!(
        (u.source, u.destination, u.checksum),
        (0xF0B1, 0xF0B3, None)
    );
    assert_eq!((d.next, d.payload), (NextHeader::Udp, 14));
}

#[test]
fn t4_a_later_fragment_has_no_header_of_the_datagram() {
    let d = read(&hex("E1 2C 00 07 05 01 02 03"));
    let f = d.fragment.unwrap();
    assert_eq!(
        (f.datagram_size, f.datagram_tag, f.offset, f.first),
        (300, 7, 40, false)
    );
    assert_eq!(d.iphc, None);
    assert_eq!((d.next, d.payload), (NextHeader::FragmentData, 5));
}

#[test]
fn t5_coap_over_compressed_udp() {
    let b = hex("7F 3B 02 F0 16 33 16 33 00 00 40 01 12 34");
    let d = read(&b);
    let u = d.udp.unwrap();
    assert_eq!((u.source, u.destination), (5683, 5683));
    let m = coap::read(&b[d.payload..]).unwrap();
    assert_eq!((m.kind, m.token_len, m.message_id), (Kind::Con, 0, 0x1234));
    assert_eq!(
        (m.code_text().as_str(), m.code_name()),
        ("0.01", Some("GET"))
    );
}

#[test]
fn t6_next_header_in_line_and_addresses_in_line() {
    let d = read(&hex(
        "7B 10 3A 01 02 03 04 05 06 07 08 FD 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01",
    ));
    let ip = d.iphc.unwrap();
    assert_eq!(ip.next_header, Some(58));
    assert_eq!(ip.hop_limit, 255);
    assert_eq!(
        ip.source,
        SrcAddress::Inline {
            bytes: [1, 2, 3, 4, 5, 6, 7, 8],
            len: 8,
            context: false
        }
    );
    let mut dst = [0u8; 16];
    dst[0] = 0xFD;
    dst[15] = 0x01;
    assert_eq!(ip.destination, DstAddress::Full(dst));
    assert_eq!(
        (d.next, d.payload, d.truncated),
        (NextHeader::Icmpv6, 27, false)
    );
    // The in-line 64 bits follow the link-local prefix.
    let src = ip.source_address(None).unwrap();
    assert_eq!(src[..8], [0xFE, 0x80, 0, 0, 0, 0, 0, 0]);
    assert_eq!(src[8..], [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn t7_and_t8_other_payloads_and_cut_off_headers() {
    let d = read(&[0x01, 0x02]);
    assert_eq!(
        (d.next, d.iphc, d.truncated),
        (NextHeader::NotLowpan, None, false)
    );
    // A lone first byte of IPHC, and a mesh header without its addresses.
    for b in [vec![0x7F], vec![0xB5, 0x44]] {
        let d = read(&b);
        assert!(d.truncated, "{b:02X?}");
        assert_eq!(d.next, NextHeader::None);
        assert_eq!(d.payload, b.len());
    }
    assert_eq!(read(&[]).next, NextHeader::None);
    // Every prefix of a long chain of headers is read without a failure.
    let whole = hex("B5 44 00 10 00 C1 2C 00 07 7F 3B 02 F0 4D 4C 4D 4C 12 34 FF 10");
    for n in 0..whole.len() {
        let d = read(&whole[..n]);
        assert!(d.payload <= n);
    }
    assert!(!read(&whole).truncated);
}

#[test]
fn the_mesh_header_takes_extended_addresses_and_deep_hops() {
    // V = 0 and F = 0: two 64-bit addresses. Hops left 15: the count is in the next byte (RFC 4944 section 5.2, RFC 6282
    // section 2 for the escape value).
    let mut b = vec![0x8F, 0x20];
    b.extend([0x11; 8]);
    b.extend([0x22; 8]);
    b.extend([0x41]);
    let d = read(&b);
    let m = d.mesh.unwrap();
    assert_eq!(m.hops_left, 32);
    assert_eq!(m.originator, LinkAddress::Extended([0x11; 8]));
    assert_eq!(m.final_destination, LinkAddress::Extended([0x22; 8]));
    assert_eq!(d.next, NextHeader::Ipv6);
}

#[test]
fn stateless_addresses_are_rebuilt_from_the_link_layer_addresses() {
    // TF 3, NH in line (UDP), hop limit 64, SAM 3 and DAM 3 without contexts: 7A 33, then next header 17 and a plain UDP header.
    let d = read(&hex("7A 33 11 16 33 16 33 00 0C AB CD"));
    let ip = d.iphc.unwrap();
    assert_eq!(ip.hop_limit, 64);
    assert_eq!(ip.source, SrcAddress::Elided { context: false });
    assert_eq!(ip.destination, DstAddress::Elided { context: false });
    // An EUI-64 gives the interface identifier with the universal/local bit inverted.
    let ext = LinkAddress::Extended([0x00, 0x12, 0x4B, 0x00, 0x01, 0x02, 0x03, 0x04]);
    let a = ip.source_address(Some(ext)).unwrap();
    assert_eq!(a[..2], [0xFE, 0x80]);
    assert_eq!(a[8..], [0x02, 0x12, 0x4B, 0x00, 0x01, 0x02, 0x03, 0x04]);
    // A short address gives 0000:00ff:fe00:XXXX.
    let a = ip
        .destination_address(Some(LinkAddress::Short(0x1A2B)))
        .unwrap();
    assert_eq!(a[8..], [0, 0, 0, 0xFF, 0xFE, 0, 0x1A, 0x2B]);
    assert_eq!(ip.source_address(None), None);
    let u = d.udp.unwrap();
    assert_eq!(
        (u.source, u.destination, u.checksum),
        (5683, 5683, Some(0xABCD))
    );
    assert!(!u.compressed);
    assert_eq!(d.payload, 11);
    // With a context the header alone does not give the address.
    let d = read(&hex("7A 77 11 16 33 16 33 00 0C AB CD"));
    let ip = d.iphc.unwrap();
    assert_eq!(ip.source, SrcAddress::Elided { context: true });
    assert_eq!(ip.source_address(Some(ext)), None);
}

#[test]
fn every_address_mode_takes_its_bytes() {
    // TF 0 (4 bytes), CID set, hop limit in line, SAC 1 SAM 2, M 1 DAC 0 DAM 1 (6 bytes): 61 = 011 00 0 01 -> hop limit 1.
    // Base 0x60 0xE9: TF 0, NH in line, HLIM 0; CID 1, SAC 1, SAM 2, M 1, DAC 0, DAM 1.
    let b = hex("60 E9 21 01 02 03 04 3A 40 AA BB 05 A1 A2 A3 A4 A5 99");
    let d = read(&b);
    let ip = d.iphc.unwrap();
    assert_eq!(ip.context, Some(0x21));
    assert_eq!(ip.traffic_len, 4);
    assert_eq!((ip.next_header, ip.hop_limit), (Some(58), 0x40));
    assert_eq!(
        ip.source,
        SrcAddress::Inline {
            bytes: [0, 0, 0, 0, 0, 0, 0xAA, 0xBB],
            len: 2,
            context: true
        }
    );
    let DstAddress::Multicast { len, address, .. } = ip.destination else {
        panic!("a multicast destination");
    };
    assert_eq!(len, 6);
    // ffXX::00XX:XXXX:XXXX
    let a = address.unwrap();
    assert_eq!((a[0], a[1]), (0xFF, 0x05));
    assert_eq!(a[11..], [0xA1, 0xA2, 0xA3, 0xA4, 0xA5]);
    assert_eq!(d.payload, 17);
    // The unspecified source, and the 4-byte multicast form ffXX::00XX:XXXX.
    let d = read(&hex("7B 4A 3A 02 00 00 01"));
    let ip = d.iphc.unwrap();
    assert_eq!(ip.source, SrcAddress::Unspecified);
    assert_eq!(ip.source_address(None), Some([0; 16]));
    let a = ip.destination_address(None).unwrap();
    assert_eq!((a[1], a[13], a[15]), (0x02, 0x00, 0x01));
    // The reserved destination mode.
    let d = read(&hex("7B 34 3A"));
    assert_eq!(d.iphc.unwrap().destination, DstAddress::Reserved);
}

#[test]
fn compressed_extension_headers_are_passed_over_to_reach_udp() {
    // IPHC with a compressed next header, then a hop-by-hop options header (EID 0) whose next header is compressed too
    // (1110 000 1), 4 bytes long, then compressed UDP with the ports in one byte and no checksum.
    let d = read(&hex("7F 3B 02 E1 04 6D 04 00 00 F7 13 55"));
    let u = d.udp.unwrap();
    assert_eq!((u.source, u.destination), (0xF0B1, 0xF0B3));
    assert_eq!((d.next, d.payload), (NextHeader::Udp, 11));
    // The same header with its next header in line (N = 0): ICMPv6.
    let d = read(&hex("7F 3B 02 E0 3A 02 00 00 80"));
    assert_eq!((d.next, d.payload), (NextHeader::Icmpv6, 8));
    // An extension header that runs past the end.
    let d = read(&hex("7F 3B 02 E1 09 6D"));
    assert!(d.truncated);
    // A compressed IPv6 header inside (EID 7) is reported and not opened.
    let d = read(&hex("7F 3B 02 EE 7A 33"));
    assert_eq!(d.next, NextHeader::Compressed(0xEE));
}

#[test]
fn the_udp_port_forms_follow_the_two_port_bits() {
    let ports = |b: &str| {
        let d = read(&hex(&format!("7F 3B 02 {b}")));
        let u = d.udp.unwrap();
        (u.source, u.destination, u.checksum, d.payload)
    };
    assert_eq!(
        ports("F0 12 34 56 78 AA BB"),
        (0x1234, 0x5678, Some(0xAABB), 10)
    );
    assert_eq!(
        ports("F1 12 34 56 AA BB"),
        (0x1234, 0xF056, Some(0xAABB), 9)
    );
    assert_eq!(
        ports("F2 12 34 56 AA BB"),
        (0xF012, 0x3456, Some(0xAABB), 9)
    );
    assert_eq!(ports("F3 5A AA BB"), (0xF0B5, 0xF0BA, Some(0xAABB), 7));
    assert_eq!(ports("F4 12 34 56 78"), (0x1234, 0x5678, None, 8));
    assert!(read(&hex("7F 3B 02 F0 12 34 56")).truncated);
}

#[test]
fn a_coap_message_gives_its_header_and_its_path() {
    // NON, token length 2, 2.05 Content, message id 0xBEEF, token, Uri-Path "a" and "ab" (option 11, then delta 0), payload.
    let m = coap::read(&hex("52 45 BE EF 01 02 B1 61 02 61 62 FF 99")).unwrap();
    assert_eq!((m.kind, m.token_len, m.message_id), (Kind::Non, 2, 0xBEEF));
    assert_eq!(
        (m.code_text().as_str(), m.code_name()),
        ("2.05", Some("Content"))
    );
    assert_eq!(m.uri_path, "a/ab");
    // An option delta in an extension byte: Uri-Host (3), then Uri-Path through delta 8.
    let m = coap::read(&hex("40 02 00 01 31 68 81 63")).unwrap();
    assert_eq!((m.code_name(), m.uri_path.as_str()), (Some("POST"), "c"));
    // Option 60 (Size1) through the delta 13 with one extension byte, after the path.
    let m = coap::read(&hex("60 44 00 02 B1 78 D1 24 10")).unwrap();
    assert_eq!((m.kind, m.uri_path.as_str()), (Kind::Ack, "x"));
    assert_eq!(coap::read(&hex("70 00 00 03")).unwrap().kind, Kind::Rst);
    // Not CoAP: too short, another version, a token length above 8. A cut-off option ends the options.
    assert!(coap::read(&hex("40 01 12")).is_none());
    assert!(coap::read(&hex("80 01 12 34")).is_none());
    assert!(coap::read(&hex("49 01 12 34")).is_none());
    assert_eq!(coap::read(&hex("40 01 12 34 B5 61")).unwrap().uri_path, "");
    assert_eq!(coap::read(&hex("40 A5 12 34")).unwrap().code_text(), "5.05");
}
