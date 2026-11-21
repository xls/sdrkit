//! Argos PTT-A2: the message layout, the lengths, and the coherent demodulator for a burst found by the burst finder: the
//! timing and the length come from the size of the soft bits, the decision from their quality.

use crate::modes::sarsat::coherent::{fine_carrier, mix_and_decimate};
use crate::modes::sarsat::fgb;
use sdrkit_algo_modem::pm::PmBurst;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};

pub const BIT_RATE: f64 = 400.0;
/// Phase deviation of the bi-phase modulation (rad).
pub const DEV: f64 = 1.1;

/// The A2 message lengths: 80 + 32 k bits, k = 0 to 7.
pub fn lengths() -> [usize; 8] {
    [80, 112, 144, 176, 208, 240, 272, 304]
}

/// The A2 length nearest `n` within 3 bits.
pub fn nearest_length(n: usize) -> Option<usize> {
    lengths().into_iter().find(|&l| l.abs_diff(n) <= 3)
}

/// A decoded A2 message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct A2 {
    pub bits: usize,
    /// The 24 bits of bit and format synchronisation, as received (hex).
    pub sync_hex: String,
    pub length_field: u8,
    /// 28-bit platform id (7 hex digits) and its first 20 bits.
    pub id: u32,
    pub id_hex: String,
    pub id20_hex: String,
    pub data_bits: usize,
    pub data_hex: String,
    pub raw_hex: String,
}

fn value(b: &[bool]) -> u64 {
    b.iter().fold(0u64, |v, &x| (v << 1) | x as u64)
}

fn hex_of_bits(b: &[bool]) -> String {
    let mut s = String::new();
    for c in b.chunks(4) {
        let mut v = 0;
        for (i, &x) in c.iter().enumerate() {
            v |= (x as u32) << (3 - i);
        }
        s.push(
            char::from_digit(v, 16)
                .expect("a value below 16")
                .to_ascii_uppercase(),
        );
    }
    s
}

/// Reads an A2 message of one of the A2 lengths.
pub fn parse(bits: &[bool]) -> Option<A2> {
    if !lengths().contains(&bits.len()) {
        return None;
    }
    let id = value(&bits[28..56]) as u32;
    Some(A2 {
        bits: bits.len(),
        sync_hex: hex_of_bits(&bits[..24]),
        length_field: value(&bits[24..28]) as u8,
        id,
        id_hex: format!("{id:07X}"),
        id20_hex: format!("{:05X}", id >> 8),
        data_bits: bits.len() - 56,
        data_hex: hex_of_bits(&bits[56..]),
        raw_hex: hex_of_bits(bits),
    })
}

/// Builds the bits of an A2 message (the generator's stand-in sync 0x5A5A5A, the length field k).
pub fn build(id: u32, k: usize, data: &[bool]) -> Vec<bool> {
    let mut b = Vec::with_capacity(80 + 32 * k);
    for i in (0..24).rev() {
        b.push((0x5A5A5Au32 >> i) & 1 == 1);
    }
    for i in (0..4).rev() {
        b.push((k >> i) & 1 == 1);
    }
    for i in (0..28).rev() {
        b.push((id >> i) & 1 == 1);
    }
    let n = 24 + 32 * k;
    for i in 0..n {
        b.push(data.get(i).copied().unwrap_or(false));
    }
    b
}

/// What the demodulator found in a burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Demodulated {
    /// Fine carrier, Hz from the finder's centre.
    pub carrier_hz: f64,
    /// Carrier length and the whole burst, ms.
    pub carrier_ms: f64,
    pub total_ms: f64,
    /// The quality of the coherent soft bits of the message: `(mean |y|)^2 / mean y^2` (1 without noise, 0.64 for noise
    /// alone).
    pub quality: f64,
    pub message: Option<A2>,
    /// Bits of the message.
    pub raw_bits: usize,
}

