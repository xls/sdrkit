//! The Iridium test signal: DE-QPSK bursts at 25 000 symbol/s with RRC 0.4 in 90 ms TDMA frames, downlink and uplink on the duplex
//! channels and downlink on the simplex channels, each with a Doppler shift and a known payload. Each burst is at unit power. Noise
//! (for example `gen_noise` = `cn0`) comes from the standard `gen_*` keys.

use super::phy::{bits_step, channel_hz, uw_steps, SYMBOL_RATE};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::f64::consts::FRAC_PI_2;

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// Each burst gets a Doppler shift.
pub const GEN_BURST_DOPPLER: Setting<field::Bool> =
    Setting::new("gen_burst_doppler", "Doppler", "", true)
        .help("Each burst gets a Doppler shift of up to +-35 kHz.")
        .section(TEST_SIGNAL)
        .advanced();
/// Bursts per TDMA frame.
pub const GEN_BURSTS: Setting<field::I64> = Setting::new("gen_bursts", "Bursts per frame", "", 4)
    .range(1.0, 20.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// Where the radio is tuned.
pub const GEN_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("gen_centre_hz", "Radio centre", "Hz", 1622e6)
        .range(1610e6, 1630e6, 1e3)
        .help("Where the radio is tuned, so the generator puts the channels in the right place.")
        .section(TEST_SIGNAL)
        .advanced();

/// A burst sent: what the receiver must give back.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    pub start_s: f64,
    pub channel: usize,
    pub doppler_hz: f64,
    pub downlink: bool,
    pub bits: Vec<bool>,
}

/// What the test signal holds.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    pub centre_hz: f64,
    pub doppler: bool,
    pub bursts_per_frame: usize,
    pub seed: u64,
}

impl GenParams {
    /// Four bursts a frame with Doppler, the band centre at 1622 MHz.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            centre_hz: 1622e6,
            doppler: true,
            bursts_per_frame: 4,
            seed: 1,
        }
    }
}

/// The Iridium test signal.
pub struct IrGenerator {
    p: GenParams,
    rng: Rng,
    pending: Vec<(u64, Vec<Complex32>)>,
    n: u64,
    next_frame: u64,
    /// The bursts sent so far.
    pub sent: Vec<Sent>,
}

impl IrGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.doppler = s.get(&GEN_BURST_DOPPLER);
        p.bursts_per_frame = s.get(&GEN_BURSTS) as usize;
        p.centre_hz = s.get(&GEN_CENTRE_HZ);
        p.seed = cfg.seed;
        Self::new(p)
    }

    /// The generator of these parameters.
    pub fn new(p: GenParams) -> Self {
        IrGenerator {
            rng: Rng::new(p.seed ^ 0x1D1),
            pending: Vec::new(),
            n: 0,
            next_frame: 0,
            sent: Vec::new(),
            p,
        }
    }

    /// The samples of one burst (preamble, unique word, data) that starts at sample `start`.
    fn burst(&mut self, start: u64, downlink: bool, simplex: bool, f: f64) -> Vec<Complex32> {
        let pre = if simplex { 64 } else { 16 };
        let ndata = if simplex { 432 } else { 179 };
        let mut steps: Vec<u8> = Vec::new();
        // Preamble: the downlink repeats symbol 0. The uplink alternates 2 and 0 (steps of two quarter turns).
        for k in 0..pre {
            steps.push(if downlink || k == 0 { 0 } else { 2 });
        }
        steps.extend(uw_steps(downlink));
        let bits: Vec<bool> = (0..2 * ndata).map(|_| self.rng.bit() == 1).collect();
        for p in bits.chunks(2) {
            steps.push(bits_step(p[0], p[1]));
        }
        self.sent.push(Sent {
            start_s: start as f64 / self.p.rate,
            channel: super::phy::channel_of(self.p.centre_hz + f),
            doppler_hz: f
                - (super::phy::channel_hz(super::phy::channel_of(self.p.centre_hz + f))
                    - self.p.centre_hz),
            downlink,
            bits: bits.clone(),
        });
        let mut ph = 0.0f64;
        let syms: Vec<Complex32> = steps
            .iter()
            .map(|&k| {
                ph += k as f64 * FRAC_PI_2;
                Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect();
        let sps = self.p.rate / SYMBOL_RATE;
        let mut m = Linear::rrc(0.4, 8.0, sps).expect("a valid RRC pulse");
        let mut out = Vec::with_capacity(((syms.len() as f64 + 8.0) * sps) as usize + 2);
        m.push(&syms, &mut out);
        m.flush(&mut out);
        let mut nco = Nco::new(self.p.rate, f);
        nco.mix_in_place(&mut out);
        out
    }

    fn schedule(&mut self) {
        let frame_len = (0.09 * self.p.rate) as u64;
        let start = self.next_frame * frame_len;
        self.next_frame += 1;
        let half = self.p.rate / 2.0 * 0.85;
        for k in 0..self.p.bursts_per_frame {
            let simplex = k == 0;
            let ch = if simplex {
                240 + self.rng.below(12) as usize
            } else {
                self.rng.below(240) as usize
            };
            let dop = if self.p.doppler {
                (self.rng.uniform() * 2.0 - 1.0) * 35e3
            } else {
                0.0
            };
            let f = channel_hz(ch) - self.p.centre_hz + dop;
            if f.abs() > half {
                continue;
            }
            let downlink = simplex || k % 2 == 1;
            let at = start + (k as u64) * (frame_len / (self.p.bursts_per_frame as u64 + 1)) + 1000;
            let b = self.burst(at, downlink, simplex, f);
            self.pending.push((at, b));
        }
    }
}

impl Generator for IrGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let end = self.n + out.len() as u64;
        let frame_len = (0.09 * self.p.rate) as u64;
        while self.next_frame * frame_len < end {
            self.schedule();
        }
        out.fill(Complex32::new(0.0, 0.0));
        for (at, b) in &self.pending {
            for (i, s) in b.iter().enumerate() {
                let idx = at + i as u64;
                if idx >= self.n && idx < end {
                    out[(idx - self.n) as usize] += *s;
                }
            }
        }
        self.pending.retain(|(at, b)| at + b.len() as u64 > end);
        self.n = end;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Each burst at unit power while it sends: two bits a symbol.
        let mut i = SignalInfo::new(SYMBOL_RATE * 1.4);
        i.symbol_rate_hz = Some(SYMBOL_RATE);
        i.bit_rate_hz = Some(2.0 * SYMBOL_RATE);
        Some(i)
    }
}
