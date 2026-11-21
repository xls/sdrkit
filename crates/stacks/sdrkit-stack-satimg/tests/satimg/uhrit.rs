//! GK-2A UHRIT: the computed packet and CADU, files across CADUs with frame count jumps, a damaged CADU, and the test signal
//! through the whole receiver.

use super::common::{impaired, run};
use sdrkit_dev_test::Harness;
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::cadufind::CaduFinder;
use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_stack_satimg::items::XritFile;
use sdrkit_stack_satimg::modes::s2::{cadu_2048, ZONE_BYTES};
use sdrkit_stack_satimg::modes::uhrit::{
    file_packets, test_file, UhritDecoder, UhritFile, DESCRIPTOR, SECONDARY,
};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// The packet of the computed example: APID 020, unsegmented, sequence 0, a 35-byte file with the annotation "ABC".
fn example() -> Vec<u8> {
    let mut seq = 0;
    let mut p = file_packets(0x020, &test_file("ABC", &[]), 3000, &mut seq);
    assert_eq!(p.len(), 1);
    p.remove(0)
}

/// The files of `cadus` through the finder, the frame check, the packets and the file layer.
fn decode(stream: &[u8]) -> (Vec<UhritFile>, u64) {
    let mut finder = CaduFinder::new(2048);
    let layout = Layout::CADU_2048;
    let mut asm = Assembler::new(AssemblerConfig {
        uncounted_secondary: SECONDARY,
        ..AssemblerConfig::default()
    });
    let mut d = UhritDecoder::default();
    let mut files = Vec::new();
    let mut crc_bad = 0;
    finder.push(stream, |c| {
        let f = layout.parse(&c[4..]).unwrap();
        if f.fecf_ok != Some(true) {
            crc_bad += 1;
            return;
        }
        asm.push_mpdu(f.header.vcid, Some(f.header.counter), f.first_header, f.zone, |p| {
            let mut whole = p.header.bytes().unwrap().to_vec();
            whole.extend_from_slice(p.data);
            d.push(f.header.vcid, &whole, &mut |x| files.push(x));
        });
    });
    (files, crc_bad)
}

