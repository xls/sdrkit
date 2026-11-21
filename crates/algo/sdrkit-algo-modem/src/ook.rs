//! On-off keying, amplitude-shift keying and pulse-position modulation.
//!
//! - [`OokSlicer`] turns a power stream into pulses: it opens when the power rises a set number of dB above the noise floor (a
//!   median of chunks of power values from `sdrkit_algo_dsp::floor::ChunkPercentile`, corrected to the mean, and fed only while closed,
//!   so a long pulse does not lift it) and closes a few dB lower (hysteresis), after a debounce. Edges are where the power crosses
//!   the thresholds. Edges are placed between samples by linear interpolation of the power, so pulse widths
//!   are not quantised to the sample period. ISM sensors, DME pulse pairs, CW keying, NDB idents and Mode A/C use it.
//! - [`AskSlicer`] decides a two-level envelope (10 % ASK of NFC type B, 100 % ASK, keyed carriers with a residual level): the high
//!   and low levels are tracked by decision-directed means, the threshold is half way, with a hysteresis share of the gap.
//! - [`PowerIntegral`] holds a stored burst's power as a continuous function (piecewise linear between samples, integrated in
//!   `f64`), so the energy over any fractional window is exact: pulse-position bits ([`PowerIntegral::ppm_bit`], ADS-B Mode S) and
//!   M-ary positions ([`PowerIntegral::ppm_symbol`], ISO 15693 1-of-4) at any sample rate, not only a whole number of samples per
//!   chip.
//!
//! ```
//! use sdrkit_algo_modem::ook::{OokConfig, OokSlicer};
//! let mut s = OokSlicer::new(OokConfig::default()).unwrap();
//! let mut p = vec![1e-3f32; 1000];
//! for v in &mut p[300..450] { *v = 1.0; }
//! let mut pulses = Vec::new();
//! s.process(&p, &mut pulses);
//! assert_eq!(pulses.len(), 1);
//! assert!((pulses[0].width - 150.0).abs() < 1.0);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::floor::ChunkPercentile;

/// One detected pulse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    /// Where the power crossed the opening threshold, in samples since the start of the stream (fractional).
    pub start: f64,
    /// The width in samples, from the opening crossing to the closing crossing.
    pub width: f64,
    /// The mean power over the pulse.
    pub power: f32,
    /// The noise floor when the pulse opened.
    pub floor: f32,
}

/// Settings of [`OokSlicer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OokConfig {
    /// Opening threshold above the floor, dB.
    pub open_db: f32,
    /// Closing threshold above the floor, dB (at most `open_db`).
    pub close_db: f32,
    /// Samples the power must stay below the closing threshold before the pulse ends (debounce).
    pub hold: usize,
    /// Power values per floor estimate (the median of each chunk, corrected to the mean noise power).
    pub floor_chunk: usize,
    /// Smoothing of the floor from chunk to chunk, `(0, 1]`.
    pub floor_alpha: f64,
    /// The lowest floor allowed (a power), so silence does not make every speck a pulse.
    pub min_floor: f32,
}

impl Default for OokConfig {
    /// Open 10 dB and close 7 dB above the floor, 2 samples of hold, the floor from chunks of 256 values smoothed by 0.25, floor at
    /// least 1e-12. No pulse opens before the first floor estimate (or [`OokSlicer::set_floor`]).
    fn default() -> Self {
        OokConfig {
            open_db: 10.0,
            close_db: 7.0,
            hold: 2,
            floor_chunk: 256,
            floor_alpha: 0.25,
            min_floor: 1e-12,
        }
    }
}

/// Pulses from a power stream (module documentation).
#[derive(Clone, Debug)]
pub struct OokSlicer {
    cfg: OokConfig,
    floor: ChunkPercentile,
    fl: f32,
    known: bool,
    open: f32,
    close: f32,
    on: bool,
    start: f64,
    sum: f64,
    count: u64,
    below: usize,
    /// Fractional position of the first closing crossing of the current low run.
    close_at: f64,
    open_floor: f32,
    prev: f32,
    n: u64,
}

