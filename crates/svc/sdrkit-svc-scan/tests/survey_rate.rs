//! The detection rate of stage one of a survey, measured on generated signals through [`SurveyScanner`] in looks of 2, 3,
//! 4 and 6 s: Morse at stated levels and speeds, also with one frame of the look 20 dB up in every bin; and the
//! candidates that noise, FM voice, pulse trains and a steady carrier give; and the occupied channels that stage one lists
//! for narrow FM voice and for the other signals.
//!
//! The level of a signal is its carrier power over the noise power in one bin width (`rate / bins`), with no correction
//! for the window. The signal tests take frames of 156.25 kHz in 2048 bins (76.3 Hz, 13.1 ms), the frame of a 10 MHz
//! block; the noise test takes whole frames of 10 MHz in 131 072 bins. The level of narrow FM voice is its power over the
//! noise power in a channel of 12.5 kHz, which is 22.1 dB under its level by the first measure.
//!
//! One test checks on fixed trials that a longer look finds no fewer stations. The measurements are ignored tests: `cargo test --profile shipped -p sdrkit-svc-scan --test survey_rate -- --ignored
//! --nocapture`. `SURVEY_TRIALS` sets the trials per cell (200), `SURVEY_SEED` the first seed.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::Complex32;
use sdrkit_algo_modem::keysurvey::{KeyedSurvey, SurveyConfig};
use sdrkit_algo_sim::modulate::{Am, Fm};
use sdrkit_algo_sim::rng::Rng as SimRng;
use sdrkit_algo_sim::voice::Voice;
use sdrkit_svc_scan::*;
use sdrkit_util_text::morse::Keyer;
use std::f64::consts::TAU;
use std::time::{Duration, Instant};

const RATE: f64 = 156_250.0;
const LOOK_S: f64 = 2.0;
/// A burst is this far over the frame it is in, in every bin, dB.
const BURST_DB: f32 = 20.0;
const CENTRE_HZ: f64 = 100e6;
/// The rise and fall time of the keying, s.
const EDGE_S: f64 = 0.005;
/// The channel of narrow FM voice, Hz.
const CHANNEL_HZ: f64 = 12.5e3;
/// The peak deviation of narrow FM voice, Hz.
const DEVIATION_HZ: f64 = 2.5e3;

fn env(name: &str, or: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(or)
}

fn trials() -> u64 {
    env("SURVEY_TRIALS", 200)
}

fn base() -> u64 {
    env("SURVEY_SEED", 500_000)
}

/// A repeatable generator of uniform and normal numbers.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for _ in 0..8 {
            r.next();
        }
        r
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in (0, 1).
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Two independent normal numbers of variance 1.
    fn normal2(&mut self) -> (f32, f32) {
        let r = (-2.0 * self.uniform().ln()).sqrt();
        let a = TAU * self.uniform();
        ((r * a.cos()) as f32, (r * a.sin()) as f32)
    }
}

/// A survey of one block of `rate_hz` around [`CENTRE_HZ`], tuned and ready for frames.
fn scanner(rate_hz: f64) -> SurveyScanner {
    scanner_for(rate_hz, LOOK_S)
}

/// As [`scanner`], with a look of `look_s`.
fn scanner_for(rate_hz: f64, look_s: f64) -> SurveyScanner {
    let half = rate_hz * USED_SHARE / 2.0;
    let mut s = SurveyScanner::new(SurveyScanConfig {
        start_hz: CENTRE_HZ - half,
        stop_hz: CENTRE_HZ + half,
        rate_hz,
        look_s,
        reject_hold_s: 0.0,
        find: FindConfig {
            stack: "cw".into(),
            watch: "signals".into(),
            above: 0.0,
            settle_s: 0.0,
            dwell_s: 1.0,
            after: AfterFind::Stay,
            quiet_s: 1.0,
            total_s: 0.0,
            clock_offset_s: 0.0,
            hit_cap: 16,
        },
    })
    .unwrap();
    assert_eq!(s.blocks(), [CENTRE_HZ]);
    assert_eq!(s.start(0.0), Step::Tune);
    assert_eq!(s.tick(0.0, &Observation::default(), None), None);
    s
}

/// The frames of the survey per look.
fn look_frames(bins: usize, rate_hz: f64) -> usize {
    (LOOK_S / (bins as f64 / rate_hz)).ceil() as usize
}

/// Ends the look and gives the offsets of the candidates from the middle of the block, Hz.
fn end_look(s: &mut SurveyScanner) -> Vec<f64> {
    s.tick(1e3, &Observation::default(), None);
    s.candidates().map(|c| c.freq_hz - CENTRE_HZ).collect()
}

/// The candidates that stage one gives for the samples `x` with noise for a carrier of power 1 at `snr_db`.
fn survey(x: &mut [Complex32], snr_db: f64, rng: &mut Rng) -> Vec<f64> {
    survey_with(x, snr_db, rng, false)
}

