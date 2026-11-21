//! Direct-sequence spread spectrum: despreading, code acquisition and code tracking, for any code at any number of samples per
//! chip (whole or fractional).
//!
//! | Type | What it does | Used by |
//! |---|---|---|
//! | [`ChipDespreader`] | correlates the input with one code period at a time (coherent), streaming or at any fractional start | 802.11b Barker-11, SARSAT second-generation beacons, Loran-C phase codes |
//! | [`CodeBank`] | correlation energies with several codes at once (non-coherent symbol decisions), and a coherent decision for codes whose second half are sign negatives | IEEE 802.15.4 32-chip symbols |
//! | [`CodeAcquisition`] | code phase and Doppler by FFT circular correlation; frequency hypotheses are bin shifts of one FFT, a fraction of a bin by a mix before it; non-coherent sums over periods | GNSS, burst preambles |
//! | [`CodeSearch`] | the same FFT correlation over a segment as a job that runs a slice of work per call, with coherent groups of periods, code-drift compensation, a CFAR threshold ([`gamma_threshold`]) and frequency refinement | GNSS acquisition |
//! | [`CodeTracker`] | early, prompt and late correlators with a delay-locked loop, aided by the carrier frequency the caller provides it | GNSS tracking |
//!
//! The carrier loop is not here. The tracker wipes off the carrier at the frequency given to [`CodeTracker::set_carrier`] (the
//! acquisition's); feed each prompt to `crate::carrier::CarrierSync` (its update rate is the code period), which tracks the
//! residual phase and frequency, and give the total carrier frequency back to [`CodeTracker::set_code_aid`] so the code follows
//! the Doppler (`tests/gnss.rs` shows the whole channel). Codes are given as `±1` values (`f32`); [`chips_from_bits`] turns `0/1` chips (as
//! `sdrkit_util_bits::seq::gps_ca` writes them) into `+1/-1`.
//!
//! A sample belongs to the chip in which its centre lies. Correlations are averages over the samples of a period, so an aligned
//! unit-amplitude signal produces 1, and white noise of variance `s` per sample produces `s / samples` (the processing gain).
//!
//! ```
//! use sdrkit_algo_modem::dsss::{ChipDespreader, BARKER_11};
//! use sdrkit_algo_modem::Complex32;
//! let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
//! // Two symbols of 2 samples per chip, the second inverted.
//! let x: Vec<Complex32> = (0..44).map(|n| Complex32::new(code[(n % 22) / 2] * if n < 22 { 1.0 } else { -1.0 }, 0.0)).collect();
//! let mut d = ChipDespreader::new(&code, 2.0).unwrap();
//! let mut out = Vec::new();
//! d.process(&x, &mut out);
//! assert_eq!(out.len(), 2);
//! assert!((out[0].re - 1.0).abs() < 1e-6 && (out[1].re + 1.0).abs() < 1e-6);
//! ```

use crate::ofdm::rotator;
use crate::{Complex32, ModemError, Param};
mod acquire;
pub mod segmented;

pub use acquire::{
    gamma_threshold, CodeReplica, CodeSearch, Integration, SearchConfig, SearchGrid, SearchJob,
    SearchResult, MAX_OFFSETS, UNIT_POINTS,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::peak;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};

/// The 11-chip Barker sequence of the IEEE 802.11 DSSS PHY at 1 and 2 Mbit/s (IEEE Std 802.11-2016 clause 15.4.4.3: `+1, -1, +1,
/// +1, -1, +1, +1, +1, -1, -1, -1`, the first chip sent first).
pub const BARKER_11: [i8; 11] = [1, -1, 1, 1, -1, 1, 1, 1, -1, -1, -1];

/// `0/1` chips as `+1/-1` values (0 is +1), appended to `out`.
pub fn chips_from_bits(bits: &[u8], out: &mut Vec<f32>) {
    out.extend(bits.iter().map(|&b| 1.0 - 2.0 * (b & 1) as f32));
}

fn check_code(code: &[f32], spc: f64) -> Result<(), ModemError> {
    if code.is_empty() {
        return Param::Code.at_least(1.0, 0.0);
    }
    if code.iter().any(|c| !c.is_finite()) {
        return Param::Code.not_finite();
    }
    if !(spc.is_finite() && spc >= 1.0) {
        return Param::SamplesPerChip.at_least(1.0, spc);
    }
    Ok(())
}

/// A coherent despreader for one code (module documentation).
#[derive(Clone, Debug)]
pub struct ChipDespreader {
    code: Vec<f32>,
    spc: f64,
    inv: f64,
    len: f64,
    pos: f64,
    acc: Complex32,
    count: u32,
}

