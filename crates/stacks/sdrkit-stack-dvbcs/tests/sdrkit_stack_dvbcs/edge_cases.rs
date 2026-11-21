//! Edge cases of the stacks of this crate: the decoders, the shared codes and the shared transport stream layer.

use crate::common::*;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Complex32;
use sdrkit_core::{Receiver, StackConfig};
use sdrkit_graph::{GraphError, StackGraph};
use sdrkit_proto_dvbs2::transmit::{Payload, GEN_PAYLOAD};
use sdrkit_proto_dvbs2::typed::Dvbs2Fields;
use sdrkit_proto_dvbs2::CodeRate;
use sdrkit_stack_dvbcs::dvbc::demod::cma_radius;
use sdrkit_stack_dvbcs::dvbc::qam::Qam;
use sdrkit_stack_dvbcs::dvbs::DVBS;
use sdrkit_stack_dvbcs::isdbs::rx::FrameTrellis;
use sdrkit_stack_dvbcs::{dvbc, dvbs, isdbs, DVBC, ISDBS};

/// The receiver function of each TV stack, with its test rate.
const TV: [(&sdrkit_core::StackDescriptor, Build, f64); 3] = [
    (&DVBC, dvbc::receiver, 10e6),
    (&DVBS, dvbs::receiver, 4e6),
    (&ISDBS, isdbs::receiver, 57.72e6),
];

type Build = fn(&StackConfig) -> Result<StackGraph, GraphError>;

/// The DVB-S2 frames of a carrier are decoded with the LDPC and BCH codes of EN 302 307-1, not only counted.
#[test]
fn dvbs2_frames_are_decoded_not_only_counted() {
    let x = generate(
        &DVBS,
        4e6,
        0.8,
        &[
            CaseSetting::new(&sdrkit_proto_dvbs2::settings::GEN_RATE, CodeRate::R35),
            CaseSetting::new(&sdrkit_proto_dvbs2::settings::GEN_SYMBOL_RATE, 1e6),
            CaseSetting::new(&GEN_PAYLOAD, Payload::Numbered),
        ],
    );
    let mut rx = Rx::new(&DVBS, &[], &recording(4e6, 1.5e9));
    rx.feed(&x, 65536);
    let c = rx.check();
    assert!(c.good > 100 && c.wrong == 0 && c.flagged == 0, "{c:?}");
    assert_eq!(rx.at(&Dvbs2Fields::UNSUPPORTED_FRAMES), Some(0));
    assert!(rx.num("bch_ok") > 0.0);
}

/// Soft values that are not finite reach the ISDB-S trellis without a panic.
#[test]
fn the_isdbs_trellis_takes_nan_without_a_panic() {
    let mut t = FrameTrellis::default();
    for i in 0..200 {
        let v = if i % 7 == 0 { f32::NAN } else { 1.0 };
        t.pair(v, f32::INFINITY, -1);
        t.table([f32::NAN, 0.0, f32::NEG_INFINITY, 1.0]);
    }
    let (mut bits, mut states) = (Vec::new(), Vec::new());
    t.decode(None, &mut bits, &mut states);
    assert_eq!(bits.len(), 400);
    t.decode(Some(5), &mut bits, &mut states);
    assert_eq!(states[0], 5);
}

/// The constant modulus radius of the blind equaliser follows the QAM order (E|a|^4 / E|a|^2 of its points).
#[test]
fn the_cma_radius_follows_the_qam_order() {
    let want = |m: usize| {
        let s = sdrkit_algo_dsp::constellation::PointSet::dvb_c(m).unwrap();
        let (m2, m4) = s.points().iter().fold((0.0f64, 0.0f64), |a, p| {
            let e = p.norm_sqr() as f64;
            (a.0 + e, a.1 + e * e)
        });
        (m4 / m2) as f32
    };
    for (q, m) in [
        (Qam::Q16, 16),
        (Qam::Q32, 32),
        (Qam::Q64, 64),
        (Qam::Q128, 128),
        (Qam::Q256, 256),
    ] {
        assert!((cma_radius(q) - want(m)).abs() < 1e-5, "{q:?}");
    }
    assert!((cma_radius(Qam::Q16) - 1.32).abs() < 1e-3);
    assert!((cma_radius(Qam::Q256) - 1.3953).abs() < 1e-3);
}

