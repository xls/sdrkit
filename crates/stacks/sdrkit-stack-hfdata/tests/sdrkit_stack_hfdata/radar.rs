//! HF ocean radar: the recording, the acceptance cases, saved state and the scan requests.

use crate::common::*;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::voice::Voice;
use sdrkit_core::media::Picture;
use sdrkit_core::{
    Complex32, Generator, HostRequest, InputInfo, LockState, Output, Receiver, StackConfig, Value,
};
use sdrkit_graph::{RunContext, StackGraph, TapReader};
use sdrkit_stack_hfdata::radar::{
    self,
    gen::{RadarGenOptions, RadarGenerator, TestRadar},
    radars::{MARKS_SERIES, SNR_SERIES},
};
use sdrkit_stack_hfdata::typed::{HFRADAR_BANDS, HFRADAR_HEARD, HFRADAR_RADARS};

fn radars(rx: &dyn Receiver) -> Vec<Row> {
    rows(rx.results(), &HFRADAR_RADARS)
}

fn show(rx: &dyn Receiver) {
    for r in radars(rx) {
        eprintln!(
            "#{} centre {:.4} MHz width {:.1} kHz rate {:.1} kHz/s {} repeat {:?} snr {:.1} sweeps {} band {} id {} hint {}",
            num(&r, "number"),
            num(&r, "centre") / 1e6,
            num(&r, "width") / 1e3,
            num(&r, "rate_hz_s") / 1e3,
            text(&r, "direction"),
            Some(as_num(&cell(&r, "repeat"))),
            num(&r, "snr_db"),
            num(&r, "sweeps"),
            text(&r, "band"),
            text(&r, "id"),
            text(&r, "hint")
        );
    }
    eprintln!("{}", rx.results().status().summary);
}

/// The ocean radar receiver with its default settings.
fn graph_receiver() -> StackGraph {
    radar::receiver(&StackConfig::new(values(&radar::HFRADAR, &[]))).expect("the graph builds")
}

/// The newest waterfall picture that the tap holds.
fn last_picture(tap: &mut TapReader) -> Option<Picture> {
    use sdrkit_graph::{Buffer, Data, Kind, Timed};
    let mut last = None;
    let mut b = Buffer::new(&Kind::typed::<Picture>());
    while tap.pop(&mut b) {
        if let Data::Items(Some(items)) = &b.data {
            if let Some(v) = items.as_any().downcast_ref::<Vec<Timed<Picture>>>() {
                if let Some(p) = v.last() {
                    last = Some(p.value.clone());
                }
            }
        }
    }
    last
}

/// The number of points of series `key` of the results.
fn series_len(rx: &dyn Receiver, key: &str) -> usize {
    let r = rx.results();
    r.decl()
        .series_id(key)
        .and_then(|id| r.series(id))
        .map_or(0, |s| s.points.len())
}

#[test]
fn c8_recording() {
    let rec = vector("codar_250k");
    let exp = expected("codar_250k");
    let mut rx = receiver(&radar::HFRADAR, &[]);
    run_vector(rx.as_mut(), &rec);
    show(rx.as_ref());
    let got = radars(rx.as_ref());
    assert_eq!(got.len(), 2);
    for w in exp["radars"].as_array().unwrap() {
        let dir = if w["dir"] == 1 { "rising" } else { "falling" };
        let g = got
            .iter()
            .find(|r| text(r, "direction") == dir)
            .expect("radar");
        assert!((num(g, "centre") / 1e6 - w["centre_mhz"].as_f64().unwrap()).abs() <= 0.004);
        let wk = w["width_khz"].as_f64().unwrap();
        assert!((num(g, "width") / 1e3 - wk).abs() <= 0.15 * wk);
        let rk = w["rate_khz_s"].as_f64().unwrap();
        assert!((num(g, "rate_hz_s") / 1e3 - rk).abs() <= 0.12 * rk);
        assert!((num(g, "repeat") - w["period_s"].as_f64().unwrap()).abs() <= 0.02);
        assert_eq!(text(g, "band"), w["band"].as_str().unwrap());
        assert!(text(g, "hint").starts_with(w["hint_starts"].as_str().unwrap()));
        let snr = num(g, "snr_db");
        assert!((14.0..=24.0).contains(&snr), "snr {snr}");
    }
    assert_eq!(field_text(rx.as_ref(), "band"), "12 - 14 MHz");
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert!(field(rx.as_ref(), "sweeps_counted").unwrap() >= 12.0);
    assert_eq!(rows(rx.results(), &HFRADAR_HEARD).len(), 2);
}

