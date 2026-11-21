//! Direct-broadcast and dump downlinks: the data files, the test signal of each profile through the whole receiver (the
//! link rows of the specification), noise alone, frame files of every CADU profile, and each instrument through a frame file.

use super::common::{impaired, run, Ran};
use sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S;
use sdrkit_core::media::Picture;
use sdrkit_core::{Complex32, Facilities, InputInfo, MemStorage, Output, Receiver, Storage, StorePath};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Graph;
use sdrkit_stack_satimg::instruments::avhrr_dump::{temperature, CalTable};
use sdrkit_stack_satimg::instruments::planes::test_pixel;
use sdrkit_stack_satimg::instruments::symbols::{kanopus_test_value, ols_vis};
use sdrkit_stack_satimg::instruments::{modis, small, viirs};
use sdrkit_stack_satimg::items::Cadu;
use sdrkit_stack_satimg::modes::eodb::gen::frame_file;
use sdrkit_stack_satimg::modes::eodb::profiles::{profile, PROFILES};
use sdrkit_stack_satimg::modes::eodb::{parse_satellites, EodbComponent, DESCRIPTOR, GEN_AMBIGUITY};
use std::sync::Arc;

const SATELLITES: &str = include_str!("../../../../../data/links/xband_downlinks.csv");
const CALIBRATION: &str = include_str!("../../../../../data/satimg/avhrr_calibration.csv");

#[test]
fn the_data_files_read() {
    let s = parse_satellites(SATELLITES).expect("the satellite list");
    assert_eq!(s.len(), 30);
    let n20 = s.iter().find(|s| s.spacecraft == Some(159)).unwrap();
    assert_eq!((n20.submode.as_str(), n20.name.as_str()), ("jpss_hrd", "NOAA-20"));
    // Every profile has a downlink in the list, and the stack holds no frequency of its own.
    assert_eq!(DESCRIPTOR.tuning.default_hz, 0.0);
    for p in PROFILES.iter() {
        let row = s.iter().find(|s| s.submode == p.id).unwrap_or_else(|| panic!("{}", p.id));
        assert!(row.centre_mhz > 0.0, "{}", p.id);
        let sub = DESCRIPTOR.submodes.iter().find(|m| m.id == p.id).unwrap();
        assert_eq!(sub.default_hz, 0.0, "{}", p.id);
    }
    let c = CalTable::parse(CALIBRATION).expect("the calibration file");
    assert_eq!(c.sats.len(), 2);
    assert_eq!(c.sats["MetOp-C"].prt.len(), 4);
}

/// The test signal of `id` at `rate` for `seconds` with the impairments, through the receiver.
#[allow(clippy::too_many_arguments)]
fn link(id: &str, rate: f64, seconds: f64, esn0: f64, offset: f64, drift: f64, ppm: f64, amb: i64) -> Ran {
    let h = impaired(
        Harness::new(&DESCRIPTOR).submode(id).rate(rate).seconds(seconds),
        Some(esn0),
        offset,
        ppm,
    )
    .set(&GEN_DRIFT_HZ_S, drift)
    .set(&GEN_AMBIGUITY, amb);
    let mut x = h.generate().expect("a signal");
    // Silence after a short signal, so that the receiver reports at least once (every 0.1 s of input).
    let least = (0.15 * rate) as usize;
    if x.len() < least {
        x.resize(least, Complex32::new(0.0, 0.0));
    }
    run::<Cadu>(&h, &x, rate, 0.0, 0.0, 1 << 18, None)
}

fn frames(r: &Ran) -> f64 {
    r.num("frames_ok")
}

macro_rules! link_test {
    ($name:ident, $id:expr, $rate:expr, $s:expr, $esn0:expr, $off:expr, $drift:expr, $ppm:expr, $amb:expr, $min:expr) => {
        #[test]
        fn $name() {
            let r = link($id, $rate, $s, $esn0, $off, $drift, $ppm, $amb);
            assert!(frames(&r) >= $min, "{} frames: {}", frames(&r), r.text("hypothesis"));
        }
    };
}

