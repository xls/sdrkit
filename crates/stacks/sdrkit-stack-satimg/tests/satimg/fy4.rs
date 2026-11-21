//! FY-4: the transport stream, the CADU import and the file layer, and JPEG 2000 segments in the product store.

use super::common::{assert_declared, feed, run, Ran};
use sdrkit_core::{Complex32, InputInfo};
use sdrkit_dev_test::Harness;
use sdrkit_graph::{Graph, Picture};
use sdrkit_stack_satimg::components::xrit::CaduImportComponent;
use sdrkit_stack_satimg::items::ProductKind;
use sdrkit_stack_satimg::modes::fy4::{
    declaration, image_record, pack_cadus, test_cadus, test_files, test_picture, TsCaduExtractor,
    HRIT, LRIT,
};
use sdrkit_stack_satimg::typed::ProductEvent;

/// Imports `cadus` into a fresh FY-4 LRIT receiver fed with silence.
fn import(cadus: &[[u8; 1024]]) -> Ran {
    let mut rx = Harness::new(&LRIT).receiver().unwrap();
    rx.configure(&InputInfo::new(2e6, 1697e6, 1697e6).recording())
        .unwrap();
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    g.component_as_mut::<CaduImportComponent>("import")
        .expect("the CADU import")
        .queue(cadus.concat());
    feed::<Picture>(
        rx,
        &vec![Complex32::default(); 1_000_000],
        100_000,
        Some("products.pictures"),
    )
}

#[test]
fn two_rounds_of_cadus_give_the_pictures() {
    let r = import(&test_cadus(2, 40));
    assert_eq!(r.num("files"), 10.0);
    assert_eq!(r.num("image_segments"), 8.0);
    assert_eq!(r.num("pictures_complete"), 4.0);
    let mut pics: Vec<Picture> = r.of::<Picture>();
    pics.sort_by_key(|p| p.name.clone());
    pics.dedup_by_key(|p| p.name.clone());
    assert_eq!(pics.len(), 4);
    for p in &pics {
        let ch = if p.name.as_deref().unwrap_or("").contains("C02") {
            2
        } else {
            12
        };
        let want: Vec<u8> = test_picture(ch).iter().map(|&v| (v >> 4) as u8).collect();
        assert_eq!(&p.data[..], &want[..]);
    }
    assert!(r
        .events
        .iter()
        .any(|e| e.at(&ProductEvent::KIND) == Some(ProductKind::Text)));
    assert_declared(&r, &declaration());
}

#[test]
fn an_encrypted_file_and_a_damaged_segment_are_listed() {
    use sdrkit_proto_ccsds::xrit::headers::Builder;
    let mut files = test_files(0);
    // Cut the second C02 segment short.
    let (apid, f) = files[1].clone();
    files[1] = (apid, f[..f.len() - f.len() / 3].to_vec());
    let mut enc = Builder::new(0)
        .record(1, &image_record(7, 1, 1, 0, 0))
        .record(7, &0x1234u32.to_be_bytes())
        .annotation("FY4A_ENCRYPTED_TEST")
        .build(16);
    enc.extend_from_slice(&[0x5A; 16]);
    files.push((150, enc));
    let r = import(&pack_cadus(vec![files], 4));
    assert_eq!(r.num("encrypted_files"), 1.0);
    let notes: Vec<String> = r
        .events
        .iter()
        .filter_map(|e| e.at(&ProductEvent::NOTE).map(String::from))
        .collect();
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("Encrypted file (key 4660)")),
        "{notes:?}"
    );
    assert!(r.events.iter().any(|e| {
        e.at(&ProductEvent::NAME).is_some_and(|n| n.contains("C02"))
            && e.at(&ProductEvent::ERRORS) == Some(true)
    }));
}

#[test]
fn cadus_in_the_transport_stream_come_back_in_order() {
    let cadus = test_cadus(1, 3);
    let ts = sdrkit_stack_satimg::modes::fy4::test_ts(&cadus, 0x100);
    let mut x = TsCaduExtractor::default();
    let mut out = Vec::new();
    for p in &ts {
        x.push(p.bytes(), &mut out);
    }
    assert_eq!(out.len(), cadus.len());
    for (a, b) in out.iter().zip(&cadus) {
        assert_eq!(&a.bytes, b);
    }
}

#[test]
fn noise_only_searches() {
    let h = Harness::new(&LRIT);
    let r = run::<Picture>(
        &h,
        &vec![Complex32::new(0.01, 0.0); 1_000_000],
        2e6,
        1697e6,
        1697e6,
        100_000,
        None,
    );
    assert!(
        r.results.status().summary.contains("searching"),
        "{}",
        r.results.status().summary
    );
    assert_eq!(r.num("cadus"), 0.0);
}

