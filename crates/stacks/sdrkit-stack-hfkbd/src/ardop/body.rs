//! The 4PSK, 8PSK and 16QAM data frames of ARDOP at 100 baud on 1, 2, 4 or 8 carriers: the carrier frequencies, the symbol
//! wave, the bits of a carrier's block as phase steps, and the reading of a body: the per-carrier phase steps after the phase
//! start symbol, the decisions, the good-carrier rule, the combining of repeats and the quality value.

use super::{frame_check, FrameType, RsCodes};
use sdrkit_core::Complex32;
use sdrkit_util_bits::{BitReader, BitWriter};
use std::f64::consts::{PI, TAU};

/// The rate at which the symbol formula counts its samples, per second.
pub const FORMULA_RATE: f64 = 12_000.0;
/// One body symbol, seconds.
pub const SYMBOL_S: f64 = 0.01;
/// The trailer symbols that end a frame with a body (a trailer of 20 ms).
pub const TRAILER_SYMBOLS: usize = 3;
/// The peak of one carrier at full amplitude, in 16-bit units.
pub const CARRIER_PEAK: f64 = 26_000.0;
/// The peak of the leader and the frame-type tones, in 16-bit units: the sending level.
pub const LEADER_PEAK: f64 = 28_600.0;

/// The modulation of a data body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    /// Differential 4PSK, 2 bits a symbol.
    Psk4,
    /// Differential 8PSK, 3 bits a symbol.
    Psk8,
    /// Eight differential phases on two rings of radius 2:1, 4 bits a symbol.
    Qam16,
}

impl Body {
    /// The body modulation of a data frame type, `None` for 4FSK and control frames.
    pub fn of(ft: &FrameType) -> Option<Body> {
        if ft.kind != "data" {
            return None;
        }
        match ft.modulation {
            "4PSK" => Some(Body::Psk4),
            "8PSK" => Some(Body::Psk8),
            "16QAM" => Some(Body::Qam16),
            _ => None,
        }
    }

    /// Bits a symbol.
    pub fn bits(self) -> u32 {
        match self {
            Body::Psk4 => 2,
            Body::Psk8 => 3,
            Body::Qam16 => 4,
        }
    }

    /// The spacing of the decision lattice, degrees: 90 for 4PSK, 45 for 8PSK and 16QAM.
    pub fn lattice_deg(self) -> f64 {
        if self == Body::Psk4 {
            90.0
        } else {
            45.0
        }
    }

    /// The carrier scaling factor for `carriers` carriers.
    pub fn scale(self, carriers: u32) -> f64 {
        let qam = self == Body::Qam16;
        match carriers {
            1 => 1.2,
            2 if qam => 0.67,
            2 => 0.65,
            4 => 0.4,
            _ if qam => 0.27,
            _ => 0.25,
        }
    }
}

/// The audio frequencies of the carriers of a frame with `carriers` carriers, lowest first. 1500 Hz for one carrier (and for
/// any count that is not 2, 4 or 8).
pub fn carrier_hz(carriers: u32) -> &'static [f64] {
    match carriers {
        2 => &[1400.0, 1600.0],
        4 => &[1200.0, 1400.0, 1600.0, 1800.0],
        8 => &[
            800.0, 1000.0, 1200.0, 1400.0, 1600.0, 1800.0, 2000.0, 2200.0,
        ],
        _ => &[1500.0],
    }
}

/// The data symbols of each carrier of frame type `ft` (the phase start symbol not counted); 0 for a frame with no PSK or
/// QAM body.
pub fn symbols(ft: &FrameType) -> usize {
    Body::of(ft).map_or(0, |b| ft.block_len() * 8 / b.bits() as usize)
}

/// The block of one carrier of frame type `code`: the count, `part` padded with zeros to the frame's data size, the CRC-16 high
/// byte, the CRC-16 low byte XOR `code`, then the Reed-Solomon parity. `part` is cut to the data size.
pub fn block(rs: &mut RsCodes, ft: &FrameType, code: u8, part: &[u8]) -> Vec<u8> {
    let n = part.len().min(ft.data);
    let mut b = Vec::with_capacity(ft.block_len());
    b.push(n as u8);
    b.extend_from_slice(&part[..n]);
    b.resize(1 + ft.data, 0);
    let crc = frame_check(&b);
    b.push((crc >> 8) as u8);
    b.push((crc as u8) ^ code);
    let p = rs.parity(&b, ft.rs);
    b.extend(p);
    b
}