link_test!(saral_l_clean, "saral_l", 3e6, 0.25, 20.0, 12_000.0, 0.0, 0.0, 0, 5.0);
link_test!(saral_l_drift_and_clock, "saral_l", 3e6, 0.4, 8.0, -8000.0, 500.0, 20.0, 5, 5.0);
link_test!(saral_l_5_db, "saral_l", 3e6, 0.4, 5.0, 6000.0, 200.0, 15.0, 0, 8.0);
link_test!(saral_l_10_db, "saral_l", 3e6, 0.4, 10.0, 6000.0, 200.0, 15.0, 0, 8.0);
link_test!(oceansat3_l, "oceansat3_l", 3e6, 0.2, 12.0, 20_000.0, 0.0, 0.0, 1, 3.0);
link_test!(proba_s, "proba_s", 6e6, 0.2, 12.0, 15_000.0, 0.0, 0.0, 0, 3.0);
link_test!(landsat_s, "landsat_s", 4e6, 0.5, 15.0, 5000.0, 0.0, 0.0, 2, 5.0);
link_test!(jpss_hrd, "jpss_hrd", 25e6, 0.3, 20.0, 50_000.0, 0.0, 0.0, 3, 5.0);
link_test!(jpss_hrd_n21, "jpss_hrd_n21", 40e6, 0.2, 20.0, -30_000.0, 0.0, 0.0, 0, 5.0);
link_test!(eos_aqua_aura, "eos_aqua_aura", 15e6, 0.1, 25.0, 10_000.0, 0.0, 0.0, 0, 5.0);
link_test!(eos_terra, "eos_terra", 45e6, 0.12, 20.0, 40_000.0, 0.0, 0.0, 0, 5.0);
link_test!(metop_dump_25_db, "metop_dump", 45e6, 0.02, 25.0, 10_000.0, 0.0, 0.0, 2, 5.0);
link_test!(metop_dump_12_db, "metop_dump", 45e6, 0.03, 12.0, 2000.0, 0.0, 0.0, 0, 10.0);
link_test!(proba_v_x, "proba_v_x", 45e6, 0.05, 20.0, 0.0, 0.0, 0.0, 0, 3.0);
link_test!(gcom_w1, "gcom_w1", 20e6, 0.1, 20.0, 5000.0, 0.0, 0.0, 0, 3.0);
link_test!(saral_x, "saral_x", 25e6, 0.1, 20.0, 5000.0, 0.0, 0.0, 1, 3.0);
link_test!(oceansat3_x, "oceansat3_x", 120e6, 0.02, 25.0, 0.0, 0.0, 0.0, 0, 3.0);
link_test!(gcom_c1, "gcom_c1", 90e6, 0.03, 20.0, 5000.0, 0.0, 0.0, 1, 3.0);
link_test!(metopsg_ddb, "metopsg_ddb", 140e6, 0.02, 20.0, 0.0, 0.0, 0.0, 1, 3.0);

#[test]
fn saral_l_every_ambiguity() {
    for amb in 0..8 {
        let r = link("saral_l", 3e6, 0.25, 12.0, 3000.0, 0.0, 0.0, amb);
        assert!(frames(&r) >= 5.0, "ambiguity {amb}: {} frames", frames(&r));
    }
}

/// The component `eodb` of a receiver.
fn component(rx: &mut dyn Receiver) -> &EodbComponent {
    rx.as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .component_as::<EodbComponent>("eodb")
        .expect("the eodb component")
}

#[test]
fn oceansat2_lines_are_the_test_picture() {
    let r = link("oceansat2", 35e6, 0.19, 20.0, 8000.0, 0.0, 0.0, 0);
    let mut rx = r.rx;
    let c = component(&mut *rx);
    let p = &c.planes().unwrap().planes[2];
    assert!(p.rows >= 3, "{} lines", p.rows);
    // The first frame may be lost while the synchroniser confirms its lock: row r is line r + d of the test picture.
    let ok = (0..4).any(|d| {
        (0..p.rows).all(|row| (0..p.width).step_by(7).all(|x| p.get(x, row) == test_pixel(2, x, row + d) * 16))
    });
    assert!(ok, "OCM channel 3 differs from the test picture");
}

