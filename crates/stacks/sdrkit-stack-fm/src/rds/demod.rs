//! The RDS physical layer: the 57 kHz subcarrier to baseband, the matched filter, symbol timing and the differential decoding.
//! The carrier is the third harmonic of the pilot loop while it is locked, else the receiver's own oscillator with a Costas
//! loop updated once per symbol.
//!
//! Standard: IEC 62106 (EN 50067 clause 1: modulation and baseband coding).

use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::{FRAC_1_SQRT_2, TAU};

/// The RDS bit rate (57 kHz / 48).
pub const BIT_RATE: f64 = 1187.5;
/// Samples per bit after the resampler.
pub const SPS: usize = 16;
/// The rate the matched filter runs at.
pub const SYMBOL_SAMPLE_RATE: f64 = BIT_RATE * SPS as f64;
/// The Costas loop's noise bandwidth before block sync and after it, Hz.
pub const COSTAS_BN_HZ: (f64, f64) = (20.0, 5.0);
/// The furthest the Costas loop pulls the carrier, Hz.
pub const COSTAS_MAX_HZ: f64 = 25.0;

/// A loop filter of noise bandwidth `bn_hz` updated once per bit.
fn costas_filter(bn_hz: f64) -> LoopFilter {
    let mut lf = LoopFilter::new(bn_hz / BIT_RATE, FRAC_1_SQRT_2)
        .unwrap_or_else(|e| unreachable!("the Costas bandwidths are fixed and valid: {e}"));
    lf.set_clamp(COSTAS_MAX_HZ / BIT_RATE);
    lf
}

/// The RDS demodulator, from MPX samples to data bits with reliabilities.
#[derive(Clone, Debug)]
pub struct RdsDemod {
    mpx_rate: f64,
    nco: PhaseAcc,
    lpf: Fir<Complex32>,
    decim: usize,
    count: usize,
    res: FractionalResampler<Complex32>,
    res_out: Vec<Complex32>,
    // Carrier phase.
    pilot_mode: bool,
    sq_avg: (f64, f64),
    costas: LoopFilter,
    costas_nco: PhaseAcc,
    costas_narrow: bool,
    // Matched filter and timing.
    mf: Fir<Complex32>,
    energy: [f32; SPS],
    best: usize,
    sample: u64,
    since: u32,
    prev_sym: bool,
    mean_abs: f32,
    points: VecDeque<Complex32>,
    /// Bits decided since the caller last drained them: (data bit, reliability).
    pub bits: Vec<(bool, f32)>,
    synced: bool,
}

impl RdsDemod {
    /// A demodulator for MPX samples at `mpx_rate` (at least 50 kHz).
    pub fn new(mpx_rate: f64) -> Self {
        let decim = ((mpx_rate / 25e3).round() as usize).max(1);
        let rate = mpx_rate / decim as f64;
        let lpf = Fir::new(
            &sdrkit_algo_dsp::fir::lowpass(mpx_rate, 3200.0, 10_000.0, 60.0)
                .expect("the filter settings are valid"),
        );
        let res = FractionalResampler::new(rate, SYMBOL_SAMPLE_RATE, 3200.0, 9000.0, 60.0)
            .unwrap_or_else(|e| unreachable!("the RDS resampler fits every MPX rate: {e}"));
        // One sine period, time-reversed: taps[0] meets the newest sample.
        let shape: Vec<f32> = (0..SPS)
            .map(|i| (TAU * (i as f64 + 0.5) / SPS as f64).sin() as f32 / 8.0)
            .collect();
        let taps: Vec<f32> = (0..SPS).map(|k| shape[SPS - 1 - k]).collect();
        RdsDemod {
            mpx_rate,
            nco: PhaseAcc::new(57_000.0 / mpx_rate),
            lpf,
            decim,
            count: 0,
            res,
            res_out: Vec::with_capacity(4),
            pilot_mode: false,
            sq_avg: (0.0, 0.0),
            costas: costas_filter(COSTAS_BN_HZ.0),
            costas_nco: PhaseAcc::new(0.0),
            costas_narrow: false,
            mf: Fir::new(&taps),
            energy: [0.0; SPS],
            best: 0,
            sample: 0,
            since: 0,
            prev_sym: false,
            mean_abs: 0.0,
            points: VecDeque::with_capacity(300),
            bits: Vec::new(),
            synced: false,
        }
    }

    /// Back to the state after construction.
    pub fn reset(&mut self) {
        *self = Self::new(self.mpx_rate);
    }