/// The clean radars of `o` with noise at `snr_db` (in one bin of the 1 ms Hann FFT), fed to `rx` with the radio at `centre`.
fn run_into(rx: &mut dyn Receiver, o: RadarGenOptions, secs: f64, centre: f64, snr_db: f64) {
    let rate = o.rate;
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(1.0));
    put(&mut v, "gen_noise", json!("snr"));
    put(&mut v, "gen_snr_db", json!(snr_db));
    let mut g = sdrkit_components::sim::impaired(Box::new(RadarGenerator::new(o)), &v)
        .expect("impairments");
    let x = generate(g.as_mut(), secs);
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    feed(rx, &x, (rate / 10.0) as usize);
}

fn run(o: RadarGenOptions, secs: f64, centre: f64) -> StackGraph {
    let mut rx = graph_receiver();
    run_into(&mut rx, o, secs, centre, 20.0);
    rx
}

/// As [`run`], with a tap on the waterfall pictures.
fn run_tapped(o: RadarGenOptions, secs: f64, centre: f64) -> (StackGraph, TapReader) {
    let rate = o.rate;
    let mut rx = graph_receiver();
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(1));
    put(&mut v, "gen_noise", json!("snr"));
    put(&mut v, "gen_snr_db", json!(20.0));
    let mut g = sdrkit_components::sim::impaired(Box::new(RadarGenerator::new(o)), &v)
        .expect("impairments");
    let x = generate(g.as_mut(), secs);
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    let tap = rx
        .graph_mut()
        .tap("radars.waterfall", 1 << 12)
        .expect("a tap");
    feed(&mut rx, &x, (rate / 10.0) as usize);
    (rx, tap)
}

#[test]
fn c1_three_radars() {
    let (rx, mut tap) = run_tapped(RadarGenOptions::default(), 12.0, 13.0e6);
    show(&rx);
    let got = radars(&rx);
    assert_eq!(got.len(), 3);
    let a = got
        .iter()
        .find(|r| (num(r, "offset_hz") + 300e3).abs() < 10e3)
        .unwrap();
    assert_eq!(text(a, "direction"), "rising");
    assert!((num(a, "repeat") - 0.5).abs() <= 0.02);
    assert!((num(a, "rate_hz_s") / 1e3 - 111.0).abs() <= 12.0);
    assert!((num(a, "centre") / 1e6 - 12.7).abs() <= 0.005);
    assert!(num(a, "sweeps") >= 15.0);
    assert!(num(a, "snr_db") > 8.0);
    assert!(text(a, "hint").starts_with("possible"));
    assert_eq!(cell(a, "id"), Value::None);
    let b = got
        .iter()
        .find(|r| text(r, "direction") == "falling")
        .unwrap();
    assert!((num(b, "repeat") - 1.0).abs() <= 0.03);
    let c = got
        .iter()
        .find(|r| (num(r, "offset_hz") - 550e3).abs() < 10e3)
        .unwrap();
    assert!((num(c, "width") / 1e3 - 25.0).abs() <= 5.0);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert_eq!(field_text(&rx, "band"), "12 - 14 MHz");
    let heard = rows(rx.results(), &HFRADAR_HEARD);
    assert!(heard.iter().any(|e| text(e, "band") == "12 - 14 MHz"));
    let pic = last_picture(&mut tap).expect("a waterfall picture");
    let layout = pic.layout.as_ref().unwrap();
    assert_eq!((layout.width, layout.height), (256, 160));
    let lit = pic.data.iter().filter(|&&v| v > 64).count();
    assert!(lit > 200, "lit {lit}");
    assert!(series_len(&rx, MARKS_SERIES) > 0);
    assert!(series_len(&rx, SNR_SERIES) > 0);
}