impl OokSlicer {
    /// A slicer with these settings. Errors unless `0 <= close_db <= open_db`, the floor settings are valid and `min_floor > 0`.
    pub fn new(cfg: OokConfig) -> Result<Self, ModemError> {
        Param::OpenLevel.check_finite(cfg.open_db)?;
        if !(cfg.close_db >= 0.0 && cfg.close_db.is_finite()) {
            return Param::CloseLevel.at_least(0.0, cfg.close_db);
        }
        if cfg.close_db > cfg.open_db {
            return Param::CloseLevel.conflict(Param::OpenLevel);
        }
        if !(cfg.min_floor > 0.0 && cfg.min_floor.is_finite()) {
            return Param::Floor.positive(cfg.min_floor);
        }
        Ok(OokSlicer {
            floor: ChunkPercentile::new(cfg.floor_chunk, 0.5, 1, cfg.floor_alpha)?,
            fl: cfg.min_floor,
            known: false,
            open: 10f32.powf(cfg.open_db / 10.0),
            close: 10f32.powf(cfg.close_db / 10.0),
            cfg,
            on: false,
            start: 0.0,
            sum: 0.0,
            count: 0,
            below: 0,
            close_at: 0.0,
            open_floor: 0.0,
            prev: 0.0,
            n: 0,
        })
    }

    /// The noise floor (a power).
    pub fn floor(&self) -> f32 {
        self.fl
    }

    /// Sets the floor (for example from a quiet stretch measured elsewhere).
    pub fn set_floor(&mut self, p: f32) {
        self.floor.reset();
        if p.is_finite() {
            self.fl = p.max(self.cfg.min_floor);
            self.known = true;
        }
    }

    /// Whether a pulse is open.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Forgets the floor and any open pulse; the sample count restarts at 0.
    pub fn reset(&mut self) {
        self.floor.reset();
        self.fl = self.cfg.min_floor;
        self.known = false;
        self.on = false;
        self.below = 0;
        self.prev = 0.0;
        self.n = 0;
    }

