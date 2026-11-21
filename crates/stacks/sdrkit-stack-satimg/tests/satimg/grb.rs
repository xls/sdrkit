//! GOES-R Rebroadcast: the computed packet, a damaged packet, the CADUs of the test signal through the frame layer, and the
//! test signal through the whole receiver with noise and an offset.

use super::common::{impaired, run};
use sdrkit_codec_image::strip::Strip;
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::items::FileKind;
use sdrkit_stack_satimg::modes::grb::{
    generic_payload, payload_packets, test_pixel, GrbDecoder, GrbOut, Product, TestCadus,
    DESCRIPTOR, TEST_APID,
};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

/// The packet of the computed example: APID 580, unsegmented, sequence 0, variant 0, seconds 123456, data 01 02 03 04.
fn example() -> Vec<u8> {
    let mut seq = 0;
    let mut p = payload_packets(0x580, 0, &generic_payload(123_456, &[1, 2, 3, 4]), &mut seq);
    assert_eq!(p.len(), 1);
    p.remove(0)
}

#[test]
fn the_computed_packet_gives_its_file() {
    let p = example();
    assert_eq!(p[..6], [0x0D, 0x80, 0xC0, 0x00, 0x00, 0x24]);
    assert_eq!(p[14..19], [0x00, 0x00, 0x01, 0xE2, 0x40]);
    assert_eq!(p[p.len() - 4..], [0x6D, 0x56, 0x58, 0xB6]);
    assert_eq!(CRC32_ISO_HDLC.checksum(b"123456789"), 0xCBF4_3926);
    let mut d = GrbDecoder::default();
    let mut files = Vec::new();
    d.push(5, &p, 0.0, &mut |o| {
        if let GrbOut::File(f) = o {
            files.push(f)
        }
    });
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "580_123456.xml");
    assert_eq!(&files[0].bytes[..], &[1, 2, 3, 4]);
    assert_eq!(files[0].kind, FileKind::GrbGeneric);
    assert_eq!(d.stats.crc_bad, 0);
}

#[test]
fn a_flipped_bit_fails_the_crc() {
    let mut p = example();
    p[40] ^= 0x10;
    let mut d = GrbDecoder::default();
    d.push(5, &p, 0.0, &mut |_| panic!("no file from a damaged packet"));
    assert_eq!(d.stats.crc_bad, 1);
}

#[test]
fn the_apid_table_names_products() {
    assert_eq!(Product::of(0x0BC), Product::AbiConus(13));
    assert_eq!(Product::of(0x0BC).abi_geometry(), Some((2500, 1500, 2)));
    assert_eq!(Product::of(0x091).abi_geometry(), Some((21_696, 21_696, 8)));
    assert_eq!(Product::of(0x488).name(), "SUVI Fe171");
    assert_eq!(Product::of(0x302), Product::Glm(2));
    assert_eq!(Product::of(0x085), Product::AbiMetadata);
}

/// Checks the strips of APID 0BC against the test formula (4 km reduction: every second row and column); returns the rows
/// that were right.
fn right_rows(strips: &[Strip]) -> usize {
    let mut rows = std::collections::BTreeSet::new();
    for s in strips.iter().filter(|s| s.picture == u32::from(TEST_APID)) {
        for r in 0..u32::from(s.rows) {
            let ry = s.first_row + r;
            let gy = 2 * ry;
            let ok = (0..s.width).all(|c| {
                let gx = 2 * (s.x + c);
                let want = (test_pixel(gx % 64, gy % 16, (gy / 16) % 40) >> 4) as u8;
                gx >= 64 || s.samples[(r * s.width + c) as usize] == want
            });
            if ok {
                rows.insert(ry);
            }
        }
    }
    rows.len()
}

