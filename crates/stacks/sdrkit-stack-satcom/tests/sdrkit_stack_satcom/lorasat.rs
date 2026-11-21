//! LoRa satellites: the test signal's passes through the receiver, the Doppler rate, saved state and settings.

use crate::common::{cell, cell_num, command, number, row_count, set};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Choice;
use sdrkit_core::{
    Complex32, Event, Generator, InputInfo, Output, Receiver, Results, StackConfig, StateBlob,
    Values,
};
use sdrkit_stack_satcom::modes::lorasat::{self, gen};
use serde_json::json;

const RATE: f64 = 1e6;

/// One frame row of the results.
#[derive(Debug)]
struct Row {
    sf: i64,
    bw: f64,
    crc: String,
    doppler: f64,
    rate: f64,
    satellite: String,
    text: String,
}

fn rows(r: &Results) -> Vec<Row> {
    crate::common::rows(r, "frames")
        .into_iter()
        .map(|(_, c)| Row {
            sf: cell_num(c, "sf").map_or(-1, |v| v as i64),
            bw: cell_num(c, "bw_hz").unwrap_or(f64::NAN),
            crc: cell(c, "crc"),
            doppler: cell_num(c, "doppler_hz").unwrap_or(f64::NAN),
            rate: cell_num(c, "doppler_rate_hz_s").unwrap_or(f64::NAN),
            satellite: cell(c, "satellite"),
            text: cell(c, "text"),
        })
        .collect()
}

fn texts(r: &Results) -> Vec<String> {
    let mut v: Vec<String> = rows(r)
        .into_iter()
        .filter(|f| f.crc == "ok")
        .map(|f| f.text)
        .collect();
    v.sort();
    v
}

fn field(r: &Results, key: &str) -> f64 {
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(number)
        .unwrap_or(f64::NAN)
}

/// The stack's settings with `extra` on top.
fn values(extra: &[CaseSetting]) -> Values {
    crate::common::values(&lorasat::DESCRIPTOR, extra)
}

/// The test signal of `p` for `seconds`, with noise at `snr_db` in the LoRa bandwidth and a radio error of `offset_hz`.
fn signal(p: &gen::GenParams, seconds: f64, snr_db: Option<f64>, offset_hz: f64) -> Vec<Complex32> {
    let mut set = vec![
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset_hz),
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 3),
    ];
    if let Some(s) = snr_db {
        set.push(CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        ));
        set.push(CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, s));
    }
    let imp = values(&set);
    let g = gen::LoraSatGenerator::new(p.clone());
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &imp).unwrap();
    let mut x = vec![Complex32::default(); (seconds * p.rate) as usize];
    g.generate(&mut x);
    // The generator's 0 Hz is the carrier; the receiver's centre is the tune offset above it.
    sdrkit_algo_sim::offset::FrequencyOffset::new(p.rate, p.freq - p.centre, 0.0)
        .unwrap()
        .process(&mut x);
    x
}

fn receiver(p: &gen::GenParams, extra: &[CaseSetting]) -> Box<dyn Receiver> {
    let mut r = (lorasat::DESCRIPTOR.make_receiver)(&StackConfig::new(values(extra))).unwrap();
    r.configure(&InputInfo::new(p.rate, p.centre, p.freq).recording())
        .unwrap();
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32]) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(65_536) {
        out.clear();
        r.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    ev
}

fn run(
    p: &gen::GenParams,
    seconds: f64,
    snr_db: Option<f64>,
    extra: &[CaseSetting],
) -> (Box<dyn Receiver>, Vec<Event>) {
    let x = signal(p, seconds, snr_db, 0.0);
    let mut r = receiver(p, extra);
    let ev = feed(r.as_mut(), &x);
    (r, ev)
}

