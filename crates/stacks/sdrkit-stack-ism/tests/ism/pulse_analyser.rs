//! `ism`, submode `pulse_analyser`: the three test patterns through the standard impairments and the receiver at 250 ksps,
//! the noise sweep of the PWM frames, noise alone and a plain carrier, the result fields and the status, and the verify case.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::owner::Owner;
use sdrkit_core::verify::{VerifyHooks, VerifyOptions};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig, Values};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_proto_ookdev::guess::LineCode;
use sdrkit_stack_ism::ism::analyser::{PulseFrame, Submode, SUBMODE};
use sdrkit_stack_ism::ism::devices::Devices;
use sdrkit_stack_ism::ism::pulsegen::{
    GenPattern, PulseParams, GEN_FRAMES, GEN_FRAME_GAP_S, GEN_JITTER, GEN_PATTERN,
};
use sdrkit_stack_ism::ism::{self, ISM};
use sdrkit_stack_ism::typed::{IsmPulseFields, PulseCode};

const RATE: f64 = 250e3;

/// A harness of the pulse analyser at 250 ksps: `pattern`, `frames` frames, CNR `cnr_db` in 100 kHz, carrier offset
/// `offset_hz`, jitter share `jitter`, a gap of `gap_s` after each frame, the seed `seed`, and 1.5 s of signal (two groups, at
/// 0.2 s and 1.2 s).
fn harness(
    pattern: GenPattern,
    frames: i64,
    cnr_db: f64,
    offset_hz: f64,
    jitter: f64,
    gap_s: f64,
    seed: u64,
) -> Harness {
    Harness::new(&ISM)
        .rate(RATE)
        .seconds(1.5)
        .seed(seed)
        .set(&SUBMODE, Submode::PulseAnalyser)
        .set(&GEN_PATTERN, pattern)
        .set(&GEN_FRAMES, frames)
        .set(&GEN_JITTER, jitter)
        .set(&GEN_FRAME_GAP_S, gap_s)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr_db)
}

/// A receiver of the settings `v` at 250 ksps.
fn rx(v: Values) -> StackGraph {
    let mut r = ism::receiver(&StackConfig::new(v)).expect("the receiver builds");
    r.configure(&InputInfo::new(RATE, 433.92e6, 433.92e6).recording())
        .expect("250 ksps is accepted in the submode pulse_analyser");
    r
}

/// Feeds `x` to `r` in pieces of 10 ms.
fn feed(r: &mut StackGraph, x: &[Complex32]) {
    let mut out = Output::default();
    for c in x.chunks((RATE / 100.0) as usize) {
        r.feed(c, &mut out);
        out.clear();
    }
}

/// The frames with a line code that the receiver logged, and the count of all frames it read.
fn frames(r: &StackGraph) -> (Vec<PulseFrame>, u64) {
    let s = r
        .graph()
        .component_as::<Devices>("devices")
        .expect("the devices component")
        .store();
    (s.pulse_frames.iter().cloned().collect(), s.frames_seen)
}

/// The frames of one run of `h`, and the pattern the generator sent.
fn run(h: &Harness) -> (Vec<PulseFrame>, u64, PulseParams) {
    let v = h.values().unwrap();
    let sent = PulseParams::of(&v);
    let x = h.generate().unwrap();
    let mut r = rx(v);
    feed(&mut r, &x);
    let (f, seen) = frames(&r);
    (f, seen, sent)
}

/// Every frame of the 7.7 cases: the right code and bits (the first `bits` sent), at three offsets, without and with 3 % jitter,
/// over two seeds; at least `min` such frames, and none other.
fn check_pattern(
    pattern: GenPattern,
    frames_sent: i64,
    gap_s: f64,
    code: LineCode,
    bits: usize,
    min: usize,
) {
    for offset in [0.0, 12e3, -30e3] {
        for jitter in [0.0, 0.03] {
            for seed in [1, 2] {
                let (f, seen, sent) = run(&harness(
                    pattern,
                    frames_sent,
                    25.0,
                    offset,
                    jitter,
                    gap_s,
                    seed,
                ));
                let at = format!("{pattern:?} offset {offset} jitter {jitter} seed {seed}");
                assert!(f.len() >= min, "{at}: {} frames of {seen}", f.len());
                for x in &f {
                    assert_eq!(x.code, code, "{at}: {x:?}");
                    assert!(x.bits.len() >= bits, "{at}: {x:?}");
                    assert_eq!(&x.bits[..bits], &sent.bits[..bits], "{at}: {}", x.bin());
                }
                for x in &f {
                    // The timings within the tolerances of 7.7: 40 µs for a short and 90 µs for a long value.
                    let near = |got: Option<f32>, want: f32, tol: f32| {
                        got.is_some_and(|g| (g - want).abs() < tol)
                    };
                    let ok = match pattern {
                        GenPattern::Pwm => {
                            near(x.short_us, sent.t_us, 40.0)
                                && near(x.long_us, 3.0 * sent.t_us, 90.0)
                        }
                        GenPattern::Ppm => {
                            near(x.short_us, sent.gap0_us, 40.0)
                                && near(x.long_us, sent.gap1_us, 90.0)
                                && near(x.pulse_us, sent.pulse_us, 40.0)
                        }
                        GenPattern::Manchester => near(x.unit_us, sent.t_us, 40.0),
                    };
                    assert!(ok, "{at}: {x:?}");
                }
                if pattern == GenPattern::Pwm {
                    assert_eq!(f[0].bits.len(), 24, "{at}");
                    let most = f.iter().map(|x| x.repeats).max().unwrap();
                    assert!(most >= 3, "{at}: repeats {most}");
                }
            }
        }
    }
}