#[test]
fn c2_morse_id() {
    // 30 s at 2 Msps.
    let mut o = RadarGenOptions::default();
    o.radars[0].id = Some(("AB1CD".into(), 15.0, 14.0));
    let rx = run(o, 30.0, 13.0e6);
    show(&rx);
    let got = radars(&rx);
    assert_eq!(got.len(), 3);
    let a = got
        .iter()
        .find(|r| (num(r, "offset_hz") + 300e3).abs() < 10e3)
        .unwrap();
    assert_eq!(text(a, "id"), "AB1CD");
    assert!(num(a, "id_seen") >= 2.0);
    assert!((num(a, "id_wpm") - 15.0).abs() <= 3.0);
    let b = got
        .iter()
        .find(|r| text(r, "direction") == "falling")
        .unwrap();
    assert_eq!(cell(b, "id"), Value::None);
}

#[test]
fn c3_noise_and_junk() {
    let quiet = RadarGenOptions {
        radars: vec![],
        ..Default::default()
    };
    let rx = run(quiet, 8.0, 13.0e6);
    assert!(radars(&rx).is_empty());
    assert_eq!(value(&rx, "sweeps_counted"), Value::I64(0));
    let junk = RadarGenOptions {
        radars: vec![],
        junk: true,
        ..Default::default()
    };
    let rx = run(junk, 14.0, 13.0e6);
    show(&rx);
    assert!(radars(&rx).is_empty());
}

#[test]
fn c4_one_radar_other_rates() {
    for &(rate, off) in &[(250e3, 20e3), (1e6, 100e3), (4e6, 100e3)] {
        let o = RadarGenOptions {
            rate,
            radars: vec![TestRadar {
                offset_hz: off,
                width_hz: 50e3,
                repeat_s: 0.5,
                dir: 1.0,
                level_db: 0.0,
                start_s: 0.1,
                id: None,
            }],
            ..Default::default()
        };
        let rx = run(o, 8.0, 4.5e6);
        show(&rx);
        let got = radars(&rx);
        assert_eq!(got.len(), 1, "{rate}");
        assert_eq!(text(&got[0], "direction"), "rising");
        assert!((num(&got[0], "repeat") - 0.5).abs() <= 0.03);
        assert_eq!(text(&got[0], "band"), "4.4 - 4.65 MHz");
    }
}

#[test]
fn c5_save_restore_clear() {
    let rec = vector("codar_250k");
    let mut rx = receiver(&radar::HFRADAR, &[]);
    run_vector(rx.as_mut(), &rec);
    let state = rx.save_state().expect("a saved state");
    let mut rx2 = receiver(&radar::HFRADAR, &[]);
    rx2.configure(&InputInfo::new(250e3, 13e6, 13e6).recording())
        .unwrap();
    rx2.restore_state(&state).unwrap();
    assert_eq!(radars(rx2.as_ref()).len(), 2);
    assert_eq!(rows(rx2.results(), &HFRADAR_HEARD).len(), 2);
    let garbage = sdrkit_core::StateBlob::new("hfradar", 1, b"garbage".to_vec());
    assert!(rx2.restore_state(&garbage).is_err());
    assert_eq!(radars(rx2.as_ref()).len(), 2);
    invoke(rx2.as_mut(), "clear", None);
    assert!(radars(rx2.as_ref()).is_empty());
    assert!(rows(rx2.results(), &HFRADAR_HEARD).is_empty());
}