/// As [`survey`], over a look as long as `x`; with `burst`, one frame at a random place is [`BURST_DB`] up in every bin.
fn survey_with(x: &mut [Complex32], snr_db: f64, rng: &mut Rng, burst: bool) -> Vec<f64> {
    let mut s = looked(x, snr_db, rng, burst);
    end_look(&mut s)
}

/// The occupied channels that stage one lists for the samples `x` with noise for a carrier of power 1 at `snr_db`.
fn occupied(x: &mut [Complex32], snr_db: f64, rng: &mut Rng) -> Vec<OccupiedChannel> {
    let mut s = looked(x, snr_db, rng, false);
    end_look(&mut s);
    s.occupied().copied().collect()
}

/// A survey that has taken the frames of `x` with noise, as [`survey_with`], before the look ends.
fn looked(x: &mut [Complex32], snr_db: f64, rng: &mut Rng, burst: bool) -> SurveyScanner {
    let frames = x.len() / 2048;
    let mut s = scanner_for(RATE, x.len() as f64 / RATE);
    let need = s.need().unwrap();
    let n = need.bins;
    let sd = (n as f64 / 10f64.powf(snr_db / 10.0) / 2.0).sqrt() as f32;
    for v in x.iter_mut() {
        let (a, b) = rng.normal2();
        *v += Complex32::new(a * sd, b * sd);
    }
    let mut fft = WindowedFft::new(n, n, need.window).unwrap();
    let mut db = vec![0f32; n];
    let burst_at = burst.then(|| (rng.uniform() * frames as f64) as usize);
    for (i, chunk) in x.chunks_exact(n).enumerate() {
        let p = fft.run_power(chunk);
        let up = if burst_at == Some(i) { BURST_DB } else { 0.0 };
        for (k, d) in db.iter_mut().enumerate() {
            *d = 10.0 * p[(k + n / 2) % n].max(1e-30).log10() + up;
        }
        s.frame(&SpectrumFrame {
            time_s: 0.0,
            centre_hz: CENTRE_HZ,
            span_hz: RATE,
            bins_db: &db,
        });
    }
    s
}

/// The samples of one look of [`LOOK_S`].
fn samples() -> usize {
    samples_in(LOOK_S)
}

/// The samples of one look of `look_s`: whole frames.
fn samples_in(look_s: f64) -> usize {
    2048 * (look_s / (2048.0 / RATE)).ceil() as usize
}

/// An offset inside the used part of the block and outside the guard at its middle, Hz.
fn offset(rng: &mut Rng) -> f64 {
    let at = 3e3 + rng.uniform() * (0.4 * RATE - 3e3);
    if rng.uniform() < 0.5 {
        -at
    } else {
        at
    }
}

fn tone(at_hz: f64, i: usize) -> Complex32 {
    let p = TAU * (at_hz * i as f64 / RATE).fract() + 0.3;
    Complex32::new(p.cos() as f32, p.sin() as f32)
}

/// Random letters and figures in words of 2 to 6.
fn text(rng: &mut Rng) -> String {
    let set = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut t = String::new();
    for _ in 0..12 {
        let len = 2 + (rng.uniform() * 5.0) as usize;
        for _ in 0..len {
            t.push(set[(rng.uniform() * set.len() as f64) as usize % set.len()] as char);
        }
        t.push(' ');
    }
    t
}

/// Morse of random text at `wpm` on a carrier at `at_hz`, from a random point of its keying, with raised-cosine edges.
fn morse(wpm: f64, at_hz: f64, rng: &mut Rng) -> Vec<Complex32> {
    morse_for(wpm, at_hz, rng, LOOK_S)
}

/// As [`morse`], for a look of `look_s`.
fn morse_for(wpm: f64, at_hz: f64, rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let mut marks = Vec::new();
    let mut t = 0.0;
    for k in Keyer::new(wpm).unwrap().key(&text(rng)) {
        if k.on {
            marks.push((t, t + k.seconds));
        }
        t += k.seconds;
    }
    let period = t + 0.5;
    let start = rng.uniform() * period;
    let mut at = 0;
    (0..samples_in(look_s))
        .map(|i| {
            let now = (start + i as f64 / RATE) % period;
            if now < marks[at].0 && at > 0 && now < marks[at - 1].1 + EDGE_S {
                at = 0;
            }
            while at + 1 < marks.len() && now >= marks[at].1 + EDGE_S {
                at += 1;
            }
            if at > 0 && now < marks[0].0 {
                at = 0;
            }
            let (a, b) = marks[at];
            let up = ((now - a) / EDGE_S + 0.5).clamp(0.0, 1.0);
            let down = ((b - now) / EDGE_S + 0.5).clamp(0.0, 1.0);
            let e = 0.5 - 0.5 * (std::f64::consts::PI * up.min(down)).cos();
            tone(at_hz, i) * e as f32
        })
        .collect()
}

