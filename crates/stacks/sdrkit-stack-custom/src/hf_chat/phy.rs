//! The waveform of `hf_chat` at 2400 samples a second: the weave of chirp directions, chirp symbols with the Gray map, OFDM
//! symbols with their pilots, the lattice unit and the closing weave, the layout of a burst, and the 12 kHz audio interface.

use super::{generator, interleave, encode, Modulation, Profile, AUDIO_CENTRE_HZ, CLOSING, PILOT_SEED, RATE, UNIT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::ofdm::{OfdmLayout, PilotPattern};
use sdrkit_algo_modem::sweeps::Sweep;
use sdrkit_core::Complex32;
use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

/// OFDM carriers.
pub const CARRIERS: usize = 48;
/// The cyclic prefix, samples.
pub const OFDM_CP: usize = 12;
/// Samples of an OFDM symbol with its prefix.
pub const OFDM_SYMBOL: usize = CARRIERS + OFDM_CP;
/// Every fourth carrier (0, 4, ..., 44) is a pilot.
pub const PILOT_STEP: usize = 4;
/// Samples of the raised-cosine edges of every sweep.
pub const RAMP: usize = 10;
/// The start state of the weave register.
pub const WEAVE_SEED: u16 = 0xACE1;

/// The first `n` directions of the weave (true: falling): a 16-bit Galois LFSR from [`WEAVE_SEED`]; each output is the low bit,
/// then the register shifts right and takes 0xB400 when the output was 1.
pub fn weave(n: usize) -> Vec<bool> {
    let mut s = WEAVE_SEED;
    (0..n)
        .map(|_| {
            let out = s & 1 == 1;
            s >>= 1;
            if out {
                s ^= 0xB400;
            }
            out
        })
        .collect()
}

/// Chip `k` of the base chirp of `m` chips, `exp(j pi (k^2 - m k) / m)`, with the phase reduced exactly in integers.
pub fn base_chirp(m: usize, k: usize) -> Complex32 {
    let (m, k) = (m as i64, (k % m) as i64);
    let t = (k * k - m * k).rem_euclid(2 * m);
    let a = PI * t as f64 / m as f64;
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

/// Appends a chirp symbol of value `v`: the last `guard` chips, then the `m` chips; rising chip `n` is `b[(n + v) mod m]`,
/// falling chip `n` is `conj(b[(n - v) mod m])`.
pub fn chirp_symbol(m: usize, guard: usize, v: usize, falling: bool, out: &mut Vec<Complex32>) {
    let chip = |n: usize| if falling { base_chirp(m, (n + m - v % m) % m).conj() } else { base_chirp(m, n + v) };
    out.extend((m - guard..m).map(chip));
    out.extend((0..m).map(chip));
}

/// The chirp values of the interleaved bits `bits` of profile `p`: groups of SF bits (the last filled with zero bits), each
/// group `B` sent as its Gray code `B ^ (B >> 1)`.
pub fn values(p: &Profile, bits: &[u8]) -> Vec<u16> {
    let Modulation::Chirp { sf, .. } = p.modulation else {
        return Vec::new();
    };
    let sf = usize::from(sf);
    (0..p.symbols())
        .map(|s| {
            let b = (0..sf).fold(0u16, |a, i| (a << 1) | u16::from(bits.get(s * sf + i).copied().unwrap_or(0) & 1));
            b ^ (b >> 1)
        })
        .collect()
}

/// The pilot of OFDM symbol `s` of a block on carrier `i`: +1 when the first output of the generator of
/// [`PILOT_SEED`] + `64 s + i` is even, else -1.
pub fn pilot(s: usize, i: usize) -> f32 {
    if generator(PILOT_SEED + 64 * s as u64 + i as u64).step() & 1 == 0 { 1.0 } else { -1.0 }
}

/// The OFDM grid of a block of `symbols` symbols: carriers -24 to 23 (carrier `i` of the protocol is `i - 24`), a pilot on
/// every fourth carrier with the value of [`pilot`], the other 36 data.
pub fn ofdm_layout(symbols: usize) -> &'static OfdmLayout {
    static L40: OnceLock<OfdmLayout> = OnceLock::new();
    static L24: OnceLock<OfdmLayout> = OnceLock::new();
    let make = move || {
        let cells = (0..symbols)
            .map(|s| (0..CARRIERS).step_by(PILOT_STEP).map(|i| (i as i32 - 24, Complex32::new(pilot(s, i), 0.0))).collect())
            .collect();
        OfdmLayout::new(CARRIERS, &[OFDM_CP], -24, 23, &[], &[], &PilotPattern { cells }).expect("the hf_chat OFDM grid")
    };
    if symbols == 40 { L40.get_or_init(make) } else { L24.get_or_init(make) }
}

/// The 60 samples of an OFDM symbol of the carrier values `cells` (carrier `i` of the protocol at index `i`):
/// `x[n] = sum X[i] exp(j 2 pi (i - 24) n / 48) / sqrt(48)`, sent as `x[36..48]` then `x[0..48]`.
pub fn ofdm_symbol(fft: &mut Fft, cells: &[Complex32], out: &mut Vec<Complex32>) {
    let mut buf = vec![Complex32::new(0.0, 0.0); CARRIERS];
    for (i, &c) in cells.iter().enumerate().take(CARRIERS) {
        buf[(i + CARRIERS - 24) % CARRIERS] = c;
    }
    fft.process(&mut buf);
    let g = 1.0 / (CARRIERS as f32).sqrt();
    out.extend(buf[CARRIERS - OFDM_CP..].iter().map(|v| v * g));
    out.extend(buf.iter().map(|v| v * g));
}

/// The value of a data carrier: BPSK `1 - 2 b`, or QPSK `((1 - 2 b0) + j (1 - 2 b1)) / sqrt 2`.
fn data_cell(bits: &[u8]) -> Complex32 {
    let s = |b: u8| 1.0 - 2.0 * f32::from(b & 1);
    match bits {
        [b] => Complex32::new(s(*b), 0.0),
        [b0, b1] => Complex32::new(s(*b0), s(*b1)) * std::f32::consts::FRAC_1_SQRT_2,
        _ => Complex32::new(0.0, 0.0),
    }
}

/// The symbols of one block of profile `p` for its `K` information bits `info`, in transmit order (the repeats of profile 0
/// one after the other, the weave run on across them).
pub fn block_signal(p: &Profile, info: &[u8]) -> Vec<Complex32> {
    let bits = interleave(&encode(p, info));
    let mut out = Vec::with_capacity(p.block_samples());
    match p.modulation {
        Modulation::Chirp { sf, guard } => {
            let m = 1usize << sf;
            let v = values(p, &bits);
            let w = weave(p.block_symbols());
            for (i, &d) in w.iter().enumerate() {
                chirp_symbol(m, guard, usize::from(v[i % v.len()]), d, &mut out);
            }
        }
        Modulation::Ofdm { bits: per, symbols } => {
            let layout = ofdm_layout(symbols);
            let mut fft = Fft::inverse(CARRIERS);
            let mut at = 0;
            for s in 0..symbols {
                let mut cells = vec![Complex32::new(0.0, 0.0); CARRIERS];
                for pl in layout.pilots(s as u64) {
                    cells[pl.index as usize] = pl.value;
                }
                for &d in layout.data(s as u64) {
                    cells[d as usize] = data_cell(&bits[at..at + per]);
                    at += per;
                }
                ofdm_symbol(&mut fft, &cells, &mut out);
            }
        }
    }
    out
}

/// One element of the lattice or the closing weave: start `t0` s, duration `d` s, audio frequency `f0` Hz, slope `k` Hz/s,
/// start phase `ph`.
fn element(t0: f64, d: f64, f0: f64, k: f64, ph: f64) -> Sweep {
    Sweep {
        start: (t0 * RATE + 0.5).floor() as usize,
        len: (d * RATE + 0.5).floor() as usize,
        start_hz: f0 - AUDIO_CENTRE_HZ,
        slope_hz_s: k,
        phase: ph,
    }
}

/// The step of the start frequencies of the lattice sweeps, Hz (the protocol's literal).
const LATTICE_STEP_HZ: f64 = 266.666_666_7;

/// The rising and falling sweeps of a lattice unit (4 each, 1 s, 1600 Hz/s).
pub fn lattice_sweeps() -> Vec<Sweep> {
    const RISE: [f64; 4] = [1.910, 2.055, 0.020, 2.763];
    const FALL: [f64; 4] = [2.784, 1.683, 2.367, 4.943];
    let mut v: Vec<Sweep> = (0..4).map(|k| element(0.5 * k as f64, 1.0, 300.0 + LATTICE_STEP_HZ * k as f64, 1600.0, RISE[k])).collect();
    v.extend((0..4).map(|j| element(0.25 + 0.5 * j as f64, 1.0, 2700.0 - LATTICE_STEP_HZ * j as f64, -1600.0, FALL[j])));
    v
}

/// The seven comb tones of a lattice unit (0.4 s each).
fn comb() -> Vec<Sweep> {
    const PHASE: [f64; 7] = [3.946, 1.721, 1.005, 0.368, 1.186, 5.295, 0.290];
    const ORDER: [u32; 7] = [0, 3, 6, 2, 5, 1, 4];
    (0..7).map(|n| element(0.35 + 0.4 * n as f64, 0.4, 450.0 + 350.0 * f64::from(ORDER[n]), 0.0, PHASE[n])).collect()
}

/// The four sweeps of the closing weave (0.5 s, 3200 Hz/s, phase 0).
pub fn closing_sweeps() -> Vec<Sweep> {
    vec![
        element(0.0, 0.5, 300.0, 3200.0, 0.0),
        element(0.25, 0.5, 800.0, 3200.0, 0.0),
        element(0.125, 0.5, 2700.0, -3200.0, 0.0),
        element(0.375, 0.5, 2200.0, -3200.0, 0.0),
    ]
}

/// `parts` summed into `len` samples and scaled to a mean power of 1.
fn render(parts: &[Sweep], len: usize) -> Vec<Complex32> {
    let mut x = vec![Complex32::new(0.0, 0.0); len];
    for s in parts {
        s.add_to(RATE, RAMP, &mut x);
    }
    let p = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / len as f64;
    let g = (1.0 / p.sqrt()) as f32;
    x.iter_mut().for_each(|v| *v *= g);
    x
}

/// The lattice unit: 8400 samples of mean power 1.
pub fn unit() -> &'static [Complex32] {
    static U: OnceLock<Vec<Complex32>> = OnceLock::new();
    U.get_or_init(|| {
        let mut parts = lattice_sweeps();
        parts.extend(comb());
        render(&parts, UNIT)
    })
}