#[test]
fn the_test_cadus_give_pictures_and_files() {
    let mut cadus = TestCadus::new().expect("the test blocks");
    let mut stream = Vec::new();
    for _ in 0..60 {
        cadus.next_cadu(&mut stream);
    }
    let mut finder = sdrkit_proto_ccsds::cadufind::CaduFinder::new(2048);
    let layout = sdrkit_proto_ccsds::aos::Layout::CADU_2048;
    let mut asm = sdrkit_proto_ccsds::packet::Assembler::new(Default::default());
    let mut d = GrbDecoder::default();
    let mut strips = Vec::new();
    let mut files = 0;
    finder.push(&stream, |c| {
        let f = layout.parse(&c[4..]).unwrap();
        assert_eq!((f.header.spacecraft, f.header.vcid), (0x12, 5));
        asm.push_mpdu(5, Some(f.header.counter), f.first_header, f.zone, |p| {
            let mut whole = p.header.bytes().unwrap().to_vec();
            whole.extend_from_slice(p.data);
            d.push(5, &whole, 0.0, &mut |o| match o {
                GrbOut::Strip(s) => strips.push(s),
                GrbOut::File(_) => files += 1,
            });
        });
    });
    assert!(files >= 10, "{files}");
    assert!(right_rows(&strips) >= 20, "{}", right_rows(&strips));
    assert_eq!(d.stats.crc_bad, 0);
}

#[test]
fn the_test_signal_gives_packets_and_a_right_picture() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(20e6).seconds(1.0),
        Some(14.0),
        10e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, 20e6, 1686.6e6, 1686.6e6, 65_536, Some("grb.strips"));
    assert!(r.num("space_packets") >= 50.0, "{}", r.num("space_packets"));
    assert!(
        r.num("picture_blocks") >= 1.0,
        "{}",
        r.num("picture_blocks")
    );
    assert!(
        right_rows(&r.of::<Strip>()) >= 3,
        "{}",
        right_rows(&r.of::<Strip>())
    );
    assert_eq!(r.num("packets_crc_bad"), 0.0);
    assert!(!r.text("shown_product").is_empty());
}

#[test]
fn hostile_block_headers_are_refused_without_a_fault() {
    let stream = sdrkit_stack_satimg::modes::grb::test_block(0).unwrap();
    for (apid, at, v) in [
        (0x486u16, 18usize, 0xFFFF_FFF0u32),
        (0x486, 22, 0xFFFF_FFFF),
        (0x486, 26, 0xFFFF_FFFF),
        (0x0BC, 14, 0xFFFF_FFFF),
        (0x0BC, 18, 0xFFFF_FFFF),
    ] {
        let mut pl = sdrkit_stack_satimg::modes::grb::block_payload(0, &stream);
        pl[at..at + 4].copy_from_slice(&v.to_be_bytes());
        let mut seq = 0;
        let mut d = GrbDecoder::default();
        for p in payload_packets(apid, 2, &pl, &mut seq) {
            d.push(6, &p, 0.0, &mut |_| {});
        }
        assert_eq!(d.stats.payloads, 1);
    }
}

/// A change of frequency keeps the list of products and forgets the frames and the packets of the carrier.
#[test]
fn a_change_of_frequency_keeps_the_products_and_forgets_the_frames() {
    use sdrkit_core::Owner;
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(20e6).seconds(1.0),
        Some(14.0),
        10e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let mut r = run::<Strip>(&h, &x, 20e6, 1686.6e6, 1686.6e6, 65_536, None);
    let rows = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let r = Owner::results(rx);
        r.decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len())
    };
    let products = rows(r.rx.as_ref(), "grb_products");
    assert!(products > 0);
    assert!(rows(r.rx.as_ref(), "packets_by_apid") > 0);
    assert!(rows(r.rx.as_ref(), "frames") > 0);
    r.rx.forget();
    assert_eq!(rows(r.rx.as_ref(), "grb_products"), products);
    assert_eq!(rows(r.rx.as_ref(), "packets_by_apid"), 0);
    assert_eq!(rows(r.rx.as_ref(), "frames"), 0);
}