impl ChipDespreader {
    /// A despreader for `code` (`±1` or any real chip weights) at `samples_per_chip` (at least 1, fractional allowed).
    pub fn new(code: &[f32], samples_per_chip: f64) -> Result<Self, ModemError> {
        check_code(code, samples_per_chip)?;
        Ok(ChipDespreader {
            code: code.to_vec(),
            spc: samples_per_chip,
            inv: 1.0 / samples_per_chip,
            len: code.len() as f64 * samples_per_chip,
            pos: 0.0,
            acc: Complex32::default(),
            count: 0,
        })
    }

    /// Samples per code period.
    pub fn period(&self) -> f64 {
        self.len
    }

    /// Samples per chip.
    pub fn samples_per_chip(&self) -> f64 {
        self.spc
    }

    /// The correlation of one period starting at fractional sample `start` of `x` (burst form). Errors when the period does not lie
    /// inside `x` or `start` is not finite.
    pub fn despread_at(&self, x: &[Complex32], start: f64) -> Result<Complex32, ModemError> {
        if !start.is_finite() || start < -0.5 {
            return Param::Start.at_least(-0.5, start);
        }
        let first = (start - 0.5).ceil().max(0.0) as usize;
        let end = start + self.len;
        if end - 0.5 > x.len() as f64 {
            return Err(ModemError::InputTooShort {
                need: (end - 0.5).ceil() as usize,
            });
        }
        let (mut acc, mut n) = (Complex32::default(), 0u32);
        let mut i = first;
        // Sample i's centre is at i + 0.5 from the slice start; chip of position (i + 0.5 - start).
        while (i as f64 + 0.5) < end && i < x.len() {
            let c = (((i as f64 + 0.5 - start) * self.inv) as usize).min(self.code.len() - 1);
            acc += x[i] * self.code[c];
            n += 1;
            i += 1;
        }
        Ok(if n > 0 { acc / n as f32 } else { acc })
    }

    /// Sets where the next period starts: `offset` samples after the next input sample (a fraction allowed).
    pub fn align(&mut self, offset: f64) {
        if offset.is_finite() {
            self.pos = -offset;
            self.acc = Complex32::default();
            self.count = 0;
        }
    }

    /// Despreads a stream, appending one correlation per complete code period to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        for &v in x {
            let centre = self.pos + 0.5;
            if centre >= self.len {
                out.push(if self.count > 0 {
                    self.acc / self.count as f32
                } else {
                    self.acc
                });
                self.acc = Complex32::default();
                self.count = 0;
                self.pos -= self.len;
            }
            let centre = self.pos + 0.5;
            if centre >= 0.0 {
                let c = ((centre * self.inv) as usize).min(self.code.len() - 1);
                self.acc += v * self.code[c];
                self.count += 1;
            }
            self.pos += 1.0;
        }
        // A period whose last sample has arrived is complete now.
        if self.pos + 0.5 >= self.len && self.count > 0 {
            out.push(self.acc / self.count as f32);
            self.acc = Complex32::default();
            self.count = 0;
            self.pos -= self.len;
        }
    }

    /// Forgets the partial period and aligns the next period with the next sample.
    pub fn reset(&mut self) {
        self.align(0.0);
    }
}

/// Correlations with several codes of the same length at one start: for M-ary spreading (one code per symbol value), decided by
/// the largest energy, which needs no carrier phase.
#[derive(Clone, Debug)]
pub struct CodeBank {
    codes: Vec<ChipDespreader>,
}

