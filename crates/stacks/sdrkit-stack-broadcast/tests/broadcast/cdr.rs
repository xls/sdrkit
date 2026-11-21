//! CDR: the tables loaded at run time, the refusal without them, and the round trip with the self-check tables.

use super::support::*;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Results, SettingSet, StackConfig};
use sdrkit_stack_broadcast::cdr::mux::{
    sample_rate, AudioStream, ControlFrame, Nit, ServiceFrame, SmctEntry, SubFrame,
};
use sdrkit_stack_broadcast::cdr::phy::{self, SpectrumMode, SystemInfo, TxMode};
use sdrkit_stack_broadcast::cdr::results::{self as res, SERVICES};
use sdrkit_stack_broadcast::cdr::tables::{CdrTables, LdpcRate};
use sdrkit_stack_broadcast::cdr::{self, CdrGenerator, GenOptions, TxConfig};
use sdrkit_stack_broadcast::qam::Qam;
use serde_json::json;
use std::sync::Arc;

const DIAL: f64 = 106.1e6;
const RATE: f64 = 816e3;

/// The self-check tables: a fixture for the loader and the decoder, not the GY/T 268.1 tables.
fn fixture_path() -> String {
    format!(
        "{}/../../../tests/vectors/broadcast/cdr/self-check-tables.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn fixture() -> Arc<CdrTables> {
    Arc::new(CdrTables::load(&fixture_path()).expect("the self-check tables load"))
}

fn generate(tables: Option<Arc<CdrTables>>, seconds: f64, settings: &[Set]) -> Vec<Complex32> {
    let o = GenOptions {
        tx: TxConfig::default(),
        fm_programme: false,
        tables,
    };
    let g = CdrGenerator::new(o, RATE).expect("a generator at 816 ksps");
    impair(Box::new(g), seconds, settings)
}

fn config(tables: Option<&str>) -> StackConfig {
    let settings: Vec<Set> = tables
        .map(|p| ("cdr_tables", json!(p)))
        .into_iter()
        .collect();
    stack_config(cdr::CdrSetting::schema(), &settings)
}

fn receive(x: &[Complex32], cfg: &StackConfig) -> Results {
    let mut rx = cdr::make_receiver(cfg).unwrap();
    rx.configure(&InputInfo::new(RATE, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(100_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.results().clone()
}

#[test]
fn without_tables_nothing_is_decoded_and_the_receiver_says_why() {
    let x = generate(None, 2.0, &[]);
    let r = receive(&x, &config(None));
    let t = text(&r, res::CdrFields::TABLES.key);
    assert!(t.starts_with("not loaded"), "{t}");
    assert!(
        r.status().summary.contains("tables"),
        "{}",
        r.status().summary
    );
    assert_ne!(r.status().lock_state, LockState::Locked);
    assert_eq!(int(&r, res::CdrFields::SI_OK.key), 0);
    assert_eq!(int(&r, res::CdrFields::CW_OK.key), 0);
    assert_eq!(int(&r, res::CdrFields::CONTROL_OK.key), 0);
    assert!(row_ids(&r, SERVICES.key).is_empty());
    // The beacon is still found: the transmission mode is known.
    assert!(int(&r, res::CdrFields::SUBFRAMES.key) > 0);
    assert_eq!(int(&r, res::CdrFields::TX_MODE.key), 1);
}

#[test]
fn without_tables_the_generator_sends_the_beacon_and_pilots_only() {
    let o = GenOptions {
        tx: TxConfig::default(),
        fm_programme: false,
        tables: None,
    };
    let g = CdrGenerator::new(o, RATE).unwrap();
    assert!(g.note().starts_with("not loaded"), "{}", g.note());
}

#[test]
fn a_tables_file_that_does_not_load_is_named() {
    let r = receive(
        &generate(None, 0.5, &[]),
        &config(Some("/no/such/cdr-tables.json")),
    );
    let t = text(&r, res::CdrFields::TABLES.key);
    assert!(t.contains("/no/such/cdr-tables.json"), "{t}");
}

/// A change made to a good tables file.
type Edit = Box<dyn Fn(&mut serde_json::Value)>;

#[test]
fn malformed_tables_are_refused_with_the_field_named() {
    let good = std::fs::read_to_string(fixture_path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&good).unwrap();
    let cases: [(&str, Edit); 6] = [
        (
            "si: 215 entries",
            Box::new(|v| {
                v["si"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "msds: position",
            Box::new(|v| {
                v["msds"][1] = v["msds"][0].clone();
            }),
        ),
        (
            "sdis 3408: entry",
            Box::new(|v| {
                v["sdis"]["3408"][0] = serde_json::json!(5000);
            }),
        ),
        (
            "column 40 is beyond",
            Box::new(|v| {
                v["ldpc"][0]["rows"][0][0][0] = serde_json::json!(40);
            }),
        ),
        (
            "rows, the base matrix has 9",
            Box::new(|v| {
                v["ldpc"][0]["rows"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "a rate that is not",
            Box::new(|v| {
                v["ldpc"][0]["rate"] = serde_json::json!("2/3");
            }),
        ),
    ];
    for (want, edit) in cases {
        let mut bad = v.clone();
        edit(&mut bad);
        let e = CdrTables::from_json(&bad.to_string())
            .expect_err(want)
            .to_string();
        assert!(e.contains(want), "{want}: {e}");
    }
    assert!(CdrTables::from_json(&good).is_ok());
}

#[test]
fn tables_without_the_rate_in_use_are_named() {
    let t = fixture();
    assert!(t.code(LdpcRate::R3_4).is_some());
    assert!(t.code(LdpcRate::R1_2).is_none());
    let m = t.missing(3408, LdpcRate::R1_2).unwrap();
    assert!(m.contains("LDPC matrix of rate 1/2"), "{m}");
    let m = t.missing(1234, LdpcRate::R3_4).unwrap();
    assert!(m.contains("SDIS interleaver of 1234 bits"), "{m}");
}

#[test]
fn the_self_check_tables_decode_the_test_signal() {
    let x = generate(Some(fixture()), 3.0, &snr(20.0));
    let r = receive(&x, &config(Some(&fixture_path())));
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{}",
        r.status().summary
    );
    assert_eq!(text(&r, res::CdrFields::TABLES.key), "loaded");
    assert_eq!(int(&r, res::CdrFields::TX_MODE.key), 1);
    assert_eq!(int(&r, res::CdrFields::SPECTRUM_MODE.key), 1);
    assert_eq!(text(&r, res::CdrFields::LDPC_RATE.key), "3/4");
    assert!(int(&r, res::CdrFields::SI_OK.key) >= 8);
    assert!(int(&r, res::CdrFields::SI_BAD.key) <= 1);
    // The control multiplex frame rides on the SDIS (convolutional code): it decodes in full.
    assert!(int(&r, res::CdrFields::CONTROL_OK.key) >= 2);
    assert_eq!(int(&r, res::CdrFields::CONTROL_BAD.key), 0);
    assert_eq!(text(&r, res::CdrFields::NETWORK.key), "Test CDR");
    assert_eq!(text(&r, res::CdrFields::COUNTRY.key), "CHN");
    assert_eq!(int(&r, res::CdrFields::NETWORK_ID.key), 0x100);
    assert_eq!(
        cell_num(&r, &res::NETWORK_FREQUENCIES, "0", "freq_hz"),
        106.1e6
    );
    let mut ids = row_ids(&r, SERVICES.key);
    ids.sort();
    assert_eq!(ids, ["1001", "1002", "1003"]);
    // sdrkit-algo-fec has no encoder for a 9216-bit code word, so the generator sends random service data cells: the code words
    // are decoded and refused, and no service frame is made from them.
    let g = CdrGenerator::new(
        GenOptions {
            tables: Some(fixture()),
            ..Default::default()
        },
        RATE,
    )
    .unwrap();
    assert!(g.note().contains("no encoder"), "{}", g.note());
    assert!(int(&r, res::CdrFields::CW_BAD.key) > 0);
    assert_eq!(int(&r, res::CdrFields::SERVICE_OK.key), 0);
}

#[test]
fn the_offset_and_clock_error_are_followed() {
    let mut s = snr(20.0).to_vec();
    s.push(("gen_offset_hz", json!(1500.0)));
    s.push(("gen_clock_ppm", json!(40.0)));
    let x = generate(Some(fixture()), 3.0, &s);
    let r = receive(&x, &config(Some(&fixture_path())));
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{}",
        r.status().summary
    );
    let off = num(&r, "offset_hz");
    assert!((off - 1500.0).abs() < 5.0, "{off}");
    assert!(int(&r, res::CdrFields::CONTROL_OK.key) >= 2);
}

#[test]
fn noise_only() {
    let s = [
        ("gen_noise", json!("floor")),
        ("gen_noise_floor_dbfs", json!(-10.0)),
        ("gen_seed", json!(9)),
    ];
    let x = impair(
        Box::new(sdrkit_stack_broadcast::silence::Silent(RATE)),
        2.0,
        &s,
    );
    let r = receive(&x, &config(Some(&fixture_path())));
    assert_eq!(
        r.status().lock_state,
        LockState::Searching,
        "{}",
        r.status().summary
    );
    assert!(row_ids(&r, SERVICES.key).is_empty());
    assert_eq!(int(&r, res::CdrFields::SI_OK.key), 0);
}

#[test]
fn the_physical_layer_follows_table_4_3() {
    for t in [TxMode(1), TxMode(2), TxMode(3)] {
        let l = phy::layout(t, true);
        assert_eq!(l.frame_len(), phy::SUBFRAME);
        assert_eq!(
            t.beacon_cp() + t.nfft(),
            [2432, 1356, 2216][t.0 as usize - 1]
        );
        assert!((t.spacing() - [398.4375, 796.875, 398.4375][t.0 as usize - 1]).abs() < 1e-9);
    }
    assert_eq!(phy::half_carriers(TxMode(3), 0.0, true), -121..=-1);
    assert_eq!(phy::half_carriers(TxMode(1), 200.0, false), 503..=623);
    for t in [TxMode(1), TxMode(2), TxMode(3)] {
        for m in SpectrumMode::ALL {
            let c = phy::subframe_cells(t, m);
            assert!(c.si.iter().all(|v| v.len() == 108));
            assert!(4 * c.data.len() >= m.ni() * (t.msds_cells() + t.sdis_cells()));
        }
    }
    let p = cdr::plan(TxMode(1), SpectrumMode(1), Qam::Q4, Qam::Q4, LdpcRate::R3_4);
    assert_eq!(p.sdis_bits, 3408);
}

#[test]
fn the_system_information_round_trips_with_its_crc() {
    let si = SystemInfo {
        mf: true,
        next_freq: 511,
        nominal: 3,
        spectrum: 9,
        frame: 2,
        subframe: 1,
        allocation: 3,
        sdi_qam: 1,
        data_qam: 2,
        hierarchy: 0,
        uniform: true,
        rate_hi: 3,
        rate_lo: 3,
    };
    assert_eq!(SystemInfo::from_bits(&si.bits()), Some(si));
    let mut b = si.bits();
    b[5] ^= 1;
    assert_eq!(SystemInfo::from_bits(&b), None);
}

#[test]
fn control_and_service_frames_round_trip() {
    let cf = ControlFrame {
        smct_update: 3,
        smct: vec![SmctEntry {
            id: 0,
            hierarchical: false,
            high: false,
            mask: 15,
            services: vec![0x1001, 0x1002, 0x1003],
        }],
        nit: Some(Nit {
            update: 1,
            country: "CHN".into(),
            network_id: 0x100,
            frequencies: vec![10_610_000],
            name: "Test CDR".into(),
        }),
        not_decoded: vec![0x03],
    };
    let b = cf.to_bytes(200).unwrap();
    let (back, good, bad) = ControlFrame::parse(&b).unwrap();
    assert_eq!(back, cf);
    assert_eq!((good, bad), (3, 0));
    let sf = ServiceFrame {
        id: 0,
        emergency: false,
        updates: (1, 3, 0),
        subs: vec![SubFrame {
            start: 1234,
            audio: vec![(0, vec![1, 2, 3]), (0, vec![4, 5])],
            data: vec![(160, b"hello".to_vec())],
            streams: vec![AudioStream {
                algorithm: 0,
                bitrate: 640,
                rate_code: 8,
                channels: 2,
                language: "chi".into(),
            }],
        }],
    };
    let b = sf.to_bytes(300).unwrap();
    let (h, subs) = ServiceFrame::parse(&b).unwrap();
    assert_eq!(h.updates, (1, 3, 0));
    let s = subs[0].as_ref().unwrap();
    assert!(s.audio_ok && s.data_ok);
    assert_eq!(s.sub, sf.subs[0]);
    assert_eq!(sample_rate(8), 48_000);
}

/// A change of frequency forgets the multiplex: the services, the network, the counts and the measurements. The stack
/// keeps nothing over it. What the tables lack stays: it is no result of the signal.
#[test]
fn a_change_of_frequency_forgets_the_multiplex() {
    use res::CdrFields as F;
    let x = generate(Some(fixture()), 3.0, &snr(20.0));
    let mut rx = cdr::make_receiver(&config(Some(&fixture_path()))).unwrap();
    rx.configure(&InputInfo::new(RATE, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(100_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = rx.results();
    assert!(!row_ids(r, SERVICES.key).is_empty());
    assert_eq!(text(r, F::NETWORK.key), "Test CDR");
    assert!(!is_none(r, F::SNR_DB.key));
    let subframes = int(r, F::SUBFRAMES.key);
    rx.forget();
    let r = rx.results();
    assert!(row_ids(r, SERVICES.key).is_empty());
    assert!(row_ids(r, res::NETWORK_FREQUENCIES.key).is_empty());
    for key in [
        F::NETWORK.key,
        F::NETWORK_ID.key,
        F::SNR_DB.key,
        F::OFFSET_HZ.key,
        F::LEVEL_DBFS.key,
        F::TX_MODE.key,
        F::LDPC_RATE.key,
        F::ITERATIONS.key,
    ] {
        assert!(is_none(r, key), "{key}");
    }
    assert_eq!(int(r, F::SUBFRAMES.key), 0);
    assert_eq!(int(r, F::CONTROL_OK.key), 0);
    assert_eq!(text(r, F::TABLES.key), "loaded");
    // The counts start from zero: the same signal again counts about as much as the first time, which includes the
    // first search, and far from twice as much.
    for c in x.chunks(100_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let again = int(rx.results(), F::SUBFRAMES.key);
    assert!(
        again > 0 && again < subframes * 3 / 2,
        "{again} after {subframes}"
    );
}

/// Frame error rate against the min-sum scale (12 to 16 sixteenths) for each LDPC code of a tables file: BPSK in white
/// noise, the default quantiser and 40 iterations as in the receiver, the same frames for every scale. Each line gives, per
/// scale, the Eb/N0 at a frame error rate of 0.1, then the best scale and its gain over 13/16. `SDRKIT_CDR_TABLES` names
/// the file (default: the self-check tables), `LDPC_SCALE_FRAMES` the frames a point (default 1000). Run with
/// `cargo test --release -p sdrkit-stack-broadcast --test broadcast cdr::ldpc_scale -- --ignored --nocapture`.
#[test]
#[ignore = "measurement: minutes in release"]
fn ldpc_scale() {
    use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
    use sdrkit_algo_fec::llr::Quantizer;
    use sdrkit_algo_sim::rng::Rng;
    use sdrkit_dev_test::fer::{self, Trial, SCALES};
    const TARGET: f64 = 0.1;

    /// One code with a decoder per scale; a code without an encoder sends the all-zero word.
    struct Run<'a> {
        code: &'a LdpcCode,
        decs: Vec<LdpcDecoder>,
        rng: Rng,
        sigma2: f64,
        info: Vec<u8>,
        cw: Vec<u8>,
        word: Vec<f32>,
        llr: Vec<i8>,
        out: Vec<u8>,
    }

    impl Trial for Run<'_> {
        fn start(&mut self, ebn0_db: f64, seed: u64) {
            let (k, n) = (self.code.k(), self.code.n());
            self.sigma2 = n as f64 / (2.0 * k as f64 * 10f64.powf(ebn0_db / 10.0));
            self.rng = Rng::new(seed);
        }

        fn send(&mut self, _f: u64) {
            if self.code.can_encode() {
                self.rng.fill_bits(&mut self.info);
                self.code.encode(&self.info, &mut self.cw).unwrap();
            }
            for (w, &b) in self.word.iter_mut().zip(&self.cw) {
                let y = if b == 0 { 1.0 } else { -1.0 } + self.sigma2.sqrt() * self.rng.normal();
                *w = (2.0 * y / self.sigma2) as f32;
            }
            Quantizer::default().quantize_auto(&self.word, &mut self.llr).unwrap();
        }

        fn lost(&mut self, i: usize) -> bool {
            self.decs[i].decode(&self.llr, 40, &mut self.out).is_err() || self.out != self.cw
        }
    }

    let path = std::env::var("SDRKIT_CDR_TABLES").unwrap_or_else(|_| fixture_path());
    let frames = fer::frames(1000);
    let tables = CdrTables::load(&path).expect("the tables load");
    let codes: Vec<_> = LdpcRate::ALL.iter().filter_map(|&r| tables.code(r)).collect();
    println!("Eb/N0 at frame error rate {TARGET}, {frames} frames a point; scale/16 = {SCALES:?}");
    let lines = fer::parallel(&codes, |&code| {
        let (k, n) = (code.k(), code.n());
        let mut run = Run {
            code,
            decs: SCALES.iter().map(|&q| LdpcDecoder::new(code, MinSum::with_scale(q))).collect(),
            rng: Rng::new(0),
            sigma2: 1.0,
            info: vec![0; k],
            cw: vec![0; n],
            word: vec![0.0; n],
            llr: vec![0; n],
            out: vec![0; n],
        };
        let points = fer::search(&mut run, SCALES.len(), -1.0, 6.0, frames, TARGET);
        fer::line(&format!("{:<16}", code.name()), &SCALES, &points, TARGET)
    });
    for l in lines {
        println!("{l}");
    }
}
