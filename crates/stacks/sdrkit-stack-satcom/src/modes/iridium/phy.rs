//! The Iridium physical layer: the channel plan, DE-QPSK at 25 000 symbol/s with RRC 0.4, the unique words, the FFT burst detector
//! and the per-burst demodulator.

use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::spectrum::{Scaling, SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;
use std::f64::consts::{FRAC_PI_2, TAU};

pub const SYMBOL_RATE: f64 = 25_000.0;
pub const CHANNEL_HZ: f64 = 10e6 / 240.0;
pub const BAND_LO: f64 = 1616.0e6;
/// The unique words as bits after differential decoding.
pub const UW_DOWN: &str = "001100000011000011110011";
pub const UW_UP: &str = "110011000011110011111100";

/// The centre of channel `ch` (0 to 251).
pub fn channel_hz(ch: usize) -> f64 {
    BAND_LO + CHANNEL_HZ * (ch as f64 + 0.5)
}

/// The channel nearest a frequency.
pub fn channel_of(f: f64) -> usize {
    (((f - BAND_LO) / CHANNEL_HZ - 0.5).round().max(0.0) as usize).min(251)
}

/// The bits of a phase step of `k` quarter turns counter-clockwise (first bit = high bit of the pair).
pub fn step_bits(k: u8) -> [bool; 2] {
    match k & 3 {
        0 => [false, false],
        1 => [true, false],
        2 => [true, true],
        _ => [false, true],
    }
}

/// The step of a pair of bits.
pub fn bits_step(b0: bool, b1: bool) -> u8 {
    match (b0, b1) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

/// The steps of a unique word given as 24 bit characters.
const fn steps_of(uw: &str) -> [u8; 12] {
    let b = uw.as_bytes();
    let mut out = [0u8; 12];
    let mut i = 0;
    while i < 12 {
        out[i] = match (b[2 * i] == b'1', b[2 * i + 1] == b'1') {
            (false, false) => 0,
            (true, false) => 1,
            (true, true) => 2,
            (false, true) => 3,
        };
        i += 1;
    }
    out
}

const UW_DOWN_STEPS: [u8; 12] = steps_of(UW_DOWN);
const UW_UP_STEPS: [u8; 12] = steps_of(UW_UP);

/// The steps of a unique word (12 symbols).
pub fn uw_steps(downlink: bool) -> &'static [u8; 12] {
    if downlink {
        &UW_DOWN_STEPS
    } else {
        &UW_UP_STEPS
    }
}

/// A burst the detector found (indices into the input).
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub start: u64,
    pub end: u64,
    /// Centre, Hz from the input's 0 Hz.
    pub centre: f64,
    pub peak_db: f64,
}

/// A burst being followed by the detector: first and last bin, first and last frame end, frames since it was last seen, peak.
#[derive(Clone, Copy, Debug)]
struct Open {
    lo: usize,
    hi: usize,
    start: u64,
    end: u64,
    quiet: u32,
    peak: f32,
}

/// The FFT burst detector: power per bin over a per-bin noise floor. A burst starts where a bin rises over the threshold and ends
/// when its bins have fallen back for two frames.
pub struct Detector {
    /// The FFT size.
    pub n: usize,
    rate: f64,
    spectrum: SpectrumAnalyzer,
    floor: Vec<f32>,
    frames: u64,
    /// The input index of the first sample.
    origin: Option<u64>,
    active: Vec<Open>,
    hot: Vec<(usize, usize, f32)>,
    /// Burst level over the noise floor needed, dB.
    pub threshold_db: f64,
    /// Bursts found twice (a strong burst's skirt) and counted once.
    pub duplicates: u64,
    /// The mean noise floor, power per Hz (full scale 1).
    pub noise_per_hz: f64,
}

