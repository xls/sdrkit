//! FreeDV 700D and 700E: the demodulator component on the worked frames and on the test signals at 8 kHz (clean, in white
//! noise at stated SNRs, with a frequency offset, clipped, and when the signal ends), and the stack on its test signals.

use crate::common::{calls, desc, show, values, view};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, StackConfig};
use serde_json::Value as Json;
use sdrkit_stack_dvoice::freedv_ofdm::{
    render, test_frame_bits, Modulator, OfdmDemod, RenderOptions, SyncState, UpperSideband, Waveform,
    RATE, S, W700D, W700E,
};

/// The test signal of `w`: `seconds` of frames, unit power.
fn signal(w: &'static Waveform, seconds: f64, clip: bool) -> Vec<Complex32> {
    render(w, &RenderOptions { text: "N0CALL", seconds, clip })
}

/// `x` moved by `offset_hz`, in white noise for `snr_db` in 3000 Hz against a signal of unit power (`None`: no noise), through
/// the sideband filter.
fn channel(x: &[Complex32], offset_hz: f64, snr_db: Option<f64>, seed: u64) -> Vec<Complex32> {
    let mut rng = Rng::new(seed);
    // Complex noise of power N over 8000 Hz has N × 3000 / 8000 in 3000 Hz.
    let sigma = snr_db.map_or(0.0, |s| (8000.0 / 3000.0 / 10f64.powf(s / 10.0) / 2.0).sqrt() as f32);
    let mut y = Vec::with_capacity(x.len());
    for (n, s) in x.iter().enumerate() {
        let turn = (offset_hz * n as f64 / RATE).fract() as f32 * std::f32::consts::TAU;
        let noise = Complex32::new(rng.normal_f32(), rng.normal_f32()) * sigma;
        y.push(s * Complex32::from_polar(1.0, turn) + noise);
    }
    let mut out = Vec::with_capacity(y.len());
    UpperSideband::default().process(&y, &mut out);
    out
}

/// What a demodulator did on a signal.
#[derive(Debug, Default)]
struct Run {
    /// Frames with sync.
    frames: usize,
    /// Frames with sync whose data bits are the test-frame bits.
    good: usize,
    /// Frames with sync whose LDPC decoder satisfied every check.
    ldpc_ok: usize,
    /// Input seconds before the first frame in the synced state.
    synced_s: Option<f64>,
    /// Frames in the synced state.
    synced: usize,
    text: Vec<u8>,
    last: Option<SyncState>,
    snr_db: f32,
    freq_hz: f32,
    uw_errors: usize,
}

fn run_on(d: &mut OfdmDemod, x: &[Complex32]) -> Run {
    let w = d.waveform();
    let sent = test_frame_bits(w.k());
    let mut r = Run::default();
    let mut vari = sdrkit_util_text::varicode::psk::Decoder::new();
    let mut at = 0usize;
    for piece in x.chunks(400) {
        d.feed(piece);
        while let Some(step) = d.step() {
            at += step.samples;
            r.last = Some(step.sync);
            if !step.frame {
                continue;
            }
            let f = d.frame();
            r.frames += 1;
            r.good += (f.data == sent) as usize;
            r.ldpc_ok += f.ldpc_ok as usize;
            r.uw_errors += f.uw_errors;
            if step.sync == SyncState::Synced {
                r.synced += 1;
                r.synced_s.get_or_insert(at as f64 / RATE);
                r.snr_db = d.snr_db();
                r.freq_hz = d.freq_offset_hz();
            }
            for &b in &f.text {
                if let Some(Ok(c)) = vari.push(b) {
                    r.text.push(c);
                }
            }
        }
    }
    r
}

fn run(w: &'static Waveform, x: &[Complex32]) -> Run {
    run_on(&mut OfdmDemod::new(w), x)
}

/// The amplitude of `x[from..from + n]` at `hz` (a single-bin transform).
fn bin(x: &[Complex32], from: usize, n: usize, hz: f64) -> f32 {
    let mut acc = Complex32::new(0.0, 0.0);
    for (i, s) in x[from..from + n].iter().enumerate() {
        let turn = (hz * i as f64 / RATE).fract() as f32 * std::f32::consts::TAU;
        acc += s * Complex32::from_polar(1.0, -turn);
    }
    acc.norm()
}