/// The symbols of one carrier's block, one digit each: the phase index p (45° steps) after the group's step, plus 8 when the
/// symbol is on the half ring. The block is read as one bit stream, bit 7 of each byte first, in groups of [`Body::bits`] bits.
/// The phase before the first symbol is 0 (the phase start symbol). 4PSK steps 2v, 8PSK v, 16QAM (v AND 7) with the ring bit
/// v AND 8 set for that symbol alone.
pub fn digits(body: Body, block: &[u8]) -> Vec<u8> {
    let b = body.bits();
    let n = block.len() * 8 / b as usize;
    let mut r = BitReader::new(block);
    let mut p = 0u8;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let v = r.u(b) as u8;
        let (step, half) = match body {
            Body::Psk4 => (2 * v, 0),
            Body::Psk8 => (v, 0),
            Body::Qam16 => (v & 7, v & 8),
        };
        p = (p + step) & 7;
        out.push(p | half);
    }
    out
}

/// One carrier at `hz` in a symbol of digit `digit` (as [`digits`] gives it), `tau` formula samples (12 000 a second) after
/// the symbol start: a × sin(π τ / 119) × sin(p π / 4 + 2π f τ / 12 000) as its analytic signal (−j times the complex tone),
/// with a = 1, or ½ on the half ring. Zero outside 0 ≤ τ ≤ 119. Returns (real, imaginary) with the full-ring peak 1.
pub fn carrier_wave(hz: f64, digit: u8, tau: f64) -> (f64, f64) {
    if !(0.0..=119.0).contains(&tau) {
        return (0.0, 0.0);
    }
    let a = if digit & 8 != 0 { 0.5 } else { 1.0 };
    let env = a * (PI * tau / 119.0).sin();
    let th = f64::from(digit & 7) * PI / 4.0 + TAU * hz * tau / FORMULA_RATE;
    let (s, c) = th.sin_cos();
    (env * s, -env * c)
}

/// The DFT gain of the receiver's window for one symbol of `n` samples at peak 1: Σ hann(i) × the half-sine envelope.
fn window_gain(n: usize) -> f64 {
    let m = (n - 1) as f64;
    (0..n)
        .map(|i| {
            let x = i as f64 / m;
            (0.5 - 0.5 * (TAU * x).cos()) * (PI * x).sin()
        })
        .sum()
}

/// Measures a body on baseband samples centred on 1500 Hz. `x` starts at the phase start symbol, whose first sample has the
/// absolute index `index` (the phase of every carrier is counted from index 0, so a carrier keeps one phase origin over the
/// symbols); `sps` samples a symbol at `rate`; `offset_hz` is the tuning error. Returns for each carrier the complex value at
/// its frequency of every whole symbol in `x`, Hann-weighted, the phase start symbol first.
pub fn measure(
    x: &[Complex32],
    index: u64,
    rate: f64,
    carriers: u32,
    offset_hz: f64,
    out: &mut Vec<Vec<Complex32>>,
) {
    let sps = (rate * SYMBOL_S).round() as usize;
    let nsym = x.len() / sps.max(1);
    let hann: Vec<f64> = (0..sps)
        .map(|i| 0.5 - 0.5 * (TAU * i as f64 / (sps - 1) as f64).cos())
        .collect();
    out.clear();
    for &hz in carrier_hz(carriers) {
        let f = hz - 1500.0 + offset_hz;
        let step = -TAU * f / rate;
        let rot = (step.cos(), step.sin());
        let mut v = Vec::with_capacity(nsym);
        for k in 0..nsym {
            let a = k * sps;
            let start = index + a as u64;
            // The phase at the window start, counted in cycles to keep the precision.
            let cyc = (f * start as f64 / rate).rem_euclid(1.0);
            let ph0 = -TAU * cyc;
            let (mut c, mut s) = (ph0.cos(), ph0.sin());
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, &y) in x[a..a + sps].iter().enumerate() {
                let w = hann[i];
                let (yr, yi) = (f64::from(y.re), f64::from(y.im));
                re += w * (yr * c - yi * s);
                im += w * (yr * s + yi * c);
                let nc = c * rot.0 - s * rot.1;
                s = c * rot.1 + s * rot.0;
                c = nc;
            }
            v.push(Complex32::new(re as f32, im as f32));
        }
        out.push(v);
    }
}

/// The magnitude a carrier of frame type `ft` has in [`measure`] at full amplitude, when the frame-type tones have the
/// amplitude `tone` and the symbol has `sps` samples.
pub fn expected_magnitude(ft: &FrameType, tone: f64, sps: usize) -> f64 {
    let s = Body::of(ft).map_or(1.0, |b| b.scale(ft.carriers));
    tone * CARRIER_PEAK / LEADER_PEAK * s * window_gain(sps)
}