/// Feeds `x` in pieces of `piece` and keeps the host requests.
fn feed_requests(
    rx: &mut dyn Receiver,
    x: &[sdrkit_core::Complex32],
    piece: usize,
) -> Vec<HostRequest> {
    let mut all = Vec::new();
    let mut out = Output::default();
    for c in x.chunks(piece) {
        rx.feed(c, &mut out);
        all.append(&mut out.requests);
        out.clear();
    }
    all
}

#[test]
fn scan_asks_the_host_to_step_through_the_bands() {
    let mut rx = graph_receiver();
    rx.set_context(RunContext {
        live: true,
        ..Default::default()
    });
    set(&mut rx, "scan", json!(true)).unwrap();
    set(&mut rx, "dwell_s", json!(30.0)).unwrap();
    let mut live = InputInfo::new(2e6, 13e6, 13e6);
    live.live = true;
    rx.configure(&live).unwrap();
    let requests = feed_requests(&mut rx, &vec![Default::default(); 600_000], 200_000);
    let hop = requests
        .iter()
        .find_map(|r| match r {
            HostRequest::Hop {
                channels, dwell_s, ..
            } => Some((channels.clone(), *dwell_s)),
            _ => None,
        })
        .expect("a hop request");
    assert_eq!(hop.0, radar::bands::scan_plan(2e6));
    assert_eq!(hop.1, 30.0);
    assert!(rx.results().status().summary.contains("stepping"));
    set(&mut rx, "scan", json!(false)).unwrap();
    let requests = feed_requests(&mut rx, &vec![Default::default(); 200_000], 200_000);
    assert!(requests
        .iter()
        .any(|r| matches!(r, HostRequest::Retune { .. })));
    // A recording never scans.
    let mut rec = graph_receiver();
    set(&mut rec, "scan", json!(true)).unwrap();
    rec.configure(&InputInfo::new(2e6, 13e6, 13e6).recording())
        .unwrap();
    assert!(feed_requests(&mut rec, &vec![Default::default(); 600_000], 200_000).is_empty());
}

#[test]
fn a_retune_keeps_each_radar_on_its_frequency() {
    let mut rx = run(RadarGenOptions::default(), 6.0, 13.0e6);
    let a0 = radars(&rx)
        .into_iter()
        .find(|r| (num(r, "offset_hz") + 300e3).abs() < 10e3)
        .unwrap();
    rx.retuned(&InputInfo::new(2e6, 13.1e6, 13.1e6).recording());
    feed(&mut rx, &vec![Default::default(); 400_000], 200_000);
    let a1 = radars(&rx)
        .into_iter()
        .find(|r| num(r, "number") == num(&a0, "number"))
        .unwrap();
    assert!((num(&a1, "centre") - num(&a0, "centre")).abs() < 1.0);
    assert!((num(&a1, "offset_hz") - num(&a0, "offset_hz") + 100e3).abs() < 1.0);
    assert!(num(&a1, "last") <= 6.0);
}

/// The amplitude of the strongest radar in [`Band`]: 40 dB under full scale, so strong carriers fit beside it.
const RADAR_GAIN: f32 = 0.01;

/// A band for the tests: radars of the test generator, AM broadcast carriers with speech, and noise with its own seed.
struct Band {
    radars: RadarGenerator,
    /// Offset (Hz) and amplitude of each carrier, with its oscillator and its speech.
    carriers: Vec<(Nco, Voice, f32)>,
    noise: sdrkit_dev_test::Awgn,
    rate: f64,
    audio: Vec<f32>,
    am: Vec<Complex32>,
}