#[test]
fn the_carriers_are_where_the_specification_puts_them() {
    // 700D: nineteen pilot carriers from 1000.0 Hz to 2000.0 Hz, each of amplitude 1/M over the M body samples.
    let m = Modulator::new(&W700D);
    let mut x = Vec::new();
    m.synthesize(&m.frame_symbols(&test_frame_bits(112), &[1, 0, 1, 1]), &mut x);
    assert_eq!(x.len(), 1280);
    for c in 0..19 {
        let hz = 1000.0 + c as f64 * 8000.0 / 144.0;
        assert!((W700D.carrier_hz(c) - hz).abs() < 1e-9);
        assert!((bin(&x, 16, 144, hz) - 1.0).abs() < 1e-3, "pilot carrier {c}");
    }
    assert!(bin(&x, 16, 144, 1000.0 - 8000.0 / 144.0) < 1e-3);
    assert!(bin(&x, 16, 144, 2000.0 + 8000.0 / 144.0) < 1e-3);
    // The data rows leave the edge carriers empty, and the prefix repeats the end of the body.
    assert!(bin(&x, S + 16, 144, 1000.0) < 1e-3 && bin(&x, S + 16, 144, 2000.0) < 1e-3);
    assert!((bin(&x, S + 16, 144, W700D.carrier_hz(1)) - 1.0).abs() < 1e-3);
    assert!((0..16).all(|i| (x[i] - x[144 + i]).norm() < 1e-6));
    // 700E: twenty-one carriers from 785.7 Hz to 2214.3 Hz, nothing at the two edge positions.
    let m = Modulator::new(&W700E);
    let mut x = Vec::new();
    m.synthesize(&m.frame_symbols(&test_frame_bits(56), &[1, 0]), &mut x);
    assert_eq!(x.len(), 640);
    assert!((W700E.carrier_hz(1) - 785.714).abs() < 1e-3 && (W700E.carrier_hz(21) - 2214.286).abs() < 1e-3);
    for row in 0..4 {
        for c in 0..23 {
            let a = bin(&x, row * S + 48, 112, W700E.carrier_hz(c));
            let expect = if c == 0 || c == 22 { 0.0 } else { 1.0 };
            assert!((a - expect).abs() < 1e-3, "row {row} carrier {c}: {a}");
        }
    }
    assert!((0..48).all(|i| (x[i] - x[112 + i]).norm() < 1e-6));
}

#[test]
fn the_demodulator_returns_the_bits_of_the_worked_frames() {
    for (w, text) in [(&W700D, &[1u8, 0, 1, 1][..]), (&W700E, &[1, 0][..])] {
        let m = Modulator::new(w);
        let data = test_frame_bits(w.k());
        let mut x = vec![Complex32::new(0.0, 0.0); 1000];
        for _ in 0..12 {
            m.synthesize(&m.frame_symbols(&data, text), &mut x);
        }
        x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 4 * w.frame()));
        let mut d = OfdmDemod::new(w);
        let mut good = 0;
        for piece in channel(&x, 0.0, None, 0).chunks(333) {
            d.feed(piece);
            while let Some(step) = d.step() {
                let f = d.frame();
                if step.frame && f.uw_errors == 0 {
                    assert!(f.ldpc_ok && f.corrected == 0, "{}", w.name);
                    assert_eq!((&f.data[..], &f.text[..]), (&data[..], text), "{}", w.name);
                    good += 1;
                }
            }
        }
        assert_eq!(good, 12, "{}", w.name);
    }
}

#[test]
fn the_test_signals_lock_and_decode_every_frame() {
    for (w, by_s) in [(&W700D, 1.3), (&W700E, 0.7)] {
        for clip in [false, true] {
            let x = signal(w, 10.0, clip);
            let sent = (10.0 / w.frame_s()).round() as usize;
            let r = run(w, &channel(&x, 0.0, None, 0));
            assert_eq!(r.good, sent, "{} clip {clip}: {r:?}", w.name);
            assert!(r.synced_s.is_some_and(|t| t < by_s), "{} clip {clip}: {r:?}", w.name);
            let text = String::from_utf8_lossy(&r.text).into_owned();
            assert!(text.starts_with("N0CALL\rN0CALL\rN0CALL\r"), "{}: {text:?}", w.name);
        }
    }
}

