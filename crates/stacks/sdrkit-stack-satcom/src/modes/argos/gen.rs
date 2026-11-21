//! The Argos test signal: PTT-A2 uplink bursts from three made-up platforms, or the A-DCS downlink through a satellite pass.

use super::a2;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo, Values};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::hdlc_encode;
use sdrkit_util_bits::line::Nrzi;
use std::collections::VecDeque;
use std::f64::consts::{FRAC_PI_2, TAU};

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The test signal sends the downlink instead of uplink bursts.
pub const GEN_DOWNLINK: Setting<field::Bool> = Setting::new("gen_downlink", "Downlink", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The silence between uplink bursts.
pub const GEN_GAP_S: Setting<field::Duration> =
    Setting::new("gen_gap_s", "Seconds between bursts", "s", 2.0)
        .range(0.5, 60.0, 0.1)
        .section(TEST_SIGNAL)
        .advanced();
/// The frequency step of a platform from one burst to its next.
pub const GEN_STEP_HZ: Setting<field::Hz> =
    Setting::new("gen_step_hz", "Burst-to-burst step", "Hz", 0.0)
        .range(-500.0, 500.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// A burst of another modulation each cycle.
pub const GEN_OTHER: Setting<field::Bool> =
    Setting::new("gen_other", "Another modulation each cycle", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The largest Doppler of the downlink pass.
pub const GEN_PASS_DOPPLER_HZ: Setting<field::Hz> = Setting::new(
    "gen_pass_doppler_hz",
    "Largest Doppler of the pass",
    "Hz",
    9000.0,
)
.range(0.0, 20e3, 10.0)
.section(TEST_SIGNAL)
.advanced();
/// The time constant of the pass's Doppler curve.
pub const GEN_PASS_TAU_S: Setting<field::Duration> =
    Setting::new("gen_pass_tau_s", "Doppler time constant", "s", 60.0)
        .range(5.0, 600.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The time of closest approach.
pub const GEN_PASS_TCA_S: Setting<field::Duration> =
    Setting::new("gen_pass_tca_s", "Closest approach", "s", 40.0)
        .range(0.0, 3600.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The downlink is BPSK.
pub const GEN_BPSK: Setting<field::Bool> = Setting::new("gen_bpsk", "BPSK", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The downlink bits are NRZI coded.
pub const GEN_NRZI: Setting<field::Bool> = Setting::new("gen_nrzi", "NRZI", "", true)
    .section(TEST_SIGNAL)
    .advanced();

/// Options of the generator.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The sample rate, Hz.
    pub rate: f64,
    /// Send the downlink instead of uplink bursts.
    pub downlink: bool,
    /// Silence between uplink bursts, seconds.
    pub gap_s: f64,
    /// The frequency step of a platform from one burst to its next, Hz.
    pub step_hz: f64,
    /// Add a burst of another modulation each cycle.
    pub other: bool,
    /// The largest Doppler of the downlink pass, Hz.
    pub doppler_hz: f64,
    /// The time constant of the pass's Doppler curve, seconds.
    pub tau_s: f64,
    /// The time of closest approach, seconds.
    pub tca_s: f64,
    /// Send the downlink as BPSK (plus or minus 90 degrees) instead of plus or minus 1.1 rad.
    pub bpsk: bool,
    /// Send the downlink bits NRZI coded.
    pub nrzi: bool,
    /// The seed of the downlink's frame contents.
    pub seed: u64,
}

impl GenParams {
    /// The defaults at `rate`.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            downlink: false,
            gap_s: 2.0,
            step_hz: 0.0,
            other: false,
            doppler_hz: 9000.0,
            tau_s: 60.0,
            tca_s: 40.0,
            bpsk: false,
            nrzi: true,
            seed: 1,
        }
    }

    /// The options of a configuration (a setting that is missing keeps its default).
    pub fn from_values(rate: f64, s: &Values, seed: u64) -> Self {
        let mut p = GenParams::new(rate);
        p.downlink = s.get(&GEN_DOWNLINK);
        p.gap_s = s.get(&GEN_GAP_S);
        p.step_hz = s.get(&GEN_STEP_HZ);
        p.other = s.get(&GEN_OTHER);
        p.doppler_hz = s.get(&GEN_PASS_DOPPLER_HZ);
        p.tau_s = s.get(&GEN_PASS_TAU_S);
        p.tca_s = s.get(&GEN_PASS_TCA_S);
        p.bpsk = s.get(&GEN_BPSK);
        p.nrzi = s.get(&GEN_NRZI);
        p.seed = seed;
        p
    }
}

/// The made-up platforms: id, offset from the dial (Hz), k (length 80 + 32 k).
pub const PLATFORMS: [(u32, f64, usize); 3] = [
    (0x0ABCDEF, -15_000.0, 0),
    (0x0ABCDF2, 3_000.0, 2),
    (0x0ABCDF5, 18_000.0, 5),
];

/// A platform's data bits.
pub fn platform_data(i: usize) -> Vec<bool> {
    let (_, _, k) = PLATFORMS[i];
    let mut r = Rng::new(0xA2 + i as u64);
    (0..24 + 32 * k).map(|_| r.bit() == 1).collect()
}

/// The pass's Doppler at `t`.
pub fn pass_doppler(p: &GenParams, t: f64) -> f64 {
    let d = t - p.tca_s;
    -p.doppler_hz * d / (d * d + p.tau_s * p.tau_s).sqrt()
}

/// The generator. It makes unit power while it sends.
pub struct ArgosGenerator {
    p: GenParams,
    /// The burst being sent: samples left.
    burst: VecDeque<Complex32>,
    gap_left: usize,
    next: usize,
    bursts_sent: Vec<usize>,
    phase: f64,
    n: u64,
    // Downlink.
    dl_bits: VecDeque<u8>,
    dl_nrzi: Nrzi,
    dl_scratch: Vec<u8>,
    dl_rng: Rng,
    dl_half: u64,
    dl_level: f64,
    /// The payload of each downlink frame sent, in order.
    pub frames_sent: Vec<Vec<u8>>,
}

impl ArgosGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(GenParams::from_values(cfg.rate_hz, &cfg.settings, cfg.seed))
    }

    /// The generator with these options.
    pub fn new(p: GenParams) -> Self {
        ArgosGenerator {
            burst: VecDeque::new(),
            gap_left: (p.rate * 0.5) as usize,
            next: 0,
            bursts_sent: vec![0; 4],
            phase: 0.0,
            n: 0,
            dl_bits: VecDeque::new(),
            dl_nrzi: Nrzi::new(0),
            dl_scratch: Vec::new(),
            dl_rng: Rng::new(p.seed ^ 0xD1),
            dl_half: 0,
            dl_level: 1.0,
            frames_sent: Vec::new(),
            p,
        }
    }

    /// The carrier of platform `i` at its `n`-th burst, Hz from the dial.
    pub fn platform_hz(&self, i: usize, n: usize) -> f64 {
        PLATFORMS[i].1 + self.p.step_hz * n as f64
    }

    fn make_burst(&mut self) {
        let rate = self.p.rate;
        let i = self.next % (PLATFORMS.len() + self.p.other as usize);
        self.next += 1;
        let mut out = Vec::new();
        let mut ph = 0.0f64;
        if i < PLATFORMS.len() {
            let (id, _, k) = PLATFORMS[i];
            let f0 = self.platform_hz(i, self.bursts_sent[i]);
            self.bursts_sent[i] += 1;
            let bits = a2::build(id, k, &platform_data(i));
            let nc = (0.160 * rate) as usize;
            let spb = rate / a2::BIT_RATE;
            let total = nc + (bits.len() as f64 * spb) as usize;
            for s in 0..total {
                ph = (ph + TAU * f0 / rate) % TAU;
                let m = if s < nc {
                    0.0
                } else {
                    let x = (s - nc) as f64 / spb;
                    let b = bits[(x as usize).min(bits.len() - 1)];
                    let first = x.fract() < 0.5;
                    // A 1 goes from -1.1 to +1.1 rad (the step +2.2 rad).
                    let v = if first { -a2::DEV } else { a2::DEV };
                    if b {
                        v
                    } else {
                        -v
                    }
                };
                let a = ph + m;
                out.push(Complex32::new(a.cos() as f32, a.sin() as f32));
            }
        } else {
            // Another modulation: 160 ms of carrier, then 400 ms of QPSK at 1600 symbol/s.
            let f0 = 10_000.0;
            let nc = (0.160 * rate) as usize;
            let total = nc + (0.4 * rate) as usize;
            let mut r = Rng::new(self.next as u64);
            let sps = (rate / 1600.0) as usize;
            let mut q = 0.0;
            for s in 0..total {
                ph = (ph + TAU * f0 / rate) % TAU;
                if s >= nc && (s - nc).is_multiple_of(sps) {
                    q = FRAC_PI_2 * (r.below(4) as f64) + std::f64::consts::FRAC_PI_4;
                }
                let a = ph + if s >= nc { q } else { 0.0 };
                out.push(Complex32::new(a.cos() as f32, a.sin() as f32));
            }
        }
        self.burst.extend(out);
    }

    fn dl_refill(&mut self) {
        // Flags, then a frame of 20 to 40 bytes.
        let n = 20 + self.dl_rng.below(21) as usize;
        let mut payload = vec![0u8; n];
        self.dl_rng.fill_bytes(&mut payload);
        self.dl_scratch.clear();
        hdlc_encode(&[&payload], &CRC16_IBM_SDLC, 4, &mut self.dl_scratch);
        if self.p.nrzi {
            self.dl_nrzi.encode(&mut self.dl_scratch);
        }
        self.dl_bits.extend(self.dl_scratch.iter().copied());
        self.frames_sent.push(payload);
    }
}

impl Generator for ArgosGenerator {
    /// The dial frequency is at 0 Hz.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.p.rate;
        for o in out.iter_mut() {
            let mut v = Complex32::default();
            if self.p.downlink {
                let t = self.n as f64 / rate;
                let f = pass_doppler(&self.p, t);
                self.phase = (self.phase + TAU * f / rate) % TAU;
                // The half-bit clock: a new bit at every even half, the level flips at every odd half (Manchester).
                let h = (self.n as f64 * 2.0 * a2::BIT_RATE / rate) as u64;
                if h != self.dl_half || self.n == 0 {
                    self.dl_half = h;
                    if h.is_multiple_of(2) {
                        if self.dl_bits.is_empty() {
                            self.dl_refill();
                        }
                        let b = self.dl_bits.pop_front().unwrap_or(0);
                        self.dl_level = if b == 1 { 1.0 } else { -1.0 };
                    } else {
                        self.dl_level = -self.dl_level;
                    }
                }
                let dev = if self.p.bpsk { FRAC_PI_2 } else { a2::DEV };
                let a = self.phase + dev * self.dl_level;
                v += Complex32::new(a.cos() as f32, a.sin() as f32);
            } else if let Some(s) = self.burst.pop_front() {
                v += s;
            } else if self.gap_left > 0 {
                self.gap_left -= 1;
            } else {
                self.make_burst();
                self.gap_left = (self.p.gap_s * rate) as usize;
            }
            *o = v;
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR is the carrier over the noise in 2.4 kHz; 400 bit/s on both links.
        Some(
            SignalInfo::new(2400.0)
                .with_symbol_rate(a2::BIT_RATE)
                .with_bit_rate(a2::BIT_RATE),
        )
    }
}
