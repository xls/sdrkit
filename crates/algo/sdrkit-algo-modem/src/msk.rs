//! MSK received non-coherently: a one-bit differential detector, the shared symbol synchroniser and the level slicer.
//!
//! MSK turns the phase by `+-pi/2` over each bit. [`MskDemod`] averages the input over one bit (a running complex sum: the matched
//! filter of the rectangular frequency pulse's phasor, roughly), forms `Im{z[n] conj(z[n - D])}` with `D` one bit, which is
//! positive when the phase rose over the last bit (the higher tone), and passes that level stream to
//! [`crate::timing::SymbolSync`] (zero-crossing detector) and a [`LevelSlicer`], which removes the bias a frequency offset leaves
//! and returns LLRs (positive means 0; the higher tone is bit 0 unless inverted). No carrier phase is needed and a frequency offset
//! up to about an eighth of the bit rate is tolerated: ACARS, the aero 600/1200 bit/s channels, DGPS, MSK144.
//!
//! Coherent MSK is offset QPSK with half-sine pulses: [`crate::timing::SymbolSync`] with `oqpsk`, a carrier loop on QPSK and the
//! differential decoding of [`crate::differential`] give it about 3 dB more; acquisition by the lines of the squared signal at
//! `2 fc +- Rs/2` is `crate::freq`'s line-pair search.
//!
//! [`OffsetTracker`] removes a slow carrier offset larger than the receiver follows alone: an oscillator moves the slow mean of the
//! instantaneous frequency to 0 Hz. [`MskDemod::with_offset_tracking`] puts it in front of the receiver. A coherent receiver can
//! use it the same way in front of its carrier loop.
//!
//! ```
//! use sdrkit_algo_modem::msk::MskDemod;
//! let d = MskDemod::new(8.0, 0.01).unwrap();
//! assert_eq!(d.lag(), 8);
//! ```

use crate::fsk::LevelSlicer;
use crate::timing::{SymbolSync, SymbolSyncConfig, ZeroCrossing};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;

/// The samples between two updates of the [`OffsetTracker`] oscillator, counted from the start of the stream.
const OFFSET_STEP: u64 = 256;

/// Slow carrier-offset removal for continuous-phase FSK whose symbols are balanced (MSK, GMSK, CPFSK).
///
/// The tracker measures the instantaneous frequency of each input sample and keeps its slow mean, with time constant `tau`
/// samples. The data average out, so the mean is the carrier offset. An oscillator moves that offset to 0 Hz. The oscillator
/// frequency changes every 256 samples, counted from the start of the stream, so the output does not depend on how the input is
/// split. At the start the mean is a plain running mean, so the tracker does not wait `tau` samples.
///
/// ```
/// use sdrkit_algo_modem::msk::OffsetTracker;
/// use sdrkit_algo_modem::Complex32;
/// // A carrier 0.01 cycles per sample above 0 Hz.
/// let mut x: Vec<Complex32> = (0..20_000)
///     .map(|i| Complex32::from_polar(1.0, std::f32::consts::TAU * 0.01 * i as f32))
///     .collect();
/// let mut t = OffsetTracker::new(1000.0).unwrap();
/// t.process_in_place(&mut x);
/// assert!((t.offset() - 0.01).abs() < 1e-4);
/// // At the end the carrier is at 0 Hz: the phase is constant.
/// let turn = (x[19_999] * x[19_998].conj()).arg();
/// assert!(turn.abs() < 1e-3);
/// ```
#[derive(Clone, Debug)]
pub struct OffsetTracker {
    disc: Discriminator,
    nco: Nco,
    mean: f64,
    alpha: f64,
    n: u64,
}

