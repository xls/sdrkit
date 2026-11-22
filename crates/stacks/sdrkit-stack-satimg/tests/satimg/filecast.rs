//! File broadcasts: segment names, the HRIT mosaic (order, losses, bad segments), and both test signals through the whole
//! receiver with impairments, a wrong PID, and a receiver dropped while data is in flight.

use super::common::{impaired, run};
use sdrkit_codec_image::Raster;
use sdrkit_core::media::Picture;
use sdrkit_core::{InputInfo, Output};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::items::{FileKind, SatFile};
use sdrkit_stack_satimg::modes::filecast::{
    segment_name, test_count, test_files, test_segment, HritMosaics, MosaicOut, Service,
    DESCRIPTOR, GEN_SYMBOL_RATE, GEONETCAST, HIMAWARICAST, PID,
};

#[test]
fn segment_names_are_read() {
    let s = segment_name("IMG_DK01IR1_201703080230_001").unwrap();
    assert_eq!((s.channel.as_str(), s.number), ("IR1", 1));
    assert_eq!(
        segment_name("IMG_DK01B13_201703080230_010")
            .unwrap()
            .channel,
        "B13"
    );
    assert!(segment_name("IMG_XX01IR1_201703080230_001").is_none());
    assert!(segment_name("SOMETHING.txt").is_none());
    assert!(segment_name("IMG_DK01IR1_201703080230_000").is_none());
}

fn segment(n: u32, stamp: &str) -> (String, Vec<u8>) {
    let name = format!("IMG_DK01IR1_{stamp}_{n:03}");
    let s = test_segment(&name, n);
    (name, s)
}

fn pixels_right(p: &Picture, rows: std::ops::Range<u32>) -> bool {
    let r = Raster::from_picture(p).unwrap();
    rows.into_iter()
        .all(|y| (0..640).all(|x| r.data()[(y * 640 + x) as usize] == (test_count(x, y) / 4) as u8))
}

#[test]
fn segments_out_of_order_make_one_right_picture() {
    let mut m = HritMosaics::default();
    let mut done = Vec::new();
    for n in [4, 1, 10, 2, 3, 3, 5, 6, 7, 8, 9] {
        let (name, s) = segment(n, "202611090230");
        assert!(m.push(
            &name,
            &s,
            &mut |o| if let MosaicOut::Done { picture, complete } = o {
                done.push((picture, complete))
            }
        ));
    }
    assert_eq!(done.len(), 1);
    let (p, complete) = &done[0];
    assert!(complete);
    assert_eq!(p.name.as_deref(), Some("IR1_202611090230"));
    let r = Raster::from_picture(p).unwrap();
    assert_eq!((r.width(), r.height()), (640, 640));
    assert!(pixels_right(p, 0..640));
    assert_eq!(m.finished, 1);
}

#[test]
fn a_lost_segment_then_the_next_scan_pushes_out_a_partial_picture() {
    let mut m = HritMosaics::default();
    let mut done = Vec::new();
    for n in (1..=10).filter(|&n| n != 6) {
        let (name, s) = segment(n, "202611090230");
        m.push(&name, &s, &mut |_| {});
    }
    let (name, s) = segment(1, "202611090240");
    m.push(&name, &s, &mut |o| {
        if let MosaicOut::Done { picture, complete } = o {
            done.push((picture, complete))
        }
    });
    assert_eq!(done.len(), 1);
    assert!(!done[0].1);
    assert_eq!(done[0].0.name.as_deref(), Some("IR1_202611090230_partial"));
    assert!(pixels_right(&done[0].0, 0..320));
}

#[test]
fn bad_segments_are_counted() {
    let mut m = HritMosaics::default();
    let (name, s) = segment(2, "202611090230");
    m.push(&name, &s[..s.len() - 100], &mut |_| panic!("a cut segment"));
    assert_eq!(m.bad, 1);
    m.push(&name, &[0u8; 40], &mut |_| panic!("zeros"));
    assert_eq!(m.bad, 2);
    assert!(!m.push("OTHER.DAT", &s, &mut |_| panic!("not a segment")));
    assert_eq!(m.bad, 2);
}

#[test]
fn the_test_files_decompress() {
    let f = test_files(Service::Himawaricast, 0);
    assert_eq!(f.len(), 11);
    assert_eq!(f[0].0, "IMG_DK01IR1_202611090230_001.bz2");
    let plain = sdrkit_codec_bzip2::decompress(&f[10].1, 1000).unwrap();
    assert_eq!(plain, b"sdrkit test notice\n");
    let g = test_files(Service::Geonetcast, 0);
    assert_eq!(g[3].1.len(), 7100);
    assert!(g[3].0.ends_with(&"X".repeat(45)));
}