    /// The fractional position where the power crossed `thr` between the previous sample and this one.
    #[inline]
    fn crossing(&self, p: f32, thr: f32) -> f64 {
        let d = p - self.prev;
        let f = if d != 0.0 {
            ((thr - self.prev) / d).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.n as f64 - 1.0 + f as f64
    }

    #[inline]
    fn feed_floor(&mut self, p: f32) {
        if let Some(f) = self.floor.push(p) {
            self.fl = f.max(self.cfg.min_floor);
            self.known = true;
        }
    }

    /// Takes one power value; returns a pulse when one ends here.
    #[inline]
    pub fn push(&mut self, p: f32) -> Option<Pulse> {
        let p = if p.is_finite() { p.max(0.0) } else { 0.0 };
        let fl = self.floor();
        let mut done = None;
        if !self.on {
            if self.known && p >= fl * self.open {
                self.on = true;
                self.start = if self.n == 0 {
                    0.0
                } else {
                    self.crossing(p, fl * self.open)
                };
                self.open_floor = fl;
                self.sum = p as f64;
                self.count = 1;
                self.below = 0;
            } else {
                self.feed_floor(p);
            }
        } else {
            let thr = self.open_floor * self.close;
            if p < thr {
                if self.below == 0 {
                    self.close_at = self.crossing(p, thr);
                }
                self.below += 1;
                if self.below > self.cfg.hold {
                    self.on = false;
                    done = Some(Pulse {
                        start: self.start,
                        width: self.close_at - self.start,
                        power: (self.sum / self.count.max(1) as f64) as f32,
                        floor: self.open_floor,
                    });
                    self.feed_floor(p);
                }
            } else {
                self.below = 0;
                self.sum += p as f64;
                self.count += 1;
            }
        }
        self.prev = p;
        self.n += 1;
        done
    }

    /// Takes power values, appending each finished pulse to `out`.
    pub fn process(&mut self, power: &[f32], out: &mut Vec<Pulse>) {
        for &p in power {
            if let Some(q) = self.push(p) {
                out.push(q);
            }
        }
    }

    /// Takes complex samples (power `|x|^2`), appending each finished pulse to `out`.
    pub fn process_iq(&mut self, x: &[Complex32], out: &mut Vec<Pulse>) {
        for &v in x {
            if let Some(q) = self.push(v.norm_sqr()) {
                out.push(q);
            }
        }
    }
}

/// A two-level slicer for an envelope (module documentation). Output per sample: 1 for the high level, 0 for the low, and a soft
/// value `(x - threshold) / (high - low)`.
#[derive(Clone, Debug)]
pub struct AskSlicer {
    hi: f32,
    lo: f32,
    alpha: f32,
    hyst: f32,
    state: u8,
}

impl AskSlicer {
    /// Levels first guessed at `low` and `high` (`high > low`), followed with forgetting factor `alpha` in `(0, 1)`, hysteresis a
    /// share `hysteresis` (0 to 0.5) of the gap either side of the threshold.
    pub fn new(low: f32, high: f32, alpha: f32, hysteresis: f32) -> Result<Self, ModemError> {
        Param::Level.check_finite(low)?;
        Param::Level.check_finite(high)?;
        if high <= low {
            return Param::HighLevel.conflict(Param::LowLevel);
        }
        Param::Smoothing.check_open(alpha, 0.0, 1.0)?;
        Param::Hysteresis.check_in(hysteresis, 0.0, 0.5)?;
        Ok(AskSlicer {
            hi: high,
            lo: low,
            alpha,
            hyst: hysteresis,
            state: 0,
        })
    }

    /// The tracked high level.
    pub fn high(&self) -> f32 {
        self.hi
    }

    /// The tracked low level.
    pub fn low(&self) -> f32 {
        self.lo
    }

    /// The modulation depth `(high - low) / (high + low)` (0.1 ASK at about 0.05).
    pub fn depth(&self) -> f32 {
        let s = self.hi + self.lo;
        if s > 0.0 {
            (self.hi - self.lo) / s
        } else {
            0.0
        }
    }

    /// Decides one envelope value: returns `(bit, soft)`.
    #[inline]
    pub fn push(&mut self, x: f32) -> (u8, f32) {
        if !x.is_finite() {
            return (self.state, 0.0);
        }
        let gap = (self.hi - self.lo).max(f32::MIN_POSITIVE);
        let thr = 0.5 * (self.hi + self.lo);
        let h = self.hyst * gap;
        if self.state == 0 && x > thr + h {
            self.state = 1;
        } else if self.state == 1 && x < thr - h {
            self.state = 0;
        }
        // The level on this side follows; the other drifts slowly towards the value, so a first guess too far apart closes in.
        let slow = self.alpha * (1.0 / 64.0);
        if self.state == 1 {
            self.hi += self.alpha * (x - self.hi);
            self.lo += slow * (x - self.lo);
        } else {
            self.lo += self.alpha * (x - self.lo);
            self.hi += slow * (x - self.hi);
        }
        if self.hi <= self.lo {
            self.hi = self.lo + gap * 0.5;
        }
        (self.state, (x - thr) / gap)
    }

