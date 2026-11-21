//! GOES HRIT, GK-2A LRIT and HRIT and Elektro-L: generator, standard impairments, receiver, and the file and product layers.

use super::common::{
    assert_declared, feed, fields_without, impaired, invoke, run, set, table_rows, Ran,
};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, StackDescriptor, StateBlob};
use sdrkit_dev_test::Harness;
use sdrkit_graph::{Graph, Picture};
use sdrkit_stack_satimg::components::link::{FrameComponent, VcduComponent};
use sdrkit_stack_satimg::components::xrit::FilesComponent;
use sdrkit_stack_satimg::items::XritFile;
use sdrkit_stack_satimg::typed::{GgakFrameEvent, ProductEvent, GGAK_EVENT, TEXT_EVENT};
use sdrkit_stack_satimg::modes::xrit::{
    declaration_for, XritMode, ELEKTRO, GK2A_HRIT, GK2A_LRIT, GOES,
};
use sdrkit_stack_satimg::modes::xrit_gen::{
    gk2a_picture, goes_files, goes_full_disk, goes_mesoscale, goes_relay_jpeg, goes_rice_segment,
    XritFrames, ADMIN_TEXT, EMWIN_TEXT, FULL_DISK_NAME, MESOSCALE_NAME,
};

fn harness(d: &'static StackDescriptor, rate: f64, secs: f64) -> Harness {
    Harness::new(d).rate(rate).seconds(secs)
}

/// Runs the impaired signal of `h` with a tap on the product pictures.
fn go(h: &Harness, rate: f64, x: Option<Vec<Complex32>>) -> Ran {
    let x = x.unwrap_or_else(|| h.generate().expect("a signal"));
    run::<Picture>(
        h,
        &x,
        rate,
        1694.1e6,
        1694.1e6,
        60_000,
        Some("products.pictures"),
    )
}

/// The graph of a receiver.
fn graph(rx: &mut dyn Receiver) -> &mut Graph {
    rx.as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
}

/// Whether a picture named `name` holds exactly `want` (a product sent again in a later round shows partial pictures too).
fn has_picture(r: &Ran, name: &str, want: &[u8]) -> bool {
    r.of::<Picture>()
        .iter()
        .any(|p| p.name.as_deref() == Some(name) && p.data[..] == *want)
}

fn texts(r: &Ran) -> Vec<String> {
    r.events
        .iter()
        .filter(|e| e.is(&TEXT_EVENT))
        .filter_map(|e| e.get(&keys::TEXT).map(String::from))
        .collect()
}

/// Every GOES test product came back exactly.
fn goes_all_good(r: &Ran) {
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked,
        "{}",
        r.results.status().summary
    );
    assert_eq!(r.num("carrier_locked"), 1.0);
    assert!(has_picture(r, FULL_DISK_NAME, &goes_full_disk()));
    assert!(has_picture(r, MESOSCALE_NAME, &goes_mesoscale()));
    assert!(has_picture(r, "relay_test_picture.jpg", &goes_relay_jpeg()));
    let t = texts(r);
    assert!(t.iter().any(|t| t == ADMIN_TEXT), "{t:?}");
    assert!(t.iter().any(|t| t == EMWIN_TEXT), "{t:?}");
    assert_eq!(r.num("crc_errors"), 0.0);
    assert_eq!(r.num("file_errors"), 0.0);
    assert_eq!(r.num("length_errors"), 0.0);
}

fn goes(rate: f64, secs: f64) -> Harness {
    harness(&GOES, rate, secs).set(&sdrkit_stack_satimg::modes::xrit::GEN_FILL, 12)
}

#[test]
fn goes_eight_db_with_offset_and_clock_error() {
    let h = impaired(goes(2.4e6, 1.5), Some(8.0), 35e3, 30.0);
    let r = go(&h, 2.4e6, None);
    goes_all_good(&r);
    assert!(
        (r.num("offset_hz") - 35e3 * (1.0 - 30e-6)).abs() < 300.0,
        "{}",
        r.num("offset_hz")
    );
    assert_eq!(r.num("g2_inverted"), 1.0);
}