/// The closing weave: 2400 samples of mean power 1.
pub fn closing() -> &'static [Complex32] {
    static C: OnceLock<Vec<Complex32>> = OnceLock::new();
    C.get_or_init(|| render(&closing_sweeps(), CLOSING))
}

/// A run of data symbols in a burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// The block.
    pub block: usize,
    /// The first symbol of the run in the block's transmit order.
    pub first: usize,
    /// Symbols of the run.
    pub count: usize,
    /// The first sample.
    pub start: usize,
}

/// Where the parts of a burst lie, in samples from its start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The start of each lattice unit.
    pub units: Vec<usize>,
    /// The runs of data symbols.
    pub segments: Vec<Segment>,
    /// The start of the closing weave.
    pub closing: usize,
    /// Samples of the burst.
    pub total: usize,
}

/// The layout of a burst of `blocks` blocks of profile `p` (section 8): the front units, the first block cut into
/// `midambles + 1` runs with a unit before each run but the first, the later blocks in one run each, the closing weave.
pub fn layout(p: &Profile, blocks: usize) -> Layout {
    let mut units: Vec<usize> = (0..p.front).map(|u| u * UNIT).collect();
    let mut at = p.front * UNIT;
    let mut segments = Vec::new();
    let len = p.symbol_len();
    let t = p.block_symbols();
    for b in 0..blocks {
        let runs = if b == 0 { p.midambles + 1 } else { 1 };
        let mut done = 0;
        for s in 0..runs {
            if s > 0 {
                units.push(at);
                at += UNIT;
            }
            let count = t * (s + 1) / runs - done;
            segments.push(Segment { block: b, first: done, count, start: at });
            at += count * len;
            done += count;
        }
    }
    Layout { units, segments, closing: at, total: at + CLOSING }
}