#[test]
fn pwm_frames_read_with_their_bits_timings_and_repeats() {
    check_pattern(GenPattern::Pwm, 4, 0.0, LineCode::Pwm, 24, 3);
}

#[test]
fn ppm_frames_read_with_their_bits() {
    // A 20 ms gap after each frame and 15 ms between frames.
    check_pattern(GenPattern::Ppm, 3, 0.015, LineCode::Ppm, 16, 2);
}

#[test]
fn manchester_frames_read_with_their_first_19_bits() {
    check_pattern(GenPattern::Manchester, 3, 0.0, LineCode::Manchester, 19, 2);
}

/// The PWM frames without jitter, CNR in 100 kHz from 15 dB up, three seeds: at least 3 frames with the right bits, none with
/// wrong ones. Below, the counts are printed.
#[test]
fn pwm_noise_sweep() {
    for cnr in [30.0, 25.0, 20.0, 15.0, 12.0, 9.0] {
        for seed in [1, 2, 3] {
            let (f, seen, sent) = run(&harness(GenPattern::Pwm, 4, cnr, 0.0, 0.0, 0.0, seed));
            let good = f
                .iter()
                .filter(|x| x.code == LineCode::Pwm && x.bits == sent.bits)
                .count();
            let wrong = f.len() - good;
            println!("CNR {cnr} dB seed {seed}: {good} right, {wrong} wrong, {seen} frames read");
            if cnr >= 15.0 {
                assert!(
                    good >= 3 && wrong == 0,
                    "CNR {cnr} seed {seed}: {good} right, {wrong} wrong"
                );
            }
        }
    }
}

/// Complex noise of total power `power` for `seconds`, plus a carrier of amplitude `carrier` at `carrier_hz`.
fn noise_and_carrier(
    seconds: f64,
    power: f64,
    carrier: f32,
    carrier_hz: f64,
    seed: u64,
) -> Vec<Complex32> {
    let mut rng = Rng::new(seed);
    let w = std::f64::consts::TAU * carrier_hz / RATE;
    (0..(seconds * RATE) as usize)
        .map(|k| {
            Complex32::from_polar(
                carrier,
                (w * k as f64).rem_euclid(std::f64::consts::TAU) as f32,
            ) + rng.complex_normal(power)
        })
        .collect()
}

/// 20 s of noise alone and 10 s of a carrier at +5 kHz 30 dB over the noise in 100 kHz, over three seeds: no frame.
#[test]
fn noise_alone_and_a_plain_carrier_show_no_frame() {
    let v = harness(GenPattern::Pwm, 4, 25.0, 0.0, 0.0, 0.0, 1)
        .values()
        .unwrap();
    for seed in [11, 12, 13] {
        let mut r = rx(v.clone());
        feed(&mut r, &noise_and_carrier(20.0, 0.01, 0.0, 0.0, seed));
        let (f, seen) = frames(&r);
        assert!(
            f.is_empty() && seen == 0,
            "noise, seed {seed}: {} frames with a code, {seen} read",
            f.len()
        );
        // CNR 30 dB in 100 kHz: the noise of the whole band is 2.5 x 1e-3 of the carrier power.
        let mut r = rx(v.clone());
        feed(
            &mut r,
            &noise_and_carrier(10.0, RATE / 100e3 * 1e-3, 1.0, 5e3, seed),
        );
        let (f, seen) = frames(&r);
        assert!(
            f.is_empty() && seen == 0,
            "carrier, seed {seed}: {} frames with a code, {seen} read",
            f.len()
        );
    }
}