#[test]
fn goes_four_db() {
    let r = go(
        &impaired(goes(2.4e6, 2.0), Some(4.0), -12e3, -20.0),
        2.4e6,
        None,
    );
    goes_all_good(&r);
}

#[test]
fn goes_conventional_basis_nrzm_swapped_and_negated() {
    let h = impaired(
        goes(2.4e6, 1.5).set(
            &sdrkit_stack_satimg::modes::xrit::GEN_BASIS,
            sdrkit_stack_satimg::modes::xrit::RsBasis::Conventional,
        ),
        Some(8.0),
        0.0,
        0.0,
    );
    let r = go(&h, 2.4e6, None);
    goes_all_good(&r);
    assert_eq!(r.text("rs_basis"), "conventional");
    let h = impaired(
        goes(2.4e6, 1.5).set(&sdrkit_stack_satimg::modes::xrit::GEN_NRZM, true),
        Some(8.0),
        10e3,
        0.0,
    );
    let r = go(&h, 2.4e6, None);
    goes_all_good(&r);
    assert_eq!(r.num("nrzm"), 1.0);
    // I and Q swapped (a spectrum turned over) and negated.
    let h = impaired(goes(2.4e6, 1.5), Some(8.0), 20e3, 0.0);
    let x: Vec<_> = h
        .generate()
        .unwrap()
        .iter()
        .map(|v| Complex32::new(v.im, v.re))
        .collect();
    goes_all_good(&go(&h, 2.4e6, Some(x)));
    let x: Vec<_> = h.generate().unwrap().iter().map(|v| -*v).collect();
    goes_all_good(&go(&h, 2.4e6, Some(x)));
}

#[test]
fn goes_noise_only() {
    let h = goes(2.4e6, 1.0)
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0);
    let r = go(&h, 2.4e6, None);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert!(r.num("products").is_nan() || r.num("products") == 0.0);
    assert_ne!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
}

#[test]
fn goes_cadu_recording_and_import() {
    use sdrkit_stack_satimg::items::Cadu;
    let h = impaired(goes(2.4e6, 1.5), Some(12.0), 10e3, 0.0);
    let x = h.generate().unwrap();
    // The host records the good CADUs from a tap on the frame layer.
    let r = run::<Cadu>(
        &h,
        &x,
        2.4e6,
        1694.1e6,
        1694.1e6,
        60_000,
        Some("frames.out"),
    );
    let file: Vec<u8> = r
        .of::<Cadu>()
        .iter()
        .filter(|c| c.good())
        .flat_map(|c| c.bytes)
        .collect();
    assert!(file.len() >= 50 * 1024);
    assert_eq!(&file[..4], &[0x1A, 0xCF, 0xFC, 0x1D]);
    // Import into a fresh receiver, fed with silence so it runs.
    let mut rx = goes(2.4e6, 1.0).receiver().unwrap();
    rx.configure(&InputInfo::new(2.4e6, 1694.1e6, 1694.1e6).recording())
        .unwrap();
    graph(&mut *rx)
        .component_as_mut::<FrameComponent>("frames")
        .expect("the frame layer")
        .import(file);
    let r = feed::<Picture>(rx, &vec![Complex32::default(); 600_000], 60_000, None);
    assert!(
        r.num("cadus_imported") >= 50.0,
        "{}",
        r.num("cadus_imported")
    );
    assert!(r.num("products") >= 5.0, "{}", r.num("products"));
}

#[test]
fn goes_any_cut_returns_the_same_products() {
    let h = impaired(goes(2.4e6, 1.0), Some(10.0), 5e3, 0.0);
    let x = h.generate().unwrap();
    let mut all = Vec::new();
    for piece in [3_000, 65_536, x.len()] {
        let r = run::<Picture>(&h, &x, 2.4e6, 1694.1e6, 1694.1e6, piece, None);
        let v = fields_without(&r.results, &["signal_time", "ber", "byte_errors_pct"]);
        let products: Vec<_> = table_rows(&r.results)
            .into_iter()
            .filter(|t| t.0 == "products")
            .collect();
        all.push((v, products));
    }
    assert!(all[0].0.iter().any(|(k, _)| k == "products"));
    assert!(!all[0].1.is_empty());
    for a in &all[1..] {
        assert_eq!(a, &all[0]);
    }
}