#[test]
fn the_computed_packet_and_cadu_are_reproduced() {
    let p = example();
    assert_eq!(
        p,
        [
            0x08, 0x20, 0xC0, 0x00, 0x00, 0x23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x16, 0, 0, 0, 0, 0, 0, 0, 0, 0x04, 0x00,
            0x06, 0x41, 0x42, 0x43, 0x4E, 0x14, 0x39, 0xA4
        ]
    );
    let mut zone = p.clone();
    zone.resize(ZONE_BYTES, 0);
    let c = cadu_2048(1, 7, 0, FirstHeader::At(0), &zone);
    assert_eq!(
        c[..12],
        [0x1A, 0xCF, 0xFC, 0x1D, 0x40, 0x47, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(c[2046..], [0x12, 0x8C]);
    assert_eq!(CRC16_IBM_3740.checksum(b"123456789"), 0x29B1);
    let mut stream = c.clone();
    stream.extend_from_slice(&c[..4]);
    let (files, _) = decode(&stream);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "ABC");
    assert_eq!(files[0].bytes.len(), 22);
}

/// CADUs of `files`, each file starting a new frame, the frame count jumping by 10 between files.
fn cadus_of(files: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut counter = 0u32;
    let mut seq = 0;
    for f in files {
        let mut stream = Vec::new();
        for p in file_packets(0x020, f, 6000, &mut seq) {
            stream.extend(p);
        }
        let room = stream.len().next_multiple_of(ZONE_BYTES) - stream.len();
        if room > 0 {
            sdrkit_proto_ccsds::packet::idle(room.max(7), &mut stream).unwrap();
        }
        for (i, z) in stream.chunks(ZONE_BYTES).enumerate() {
            let fh = if i == 0 {
                FirstHeader::At(0)
            } else {
                FirstHeader::None
            };
            out.extend(cadu_2048(0x12, 7, counter, fh, z));
            counter += 1;
        }
        counter += 9;
    }
    out.extend_from_slice(&[0x1A, 0xCF, 0xFC, 0x1D]);
    out
}

#[test]
fn files_across_cadus_survive_frame_count_jumps() {
    let files: Vec<Vec<u8>> = (0..3u8)
        .map(|k| test_file("ABC", &vec![k; 5000 - 22]))
        .collect();
    let (got, bad) = decode(&cadus_of(&files));
    assert_eq!(bad, 0);
    assert_eq!(got.len(), 3);
    assert_eq!(got[2].name, "ABC");
    assert_eq!(got[2].bytes, files[2]);
}

#[test]
fn a_flipped_bit_fails_the_frame_crc() {
    let mut c = cadus_of(&[test_file("ABC", &[])]);
    c[100] ^= 0x04;
    let (got, bad) = decode(&c);
    assert_eq!(bad, 1);
    assert!(got.is_empty());
}

#[test]
fn the_test_signal_gives_files() {
    let h = impaired(Harness::new(&DESCRIPTOR).rate(30e6).seconds(0.4), Some(16.0), 10e3, 0.0);
    let x = h.generate().expect("a signal");
    let r = run::<XritFile>(&h, &x, 30e6, 8070e6, 8070e6, 65_536, Some("uhrit.files"));
    let files = r.of::<XritFile>();
    assert!(files.len() >= 2, "{} files, {} packets", files.len(), r.num("space_packets"));
    assert!(r.text("last_file").contains("IMG_TEST"), "{}", r.text("last_file"));
    assert_eq!(r.num("frames_crc_bad"), 0.0);
    assert_eq!(r.num("packets_crc_bad"), 0.0);
}

#[test]
fn a_change_of_frequency_keeps_the_products_and_forgets_the_frames() {
    use sdrkit_core::{InputInfo, Output, Owner};
    let h = impaired(Harness::new(&DESCRIPTOR).rate(30e6).seconds(0.4), Some(16.0), 10e3, 0.0);
    let x = h.generate().expect("a signal");
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(30e6, 8070e6, 8070e6).recording())
        .expect("the receiver takes the rate");
    let mut out = Output::default();
    for piece in x.chunks(65_536) {
        rx.feed(piece, &mut out);
    }
    let rows = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let r = Owner::results(rx);
        r.decl().table_id(key).and_then(|id| r.table(id)).map_or(0, |t| t.len())
    };
    let products = rows(rx.as_ref(), "products");
    assert!(products > 0);
    assert!(rows(rx.as_ref(), "packets_by_apid") > 0);
    assert!(rows(rx.as_ref(), "frames") > 0);
    rx.forget();
    assert_eq!(rows(rx.as_ref(), "products"), products);
    assert_eq!(rows(rx.as_ref(), "packets_by_apid"), 0);
    assert_eq!(rows(rx.as_ref(), "frames"), 0);
    // The counts start again with the carrier of the new frequency.
    let cadus = |rx: &dyn sdrkit_core::Receiver| {
        let r = Owner::results(rx);
        let id = r.decl().field_id("cadus").expect("the field");
        match r.fields().value(id) {
            Some(sdrkit_core::Value::I64(n)) => *n,
            _ => 0,
        }
    };
    // A second receiver takes the same samples without the change of frequency: its counts hold the first pass.
    let mut whole = h.receiver().expect("the receiver");
    whole
        .configure(&InputInfo::new(30e6, 8070e6, 8070e6).recording())
        .expect("the receiver takes the rate");
    let half = &x[..x.len() / 2];
    for piece in x.chunks(65_536).chain(half.chunks(65_536)) {
        whole.feed(piece, &mut out);
    }
    for piece in half.chunks(65_536) {
        rx.feed(piece, &mut out);
    }
    let (after, both) = (cadus(rx.as_ref()), cadus(whole.as_ref()));
    assert!(after > 0 && after < both, "{after} of {both}");
}