/// The burst of profile `p` for the `K`-bit blocks `blocks`: lattice units, data and the closing weave, at mean power about 1.
pub fn burst(p: &Profile, blocks: &[Vec<u8>]) -> Vec<Complex32> {
    let l = layout(p, blocks.len());
    let mut x = vec![Complex32::new(0.0, 0.0); l.total];
    for &u in &l.units {
        x[u..u + UNIT].copy_from_slice(unit());
    }
    let len = p.symbol_len();
    let signals: Vec<Vec<Complex32>> = blocks.iter().map(|b| block_signal(p, b)).collect();
    for s in &l.segments {
        let src = &signals[s.block][s.first * len..(s.first + s.count) * len];
        x[s.start..s.start + src.len()].copy_from_slice(src);
    }
    x[l.closing..].copy_from_slice(closing());
    x
}

/// Audio samples a second.
pub const AUDIO_RATE: f64 = 12_000.0;
/// Audio samples a baseband sample.
pub const AUDIO_STEP: usize = 5;
/// Taps of the audio filter.
pub const AUDIO_TAPS: usize = 127;

/// The 127 taps of the audio filter, not normalised: a sinc of cut-off 1250 / 12000 cycles a sample under a Blackman window.
pub fn audio_taps() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| {
        let fc = 1250.0 / AUDIO_RATE;
        (0..AUDIO_TAPS)
            .map(|i| {
                let x = i as f64 - 63.0;
                let s = if x == 0.0 { 2.0 * fc } else { (TAU * fc * x).sin() / (PI * x) };
                let w = 0.42 - 0.5 * (TAU * i as f64 / 126.0).cos() + 0.08 * (2.0 * TAU * i as f64 / 126.0).cos();
                (s * w) as f32
            })
            .collect()
    })
}