/// The frames of `seeds` runs of 10 s that decode to the sent bits, of the frames sent, at `snr_db` in 3000 Hz.
fn frame_success(w: &'static Waveform, snr_db: f64, seeds: std::ops::RangeInclusive<u64>) -> (usize, usize) {
    let x = signal(w, 10.0, false);
    let sent = (10.0 / w.frame_s()).round() as usize;
    let mut good = 0;
    let mut all = 0;
    for seed in seeds {
        good += run(w, &channel(&x, 0.0, Some(snr_db), seed)).good;
        all += sent;
    }
    (good, all)
}

/// Measured over the seeds 1 to 10 (10 s each, white noise, SNR in 3000 Hz): 700D decodes 85.7 % of its frames at −2 dB and
/// 98.9 % at 3 dB; 700E decodes 91.0 % at +1 dB and 97.9 % at 3 dB. The frames before sync count as lost.
#[test]
fn frame_success_in_white_noise_at_the_published_threshold_and_at_3_db() {
    for (w, snr_db, least) in [(&W700D, -2.0, 0.80), (&W700D, 3.0, 0.97), (&W700E, 1.0, 0.86), (&W700E, 3.0, 0.96)] {
        let (good, sent) = frame_success(w, snr_db, 1..=10);
        let rate = good as f64 / sent as f64;
        println!("{} at {snr_db} dB: {good} of {sent} frames ({:.1} %)", w.name, 100.0 * rate);
        assert!(rate >= least, "{} at {snr_db} dB: {good} of {sent}", w.name);
    }
}

#[test]
fn offsets_across_the_acquisition_range_sync_within_3_s_and_are_measured_to_2_hz() {
    for w in [&W700D, &W700E] {
        let x = signal(w, 8.0, false);
        for offset in [-60.0, -55.0, -20.0, 0.0, 20.0, 55.0, 59.0] {
            // 10 dB in the 4000 Hz audio band is 11.2 dB in 3000 Hz.
            let y = channel(&x, offset, Some(11.2), 3);
            let mut d = OfdmDemod::new(w);
            let r = run_on(&mut d, &y[..(5.3 * RATE) as usize]);
            assert!(r.synced_s.is_some_and(|t| t < 3.3), "{} {offset} Hz: {r:?}", w.name);
            let got = f64::from(d.freq_offset_hz());
            assert!((got - offset).abs() < 2.0, "{} {offset} Hz: {got}", w.name);
            assert!(r.good as f64 > 0.8 * 5.0 / w.frame_s(), "{} {offset} Hz: {r:?}", w.name);
        }
    }
}

#[test]
fn sync_is_lost_when_the_signal_ends() {
    for (w, within_s) in [(&W700D, 2.0), (&W700E, 1.5)] {
        for noise in [None, Some(6.0)] {
            let mut x = signal(w, 5.0, false);
            x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 6 * 8000));
            let mut d = OfdmDemod::new(w);
            let (mut at, mut synced, mut lost) = (0, false, None);
            for piece in channel(&x, 0.0, noise, 5).chunks(400) {
                d.feed(piece);
                while let Some(step) = d.step() {
                    at += step.samples;
                    synced |= step.sync == SyncState::Synced;
                    if synced && step.sync == SyncState::Search && lost.is_none() {
                        lost = Some(at as f64 / RATE);
                    }
                }
            }
            // The frames end 5.3 s into the signal.
            assert!(
                lost.is_some_and(|t| t > 5.3 && t < 5.3 + within_s),
                "{} noise {noise:?}: {lost:?}",
                w.name
            );
            assert_eq!(d.snr_db(), -5.0);
        }
    }
}

#[test]
fn a_sample_clock_100_ppm_off_keeps_sync_and_is_measured() {
    for w in [&W700D, &W700E] {
        for ppm in [100.0, -100.0] {
            let x = signal(w, 30.0, false);
            let y = channel(&stretch(&x, ppm), 0.0, Some(11.2), 4);
            let mut d = OfdmDemod::new(w);
            let r = run_on(&mut d, &y[..29 * 8000]);
            assert_eq!(r.last, Some(SyncState::Synced), "{} {ppm} ppm: {r:?}", w.name);
            assert!(r.good as f64 > 0.95 * 28.0 / w.frame_s(), "{} {ppm} ppm: {r:?}", w.name);
            let got = f64::from(d.clock_offset_ppm());
            assert!((got - ppm).abs() < 30.0, "{} {ppm} ppm: {got}", w.name);
        }
    }
}