/// The DVB-C, DVB-S and ISDB-S graphs end in the demultiplexer of `sdrkit-proto-mpegts`, which names the service of the test
/// multiplex.
#[test]
fn satellite_and_cable_graphs_end_in_the_demultiplexer() {
    for (desc, build, rate) in TV {
        let mut rx = build(&config(desc, &[])).unwrap();
        rx.configure(&recording(rate, 1e9)).unwrap();
        let g = rx.graph_mut();
        for port in [
            "ts_demux.sections",
            "psi_si.services",
            "ts_demux.video",
            "teletext.pages",
        ] {
            assert!(g.format(port).is_some(), "{} has {port}", desc.id);
        }
    }
    let x = generate(&DVBC, 10e6, 0.6, &[]);
    let mut rx = Rx::new(&DVBC, &[], &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    assert!(
        rx.events
            .iter()
            .any(|e| e.is(&sdrkit_proto_mpegts::publish::SERVICE_LIST_EVENT)),
        "the service list comes out of the graph"
    );
}

/// Every event that a generator's signal makes is of a declared kind, and none reports a frequency of 0.
#[test]
fn every_event_is_declared() {
    for (desc, rate, secs) in [(&DVBC, 10e6, 0.6), (&DVBS, 4e6, 0.8)] {
        let x = generate(desc, rate, secs, &[]);
        let mut rx = Rx::new(desc, &[], &recording(rate, 1e9));
        rx.feed(&x, 65536);
        let decl = declaration(desc.id);
        assert!(!rx.events.is_empty(), "{}", desc.id);
        for e in &rx.events {
            assert!(
                decl.events().iter().any(|d| d.kind == e.kind()),
                "{}: {}",
                desc.id,
                e.kind()
            );
            assert_ne!(e.freq_hz, 0.0, "{}: {}", desc.id, e.kind());
        }
    }
}

/// The outer code is the Reed-Solomon codec of `sdrkit-algo-fec`: a root spacing that shares a factor with 255 is refused at
/// once, and RS(204,188) corrects 8 byte errors and no more.
#[test]
fn the_outer_code_refuses_a_bad_root_spacing_and_corrects_eight_errors() {
    use sdrkit_algo_fec::rs::{codes::DVB_204_188, Rs, RsCode};
    for prim in [3u16, 5, 15, 17, 85] {
        assert!(
            Rs::new(RsCode {
                prim,
                ..DVB_204_188
            })
            .is_err(),
            "prim {prim}"
        );
    }
    let mut rs = sdrkit_proto_mpegts::outer::rs();
    let data: Vec<u8> = (0..188).map(|i| (i * 7 + 3) as u8).collect();
    let mut word = data.clone();
    word.resize(204, 0);
    let (d, p) = word.split_at_mut(188);
    rs.encode(d, p).unwrap();
    let mut bad = word.clone();
    for k in 0..8 {
        bad[k * 25] ^= 0x5A;
    }
    assert!(rs.decode(&mut bad, &[]).is_ok());
    assert_eq!(bad, word);
    let mut worse = word.clone();
    for k in 0..9 {
        worse[k * 22] ^= 0xA5;
    }
    assert!(rs.decode(&mut worse, &[]).is_err() || worse != word);
}

/// The stacks reach the named codes of the shared crates, each with its published check value: CRC-8/DVB-S2 0xBC and
/// CRC-32/MPEG-2 0x0376E6E7 over "123456789", the BBHEADER CRC over its first nine bytes (EN 302 307-1 clause 5.1.6), and the
/// ISDB-S PN9 with period 511.
#[test]
fn named_codes_have_their_known_answers() {
    use sdrkit_proto_ipcast::bbheader::{BbHeader, BBHEADER_LEN};
    use sdrkit_util_bits::crc::catalog;
    let crc8 = catalog::by_key("crc8_dvb_s2").unwrap().engine;
    assert_eq!(crc8.checksum(b"123456789"), 0xBC);
    let crc32 = catalog::by_key("crc32_mpeg_2").unwrap().engine;
    assert_eq!(crc32.checksum(b"123456789"), 0x0376_E6E7);
    let mut h = BbHeader::transport(38688 - 80, 0);
    h.issyi = true;
    h.npd = true;
    let mut b = [0u8; BBHEADER_LEN];
    h.write(&mut b);
    assert_eq!(b[9] as u64, crc8.checksum(&b[..9]));
    assert_eq!(BbHeader::parse(&b).unwrap(), h);
    assert_eq!(
        sdrkit_util_bits::seq::PRBS9_DISPERSAL.period(1000),
        Some(511)
    );
}

/// The beacon's carrier loop is the tone loop of `sdrkit-algo-modem` and its mixers use no library sine or cosine per sample: the
/// carrier stage runs a second of signal with no allocation after its first piece.
#[test]
fn the_beacon_carrier_loop_allocates_nothing_per_piece() {
    use sdrkit_stack_dvbcs::beacon::dsp::CarrierPll;
    let fs = 250e3;
    let x: Vec<Complex32> = (0..250_000)
        .map(|n| {
            let ph = std::f64::consts::TAU * 1000.0 * n as f64 / fs
                + 0.3 * (std::f64::consts::TAU * 48e3 * n as f64 / fs).sin();
            Complex32::new(ph.cos() as f32, ph.sin() as f32)
        })
        .collect();
    let mut pll = CarrierPll::new(fs, 1000.0, 15.0);
    let mut pm = Vec::with_capacity(x.len());
    pll.process(&x[..4096], &mut pm);
    let before = crate::alloc::count();
    for c in x[4096..].chunks(4096) {
        pll.process(c, &mut pm);
    }
    assert_eq!(crate::alloc::count(), before);
    assert!((pll.freq_hz() - 1000.0).abs() < 1.0, "{}", pll.freq_hz());
    assert!(pll.lock > 0.3);
}

/// The transport stream monitor is the shared `mpegts.ts_monitor` in each TV graph, with its declared counters.
#[test]
fn the_ts_monitor_is_the_shared_component() {
    let monitor = sdrkit_proto_mpegts::monitor::results_decl();
    for (desc, build, rate) in TV {
        let mut rx = build(&config(desc, &[])).unwrap();
        rx.configure(&recording(rate, 1e9)).unwrap();
        assert!(
            rx.graph_mut().format("monitor.out").is_some(),
            "{}",
            desc.id
        );
        let decl = declaration(desc.id);
        for f in monitor.fields().fields() {
            assert!(
                decl.fields().fields().iter().any(|d| d.key == f.key),
                "{}: {}",
                desc.id,
                f.key
            );
        }
    }
}

fn declaration(id: &str) -> sdrkit_core::ResultsDecl {
    match id {
        "dvbc" => sdrkit_stack_dvbcs::dvbc::declaration(),
        "dvbs" => sdrkit_stack_dvbcs::dvbs::declaration(),
        "isdbs" => sdrkit_stack_dvbcs::isdbs::declaration(),
        _ => sdrkit_stack_dvbcs::typed::shape(id).results_decl(),
    }
}