#[test]
fn kanopus_v_gives_pmss_frames_and_lines() {
    let r = link("kanopus_v", 90e6, 0.3, 22.0, 5000.0, 0.0, 0.0, 0);
    assert!(r.num("instrument_packets") > 50.0, "{} PMSS frames", r.num("instrument_packets"));
    assert!(r.num("lines") > 400.0, "{} lines", r.num("lines"));
    let mut rx = r.rx;
    let c = component(&mut *rx);
    let p = &c.planes().unwrap().planes[0];
    let rows = p.rows.min(463);
    let same = (0..rows)
        .flat_map(|y| (0..p.width).step_by(5).map(move |x| (x, y)))
        .filter(|&(x, y)| p.get(x, y) == u16::from(kanopus_test_value(0, x, y)) * 256)
        .count();
    assert!(same * 100 > 99 * rows * p.width.div_ceil(5), "{same} pixels equal");
}

#[test]
fn dmsp_rtd_lines_are_the_test_picture() {
    let r = link("dmsp_rtd", 2e6, 2.2, 16.0, 3000.0, 0.0, 0.0, 0);
    let mut rx = r.rx;
    let c = component(&mut *rx);
    let p = &c.planes().unwrap().planes[0];
    assert!(p.rows >= 20, "{} lines", p.rows);
    let good_rows = |d: usize| {
        (0..p.rows)
            .filter(|&row| (16..7330).step_by(3).all(|x| p.get(x, row) == u16::from(ols_vis(x, row + d)) * 256))
            .count()
    };
    let best = (0..3).map(good_rows).max().unwrap_or(0);
    assert!(best >= 20, "{best} lines equal to the test picture");
}

#[test]
fn noise_alone_gives_no_frame_and_is_mostly_let_go_by() {
    for p in PROFILES.iter() {
        let mut rx = Harness::new(&DESCRIPTOR).submode(p.id).receiver().expect("a receiver");
        rx.configure(&InputInfo::new(p.rate, 0.0, 0.0).recording()).expect("the rate");
        let dec = ((p.rate / (4.0 * p.symbol_rate)).floor() as usize).max(1);
        let n = (20 * 131_072 * dec).max((0.12 * p.rate) as usize);
        let mut g = sdrkit_algo_sim::noise::Awgn::with_power(0.05, 3).expect("a power");
        let mut out = Output::default();
        let mut x = vec![Complex32::new(0.0, 0.0); 1 << 18];
        let mut left = n;
        while left > 0 {
            let k = left.min(x.len());
            x[..k].fill(Complex32::new(0.0, 0.0));
            g.process(&mut x[..k]);
            rx.feed(&x[..k], &mut out);
            out.clear();
            left -= k;
        }
        let r = sdrkit_core::Owner::results(&*rx);
        let num = |k: &str| match r.decl().field_id(k).and_then(|id| r.fields().value(id)) {
            Some(sdrkit_core::Value::I64(v)) => *v as f64,
            Some(sdrkit_core::Value::F64(v)) => *v,
            _ => f64::NAN,
        };
        assert_eq!(num("frames_ok"), 0.0, "{}", p.id);
        assert!(
            num("gate_skipped_pct") > 60.0,
            "{}: {} % let go by, last line {}, lock metric {}",
            p.id,
            num("gate_skipped_pct"),
            num("gate_line"),
            num("lock_metric")
        );
    }
}

/// A receiver of `id` whose storage holds `file` as `frames.cadu` and the data files, after the frame file was decoded.
fn imported(id: &str, file: Vec<u8>) -> Box<dyn Receiver> {
    let p = profile(Some(id));
    let st = MemStorage::new();
    st.write(&StorePath::new("frames.cadu").unwrap(), &file).unwrap();
    st.write(&StorePath::new("links/xband_downlinks.csv").unwrap(), SATELLITES.as_bytes()).unwrap();
    st.write(&StorePath::new("satimg/avhrr_calibration.csv").unwrap(), CALIBRATION.as_bytes()).unwrap();
    let mut rx = Harness::new(&DESCRIPTOR).submode(id).receiver().expect("a receiver");
    rx.set_facilities(&Facilities {
        storage: Arc::new(st),
        ..Facilities::default()
    });
    rx.configure(&InputInfo::new(p.rate, 0.0, 0.0).recording()).expect("the rate");
    super::common::invoke(&mut *rx, "open_frames", &[("path", "frames.cadu")]).expect("the command");
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 4096], &mut out);
    rx
}