/// The working rate of the demodulator.
const FS: f64 = fgb::RATE / 4.0;
/// Samples of one bit.
const BIT: f64 = FS / BIT_RATE;
/// The message starts this long after the line appeared, seconds: 160 ms, less a line found up to 2 frames late, plus one
/// placed up to 4 frames early.
const START_S: (f64, f64) = (0.060, 0.300);
/// The carrier may drift this fast in a burst, Hz/s.
const MAX_DRIFT_HZ_S: f64 = 80.0;
/// The bit rate errors searched.
const RATES: [f64; 5] = [-0.005, -0.0025, 0.0, 0.0025, 0.005];
/// The step of the start search, samples.
const SEARCH_STEP: f64 = 2.0;
/// The start search: this many bits after the start are modulated, this many before are not.
const LEAD_BITS: usize = 24;
const QUIET_BITS: usize = 12;
/// A bit counts for the message when its soft value is over this share of the mean of the message's.
const MESSAGE_SHARE: f64 = 0.5;
/// The first level of a message is the mean of this many largest soft values.
const TOP_BITS: usize = 40;
/// Bits either side of the start that measure the carrier's levels, and bits next to the start left out of them.
const LEVEL_BITS: usize = 12;
const LEVEL_SKIP: usize = 2;
/// The refinement: half samples and rate steps of 0.05 % either side.
const REFINE_HALF_SAMPLES: i32 = 8;
const REFINE_RATE_STEPS: i32 = 5;
const REFINE_RATE_STEP: f64 = 0.0005;
/// A message needs this quality.
const MIN_QUALITY: f64 = 0.76;
/// The end of a message is seen when this many bits of samples follow it.
const SEEN_END_BITS: usize = 4;

/// The carrier's step at the start of the message, for the fit: the bit sums fall from the bare carrier to `cos(1.1)` of it.
#[derive(Clone, Copy)]
struct Step {
    /// The level halfway between the bit sums before and after the start.
    mid: f64,
    /// The fall of the level over the mean soft value of the message.
    weight: f64,
}

/// A burst at 32 kHz, at 0 Hz, in the coherent detector, with the soft values of the last timing read.
struct Burst {
    pm: PmBurst,
    /// Prefix sums of the samples.
    cum: Vec<Complex32>,
    /// Soft values, positive for a 1 (the first half of the bit at -1.1 rad), in the units of the half-bit sums.
    v: Vec<f32>,
    /// Prefix sums of `|v|`.
    abs: Vec<f64>,
    /// Prefix sums of the sizes of the bit sums of the last grid read by [`Burst::carrier`].
    car: Vec<f64>,
}

impl Burst {
    /// Reads `n` bits from `t0` with period `t` into `v` and `abs`; bits outside the samples read 0.
    fn soft(&mut self, t0: f64, t: f64, n: usize) {
        self.v.clear();
        self.v.resize(n, 0.0);
        match self.pm.soft_bits(t0, t, &mut self.v) {
            // Back from LLRs (positive for a 0 of the detector: the first half at the negative phase) to the soft values.
            Ok(st) if st.m1 > 0.0 => {
                let k = st.var / (2.0 * st.m1);
                self.v.iter_mut().for_each(|v| *v *= k);
            }
            _ => self.v.fill(0.0),
        }
        self.abs.clear();
        self.abs.push(0.0);
        let mut acc = 0.0;
        for &v in &self.v {
            acc += f64::from(v.abs());
            self.abs.push(acc);
        }
    }

    /// Reads the sizes of the sums of `n` bits from `t0` with period `t` into `car`.
    fn carrier(&mut self, t0: f64, t: f64, n: usize) {
        let last = self.cum.len() - 1;
        let at = |p: f64| (p.round().max(0.0) as usize).min(last);
        self.car.clear();
        self.car.push(0.0);
        let mut acc = 0.0;
        for k in 0..n {
            let (a, b) = (at(t0 + k as f64 * t), at(t0 + (k + 1) as f64 * t));
            acc += f64::from((self.cum[b] - self.cum[a]).norm());
            self.car.push(acc);
        }
    }

    /// The bits that lie wholly in the samples from `s` with period `t`.
    fn bits_from(&self, s: f64, t: f64) -> usize {
        ((self.pm.len() as f64 - s) / t).max(0.0) as usize
    }

    /// The start and period of the message from its first bits: starts every 2 samples from 60 to 300 ms after `st`, rates
    /// within 0.5 %; the largest sum of `|y|` over the 24 bits after the start less that over the 12 bits before.
    fn start(&mut self, st: usize) -> (f64, f64) {
        let lo = st as f64 + START_S.0 * FS;
        let hi = st as f64 + START_S.1 * FS;
        let mut best = (f64::MIN, lo, BIT);
        for e in RATES {
            let t = BIT * (1.0 + e);
            let span = ((hi - lo) / t).ceil() as usize;
            let mut o = 0.0;
            while o < t {
                let b0 = lo + o - QUIET_BITS as f64 * t;
                let n = self.bits_from(b0, t).min(QUIET_BITS + span + LEAD_BITS);
                self.soft(b0, t, n);
                for m in QUIET_BITS..(n.saturating_sub(LEAD_BITS) + 1).min(QUIET_BITS + span) {
                    let score =
                        self.abs[m + LEAD_BITS] - 2.0 * self.abs[m] + self.abs[m - QUIET_BITS];
                    if score > best.0 {
                        best = (score, b0 + m as f64 * t, t);
                    }
                }
                o += SEARCH_STEP;
            }
        }
        (best.1, best.2)
    }