impl Detector {
    /// A detector for input at `rate`: contiguous frames of about 10 kHz bins (256 to 2048 points). Errors for a rate that is not
    /// positive.
    pub fn new(rate: f64) -> Result<Self, DspError> {
        let n = ((rate / 10e3) as usize)
            .next_power_of_two()
            .clamp(256, 2048);
        let mut cfg = SpectrumConfig::new(n, rate);
        cfg.window = Window::Hann;
        cfg.scaling = Scaling::Power;
        cfg.frames_per_second = rate / n as f64;
        Ok(Detector {
            n,
            rate,
            spectrum: SpectrumAnalyzer::new(cfg)?,
            floor: vec![0.0; n],
            frames: 0,
            origin: None,
            active: Vec::with_capacity(64),
            hot: Vec::with_capacity(64),
            threshold_db: 13.0,
            duplicates: 0,
            noise_per_hz: 0.0,
        })
    }

    /// Takes input samples starting at absolute index `first` and appends the bursts that ended to `out`.
    pub fn process(&mut self, x: &[Complex32], first: u64, out: &mut Vec<Found>) {
        let n = self.n;
        let origin = *self.origin.get_or_insert(first);
        for part in x.chunks(n) {
            // A part of at most one frame completes at most one frame.
            if self.spectrum.push(part) > 0 {
                self.frames += 1;
                let end = origin + self.frames * n as u64;
                self.frame(end, out);
            }
        }
    }

    fn frame(&mut self, end: u64, out: &mut Vec<Found>) {
        let n = self.n;
        let start = end - n as u64;
        let pow = self.spectrum.power();
        let thr = 10f32.powf(self.threshold_db as f32 / 10.0);
        // The per-bin floor: the minimum-tracking average (it follows the noise, not the bursts).
        for (f, &p) in self.floor.iter_mut().zip(pow.iter()) {
            if self.frames <= 8 {
                // The first frames set the floor: their mean.
                *f += (p - *f) / self.frames as f32;
            } else if p < *f * 4.0 {
                *f += 0.05 * (p - *f);
            } else {
                // A burst: the floor rises only slowly.
                *f *= 1.002;
            }
        }
        if self.frames < 8 {
            return;
        }
        let mean_floor = self.floor.iter().sum::<f32>() / n as f32;
        self.noise_per_hz = mean_floor as f64 * n as f64 / self.rate;
        // Bins over the threshold, grouped.
        self.hot.clear();
        let edge = (0.05 * n as f64) as usize;
        let mut k = edge;
        while k < n - edge {
            if pow[k] > self.floor[k] * thr {
                let s = k;
                let mut pk = 0.0f32;
                while k < n && pow[k] > self.floor[k] * thr * 0.25 {
                    pk = pk.max(pow[k] / self.floor[k]);
                    k += 1;
                }
                self.hot.push((s, k - 1, pk));
            } else {
                k += 1;
            }
        }
        for a in self.active.iter_mut() {
            a.quiet += 1;
        }
        for &(s, e, pk) in &self.hot {
            match self
                .active
                .iter_mut()
                .find(|a| s <= a.hi + 1 && e + 1 >= a.lo)
            {
                Some(a) => {
                    a.lo = a.lo.min(s);
                    a.hi = a.hi.max(e);
                    a.end = end;
                    a.quiet = 0;
                    a.peak = a.peak.max(pk);
                }
                None => self.active.push(Open {
                    lo: s,
                    hi: e,
                    start,
                    end,
                    quiet: 0,
                    peak: pk,
                }),
            }
        }
        // Ended: two frames back at the noise.
        let bin = self.rate / n as f64;
        let mut i = 0;
        while i < self.active.len() {
            let a = self.active[i];
            if a.quiet < 2 {
                i += 1;
                continue;
            }
            self.active.swap_remove(i);
            let c = ((a.lo + a.hi) as f64 / 2.0 - (n / 2) as f64) * bin;
            // A strong burst's skirt found again next to it is counted once.
            if out.iter().any(|o: &Found| {
                (o.centre - c).abs() < CHANNEL_HZ && o.start.abs_diff(a.start) < n as u64
            }) {
                self.duplicates += 1;
                continue;
            }
            out.push(Found {
                start: a.start.saturating_sub(n as u64),
                end: a.end + n as u64,
                centre: c,
                peak_db: db(a.peak as f64),
            });
        }
    }
}