#[test]
fn goes_products_survive_through_the_saved_state() {
    let h = impaired(goes(2.4e6, 1.5), Some(10.0), 0.0, 0.0);
    let r = go(&h, 2.4e6, None);
    let products = r.num("products");
    assert!(products >= 5.0);
    let state = r.rx.save_state().expect("a saved state");
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(2.4e6, 1694.1e6, 1694.1e6).recording())
        .unwrap();
    rx.restore_state(&state).unwrap();
    let r = feed::<Picture>(rx, &vec![Complex32::default(); 1_200_000], 60_000, None);
    assert_eq!(r.num("products"), products);
    // Another version is refused and changes nothing.
    let mut rx = h.receiver().unwrap();
    let other = StateBlob::new(GOES.id, 7, state.bytes().to_vec());
    assert!(rx.restore_state(&other).is_err());
}

fn gk2a(d: &'static StackDescriptor, rate: f64, secs: f64) -> Harness {
    harness(d, rate, secs)
}

/// The GK-2A channel pictures that came back exactly.
fn gk2a_pictures_equal(r: &Ran) -> usize {
    ["IR105", "VI006"]
        .iter()
        .filter(|ch| {
            let want: Vec<u8> = gk2a_picture(ch).iter().map(|&v| (v >> 2) as u8).collect();
            has_picture(r, &format!("IMG_FD_001_{ch}_20261101_030000"), &want)
        })
        .count()
}

fn gk2a_run(h: &Harness, rate: f64) -> Ran {
    let x = h.generate().unwrap();
    let f = 1692.14e6;
    run::<Picture>(h, &x, rate, f, f, 60_000, Some("products.pictures"))
}

#[test]
fn gk2a_lrit_decodes_the_pictures() {
    let h = impaired(gk2a(&GK2A_LRIT, 2.4e6, 14.0), Some(6.0), 20e3, 25.0);
    let r = gk2a_run(&h, 2.4e6);
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert!((r.num("offset_hz") - 20e3).abs() < 300.0);
    assert_eq!(r.num("crc_errors"), 0.0);
    assert_eq!(r.num("file_errors"), 0.0);
    assert_eq!(gk2a_pictures_equal(&r), 2);
    assert!(texts(&r).iter().any(|t| t.contains("OBSERVATION SCHEDULE")));
}

#[test]
fn gk2a_lrit_at_1_msps_and_3_db() {
    let h = impaired(gk2a(&GK2A_LRIT, 1e6, 14.0), Some(3.0), -9e3, 0.0);
    let r = gk2a_run(&h, 1e6);
    assert_eq!(gk2a_pictures_equal(&r), 2);
}

#[test]
fn gk2a_encrypted_pictures_need_the_key() {
    let h = impaired(
        gk2a(&GK2A_LRIT, 2.4e6, 14.0).set(&sdrkit_stack_satimg::modes::xrit::GEN_ENCRYPT, true),
        Some(8.0),
        0.0,
        0.0,
    );
    let r = gk2a_run(&h, 2.4e6);
    assert!(r.num("segments_locked") > 0.0);
    assert!(
        r.text("hint").contains("DES-encrypted"),
        "{}",
        r.text("hint")
    );
    assert!(r.events.iter().any(|e| e
        .at(&ProductEvent::NOTE)
        .is_some_and(|n| n.contains("encrypted (KMA key 1), key not loaded"))));
    assert_eq!(gk2a_pictures_equal(&r), 1);
    // With the key (its text form), every picture decodes.
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(2.4e6, 1692.14e6, 1692.14e6).recording())
        .unwrap();
    invoke(&mut *rx, "add_keys", &[("keys", "1:133457799BBCDFF1")]).unwrap();
    let r = feed::<Picture>(rx, &x, 60_000, Some("products.pictures"));
    assert_eq!(r.num("keys"), 1.0);
    assert_eq!(r.num("segments_locked"), 0.0);
    assert_eq!(gk2a_pictures_equal(&r), 2);
    // A value that is not a key is refused.
    let mut rx = h.receiver().unwrap();
    assert!(invoke(&mut *rx, "add_keys", &[("keys", "not a key")]).is_err());
}