    /// The start, period and length of the message: starts every 2 samples from 60 to 300 ms after `st`, rates within 0.5 %,
    /// every A2 length that fits the samples. The best gathers the largest sum of `|y| - level / 2` over its bits (a bit of
    /// the message adds to the sum, a bit of noise takes from it), where `level` is the mean `|y|` of a message; with
    /// `step`, also the largest weighted sum of the bit sums less `mid` over the bits before the start.
    fn fit(&mut self, st: usize, level: f64, step: Option<Step>) -> Option<(f64, f64, usize)> {
        let lo = st as f64 + START_S.0 * FS;
        let hi = st as f64 + START_S.1 * FS;
        let gate = MESSAGE_SHARE * level;
        let mut best: Option<(f64, f64, f64, usize)> = None;
        for e in RATES {
            let t = BIT * (1.0 + e);
            let span = ((hi - lo) / t).ceil() as usize;
            let mut o = 0.0;
            while o < t {
                let b0 = lo + o - QUIET_BITS as f64 * t;
                let n = self.bits_from(b0, t).min(QUIET_BITS + span + 304);
                self.soft(b0, t, n);
                if step.is_some() {
                    self.carrier(b0, t, n);
                }
                for m in QUIET_BITS..(QUIET_BITS + span).min(n + 1) {
                    let before = step.map_or(0.0, |s| s.weight * (self.car[m] - s.mid * m as f64));
                    for l in lengths() {
                        if m + l > n {
                            break;
                        }
                        let j = self.abs[m + l] - self.abs[m] - gate * l as f64 + before;
                        if best.is_none_or(|b| j > b.0) {
                            best = Some((j, b0 + m as f64 * t, t, l));
                        }
                    }
                }
                o += SEARCH_STEP;
            }
        }
        best.map(|b| (b.1, b.2, b.3))
    }

    /// The mean of the 40 largest `|y|` of the bits on the grid of the start `s` with period `t` (40 bits before `s` to the
    /// end of the samples): not under the level of a message, whatever its start and length.
    fn strongest(&mut self, s: f64, t: f64) -> f64 {
        let b0 = s - TOP_BITS as f64 * t;
        let n = self.bits_from(b0, t);
        self.soft(b0, t, n);
        let mut a: Vec<f32> = self.v.iter().map(|v| v.abs()).collect();
        a.sort_by(|x, y| y.total_cmp(x));
        let top = &a[..TOP_BITS.min(a.len())];
        top.iter().map(|&v| f64::from(v)).sum::<f64>() / top.len().max(1) as f64
    }

    /// The mean `|y|` of the `l` bits from `s` with period `t`, and the carrier's step at `s`: the mean bit sums of the 12
    /// bits before and after the start, leaving out the 2 next to it.
    fn levels(&mut self, s: f64, t: f64, l: usize) -> (f64, Step) {
        self.soft(s, t, l);
        let level = self.abs[l] / l as f64;
        let span = LEVEL_SKIP + LEVEL_BITS;
        self.carrier(s - span as f64 * t, t, 2 * span);
        let before = (self.car[LEVEL_BITS] - self.car[0]) / LEVEL_BITS as f64;
        let after = (self.car[2 * span] - self.car[span + LEVEL_SKIP]) / LEVEL_BITS as f64;
        let step = Step {
            mid: 0.5 * (before + after),
            weight: if level > 0.0 {
                ((before - after) / level).clamp(0.0, 1.0)
            } else {
                0.0
            },
        };
        (level, step)
    }

    /// The timing within 4 samples (in steps of 0.5) and 0.25 % (in steps of 0.05 %) of `(s, t)` whose `l` bits gather the
    /// largest sum of `|y|`.
    fn refine(&mut self, s: f64, t: f64, l: usize) -> (f64, f64) {
        let mut best = (f64::MIN, s, t);
        for ds in -REFINE_HALF_SAMPLES..=REFINE_HALF_SAMPLES {
            for dr in -REFINE_RATE_STEPS..=REFINE_RATE_STEPS {
                let s1 = s + 0.5 * f64::from(ds);
                let t1 = t * (1.0 + REFINE_RATE_STEP * f64::from(dr));
                if self.bits_from(s1, t1) < l {
                    continue;
                }
                self.soft(s1, t1, l);
                if self.abs[l] > best.0 {
                    best = (self.abs[l], s1, t1);
                }
            }
        }
        (best.1, best.2)
    }
}

