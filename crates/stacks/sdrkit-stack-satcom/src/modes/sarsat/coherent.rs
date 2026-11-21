//! The coherent first-generation demodulator: timings from a sync search on coherent soft bits, the soft bits of the
//! split-phase burst at each timing, the sync word and the polarity read from them, and Chase decoding of the BCH-1 word with the guards of the message.
//!
//! Standard: C/S T.001 section 2.2 (signal), Annex B (BCH-1 (82, 61), BCH-2 (38, 26)), section 3 (country code, bits 27 to 36).

use crate::modes::sarsat::t001::{self, Fgb};
use sdrkit_algo_fec::bch::{codes, Bch};
use sdrkit_algo_fec::chase::{ChaseDecoder, DEFAULT_MAX_FAILED};
use sdrkit_algo_modem::pm::{PmBurst, BURST_PREAMBLE_BITS};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::ops::Range;

/// The bits of a long message; a short one has 112.
pub const BITS: usize = 144;
/// The bits of a short message.
const SHORT: usize = 112;
/// The bit sync and the frame sync.
const SYNC: usize = 24;
/// PDF-1 and BCH-1: bits 25 to 106.
const PDF1: Range<usize> = 24..106;
/// The weakest bits of the BCH-1 word that Chase decoding flips.
const WEAKEST: usize = 7;
/// A repaired message needs this margin to the next code word.
const MIN_MARGIN: f32 = 1.5;
/// A repaired message may cost this much per repaired bit.
const MAX_COST_PER_BIT: f32 = 1.0;
/// Sync bits that must match to decode.
const SYNC_TO_DECODE: u32 = 23;
/// Sync bits that must match to keep a timing for repeat combining.
const SYNC_TO_KEEP: u32 = 20;
/// Sync bits of the search that make a timing candidate.
const SYNC_TO_TRY: u32 = 19;
/// Timing candidates tried.
const MAX_CANDIDATES: usize = 16;
/// The step of the sync search, samples.
const SEARCH_STEP: f64 = 2.0;
/// Timing candidates lie at least this many samples apart.
const CANDIDATE_SPACING: f64 = 8.0;
/// The refinement of a candidate: samples either side, and rate steps either side of 0.125 %.
const REFINE_SAMPLES: i32 = 8;
const REFINE_RATE_STEPS: i32 = 4;
const REFINE_RATE_STEP: f64 = 0.00125;
/// Timings kept for repeat combining.
const KEEP_TIMINGS: usize = 6;
/// The bare carrier before the sync: the mean cosine of the half-bit angle, and the bits it needs.
const PREAMBLE_COS: f64 = 0.35;
const PREAMBLE_MIN_BITS: usize = 16;
/// A long message: the mean reliability of bits 113 to 144 is at least this share of that of bits 25 to 112.
const TAIL_SHARE: f32 = 0.5;
/// The most bits a decode of combined bursts may repair.
const MAX_COMBINED_REPAIR: u32 = 6;
/// The working rate of the demodulator.
const FS: f64 = super::fgb::RATE / 4.0;
/// Samples of one bit at 400 bit/s.
const BIT: f64 = FS / 400.0;
/// The fine carrier search: half width and step, Hz.
const FINE_SPAN_HZ: f64 = 40.0;
const FINE_STEP_HZ: f64 = 0.5;
/// The step of the drift search, Hz/s.
const DRIFT_STEP_HZ_S: f64 = 4.0;

/// The decision rules of a single burst or of a sum of bursts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// One burst.
    Single,
    /// The soft bits of several bursts summed.
    Combined,
}

/// A decoded message with the measures of its repair.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The message (`bits_corrected` counts the bits Chase decoding and BCH-2 changed).
    pub message: Fgb,
    /// Bits of the message Chase decoding changed.
    pub changed: u32,
    /// The distance to the next code word that passes, over the mean reliability.
    pub margin: f32,
    /// The reliability of the changed bits over the mean reliability.
    pub cost: f32,
    /// No bit changed, or one with the message's checks good (a long message's BCH-2 decodes): no repeat needs to confirm it.
    pub sure: bool,
}

/// What [`Demodulator::demodulate`] found in one burst.
#[derive(Clone, Debug, Default)]
pub struct FgbDemod {
    /// The fine carrier offset, Hz.
    pub carrier_hz: f64,
    /// The decoded message.
    pub decoded: Option<Decoded>,
    /// When no sure decode came: the soft bits (LLRs, positive means 0, polarity corrected) of the best timings, best first.
    pub soft: Vec<Vec<f32>>,
}