fn harness(submode: &str, rate: f64, seconds: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .submode(submode)
        .rate(rate)
        .seconds(seconds)
}

#[test]
fn himawaricast_gives_files_and_a_right_picture() {
    let h = impaired(harness(HIMAWARICAST, 6e6, 4.0), Some(10.0), 60e3, 15.0);
    let x = h.generate().expect("a signal");
    let r = run::<Picture>(
        &h,
        &x,
        6e6,
        1002e6,
        1002e6,
        65_536,
        Some("filecast.pictures"),
    );
    assert!(
        r.num("files_complete") >= 11.0,
        "{}",
        r.num("files_complete")
    );
    assert_eq!(r.num("files_failed"), 0.0);
    let pics = r.of::<Picture>();
    let done: Vec<&Picture> = pics
        .iter()
        .filter(|p| p.name.as_deref() == Some("IR1_202611090230"))
        .collect();
    assert!(
        !done.is_empty(),
        "{:?} {} {}",
        pics.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        r.num("pictures_finished"),
        r.num("files_complete")
    );
    let last = done.last().unwrap();
    let rows = Raster::from_picture(last).unwrap();
    assert_eq!(rows.height(), 640);
    assert!(r.num("pictures_finished") >= 1.0);
    assert!(pixels_right(last, 0..640));
}

#[test]
fn geonetcast_gives_files_without_the_tag() {
    let h = impaired(
        harness(GEONETCAST, 8e6, 3.0).set(&GEN_SYMBOL_RATE, 3e6),
        Some(16.0),
        30e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<SatFile>(&h, &x, 8e6, 1070e6, 1070e6, 65_536, Some("filecast.files"));
    let files: Vec<SatFile> = r
        .of::<SatFile>()
        .into_iter()
        .filter(|f| f.complete)
        .collect();
    assert!(files.len() >= 5, "{} files", files.len());
    let want = test_files(Service::Geonetcast, 0);
    for f in &files {
        assert_eq!(f.kind, FileKind::Complete);
        assert!(!f.name.contains('X'), "{}", f.name);
        let w = want
            .iter()
            .find(|(n, _)| n.starts_with(&f.name))
            .expect("a test file");
        assert_eq!(&f.bytes[..], &w.1[..]);
    }
}

#[test]
fn a_wrong_pid_gives_no_datagram_and_dropping_the_receiver_is_safe() {
    let h = harness(HIMAWARICAST, 6e6, 1.5);
    let x = h.generate().expect("a signal");
    let wrong = h.clone().set(&PID, 300);
    let r = run::<SatFile>(&wrong, &x, 6e6, 1002e6, 1002e6, 65_536, None);
    assert_eq!(r.num("datagrams"), 0.0);
    assert!(r.num("pid") == 300.0);
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(6e6, 1002e6, 1002e6)).unwrap();
    let mut out = Output::default();
    rx.feed(&x[..x.len() / 2], &mut out);
    drop(rx);
}

/// The rows of `data/plans/dvbs2_products.csv`: each row's values are taken by its stack's settings, and the stack builds.
#[test]
fn the_data_file_rows_fit_the_stacks() {
    use sdrkit_core::{StackConfig, Values};
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../data/plans/dvbs2_products.csv"
    );
    let text = std::fs::read_to_string(path).expect("the data file");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let head: Vec<&str> = lines.next().expect("a heading").split(',').collect();
    assert_eq!(
        head[..10],
        [
            "band",
            "submode",
            "name",
            "centre_mhz",
            "width_mhz",
            "downlink_mhz",
            "symbol_rate",
            "roll_off",
            "pid",
            "part_bytes"
        ]
    );
    let stacks = sdrkit_stack_satimg::descriptors();
    let mut n = 0;
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let d = stacks
            .iter()
            .find(|d| d.id == f[0])
            .expect("a stack of this crate");
        let schema = (d.settings)();
        let mut v = Values::defaults(&schema);
        let rs: f64 = f[6].parse().unwrap();
        v.set(
            schema.id("symbol_rate").unwrap(),
            &sdrkit_core::Value::Hz(rs),
        )
        .unwrap();
        for (k, col) in [("pid", 8), ("part_bytes", 9)] {
            if !f[col].is_empty() {
                let x: i64 = f[col].parse().unwrap();
                v.set(schema.id(k).unwrap(), &sdrkit_core::Value::I64(x))
                    .unwrap();
            }
        }
        let cfg = StackConfig::new(v).with_submode_opt((!f[1].is_empty()).then(|| f[1].to_owned()));
        assert!((d.make_receiver)(&cfg).is_ok(), "{l}");
        n += 1;
    }
    assert_eq!(n, 4);
}