/// Baseband to 12 kHz audio (5 samples a baseband sample): up-sampled by 5 with a gain of 5, filtered without delay, moved up
/// by 1500 Hz, the real part taken, and scaled so that the largest magnitude is `peak`.
pub fn to_audio(bb: &[Complex32], peak: f32) -> Vec<f32> {
    let h = audio_taps();
    let n = bb.len() * AUDIO_STEP;
    let mut a: Vec<f32> = (0..n)
        .map(|i| {
            // f[i] = sum over m of h[i + 63 - m] up[m]; up is non-zero at multiples of 5.
            let mut f = Complex32::new(0.0, 0.0);
            let lo = (i + 63).saturating_sub(AUDIO_TAPS - 1).div_ceil(AUDIO_STEP);
            let hi = ((i + 63) / AUDIO_STEP).min(bb.len().saturating_sub(1));
            for m in lo..=hi {
                f += bb[m] * (5.0 * h[i + 63 - AUDIO_STEP * m]);
            }
            let ph = TAU * (AUDIO_CENTRE_HZ * i as f64 / AUDIO_RATE).fract();
            (f * Complex32::new(ph.cos() as f32, ph.sin() as f32)).re
        })
        .collect();
    let top = a.iter().fold(0f32, |m, v| m.max(v.abs()));
    if top > 0.0 {
        a.iter_mut().for_each(|v| *v *= peak / top);
    }
    a
}

/// Baseband to 12 kHz complex samples whose 0 Hz is the upper-sideband dial, one baseband sample at a time: up-sampled by 5
/// with a gain of 5, filtered with the audio taps, moved up by 1500 Hz. The output is that of [`to_audio`] before its real
/// part and its scale, [`AUDIO_TAPS`] / 2 samples later.
#[derive(Clone, Debug, Default)]
pub struct ChannelUp {
    hist: Vec<Complex32>,
    n: u64,
}

impl ChannelUp {
    /// Takes baseband sample `x` and appends 5 samples at 12 kHz.
    pub fn push(&mut self, x: Complex32, out: &mut Vec<Complex32>) {
        let h = audio_taps();
        let keep = AUDIO_TAPS.div_ceil(AUDIO_STEP);
        self.hist.push(x);
        if self.hist.len() > keep {
            self.hist.remove(0);
        }
        let last = self.hist.len() - 1;
        for r in 0..AUDIO_STEP {
            // Output 5 m + r takes the baseband sample m - j through tap 5 j + r.
            let mut f = Complex32::new(0.0, 0.0);
            for j in 0..=last {
                if let Some(&t) = h.get(AUDIO_STEP * j + r) {
                    f += self.hist[last - j] * (5.0 * t);
                }
            }
            let ph = TAU * (AUDIO_CENTRE_HZ * self.n as f64 / AUDIO_RATE).fract();
            self.n += 1;
            out.push(f * Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
    }
}

/// 12 kHz audio to baseband (one sample in 5): moved down by 1500 Hz with a gain of 2, filtered without delay.
pub fn from_audio(a: &[f32]) -> Vec<Complex32> {
    let m: Vec<Complex32> = a
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let ph = -TAU * (AUDIO_CENTRE_HZ * i as f64 / AUDIO_RATE).fract();
            Complex32::new(ph.cos() as f32, ph.sin() as f32) * (2.0 * v)
        })
        .collect();
    from_channel(&m)
}