/// The two sync words.
fn sync_word(test: bool) -> [u8; SYNC] {
    let mut w = [1u8; SYNC];
    let f = if test {
        t001::FRAME_SYNC_TEST
    } else {
        t001::FRAME_SYNC_NORMAL
    };
    for (o, &b) in w[15..].iter_mut().zip(&f) {
        *o = b as u8;
    }
    w
}

/// Whether the country code of a message (bits 27 to 36) is one that exists.
fn country_exists(msg: &[u8]) -> bool {
    let mid = msg[26..36]
        .iter()
        .fold(0u16, |v, &b| (v << 1) | u16::from(b & 1));
    sdrkit_util_ids::mid::country(mid).is_some()
}

/// The decision on a message's soft bits: the sync word, Chase decoding of BCH-1 and the guards of the message.
#[derive(Clone, Debug)]
pub struct Decider {
    bch: Bch,
    chase: ChaseDecoder,
    codes: t001::Codes,
    hard: Vec<u8>,
    out: Vec<u8>,
    bits: Vec<bool>,
}

impl Default for Decider {
    fn default() -> Self {
        Self::new()
    }
}

impl Decider {
    /// A decider with its codes.
    pub fn new() -> Self {
        Decider {
            bch: Bch::from_code(&codes::SARSAT_82_61).expect("a named code builds"),
            chase: ChaseDecoder::new(WEAKEST, WEAKEST, DEFAULT_MAX_FAILED)
                .expect("a valid number of bits"),
            codes: t001::Codes::new(),
            hard: Vec::with_capacity(BITS),
            out: vec![0; BITS],
            bits: Vec::with_capacity(BITS),
        }
    }

    /// Decides on the LLRs `llr` of a message (positive means 0, polarity corrected; 112 to 144 values). The sync word needs
    /// 23 of 24 bits (either frame sync). BCH-1 is decoded by Chase over its 7 weakest bits, each candidate word with a country
    /// code that exists. A repair is refused when its margin is under 1.5 or it costs more than 1.0 a bit; three repaired bits
    /// need a perfect sync, and two in a long message need BCH-2 intact. The length is read from the burst (bits 113 to 144
    /// modulated or not) and the repaired length flag must agree. Under [`Rule::Combined`] at most 6 bits may be repaired
    /// and two need a perfect sync.
    pub fn decide(&mut self, llr: &[f32], rule: Rule) -> Option<Decoded> {
        if llr.len() < SHORT {
            return None;
        }
        self.hard.clear();
        self.hard.extend(llr.iter().map(|&v| u8::from(v < 0.0)));
        let matches = [false, true]
            .iter()
            .map(|&t| {
                sync_word(t)
                    .iter()
                    .zip(&self.hard)
                    .filter(|(a, b)| a == b)
                    .count() as u32
            })
            .max()
            .unwrap_or(0);
        if matches < SYNC_TO_DECODE {
            return None;
        }
        let perfect = matches == SYNC as u32;
        // The length from the burst itself: bits 113 to 144 of a long message are modulated, those after a short one are not.
        let mean_abs = |r: Range<usize>| {
            let n = r.len().max(1) as f32;
            llr.get(r)
                .map_or(0.0, |v| v.iter().map(|x| x.abs()).sum::<f32>() / n)
        };
        let long = llr.len() >= BITS && mean_abs(SHORT..BITS) > TAIL_SHARE * mean_abs(SYNC..SHORT);
        let len = if long { BITS } else { SHORT };
        let r = self.chase.decode(
            &mut self.bch,
            &self.hard[..len],
            &llr[..len],
            PDF1,
            SYNC,
            country_exists,
            &mut self.out[..len],
        )?;
        // The length flag (bit 25) is inside the code word: a winner must read the length the burst has.
        if (self.out[24] == 1) != long {
            return None;
        }
        self.bits.clear();
        self.bits.extend(self.out[..len].iter().map(|&b| b == 1));
        let mut message = t001::decode(&self.bits, &mut self.codes)?;
        // BCH-1 is a code word now: what `decode` corrected is BCH-2's. BCH-2 good: it decodes; intact: it needed no repair.
        let bch2 = message.bits_corrected as u32;
        let good = !long || message.bch2_ok == Some(true);
        let intact = good && bch2 == 0;
        let c = r.changed;
        let refused = (c > 0 && (r.margin < MIN_MARGIN || r.cost > MAX_COST_PER_BIT * c as f32))
            || (c >= 2 && long && !intact)
            || match rule {
                Rule::Single => c >= 3 && !perfect,
                Rule::Combined => c > MAX_COMBINED_REPAIR || (c >= 2 && !perfect),
            };
        if refused {
            return None;
        }
        message.bits_corrected = (c + bch2) as usize;
        Some(Decoded {
            message,
            changed: c,
            margin: r.margin,
            cost: r.cost,
            sure: c == 0 || (c == 1 && good),
        })
    }
}