impl CodeBank {
    /// A bank of `codes` (each `±1` chips, all the same length) at `samples_per_chip`.
    pub fn new(codes: &[&[f32]], samples_per_chip: f64) -> Result<Self, ModemError> {
        if codes.is_empty() {
            return Param::Code.at_least(1.0, 0.0);
        }
        if let Some(c) = codes.iter().find(|c| c.len() != codes[0].len()) {
            return Param::Code.length(codes[0].len(), c.len());
        }
        Ok(CodeBank {
            codes: codes
                .iter()
                .map(|c| ChipDespreader::new(c, samples_per_chip))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Number of codes.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    /// Never true.
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    /// The energy `|correlation|^2` of every code for the period at `start` into `energies`; returns the index of the largest.
    pub fn correlate_at(
        &self,
        x: &[Complex32],
        start: f64,
        energies: &mut [f32],
    ) -> Result<usize, ModemError> {
        if energies.len() < self.codes.len() {
            return Err(ModemError::OutputTooShort {
                need: self.codes.len(),
            });
        }
        let mut best = (0, -1.0f32);
        for (k, (d, e)) in self.codes.iter().zip(energies.iter_mut()).enumerate() {
            *e = d.despread_at(x, start)?.norm_sqr();
            if *e > best.1 {
                best = (k, *e);
            }
        }
        Ok(best.0)
    }

    /// A coherent decision for codes whose second half are the sign negatives of the first (IEEE 802.15.4 O-QPSK received as MSK:
    /// the chip-sign pattern of symbol `s + 8` is that of symbol `s` negated, from the chip values of IEEE 802.15.4-2020 table
    /// 12-1). The bank holds the first half only. Each correlation is turned by the conjugate of `reference` (the carrier phase),
    /// and its real part goes to `values`. The largest magnitude wins and its sign picks the half. Errors: `values` shorter than
    /// the bank, or a period outside `x`.
    pub fn decide_signed(
        &self,
        x: &[Complex32],
        start: f64,
        reference: Complex32,
        values: &mut [f32],
    ) -> Result<SignedDecision, ModemError> {
        if values.len() < self.codes.len() {
            return Err(ModemError::OutputTooShort {
                need: self.codes.len(),
            });
        }
        let r = reference.conj();
        let mut best = SignedDecision {
            symbol: 0,
            value: 0.0,
        };
        for (k, (d, v)) in self.codes.iter().zip(values.iter_mut()).enumerate() {
            *v = (d.despread_at(x, start)? * r).re;
            if v.abs() > best.value.abs() {
                best = SignedDecision {
                    symbol: if *v < 0.0 { k + self.codes.len() } else { k },
                    value: *v,
                };
            }
        }
        Ok(best)
    }
}

/// What [`CodeBank::decide_signed`] chose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignedDecision {
    /// The symbol: the code index, plus the bank size if the correlation was negative.
    pub symbol: usize,
    /// The signed correlation of the chosen code.
    pub value: f32,
}

/// What [`CodeAcquisition`] found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acquisition {
    /// Where a code period starts, in samples from the start of each period given to [`CodeAcquisition::accumulate`] (fraction
    /// from a parabola through the peak).
    pub code_phase: f64,
    /// The carrier offset in cycles per sample (the hypothesis's centre).
    pub freq: f64,
    /// The peak's power over the mean power of all cells searched (about 1 for noise alone; a detection threshold is near
    /// `ln(cells / false-alarm probability)` for one period).
    pub ratio: f32,
}

/// Parallel code-phase acquisition by FFT (module documentation). One forward FFT per period and sub-bin, then per integer bin
/// shift a product with the conjugate code spectrum and an inverse FFT; the powers add over the periods given.
#[derive(Clone, Debug)]
pub struct CodeAcquisition {
    n: usize,
    max_shift: i32,
    sub: usize,
    code: Vec<Complex32>,
    fwd: Fft,
    inv: Fft,
    x: Vec<Complex32>,
    y: Vec<Complex32>,
    grid: Vec<f32>,
    periods: u32,
}

impl CodeAcquisition {
    /// A search for `code` at `samples_per_chip` (a code period must be at least 8 whole samples: `code.len() * samples_per_chip`
    /// is rounded), over carrier offsets of `-max_shift ..= max_shift` FFT bins (a bin is `1 / period` cycles per sample), each
    /// split into `sub_bins` (1 to 8) steps.
    pub fn new(
        code: &[f32],
        samples_per_chip: f64,
        max_shift: u32,
        sub_bins: usize,
    ) -> Result<Self, ModemError> {
        check_code(code, samples_per_chip)?;
        let n = (code.len() as f64 * samples_per_chip).round() as usize;
        if n < 8 {
            return Param::Period.at_least(8.0, n as f64);
        }
        Param::SubBins.check_count(sub_bins, 1, 8)?;
        if max_shift as usize >= n / 2 {
            return Param::Shift.range(0.0, (n / 2 - 1) as f64, max_shift as f64);
        }
        let mut c: Vec<Complex32> = (0..n)
            .map(|i| {
                let k = (((i as f64 + 0.5) / samples_per_chip) as usize).min(code.len() - 1);
                Complex32::new(code[k], 0.0)
            })
            .collect();
        let mut fwd = Fft::forward(n);
        fwd.process(&mut c);
        c.iter_mut().for_each(|v| *v = v.conj());
        let hyps = (2 * max_shift as usize + 1) * sub_bins;
        Ok(CodeAcquisition {
            n,
            max_shift: max_shift as i32,
            sub: sub_bins,
            code: c,
            fwd,
            inv: Fft::inverse(n),
            x: vec![Complex32::default(); n],
            y: vec![Complex32::default(); n],
            grid: vec![0.0; hyps * n],
            periods: 0,
        })
    }