/// A carrier at `at_hz` on for `on_s` in every `period_s`, from a random point of the period.
fn gated(at_hz: f64, on_s: f64, period_s: f64, rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let start = rng.uniform() * period_s;
    (0..samples_in(look_s))
        .map(|i| {
            if (i as f64 / RATE + start) % period_s < on_s {
                tone(at_hz, i)
            } else {
                Complex32::default()
            }
        })
        .collect()
}

/// FM voice: 2.5 kHz peak deviation, audio of three tones from 300 to 2700 Hz in syllables of 4 Hz with pauses.
fn fm_voice(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    let f: Vec<f64> = (0..3).map(|_| 300.0 + rng.uniform() * 2400.0).collect();
    let mut phase = 0.0f64;
    (0..samples_in(look_s))
        .map(|i| {
            let t = i as f64 / RATE;
            let syllable = (TAU * 4.0 * t).sin().max(0.0);
            let audio: f64 = f.iter().map(|f| (TAU * f * t).sin()).sum::<f64>() / 3.0;
            phase = (phase + TAU * (at + 2500.0 * syllable * audio) / RATE) % TAU;
            Complex32::new(phase.cos() as f32, phase.sin() as f32)
        })
        .collect()
}

/// Narrow FM voice on a carrier at `at_hz`: [`DEVIATION_HZ`] peak deviation, audio of noise from 300 to 2700 Hz in
/// syllables of 4 Hz with pauses as long, limited at the peak deviation.
fn narrow_fm(at_hz: f64, rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    fm_of_noise(at_hz, rng, look_s, 0.4, true)
}

/// FM on a carrier at `at_hz` with [`DEVIATION_HZ`] for audio of 1: the audio of [`Voice`] with the RMS value `level`,
/// limited at 1; in syllables of 4 Hz with pauses as long, from a random point of the cycle, if `syllables`, else steady.
fn fm_of_noise(
    at_hz: f64,
    rng: &mut Rng,
    look_s: f64,
    level: f64,
    syllables: bool,
) -> Vec<Complex32> {
    let n = samples_in(look_s);
    let voice = Voice::new(RATE, level, SimRng::new(rng.next())).unwrap();
    let mut voice = if syllables {
        voice.with_phase(rng.uniform())
    } else {
        voice.steady()
    };
    let mut audio = vec![0f32; n];
    voice.fill(&mut audio);
    let mut out = vec![Complex32::default(); n];
    Fm::new(RATE, DEVIATION_HZ)
        .unwrap()
        .process(&audio, &mut out)
        .unwrap();
    for (i, y) in out.iter_mut().enumerate() {
        *y *= tone(at_hz, i);
    }
    out
}

/// The level of a carrier over the noise of one bin that has `snr_db` over the noise of [`CHANNEL_HZ`], dB.
fn in_channel(snr_db: f64) -> f64 {
    snr_db + 10.0 * (CHANNEL_HZ / (RATE / 2048.0)).log10()
}

/// Narrow FM voice at `snr_db` in its channel over `trials` trials from seed `first`, in looks of `look_s`: the trials
/// in which an occupied channel is listed within 3 kHz of it, the occupied channels elsewhere, and the keyed candidates.
fn voice_rate_over(snr_db: f64, look_s: f64, first: u64, trials: u64) -> (u64, u64, u64) {
    let (mut hits, mut others, mut keyed) = (0, 0, 0);
    for seed in 0..trials {
        let mut rng = Rng::new(first + 31 + seed * 11 + (snr_db + 50.0) as u64 * 1009);
        let at = offset(&mut rng);
        let mut x = narrow_fm(at, &mut rng, look_s);
        let mut s = looked(&mut x, in_channel(snr_db), &mut rng, false);
        keyed += end_look(&mut s).len() as u64;
        let near = s
            .occupied()
            .filter(|c| (c.freq_hz - CENTRE_HZ - at).abs() <= 3e3)
            .count() as u64;
        hits += near.min(1);
        others += s.occupied().count() as u64 - near;
    }
    (hits, others, keyed)
}

/// The occupied channels over the trials of the signal made by `make` at `snr_db`, in looks of `look_s`.
fn occupied_channels(snr_db: f64, look_s: f64, make: Make) -> u64 {
    (0..trials())
        .map(|seed| {
            let mut rng = Rng::new(base() + 77 + seed * 13 + snr_db as u64 * 1009);
            let mut x = make(&mut rng, look_s);
            occupied(&mut x, snr_db, &mut rng).len() as u64
        })
        .sum()
}

/// Morse of random text at 20 WPM.
fn morse_20(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    morse_for(20.0, at, rng, look_s)
}

fn steady(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    (0..samples_in(look_s)).map(|i| tone(at, i)).collect()
}