/// Mixes a burst at 128 kHz down by `coarse` Hz, low-passes it (6 kHz pass, 12 kHz stop) and writes every fourth sample
/// (32 kHz) to `y`.
pub(crate) fn mix_and_decimate(x: &[Complex32], coarse: f64, y: &mut Vec<Complex32>) {
    let rate = super::fgb::RATE;
    let dph = -TAU * coarse / rate;
    let taps = sdrkit_algo_dsp::fir::lowpass(rate, 6_000.0, 12_000.0, 60.0)
        .expect("the filter settings are valid");
    let mut fir = sdrkit_algo_dsp::fir::Fir::<Complex32>::new(&taps);
    let delay = taps.len() / 2;
    y.clear();
    for (i, &v) in x.iter().enumerate() {
        let (s, c) = (dph * i as f64).sin_cos();
        let o = fir.process_sample(v * Complex32::new(c as f32, s as f32));
        if i >= delay && (i - delay).is_multiple_of(4) {
            y.push(o);
        }
    }
}

/// Moves the burst `y` at 32 kHz to 0 Hz: the carrier frequency within +-40 Hz that gathers the most power in sums of one bit
/// period from `st` on (a parabola through the three best of a 0.5 Hz grid), with the carrier's drift within `max_drift`
/// Hz/s (a 4 Hz/s grid; 0 searches none). Returns the frequency at the middle of those samples and the drift removed.
pub(crate) fn fine_carrier(y: &mut [Complex32], st: usize, max_drift: f64) -> (f64, f64) {
    let st = st.min(y.len());
    let blocks: Vec<(f64, f64)> = y[st..]
        .as_chunks::<{ BIT as usize }>()
        .0
        .iter()
        .map(|c| {
            let z: Complex32 = c.iter().sum();
            (f64::from(z.re), f64::from(z.im))
        })
        .collect();
    if blocks.len() < 8 {
        return (0.0, 0.0);
    }
    let dt = BIT / FS;
    let mid = 0.5 * blocks.len() as f64 * dt;
    let steps = (2.0 * FINE_SPAN_HZ / FINE_STEP_HZ) as usize + 1;
    let mut flat = Vec::with_capacity(blocks.len());
    // The power at each frequency of the grid after the drift `r` is removed.
    let mut powers = |r: f64, p: &mut Vec<f64>| {
        flat.clear();
        flat.extend(blocks.iter().enumerate().map(|(m, b)| {
            let t = (m as f64 + 0.5) * dt - mid;
            let (s, c) = (-0.5 * TAU * r * t * t).sin_cos();
            (b.0 * c - b.1 * s, b.0 * s + b.1 * c)
        }));
        p.clear();
        for i in 0..steps {
            let f = -FINE_SPAN_HZ + i as f64 * FINE_STEP_HZ;
            let (ws, wc) = (-TAU * f * dt).sin_cos();
            let (mut re, mut im) = (0f64, 0f64);
            let (mut c, mut s) = (1f64, 0f64);
            for b in &flat {
                re += b.0 * c - b.1 * s;
                im += b.0 * s + b.1 * c;
                (c, s) = (c * wc - s * ws, c * ws + s * wc);
            }
            p.push(re * re + im * im);
        }
    };
    let rates = if max_drift > 0.0 {
        (max_drift / DRIFT_STEP_HZ_S).floor() as i32
    } else {
        0
    };
    let mut p = Vec::with_capacity(steps);
    let mut best = (f64::MIN, 0.0, 0.0);
    for i in -rates..=rates {
        let r = f64::from(i) * DRIFT_STEP_HZ_S;
        powers(r, &mut p);
        let k = (0..steps)
            .max_by(|&a, &b| p[a].total_cmp(&p[b]))
            .unwrap_or(0);
        if p[k] > best.0 {
            let mut f = -FINE_SPAN_HZ + k as f64 * FINE_STEP_HZ;
            if k > 0 && k + 1 < steps {
                let (a, b, c) = (p[k - 1], p[k], p[k + 1]);
                let d = a - 2.0 * b + c;
                if d < 0.0 {
                    f += (0.5 * (a - c) / d).clamp(-0.5, 0.5) * FINE_STEP_HZ;
                }
            }
            best = (p[k], f, r);
        }
    }
    let (_, f, r) = best;
    for (i, v) in y.iter_mut().enumerate() {
        let t = (i as f64 - st as f64) / FS - mid;
        let (s, c) = (-TAU * (f * t + 0.5 * r * t * t)).sin_cos();
        *v *= Complex32::new(c as f32, s as f32);
    }
    (f, r)
}