/// `x` made longer by `ppm` parts per million (cubic interpolation): each frame arrives that much later.
fn stretch(x: &[Complex32], ppm: f64) -> Vec<Complex32> {
    let step = 1.0 - ppm * 1e-6;
    let mut out = Vec::with_capacity(x.len());
    let mut t = 1.0f64;
    while (t as usize) + 2 < x.len() {
        let i = t as usize;
        let u = (t - i as f64) as f32;
        let (a, b, c, d) = (x[i - 1], x[i], x[i + 1], x[i + 2]);
        out.push(
            a * (-u * (u - 1.0) * (u - 2.0) / 6.0)
                + b * ((u + 1.0) * (u - 1.0) * (u - 2.0) / 2.0)
                + c * (-(u + 1.0) * u * (u - 2.0) / 2.0)
                + d * ((u + 1.0) * u * (u - 1.0) / 6.0),
        );
        t += step;
    }
    out
}

/// The test signal of the generator in `submode` at 96 ksps, `offset_hz` off, in noise at `snr_db` in 3000 Hz: one pass.
fn stack_signal(submode: &str, offset_hz: f64, snr_db: f64) -> Vec<Complex32> {
    sdrkit_dev_test::Harness::new(desc("freedv"))
        .submode(submode)
        .rate(96_000.0)
        .seed(7)
        .seconds(10.8)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .generate()
        .expect("the test signal")
}

/// The results of the receiver in `submode` (none: the stack's default) after `x`.
fn stack_run(submode: Option<&str>, x: &[Complex32]) -> Json {
    let mut cfg = StackConfig::new(values("freedv", &[]));
    cfg.submode = submode.map(str::to_owned);
    let mut rx = (desc("freedv").make_receiver)(&cfg).expect("the receiver builds");
    rx.configure(&InputInfo::new(96_000.0, 0.0, 0.0).recording())
        .expect("configure");
    let mut out = Output::default();
    for c in x.chunks(4096) {
        rx.feed(c, &mut out);
        out.clear();
    }
    view(Owner::results(rx.as_ref()))
}

#[test]
fn the_stack_locks_and_decodes_its_700d_and_700e_test_signals() {
    for (sub, sync) in [("700D", "sync_700d"), ("700E", "sync_700e")] {
        // 10 dB in 3000 Hz, 20 Hz off; the results are read while the signal is on.
        let x = stack_signal(sub, 20.0, 10.0);
        let v = stack_run(Some(sub), &x[..9 * 96_000]);
        let (f, c) = (&v["fields"], calls(&v));
        assert_eq!(v["status"]["lock_state"], "locked", "{sub}: {}", v["status"]);
        assert_eq!(f["verdict"], "decoding", "{sub}");
        assert_eq!((&f["submode"], &f[sync], &f["confirmed"]), (&sub.into(), &"synced".into(), &true.into()));
        assert_eq!(f["sync_1600"], "", "{sub}");
        assert!(c.len() == 1 && c[0]["src"] == "N0CALL" && c[0]["text"] == "N0CALL", "{sub}: {}", show(&c));
        assert!(c[0]["voice_frames"].as_u64().unwrap() >= 150, "{sub}: {}", show(&c));
        assert!(f["speech_frames"].as_u64().unwrap() >= 150 && f["ldpc_failed_frames"] == 0, "{sub}: {f}");
        assert!(f["blocks_ok"].as_u64().unwrap() >= 45 && f["blocks_bad"].as_u64().unwrap() <= 1, "{sub}: {f}");
        assert!((f["freq_offset_hz"].as_f64().unwrap() - 20.0).abs() < 2.0, "{sub}: {f}");
        assert!((f["offset_hz"].as_f64().unwrap() - 20.0).abs() < 2.0, "{sub}: {f}");
        assert!((f["snr_db"].as_f64().unwrap() - 10.0).abs() < 3.0, "{sub}: {f}");
        assert!(f["uw_ber_pct"].as_f64().unwrap() < 1.0 && f["timing_metric"].as_f64().unwrap() > 0.3, "{sub}: {f}");
        assert!("N0CALL".starts_with(f["callsign"].as_str().unwrap()), "{sub}: {f}");
    }
}

#[test]
fn the_automatic_submode_takes_the_waveform_that_is_on_the_channel() {
    for sub in ["700D", "700E", "1600"] {
        let x = stack_signal(sub, 5.0, 12.0);
        let v = stack_run(None, &x[..10 * 96_000]);
        let (f, c) = (&v["fields"], calls(&v));
        assert_eq!(f["submode"], sub, "{f}");
        assert!(c.len() == 1 && c[0]["src"] == "N0CALL", "{sub}: {}", show(&c));
        assert!(f["speech_frames"].as_u64().unwrap() >= 80, "{sub}: {f}");
    }
}