#[test]
fn frames_over_a_pass_at_436_8() {
    for start in [20.0, 295.0, 560.0] {
        let mut p = gen::GenParams::new(RATE, 436.8e6);
        p.period_s = 1.5;
        p.start_s = start;
        let n = 3;
        let (r, _) = run(
            &p,
            0.3 + 1.5 * n as f64,
            Some(5.0),
            &[CaseSetting::new(
                &sdrkit_stack_satcom::modes::lorasat::display::PASS_SATELLITE,
                "FOSSASAT-2",
            )],
        );
        let res = r.results();
        let want: Vec<String> = (0..n).map(gen::beacon).collect();
        assert_eq!(texts(res), want, "start {start}: {:?}", rows(res));
        assert_eq!(field(res, "frames_bad"), 0.0);
        let curve = res.decl().series_id("doppler").unwrap();
        assert_eq!(res.series(curve).unwrap().points.len(), n as usize);
        let g = gen::LoraSatGenerator::new(p.clone());
        let sym = 2048.0 / 125e3;
        for (i, f) in rows(res).iter().rev().enumerate() {
            assert_eq!(f.satellite, "FOSSASAT-2");
            let k: u64 = f.text[19..].trim().parse().unwrap();
            let t0 = g.frame_start(k);
            // The offset at the first data symbol (about 12 symbols in).
            let want = g.shift(t0 + 12.25 * sym);
            assert!(
                (f.doppler - want).abs() < 60.0,
                "start {start}: {} vs {want}",
                f.doppler
            );
            let wr = lorasat::pass_rate(436.8e6, g.pass_time(t0 + 0.4), 300.0);
            if i > 0 {
                // From the Doppler curve: frames 1.5 s apart, each offset within a few Hz.
                assert!(
                    (f.rate - wr).abs() < (0.3 * wr.abs()).max(30.0),
                    "start {start}: rate {} vs {wr}",
                    f.rate
                );
            }
        }
        // A preset runs one detector: SF11, sync 0x12.
        assert_eq!(row_count(res, "demodulators"), 1);
        let d = crate::common::rows(res, "demodulators");
        assert_eq!(cell(d[0].1, "sf"), "11");
        let syncs = crate::common::rows(res, "demodulator_syncs");
        assert_eq!(syncs.len(), 1);
        assert_eq!(cell_num(syncs[0].1, "sync"), Some(18.0));
    }
}

#[test]
fn telemetry_at_137_mhz() {
    let mut p = gen::GenParams::new(RATE, 137.055e6);
    p.period_s = 1.0;
    p.start_s = 250.0;
    let (r, _) = run(&p, 3.3, Some(10.0), &[]);
    assert_eq!(
        texts(r.results()),
        (0..3).map(gen::beacon).collect::<Vec<_>>()
    );
    assert!(rows(r.results()).iter().all(|f| f.sf == 8));
}

#[test]
fn spreading_factors_at_868_mhz() {
    for sf in [7u8, 9, 12] {
        let mut p = gen::GenParams::new(RATE, 868.1e6);
        p.sf = sf;
        p.sync = 0x34;
        p.period_s = if sf == 12 { 3.5 } else { 1.0 };
        p.start_s = 150.0;
        let n = 2;
        let (r, _) = run(
            &p,
            0.3 + p.period_s * n as f64,
            Some(10.0),
            &[
                CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::FOLLOW_DIAL, false),
                CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::AUTO_SF, false),
                CaseSetting::new(
                    &sdrkit_stack_satcom::modes::lorasat::rx::SF,
                    sdrkit_stack_satcom::modes::lorasat::Sf::from_key(&sf.to_string())
                        .expect("a spreading factor"),
                ),
            ],
        );
        assert_eq!(
            texts(r.results()),
            (0..n).map(gen::beacon).collect::<Vec<_>>(),
            "SF{sf}"
        );
    }
}

#[test]
fn bandwidth_searched() {
    let mut p = gen::GenParams::new(2e6, 436.0e6);
    p.sf = 9;
    p.bw = 250e3;
    p.cr = 5;
    p.period_s = 0.6;
    p.start_s = 100.0;
    let (r, _) = run(
        &p,
        2.1,
        Some(10.0),
        &[
            CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::AUTO_BW, true),
            CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::AUTO_SF, false),
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::lorasat::rx::SF,
                sdrkit_stack_satcom::modes::lorasat::Sf::Sf9,
            ),
        ],
    );
    assert_eq!(
        texts(r.results()),
        (0..3).map(gen::beacon).collect::<Vec<_>>()
    );
    assert!(rows(r.results()).iter().all(|f| f.bw == 250e3));
}

#[test]
fn low_snr_with_a_radio_error() {
    let mut p = gen::GenParams::new(RATE, 436.8e6);
    p.period_s = 1.5;
    p.start_s = 250.0;
    let x = signal(&p, 0.3 + 1.5 * 3.0, Some(-5.0), 3000.0);
    let mut r = receiver(&p, &[]);
    feed(r.as_mut(), &x);
    assert!(texts(r.results()).len() >= 2, "{:?}", rows(r.results()));
}