/// A timing candidate: the start of the first sync bit and the bit period (samples at 32 kHz), the polarity.
#[derive(Clone, Copy, Debug)]
struct Timing {
    t0: f64,
    period: f64,
    reversed: bool,
}

/// The coherent demodulator of a burst, with its working buffers.
#[derive(Clone, Debug, Default)]
pub struct Demodulator {
    pm: PmBurst,
    decider: Decider,
    y: Vec<Complex32>,
    cum: Vec<Complex32>,
    llr: Vec<f32>,
}

impl Demodulator {
    /// A demodulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// The sum of the samples from `a` to `b` (rounded to samples, clipped to the burst).
    fn sum(&self, a: f64, b: f64) -> Complex32 {
        let n = self.y.len();
        let ia = (a.round().max(0.0) as usize).min(n);
        let ib = (b.round().max(0.0) as usize).min(n);
        self.cum[ib] - self.cum[ia]
    }

    /// Whether the 20 bit periods before `t0` hold a bare carrier: their two halves point the same way (a mean cosine of the
    /// half-bit angle of at least 0.35 over at least 16 bits).
    fn preamble_ok(&self, tm: Timing) -> bool {
        let (mut sum, mut n) = (0f64, 0usize);
        for j in 1..=BURST_PREAMBLE_BITS {
            let a = tm.t0 - j as f64 * tm.period;
            if a < 0.0 {
                break;
            }
            let h1 = self.sum(a, a + tm.period / 2.0);
            let h2 = self.sum(a + tm.period / 2.0, a + tm.period);
            let m = (h1.norm() * h2.norm()) as f64;
            if m > 0.0 {
                sum += (h2 * h1.conj()).re as f64 / m;
                n += 1;
            }
        }
        n >= PREAMBLE_MIN_BITS && sum / n as f64 >= PREAMBLE_COS
    }