    /// Samples per period: what [`accumulate`](Self::accumulate) takes.
    pub fn period(&self) -> usize {
        self.n
    }

    /// The frequency of hypothesis `h` in cycles per sample.
    fn freq_of(&self, h: usize) -> f64 {
        let s = (h / self.sub) as i32 - self.max_shift;
        let j = h % self.sub;
        (s as f64 + j as f64 / self.sub as f64) / self.n as f64
    }

    /// Adds one period (`period()` samples) to the search.
    pub fn accumulate(&mut self, x: &[Complex32]) -> Result<(), ModemError> {
        let n = self.n;
        if x.len() < n {
            return Err(ModemError::InputTooShort { need: n });
        }
        let scale = 1.0 / (n as f32 * n as f32);
        for j in 0..self.sub {
            // Mix down by the sub-bin fraction, then every integer shift is a rotation of the spectrum.
            let off = j as f64 / (self.sub as f64 * n as f64);
            acquire::mix_transform(&mut self.fwd, &x[..n], 0.0, off, &mut self.x);
            for si in 0..=2 * self.max_shift as usize {
                let s = si as i32 - self.max_shift;
                let shift = s.rem_euclid(n as i32) as usize;
                acquire::shift_product(&self.x, shift, &self.code, &mut self.y, false);
                self.inv.process(&mut self.y);
                let h = si * self.sub + j;
                acquire::add_power(&self.y, 0, &mut self.grid[h * n..(h + 1) * n], scale);
            }
        }
        self.periods += 1;
        Ok(())
    }

    /// The strongest cell so far, or `None` before the first period.
    pub fn result(&self) -> Option<Acquisition> {
        if self.periods == 0 {
            return None;
        }
        let n = self.n;
        let k = peak::argmax(&self.grid)?;
        let mean = self.grid.iter().map(|&v| v as f64).sum::<f64>() / self.grid.len() as f64;
        let (h, tau) = (k / n, k % n);
        let row = &self.grid[h * n..(h + 1) * n];
        let (a, b, c) = peak::neighbours(row, tau, true)?;
        // The correlation peak is a triangle in amplitude: fit the parabola to the magnitudes.
        let p = peak::parabolic((a as f64).sqrt(), (b as f64).sqrt(), (c as f64).sqrt());
        Some(Acquisition {
            code_phase: (tau as f64 + p.offset).rem_euclid(n as f64),
            freq: self.freq_of(h),
            ratio: if mean > 0.0 {
                (b as f64 / mean) as f32
            } else {
                0.0
            },
        })
    }

    /// Clears the sums.
    pub fn reset(&mut self) {
        self.grid.fill(0.0);
        self.periods = 0;
    }
}

/// The correlations of one code period from [`CodeTracker`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correlations {
    /// The early correlator (replica `spacing` chips ahead).
    pub early: Complex32,
    /// The prompt correlator: feed it to the carrier loop and the data detector.
    pub prompt: Complex32,
    /// The late correlator (`spacing` chips behind).
    pub late: Complex32,
    /// The code phase error the discriminator measured, chips (positive: the signal leads the replica).
    pub error: f32,
}

/// Settings of a [`CodeTracker`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackerConfig {
    /// Early-to-prompt spacing in chips (`0 < spacing < 1`; 0.5 is usual).
    pub spacing: f64,
    /// Loop noise bandwidth times the period (per update).
    pub bn_t: f64,
    /// Loop damping.
    pub zeta: f64,
    /// Chip rate over carrier frequency (for example `1.023e6 / 1575.42e6` for GPS L1 C/A): the code Doppler that a carrier
    /// Doppler implies. 0 for no carrier aiding.
    pub carrier_to_code: f64,
}

impl TrackerConfig {
    /// Spacing 0.5 chip, loop bandwidth `bn_t` per period, damping 0.707, no aiding.
    pub fn new(bn_t: f64) -> Self {
        TrackerConfig {
            spacing: 0.5,
            bn_t,
            zeta: std::f64::consts::FRAC_1_SQRT_2,
            carrier_to_code: 0.0,
        }
    }
}

