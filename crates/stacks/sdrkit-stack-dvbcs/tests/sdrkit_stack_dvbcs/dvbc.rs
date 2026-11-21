//! DVB-C: the recording, every order round trip, the cuts of the input, offsets, echoes, inversion and manual settings.

use crate::common::*;
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_ECHO_DB, GEN_ECHO_DELAY_S, GEN_ECHO_PHASE_DEG, GEN_ESN0_DB,
    GEN_FADING, GEN_INVERTED, GEN_NOISE, GEN_OFFSET_HZ,
};
use sdrkit_algo_sim::fading::profiles::ProfileName;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Receiver};
use sdrkit_proto_mpegts::monitor;
use sdrkit_stack_dvbcs::dvbc::qam::Qam;
use sdrkit_stack_dvbcs::dvbc::DVBC;
use sdrkit_stack_dvbcs::dvbc::{self, GEN_QAM};
use sdrkit_proto_dvbs2::transmit::{Payload, GEN_PAYLOAD};
use sdrkit_stack_dvbcs::typed::DvbcFields;

#[test]
fn recorded_vector_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("dvbcs/dvbc_64qam_5361k_7msps")
        .expect("the DVB-C recording");
    let mut rx = Rx::new(&DVBC, &[], &recording(rec.rate_hz, rec.centre_hz));
    rx.feed(&rec.samples, 50000);
    rx.dump();
    let c = rx.check();
    println!("{c:?}");
    assert!(c.good >= 600, "{}", c.good);
    assert_eq!((c.wrong, c.flagged, c.gaps), (0, 0, 0));
    assert!(c.first.unwrap() <= 3350);
    assert_eq!(rx.at(&DvbcFields::QAM), Some(Qam::Q64));
    assert_eq!(rx.at(&DvbcFields::INVERTED), Some(false));
    assert_eq!(rx.at(&DvbcFields::TS_LOCK), Some(true));
    let rs = rx.num("symbol_rate_hz");
    assert!((rs - 5.361e6).abs() < 5.361e6 * 30e-6, "{rs}");
    let cfo = rx.num("offset_hz");
    assert!((14700.0..=15300.0).contains(&cfo), "{cfo}");
    assert!(rx.num("mer_db") >= 28.0);
    let br = rx.num("net_bitrate_bps");
    assert!((br / 1e6 - 29.643).abs() < 0.01, "{br}");
}

fn gen(seconds: f64, settings: &[CaseSetting]) -> Vec<Complex32> {
    generate(&DVBC, 10e6, seconds, settings)
}

fn esn0(db: f64) -> [CaseSetting; 2] {
    [
        CaseSetting::new(&GEN_NOISE, NoiseKind::EsN0),
        CaseSetting::new(&GEN_ESN0_DB, db),
    ]
}

#[test]
fn every_order_round_trips_and_names_its_service() {
    for (qam, es) in [
        (Qam::Q16, 35.0),
        (Qam::Q32, 35.0),
        (Qam::Q64, 40.0),
        (Qam::Q128, 40.0),
        (Qam::Q256, 40.0),
    ] {
        let mut kv = esn0(es).to_vec();
        kv.push(CaseSetting::new(&GEN_QAM, qam));
        let x = gen(1.2, &kv);
        let mut rx = Rx::new(&DVBC, &[], &recording(10e6, 346e6));
        rx.feed(&x, 65536);
        rx.dump();
        let c = rx.check();
        assert!(
            c.good > 400 && c.wrong == 0 && c.flagged == 0 && c.gaps == 0,
            "{qam:?} {c:?}"
        );
        assert_eq!(rx.at(&DvbcFields::QAM), Some(qam));
        assert_eq!(rx.get(&monitor::PAT_SEEN), Some(true));
        assert_eq!(rx.get(&monitor::SERVICES), Some("sdrkit test"));
    }
}

/// The test signal as the test source delivers it: no noise, 8-bit samples with 12 dB of headroom. The receiver locks, knows
/// the order, measures a high MER, has no failed Reed-Solomon word and lists the service.
#[test]
fn the_clean_8_bit_test_signal_locks_with_a_high_mer() {
    for qam in [Qam::Q64, Qam::Q256] {
        let mut x = gen(1.5, &[CaseSetting::new(&GEN_QAM, qam)]);
        let step = 4.0 / 128.0;
        let q = |v: f32| (v / step).clamp(-128.0, 127.0).round() * step;
        for v in &mut x {
            *v = Complex32::new(q(v.re), q(v.im));
        }
        let mut rx = Rx::new(&DVBC, &[], &recording(10e6, 346e6));
        rx.feed(&x, 16384);
        rx.dump();
        assert_eq!(rx.at(&DvbcFields::TS_LOCK), Some(true), "{qam:?}");
        assert_eq!(rx.at(&DvbcFields::QAM), Some(qam));
        let mer = rx.num("mer_db");
        assert!(mer > 30.0, "{qam:?}: MER {mer}");
        assert_eq!(rx.at(&DvbcFields::RS_FAILED), Some(0), "{qam:?}");
        assert!(rx.num("rs_clean") > 2000.0, "{qam:?}");
        assert_eq!(rx.get(&monitor::SERVICES), Some("sdrkit test"), "{qam:?}");
        let offset = rx.num("offset_hz");
        assert!(offset.abs() < 100.0, "{qam:?}: offset {offset}");
    }
}