    /// The timing candidates of the sync search on coherent soft bits: starts every 2 samples from 60 to 300 ms after `st`, bit
    /// rates of 400 bit/s and +-1 %, both polarities and both frame syncs; those with 19 or more matching sync bits, best score
    /// (the sum of the soft sync bits times the bits wanted) first, at least 8 samples apart, at most 16.
    fn candidates(&mut self, st: usize) -> Vec<Timing> {
        let lo = st as f64 + 0.06 * FS;
        let hi = st as f64 + 0.30 * FS;
        let words = [sync_word(false), sync_word(true)];
        let mut found: Vec<(f32, Timing)> = Vec::new();
        let mut y = [0f32; SYNC];
        let mut s = lo;
        while s < hi && s + SHORT as f64 * BIT < self.y.len() as f64 + BIT {
            for e in [-0.01, 0.0, 0.01] {
                let t = BIT * (1.0 + e);
                let Ok(stats) = self.pm.soft_bits(s, t, &mut y) else {
                    continue;
                };
                if stats.m1 <= 0.0 {
                    continue;
                }
                // Back from LLRs to the soft values, positive for a 1.
                let k = stats.var / (2.0 * stats.m1);
                for reversed in [false, true] {
                    let sign = if reversed { k } else { -k };
                    for w in &words {
                        let (mut score, mut matches) = (0f32, 0u32);
                        for (&b, &l) in w.iter().zip(&y) {
                            let v = sign * l;
                            let want = if b == 1 { 1.0 } else { -1.0 };
                            score += want * v;
                            matches += u32::from(want * v > 0.0);
                        }
                        if matches >= SYNC_TO_TRY {
                            found.push((
                                score,
                                Timing {
                                    t0: s,
                                    period: t,
                                    reversed,
                                },
                            ));
                        }
                    }
                }
            }
            s += SEARCH_STEP;
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out: Vec<Timing> = Vec::with_capacity(MAX_CANDIDATES);
        for (_, tm) in found {
            if out.len() >= MAX_CANDIDATES {
                break;
            }
            if out
                .iter()
                .all(|o| (o.t0 - tm.t0).abs() >= CANDIDATE_SPACING)
            {
                out.push(tm);
            }
        }
        out
    }

    /// The timing within +-8 samples and +-0.5 % of `tm` (the candidates lie 8 samples and 1 % apart) whose first 112 coherent
    /// soft bits are the largest.
    fn refine(&mut self, tm: Timing) -> Timing {
        let mut best = (f32::MIN, tm);
        self.llr.resize(SHORT, 0.0);
        for ds in -REFINE_SAMPLES..=REFINE_SAMPLES {
            for dr in -REFINE_RATE_STEPS..=REFINE_RATE_STEPS {
                let t0 = tm.t0 + ds as f64;
                let period = tm.period * (1.0 + dr as f64 * REFINE_RATE_STEP);
                if let Ok(st) = self.pm.soft_bits(t0, period, &mut self.llr[..SHORT]) {
                    if st.sum_abs > best.0 {
                        best = (st.sum_abs, Timing { t0, period, ..tm });
                    }
                }
            }
        }
        best.1
    }

    /// Demodulates a burst (`x` at 128 kHz, the line found `coarse` Hz from 0, the first frame with the line at `start` samples
    /// into `x`). With `keep_soft`, a burst without a sure decode hands on the soft bits of its best timings.
    pub fn demodulate(
        &mut self,
        x: &[Complex32],
        coarse: f64,
        start: u64,
        keep_soft: bool,
    ) -> FgbDemod {
        mix_and_decimate(x, coarse, &mut self.y);
        let st = (start / 4) as usize;
        let fine = coarse + fine_carrier(&mut self.y, st, 0.0).0;
        self.cum.clear();
        let mut acc = Complex32::new(0.0, 0.0);
        self.cum.push(acc);
        for &v in &self.y {
            acc += v;
            self.cum.push(acc);
        }
        self.pm.load(&self.y);
        let words = [sync_word(false), sync_word(true)];
        let mut best: Option<Decoded> = None;
        let mut keep: Vec<(f32, Timing, Vec<f32>)> = Vec::new();
        let cands = self.candidates(st);
        for cand in cands {
            let tm = self.refine(cand);
            self.llr.resize(BITS, 0.0);
            let Ok(stats) = self.pm.soft_bits(tm.t0, tm.period, &mut self.llr) else {
                continue;
            };
            // The sync word and the polarity from the coherent bits: the most matches, then the largest sum of want x y.
            let mut pick = (0u32, f32::MIN, false);
            for reversed in [false, true] {
                for w in &words {
                    let (mut m, mut conf) = (0u32, 0f32);
                    for (k, &b) in w.iter().enumerate() {
                        // y has the sign of the bit: positive for a 1.
                        let y = if reversed { self.llr[k] } else { -self.llr[k] };
                        let want = if b == 1 { 1.0 } else { -1.0 };
                        conf += want * y;
                        m += u32::from(want * y > 0.0);
                    }
                    if (m, conf) > (pick.0, pick.1) {
                        pick = (m, conf, reversed);
                    }
                }
            }
            let (matches, _, reversed) = pick;
            if reversed {
                self.llr.iter_mut().for_each(|v| *v = -*v);
            }
            let tm = Timing { reversed, ..tm };
            if !self.preamble_ok(tm) {
                continue;
            }
            if matches >= SYNC_TO_DECODE {
                if let Some(d) = self.decider.decide(&self.llr, Rule::Single) {
                    let better = best.as_ref().is_none_or(|b| {
                        (!d.sure, d.changed, d.cost) < (!b.sure, b.changed, b.cost)
                    });
                    if better {
                        best = Some(d);
                    }
                    if best.as_ref().is_some_and(|b| b.changed == 0) {
                        break;
                    }
                }
            }
            if keep_soft && matches >= SYNC_TO_KEEP {
                keep.push((stats.sum_abs, tm, self.llr.clone()));
            }
        }
        let mut soft = Vec::new();
        if best.as_ref().is_none_or(|b| !b.sure) {
            keep.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut taken: Vec<Timing> = Vec::new();
            for (_, tm, llr) in keep {
                if taken.len() >= KEEP_TIMINGS {
                    break;
                }
                let apart = |o: &Timing| {
                    (o.t0 - tm.t0).abs() >= o.period / 4.0
                        || (o.period / tm.period - 1.0).abs() >= 0.002
                        || o.reversed != tm.reversed
                };
                if taken.iter().all(apart) {
                    taken.push(tm);
                    soft.push(llr);
                }
            }
        }
        FgbDemod {
            carrier_hz: fine,
            decoded: best,
            soft,
        }
    }
}