/// What one copy of a body gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyRead {
    /// Every carrier is good, in this copy or a copy before.
    pub good: bool,
    /// The data of carriers 0, 1, ... joined in order, each cut to its count, when `good`.
    pub data: Vec<u8>,
    /// The Reed-Solomon corrections of the carriers found good in this copy.
    pub corrections: u32,
    /// The carriers good so far.
    pub good_carriers: usize,
    /// The quality from the highest carrier, 0 to 100.
    pub quality: u32,
}

/// What is kept of one carrier between copies of a frame.
#[derive(Clone, Debug, Default)]
struct Kept {
    /// The data of a carrier found good.
    data: Option<Vec<u8>>,
    copies: u32,
    /// The phase steps, degrees, averaged over the copies on the circle.
    deltas: Vec<f32>,
    /// The magnitudes of the data symbols, the mean over the copies.
    mags: Vec<f32>,
    /// The magnitude of the phase start symbol, the mean over the copies.
    first: f32,
}

/// The reader of PSK and QAM bodies with its memory of repeats: a carrier found good stays good, and the phase steps and
/// magnitudes of the others are averaged over the copies of one frame type. The memory is cleared by a frame of another type
/// code ([`Repeats::seen`]; so a change of the even/odd toggle clears it), after a good frame, and by [`Repeats::clear`]. It
/// has no age limit.
#[derive(Debug, Default)]
pub struct Repeats {
    code: Option<u8>,
    carriers: Vec<Kept>,
    bytes: Vec<u8>,
}

impl Repeats {
    /// Nothing kept.
    pub fn new() -> Self {
        Self::default()
    }

    /// Notes a frame of type `code` heard: the copies kept are forgotten when their code differs.
    pub fn seen(&mut self, code: u8) {
        if self.code.is_some_and(|c| c != code) {
            self.clear();
        }
    }

    /// Forgets every copy.
    pub fn clear(&mut self) {
        self.code = None;
        self.carriers.clear();
    }

    /// Reads one copy of a body of frame type `ft` from the measurements of [`measure`] (per carrier: the phase start symbol,
    /// then the data symbols). Phase steps are taken after removing their common mean error (a residual tuning error turns every
    /// step by the same angle), found from the steps raised to the power of the lattice size. Each carrier not yet good is
    /// decided on its values averaged over the copies, corrected by Reed-Solomon, and is good when the decoder succeeds, the
    /// count is at most the data size and the CRC matches. The frame is good when every carrier is.
    pub fn read(&mut self, ft: &FrameType, rs: &mut RsCodes, x: &[Vec<Complex32>]) -> BodyRead {
        let Some(body) = Body::of(ft) else {
            return BodyRead::default();
        };
        let nc = ft.carriers as usize;
        let nsym = symbols(ft);
        if x.len() != nc || x.iter().any(|v| v.len() < nsym + 1) {
            return BodyRead::default();
        }
        if self.code != Some(ft.code) || self.carriers.len() != nc {
            self.code = Some(ft.code);
            self.carriers = vec![Kept::default(); nc];
        }
        let bias = common_bias(body, x, nsym);
        let mut read = BodyRead::default();
        let blen = ft.block_len();
        for (c, xs) in x.iter().enumerate() {
            let (deltas, mags) = steps(&xs[..=nsym], bias);
            if c == nc - 1 {
                read.quality = quality(body, &deltas[1..], &mags[1..]);
            }
            let k = &mut self.carriers[c];
            if k.data.is_some() {
                continue;
            }
            let first = xs[0].norm();
            if k.copies == 0 {
                k.deltas = deltas;
                k.mags = mags;
                k.first = first;
            } else {
                let n = (k.copies + 1) as f32;
                for (d, nd) in k.deltas.iter_mut().zip(&deltas) {
                    let (a, b) = (d.to_radians(), nd.to_radians());
                    *d = (a.sin() + b.sin()).atan2(a.cos() + b.cos()).to_degrees();
                }
                for (m, nm) in k.mags.iter_mut().zip(&mags) {
                    *m += (nm - *m) / n;
                }
                k.first += (first - k.first) / n;
            }
            k.copies += 1;
            self.bytes.clear();
            decide(body, &k.deltas, &k.mags, k.first, &mut self.bytes);
            self.bytes.truncate(blen);
            if self.bytes.len() < blen {
                continue;
            }
            let Some(fixed) = rs.correct(&mut self.bytes, ft.rs) else {
                continue;
            };
            let count = self.bytes[0] as usize;
            let crc = frame_check(&self.bytes[..=ft.data]);
            let crc_ok = self.bytes[ft.data + 1] == (crc >> 8) as u8
                && self.bytes[ft.data + 2] == (crc as u8) ^ ft.code;
            if count <= ft.data && crc_ok {
                k.data = Some(self.bytes[1..=count].to_vec());
                read.corrections += fixed as u32;
            }
        }
        read.good_carriers = self.carriers.iter().filter(|k| k.data.is_some()).count();
        read.good = read.good_carriers == nc;
        if read.good {
            for k in &self.carriers {
                read.data
                    .extend_from_slice(k.data.as_deref().unwrap_or_default());
            }
            self.clear();
        }
        read
    }
}