    /// Demodulates MPX samples with the 57 kHz phasor of the pilot loop for each sample, zero while the pilot is unlocked (the
    /// stereo component's `carrier` output). Each decided bit goes to `sink` with its reliability. The sink returns whether the
    /// block layer is synchronised, which narrows the Costas loop.
    pub fn process(
        &mut self,
        mpx: &[f32],
        carrier: &[Complex32],
        sink: &mut dyn FnMut(bool, f32) -> bool,
    ) {
        for (&x, &c) in mpx.iter().zip(carrier) {
            let locked = c.re != 0.0 || c.im != 0.0;
            self.push(x, c, locked, self.synced);
            if !self.bits.is_empty() {
                for i in 0..self.bits.len() {
                    let (b, r) = self.bits[i];
                    self.synced = sink(b, r);
                }
                self.bits.clear();
            }
        }
    }

    /// The newest normalised matched-filter outputs (clean symbols sit near +1 and -1 on the real axis), oldest first, at most 200.
    pub fn points(&self) -> impl Iterator<Item = &Complex32> {
        self.points
            .iter()
            .skip(self.points.len().saturating_sub(200))
    }

    /// One MPX sample with the 57 kHz phasor of the pilot loop and whether the pilot is locked. `synced` (RDS block sync)
    /// narrows the Costas loop. Bits go to [`bits`](Self::bits).
    #[inline]
    pub fn push(&mut self, x: f32, carrier: Complex32, locked: bool, synced: bool) {
        let own = self.nco.next_phasor();
        let lo = if locked { carrier.conj() } else { own.conj() };
        self.lpf.push(lo * (2.0 * x));
        self.count += 1;
        if self.count < self.decim {
            return;
        }
        self.count = 0;
        let y = self.lpf.output();
        self.res_out.clear();
        self.res.process(&[y], &mut self.res_out);
        for i in 0..self.res_out.len() {
            let z = self.res_out[i];
            self.symbol_sample(z, locked, synced);
        }
    }

    fn symbol_sample(&mut self, y: Complex32, locked: bool, synced: bool) {
        if locked != self.pilot_mode {
            self.pilot_mode = locked;
            self.costas.reset();
            self.costas_nco = PhaseAcc::new(0.0);
        }
        let z = if self.pilot_mode {
            let (re, im) = (y.re as f64, y.im as f64);
            let (sr, si) = (re * re - im * im, 2.0 * re * im);
            self.sq_avg.0 += (sr - self.sq_avg.0) / 4000.0;
            self.sq_avg.1 += (si - self.sq_avg.1) / 4000.0;
            let ph = approx::atan2(self.sq_avg.1 as f32, self.sq_avg.0 as f32) as f64 / 2.0;
            let (s, c) = approx::sin_cos(ph);
            y * Complex32::new(c, -s)
        } else {
            let z = self.costas_nco.derotate(y);
            self.costas_nco
                .set_freq(self.costas.integrator() / SPS as f64);
            self.costas_nco.advance();
            z
        };
        let m = self.mf.process_sample(z);
        let phase = (self.sample % SPS as u64) as usize;
        self.sample += 1;
        let e = &mut self.energy[phase];
        *e += 0.01 * (m.norm_sqr() - *e);
        let (arg, emax) = self
            .energy
            .iter()
            .enumerate()
            .fold(
                (0, f32::MIN),
                |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc },
            );
        if emax > 1.15 * self.energy[self.best] {
            self.best = arg;
        }
        self.since += 1;
        let d = (phase + SPS - self.best) % SPS;
        if d <= 2 && self.since >= 8 {
            self.since = 0;
            self.decide(m, synced);
        }
    }

    fn decide(&mut self, m: Complex32, synced: bool) {
        self.mean_abs += 0.02 * (m.re.abs() - self.mean_abs);
        let norm = self.mean_abs.max(1e-12);
        let p = m / norm;
        if self.points.len() >= 300 {
            self.points.pop_front();
        }
        self.points.push_back(p);
        if !self.pilot_mode {
            if synced != self.costas_narrow {
                self.costas_narrow = synced;
                let bn = if synced {
                    COSTAS_BN_HZ.1
                } else {
                    COSTAS_BN_HZ.0
                };
                let _ = self.costas.set_bandwidth(bn / BIT_RATE, FRAC_1_SQRT_2, 1.0);
            }
            // The Costas error for BPSK, radians, as cycles for the loop.
            let err = (if p.re >= 0.0 { p.im } else { -p.im }).clamp(-1.0, 1.0) as f64 / TAU;
            let v = self.costas.update(err);
            self.costas_nco.steer(v - self.costas.integrator());
        }
        let sym = m.re > 0.0;
        let bit = sym != self.prev_sym;
        self.prev_sym = sym;
        self.bits.push((bit, p.re.abs()));
    }
}