/// The stack gives speech only from a waveform that held sync at 3 dB or more for 1.5 s. At 1 dB the frames of both
/// waveforms decode and the stack stays silent; at 5 dB it gives speech.
#[test]
fn the_stack_is_silent_at_1_db_and_gives_speech_at_5_db() {
    for (sub, sync) in [("700D", "sync_700d"), ("700E", "sync_700e")] {
        let v = stack_run(Some(sub), &stack_signal(sub, 0.0, 1.0)[..10 * 96_000]);
        let f = &v["fields"];
        assert_eq!((&f[sync], &f["confirmed"], &f["speech_frames"]), (&"synced".into(), &false.into(), &0.into()), "{f}");
        assert!(f["blocks_ok"].as_u64().unwrap() >= 50 && calls(&v).is_empty(), "{sub}: {f}");
        let v = stack_run(Some(sub), &stack_signal(sub, 0.0, 5.0)[..10 * 96_000]);
        assert!(v["fields"]["speech_frames"].as_u64().unwrap() >= 150, "{sub}: {}", v["fields"]);
    }
}

#[test]
fn noise_and_a_tone_give_no_speech_in_the_automatic_submode() {
    let mut rng = Rng::new(9);
    let noise: Vec<Complex32> = (0..20 * 96_000)
        .map(|_| Complex32::new(rng.normal_f32(), rng.normal_f32()) * 0.1)
        .collect();
    let tone: Vec<Complex32> = (0..20 * 96_000)
        .map(|n| Complex32::from_polar(0.5, std::f32::consts::TAU * (n % 96) as f32 / 96.0))
        .collect();
    for x in [noise, tone] {
        let v = stack_run(None, &x);
        let f = &v["fields"];
        assert_eq!((&f["confirmed"], &f["speech_frames"], &f["submode"]), (&false.into(), &0.into(), &"".into()), "{f}");
        assert!(calls(&v).is_empty());
    }
}

#[test]
fn the_call_ends_when_the_signal_ends_and_rade_is_not_received() {
    let x = stack_signal("700D", 0.0, 15.0);
    let mut longer = x.clone();
    longer.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 5 * 96_000));
    let v = stack_run(Some("700D"), &longer);
    let (f, c) = (&v["fields"], calls(&v));
    assert_eq!(f["submode"], "", "{f}");
    assert!(c.len() == 1 && c[0]["end_s"].as_f64().unwrap() < 13.5, "{}", show(&c));
    assert_eq!(v["status"]["lock_state"], "searching");
    let v = stack_run(Some("rade"), &x);
    let f = &v["fields"];
    assert_eq!((&f["sync_700d"], &f["sync_700e"], &f["sync_1600"]), (&"".into(), &"".into(), &"".into()), "{f}");
    assert!(calls(&v).is_empty() && f["freedv_frames"] == 0, "{f}");
}

#[test]
fn the_freedv_benchmarks_run_faster_than_real_time() {
    for b in sdrkit_stack_dvoice::verify::freedv_benchmarks() {
        let r = (b.run)(0.5);
        println!("{}: {:.2} % of one core in real time", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{}", b.name);
    }
}

#[test]
fn the_verification_cases_of_700d_and_700e_pass_their_checks() {
    let d = &sdrkit_stack_dvoice::FREEDV;
    let o = sdrkit_core::verify::VerifyOptions { seconds: 12.0, clean: false, snr_db: 30.0 };
    let cases = (d.verify.expect("the hooks").cases)(&o);
    assert_eq!(cases.len(), 3);
    for case in &cases[1..] {
        let run = sdrkit_dev_test::Harness::new(d)
            .submode(case.submode.as_deref().expect("a submode"))
            .rate(96_000.0)
            .seconds(case.seconds)
            .set(&GEN_OFFSET_HZ, case.offset_hz)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, case.snr_db.unwrap_or(30.0))
            .run()
            .expect("the case runs");
        for line in (case.check.expect("a check"))(&run.results) {
            assert_eq!(line.outcome, sdrkit_core::verify::Outcome::Pass, "{}: {}: {}", case.name, line.key, line.detail);
        }
    }
}