/// The common error of the phase steps of every carrier, degrees: the angle of Σ z^M / |z|^(M−1) over the steps z, divided by
/// M, the lattice size (4 for 4PSK, 8 otherwise).
fn common_bias(body: Body, x: &[Vec<Complex32>], nsym: usize) -> f32 {
    let m = (360.0 / body.lattice_deg()) as f32;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for xs in x {
        for w in xs[..=nsym].windows(2) {
            let z = w[1] * w[0].conj();
            let (r, a) = (z.norm(), z.arg());
            re += f64::from(r * (m * a).cos());
            im += f64::from(r * (m * a).sin());
        }
    }
    if re == 0.0 && im == 0.0 {
        return 0.0;
    }
    (im.atan2(re) as f32 / m).to_degrees()
}

/// The phase steps (degrees, wrapped to (−180, 180], less `bias`) and the magnitudes of the data symbols of one carrier, from
/// its values with the phase start symbol first.
fn steps(xs: &[Complex32], bias: f32) -> (Vec<f32>, Vec<f32>) {
    let mut d = Vec::with_capacity(xs.len());
    let mut m = Vec::with_capacity(xs.len());
    for w in xs.windows(2) {
        d.push(wrap((w[1] * w[0].conj()).arg().to_degrees() - bias));
        m.push(w[1].norm());
    }
    (d, m)
}

/// An angle in degrees wrapped to (−180, 180].
fn wrap(a: f32) -> f32 {
    let w = a.rem_euclid(360.0);
    if w > 180.0 {
        w - 360.0
    } else {
        w
    }
}

/// The bytes of a carrier from its phase steps and magnitudes. 4PSK: round(Δ / 90°) mod 4; 8PSK: round(Δ / 45°) mod 8;
/// 16QAM: round(Δ / 45°) mod 8 plus 8 when the magnitude is below θ, which starts at 0.75 × the phase start symbol's
/// magnitude and moves by θ ← 0.9 θ + 0.15 |X| after an inner and 0.9 θ + 0.075 |X| after an outer decision. Groups are
/// written most significant bit first.
fn decide(body: Body, deltas: &[f32], mags: &[f32], first: f32, out: &mut Vec<u8>) {
    let step = body.lattice_deg() as f32;
    let n = (360.0 / step) as i32;
    let mut th = 0.75 * first;
    let mut w = BitWriter::new(out);
    for (&d, &m) in deltas.iter().zip(mags) {
        let v = ((d / step).round() as i32).rem_euclid(n) as u64;
        let v = match body {
            Body::Qam16 => {
                if m < th {
                    th = 0.9 * th + 0.15 * m;
                    v | 8
                } else {
                    th = 0.9 * th + 0.075 * m;
                    v
                }
            }
            _ => v,
        };
        w.put(v, body.bits());
    }
}

/// The quality, 0 to 100, from the phase steps and magnitudes of the data symbols (the first left out by the caller).
/// PSK: 100 − 200 × mean |e| / step, e the distance of each step from the nearest lattice angle. 16QAM: that times
/// 1 − (E_in / (N_in R_in) + E_out / (N_out R_out)), the symbols cut into rings at 0.75 × the largest magnitude, R the mean
/// radius of a ring, N its symbols and E the sum of |radius − R| in it.
pub fn quality(body: Body, deltas: &[f32], mags: &[f32]) -> u32 {
    if deltas.is_empty() {
        return 0;
    }
    let step = body.lattice_deg() as f32;
    let e: f32 = deltas
        .iter()
        .map(|&d| (d - (d / step).round() * step).abs())
        .sum::<f32>()
        / deltas.len() as f32;
    let mut q = 100.0 - 200.0 * e / step;
    if body == Body::Qam16 {
        let top = mags.iter().cloned().fold(0.0f32, f32::max);
        let cut = 0.75 * top;
        let spread = |inner: bool| -> f32 {
            let ring: Vec<f32> = mags
                .iter()
                .copied()
                .filter(|&m| (m < cut) == inner)
                .collect();
            if ring.is_empty() {
                return 0.0;
            }
            let r = ring.iter().sum::<f32>() / ring.len() as f32;
            if r <= 0.0 {
                return 0.0;
            }
            ring.iter().map(|m| (m - r).abs()).sum::<f32>() / (ring.len() as f32 * r)
        };
        q *= 1.0 - (spread(true) + spread(false));
    }
    q.max(0.0).round() as u32
}