#[test]
fn thirty_test_cadus_of_every_cadu_profile_are_good() {
    for p in PROFILES.iter().filter(|p| p.has_cadus()) {
        let mut f = frame_file(p, 30);
        if p.rs.is_some() {
            // Errors within the power of the code: a few bytes of each frame.
            for k in 0..30 {
                for at in [10, 300, 700] {
                    f[k * p.frame + at] ^= 0x5A;
                }
            }
        }
        let mut c = EodbComponent::new(p);
        c.import_frames(&f);
        assert_eq!(c.counts(), (30, 0), "{}", p.id);
    }
}

/// The share of compared pixels of `ch` that equal `want`, over at most `max_rows` rows.
fn equal_share_rows(
    c: &EodbComponent,
    ch: usize,
    step: usize,
    max_rows: usize,
    want: impl Fn(usize, usize) -> u16,
) -> (f64, usize) {
    let p = &c.planes().unwrap().planes[ch];
    let mut n = 0;
    let mut same = 0;
    for y in 0..p.rows.min(max_rows) {
        for x in (0..p.width).step_by(step) {
            n += 1;
            same += usize::from(p.get(x, y) == want(x, y));
        }
    }
    (same as f64 / n.max(1) as f64, p.rows)
}

/// [`equal_share_rows`] over every row.
fn equal_share(c: &EodbComponent, ch: usize, step: usize, want: impl Fn(usize, usize) -> u16) -> (f64, usize) {
    equal_share_rows(c, ch, step, usize::MAX, want)
}

#[test]
fn viirs_through_a_frame_file() {
    let p = profile(Some("jpss_hrd"));
    let mut rx = imported(p.id, frame_file(p, 40_000));
    let c = component(&mut *rx);
    for name in ["M4", "M1", "M5", "M10", "M8", "M14", "I1", "I2", "I5"] {
        let bi = viirs::index(name).unwrap();
        let scale = viirs::BANDS[bi].scale;
        let (share, rows) = equal_share(c, bi, 11, |x, y| {
            (u32::from(viirs::test_value(bi, x, y)) * scale).min(65_535) as u16
        });
        assert!(rows > 0 && share > 0.99, "{name}: {share} of {rows} rows");
    }
    assert_eq!(c.satellite_name(), "NOAA-20");
    let n = ["natural", "true", "dust", "fog"].iter().filter(|k| c.product(k, 400).is_some()).count();
    assert!(n >= 2, "{n} products");
}

#[test]
fn modis_through_a_frame_file() {
    let p = profile(Some("eos_aqua_aura"));
    let mut rx = imported(p.id, frame_file(p, 7000));
    let c = component(&mut *rx);
    for name in ["1", "2", "3", "7", "8", "29", "36"] {
        let pl = modis::plane(name).unwrap();
        // The first scan: ten 1-km rows.
        let first = 10 * c.planes().unwrap().planes[pl].width / modis::POSITIONS;
        let (share, rows) = equal_share_rows(c, pl, 3, first, |x, y| modis::test_value(pl, x, y) * 16);
        assert!(rows > 0 && share > 0.99, "band {name}: {share} of {rows} rows");
    }
}

#[test]
fn avhrr_through_a_frame_file() {
    let p = profile(Some("metop_dump"));
    let mut rx = imported(p.id, frame_file(p, 400));
    let c = component(&mut *rx);
    for ch in 0..5 {
        let (share, rows) = equal_share(c, ch, 1, |x, y| test_pixel(ch, x, y) / 4 * 64);
        assert!(rows > 10 && share > 0.99, "channel {}: {share} of {rows} rows", ch + 1);
    }
    assert_eq!(c.satellite_name(), "MetOp-B");
    let n = ["natural", "cloud_top", "sst", "fire"].iter().filter(|k| c.product(k, 0).is_some()).count();
    assert!(n >= 3, "{n} products");
    let cal = CalTable::parse(CALIBRATION).unwrap();
    let line = c.avhrr_line(0).expect("the calibration views of line 0");
    let t = temperature(&cal.sats["MetOp-B"], &line, 1, 700.0).unwrap();
    assert!((t - 251.739).abs() < 2.0, "{t}");
    // The command through the graph: the product leaves on the products output.
    super::common::invoke(&mut *rx, "product", &[("product", "cloud_top")]).expect("the command");
    let r = super::common::feed::<Picture>(rx, &[Complex32::new(0.0, 0.0); 4096], 4096, Some("eodb.products"));
    let pics = r.of::<Picture>();
    assert_eq!(pics.len(), 1);
    assert!(pics[0].data.len() >= 3 * 2048 * 10);
}