impl Band {
    /// The radars of `o` at [`RADAR_GAIN`], noise at `snr_db` in one bin of the receiver's FFT for the strongest radar,
    /// seed `seed`, and the carriers `(offset Hz, dB over the strongest radar)`.
    fn new(o: RadarGenOptions, snr_db: f64, seed: u64, carriers: &[(f64, f32)]) -> Band {
        let rate = o.rate;
        let bin_bw = 1.5 * rate / radar::detect::fft_size(rate) as f64;
        Band {
            radars: RadarGenerator::new(o),
            carriers: carriers
                .iter()
                .enumerate()
                .map(|(k, &(f, db))| {
                    let a = RADAR_GAIN * 10f32.powf(db / 20.0);
                    let v = Voice::new(rate, 0.3, Rng::new(seed * 100 + k as u64))
                        .expect("a voice")
                        .with_phase(0.17 * k as f64);
                    (Nco::new(rate, f), v, a)
                })
                .collect(),
            noise: sdrkit_dev_test::Awgn::new(
                f64::from(RADAR_GAIN * RADAR_GAIN),
                snr_db,
                bin_bw,
                rate,
                seed,
            ),
            rate,
            audio: Vec::new(),
            am: Vec::new(),
        }
    }

    /// Feeds `secs` of the band to `rx` in blocks of 0.1 s.
    fn feed(&mut self, rx: &mut dyn Receiver, secs: f64) {
        let block = (self.rate / 10.0) as usize;
        let mut x = vec![Complex32::default(); block];
        let am = sdrkit_algo_sim::modulate::Am::new(0.8).expect("an AM modulator");
        let mut out = Output::default();
        for _ in 0..(secs * 10.0).round() as usize {
            self.radars.generate(&mut x);
            for s in x.iter_mut() {
                *s *= RADAR_GAIN;
            }
            for (nco, voice, a) in &mut self.carriers {
                self.audio.resize(block, 0.0);
                self.am.resize(block, Complex32::default());
                voice.fill(&mut self.audio);
                am.process(&self.audio, &mut self.am).expect("AM");
                for (s, m) in x.iter_mut().zip(&self.am) {
                    *s += *m * nco.next_sample() * *a;
                }
            }
            self.noise.process(&mut x);
            rx.feed(&x, &mut out);
            out.clear();
        }
    }
}

/// A receiver configured for a recording at `rate` with the radio at `centre`.
fn configured(rate: f64, centre: f64) -> StackGraph {
    let mut rx = graph_receiver();
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    rx
}

/// One radar of the test generator.
fn one(offset_hz: f64, width_hz: f64, repeat_s: f64, dir: f64) -> TestRadar {
    TestRadar {
        offset_hz,
        width_hz,
        repeat_s,
        dir,
        level_db: 0.0,
        start_s: 0.1,
        id: None,
    }
}

/// Checks the measured centre, width, rate, repetition and direction of the radar of `rx` nearest to `r` against the
/// generator's values: the sweep lasts 0.9 x the repetition, so the rate is width / (0.9 x repetition).
fn check_measured(rx: &dyn Receiver, centre: f64, r: &TestRadar) {
    let got = radars(rx);
    let g = got
        .iter()
        .min_by(|a, b| {
            (num(a, "offset_hz") - r.offset_hz)
                .abs()
                .total_cmp(&(num(b, "offset_hz") - r.offset_hz).abs())
        })
        .expect("a radar");
    let rate = r.width_hz / (0.9 * r.repeat_s);
    let what = format!("{r:?}");
    assert!(
        (num(g, "centre") - (centre + r.offset_hz)).abs() <= 2e3,
        "centre {} {what}",
        num(g, "centre")
    );
    assert!(
        (num(g, "width") - r.width_hz).abs() <= 0.1 * r.width_hz,
        "width {} {what}",
        num(g, "width")
    );
    assert!(
        (num(g, "rate_hz_s") - rate).abs() <= 0.05 * rate,
        "rate {} {what}",
        num(g, "rate_hz_s")
    );
    assert!(
        (num(g, "repeat") - r.repeat_s).abs() <= 0.01 + 0.02 * r.repeat_s,
        "repeat {} {what}",
        num(g, "repeat")
    );
    let dir = if r.dir > 0.0 { "rising" } else { "falling" };
    assert_eq!(text(g, "direction"), dir, "{what}");
}