/// A code tracker: carrier wipe-off at the frequency the caller sets, early, prompt and late correlators over each code period, and
/// a second-order delay-locked loop on the normalised early-minus-late envelope `(1 - d) (|E| - |L|) / (|E| + |L|)` (chips, exact
/// within `±d` of the peak of a rectangular-chip correlation). It is aided by the carrier: the code rate moves by
/// `freq * carrier_to_code`.
///
/// Code jitter (high SNR, `d` the spacing, `snr` the prompt's `|P|^2` over its noise variance): `sigma^2 = bn_t d / snr` chips^2.
#[derive(Clone, Debug)]
pub struct CodeTracker {
    code: Vec<f32>,
    chips: f64,
    rate0: f64,
    cfg: TrackerConfig,
    lf: LoopFilter,
    phase: f64,
    freq: f64,
    aid: f64,
    carrier: f64,
    rot: Vec<Complex32>,
    acc: [Complex32; 3],
    count: u32,
}

impl CodeTracker {
    /// A tracker for `code` at `samples_per_chip`. Errors on a bad code, spacing or loop setting.
    pub fn new(
        code: &[f32],
        samples_per_chip: f64,
        cfg: TrackerConfig,
    ) -> Result<Self, ModemError> {
        check_code(code, samples_per_chip)?;
        Param::Spacing.check_open(cfg.spacing, 0.0, 1.0)?;
        Param::CarrierToCode.check_finite(cfg.carrier_to_code)?;
        Ok(CodeTracker {
            code: code.to_vec(),
            chips: code.len() as f64,
            rate0: 1.0 / samples_per_chip,
            lf: LoopFilter::new(cfg.bn_t, cfg.zeta)?,
            cfg,
            phase: 0.0,
            freq: 0.0,
            aid: 0.0,
            carrier: 0.0,
            rot: vec![Complex32::default(); 64],
            acc: [Complex32::default(); 3],
            count: 0,
        })
    }

    /// Sets the code phase (chips into the code) of the next input sample, for example from [`Acquisition::code_phase`]
    /// (`chips = code_phase_samples / samples_per_chip`, counted back from the start of the period), and clears the loop.
    pub fn set_code_phase(&mut self, chips: f64) {
        if chips.is_finite() {
            self.phase = chips.rem_euclid(self.chips);
            self.acc = [Complex32::default(); 3];
            self.count = 0;
            self.lf.reset();
        }
    }

    /// The code phase of the next input sample, in chips.
    pub fn code_phase(&self) -> f64 {
        self.phase
    }

    /// The code rate correction the loop holds, chips per period.
    pub fn rate_correction(&self) -> f64 {
        self.lf.integrator()
    }

    /// Sets the carrier frequency to wipe off (cycles per sample), as the carrier loop measures it; it also aids the code rate.
    pub fn set_carrier(&mut self, freq: f64) {
        if freq.is_finite() {
            self.freq = freq;
            self.aid = freq;
        }
    }

    /// Sets only the carrier frequency (cycles per sample) whose Doppler the code rate follows, leaving the wipe-off where it is: the
    /// total of the wipe-off and a carrier loop's residual.
    pub fn set_code_aid(&mut self, freq: f64) {
        if freq.is_finite() {
            self.aid = freq;
        }
    }

    #[inline]
    fn chip(&self, p: f64) -> f32 {
        let i = p.rem_euclid(self.chips) as usize;
        self.code[i.min(self.code.len() - 1)]
    }

    /// Tracks a stream, appending the correlations of every completed code period to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Correlations>) {
        let d = self.cfg.spacing;
        let rate = self.rate0 + self.aid * self.cfg.carrier_to_code;
        for xc in x.chunks(64) {
            // Carrier wipe-off: a rotator from the continuous f64 phase.
            rotator(-self.carrier, -self.freq).fill(&mut self.rot[..xc.len()]);
            self.carrier = (self.carrier + self.freq * xc.len() as f64).rem_euclid(1.0);
            for (k, &v) in xc.iter().enumerate() {
                let z = v * self.rot[k];
                let p = self.phase;
                self.acc[0] += z * self.chip(p + d);
                self.acc[1] += z * self.chip(p);
                self.acc[2] += z * self.chip(p - d);
                self.count += 1;
                self.phase += rate;
                if self.phase >= self.chips {
                    self.phase -= self.chips;
                    let n = self.count.max(1) as f32;
                    let (e, pr, l) = (self.acc[0] / n, self.acc[1] / n, self.acc[2] / n);
                    let (me, ml) = (e.norm(), l.norm());
                    let err = if me + ml > 0.0 {
                        (1.0 - d) * (me - ml) as f64 / (me + ml) as f64
                    } else {
                        0.0
                    };
                    // The correction moves the next period's end; it is not wrapped, so a step back past the start of the
                    // period (a negative phase) lengthens this period instead of ending a one-sample period at once.
                    self.phase = (self.phase + self.lf.update(err)).max(-0.5 * self.chips);
                    out.push(Correlations {
                        early: e,
                        prompt: pr,
                        late: l,
                        error: err as f32,
                    });
                    self.acc = [Complex32::default(); 3];
                    self.count = 0;
                }
            }
        }
    }
}