#[test]
fn a_proba_frame_without_the_randomiser_is_tried_again() {
    let p = profile(Some("proba_s"));
    let mut f = frame_file(p, 30);
    for c in f.chunks_mut(p.frame) {
        sdrkit_proto_ccsds::randomiser::tm_bytes(&mut c[4..]);
    }
    let mut c = EodbComponent::new(p);
    c.import_frames(&f);
    assert_eq!(c.counts(), (30, 0));
}

#[test]
fn the_smaller_instruments_through_frame_files() {
    let cases: [(&str, usize, &[usize]); 4] = [("gcom_w1", 300, &[0, 7, 19]), ("landsat_x", 300, &[0]), ("proba_s", 300, &[0]), ("metopsg_ddb", 12_000, &[0, 7, 13])];
    for (id, n, chans) in cases {
        let p = profile(Some(id));
        let mut rx = imported(id, frame_file(p, n));
        let c = component(&mut *rx);
        for &ch in chans {
            let (share, rows) = match id {
                "metopsg_ddb" => equal_share_rows(c, ch, 7, 24, |x, y| small::metimage_test_value(ch + 1, x, y)),
                _ => equal_share(c, ch, 1, |x, y| test_pixel(ch, x, y) * 16),
            };
            assert!(rows > 0 && share > 0.99, "{id} channel {}: {share} of {rows} rows", ch + 1);
            if id == "metopsg_ddb" {
                assert!(rows >= 24, "{rows} lines");
            }
        }
    }
}

/// Live samples run the frame table on a helper thread. Over 1.6 s the reports go on, their count of good frames follows
/// the signal, and the carrier is found once: a display that saw no report would search it again every second.
#[test]
fn live_samples_keep_the_reports_and_the_carrier() {
    use sdrkit_graph::{Buffer, Data, Kind, Timed};
    use sdrkit_stack_satimg::components::gated::GateReport;
    use sdrkit_stack_satimg::components::table::TableReport;
    let rate = 3e6;
    let h = impaired(
        Harness::new(&DESCRIPTOR).submode("saral_l").rate(rate).seconds(1.6),
        Some(12.0),
        3000.0,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let recorded = frames(&run::<Cadu>(&h, &x, rate, 0.0, 0.0, 1 << 18, None));
    assert!(recorded >= 40.0, "{recorded} frames");
    let mut rx = h.receiver().expect("a receiver");
    rx.configure(&InputInfo::new(rate, 0.0, 0.0)).expect("the rate");
    let graph = rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>()).expect("a graph");
    let mut reports = graph.tap("frames.report", 1 << 12).expect("the report port");
    let mut gates = graph.tap("demod.gate", 1 << 12).expect("the gate port");
    let mut buf = Buffer::new(&Kind::typed::<TableReport>());
    let mut gate_buf = Buffer::new(&Kind::typed::<GateReport>());
    let mut out = Output::default();
    let (mut count, mut good, mut opens) = (0, 0, 0);
    for p in x.chunks((rate * 0.01) as usize) {
        out.clear();
        rx.feed(p, &mut out);
        while reports.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                for r in v.as_any().downcast_ref::<Vec<Timed<TableReport>>>().into_iter().flatten() {
                    count += 1;
                    assert!(r.value.stats.good >= good, "the count fell from {good} to {}", r.value.stats.good);
                    good = r.value.stats.good;
                }
            }
        }
        while gates.pop(&mut gate_buf) {
            if let Data::Items(Some(v)) = &gate_buf.data {
                for g in v.as_any().downcast_ref::<Vec<Timed<GateReport>>>().into_iter().flatten() {
                    opens = g.value.opens;
                }
            }
        }
    }
    assert!(count >= 12, "{count} reports");
    assert!(good as f64 >= 0.8 * recorded, "{good} good frames in the reports for {recorded}");
    assert_eq!(opens, 1);
}