#[test]
fn a_huge_segment_header_counts_as_bad() {
    use sdrkit_proto_ccsds::xrit::headers::Builder;
    let name = "IMG_DK01IR1_202611090230_001";
    let mut f = Builder::new(0)
        .image(16, 22_000, 22_000, 0)
        .annotation(name)
        .record(128, &[1, 64, 0, 1])
        .build(16);
    f.extend_from_slice(&[0; 16]);
    let mut m = HritMosaics::default();
    assert!(m.push(name, &f, &mut |_| panic!("no picture")));
    assert_eq!(m.bad, 1);
}

/// A change of frequency keeps the list of files and the pictures, and the counts start again.
#[test]
fn a_change_of_frequency_keeps_the_files_and_the_pictures() {
    use sdrkit_core::{Owner, Value};
    let h = impaired(harness(HIMAWARICAST, 6e6, 4.0), Some(10.0), 60e3, 15.0);
    let x = h.generate().expect("a signal");
    let mut r = run::<Picture>(&h, &x, 6e6, 1002e6, 1002e6, 65_536, None);
    let rows = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let r = Owner::results(rx);
        r.decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len())
    };
    let (files, pictures) = (
        rows(r.rx.as_ref(), "recent_files"),
        rows(r.rx.as_ref(), "pictures"),
    );
    let complete = r.num("files_complete") as i64;
    assert!(files > 0 && pictures > 0 && complete >= 11);
    r.rx.forget();
    let mut out = Output::default();
    for piece in vec![sdrkit_core::Complex32::default(); 1_500_000].chunks(65_536) {
        r.rx.feed(piece, &mut out);
    }
    assert!(rows(r.rx.as_ref(), "recent_files") >= files);
    assert_eq!(rows(r.rx.as_ref(), "pictures"), pictures);
    let res = Owner::results(r.rx.as_ref());
    let id = res.decl().field_id("files_complete").expect("the field");
    // The frames that the DVB-S2 receiver held at the change still come out: the count starts again from them.
    match res.fields().value(id) {
        Some(Value::I64(n)) => assert!(*n < complete, "{n} of {complete} files"),
        v => panic!("{v:?}"),
    }
}

#[test]
fn interleaved_channels_are_two_pictures_and_each_ends_finished() {
    use sdrkit_core::PictureState::{Finished, Growing};
    let mut m = HritMosaics::default();
    // (channel, number, state) of every picture item, in the order sent.
    let mut seen: Vec<(String, u32, sdrkit_core::PictureState)> = Vec::new();
    let mut push = |m: &mut HritMosaics, channel: &str, n: u32, stamp: &str| {
        let name = format!("IMG_DK01{channel}_{stamp}_{n:03}");
        let s = test_segment(&name, n);
        assert!(m.push(&name, &s, &mut |o| {
            let p = match o {
                MosaicOut::Progress(p) => p,
                MosaicOut::Done { picture, .. } => picture,
            };
            seen.push((channel.to_string(), p.id, p.state));
        }));
    };
    for n in 1..=10 {
        push(&mut m, "IR1", n, "202611090230");
        push(&mut m, "B13", n, "202611090230");
    }
    // The next scan of one channel is a new picture.
    push(&mut m, "IR1", 1, "202611090240");
    let of = |c: &str| -> Vec<(u32, sdrkit_core::PictureState)> {
        seen.iter()
            .filter(|s| s.0 == c)
            .map(|s| (s.1, s.2))
            .collect()
    };
    let (ir, b13) = (of("IR1"), of("B13"));
    assert_eq!(ir.len(), 11);
    assert_eq!(b13.len(), 10);
    let (a, b, next) = (ir[0].0, b13[0].0, ir[10].0);
    assert!(
        a != b && next != a && next != b && a != 0 && b != 0,
        "{seen:?}"
    );
    for (list, id) in [(&ir[..10], a), (&b13[..], b)] {
        assert!(list[..9].iter().all(|p| *p == (id, Growing)), "{seen:?}");
        assert_eq!(list[9], (id, Finished), "{seen:?}");
    }
    assert_eq!(ir[10], (next, Growing));
    // The end of the input ends the picture that still waits for segments.
    let mut last = Vec::new();
    m.flush(&mut |o| {
        if let MosaicOut::Done { picture, .. } = o {
            last.push((picture.id, picture.state));
        }
    });
    assert_eq!(last, [(next, Finished)]);
}