fn barker_stream(seconds: f64) -> BenchResult {
    // 802.11b at 1 Mbit/s: 11 Mchip/s at 2 samples per chip (22 MS/s).
    let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
    let mut d = ChipDespreader::new(&code, 2.0).unwrap_or_else(|_| unreachable!());
    let x: Vec<Complex32> = (0..65_536)
        .map(|i| Complex32::new(((i * 7) % 5) as f32 - 2.0, 0.5))
        .collect();
    let mut out = Vec::with_capacity(4096);
    measure(seconds, x.len(), x.len() as f64 / 22e6, || {
        out.clear();
        d.process(&x, &mut out);
    })
}

/// The GPS PRN 1 C/A code as `+-1` chips.
fn gps_code() -> Vec<f32> {
    let mut b = [0u8; 1023];
    let _ = sdrkit_util_bits::seq::gps_ca(1, &mut b);
    let mut c = Vec::with_capacity(1023);
    chips_from_bits(&b, &mut c);
    c
}

fn tracker(seconds: f64) -> BenchResult {
    // GPS L1 C/A at 4.092 MS/s (4 samples per chip): one channel.
    let code = gps_code();
    let mut t =
        CodeTracker::new(&code, 4.0, TrackerConfig::new(0.002)).unwrap_or_else(|_| unreachable!());
    t.set_carrier(0.001);
    let x: Vec<Complex32> = (0..40_920)
        .map(|i| Complex32::new(((i * 7) % 5) as f32 - 2.0, 0.5))
        .collect();
    let mut out = Vec::with_capacity(64);
    measure(seconds, x.len(), x.len() as f64 / 4.092e6, || {
        out.clear();
        t.process(&x, &mut out);
    })
}

fn acquisition(seconds: f64) -> BenchResult {
    // GPS L1 C/A at 2.046 MS/s, +-5 kHz in half-bin steps (11 bins x 2): one 1 ms period.
    let code = gps_code();
    let mut a = CodeAcquisition::new(&code, 2.0, 5, 2).unwrap_or_else(|_| unreachable!());
    let x: Vec<Complex32> = (0..2046)
        .map(|i| Complex32::new(((i * 7) % 5) as f32 - 2.0, 0.5))
        .collect();
    measure(seconds, x.len(), 1e-3, || {
        let _ = a.accumulate(&x);
    })
}