#[test]
fn measured_parameters_match_the_generator_for_each_radar() {
    // The three built-in radars at 2 Msps, then two more forms at 1 Msps: a wide fast sweep and a narrow slow one.
    let rx = run(RadarGenOptions::default(), 10.0, 13.0e6);
    show(&rx);
    for r in radar::gen::builtin() {
        check_measured(&rx, 13.0e6, &r);
    }
    let more = [one(-200e3, 150e3, 0.26, -1.0), one(150e3, 20e3, 1.0, 1.0)];
    let o = RadarGenOptions {
        rate: 1e6,
        radars: more.to_vec(),
        ..Default::default()
    };
    let rx = run(o, 12.0, 13.0e6);
    show(&rx);
    assert_eq!(radars(&rx).len(), 2);
    for r in &more {
        check_measured(&rx, 13.0e6, r);
    }
    let got = radars(&rx);
    let wide = got.iter().find(|g| num(g, "offset_hz") < 0.0).unwrap();
    let narrow = got.iter().find(|g| num(g, "offset_hz") > 0.0).unwrap();
    assert!(text(wide, "hint").starts_with("possible HF ocean radar"));
    assert!(text(narrow, "hint").starts_with("possible SeaSonde-like radar"));
}

#[test]
fn the_test_signal_reaches_its_end_state_with_default_settings() {
    // `synth:hfradar` as a session makes it: the stack's generator at the stack's rate with the default settings and the
    // standard noise of the `gen_*` keys.
    let d = &radar::HFRADAR;
    let rate = d.tuning.rate_hz;
    let v = values(d, &[]);
    let g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v.clone()).with_seed(1))
        .unwrap();
    let mut g = sdrkit_components::sim::impaired(g, &v).expect("impairments");
    let mut rx = (d.make_receiver)(&StackConfig::new(v)).unwrap();
    let centre = d.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); (rate / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..120 {
        g.generate(&mut x);
        rx.feed(&x, &mut out);
        out.clear();
    }
    show(rx.as_ref());
    assert_eq!(radars(rx.as_ref()).len(), 3);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert!(field(rx.as_ref(), "sweeps_counted").unwrap() > 12.0);
    assert!(rx.results().status().level_dbfs.is_some());
}

#[test]
fn noise_alone_for_a_minute_gives_no_radar() {
    // 60 s of noise at 250 ksps (about 61 000 spectrum frames), then 8 s on each of four more seeds at 2 Msps.
    let quiet = |rate| RadarGenOptions {
        rate,
        radars: vec![],
        ..Default::default()
    };
    let mut rx = configured(250e3, 13.0e6);
    Band::new(quiet(250e3), 20.0, 11, &[]).feed(&mut rx, 60.0);
    assert!(radars(&rx).is_empty());
    assert_eq!(value(&rx, "sweeps_counted"), Value::I64(0));
    for seed in 12..16 {
        let mut rx = configured(2e6, 13.0e6);
        Band::new(quiet(2e6), 20.0, seed, &[]).feed(&mut rx, 8.0);
        assert!(radars(&rx).is_empty(), "seed {seed}");
        assert_eq!(value(&rx, "sweeps_counted"), Value::I64(0), "seed {seed}");
    }
}

/// Ten AM broadcast carriers with speech, 20 kHz apart, 20 dB to 11 dB over the strongest radar (40 to 31 dB over the
/// noise in one bin at 20 dB SNR). Their sum stays under full scale.
fn broadcast() -> Vec<(f64, f32)> {
    (0..10)
        .map(|k| (-95e3 + 20e3 * k as f64, 20.0 - k as f32))
        .collect()
}