#[test]
fn gk2a_hrit_decodes_the_pictures() {
    let h = impaired(gk2a(&GK2A_HRIT, 10e6, 1.5), Some(8.0), 80e3, 15.0);
    let x = h.generate().unwrap();
    let r = run::<Picture>(&h, &x, 10e6, 1695.4e6, 1695.4e6, 200_000, None);
    assert!(r.num("frames_ok") > 20.0, "{}", r.num("frames_ok"));
    assert!(r.num("products") >= 1.0);
}

#[test]
fn gk2a_a_rate_too_low_is_refused() {
    let mut rx = Harness::new(&GK2A_HRIT).receiver().unwrap();
    assert!(rx
        .configure(&InputInfo::new(2.4e6, 1695.4e6, 1695.4e6))
        .is_err());
    assert!(!Owner::results(&*rx).status().summary.is_empty());
}

#[test]
fn elektro_hrit_665k_products() {
    let h = impaired(
        harness(&ELEKTRO, 2.4e6, 2.0).set(&sdrkit_stack_satimg::modes::xrit::GEN_FILL, 12),
        Some(10.0),
        15e3,
        0.0,
    );
    let x = h.generate().unwrap();
    let r = run::<Picture>(&h, &x, 2.4e6, 1691e6, 1691e6, 60_000, None);
    assert!(r.num("products") >= 5.0, "{}", r.num("products"));
    assert!(r.results.status().summary.contains("Elektro-L N3"));
}

#[test]
fn elektro_lrit_64k_link() {
    let h = impaired(
        harness(&ELEKTRO, 400e3, 30.0)
            .set(
                &sdrkit_stack_satimg::modes::xrit::LINK_SETTING,
                sdrkit_stack_satimg::modes::xrit::ElektroLink::Lrit64,
            )
            .set(
                &sdrkit_stack_satimg::modes::xrit::GEN_LINK,
                sdrkit_stack_satimg::modes::xrit::ElektroLink::Lrit64,
            )
            .set(&sdrkit_stack_satimg::modes::xrit::GEN_FILL, 4),
        Some(12.0),
        2e3,
        0.0,
    );
    let x = h.generate().unwrap();
    let r = run::<Picture>(&h, &x, 400e3, 1691e6, 1691e6, 40_000, None);
    assert!(r.num("frames_ok") > 20.0, "{}", r.num("frames_ok"));
    assert_eq!(r.text("link"), "LRIT 64 ksym/s");
}

#[test]
fn elektro_ggak_frames_with_their_field() {
    let h = impaired(
        harness(&ELEKTRO, 240e3, 8.0)
            .set(
                &sdrkit_stack_satimg::modes::xrit::LINK_SETTING,
                sdrkit_stack_satimg::modes::xrit::ElektroLink::Ggak,
            )
            .set(
                &sdrkit_stack_satimg::modes::xrit::GEN_LINK,
                sdrkit_stack_satimg::modes::xrit::ElektroLink::Ggak,
            ),
        Some(15.0),
        800.0,
        0.0,
    );
    let x = h.generate().unwrap();
    let r = run::<Picture>(&h, &x, 240e3, 1693e6, 1693e6, 24_000, None);
    assert!(r.num("ggak_good") >= 4.0, "{}", r.num("ggak_good"));
    assert_eq!(r.num("ggak_bad"), 0.0);
    let e = r
        .events
        .iter()
        .find(|e| e.is(&GGAK_EVENT))
        .expect("a GGAK frame");
    assert_eq!(
        (e.at(&GgakFrameEvent::BX), e.at(&GgakFrameEvent::BY), e.at(&GgakFrameEvent::BZ)),
        (Some(12_000), Some(-8_000), Some(30_000))
    );
    assert!(r.results.status().summary.contains("GGAK"));
    assert!(r
        .results
        .log_lines()
        .iter()
        .any(|l| l.starts_with("GGAK: frames locked (marker ")));
    // A change of link restarts the chain at the new rate, and the counts stay.
    let mut rx = r.rx;
    set(&mut *rx, "link", "hrit665").unwrap();
    let r = feed::<Picture>(rx, &x[..120_000], 24_000, None);
    assert_eq!(r.text("link"), "HRIT 665.4 ksym/s");
}