/// The DSSS benchmarks.
pub(crate) fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "modem.dsss_barker11_despread_22m",
            run: barker_stream,
        },
        Benchmark {
            name: "modem.dsss_gps_track_channel_4m092",
            run: tracker,
        },
        Benchmark {
            name: "modem.dsss_gps_acquire_22_hyp_2m046",
            run: acquisition,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::approx;
    use sdrkit_algo_sim::rng::Rng;

    /// `e^(j 2 pi f n)` for the test signals.
    fn tone(f: f64, n: usize) -> Complex32 {
        let (s, c) = approx::sin_cos(std::f64::consts::TAU * (f * n as f64).rem_euclid(1.0));
        Complex32::new(c, s)
    }

    /// The code sampled at `spc` from code phase `phase0` (chips), `periods` long, at carrier `f`, amplitude 1, noise `n0` per sample.
    fn signal(
        code: &[f32],
        spc: f64,
        phase0: f64,
        samples: usize,
        f: f64,
        n0: f64,
        rng: &mut Rng,
    ) -> Vec<Complex32> {
        let chips = code.len() as f64;
        (0..samples)
            .map(|n| {
                let p = (phase0 + (n as f64 + 0.5) / spc).rem_euclid(chips);
                tone(f, n) * code[p as usize] + rng.complex_normal(n0)
            })
            .collect()
    }

    #[test]
    fn barker_has_its_published_autocorrelation() {
        // A Barker code: every aperiodic sidelobe is 0 or -1 for length 11.
        for k in 1..11 {
            let s: i32 = (0..11 - k)
                .map(|i| BARKER_11[i] as i32 * BARKER_11[i + k] as i32)
                .sum();
            assert!(s == 0 || s == -1, "lag {k}: {s}");
        }
        assert_eq!(BARKER_11.iter().map(|&c| c as i32).sum::<i32>(), 1);
    }

    #[test]
    fn despreading_returns_the_processing_gain() {
        let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
        let mut rng = Rng::new(2);
        for spc in [2.0f64, 2.5, 4.0] {
            let n0 = 1.0; // 0 dB per sample
            let symbols = 4000;
            let len = (11.0 * spc * symbols as f64) as usize;
            let x = signal(&code, spc, 0.0, len, 0.0, n0, &mut rng);
            let mut d = ChipDespreader::new(&code, spc).unwrap();
            let mut out = Vec::new();
            d.process(&x, &mut out);
            assert!(out.len() >= symbols - 1);
            let mean = out.iter().map(|v| v.re as f64).sum::<f64>() / out.len() as f64;
            let var = out
                .iter()
                .map(|v| (v.re as f64 - mean).powi(2) + (v.im as f64).powi(2))
                .sum::<f64>()
                / out.len() as f64;
            // Output SNR = samples per period x input SNR.
            let gain = mean * mean / var;
            assert!(
                (gain / (11.0 * spc) - 1.0).abs() < 0.1,
                "spc {spc}: gain {gain}"
            );
            // The burst form returns the same values.
            let b = d.despread_at(&x, 3.0 * 11.0 * spc).unwrap();
            assert!((b - out[3]).norm() < 1e-4, "{b} {}", out[3]);
        }
        let d = ChipDespreader::new(&code, 2.0).unwrap();
        assert!(d.despread_at(&[Complex32::default(); 10], 0.0).is_err());
        assert!(ChipDespreader::new(&code, 0.5).is_err());
    }

    #[test]
    /// IEEE 802.15.4-2020 table 12-1 chips, received as MSK: the chip-sign patterns of symbols 8 to 15 are those of 0 to 7
    /// negated, so a bank of 8 decides all 16 symbols.
    fn code_bank_decides_signed_symbols() {
        const CHIPS: [u32; 16] = [
            0xD9C3522E, 0xED9C3522, 0x2ED9C352, 0x22ED9C35, 0x522ED9C3, 0x3522ED9C, 0xC3522ED9,
            0x9C3522ED, 0x8C96077B, 0xB8C96077, 0x7B8C9607, 0x77B8C960, 0x077B8C96, 0x6077B8C9,
            0x96077B8C, 0xC96077B8,
        ];
        let chip = |s: usize, n: usize| {
            if (CHIPS[s] >> (31 - n)) & 1 != 0 {
                1.0f32
            } else {
                -1.0
            }
        };
        // The sign of the phase step during chip n (n >= 1): chip n times chip n - 1, negated on even n.
        let pattern = |s: usize| -> Vec<f32> {
            (1..32)
                .map(|n| chip(s, n) * chip(s, n - 1) * if (n + 1) % 2 == 0 { 1.0 } else { -1.0 })
                .collect()
        };
        for s in 0..8 {
            let neg: Vec<f32> = pattern(s + 8).iter().map(|v| -v).collect();
            assert_eq!(pattern(s), neg, "symbol {s}");
        }
        let codes: Vec<Vec<f32>> = (0..8).map(pattern).collect();
        let refs: Vec<&[f32]> = codes.iter().map(|c| c.as_slice()).collect();
        let bank = CodeBank::new(&refs, 2.0).unwrap();
        let rot = Complex32::from_polar(1.0, 0.7);
        let mut v = [0f32; 8];
        for s in 0..16 {
            let x: Vec<Complex32> = pattern(s)
                .iter()
                .flat_map(|&c| [rot * c, rot * c])
                .collect();
            let d = bank.decide_signed(&x, 0.0, rot, &mut v).unwrap();
            assert_eq!(d.symbol, s);
            assert!((d.value.abs() - 1.0).abs() < 1e-5);
        }
        assert!(bank.decide_signed(&[], 0.0, rot, &mut v).is_err());
        assert!(bank
            .decide_signed(&[Complex32::default(); 62], 0.0, rot, &mut v[..4])
            .is_err());
    }

    #[test]
    fn code_bank_decides_the_symbol() {
        // Four orthogonal Walsh codes of length 8, the third sent with an unknown carrier phase.
        let w: Vec<Vec<f32>> = (0..4u32)
            .map(|r| {
                (0..8u32)
                    .map(|c| {
                        if (r & c).count_ones() % 2 == 0 {
                            1.0
                        } else {
                            -1.0
                        }
                    })
                    .collect()
            })
            .collect();
        let refs: Vec<&[f32]> = w.iter().map(|c| c.as_slice()).collect();
        let bank = CodeBank::new(&refs, 3.0).unwrap();
        let x: Vec<Complex32> = (0..24)
            .map(|n| Complex32::from_polar(1.0, 2.0) * w[2][n / 3])
            .collect();
        let mut e = [0f32; 4];
        assert_eq!(bank.correlate_at(&x, 0.0, &mut e).unwrap(), 2);
        assert!((e[2] - 1.0).abs() < 1e-5 && e[0] < 1e-6);
    }

    #[test]
    fn acquisition_finds_code_phase_and_doppler() {
        let code = gps_code();
        let mut a = CodeAcquisition::new(&code, 2.0, 5, 2).unwrap();
        let n = a.period();
        assert_eq!(n, 2046);
        let mut rng = Rng::new(4);
        let f = 2.6 / n as f64; // 2.6 bins
                                // The code starts 700.0 samples into each period: phase0 such that chip 0 is at sample 700.
        let phase0 = -700.0 / 2.0;
        let x = signal(&code, 2.0, phase0, 4 * n, f, 10f64.powf(1.5), &mut rng); // -15 dB per sample
        for k in 0..4 {
            a.accumulate(&x[k * n..(k + 1) * n]).unwrap();
        }
        let r = a.result().unwrap();
        assert!((r.code_phase - 700.0).abs() < 0.6, "{r:?}");
        assert!(((r.freq - f) * n as f64).abs() <= 0.26, "{r:?}");
        assert!(r.ratio > 15.0, "{r:?}");
        a.reset();
        assert!(a.result().is_none());
        assert!(a.accumulate(&x[..10]).is_err());
    }

    #[test]
    fn delay_lock_pulls_in_and_its_jitter_follows_theory() {
        let mut rng = Rng::new(8);
        let spc = 4.3;
        let n0 = 100.0;
        let snr_post = 1023.0 * spc / n0;
        for bn_t in [0.02f64, 0.08] {
            // Run several independent tracks, recording the replica phase error against the truth after settling.
            let code = gps_code();
            let per_chips = 1023.0;
            let periods = 1500usize;
            let mut t = CodeTracker::new(&code, spc, TrackerConfig::new(bn_t)).unwrap();
            t.set_code_phase(0.3); // the replica starts 0.3 chip ahead: the signal lags it
            let samples = (periods as f64 * per_chips * spc) as usize;
            let x = signal(&code, spc, 0.0, samples, 0.0, n0, &mut rng);
            let mut out = Vec::new();
            let mut errs = Vec::new();
            let mut n = 0usize;
            for c in x.chunks(4096) {
                t.process(c, &mut out);
                n += c.len();
                if out.len() > 300 {
                    // Truth: the code phase of the next sample.
                    let truth = ((n as f64 + 0.5) / spc).rem_euclid(per_chips);
                    let e = (t.code_phase() - truth + 511.5).rem_euclid(per_chips) - 511.5;
                    errs.push(e);
                }
            }
            let mean = errs.iter().sum::<f64>() / errs.len() as f64;
            let var = errs.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / errs.len() as f64;
            let th = bn_t * 0.5 / snr_post;
            assert!(mean.abs() < 0.05, "bn {bn_t}: bias {mean}");
            assert!(
                (var / th).sqrt() > 0.7 && (var / th).sqrt() < 1.3,
                "bn {bn_t}: rms {} theory {}",
                var.sqrt(),
                th.sqrt()
            );
        }
    }

    #[test]
    fn carrier_aiding_follows_code_doppler() {
        // A carrier offset of f cycles per sample with carrier_to_code k moves the code by f k chips per sample; with aiding the
        // loop holds no rate correction.
        let spc = 4.0;
        let code = gps_code();
        let k = 1.023e6 / 1575.42e6;
        let f = 2e-3;
        let mut rng = Rng::new(12);
        let samples = 300 * 4092;
        let chips = 1023.0;
        let x: Vec<Complex32> = (0..samples)
            .map(|n| {
                let p = ((n as f64 + 0.5) * (1.0 / spc + f * k)).rem_euclid(chips);
                tone(f, n) * code[p as usize] + rng.complex_normal(1.0)
            })
            .collect();
        let mut cfg = TrackerConfig::new(0.05);
        cfg.carrier_to_code = k;
        let mut t = CodeTracker::new(&code, spc, cfg).unwrap();
        t.set_carrier(f);
        let mut out = Vec::new();
        t.process(&x, &mut out);
        let truth = ((samples as f64 + 0.5) * (1.0 / spc + f * k)).rem_euclid(chips);
        let e = (t.code_phase() - truth + 511.5).rem_euclid(chips) - 511.5;
        assert!(e.abs() < 0.1, "{e}");
        assert!(t.rate_correction().abs() < 0.01, "{}", t.rate_correction());
        // The prompt is wiped off: it stays on the real axis.
        let p = out.last().unwrap().prompt;
        assert!(p.re > 0.5 && p.im.abs() < 0.2, "{p}");
    }
}