#[test]
fn broadcast_carriers_with_speech_give_no_radar() {
    let quiet = RadarGenOptions {
        rate: 250e3,
        radars: vec![],
        ..Default::default()
    };
    // 30 s on each of five seeds at 250 ksps, and 10 s at 2 Msps with the carriers spread over 1.8 MHz.
    let mut stray = Vec::new();
    for seed in 21..26 {
        let mut rx = configured(250e3, 13.7e6);
        Band::new(quiet.clone(), 20.0, seed, &broadcast()).feed(&mut rx, 30.0);
        show(&rx);
        assert!(radars(&rx).is_empty(), "seed {seed}");
        stray.push(field(&rx, "sweeps_counted").unwrap_or(0.0));
    }
    let wide = RadarGenOptions { rate: 2e6, ..quiet };
    let spread: Vec<(f64, f32)> = (0..10)
        .map(|k| (-800e3 + 170e3 * k as f64, 20.0 - k as f32))
        .collect();
    let mut rx = configured(2e6, 13.7e6);
    Band::new(wide, 20.0, 26, &spread).feed(&mut rx, 10.0);
    assert!(radars(&rx).is_empty());
    stray.push(field(&rx, "sweeps_counted").unwrap_or(0.0));
    eprintln!("stray sweeps counted: {stray:?}");
}

#[test]
fn strong_and_clean_radars_are_measured_whole() {
    // The mask must not take a strong sweep for a steady signal: the built-in radars at 40 and 60 dB, and one radar
    // without noise.
    for snr in [40.0, 60.0] {
        let mut rx = configured(2e6, 13.0e6);
        Band::new(RadarGenOptions::default(), snr, 61, &[]).feed(&mut rx, 6.0);
        show(&rx);
        assert_eq!(radars(&rx).len(), 3, "{snr} dB");
        for r in radar::gen::builtin() {
            check_measured(&rx, 13.0e6, &r);
        }
    }
    let r = one(20e3, 25e3, 0.5, 1.0);
    let mut g = RadarGenerator::new(RadarGenOptions {
        rate: 250e3,
        radars: vec![r.clone()],
        ..Default::default()
    });
    let mut rx = configured(250e3, 13.0e6);
    let x = generate(&mut g, 6.0);
    feed(&mut rx, &x, 25_000);
    show(&rx);
    assert_eq!(radars(&rx).len(), 1);
    check_measured(&rx, 13.0e6, &r);
}

#[test]
fn a_radar_under_a_strong_broadcast_carrier_is_measured() {
    // A carrier 20 dB over the radar at the middle of its sweep, and eight more carriers beside the sweep.
    let r = one(-70e3, 50e3, 0.5, 1.0);
    let o = RadarGenOptions {
        rate: 250e3,
        radars: vec![r.clone()],
        ..Default::default()
    };
    let mut carriers = vec![(-70e3, 20.0)];
    carriers.extend((0..8).map(|k| (-15e3 + 15e3 * k as f64, 19.0 - k as f32)));
    let mut rx = configured(250e3, 13.0e6);
    Band::new(o, 20.0, 31, &carriers).feed(&mut rx, 10.0);
    show(&rx);
    assert_eq!(radars(&rx).len(), 1);
    assert!(num(&radars(&rx)[0], "sweeps") >= 17.0);
    check_measured(&rx, 13.0e6, &r);
    // The carriers alone for 40 s, then the radar: a track left in the masked bins must not take the sweeps.
    let late = TestRadar {
        start_s: 40.0,
        ..r.clone()
    };
    let o = RadarGenOptions {
        rate: 250e3,
        radars: vec![late],
        ..Default::default()
    };
    let mut rx = configured(250e3, 13.0e6);
    Band::new(o, 20.0, 32, &carriers).feed(&mut rx, 50.0);
    show(&rx);
    assert_eq!(radars(&rx).len(), 1);
    assert!(num(&radars(&rx)[0], "sweeps") >= 17.0);
    check_measured(&rx, 13.0e6, &r);
}

#[test]
fn two_radars_of_one_form_are_listed_apart() {
    let pair = [one(-50e3, 40e3, 0.5, 1.0), one(50e3, 40e3, 0.5, 1.0)];
    let o = RadarGenOptions {
        rate: 250e3,
        radars: pair.to_vec(),
        ..Default::default()
    };
    let mut rx = configured(250e3, 13.0e6);
    Band::new(o, 20.0, 41, &[]).feed(&mut rx, 8.0);
    show(&rx);
    assert_eq!(radars(&rx).len(), 2);
    for r in &pair {
        check_measured(&rx, 13.0e6, r);
    }
}