#[test]
fn noise_returns_nothing_and_the_state_is_saved() {
    let p = gen::GenParams::new(RATE, 436.8e6);
    let mut q = p.clone();
    q.period_s = 1.5;
    q.start_s = 250.0;
    let (mut r, _) = run(&q, 2.0, Some(10.0), &[]);
    assert_eq!(field(r.results(), "frames_ok"), 1.0);
    let state = r.save_state().expect("a state");
    // Noise alone: nothing decodes and the lock state shows a search.
    let mut z = receiver(&p, &[]);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
    let noise: Vec<Complex32> = (0..(2.0 * RATE) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.05)
        .collect();
    feed(z.as_mut(), &noise);
    assert!(rows(z.results()).is_empty());
    assert_eq!(
        z.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
    // The saved frame comes back. A state of another stack is refused and changes nothing.
    z.restore_state(&state).unwrap();
    feed(z.as_mut(), &noise[..1000]);
    assert_eq!(texts(z.results()), vec![gen::beacon(0)]);
    assert!(z
        .restore_state(&StateBlob::new("goes_dcs", 1, Vec::new()))
        .is_err());
    assert_eq!(texts(z.results()), vec![gen::beacon(0)]);
    command(r.as_mut(), "clear").unwrap();
    feed(r.as_mut(), &noise[..1000]);
    assert!(rows(r.results()).is_empty());
    assert_eq!(field(r.results(), "frames_ok"), 0.0);
}

/// Without a recent Doppler curve, the rate comes from the tracking detector, and it is not known when no detector tracks the
/// carrier.
#[test]
fn the_first_frame_of_a_pass_takes_the_tracked_rate() {
    let mut p = gen::GenParams::new(RATE, 436.8e6);
    p.sf = 10;
    p.cr = 5;
    p.period_s = 30.0;
    let fixed = |track: bool| {
        vec![
            CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::FOLLOW_DIAL, false),
            CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::AUTO_SF, false),
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::lorasat::rx::SF,
                sdrkit_stack_satcom::modes::lorasat::Sf::Sf10,
            ),
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::lorasat::rx::SYNC,
                sdrkit_stack_satcom::modes::lorasat::rx::Sync::Private,
            ),
            CaseSetting::new(&sdrkit_stack_satcom::modes::lorasat::rx::TRACK, track),
        ]
    };
    for start in [280.0, 299.0] {
        p.start_s = start;
        let g = gen::LoraSatGenerator::new(p.clone());
        let want = lorasat::pass_rate(436.8e6, g.pass_time(g.frame_start(0) + 0.4), 300.0);
        let (r, _) = run(&p, 1.6, Some(10.0), &fixed(true));
        let f = rows(r.results());
        assert_eq!(f.len(), 1, "start {start}: {f:?}");
        assert!(
            (f[0].rate - want).abs() < 50.0,
            "start {start}: rate {} vs {want}",
            f[0].rate
        );
        let (r, _) = run(&p, 1.6, Some(10.0), &fixed(false));
        let f = rows(r.results());
        assert_eq!(f.len(), 1, "start {start}: {f:?}");
        assert!(
            f[0].rate.is_nan(),
            "start {start}: no tracking, yet a rate of {}",
            f[0].rate
        );
    }
}

/// A retune to the same plan resets the detectors and keeps their counters.
#[test]
fn detector_counters_stay_over_a_retune() {
    let mut p = gen::GenParams::new(RATE, 436.8e6);
    p.period_s = 1.5;
    p.start_s = 250.0;
    let x = signal(&p, 1.9, Some(10.0), 0.0);
    let mut r = receiver(&p, &[]);
    feed(r.as_mut(), &x);
    let frames = |r: &dyn Receiver| -> String {
        cell(
            crate::common::rows(r.results(), "demodulators")[0].1,
            "frames",
        )
    };
    assert_eq!(frames(r.as_ref()), "1");
    r.retuned(&InputInfo::new(p.rate, p.centre, p.freq).recording());
    feed(r.as_mut(), &x[..(0.5 * RATE) as usize]);
    assert_eq!(frames(r.as_ref()), "1");
}

#[test]
fn events_are_declared_and_carry_the_frequency() {
    let mut p = gen::GenParams::new(RATE, 436.8e6);
    p.period_s = 1.5;
    p.start_s = 250.0;
    let (_, ev) = run(&p, 3.5, Some(10.0), &[]);
    assert_eq!(ev.len(), 2);
    for e in &ev {
        assert!(e.is(&sdrkit_stack_satcom::typed::LORA_SAT_FRAME), "{e:?}");
        assert!((e.freq_hz - 436.8e6).abs() < 20e3, "{}", e.freq_hz);
    }
}

#[test]
fn bad_settings_are_refused() {
    let p = gen::GenParams::new(RATE, 436.8e6);
    let mut r = receiver(&p, &[]);
    for (k, v) in [
        ("sf", json!("13")),
        ("bw_khz", json!("63")),
        ("sync", json!("0x99")),
        ("sync_other", json!(300)),
        ("preamble", json!(2)),
        ("doppler_max_hz", json!(-1.0)),
        ("track", json!("maybe")),
    ] {
        assert!(set(r.as_mut(), k, v.clone()).is_err(), "{k} = {v:?}");
    }
    assert!(set(r.as_mut(), "sf", json!("9")).is_ok());
}