    /// Decides every value of `x` into `bits` and `soft` (each at least as long as `x`).
    pub fn process(
        &mut self,
        x: &[f32],
        bits: &mut [u8],
        soft: &mut [f32],
    ) -> Result<(), ModemError> {
        if bits.len() < x.len() || soft.len() < x.len() {
            return Err(ModemError::OutputTooShort { need: x.len() });
        }
        for ((&v, b), s) in x.iter().zip(bits.iter_mut()).zip(soft.iter_mut()) {
            let (bb, ss) = self.push(v);
            *b = bb;
            *s = ss;
        }
        Ok(())
    }
}

/// A pulse-position decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PpmBit {
    /// 1 when the pulse is in the first half (ADS-B Mode S, ICAO Annex 10 Vol IV 3.1.2.3.2.1.2), 0 in the second.
    pub bit: u8,
    /// The energies of the first and second halves.
    pub energy: [f64; 2],
    /// `(E1 - E2) / (E1 + E2)`: +1 a clean 1, -1 a clean 0, near 0 unclear. A reliability, not an LLR.
    pub contrast: f32,
}

/// A stored burst's power as an exact continuous function (module documentation). Sized at `new`; loading a burst allocates nothing.
#[derive(Clone, Debug)]
pub struct PowerIntegral {
    p: Vec<f32>,
    cum: Vec<f64>,
    len: usize,
}

impl PowerIntegral {
    /// Room for bursts of up to `capacity` samples (at least 2).
    pub fn new(capacity: usize) -> Result<Self, ModemError> {
        if capacity < 2 {
            return Param::Capacity.at_least(2.0, capacity as f64);
        }
        Ok(PowerIntegral {
            p: vec![0.0; capacity],
            cum: vec![0.0; capacity],
            len: 0,
        })
    }

