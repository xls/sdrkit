//! Evaluation of the encoder on synthetic speech with a known pitch and envelope, for both coders: the share of frames
//! whose coded pitch is right, the voicing of noise, and the envelope error of encode then decode.
//!
//! `cargo test -p sdrkit-codec-mbe --test pitch -- --ignored --nocapture` prints the tables.

use sdrkit_codec_mbe::{Coder, Decoder, Encoder, FRAME};
use std::f64::consts::TAU;

/// The pitches of the steady cases in Hz.
const PITCHES: [f64; 12] = [70.0, 80.0, 100.0, 120.0, 150.0, 180.0, 200.0, 220.0, 260.0, 300.0, 350.0, 400.0];

/// A vowel: its name and three formants as (centre Hz, width Hz, level).
type Vowel = (&'static str, [(f64, f64, f64); 3]);

const VOWELS: [Vowel; 5] = [
    ("a", [(730.0, 90.0, 1.0), (1090.0, 110.0, 0.5), (2440.0, 170.0, 0.12)]),
    ("e", [(530.0, 80.0, 1.0), (1840.0, 140.0, 0.25), (2480.0, 170.0, 0.12)]),
    ("i", [(270.0, 70.0, 1.0), (2290.0, 160.0, 0.2), (3010.0, 200.0, 0.1)]),
    ("o", [(570.0, 80.0, 1.0), (840.0, 100.0, 0.45), (2410.0, 170.0, 0.06)]),
    ("u", [(300.0, 70.0, 1.0), (870.0, 100.0, 0.3), (2240.0, 170.0, 0.05)]),
];

/// The amplitude of a harmonic of `vowel` at `hz`: formant peaks over a floor that falls with frequency.
fn envelope(vowel: &Vowel, hz: f64) -> f64 {
    let peaks: f64 = vowel.1.iter().map(|&(c, w, g)| g / (1.0 + ((hz - c) / w).powi(2))).sum();
    2500.0 * (peaks + 0.04 * (-hz / 1500.0).exp())
}

fn next(x: &mut u64) -> f64 {
    *x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    ((*x >> 40) as f64 / (1u64 << 24) as f64) - 0.5
}

/// A test signal: its samples and, per sample, the pitch in Hz (0 where it has none).
#[derive(Default)]
struct Signal {
    pcm: Vec<i16>,
    f0: Vec<f64>,
    phase: f64,
    seed: u64,
    /// The gain of the odd harmonics above the first; 0 stands for 1.
    odd: f64,
}

impl Signal {
    /// Adds `n` samples of `vowel`; `f0` and `gain` are functions of the time in seconds since the start of this part.
    /// `phases` picks the harmonic phases: 0 for cosines in step (a pulse train), else a fixed scatter.
    fn voiced(&mut self, n: usize, vowel: &Vowel, phases: u32, f0: impl Fn(f64) -> f64, gain: impl Fn(f64) -> f64) {
        for i in 0..n {
            let t = i as f64 / 8000.0;
            let f = f0(t);
            self.phase += TAU * f / 8000.0;
            let mut v = 0.0;
            let mut h = 1.0;
            while h * f < 3700.0 {
                let th = phases as f64 * 1.7 * h * h;
                let weak = if self.odd > 0.0 && h > 1.0 && h % 2.0 == 1.0 { self.odd } else { 1.0 };
                v += weak * envelope(vowel, h * f) * (h * self.phase + th).cos();
                h += 1.0;
            }
            self.pcm.push((gain(t) * v).clamp(-32000.0, 32000.0) as i16);
            self.f0.push(f);
        }
    }

    /// Adds `n` samples of white noise of peak `level`, or silence for level 0.
    fn noise(&mut self, n: usize, level: f64) {
        for _ in 0..n {
            self.pcm.push((2.0 * level * next(&mut self.seed)) as i16);
            self.f0.push(0.0);
        }
    }
}

/// The result of one case.
#[derive(Default, Clone, Copy)]
struct Score {
    /// Frames with a pitch that were scored.
    frames: usize,
    /// Of those, frames whose coded pitch is within 3 % of the truth.
    close: usize,
    /// Of those, frames whose coded pitch is more than 20 % off (multiples and sub-multiples).
    gross: usize,
    /// Harmonics of scored voiced frames, and how many were coded voiced.
    harmonics: usize,
    voiced: usize,
    /// Harmonics of frames without a pitch, and how many were coded voiced.
    noise_harmonics: usize,
    noise_voiced: usize,
}

impl Score {
    fn add(&mut self, o: Score) {
        self.frames += o.frames;
        self.close += o.close;
        self.gross += o.gross;
        self.harmonics += o.harmonics;
        self.voiced += o.voiced;
        self.noise_harmonics += o.noise_harmonics;
        self.noise_voiced += o.noise_voiced;
    }
    fn close_pct(&self) -> f64 {
        100.0 * self.close as f64 / self.frames.max(1) as f64
    }
    fn gross_pct(&self) -> f64 {
        100.0 * self.gross as f64 / self.frames.max(1) as f64
    }
    fn voiced_pct(&self) -> f64 {
        100.0 * self.voiced as f64 / self.harmonics.max(1) as f64
    }
    fn noise_voiced_pct(&self) -> f64 {
        100.0 * self.noise_voiced as f64 / self.noise_harmonics.max(1) as f64
    }
}

/// The centre of the frame that the encoder returns at call `k`, as a sample index of its input: the analysis runs two
/// frames behind the newest sample, whose window ends 160 samples before the end of the input.
fn centre(k: usize) -> isize {
    (k * FRAME) as isize - 321
}

/// Encodes and decodes `sig` and scores the coded pitch of every frame against the pitch at the frame centre. A frame
/// counts as voiced when the signal has a pitch over its whole window and over that of the frame before, so the first
/// two frames of a voiced part are left out; it counts as noise when its whole window has no pitch.
fn score(coder: Coder, sig: &Signal, mut each: impl FnMut(usize, f64, &Encoder, &Decoder)) -> Score {
    let mut enc = Encoder::new(coder);
    let mut dec = Decoder::new(coder);
    let mut bits = vec![0u8; coder.speech_bits()];
    let mut out = [0i16; FRAME];
    let mut s = Score::default();
    let at = |i: isize| if i >= 0 && (i as usize) < sig.f0.len() { sig.f0[i as usize] } else { 0.0 };
    for (k, pcm) in sig.pcm.as_chunks::<FRAME>().0.iter().enumerate() {
        assert_eq!(enc.encode_speech(pcm, &mut bits), coder.speech_bits());
        dec.decode_speech(&bits, &mut out);
        let c = centre(k);
        let p = dec.params();
        let voiced = (1..=p.l).filter(|&l| p.voiced[l]).count();
        if (c - 310..=c + 150).all(|i| at(i) > 0.0) {
            let truth = at(c);
            let ratio = p.f0_hz() / truth;
            s.frames += 1;
            s.close += ((ratio - 1.0).abs() <= 0.03) as usize;
            s.gross += ((ratio - 1.0).abs() > 0.2) as usize;
            s.harmonics += p.l;
            s.voiced += voiced;
            each(k, truth, &enc, &dec);
        } else if (c - 150..=c + 150).all(|i| at(i) == 0.0) {
            s.noise_harmonics += p.l;
            s.noise_voiced += voiced;
        }
    }
    s
}

fn plain(coder: Coder, sig: &Signal) -> Score {
    score(coder, sig, |_, _, _, _| {})
}

/// A steady vowel of 30 frames at `hz`; with `moving`, a 5 Hz vibrato of 2 % and a level that rises threefold.
fn steady(vowel: &Vowel, hz: f64, phases: u32, moving: bool) -> Signal {
    let mut s = Signal::default();
    let n = 30 * FRAME;
    if moving {
        s.voiced(n, vowel, phases, |t| hz * (1.0 + 0.02 * (TAU * 5.0 * t).sin()), |t| 0.33 + 0.67 * t / 0.6);
    } else {
        s.voiced(n, vowel, phases, |_| hz, |_| 1.0);
    }
    s
}

/// The highest pitch the coders carry, in Hz: the period 19.875.
const TOP_HZ: f64 = 8000.0 / 19.875;

/// All vowels, both phase sets, steady and moving, at `hz`; steady only where the vibrato would pass [`TOP_HZ`].
fn steady_score(coder: Coder, hz: f64) -> Score {
    let mut total = Score::default();
    for vowel in &VOWELS {
        for phases in 0..2 {
            for moving in [false, true] {
                if !moving || 1.02 * hz <= TOP_HZ {
                    total.add(plain(coder, &steady(vowel, hz, phases, moving)));
                }
            }
        }
    }
    total
}

/// A glide from `a` to `b` Hz over 1.2 s after 0.2 s at `a`.
fn glide(vowel: &Vowel, a: f64, b: f64) -> Signal {
    let mut s = Signal::default();
    s.voiced(70 * FRAME, vowel, 1, |t| a * (b / a).powf(((t - 0.2) / 1.2).clamp(0.0, 1.0)), |_| 1.0);
    s
}

/// The glides between neighbouring steady pitches, up and down, each on one vowel.
fn glide_score(coder: Coder) -> Score {
    let mut total = Score::default();
    for (i, w) in PITCHES.windows(2).enumerate() {
        total.add(plain(coder, &glide(&VOWELS[i % 5], w[0], w[1])));
        total.add(plain(coder, &glide(&VOWELS[(i + 2) % 5], w[1], w[0])));
    }
    total
}

/// Silence, then a vowel.
fn onset_score(coder: Coder) -> Score {
    let mut total = Score::default();
    for (i, hz) in [90.0, 130.0, 170.0, 210.0, 250.0, 330.0].into_iter().enumerate() {
        let mut s = Signal::default();
        s.noise(13 * FRAME + 37 * i, 0.0);
        s.voiced(25 * FRAME, &VOWELS[i % 5], 1, |_| hz, |t| (t / 0.03).min(1.0));
        total.add(plain(coder, &s));
    }
    total
}

/// A vowel, then noise, then the vowel again an octave away.
fn transition_score(coder: Coder) -> Score {
    let mut total = Score::default();
    for (i, hz) in [95.0, 125.0, 160.0, 205.0, 240.0].into_iter().enumerate() {
        let mut s = Signal::default();
        s.voiced(20 * FRAME + 23 * i, &VOWELS[i % 5], 1, |_| hz, |_| 1.0);
        s.noise(20 * FRAME, 2500.0);
        s.voiced(20 * FRAME, &VOWELS[(i + 1) % 5], 0, |_| if hz < 150.0 { 2.0 * hz } else { hz / 2.0 }, |_| 1.0);
        total.add(plain(coder, &s));
    }
    total
}

/// White noise alone.
fn noise_score(coder: Coder) -> Score {
    let mut s = Signal { seed: 7, ..Signal::default() };
    s.noise(100 * FRAME, 3000.0);
    plain(coder, &s)
}

/// A phrase over the range of one voice: an intonation contour between `lo` and `hi` Hz with vibrato, the vowel and the
/// level changing every 0.4 s.
fn phrase_score(coder: Coder, lo: f64, hi: f64) -> Score {
    let mut s = Signal::default();
    for (i, vowel) in VOWELS.iter().cycle().take(10).enumerate() {
        let t0 = 0.4 * i as f64;
        let f = |t: f64| {
            let x = 0.5 - 0.5 * (TAU * (t0 + t) / 2.3).cos();
            (lo + (hi - lo) * x) * (1.0 + 0.015 * (TAU * 5.5 * (t0 + t)).sin())
        };
        s.voiced(20 * FRAME, vowel, 1, f, |t| 0.5 + 0.5 * (TAU * t / 0.8).sin().abs());
    }
    plain(coder, &s)
}

/// Vowels at the steady pitches up to 350 Hz with white noise 15 dB below them and a pitch that wanders by 1 %.
fn rough_score(coder: Coder) -> Score {
    let mut total = Score::default();
    for (i, &hz) in PITCHES[..11].iter().enumerate() {
        let vowel = &VOWELS[i % 5];
        let mut s = Signal { seed: 11 + i as u64, ..Signal::default() };
        s.voiced(40 * FRAME, vowel, 1, |t| hz * (1.0 + 0.01 * (TAU * 31.0 * t).sin() * (TAU * 3.7 * t).cos()), |_| 1.0);
        add_noise(&mut s);
        total.add(plain(coder, &s));
    }
    total
}

/// Adds white noise 15 dB below the signal.
fn add_noise(s: &mut Signal) {
    let rms = (s.pcm.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / s.pcm.len() as f64).sqrt();
    // Uniform noise of peak a has the RMS a / sqrt(3).
    let peak = rms * 10f64.powf(-15.0 / 20.0) * 3f64.sqrt();
    let mut seed = s.seed;
    for v in s.pcm.iter_mut() {
        *v = (*v as f64 + 2.0 * peak * next(&mut seed)).clamp(-32000.0, 32000.0) as i16;
    }
}

/// Noisy vowels whose odd harmonics above the first are 10 dB or 6 dB down, so that half the period fits nearly as well as
/// the period: how often the pitch is coded an octave high. From 195 Hz half the period is below the candidate 21.
fn weak_odd_score(coder: Coder) -> Score {
    let mut total = Score::default();
    let pitches = [90.0, 100.0, 120.0, 135.0, 150.0, 170.0, 185.0, 195.0, 196.0, 197.0, 198.0, 199.0, 200.0, 230.0, 260.0];
    for odd in [0.316, 0.5] {
        for (i, hz) in pitches.into_iter().enumerate() {
            for vowel in &VOWELS {
                let mut s = Signal { seed: 23 + i as u64, odd, ..Signal::default() };
                s.voiced(30 * FRAME, vowel, 1, |_| hz, |_| 1.0);
                add_noise(&mut s);
                total.add(plain(coder, &s));
            }
        }
    }
    total
}

/// The envelope error in dB (RMS over harmonics within 30 dB of the strongest, steady pitches from 80 to 300 Hz) for
/// one vowel: of the analysed amplitudes, of the coded ones as a decoder rebuilds them, and of the decoder's after its
/// enhancement.
fn envelope_error(coder: Coder, vowel: &Vowel) -> [f64; 3] {
    let mut sum = [0.0; 3];
    let mut n = 0usize;
    for &hz in &PITCHES[1..10] {
        let sig = steady(vowel, hz, 1, false);
        let top = (1..).map(|h| h as f64 * hz).take_while(|&f| f < 3700.0).map(|f| envelope(vowel, f)).fold(0.0, f64::max);
        score(coder, &sig, |k, truth, e, d| {
            if k < 10 || (d.params().f0_hz() / truth - 1.0).abs() > 0.03 {
                return;
            }
            for (which, p) in [e.analysed(), e.coded(), d.params()].into_iter().enumerate() {
                for l in 1..=p.l {
                    let f = l as f64 * p.f0_hz();
                    // The model amplitude M of a cosine of amplitude a is a / 2.
                    let want = envelope(vowel, f) / 2.0;
                    if f < 3600.0 && want > top / 2.0 / 31.6 {
                        sum[which] += (20.0 * (p.amp[l].max(1e-3) / want).log10()).powi(2);
                        n += (which == 0) as usize;
                    }
                }
            }
        });
    }
    sum.map(|v| (v / n.max(1) as f64).sqrt())
}

/// The level in dB of decoded white noise against the input, after the first ten frames.
fn noise_level(coder: Coder) -> f64 {
    let mut s = Signal { seed: 3, ..Signal::default() };
    s.noise(80 * FRAME, 3000.0);
    let mut enc = Encoder::new(coder);
    let mut dec = Decoder::new(coder);
    let mut bits = vec![0u8; coder.speech_bits()];
    let mut out = [0i16; FRAME];
    let (mut ein, mut eout) = (0.0, 0.0);
    for (k, pcm) in s.pcm.as_chunks::<FRAME>().0.iter().enumerate() {
        enc.encode_speech(pcm, &mut bits);
        dec.decode_speech(&bits, &mut out);
        if k >= 10 {
            ein += pcm.iter().map(|&v| (v as f64).powi(2)).sum::<f64>();
            eout += out.iter().map(|&v| (v as f64).powi(2)).sum::<f64>();
        }
    }
    10.0 * (eout / ein).log10()
}

#[test]
fn steady_vowels_have_no_gross_pitch_error() {
    for coder in Coder::ALL {
        for &hz in &PITCHES {
            let s = steady_score(coder, hz);
            assert_eq!(s.gross, 0, "{coder:?} {hz} Hz: {} of {} frames", s.gross, s.frames);
            assert!(s.close_pct() >= 99.0, "{coder:?} {hz} Hz: {:.1} % within 3 %", s.close_pct());
            assert!(s.voiced_pct() >= 50.0, "{coder:?} {hz} Hz: {:.1} % voiced", s.voiced_pct());
        }
    }
}

#[test]
fn glides_and_phrases_keep_the_pitch() {
    for coder in Coder::ALL {
        let g = glide_score(coder);
        assert!(g.gross_pct() < 2.0, "{coder:?} glides: {:.2} % gross of {}", g.gross_pct(), g.frames);
        let r = rough_score(coder);
        assert!(r.gross_pct() < 2.0, "{coder:?} vowels in noise: {:.2} % gross of {}", r.gross_pct(), r.frames);
        let w = weak_odd_score(coder);
        assert_eq!(w.gross, 0, "{coder:?} weak odd harmonics: {} gross of {}", w.gross, w.frames);
        for (lo, hi) in [(85.0, 155.0), (165.0, 255.0)] {
            let p = phrase_score(coder, lo, hi);
            assert!(p.gross_pct() < 2.0, "{coder:?} phrase {lo} to {hi} Hz: {:.2} % gross", p.gross_pct());
        }
    }
}

#[test]
fn onsets_and_transitions_find_the_pitch_and_noise_stays_unvoiced() {
    for coder in Coder::ALL {
        let o = onset_score(coder);
        assert_eq!(o.gross, 0, "{coder:?} onset: {} of {}", o.gross, o.frames);
        let t = transition_score(coder);
        assert_eq!(t.gross, 0, "{coder:?} transition: {} of {}", t.gross, t.frames);
        assert!(t.noise_voiced_pct() < 20.0, "{coder:?} transition: {:.1} % of noise voiced", t.noise_voiced_pct());
        let n = noise_score(coder);
        assert!(n.noise_voiced_pct() < 20.0, "{coder:?} noise: {:.1} % voiced", n.noise_voiced_pct());
        assert!(noise_level(coder).abs() < 3.0, "{coder:?} noise level {:.2} dB", noise_level(coder));
    }
}

#[test]
fn the_envelope_survives_encode_and_decode() {
    for coder in Coder::ALL {
        for vowel in &VOWELS {
            let [analysed, coded, decoded] = envelope_error(coder, vowel);
            assert!(analysed < 1.0, "{coder:?} {}: analysed envelope off by {analysed:.2} dB", vowel.0);
            assert!(coded < 4.0, "{coder:?} {}: coded envelope off by {coded:.2} dB", vowel.0);
            assert!(decoded < 6.0, "{coder:?} {}: decoded envelope off by {decoded:.2} dB", vowel.0);
        }
    }
}

/// The pitches with a frame off by more than 3 %, and the scored frames, of steady vowels every `step` Hz from 66 Hz to
/// [`TOP_HZ`].
fn sweep(coder: Coder, step: usize, phases: std::ops::Range<u32>) -> (Vec<usize>, usize) {
    let (mut bad, mut frames) = (Vec::new(), 0);
    for hz in (66..=TOP_HZ as usize).step_by(step) {
        for ph in phases.clone() {
            let s = plain(coder, &steady(&VOWELS[hz % 5], hz as f64, ph, false));
            frames += s.frames;
            if s.close != s.frames {
                bad.push(hz);
            }
        }
    }
    (bad, frames)
}

#[test]
fn every_pitch_of_the_range_is_found_from_the_onset() {
    let (bad, frames) = sweep(Coder::FullRate, 1, 1..2);
    assert!(bad.is_empty() && frames > 8000, "pitch off by more than 3 % at {bad:?} Hz");
    let (bad, _) = sweep(Coder::HalfRate, 3, 0..1);
    assert!(bad.is_empty(), "pitch off by more than 3 % at {bad:?} Hz");
}

#[test]
#[ignore]
fn table() {
    let coders = Coder::ALL;
    let row = |name: &str, f: &dyn Fn(Coder) -> Score| {
        let s: Vec<Score> = coders.iter().map(|&c| f(c)).collect();
        println!(
            "{name:<22} {:>5} {:>6.1} {:>6.1} {:>6.1} | {:>6.1} {:>6.1} {:>6.1}",
            s[0].frames,
            s[0].close_pct(),
            s[0].gross_pct(),
            s[0].voiced_pct(),
            s[1].close_pct(),
            s[1].gross_pct(),
            s[1].voiced_pct()
        );
    };
    println!("{:<22} {:>5} {:>20} | {:>20}", "case", "frames", format!("{:?}", coders[0]), format!("{:?}", coders[1]));
    println!("{:<22} {:>5} {:>6} {:>6} {:>6} | {:>6} {:>6} {:>6}", "", "", "<3%", "gross", "voiced", "<3%", "gross", "voiced");
    for &hz in &PITCHES {
        row(&format!("steady {hz} Hz"), &|c| steady_score(c, hz));
    }
    row("glides", &glide_score);
    row("noisy, wandering", &rough_score);
    row("weak odd harmonics", &weak_odd_score);
    row("onset from silence", &onset_score);
    row("vowel, noise, vowel", &transition_score);
    row("phrase 85 to 155 Hz", &|c| phrase_score(c, 85.0, 155.0));
    row("phrase 165 to 255 Hz", &|c| phrase_score(c, 165.0, 255.0));
    for &c in &coders {
        println!(
            "{c:?}: white noise {:.1} % of harmonics voiced, level {:+.2} dB; noise between vowels {:.1} % voiced",
            noise_score(c).noise_voiced_pct(),
            noise_level(c),
            transition_score(c).noise_voiced_pct()
        );
    }
    println!("envelope error in dB RMS: analysed, coded, after enhancement");
    for vowel in &VOWELS {
        let e: Vec<[f64; 3]> = coders.iter().map(|&c| envelope_error(c, vowel)).collect();
        println!("  {}: {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2}", vowel.0, e[0][0], e[0][1], e[0][2], e[1][0], e[1][1], e[1][2]);
    }
    for &c in &coders {
        let (bad, frames) = sweep(c, 1, 0..2);
        println!("{c:?}: every 1 Hz from 66 to {} Hz, two phase sets, {frames} frames: off by more than 3 % at {bad:?}", TOP_HZ as usize);
    }
}