/// The same signal cut differently decodes to the same packets, and bad samples do no harm.
#[test]
fn cuts_do_not_change_the_output_and_bad_samples_are_harmless() {
    let mut kv = esn0(30.0).to_vec();
    kv.push(CaseSetting::new(&GEN_OFFSET_HZ, 20e3));
    kv.push(CaseSetting::new(&GEN_PAYLOAD, Payload::Numbered));
    let mut x = gen(0.5, &kv);
    for i in (1000..x.len()).step_by(250_000) {
        x[i] = Complex32::new(f32::NAN, f32::INFINITY);
    }
    let mut outs = Vec::new();
    for piece in [7usize, 4096, 65536, 0] {
        let mut rx = Rx::new(&DVBC, &[], &recording(10e6, 346e6));
        if piece == 0 {
            rx.feed_mixed(&x);
        } else {
            rx.feed(&x, piece);
        }
        outs.push(rx.packets);
    }
    println!("{:?}", outs.iter().map(|o| o.len()).collect::<Vec<_>>());
    assert!(outs[0].len() > 1000);
    for o in &outs[1..] {
        assert_eq!(o, &outs[0]);
    }
}

#[test]
fn offsets_echo_inversion_and_manual_settings() {
    type Case = (&'static str, Vec<CaseSetting>, Vec<CaseSetting>);
    let cases: Vec<Case> = vec![
        (
            "echo",
            vec![
                CaseSetting::new(&GEN_OFFSET_HZ, 10e3),
                CaseSetting::new(&GEN_CLOCK_PPM, 20.0),
                CaseSetting::new(&GEN_FADING, ProfileName::Echo),
                CaseSetting::new(&GEN_ECHO_DB, -10.0),
                CaseSetting::new(&GEN_ECHO_DELAY_S, 0.5e-6),
                CaseSetting::new(&GEN_ECHO_PHASE_DEG, 60.0),
            ],
            vec![],
        ),
        (
            "inverted",
            vec![
                CaseSetting::new(&GEN_OFFSET_HZ, 15e3),
                CaseSetting::new(&GEN_INVERTED, true),
            ],
            vec![],
        ),
        (
            "manual",
            vec![CaseSetting::new(&dvbc::GEN_SYMBOL_RATE, 6.9e6)],
            vec![
                CaseSetting::new(&dvbc::QAM_ORDER, dvbc::QamChoice::Q64),
                CaseSetting::new(&dvbc::SYMBOL_RATE, 6.9e6),
            ],
        ),
        (
            "off centre",
            vec![
                CaseSetting::new(&dvbc::GEN_SYMBOL_RATE, 6.111e6),
                CaseSetting::new(&GEN_OFFSET_HZ, 600e3),
            ],
            vec![],
        ),
    ];
    for (name, mut kv, settings) in cases {
        kv.extend(esn0(32.0));
        let x = gen(1.2, &kv);
        let mut rx = Rx::new(&DVBC, &settings, &recording(10e6, 346e6));
        rx.feed(&x, 65536);
        rx.dump();
        let c = rx.check();
        assert!(
            c.good > 400 && c.wrong == 0 && c.flagged == 0 && c.gaps == 0,
            "{name}: {c:?}"
        );
        assert_eq!(
            rx.at(&DvbcFields::INVERTED),
            Some(name == "inverted"),
            "{name}"
        );
    }
}

#[test]
fn cable_raster_and_rate_refusal() {
    use sdrkit_stack_dvbcs::dvbc::cable_raster;
    assert_eq!(cable_raster(475e6), 474e6);
    assert_eq!(cable_raster(100e6), 114e6);
    assert_eq!(cable_raster(900e6), 858e6);
    let mut rx = sdrkit_stack_dvbcs::dvbc::receiver(&config(&DVBC, &[])).unwrap();
    assert!(rx.configure(&recording(5e6, 346e6)).is_err());
    assert_eq!(
        sdrkit_core::Owner::results(&rx).status().summary,
        "too low for DVB-C (7 Msps or more)"
    );
}

#[test]
fn a_change_of_frequency_keeps_the_guide_and_forgets_the_multiplex() {
    use sdrkit_core::Receiver;
    let shape = sdrkit_stack_dvbcs::typed::shape("dvbc");
    assert_eq!(
        shape.kept_keys(),
        Ok(vec![
            "table guide".to_owned(),
            "table guide_ratings".to_owned()
        ])
    );
    let rec = sdrkit_dev_test::read_recording("dvbcs/dvbc_64qam_5361k_7msps")
        .expect("the DVB-C recording");
    let mut rx = Rx::new(&DVBC, &[], &recording(rec.rate_hz, rec.centre_hz));
    rx.feed(&rec.samples, 50000);
    assert!(rx.num("packets") > 0.0);
    assert!(rx.num("ts_demux.mux_kbps") > 0.0 || rx.num("bitrate_bps") > 0.0);
    let guide = rows(rx.results(), "guide");
    rx.rx.forget();
    // The counts and the PIDs belong to the multiplex of the frequency before.
    assert_eq!(rx.num("packets"), 0.0);
    assert_eq!(rx.num("ts_demux.null_packets"), 0.0);
    assert_eq!(rows(rx.results(), "ts_demux.pids"), 0);
    assert_eq!(rows(rx.results(), "services"), 0);
    assert_eq!(rows(rx.results(), "guide"), guide);
}