/// Demodulates a burst (`x` at the finder's rate, the line `coarse` Hz from 0, the line appeared at sample `start`).
///
/// The burst goes to 0 Hz at 32 kHz (its drift within 80 Hz/s removed) and is detected coherently against its own carrier
/// ([`sdrkit_algo_modem::pm::PmBurst`]). The level of the soft bits comes from the first bits (24 modulated bits after 12
/// that are not); the start, the rate and the A2 length come together from the whole message and the fall of the carrier
/// at its start, refined in fine steps. The message is taken when
/// the quality of its soft bits is at least 0.76 and, where the samples end with the message, the length field does not ask
/// for more bits.
pub fn demodulate(x: &[Complex32], coarse: f64, start: usize) -> Demodulated {
    let mut y = Vec::with_capacity(x.len() / 4 + 1);
    mix_and_decimate(x, coarse, &mut y);
    let st = start / 4;
    let fine = coarse + fine_carrier(&mut y, st, MAX_DRIFT_HZ_S).0;
    let mut cum = Vec::with_capacity(y.len() + 1);
    let mut acc = Complex32::new(0.0, 0.0);
    cum.push(acc);
    for &v in &y {
        acc += v;
        cum.push(acc);
    }
    let mut b = Burst {
        pm: PmBurst::new(),
        cum,
        v: Vec::new(),
        abs: Vec::new(),
        car: Vec::new(),
    };
    b.pm.load(&y);
    let mut d = Demodulated {
        carrier_hz: fine,
        ..Default::default()
    };
    // The level of the message from the strongest bits at the timing of its first bits, then from the fit of the whole
    // message, then the fit with the carrier's step.
    let (s0, t0) = b.start(st);
    d.carrier_ms = (s0 - st as f64) / FS * 1e3;
    if b.bits_from(s0, t0) < 80 {
        return d;
    }
    let level = b.strongest(s0, t0);
    let Some((s, t, l)) = b.fit(st, level, None) else {
        return d;
    };
    let (level, step) = b.levels(s, t, l);
    let Some((s, t, l)) = b.fit(st, level, Some(step)) else {
        return d;
    };
    let (s, t) = b.refine(s, t, l);
    b.soft(s, t, l);
    let sum2: f64 = b.v.iter().map(|&v| f64::from(v) * f64::from(v)).sum();
    d.carrier_ms = (s - st as f64) / FS * 1e3;
    d.total_ms = d.carrier_ms + l as f64 * t / FS * 1e3;
    d.raw_bits = l;
    d.quality = if sum2 > 0.0 {
        b.abs[l] * b.abs[l] / (l as f64 * sum2)
    } else {
        0.0
    };
    let bits: Vec<bool> = b.v.iter().map(|&v| v > 0.0).collect();
    // A burst whose samples end with the message may be cut: the length field then must not ask for more.
    let asked = 80 + 32 * value(&bits[24..28]) as usize;
    let cut = b.bits_from(s, t) < l + SEEN_END_BITS && asked > l;
    if d.quality >= MIN_QUALITY && !cut {
        d.message = parse(&bits);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_lengths_and_fields() {
        assert_eq!(nearest_length(80), Some(80));
        assert_eq!(nearest_length(82), Some(80));
        assert_eq!(nearest_length(110), Some(112));
        assert_eq!(nearest_length(96), None);
        assert_eq!(nearest_length(400), None);
        assert!(parse(&[false; 81]).is_none());
        for k in 0..8 {
            let data: Vec<bool> = (0..24 + 32 * k).map(|i| i % 3 == 0).collect();
            let b = build(0x0ABCDEF + k as u32, k, &data);
            assert_eq!(b.len(), 80 + 32 * k);
            let m = parse(&b).unwrap();
            assert_eq!(m.id, 0x0ABCDEF + k as u32);
            assert_eq!(m.length_field as usize, k);
            assert_eq!(m.sync_hex, "5A5A5A");
            assert_eq!(m.data_bits, 24 + 32 * k);
            assert_eq!(m.id_hex.len(), 7);
        }
    }
}