/// Five steady carriers 1 kHz apart.
fn tones(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    (0..samples_in(look_s))
        .map(|i| (-2..=2).map(|k| tone(at + 1e3 * f64::from(k), i)).sum())
        .collect()
}

/// Telemetry: a carrier on for 25 ms in every 100 ms.
fn pulses(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    gated(at, 0.025, 0.1, rng, look_s)
}

/// Telemetry with gaps three times the pulse: 40 ms on in every 160 ms.
fn pulses_one_three(rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let at = offset(rng);
    gated(at, 0.04, 0.16, rng, look_s)
}

/// Runs `job` for each of `cells` on its own thread and gives the results in order.
fn each<C: Sync, R: Send>(cells: &[C], job: impl Fn(&C) -> R + Sync) -> Vec<R> {
    std::thread::scope(|s| {
        let hs: Vec<_> = cells.iter().map(|c| s.spawn(|| job(c))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// Morse at `wpm` and `snr_db` over the trials in looks of `look_s`, with a burst in each if `burst`: the trials in
/// which it is found within two bins, and the candidates elsewhere.
fn find_rate_with(wpm: f64, snr_db: f64, look_s: f64, burst: bool) -> (u64, u64) {
    find_rate_over(wpm, snr_db, look_s, burst, base(), trials())
}

/// As [`find_rate_with`], over `trials` trials from seed `first`.
fn find_rate_over(
    wpm: f64,
    snr_db: f64,
    look_s: f64,
    burst: bool,
    first: u64,
    trials: u64,
) -> (u64, u64) {
    let (mut hits, mut others) = (0, 0);
    let bin_hz = RATE / 2048.0;
    for seed in 0..trials {
        let mut rng = Rng::new(first + seed * 7 + wpm as u64 * 131 + snr_db as u64 * 1009);
        let at = offset(&mut rng);
        let mut x = morse_for(wpm, at, &mut rng, look_s);
        let c = survey_with(&mut x, snr_db, &mut rng, burst);
        let near = c.iter().filter(|c| (*c - at).abs() <= 2.0 * bin_hz).count() as u64;
        hits += near.min(1);
        others += c.len() as u64 - near;
    }
    (hits, others)
}

/// The candidates over the trials of the signal made by `make` at `snr_db`, in looks of `look_s`.
fn false_candidates(snr_db: f64, look_s: f64, make: Make) -> u64 {
    (0..trials())
        .map(|seed| {
            let mut rng = Rng::new(base() + 77 + seed * 13 + snr_db as u64 * 1009);
            let mut x = make(&mut rng, look_s);
            survey(&mut x, snr_db, &mut rng).len() as u64
        })
        .sum()
}

/// The look lengths of the measurements, s.
const LOOKS: [f64; 4] = [2.0, 3.0, 4.0, 6.0];

type Make = fn(&mut Rng, f64) -> Vec<Complex32>;

#[test]
#[ignore = "measured on 200 trials in looks of 2, 3, 4 and 6 s: 10 WPM at 10 dB 84, 99, 100, 100 %, at 8 dB 87, 87, 91, 96 %, at 6 dB 69, 69, 75, 85 %; 20 WPM at 8 dB 96, 99, 100, 100 %, at 6 dB 78, 85, 89, 94 %; 30 WPM at 8 dB 92, 98, 99, 100 %, at 6 dB 73, 83, 85, 92 %"]
fn rate_of_morse_found() {
    let cells: Vec<(f64, f64, f64)> = LOOKS
        .iter()
        .flat_map(|&look| {
            [15.0, 12.0, 10.0, 8.0, 6.0]
                .iter()
                .flat_map(move |&snr| [10.0, 20.0, 30.0].map(|wpm| (wpm, snr, look)))
        })
        .collect();
    let got = each(&cells, |&(wpm, snr, look)| {
        find_rate_with(wpm, snr, look, false)
    });
    let n = trials();
    for (&(wpm, snr, look), (hits, others)) in cells.iter().zip(got) {
        println!(
            "TABLE morse {look} s {snr:>2} dB {wpm:>2} WPM: found {:>5.1} % of {n}, other candidates {others}",
            100.0 * hits as f64 / n as f64
        );
    }
}

#[test]
#[ignore = "measured on 200 trials in looks of 2, 3, 4 and 6 s: at 10 dB as without the burst; at 20 dB 10 WPM 81, 94, 97, 100 %, 20 and 30 WPM 99 to 100 %"]
fn rate_of_morse_found_with_a_burst() {
    let cells: Vec<(f64, f64, f64)> = LOOKS
        .iter()
        .flat_map(|&look| {
            [20.0, 10.0]
                .iter()
                .flat_map(move |&snr| [10.0, 20.0, 30.0].map(|wpm| (wpm, snr, look)))
        })
        .collect();
    let got = each(&cells, |&(wpm, snr, look)| {
        find_rate_with(wpm, snr, look, true)
    });
    let n = trials();
    for (&(wpm, snr, look), (hits, others)) in cells.iter().zip(got) {
        println!(
            "TABLE burst {look} s {snr:>2} dB {wpm:>2} WPM: found {:>5.1} % of {n}, other candidates {others}",
            100.0 * hits as f64 / n as f64
        );
    }
}

/// A look of 4 s finds no fewer stations than one of 2 s, on 40 fixed trials at 10 dB of 6 and 10 WPM and at 8 dB of
/// 10 WPM; and it finds slow Morse that the short look misses.
#[test]
fn a_longer_look_finds_no_fewer_stations() {
    let cells = [
        (6.0, 10.0, 2.0),
        (6.0, 10.0, 4.0),
        (10.0, 10.0, 2.0),
        (10.0, 10.0, 4.0),
        (10.0, 8.0, 2.0),
        (10.0, 8.0, 4.0),
    ];
    let got = each(&cells, |&(wpm, snr, look)| {
        find_rate_over(wpm, snr, look, false, 500_000, 40).0
    });
    for (cell, found) in cells.chunks(2).zip(got.chunks(2)) {
        let (wpm, snr, _) = cell[0];
        assert!(
            found[0] <= found[1],
            "{wpm} WPM at {snr} dB: {found:?} of 40 in 2 and 4 s"
        );
    }
    assert!(
        got[1] >= got[0] + 8,
        "6 WPM: {} and {} of 40",
        got[0],
        got[1]
    );
    assert!(
        got[3] >= 37 && got[5] >= 34,
        "10 WPM in 4 s: {} and {} of 40",
        got[3],
        got[5]
    );
}

#[test]
#[ignore = "measured on 200 trials in looks of 2, 3, 4 and 6 s: FM voice at 10 dB 12, 6, 4, 1, at 20 dB 13, 8, 3, 0, at 30 dB 13, 1, 0, 0; pulses of 25 ms in 100 ms at 10 dB 5, 2, 3, 1; of 40 ms in 160 ms at 10 dB 4, 3, 0, 0; no pulses at 20 and 30 dB; a steady carrier 0"]
fn candidates_on_other_signals() {
    let kinds: [(&str, Make); 4] = [
        ("fm voice", fm_voice),
        ("pulses 25/100 ms", pulses),
        ("pulses 40/160 ms", pulses_one_three),
        ("steady carrier", steady),
    ];
    let cells: Vec<(&str, Make, f64, f64)> = LOOKS
        .iter()
        .flat_map(|&look| {
            kinds.iter().flat_map(move |&(name, make)| {
                [10.0, 20.0, 30.0].map(|snr| (name, make, snr, look))
            })
        })
        .collect();
    let got = each(&cells, |&(_, make, snr, look)| {
        false_candidates(snr, look, make)
    });
    let n = trials();
    for (&(name, _, snr, look), c) in cells.iter().zip(got) {
        println!("TABLE {name} {look} s {snr} dB: {c} candidates in {n} blocks");
    }
}

#[test]
#[ignore = "measured on 200 trials in looks of 2, 3, 4 and 6 s: at 20, 10 and 6 dB in the channel 100 % in every look, at 3 dB 64, 62, 59, 61 %, at 0 dB none; no channel elsewhere; no keyed candidate from 0 dB up"]
fn rate_of_narrow_fm_voice_listed_as_occupied() {
    let cells: Vec<(f64, f64)> = LOOKS
        .iter()
        .flat_map(|&look| [20.0, 10.0, 6.0, 3.0, 0.0, -12.0].map(|snr| (snr, look)))
        .collect();
    let got = each(&cells, |&(snr, look)| {
        voice_rate_over(snr, look, base(), trials())
    });
    let n = trials();
    for (&(snr, look), (hits, others, keyed)) in cells.iter().zip(got) {
        println!(
            "TABLE narrow fm {look} s {snr:>3} dB in the channel: listed {:>5.1} % of {n}, other channels {others}, keyed candidates {keyed}",
            100.0 * hits as f64 / n as f64
        );
    }
}

#[test]
#[ignore = "measured on 200 trials in looks of 2, 3, 4 and 6 s: no channel on Morse and a steady carrier at 10 to 40 dB; pulses of 25 ms at 30 dB 1 in the looks of 6 s, else none; FM of three tones, a spectrum of lines, at 20 dB 0 to 3, at 30 dB 83 to 86, at 40 dB 111 to 113"]
fn occupied_channels_on_other_signals() {
    let kinds: [(&str, Make); 5] = [
        ("morse 20 WPM", morse_20),
        ("fm of three tones", fm_voice),
        ("pulses 25/100 ms", pulses),
        ("pulses 40/160 ms", pulses_one_three),
        ("steady carrier", steady),
    ];
    let cells: Vec<(&str, Make, f64, f64)> = LOOKS
        .iter()
        .flat_map(|&look| {
            kinds.iter().flat_map(move |&(name, make)| {
                [10.0, 20.0, 30.0, 40.0].map(|snr| (name, make, snr, look))
            })
        })
        .collect();
    let got = each(&cells, |&(_, make, snr, look)| {
        occupied_channels(snr, look, make)
    });
    let n = trials();
    for (&(name, _, snr, look), c) in cells.iter().zip(got) {
        println!("TABLE occupied {name} {look} s {snr} dB: {c} channels in {n} blocks");
    }
}

/// Narrow FM voice is listed as an occupied channel of about its width in most looks of 2 s at 10 and at 6 dB in its
/// channel, on 20 fixed trials each; Morse, a steady carrier and noise alone give no channel.
#[test]
fn stage_one_lists_narrow_fm_voice_as_occupied_and_no_carrier() {
    let got = each(&[10.0, 6.0], |&snr| {
        voice_rate_over(snr, LOOK_S, 700_000, 20)
    });
    assert!(got[0].0 >= 18 && got[1].0 >= 12, "{got:?}");
    assert_eq!((got[0].1, got[1].1), (0, 0), "{got:?}");
    let mut rng = Rng::new(3);
    let at = 30e3;
    let mut x = narrow_fm(at, &mut rng, LOOK_S);
    let c = occupied(&mut x, in_channel(10.0), &mut rng);
    assert_eq!(c.len(), 1, "{c:?}");
    assert!((c[0].freq_hz - CENTRE_HZ - at).abs() < 1.5e3, "{c:?}");
    assert!((2e3..=CHANNEL_HZ).contains(&c[0].width_hz), "{c:?}");
    assert!((c[0].snr_db - 10.0).abs() < 6.0, "{c:?}");
    for (make, snr) in [(morse_20 as Make, 20.0), (steady, 30.0), (pulses, 20.0)] {
        let mut x = make(&mut rng, LOOK_S);
        assert_eq!(occupied(&mut x, snr, &mut rng), [], "at {snr} dB");
    }
    let mut x = vec![Complex32::default(); samples()];
    assert_eq!(occupied(&mut x, 20.0, &mut rng), []);
}

/// An FM carrier without speech in the look is listed when its residual deviation lights 2 kHz or more, here noise of
/// 250 Hz RMS at 20 dB in the channel; with a tenth of that deviation it stays a lone carrier.
#[test]
fn an_idle_fm_carrier_is_listed_by_the_width_of_its_residual_deviation() {
    let mut rng = Rng::new(5);
    let at = -40e3;
    let mut x = fm_of_noise(at, &mut rng, LOOK_S, 0.1, false);
    let c = occupied(&mut x, in_channel(20.0), &mut rng);
    assert_eq!(c.len(), 1, "{c:?}");
    assert!((c[0].freq_hz - CENTRE_HZ - at).abs() < 500.0, "{c:?}");
    assert!((2e3..=8e3).contains(&c[0].width_hz), "{c:?}");
    let mut x = fm_of_noise(at, &mut rng, LOOK_S, 0.01, false);
    assert_eq!(occupied(&mut x, in_channel(20.0), &mut rng), []);
}

/// Morse, a steady carrier, steady tones 1 kHz apart and pulse trains at 20, 40, 60 and 80 dB give no channel: the skirt that keying spreads beside a
/// strong carrier stands over the noise across some kHz but not within the skirt level of its peak.
#[test]
fn strong_carriers_give_no_channel() {
    let kinds: [Make; 5] = [morse_20, steady, pulses, pulses_one_three, tones];
    let cells: Vec<(usize, f64)> = (0..kinds.len())
        .flat_map(|k| [20.0, 40.0, 60.0, 80.0].map(|snr| (k, snr)))
        .collect();
    let got = each(&cells, |&(k, snr)| {
        let mut rng = Rng::new(900 + k as u64 * 7 + snr as u64);
        let mut x = kinds[k](&mut rng, LOOK_S);
        occupied(&mut x, snr, &mut rng)
    });
    for (cell, c) in cells.iter().zip(got) {
        assert_eq!(c, [], "{cell:?}");
    }
}

/// The width that holds 99 % of the power of the samples `x` alone, 0.5 % left out at each end, Hz.
fn occupied_bandwidth(x: &[Complex32]) -> f64 {
    let n = 2048;
    let mut fft = WindowedFft::new(n, n, sdrkit_algo_dsp::window::Window::BlackmanHarris4).unwrap();
    let mut total = vec![0f64; n];
    for chunk in x.chunks_exact(n) {
        for (t, &p) in total.iter_mut().zip(fft.run_power(chunk)) {
            *t += f64::from(p);
        }
    }
    let cut = 0.005 * total.iter().sum::<f64>();
    let edge = |bins: &mut dyn Iterator<Item = &f64>| {
        let mut acc = 0.0;
        bins.take_while(|&&p| {
            acc += p;
            acc < cut
        })
        .count()
    };
    // Bin 0 is the middle of the block: the signal lies away from the ends of the unrotated spectrum.
    let mut rotated: Vec<f64> = total[n / 2..].iter().chain(&total[..n / 2]).copied().collect();
    let lo = edge(&mut rotated.iter());
    rotated.reverse();
    let hi = n - 1 - edge(&mut rotated.iter());
    (hi + 1 - lo) as f64 * RATE / n as f64
}

/// Narrow FM voice 60 dB over the noise in its channel is listed once, with a width within a fifth of the width that
/// holds 99 % of its power; beside it a voice channel at 10 dB 25 kHz away is listed too.
#[test]
fn strong_fm_voice_is_listed_with_its_width_beside_a_weak_channel() {
    let at = -30e3;
    let mut rng = Rng::new(11);
    let mut x = narrow_fm(at, &mut rng, LOOK_S);
    let width = occupied_bandwidth(&x);
    let mut alone = x.clone();
    let c = occupied(&mut alone, in_channel(60.0), &mut rng);
    assert_eq!(c.len(), 1, "{c:?}");
    assert!((c[0].freq_hz - CENTRE_HZ - at).abs() < 1.5e3, "{c:?}");
    assert!((c[0].width_hz / width - 1.0).abs() < 0.2, "{c:?} against {width}");
    let weak = narrow_fm(at + 25e3, &mut rng, LOOK_S);
    let gain = 10f32.powf(-50.0 / 20.0);
    for (s, w) in x.iter_mut().zip(&weak) {
        *s += w * gain;
    }
    let mut c = occupied(&mut x, in_channel(60.0), &mut rng);
    c.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
    assert_eq!(c.len(), 2, "{c:?}");
    assert!((c[0].freq_hz - CENTRE_HZ - at).abs() < 1.5e3, "{c:?}");
    assert!((c[1].freq_hz - CENTRE_HZ - at - 25e3).abs() < 1.5e3, "{c:?}");
}

/// AM voice of depth `depth` on a carrier of power 1 at `at_hz`: the audio of [`Voice`] with the RMS value 0.4, in
/// syllables from a random point of the cycle.
fn am_voice(at_hz: f64, depth: f64, rng: &mut Rng, look_s: f64) -> Vec<Complex32> {
    let n = samples_in(look_s);
    let mut voice = Voice::new(RATE, 0.4, SimRng::new(rng.next()))
        .unwrap()
        .with_phase(rng.uniform());
    let mut audio = vec![0f32; n];
    voice.fill(&mut audio);
    let mut out = vec![Complex32::default(); n];
    Am::new(depth).unwrap().process(&audio, &mut out).unwrap();
    for (i, y) in out.iter_mut().enumerate() {
        *y *= tone(at_hz, i);
    }
    out
}

/// Narrow FM voice 20 dB under two neighbours one channel away on each side is listed at its own frequency, with the
/// neighbours at 40 and at 60 dB in their channels, where the skirts of the neighbours fill the gaps between the three.
#[test]
fn a_weak_voice_channel_between_two_strong_ones_is_listed() {
    let cells: Vec<(u64, f64)> = (0..5)
        .flat_map(|seed| [40.0, 60.0].map(|snr| (seed, snr)))
        .collect();
    let got = each(&cells, |&(seed, snr)| {
        let mut rng = Rng::new(1300 + seed);
        let at = 0.5 * offset(&mut rng);
        let mut x = narrow_fm(at - CHANNEL_HZ, &mut rng, LOOK_S);
        let weak = narrow_fm(at, &mut rng, LOOK_S);
        let other = narrow_fm(at + CHANNEL_HZ, &mut rng, LOOK_S);
        for ((s, w), o) in x.iter_mut().zip(&weak).zip(&other) {
            *s += w * 0.1 + o;
        }
        (at, occupied(&mut x, in_channel(snr), &mut rng))
    });
    for (cell, (at, mut c)) in cells.iter().zip(got) {
        // The guard at the middle of the block may hold one of the three.
        let wanted: Vec<f64> = [at - CHANNEL_HZ, at, at + CHANNEL_HZ]
            .into_iter()
            .filter(|f| f.abs() >= 3e3)
            .collect();
        c.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        assert_eq!(c.len(), wanted.len(), "{cell:?} at {at}: {c:?}");
        for (row, f) in c.iter().zip(wanted) {
            assert!((row.freq_hz - CENTRE_HZ - f).abs() < 1.5e3, "{cell:?}: {c:?}");
            assert!((3e3..=8e3).contains(&row.width_hz), "{cell:?}: {c:?}");
        }
    }
}

/// AM voice is listed once, by the width of its sidebands: at 20 and 30 dB in the channel with a depth of 0.5, and at
/// 10 dB with full depth; with a depth of 0.5 at 10 dB, where the sidebands are under the noise, in most looks.
#[test]
fn am_voice_is_listed_as_occupied() {
    let cells: Vec<(u64, f64, f64)> = (0..10)
        .flat_map(|seed| [(1.0, 10.0), (0.5, 10.0), (0.5, 20.0), (0.5, 30.0)].map(|(d, snr)| (seed, d, snr)))
        .collect();
    let got = each(&cells, |&(seed, depth, snr)| {
        let mut rng = Rng::new(1400 + seed * 13 + snr as u64);
        let at = offset(&mut rng);
        let mut x = am_voice(at, depth, &mut rng, LOOK_S);
        (at, occupied(&mut x, in_channel(snr), &mut rng))
    });
    let mut weak = 0;
    for (&(seed, depth, snr), (at, c)) in cells.iter().zip(got) {
        if depth == 0.5 && snr == 10.0 {
            assert!(c.len() <= 1, "seed {seed}: {c:?}");
            weak += c.len();
            continue;
        }
        assert_eq!(c.len(), 1, "seed {seed} depth {depth} at {snr} dB: {c:?}");
        assert!((c[0].freq_hz - CENTRE_HZ - at).abs() < 500.0, "seed {seed}: {c:?}");
        assert!((2e3..=8e3).contains(&c[0].width_hz), "seed {seed}: {c:?}");
    }
    assert!(weak >= 7, "{weak} of 10");
}

/// Looks at `blocks` whole blocks of 10 MHz of noise: the candidates, the processor time of the survey for its fastest
/// block, that of the detector alone on the same frames with its own defaults, and the occupied channels.
fn noise_blocks(blocks: u64, seed: u64) -> (u64, Duration, Duration, u64) {
    let rate = 10e6;
    let mut rng = Rng::new(seed);
    let (mut found, mut fastest, mut fastest_bare) = (0, Duration::MAX, Duration::MAX);
    let mut channels = 0;
    let mut db = Vec::new();
    let mut out = Vec::new();
    for _ in 0..blocks {
        let mut s = scanner(rate);
        let n = s.need().unwrap().bins;
        assert_eq!(n, 131_072);
        let mut bare = KeyedSurvey::new(SurveyConfig::new(
            n,
            rate / n as f64,
            n as f64 / rate,
            LOOK_S,
        ))
        .unwrap();
        let (mut spent, mut spent_bare) = (Duration::ZERO, Duration::ZERO);
        db.resize(n, 0f32);
        for _ in 0..look_frames(n, rate) {
            for d in db.iter_mut() {
                *d = -100.0 + 10.0 * (-rng.uniform().ln()).log10() as f32;
            }
            let t = Instant::now();
            s.frame(&SpectrumFrame {
                time_s: 0.0,
                centre_hz: CENTRE_HZ,
                span_hz: rate,
                bins_db: &db,
            });
            spent += t.elapsed();
            let t = Instant::now();
            bare.push_db(&db).unwrap();
            spent_bare += t.elapsed();
        }
        let t = Instant::now();
        found += end_look(&mut s).len() as u64;
        fastest = fastest.min(spent + t.elapsed());
        channels += s.occupied().count() as u64;
        let t = Instant::now();
        bare.candidates(&mut out);
        fastest_bare = fastest_bare.min(spent_bare + t.elapsed());
    }
    (found, fastest, fastest_bare, channels)
}

#[test]
#[ignore = "measured: no candidate in 180 blocks, no occupied channel in 45; run alone, the processor time of the survey is within a tenth of that of the detector alone"]
fn candidates_and_cost_on_noise() {
    // One thread first, for the time; then the rest of the blocks on eight.
    let (first, survey, bare, channels) = noise_blocks(5, base());
    let seeds: Vec<u64> = (1..=8).map(|k| base() + k).collect();
    let more = each(&seeds, |&s| noise_blocks(5, s));
    let channels = channels + more.iter().map(|m| m.3).sum::<u64>();
    let more: u64 = more.iter().map(|m| m.0).sum();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "TABLE noise: {} candidates and {channels} occupied channels in 45 blocks of 10 MHz; {:.0} ms of survey per block, {:.0} ms of the detector alone",
        first + more,
        ms(survey),
        ms(bare)
    );
}

#[test]
fn stage_one_lists_morse_and_nothing_on_noise() {
    let mut rng = Rng::new(1);
    let at = 20e3;
    let mut x = morse(20.0, at, &mut rng);
    let c = survey(&mut x, 20.0, &mut rng);
    assert_eq!(c.len(), 1, "{c:?}");
    assert!((c[0] - at).abs() < 160.0, "{c:?}");
    let mut x = vec![Complex32::default(); samples()];
    assert!(survey(&mut x, 20.0, &mut rng).is_empty());
}