/// Complex 12 kHz samples whose 0 Hz is the audio centre (1500 Hz) to baseband: filtered with the audio taps without delay and
/// one sample in 5 kept.
pub fn from_channel(m: &[Complex32]) -> Vec<Complex32> {
    let h = audio_taps();
    (0..m.len() / AUDIO_STEP)
        .map(|k| {
            let c = AUDIO_STEP * k + 63;
            let mut s = Complex32::new(0.0, 0.0);
            for (i, &t) in h.iter().enumerate() {
                if let Some(v) = c.checked_sub(i).and_then(|j| m.get(j)) {
                    s += v * t;
                }
            }
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hf_chat::frame::{blocks, parse_block, Block, Message};
    use crate::hf_chat::{interleave, PROFILES};

    fn near(a: Complex32, re: f32, im: f32) -> bool {
        (a.re - re).abs() < 2e-5 && (a.im - im).abs() < 2e-5
    }

    fn hex_bits(h: &str) -> Vec<u8> {
        h.chars().flat_map(|c| { let d = c.to_digit(16).unwrap() as u8; (0..4).rev().map(move |i| (d >> i) & 1) }).collect()
    }

    #[test]
    fn weave_pilots_and_lattice() {
        let w: String = weave(32).iter().map(|&b| if b { '1' } else { '0' }).collect();
        assert_eq!(w, "10000111001000110100011011011100");
        let row = |s: usize| -> String { (0..12).map(|q| if pilot(s, 4 * q) > 0.0 { '+' } else { '-' }).collect() };
        assert_eq!(row(0), "--++--++++--");
        assert_eq!(row(1), "++--++----++");
        assert_eq!(row(39), "++--++----++");
        let u = unit();
        assert!(near(u[0], 0.0, 0.0) && near(u[8000], 0.0, 0.0));
        assert!(near(u[100], -0.198_931, -0.530_228), "{}", u[100]);
        assert!(near(u[5000], -1.167_251, 1.378_682), "{}", u[5000]);
        let c = closing();
        assert!(near(c[500], -0.124_720, -0.216_022), "{}", c[500]);
        assert!(near(c[0], 0.0, 0.0) && near(c[2200], 0.0, 0.0));
    }

    #[test]
    fn layouts() {
        let l = layout(&PROFILES[0], 1);
        assert_eq!(l.units, [0, 8400, 34208, 60016, 86848, 112656, 138464, 165296, 191104, 216912]);
        let segs: Vec<(usize, usize)> = l.segments.iter().map(|s| (s.count, s.start)).collect();
        assert_eq!(segs, [(17, 16800), (17, 42608), (18, 68416), (17, 95248), (17, 121056), (18, 146864), (17, 173696), (17, 199504), (18, 225312)]);
        assert_eq!((l.closing, l.total), (243_744, 246_144));
        let l = layout(&PROFILES[1], 1);
        assert_eq!(l.units, [0, 60624]);
        assert_eq!((l.segments[0].count, l.segments[1].count, l.segments[1].start), (51, 52, 69024));
        let l = layout(&PROFILES[2], 3);
        assert_eq!(l.segments.iter().map(|s| s.start).collect::<Vec<_>>(), [8400, 65744, 123088]);
        assert_eq!((l.closing, l.total), (180_432, 182_832));
        let l = layout(&PROFILES[5], 3);
        assert_eq!(l.segments.iter().map(|s| s.start).collect::<Vec<_>>(), [8400, 10800, 13200]);
        assert_eq!(l.total, 18_000);
    }

    #[test]
    fn worked_examples_of_profiles_3_0_and_5() {
        let chat = crate::hf_chat::frame::tests_support::chat().bits().unwrap();
        // Profile 3.
        let p3 = &PROFILES[3];
        let b = blocks(&chat, 1, p3).unwrap();
        let cw = crate::hf_chat::encode(p3, &b[0]);
        assert_eq!(cw, hex_bits("104a4b5af2fb8e00146a22a000000000221212062485b0d80085214738f000000000000000000000000000000000000000000000000000000000000000002316350a023a4f5cc7d88f5451aa18a092d7144d1c6d50fdddf4cdbebb080427b3daabf14b3305d6ccfaec9b6f75d6efafa7a72a9a74d2fdd1f49bfc4a09b809b809"));
        let il = interleave(&cw);
        assert_eq!(il, hex_bits("8062b2ac402461405a180c5df8a4f46130871b86c1882a149309408a96b08402e74403408f25048cb408bc26848302f0210300a4baf812506401b13800b4018092a859c822830c0ad48109096011b82208aa0401240a32884498991c40469e10412214232101260854a792619ce5040e103032ca964c43f442669e023807a183"));
        let v = values(p3, &il);
        assert_eq!(v[..16], [192, 83, 235, 250, 96, 54, 81, 96, 119, 20, 10, 115, 132, 246, 142, 81]);
        assert_eq!(v[112..], [24, 40, 43, 175, 221, 106, 98, 142, 99, 85, 209, 3, 36, 4, 241, 194]);
        let x = burst(p3, &b);
        assert_eq!(x.len(), 43_568);
        let pw = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
        assert!((pw - 1.0).abs() < 1e-4, "{pw}");
        assert!(near(x[8400], 1.0, 0.0) && near(x[8401], 0.012_272, 0.999_925) && near(x[8500], -0.980_785, 0.195_090));
        // Profile 0, the short beacon of section 9.10.
        let p0 = &PROFILES[0];
        let sb = hex_bits("425ad797dc7002aae693f47c00");
        let b0 = blocks(&sb[..104], 0, p0).unwrap();
        assert_eq!(b0.len(), 1);
        let cw = crate::hf_chat::encode(p0, &b0[0]);
        assert_eq!(cw, hex_bits("00425ad797dc7002aae693f47c003ba5bac98c89f958215af7e0537f09949509912971e73e3580d1bf989f9cef9eb8ab121287c1871e10c28cc2599e7e7751ce"));
        let il = interleave(&cw);
        assert_eq!(il, hex_bits("9310e0c25aeeba2ad90e812b4cdea66f7e20cda32150c9cd1cc53ce539b07ce9d6c09e3ecabe87cf6e8ce7e606d82fb345801c7bc068924ce84cc40d52fa103f"));
        let v = values(p0, &il);
        assert_eq!(v[..8], [874, 393, 40, 887, 615, 627, 1005, 393]);
        assert_eq!(v[44..], [426, 966, 681, 11, 494, 625, 8, 640]);
        assert!(matches!(parse_block(&b0[0]), Some(Block::Part { id: 0, index: 0, .. })));
        assert!(Message::parse(&sb[..104]).is_some());
        // Profile 5.
        let p5 = &PROFILES[5];
        let b5 = blocks(&chat, 1, p5).unwrap();
        let cw = crate::hf_chat::encode(p5, &b5[0]);
        assert_eq!(cw, hex_bits("104a4b5af2fb8e00146a22a000000000221212062485b0d80085214738f000000000000000000000000000bb1d797dad7034cfd485cbe372fd6c768e6c768e34f4dc38bcf838bcf800bcaa850c7295ba69939e7b11debf83d404fbf79897c1b997d537c6c0348480302c9ab8a29aaca29aac9a9afe9a9afedecef3ad41f7d9431b5dc31319971e9d29016fd25b0419b68590b285d49685d496df266ddf3764df3764db1e6cdb1e6cdb1e6c1f33ea5f12a20d02aa"));
        let x = burst(p5, &b5);
        assert_eq!(x.len(), 13_200);
        let pw = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
        assert!((pw - 0.999_774).abs() < 1e-4, "{pw}");
        assert!(near(x[8400], -0.577_350, -0.288_675), "{}", x[8400]);
    }

    #[test]
    fn audio_round_trip() {
        let p3 = &PROFILES[3];
        let chat = crate::hf_chat::frame::tests_support::chat().bits().unwrap();
        let x = burst(p3, &blocks(&chat, 1, p3).unwrap());
        let a = to_audio(&x, 0.9);
        assert_eq!(a.len(), 5 * x.len());
        assert!((a.iter().fold(0f32, |m, v| m.max(v.abs())) - 0.9).abs() < 1e-6);
        let y = from_audio(&a);
        assert_eq!(y.len(), x.len());
        // The same signal, at another level, away from the edges of the band and the ends of the burst.
        let (mut c, mut e1, mut e2) = (Complex32::new(0.0, 0.0), 0f32, 0f32);
        for i in 9000..40_000 {
            c += y[i] * x[i].conj();
            e1 += x[i].norm_sqr();
            e2 += y[i].norm_sqr();
        }
        assert!(c.norm() / (e1 * e2).sqrt() > 0.97, "{}", c.norm() / (e1 * e2).sqrt());
    }
}