impl OffsetTracker {
    /// A tracker whose mean has time constant `tau` samples (1 to 1e9). Errors for a `tau` outside that range.
    pub fn new(tau: f64) -> Result<Self, ModemError> {
        if !(1.0..=1e9).contains(&tau) {
            return Param::TimeConstant.range(1.0, 1e9, tau);
        }
        Ok(OffsetTracker {
            // A rate and a deviation of 1 give the frequency in cycles per sample.
            disc: Discriminator::new(1.0, 1.0)?,
            nco: Nco::new(1.0, 0.0),
            mean: 0.0,
            alpha: 1.0 / tau,
            n: 0,
        })
    }

    /// The carrier offset the oscillator removes now, in cycles per sample (multiply by the sample rate for Hz).
    pub fn offset(&self) -> f64 {
        -self.nco.freq()
    }

    /// The slow mean of the instantaneous frequency, in cycles per sample. It leads [`offset`](Self::offset) by up to 256
    /// samples.
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Forgets the signal: the mean, the sample count and the oscillator start again from 0.
    pub fn reset(&mut self) {
        self.disc.reset();
        self.nco = Nco::new(1.0, 0.0);
        self.mean = 0.0;
        self.n = 0;
    }

    /// Takes one sample and returns it with the offset removed. A sample that is not finite counts as 0.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> Complex32 {
        let x = if x.re.is_finite() && x.im.is_finite() {
            x
        } else {
            Complex32::default()
        };
        let f = self.disc.process_sample(x) as f64;
        // A running mean first, so the start does not wait for the slow mean to fill.
        let a = self.alpha.max(1.0 / (self.n as f64 + 1.0));
        self.mean += a * (f - self.mean);
        self.n += 1;
        if self.n.is_multiple_of(OFFSET_STEP) {
            self.nco.set_freq(-self.mean);
        }
        x * self.nco.next_sample()
    }

    /// [`push`](Self::push) on every sample of `x`, in place.
    pub fn process_in_place(&mut self, x: &mut [Complex32]) {
        for v in x {
            *v = self.push(*v);
        }
    }
}

/// The non-coherent MSK receiver (module documentation).
#[derive(Clone, Debug)]
pub struct MskDemod {
    lag: usize,
    boxcar: Vec<Complex32>,
    sum: Complex32,
    at: usize,
    since_resum: usize,
    delay: Vec<Complex32>,
    dat: usize,
    sync: SymbolSync<f32, ZeroCrossing, Cubic>,
    slicer: LevelSlicer,
    work: Vec<f32>,
    levels: Vec<f32>,
    invert: bool,
    tracker: Option<OffsetTracker>,
}

impl MskDemod {
    /// A receiver for `sps` samples per bit (2 to 4096) with timing loop bandwidth `bn_t` per bit.
    pub fn new(sps: f64, bn_t: f64) -> Result<Self, ModemError> {
        if !(2.0..=4096.0).contains(&sps) {
            return Param::Sps.range(2.0, 4096.0, sps);
        }
        let lag = sps.round() as usize;
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.bn_t = bn_t;
        cfg.max_ppm = 1000.0;
        // The level after the one-bit average and the differential product is close to a raised cosine of full roll-off.
        cfg.pulse = Shape::Rc { beta: 1.0 };
        Ok(MskDemod {
            lag,
            boxcar: vec![Complex32::default(); lag],
            sum: Complex32::default(),
            at: 0,
            since_resum: 0,
            delay: vec![Complex32::default(); lag],
            dat: 0,
            sync: SymbolSync::new(ZeroCrossing, Cubic, cfg)?,
            slicer: LevelSlicer::binary(0.01, 1.0)?,
            work: Vec::with_capacity(4096),
            levels: Vec::with_capacity(4096),
            invert: false,
            tracker: None,
        })
    }

    /// Removes a slow carrier offset before the receiver, by an [`OffsetTracker`] with time constant `tau_bits` bits. Without it
    /// the receiver follows an offset up to about an eighth of the bit rate. With it the receiver follows an offset up to about
    /// half the bit rate, after a few time constants. Errors when `tau_bits` times the samples per bit is outside 1 to 1e9.
    pub fn with_offset_tracking(mut self, tau_bits: f64) -> Result<Self, ModemError> {
        self.tracker = Some(OffsetTracker::new(tau_bits * self.lag as f64)?);
        Ok(self)
    }

