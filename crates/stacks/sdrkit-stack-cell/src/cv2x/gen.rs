//! The C-V2X test signal: LTE sidelink-shaped subframes. Each busy subframe holds 13 SC-FDMA symbols (DFT-precoded random
//! QPSK, constant-modulus DM-RS symbols at subframe symbols 2, 5, 8 and 11) with the half-subcarrier shift and the normal
//! prefixes, and an empty 14th symbol. The busy subframes are chosen at random. Unit power while on.

use super::{ChannelWidth, CHANNEL_MHZ};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::{PI, TAU};

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";

/// The share of busy subframes.
pub const GEN_BUSY: Setting<field::F64> = Setting::new("gen_busy", "Test signal busy share", "%", 20.0)
    .range(1.0, 100.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The last symbol of each subframe is empty (sidelink); off gives the LTE uplink shape.
pub const GEN_GUARD: Setting<field::Bool> = Setting::new("gen_guard", "Test signal guard symbol", "", true)
    .section(TEST_SIGNAL)
    .advanced();
/// The resource blocks of the channel.
pub const GEN_RESOURCE_BLOCKS: Setting<field::I64> =
    Setting::new("gen_resource_blocks", "Test signal resource blocks", "", 50)
        .range(1.0, 100.0, 1.0)
        .help("50 in a 10 MHz channel; not confirmed by a public document read for this stack.")
        .section(TEST_SIGNAL)
        .advanced();

/// The test signal settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_BUSY, &GEN_GUARD, &GEN_RESOURCE_BLOCKS];

/// Subframe symbols that carry the DM-RS (TS 36.211 9.8).
const DMRS: [usize; 4] = [2, 5, 8, 11];

/// The generator.
pub struct Cv2xGenerator {
    rate: f64,
    width: ChannelWidth,
    busy: f64,
    guard: bool,
    used: usize,
    idft: Fft,
    precode: Fft,
    rng: Rng,
    sym: Vec<Complex32>,
    data: Vec<Complex32>,
    work: Vec<Complex32>,
    out: Vec<Complex32>,
    pos: usize,
    resampler: Option<Resampler<Complex32>>,
}

impl Cv2xGenerator {
    /// The generator of the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below the working rate of the channel width.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        let s = &cfg.settings;
        let width = s.get(&CHANNEL_MHZ);
        let work = width.rate_hz();
        if cfg.rate_hz < work * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: work,
                got: cfg.rate_hz,
            });
        }
        let n = width.fft_len();
        let used = (12 * s.get(&GEN_RESOURCE_BLOCKS).clamp(1, (n / 12 - 2) as i64) as usize).min(n - 2);
        let resampler = if (cfg.rate_hz - work).abs() > 1e-6 {
            Some(Resampler::new(work, cfg.rate_hz).map_err(|_| ConfigError::RateTooLow {
                need: work,
                got: cfg.rate_hz,
            })?)
        } else {
            None
        };
        Ok(Cv2xGenerator {
            rate: cfg.rate_hz,
            width,
            busy: (s.get(&GEN_BUSY) / 100.0).clamp(0.0, 1.0),
            guard: s.get(&GEN_GUARD),
            used,
            idft: Fft::inverse(n),
            precode: Fft::forward(used),
            rng: Rng::new(cfg.seed ^ 0x5632_5856),
            sym: vec![Complex32::default(); n],
            data: vec![Complex32::default(); used],
            work: Vec::new(),
            out: Vec::new(),
            pos: 0,
            resampler,
        })
    }

    /// Appends one SC-FDMA symbol (subframe symbol `l`) with prefix `cp` to `work`, or zeros when `on` is false.
    fn symbol(&mut self, l: usize, cp: usize, on: bool) {
        let n = self.sym.len();
        let start = self.work.len();
        if !on {
            self.work.resize(start + cp + n, Complex32::default());
            return;
        }
        let m = self.used;
        if DMRS.contains(&l) {
            // A constant-modulus sequence on the carriers: a chirp with a random start.
            let u = (self.rng.next_u64() % 29 + 1) as f64;
            for (k, d) in self.data.iter_mut().enumerate() {
                let ph = -PI * u * (k * (k + 1)) as f64 / m as f64;
                *d = Complex32::from_polar(1.0, ph as f32);
            }
        } else {
            for d in self.data.iter_mut() {
                let q = self.rng.next_u64();
                *d = Complex32::new(
                    if q & 1 == 0 { 1.0 } else { -1.0 },
                    if q & 2 == 0 { 1.0 } else { -1.0 },
                ) * std::f32::consts::FRAC_1_SQRT_2;
            }
            self.precode.process(&mut self.data);
            let g = 1.0 / (m as f32).sqrt();
            self.data.iter_mut().for_each(|d| *d *= g);
        }
        // Carrier k of -m/2 .. m/2 - 1 sits at (k + 1/2) carrier spacings: FFT bin k, and the half-carrier turn in time.
        self.sym.fill(Complex32::default());
        let g = 1.0 / (m as f32).sqrt();
        for (i, &d) in self.data.iter().enumerate() {
            let k = i as i64 - (m / 2) as i64;
            self.sym[k.rem_euclid(n as i64) as usize] = d * g;
        }
        self.idft.process(&mut self.sym);
        self.work.extend_from_slice(&self.sym[n - cp..]);
        self.work.extend_from_slice(&self.sym);
        for (i, v) in self.work[start..].iter_mut().enumerate() {
            let ph = TAU * 0.5 * (i as f64 - cp as f64) / n as f64;
            *v *= Complex32::from_polar(1.0, ph as f32);
        }
    }

    /// Appends one 1 ms subframe to `work`.
    fn subframe(&mut self) {
        let on = self.rng.uniform() < self.busy;
        let pre = self.width.prefixes();
        for l in 0..14 {
            let last = l == 13;
            self.symbol(l, pre[l % 7], on && !(last && self.guard));
        }
    }
}

impl Generator for Cv2xGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.out.clear();
                self.pos = 0;
                self.work.clear();
                for _ in 0..10 {
                    self.subframe();
                }
                match self.resampler.as_mut() {
                    Some(r) => r.process(&self.work, &mut self.out),
                    None => self.out.extend_from_slice(&self.work),
                }
                continue;
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    /// The SNR of the test signal is measured over the whole working band (15.36 or 30.72 MHz), as the stack's tests state it.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.width.rate_hz()))
    }
}