#[test]
fn what_the_xrit_modes_publish_is_declared() {
    for (d, m, rate, secs, f, extra) in [
        (&GOES, XritMode::Goes, 2.4e6, 1.5, 1694.1e6, None),
        (&GK2A_LRIT, XritMode::Gk2aLrit, 1e6, 6.0, 1692.14e6, None),
        (
            &ELEKTRO,
            XritMode::Elektro,
            240e3,
            4.0,
            1693e6,
            Some(sdrkit_stack_satimg::modes::xrit::ElektroLink::Ggak),
        ),
    ] {
        let mut h = impaired(
            harness(d, rate, secs).set(&sdrkit_stack_satimg::modes::xrit::GEN_FILL, 12),
            Some(12.0),
            0.0,
            0.0,
        );
        if let Some(l) = extra {
            h = h
                .set(&sdrkit_stack_satimg::modes::xrit::LINK_SETTING, l)
                .set(&sdrkit_stack_satimg::modes::xrit::GEN_LINK, l);
        }
        let x = h.generate().unwrap();
        let r = run::<Picture>(&h, &x, rate, f, f, 60_000, None);
        assert_declared(&r, &declaration_for(m));
        assert!(!r.events.is_empty(), "{}", d.id);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The file layer.

/// The files the receiver's file layer makes of the transfer frames of `frames` (`skip`: a frame left out).
fn files_of(
    frames: &mut XritFrames,
    n: usize,
    skip: Option<usize>,
) -> (Vec<XritFile>, FilesComponent) {
    let mut v = VcduComponent::new(sdrkit_proto_ccsds::aos::Layout::RS_I4, true);
    let mut f = FilesComponent::new(sdrkit_proto_ccsds::xrit::headers::Dialect::Goes);
    let mut out = Vec::new();
    let (mut fr, mut pk) = (Vec::new(), Vec::new());
    for k in 0..n {
        let frame = frames.next_frame();
        if Some(k) == skip {
            continue;
        }
        v.push_frame(&frame, 0.0, &mut fr, &mut pk);
        for p in pk.drain(..) {
            f.push(&p, &mut out);
        }
    }
    (out, f)
}

#[test]
fn goes_transport_returns_the_five_files() {
    let files = goes_files();
    let mut frames = XritFrames::new(126, files.clone(), 0, true);
    let (got, f) = files_of(&mut frames, 200, None);
    assert_eq!(got.len(), files.len());
    for (g, w) in got.iter().zip(&files) {
        assert_eq!(&g.bytes[..], &w.bytes[..]);
        assert!(!g.errors);
    }
    assert_eq!(f.stats().length_errors, 0);
}

#[test]
fn a_lost_frame_marks_one_file() {
    let files = goes_files();
    let mut frames = XritFrames::new(126, files.clone(), 0, true);
    let (got, _) = files_of(&mut frames, 200, Some(1));
    assert_eq!(got.iter().filter(|f| f.errors).count(), 1);
}

/// A file whose packets hold fewer bytes than its transport header says is marked with errors and counted.
#[test]
fn a_file_shorter_than_its_transport_header_is_marked() {
    use sdrkit_proto_ccsds::packet::SeqFlags;
    use sdrkit_stack_satimg::items::SpacePacket;
    let file = &goes_files()[3].bytes;
    let mut pieces = sdrkit_proto_ccsds::xrit::transport::split_file(file, 9, 400, &[]);
    // The transport header says 16 bytes more than the packets carry.
    let bits = (file.len() as u64 + 16) * 8;
    pieces[0][2..10].copy_from_slice(&bits.to_be_bytes());
    let mut f = FilesComponent::new(sdrkit_proto_ccsds::xrit::headers::Dialect::Goes);
    let mut out = Vec::new();
    let n = pieces.len();
    for (k, data) in pieces.iter().enumerate() {
        let flags = match (k == 0, k + 1 == n) {
            (true, true) => SeqFlags::Unsegmented,
            (true, false) => SeqFlags::First,
            (false, true) => SeqFlags::Last,
            _ => SeqFlags::Continuation,
        };
        f.push(
            &SpacePacket {
                vcid: 30,
                spacecraft: 126,
                apid: 1054,
                flags: flags.bits(),
                seq: k as u16,
                data: data.as_slice().into(),
                crc_ok: Some(true),
                gap_before: false,
                time: 0.0,
            },
            &mut out,
        );
    }
    assert_eq!(out.len(), 1);
    assert!(out[0].errors);
    assert_eq!(f.stats().length_errors, 1);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Rice, the CRCs and the files a component writes.

fn rice_store(bits: u8) -> Vec<u8> {
    use sdrkit_stack_satimg::xrit::store::{Changes, Store, StoreConfig};
    let pic: Vec<u8> = goes_full_disk();
    let file = goes_rice_segment(FULL_DISK_NAME, &pic, 100, 0, 64, 1, 1, bits);
    let headers = sdrkit_proto_ccsds::xrit::headers::parse(
        &file.bytes,
        sdrkit_proto_ccsds::xrit::headers::Dialect::Goes,
    )
    .unwrap();
    let mut starts = vec![0u32];
    starts.extend(file.line_starts.iter().map(|&s| s as u32));
    let n = starts.len() as u32;
    let mut store = Store::new(StoreConfig::goes());
    let mut ch = Changes::default();
    store.push(
        XritFile {
            vcid: 13,
            apid: 781,
            bytes: file.bytes.as_slice().into(),
            packet_starts: starts,
            packet_index: (0..n).collect(),
            headers,
            errors: false,
            truncated: false,
            time: 0.0,
        },
        &mut ch,
    );
    store.products.back().unwrap().picture.clone().unwrap()
}

/// A GOES picture of 4-bit samples coded with the basic option set (CCSDS 121.0-B-3 clause 5.1.2) decodes exactly.
#[test]
fn rice_four_bit_samples_use_the_basic_option_set() {
    let want: Vec<u8> = goes_full_disk().iter().map(|&v| v >> 4).collect();
    assert_eq!(rice_store(4), want);
    assert_eq!(rice_store(8), goes_full_disk());
}

/// The samples of a run of zero blocks that reaches the end of a segment in the second sample interval of one coded line.
pub fn zero_run_line() -> (sdrkit_proto_ccsds::rice::Params, Vec<u32>, Vec<u8>) {
    let p = sdrkit_proto_ccsds::rice::Params::new(8, 8, 40).unwrap();
    // 120 blocks: noise in the first interval, then constant samples (zero blocks after the preprocessor).
    let line: Vec<u32> = (0..960u32)
        .map(|i| if i < 320 { (i * 37 + i / 3) % 256 } else { 77 })
        .collect();
    let mut coded = Vec::new();
    sdrkit_proto_ccsds::rice::encode(&line, &p, &mut coded).unwrap();
    (p, line, coded)
}

/// The segments of zero blocks count from the start of each sample interval (CCSDS 121.0-B-3 clause 5.1.4), so a line of three
/// intervals decodes exactly.
#[test]
fn rice_zero_run_counts_segments_from_the_interval_start() {
    let (p, line, coded) = zero_run_line();
    let mut out = vec![0u32; line.len()];
    let d = sdrkit_proto_ccsds::rice::decode(&coded, &p, &mut out).unwrap();
    assert_eq!(d.samples, line.len());
    assert_eq!(out, line);
}

/// The stacks use the catalogue CRCs by name, each with the check value of "123456789".
#[test]
fn the_stacks_use_catalogue_crcs_by_name() {
    use sdrkit_util_bits::crc::catalog::{entry, CRC16_CMS, CRC32_ISO_HDLC};
    for (name, check) in [
        ("CRC-16/IBM-3740", 0x29B1u64),
        ("CRC-32/ISO-HDLC", 0xCBF4_3926),
        ("CRC-16/CMS", 0xAEE7),
    ] {
        assert_eq!(
            entry(name).unwrap().engine.checksum(b"123456789"),
            check,
            "{name}"
        );
    }
    // GGAK frames carry CRC-16/CMS.
    let f = sdrkit_stack_satimg::instruments::ggak::build(1, 2, 3, &[[0; 3]; 13]);
    assert_eq!(
        u16::from_be_bytes([f[222], f[223]]),
        CRC16_CMS.checksum(&f[4..222])
    );
    // ZIP archives carry CRC-32/ISO-HDLC.
    let z = sdrkit_stack_satimg::xrit::zip::stored("A.TXT", b"123456789");
    assert_eq!(
        u32::from_le_bytes([z[14], z[15], z[16], z[17]]),
        CRC32_ISO_HDLC.checksum(b"123456789")
    );
    // A CP_PDU with a wrong CRC-16/IBM-3740 is counted.
    let mut frames = XritFrames::new(126, goes_files()[3..4].to_vec(), 0, true);
    let mut v = VcduComponent::new(sdrkit_proto_ccsds::aos::Layout::RS_I4, true);
    let (mut fr, mut pk) = (Vec::new(), Vec::new());
    let mut frame = frames.next_frame();
    frame[100] ^= 0x10;
    v.push_frame(&frame, 0.0, &mut fr, &mut pk);
    for _ in 0..8 {
        v.push_frame(&frames.next_frame(), 0.0, &mut fr, &mut pk);
    }
    assert_eq!(v.stats().crc_errors, 1);
}

/// No xRIT or FY-4 stack has a setting that names a folder or saves a file, and such a key is refused.
#[test]
fn no_stack_has_a_setting_that_writes_files() {
    let modes: [&'static StackDescriptor; 6] = [
        &GOES,
        &GK2A_LRIT,
        &GK2A_HRIT,
        &ELEKTRO,
        &sdrkit_stack_satimg::modes::fy4::LRIT,
        &sdrkit_stack_satimg::modes::fy4::HRIT,
    ];
    for d in modes {
        let s = (d.settings)();
        for k in [
            "data_dir",
            "results_file",
            "auto_save",
            "save_lrit",
            "save_cadu",
            "save",
        ] {
            assert!(s.id(k).is_err(), "{} has {k}", d.id);
            let mut rx = Harness::new(d).receiver().unwrap();
            assert!(set(&mut *rx, k, "/tmp").is_err(), "{} takes {k}", d.id);
        }
    }
}

#[test]
fn elektro_ggak_frames_5_khz_off_in_a_wide_capture() {
    for off in [5000.0, -5000.0] {
        let rate = 2.4e6;
        let h = impaired(
            harness(&ELEKTRO, rate, 4.0)
                .set(
                    &sdrkit_stack_satimg::modes::xrit::LINK_SETTING,
                    sdrkit_stack_satimg::modes::xrit::ElektroLink::Ggak,
                )
                .set(
                    &sdrkit_stack_satimg::modes::xrit::GEN_LINK,
                    sdrkit_stack_satimg::modes::xrit::ElektroLink::Ggak,
                ),
            Some(25.0),
            off,
            0.0,
        );
        let x = h.generate().unwrap();
        let r = run::<Picture>(&h, &x, rate, 1693e6, 1693e6, 240_000, None);
        assert!(
            r.num("ggak_good") >= 2.0,
            "offset {off}: {}",
            r.num("ggak_good")
        );
    }
}