#[test]
fn detection_rate_against_snr() {
    // One 50 kHz radar every 0.5 s at 250 ksps, 6 s on each of ten seeds. Detected: exactly one radar listed, with its
    // repetition within 0.03 s. The SNR is that of the radar in one bin of the 1 ms FFT.
    let r = one(20e3, 50e3, 0.5, 1.0);
    let mut rates = Vec::new();
    for snr in [20.0, 11.0, 8.0, 7.0, 6.0] {
        let mut hits = 0;
        for seed in 1..=10 {
            let o = RadarGenOptions {
                rate: 250e3,
                radars: vec![r.clone()],
                ..Default::default()
            };
            let mut rx = configured(250e3, 13.0e6);
            Band::new(o, snr, 50 + seed, &[]).feed(&mut rx, 6.0);
            let got = radars(&rx);
            if got.len() == 1 && (num(&got[0], "repeat") - 0.5).abs() <= 0.03 {
                hits += 1;
            }
        }
        eprintln!("{snr} dB: {hits} of 10");
        rates.push((snr, hits));
    }
    assert!(
        rates.iter().filter(|r| r.0 >= 8.0).all(|r| r.1 == 10),
        "{rates:?}"
    );
}

#[test]
fn band_tune_asks_for_the_middle_of_a_band() {
    let mut rx = configured(2e6, 13.0e6);
    let bands = rows(rx.results(), &HFRADAR_BANDS);
    assert_eq!(bands.len(), 5);
    let b = bands
        .iter()
        .find(|b| text(b, "name") == "12 - 14 MHz")
        .unwrap();
    assert_eq!(num(b, "middle"), 13.0e6);
    assert_eq!(text(b, "range_class"), "medium range");
    invoke(&mut rx, "band.tune", Some("12 - 14 MHz"));
    let requests = feed_requests(&mut rx, &vec![Default::default(); 200_000], 200_000);
    assert!(requests.iter().any(|q| matches!(
        q,
        HostRequest::Retune { tuned_hz, .. } if *tuned_hz == 13.0e6
    )));
    let id = rx.commands().id("band.tune").unwrap();
    let mut args = rx.commands().defs()[id.index()].new_args();
    args.set(
        &sdrkit_core::publish::Field::<sdrkit_core::publish::field::Text>::new("row", "Row", ""),
        "7 MHz",
    );
    assert!(sdrkit_core::owner::invoke(&mut rx, id, &args).is_err());
    // While the scan is on, the scan owns the frequency: a band is refused.
    let mut live = graph_receiver();
    live.set_context(RunContext {
        live: true,
        ..Default::default()
    });
    set(&mut live, "scan", json!(true)).unwrap();
    let mut info = InputInfo::new(2e6, 13e6, 13e6);
    info.live = true;
    live.configure(&info).unwrap();
    feed_requests(&mut live, &vec![Default::default(); 200_000], 200_000);
    args.set(
        &sdrkit_core::publish::Field::<sdrkit_core::publish::field::Text>::new("row", "Row", ""),
        "24.45 - 24.65 MHz",
    );
    assert!(sdrkit_core::owner::invoke(&mut live, id, &args).is_err());
    // The bands stay over Clear.
    invoke(&mut rx, "clear", None);
    assert_eq!(rows(rx.results(), &HFRADAR_BANDS).len(), 5);
}

#[test]
fn processor_share_at_2_msps() {
    let b = sdrkit_stack_hfdata::benchmarks()
        .into_iter()
        .find(|b| b.name == "hfradar.receiver_2m")
        .expect("the bench entry");
    let r = (b.run)(1.0);
    eprintln!(
        "hfradar at 2 Msps: {:.1} % of one core",
        100.0 * r.cpu_fraction_of_realtime
    );
    if !cfg!(debug_assertions) {
        assert!(r.cpu_fraction_of_realtime < 0.25, "{r:?}");
    }
}