/// The result fields hold the last frame with a line code, and the status says that the stack measures and guesses.
#[test]
fn the_fields_and_the_status_of_the_last_frame() {
    let h = harness(GenPattern::Pwm, 4, 25.0, 0.0, 0.0, 0.0, 1);
    let v = h.values().unwrap();
    let mut r = rx(v);
    feed(&mut r, &h.generate().unwrap());
    let res = r.results();
    let f = IsmPulseFields::read(res.fields()).expect("the fields of the pulse analyser");
    assert_eq!(f.line_code, Some(PulseCode::Pwm));
    assert_eq!(f.bits, Some(24));
    assert_eq!(f.bits_hex, Some("B29CA6"));
    assert_eq!(f.bits_bin, Some("101100101001110010100110"));
    assert_eq!(f.repeats, Some(4));
    assert_eq!(f.pulses, Some(25));
    assert!(
        f.period_us.is_some_and(|p| (p - 1400.0).abs() < 40.0),
        "{f:?}"
    );
    assert_eq!(
        (f.half_bit_us, f.unit_us, f.bit_rate, f.pulse_us),
        (None, None, None, None)
    );
    // Two groups of 4 frames in 1.5 s.
    assert_eq!((f.frames_good, f.frames_seen), (8, 8));
    let st = res.status();
    assert!(
        st.summary.contains("guesses the line code"),
        "{}",
        st.summary
    );
    assert!(
        st.summary
            .ends_with("PWM, 24 bits: B29CA6, repeated 4 time(s)"),
        "{}",
        st.summary
    );
}

/// A change of frequency keeps the logged frames and drops the facts of the last frame and the counts.
#[test]
fn a_change_of_frequency_keeps_the_frames_and_drops_the_last_one() {
    let h = harness(GenPattern::Pwm, 4, 25.0, 0.0, 0.0, 0.0, 1);
    let mut r = rx(h.values().unwrap());
    feed(&mut r, &h.generate().unwrap());
    let rows = crate::util::table(r.results(), "pulse_frames").len();
    assert_eq!(rows, 8);
    r.forget();
    assert_eq!(crate::util::table(r.results(), "pulse_frames").len(), rows);
    let f = IsmPulseFields::read(r.results().fields()).unwrap();
    assert_eq!((f.line_code, f.bits_hex, f.repeats), (None, None, None));
    assert_eq!((f.frames_seen, f.frames_good), (0, 0));
}

/// The default submode reads devices and no pulse frame; the submode refuses rates under 100 ksps and takes 250 ksps.
#[test]
fn the_submode_switches_the_reader_and_the_rate() {
    let h = harness(GenPattern::Pwm, 4, 25.0, 0.0, 0.0, 0.0, 1);
    let x = h.generate().unwrap();
    let mut v = h.values().unwrap();
    let id = v.schema().id(SUBMODE.key()).unwrap();
    v.set(id, &sdrkit_core::Value::Choice(0)).unwrap();
    let mut r = ism::receiver(&StackConfig::new(v.clone())).unwrap();
    assert!(r
        .configure(&InputInfo::new(RATE, 433.92e6, 433.92e6).recording())
        .is_err());
    r.configure(&InputInfo::new(1e6, 433.92e6, 433.92e6).recording())
        .unwrap();
    let mut r = rx(h.values().unwrap());
    assert!(r
        .configure(&InputInfo::new(50e3, 433.92e6, 433.92e6).recording())
        .is_err());
    r.configure(&InputInfo::new(RATE, 433.92e6, 433.92e6).recording())
        .unwrap();
    feed(&mut r, &x);
    assert_eq!(frames(&r).0.len(), 8);
}

/// The verify case of the submode passes on its test signal with noise.
#[test]
fn the_verify_case_of_the_submode_passes() {
    let hooks: &VerifyHooks = ISM.verify.unwrap();
    let o = VerifyOptions {
        seconds: 1.0,
        clean: false,
        snr_db: 25.0,
    };
    let case = (hooks.cases)(&o)
        .into_iter()
        .find(|c| c.name.contains("pulse_analyser"))
        .expect("the case of the submode");
    let mut h = Harness::new(&ISM)
        .rate(2e6)
        .seconds(case.seconds)
        .seed(1)
        .with_settings(&case.settings)
        .set(&GEN_NOISE, NoiseKind::Snr);
    if let Some(snr) = case.snr_db {
        h = h.set(&GEN_SNR_DB, snr);
    }
    let run = h.run().unwrap();
    for c in (case.check.unwrap())(&run.results) {
        assert_eq!(c.outcome, sdrkit_core::verify::Outcome::Pass, "{c:?}");
    }
}