#[test]
fn the_hrit_test_signal_gives_files() {
    let h = super::common::impaired(
        Harness::new(&HRIT).rate(2e6).seconds(3.0),
        Some(12.0),
        5e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Picture>(&h, &x, 2e6, 1679e6, 1679e6, 65_536, None);
    assert!(r.num("cadus") >= 20.0, "{}", r.num("cadus"));
    assert!(r.num("files") >= 1.0, "{}", r.num("files"));
    assert!(r.num("bch_ok") > 0.0);
}

#[test]
fn the_lrit_test_signal_gives_cadus() {
    let h = super::common::impaired(
        Harness::new(&LRIT).rate(250e3).seconds(14.0),
        Some(6.0),
        1e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Picture>(&h, &x, 250e3, 1697e6, 1697e6, 65_536, None);
    assert!(
        r.num("cadus") >= 10.0,
        "{} cadus, {} BCH good",
        r.num("cadus"),
        r.num("bch_ok")
    );
}

#[test]
fn a_rate_too_low_says_so() {
    let mut rx = Harness::new(&LRIT).receiver().unwrap();
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(100e3, 1697e6, 1697e6))
        .is_err());
}

/// A JPEG 2000 segment decodes in the store into its product picture (the file layer of GK-2A and FY-4).
#[test]
fn a_jpeg_2000_segment_decodes_into_its_picture() {
    use sdrkit_proto_ccsds::xrit::headers::{parse, Builder, Dialect};
    use sdrkit_stack_satimg::items::XritFile;
    use sdrkit_stack_satimg::xrit::store::{Changes, Store, StoreConfig};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../codec/sdrkit-codec-image/tests/data/grey_sips.jp2");
    let j2k = std::fs::read(&path).expect("the JPEG 2000 file of sdrkit-codec-image");
    let pic = sdrkit_codec_image::j2k::decode(&j2k).unwrap();
    let mut file = Builder::new(0)
        .image(8, pic.width as u16, pic.height as u16, 1)
        .annotation("IMG_FD_001_IR105_20261101_030000_01.lrit")
        .record(128, &[1, 1, 0, 0])
        .build(j2k.len());
    file.extend_from_slice(&j2k);
    let headers = parse(&file, Dialect::Gk2a).unwrap();
    let mut store = Store::new(StoreConfig::gk2a("LRIT"));
    let mut ch = Changes::default();
    store.push(
        XritFile {
            vcid: 0,
            apid: 12,
            bytes: file.as_slice().into(),
            packet_starts: vec![0],
            packet_index: vec![0],
            headers,
            errors: false,
            truncated: false,
            time: 0.0,
        },
        &mut ch,
    );
    let p = store.products.back().unwrap();
    let shift = (pic.precision as u32).saturating_sub(8);
    let want: Vec<u8> = pic.components[0]
        .iter()
        .map(|&v| ((v as u32) >> shift).min(255) as u8)
        .collect();
    assert_eq!(p.picture.as_deref(), Some(&want[..]));
    assert!(p.complete);
}

/// A change of frequency keeps the products and the pictures of the store, and the CADU counts start again.
#[test]
fn a_change_of_frequency_keeps_the_products_and_forgets_the_cadus() {
    use sdrkit_core::{Output, Owner};
    let h = super::common::impaired(
        Harness::new(&HRIT).rate(2e6).seconds(3.0),
        Some(12.0),
        5e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let mut r = run::<Picture>(&h, &x, 2e6, 1679e6, 1679e6, 65_536, None);
    let rows = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let r = Owner::results(rx);
        r.decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len())
    };
    let products = rows(r.rx.as_ref(), "products");
    assert!(products > 0);
    assert!(r.num("cadus") >= 20.0);
    let count = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let res = Owner::results(rx);
        match res.decl().field_id(key).and_then(|id| res.fields().value(id)) {
            Some(sdrkit_core::Value::I64(n)) => *n,
            v => panic!("{key}: {v:?}"),
        }
    };
    let packets = ["ts_packets", "s2_bbframe.frames_ok"];
    for key in packets {
        assert!(count(r.rx.as_ref(), key) > 0, "{key}");
    }
    r.rx.forget();
    assert_eq!(rows(r.rx.as_ref(), "products"), products);
    // The transport packets and the frames counted are of the carrier before.
    for key in packets {
        assert_eq!(count(r.rx.as_ref(), key), 0, "{key}");
    }
    // Silence after the change: the counts of the display are those of the new carrier.
    let mut out = Output::default();
    for piece in vec![Complex32::default(); 1_000_000].chunks(65_536) {
        r.rx.feed(piece, &mut out);
    }
    assert_eq!(rows(r.rx.as_ref(), "products"), products);
    let ts = |key: &str| {
        let res = Owner::results(r.rx.as_ref());
        res.decl()
            .field_id(key)
            .and_then(|id| res.fields().value(id).cloned())
    };
    // The frames that the DVB-S2 receiver held at the change still come out: the count starts again from them.
    let before = r.num("cadus") as i64;
    match ts("cadus") {
        Some(sdrkit_core::Value::I64(n)) => assert!(n < before, "{n} of {before} CADUs"),
        v => panic!("{v:?}"),
    }
}