/// A demodulated burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Burst {
    pub found: bool,
    pub uw_ok: bool,
    pub downlink: bool,
    /// The bits after the unique word.
    pub bits: Vec<bool>,
    pub data_symbols: usize,
    /// Refined centre, Hz from the input's 0 Hz.
    pub centre: f64,
    /// Time of the first unique word symbol, input sample index.
    pub uw_at: f64,
    /// Percent of symbols within 22 degrees of a constellation point.
    pub confidence: u32,
    pub esn0_db: f64,
    pub level_db: f64,
    /// The unique word and data as received bits (for the RAW line).
    pub raw_bits: Vec<bool>,
}

/// The samples per symbol of the burst demodulator.
pub const BURST_SPS: f64 = 10.0;
/// The most samples (at 250 ksps) of a burst used for the carrier estimate.
const N4: usize = 8192;

/// The per-burst demodulator: mix to 0 Hz and bring to 10 samples per symbol, the carrier from the burst's fourth power, the RRC 0.4
/// matched filter, symbol timing by the widest eye, the unique word by differential correlation (both directions), DE-QPSK to
/// bits. Its buffers are made once, so a burst allocates only its own bits.
pub struct BurstDemod {
    rate: f64,
    chan: Channelizer,
    fft: Fft,
    mf: Fir<Complex32>,
    y: Vec<Complex32>,
    f4: Vec<Complex32>,
    z: Vec<Complex32>,
    syms: Vec<Complex32>,
    steps: Vec<(u8, f64)>,
}

impl BurstDemod {
    /// A demodulator for input at `rate`. Errors when the rate cannot give 250 ksps.
    pub fn new(rate: f64) -> Result<Self, DspError> {
        let fs = BURST_SPS * SYMBOL_RATE;
        let mut cc = ChannelConfig::new(rate, 0.0, 160e3, fs);
        cc.exact_rate = true;
        let mf = taps(Shape::Rrc { beta: 0.4 }, BURST_SPS, 8.0, Norm::Dc)?;
        Ok(BurstDemod {
            rate,
            chan: Channelizer::new(cc)?,
            fft: Fft::forward(N4),
            mf: Fir::new(&mf),
            y: Vec::with_capacity(N4),
            f4: vec![Complex32::new(0.0, 0.0); N4],
            z: Vec::with_capacity(N4),
            syms: Vec::with_capacity(N4 / 10),
            steps: Vec::with_capacity(N4 / 10),
        })
    }