    /// Samples loaded.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether nothing is loaded.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn integrate(&mut self) {
        let mut acc = 0f64;
        self.cum[0] = 0.0;
        for k in 1..self.len {
            acc += 0.5 * (self.p[k - 1] as f64 + self.p[k] as f64);
            self.cum[k] = acc;
        }
    }

    /// Loads complex samples (power `|x|^2`). Errors when longer than the capacity.
    pub fn load(&mut self, x: &[Complex32]) -> Result<(), ModemError> {
        if x.len() > self.p.len() {
            return Param::Input.range(0.0, self.p.len() as f64, x.len() as f64);
        }
        for (d, v) in self.p.iter_mut().zip(x) {
            let e = v.norm_sqr();
            *d = if e.is_finite() { e } else { 0.0 };
        }
        self.len = x.len();
        self.integrate();
        Ok(())
    }

    /// Loads power values. Errors when longer than the capacity.
    pub fn load_power(&mut self, power: &[f32]) -> Result<(), ModemError> {
        if power.len() > self.p.len() {
            return Param::Input.range(0.0, self.p.len() as f64, power.len() as f64);
        }
        for (d, &v) in self.p.iter_mut().zip(power) {
            *d = if v.is_finite() { v } else { 0.0 };
        }
        self.len = power.len();
        self.integrate();
        Ok(())
    }

    /// `F(t)`, the integral of the power from sample 0 to position `t` (clamped to the burst).
    #[inline]
    fn at(&self, t: f64) -> f64 {
        if self.len < 2 || t.is_nan() || t <= 0.0 {
            return 0.0;
        }
        let last = (self.len - 1) as f64;
        if t >= last {
            return self.cum[self.len - 1];
        }
        let k = t.floor() as usize;
        let f = t - k as f64;
        let (a, b) = (self.p[k] as f64, self.p[k + 1] as f64);
        self.cum[k] + f * a + 0.5 * f * f * (b - a)
    }

    /// The energy between positions `a` and `b` (samples, fractional, `a <= b`; clamped to the burst).
    #[inline]
    pub fn energy(&self, a: f64, b: f64) -> f64 {
        self.at(b) - self.at(a)
    }

    /// The pulse-position bit whose two chips of `chip` samples start at `start`.
    #[inline]
    pub fn ppm_bit(&self, start: f64, chip: f64) -> PpmBit {
        let e1 = self.energy(start, start + chip);
        let e2 = self.energy(start + chip, start + 2.0 * chip);
        let s = e1 + e2;
        PpmBit {
            bit: (e1 > e2) as u8,
            energy: [e1, e2],
            contrast: if s > 0.0 { ((e1 - e2) / s) as f32 } else { 0.0 },
        }
    }

    /// The M-ary pulse position: the slot (of `slots`, each `slot` samples, from `start`) with the most energy, and its share of the
    /// total energy.
    pub fn ppm_symbol(&self, start: f64, slot: f64, slots: usize) -> (usize, f32) {
        let mut best = (0usize, f64::NEG_INFINITY);
        let mut total = 0.0;
        for k in 0..slots {
            let a = start + k as f64 * slot;
            let e = self.energy(a, a + slot);
            total += e;
            if e > best.1 {
                best = (k, e);
            }
        }
        (
            best.0,
            if total > 0.0 {
                (best.1 / total) as f32
            } else {
                0.0
            },
        )
    }

    /// Decodes `bits.len()` PPM bits from `start`, each `2 chip` samples long (any fractional chip length), into `bits` and their
    /// contrasts into `contrast`.
    pub fn ppm_bits(
        &self,
        start: f64,
        chip: f64,
        bits: &mut [u8],
        contrast: &mut [f32],
    ) -> Result<(), ModemError> {
        if contrast.len() < bits.len() {
            return Err(ModemError::OutputTooShort { need: bits.len() });
        }
        Param::ChipLen.check_positive(chip)?;
        Param::Start.check_finite(start)?;
        for (k, (b, c)) in bits.iter_mut().zip(contrast.iter_mut()).enumerate() {
            let r = self.ppm_bit(start + 2.0 * chip * k as f64, chip);
            *b = r.bit;
            *c = r.contrast;
        }
        Ok(())
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn ook(seconds: f64) -> BenchResult {
        // ISM sensor bursts at 2 Msps.
        let n = 200_000;
        let p: Vec<f32> = (0..n)
            .map(|i| if (i / 500) % 3 == 0 { 1.0 } else { 1e-3 })
            .collect();
        let mut s = OokSlicer::new(OokConfig::default()).expect("valid");
        let mut out = Vec::with_capacity(1024);
        measure(seconds, n, n as f64 / 2e6, || {
            out.clear();
            s.process(&p, &mut out);
        })
    }
    fn ppm(seconds: f64) -> BenchResult {
        // A Mode S long reply at 2.4 Msps (112 bits, 1.2 samples per chip), loaded and sliced.
        let n = 300;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::new(((i * 7) % 3) as f32, 0.0))
            .collect();
        let mut pi = PowerIntegral::new(n).expect("valid");
        let mut bits = [0u8; 112];
        let mut c = [0f32; 112];
        measure(seconds, n, n as f64 / 2.4e6, || {
            pi.load(&x).unwrap_or(());
            pi.ppm_bits(10.3, 1.2, &mut bits, &mut c).unwrap_or(());
        })
    }
    vec![
        Benchmark {
            name: "modem.ook_slicer_2000k",
            run: ook,
        },
        Benchmark {
            name: "modem.ppm_mode_s_2400k",
            run: ppm,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn pulses_have_sub_sample_edges_and_ignore_noise() {
        // Pulses of 37.25 samples (a linear power ramp across the edge samples) in noise 20 dB below.
        let mut rng = Rng::new(4);
        let mut starts = Vec::new();
        let mut t = 100.0f64;
        while t < 19_700.0 {
            starts.push(t);
            t += 200.0 + rng.range(0.0, 50.0);
        }
        let p: Vec<f32> = (0..20_000)
            .map(|k| {
                let k = k as f64;
                // Amplitude: the share of sample [k, k+1) the pulse covers.
                let a: f64 = starts
                    .iter()
                    .map(|&st| ((k + 1.0).min(st + 37.25) - k.max(st)).clamp(0.0, 1.0))
                    .sum();
                let n = 0.02 * rng.normal_f32();
                (a as f32 + n).powi(2) * 0.5 + 0.001 * rng.exponential() as f32
            })
            .collect();
        let mut s = OokSlicer::new(OokConfig {
            open_db: 13.0,
            close_db: 10.0,
            ..OokConfig::default()
        })
        .unwrap();
        s.set_floor(0.001);
        let mut out = Vec::new();
        for c in p.chunks(333) {
            s.process(c, &mut out);
        }
        assert_eq!(out.len(), starts.len(), "{out:?}");
        for (q, &st) in out.iter().zip(&starts) {
            assert!((q.start - st).abs() < 1.5, "{} vs {st}", q.start);
            // Edges at the threshold, 13 dB above the floor, lie a little outside the half-power points.
            assert!((q.width - 37.25).abs() < 2.5, "{}", q.width);
        }
    }

    #[test]
    fn ask_tracks_ten_percent_depth() {
        let mut rng = Rng::new(8);
        let bits: Vec<u8> = (0..4000).map(|_| rng.bit()).collect();
        let mut x: Vec<f32> = bits
            .iter()
            .flat_map(|&b| [if b == 1 { 1.0 } else { 0.82 }; 8])
            .collect();
        let mut n = Awgn::with_power(1e-4, 3).unwrap();
        n.process_real(&mut x);
        let mut s = AskSlicer::new(0.6, 1.2, 0.01, 0.1).unwrap();
        let mut out = vec![0u8; x.len()];
        let mut soft = vec![0f32; x.len()];
        s.process(&x, &mut out, &mut soft).unwrap();
        let errs = bits
            .iter()
            .enumerate()
            .skip(500)
            .filter(|&(k, &b)| out[k * 8 + 4] != b)
            .count();
        assert_eq!(errs, 0);
        assert!((s.depth() - 0.0989).abs() < 0.01, "{}", s.depth());
    }

    #[test]
    fn ppm_reads_fractional_chips_exactly() {
        // 2.4 Msps, 0.5 us chips = 1.2 samples: render each chip's power as its covered share of each sample.
        let chip = 1.2;
        let bits: Vec<u8> = (0..112).map(|k| ((k * 13 + 5) % 7 < 3) as u8).collect();
        let start = 10.3;
        let n = (start + 2.0 * chip * 112.0) as usize + 10;
        let mut p = vec![0f32; n];
        for (k, &b) in bits.iter().enumerate() {
            let a = start + 2.0 * chip * k as f64 + if b == 1 { 0.0 } else { chip };
            for (i, v) in p.iter_mut().enumerate() {
                let cov = ((i as f64 + 0.5).min(a + chip) - (i as f64 - 0.5).max(a)).max(0.0);
                *v += cov as f32;
            }
        }
        let mut pi = PowerIntegral::new(n).unwrap();
        pi.load_power(&p).unwrap();
        let mut got = vec![0u8; 112];
        let mut c = vec![0f32; 112];
        pi.ppm_bits(start, chip, &mut got, &mut c).unwrap();
        assert_eq!(got, bits);
        // At 1.2 samples per chip the chips blur into each other; the decision stays clear.
        assert!(c.iter().all(|v| v.abs() > 0.15), "{c:?}");
        // Energy of a linear function: exact.
        let lin: Vec<f32> = (0..10).map(|k| k as f32).collect();
        pi.load_power(&lin).unwrap();
        assert!((pi.energy(1.5, 4.25) - (4.25f64.powi(2) - 1.5f64.powi(2)) / 2.0).abs() < 1e-9);
        let (k, share) = pi.ppm_symbol(0.0, 2.0, 4);
        assert_eq!(k, 3);
        assert!(share > 0.4);
        assert!(pi.load_power(&vec![0.0; n + 1]).is_err());
        assert!(OokSlicer::new(OokConfig {
            close_db: 12.0,
            ..OokConfig::default()
        })
        .is_err());
        assert!(AskSlicer::new(1.0, 0.5, 0.1, 0.1).is_err());
    }
}