    /// The offset tracker, when [`with_offset_tracking`](Self::with_offset_tracking) set one.
    pub fn offset_tracker(&self) -> Option<&OffsetTracker> {
        self.tracker.as_ref()
    }

    /// Makes the higher tone bit 1 instead of 0.
    pub fn with_inverted(mut self, yes: bool) -> Self {
        self.invert = yes;
        self
    }

    /// The differential lag in samples (one bit, rounded).
    pub fn lag(&self) -> usize {
        self.lag
    }

    /// The timing loop.
    pub fn timing(&self) -> &SymbolSync<f32, ZeroCrossing, Cubic> {
        &self.sync
    }

    /// Forgets the signal.
    pub fn reset(&mut self) {
        self.boxcar
            .iter_mut()
            .for_each(|v| *v = Complex32::default());
        self.delay
            .iter_mut()
            .for_each(|v| *v = Complex32::default());
        self.sum = Complex32::default();
        self.at = 0;
        self.dat = 0;
        self.sync.reset();
        if let Some(t) = &mut self.tracker {
            t.reset();
        }
    }

    /// Demodulates `x`, appending one LLR per bit to `out`. Works in pieces of up to 4096 samples, so nothing is allocated.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) -> Result<(), ModemError> {
        let n = self.lag;
        for piece in x.chunks(4096) {
            self.work.clear();
            for &v in piece {
                let v = if v.re.is_finite() && v.im.is_finite() {
                    v
                } else {
                    Complex32::default()
                };
                let v = match &mut self.tracker {
                    Some(t) => t.push(v),
                    None => v,
                };
                self.sum += v - self.boxcar[self.at];
                self.boxcar[self.at] = v;
                self.at = if self.at + 1 == n { 0 } else { self.at + 1 };
                self.since_resum += 1;
                if self.since_resum >= 1024 * n {
                    self.since_resum = 0;
                    self.sum = self.boxcar.iter().sum();
                }
                let z = self.sum;
                let old = self.delay[self.dat];
                self.delay[self.dat] = z;
                self.dat = if self.dat + 1 == n { 0 } else { self.dat + 1 };
                self.work.push((z * old.conj()).im);
            }
            self.levels.clear();
            self.sync.process(&self.work, &mut self.levels);
            let mut l = [0f32];
            for &v in &self.levels {
                self.slicer.slice(v, &mut l)?;
                out.push(if self.invert { -l[0] } else { l[0] });
            }
        }
        Ok(())
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn msk(seconds: f64) -> BenchResult {
        // ACARS-like 2400 bit/s at 48 kHz (20 samples per bit).
        let n = 48_000;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::from_polar(1.0, (i as f32 * 0.05).sin() * 2.0))
            .collect();
        let mut d = MskDemod::new(20.0, 0.01).expect("valid");
        let mut out = Vec::with_capacity(4096);
        measure(seconds, n, n as f64 / 48_000.0, || {
            out.clear();
            d.process(&x, &mut out).unwrap_or(());
        })
    }
    fn msk_tracked(seconds: f64) -> BenchResult {
        // DGPS-like 100 bit/s at 4 kHz (40 samples per bit), with the offset tracker (2 s, 200 bits).
        let n = 48_000;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::from_polar(1.0, (i as f32 * 0.05).sin() * 2.0 + i as f32 * 0.03))
            .collect();
        let mut d = MskDemod::new(40.0, 0.01)
            .and_then(|d| d.with_offset_tracking(200.0))
            .expect("valid");
        let mut out = Vec::with_capacity(4096);
        measure(seconds, n, n as f64 / 4_000.0, || {
            out.clear();
            d.process(&x, &mut out).unwrap_or(());
        })
    }
    vec![
        Benchmark {
            name: "modem.msk_2400_48k",
            run: msk,
        },
        Benchmark {
            name: "modem.msk_tracked_100_4k",
            run: msk_tracked,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::modulate::Cpm;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn msk_with_offset_and_clock_error() {
        // 8 samples per bit, 200 ppm clock error, a frequency offset of 5 % of the bit rate, 13 dB SNR per sample.
        let sps = 8.0 * (1.0 + 200e-6);
        let mut rng = Rng::new(17);
        let bits: Vec<u8> = (0..5000).map(|_| rng.bit()).collect();
        let a: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::msk(sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        let mut ph = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(0.05 / 8.0);
        for v in &mut x {
            *v *= ph.next_phasor();
        }
        Awgn::with_power(0.05, 3).unwrap().process(&mut x);
        let mut d = MskDemod::new(8.0, 0.01).unwrap();
        let mut llr = Vec::new();
        for piece in x.chunks(999) {
            d.process(piece, &mut llr).unwrap();
        }
        let got: Vec<u8> = llr.iter().map(|&l| (l < 0.0) as u8).collect();
        let want = &bits[500..4500];
        let found =
            (495..520).any(|o| got.len() >= o + want.len() && &got[o..o + want.len()] == want);
        assert!(found, "clock {} ppm", d.timing().clock_ppm());
        assert!(MskDemod::new(1.0, 0.01).is_err());
    }

    /// MSK at 8 samples per bit with a carrier offset of `offset` of the bit rate, 13 dB SNR per sample; the bits found by `d`,
    /// and the bits sent.
    fn run_offset(offset: f64, mut d: MskDemod) -> (Vec<u8>, Vec<u8>, MskDemod) {
        let mut rng = Rng::new(23);
        let bits: Vec<u8> = (0..6000).map(|_| rng.bit()).collect();
        let a: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::msk(8.0).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        let mut ph = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(offset / 8.0);
        for v in &mut x {
            *v *= ph.next_phasor();
        }
        Awgn::with_power(0.05, 5).unwrap().process(&mut x);
        let mut llr = Vec::new();
        for piece in x.chunks(777) {
            d.process(piece, &mut llr).unwrap();
        }
        let got = llr.iter().map(|&l| (l < 0.0) as u8).collect();
        (got, bits, d)
    }

    /// True when the last 2000 bits sent appear in `got` with at most `max_errors` errors.
    fn tail_found(got: &[u8], bits: &[u8], max_errors: usize) -> bool {
        let want = &bits[3500..5500];
        (3400..3600).any(|o| {
            got.len() >= o + want.len()
                && got[o..o + want.len()]
                    .iter()
                    .zip(want)
                    .filter(|(a, b)| a != b)
                    .count()
                    <= max_errors
        })
    }

    #[test]
    fn offset_tracking_follows_a_large_offset() {
        // 35 % of the bit rate: far past the eighth the receiver follows alone.
        let plain = MskDemod::new(8.0, 0.01).unwrap();
        let (got, bits, _) = run_offset(0.35, plain);
        assert!(!tail_found(&got, &bits, 20), "plain receiver decoded");
        let tracked = MskDemod::new(8.0, 0.01)
            .unwrap()
            .with_offset_tracking(100.0)
            .unwrap();
        let (got, bits, d) = run_offset(0.35, tracked);
        assert!(tail_found(&got, &bits, 2), "tracked receiver failed");
        let t = d.offset_tracker().unwrap();
        // The mean over 100 bits moves by about a tenth of the deviation (1/32) with the balance of the bits.
        assert!(
            (t.offset() - 0.35 / 8.0).abs() < 0.006,
            "offset {}",
            t.offset()
        );
        let mut d = d;
        d.reset();
        assert_eq!(d.offset_tracker().unwrap().offset(), 0.0);
        assert!(OffsetTracker::new(0.5).is_err());
        assert!(MskDemod::new(8.0, 0.01)
            .unwrap()
            .with_offset_tracking(0.0)
            .is_err());
    }
}