    /// Demodulates the burst `x` (input samples, the first at absolute index `first`) whose centre is near `centre` Hz from the
    /// input's 0 Hz.
    pub fn demodulate(&mut self, x: &[Complex32], centre: f64, first: u64) -> Burst {
        let rate = self.rate;
        let fs = BURST_SPS * SYMBOL_RATE;
        self.chan.set_offset(centre);
        self.chan.reset();
        self.y.clear();
        self.chan.process(x, &mut self.y);
        self.y.truncate(N4);
        let mut b = Burst {
            centre,
            ..Default::default()
        };
        let y = &self.y;
        if y.len() < 400 {
            return b;
        }
        // The carrier from the fourth power's line (within +-30 kHz).
        for (i, f) in self.f4.iter_mut().enumerate() {
            *f = y.get(i).map_or(Complex32::new(0.0, 0.0), |v| v.powi(4));
        }
        self.fft.process(&mut self.f4);
        let bin = fs / N4 as f64;
        let lim = (120e3 / bin) as i64;
        let mut best = (0i64, 0.0f32);
        for k in -lim..=lim {
            let v = self.f4[k.rem_euclid(N4 as i64) as usize].norm_sqr();
            if v > best.1 {
                best = (k, v);
            }
        }
        let df = best.0 as f64 * bin / 4.0;
        // Mix the residual away and filter.
        let mut nco = Nco::new(fs, -df);
        self.mf.reset();
        self.z.clear();
        for v in y {
            self.z.push(self.mf.process_sample(v * nco.next_sample()));
        }
        let delay = self.mf.len() / 2;
        let z = &self.z[delay.min(self.z.len())..];
        // Symbol timing: the instant of the widest eye, on the sample grid.
        let Ok(at) = sdrkit_algo_modem::timing::max_energy(z, BURST_SPS, 10) else {
            return b;
        };
        let ph = (at.round() as usize) % BURST_SPS as usize;
        self.syms.clear();
        self.syms
            .extend(z.iter().skip(ph).step_by(BURST_SPS as usize).copied());
        let syms = &self.syms;
        if syms.len() < 30 {
            return b;
        }
        // Residual frequency from the differential products' fourth power.
        let mut acc = Complex32::new(0.0, 0.0);
        for w in syms.windows(2) {
            let d = w[1] * w[0].conj();
            acc += d.powi(4) / (d.norm().powi(3) + 1e-20);
        }
        let rot = (acc.arg() as f64) / 4.0;
        self.steps.clear();
        self.steps.extend(syms.windows(2).map(|w| {
            let a = ((w[1] * w[0].conj()).arg() as f64 - rot).rem_euclid(TAU);
            let k = (a / FRAC_PI_2).round() as u8 % 4;
            let e = (a - k as f64 * FRAC_PI_2)
                .abs()
                .min(TAU - (a - k as f64 * FRAC_PI_2).abs());
            (k, e)
        }));
        let steps = &self.steps;
        // Unique word search: a 90 degree error counts 1 and a 180 degree error 2. A sum of 2 or less is accepted.
        let mut best_uw: Option<(usize, bool, u32)> = None;
        for dl in [true, false] {
            let uw = uw_steps(dl);
            for s in 0..steps.len().saturating_sub(uw.len()) {
                let mut err = 0u32;
                for (j, &u) in uw.iter().enumerate() {
                    err += match (steps[s + j].0 + 4 - u) % 4 {
                        0 => 0,
                        2 => 2,
                        _ => 1,
                    };
                    if err > 2 {
                        break;
                    }
                }
                if err <= 2 && best_uw.is_none_or(|b| err < b.2) {
                    best_uw = Some((s, dl, err));
                }
                if best_uw.is_some_and(|b| b.2 == 0) {
                    break;
                }
            }
        }
        let Some((s, dl, _)) = best_uw else {
            return b;
        };
        b.found = true;
        b.uw_ok = true;
        b.downlink = dl;
        let uwl = uw_steps(dl).len();
        // The burst ends where the energy falls.
        let level = syms[s..s + uwl].iter().map(|v| v.norm_sqr()).sum::<f32>() / uwl as f32;
        let mut end = s + uwl;
        while end < steps.len() && syms[end + 1].norm_sqr() > 0.2 * level {
            end += 1;
        }
        let data = &steps[s + uwl..end];
        b.data_symbols = data.len();
        for &(k, _) in data {
            b.bits.extend(step_bits(k));
        }
        for &(k, _) in &steps[s..end] {
            b.raw_bits.extend(step_bits(k));
        }
        let good = steps[s..end]
            .iter()
            .filter(|x| x.1 < 22f64.to_radians())
            .count();
        b.confidence = (100 * good / (end - s).max(1)) as u32;
        let err: f64 =
            steps[s..end].iter().map(|x| x.1 * x.1).sum::<f64>() / (end - s).max(1) as f64;
        b.esn0_db = db(1.0 / (2.0 * err).max(1e-6));
        b.level_db = db(level as f64);
        b.centre = centre + df + rot / TAU * SYMBOL_RATE;
        // The first unique word symbol: s symbols in, at the chosen phase.
        b.uw_at = first as f64 + ((s as f64 * BURST_SPS) + ph as f64) * rate / fs;
        b
    }
}
