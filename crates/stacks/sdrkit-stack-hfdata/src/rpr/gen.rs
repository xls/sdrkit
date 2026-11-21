//! The Robust Packet test signal: bursts of eight differential PSK carriers built from the waveform of the specification. Each
//! burst has 3 symbols of ramp, 8 preamble symbols per carrier (a fixed pattern of this generator; the real preamble is not
//! public), the data symbols per carrier (60 by default, pseudo-random), and 3 symbols of ramp. The first symbol of each carrier
//! is its phase start. These bursts are not Robust Packet frames: the channel coding is not public.
//!
//! A symbol lasts 20 ms: a guard of 1/300 s, over which the phase of the previous symbol fades into the new one with a raised
//! cosine, then the active part of 1/60 s at a constant phase. The ramps carry the phase of the first or the last symbol with
//! a raised-cosine amplitude. The station (the dial) is where [`test_dial`] puts it.

use super::{BAUD, CARRIERS, CENTRE_HZ, GUARD_S, PREAMBLE, RAMP, SPACING_HZ};
use crate::serial::gen::test_dial;
use num_complex::Complex64;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use std::f64::consts::{PI, TAU};

/// The generator's options.
#[derive(Clone, Debug, PartialEq)]
pub struct RprGenOptions {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The speed level: 1 (D-BPSK) or 2 (D-QPSK).
    pub level: u8,
    /// The silence between two bursts, seconds.
    pub gap_s: f64,
    /// The silence before the first burst, seconds.
    pub lead_s: f64,
    /// The data symbols of each carrier after the preamble.
    pub data_symbols: usize,
    /// The seed of the data symbols.
    pub seed: u64,
}

impl Default for RprGenOptions {
    fn default() -> Self {
        RprGenOptions {
            rate: 8000.0,
            level: 1,
            gap_s: 1.5,
            lead_s: 0.25,
            data_symbols: 60,
            seed: 1,
        }
    }
}

/// The phase steps of the eight carriers of one burst, and the phases they give.
struct Burst {
    /// Per symbol, per carrier: the phasor `e^(j phase)`.
    phasors: Vec<[Complex64; CARRIERS]>,
}

/// The generator: unit mean power while a burst is on (complex output, the upper sideband of the dial).
pub struct RprGenerator {
    o: RprGenOptions,
    /// The symbols of a burst: preamble and data.
    n_sym: usize,
    /// The samples of a symbol.
    sps: f64,
    /// The samples from one burst start to the next.
    period: f64,
    /// The current burst number and its phasors.
    burst: Option<(u64, Burst)>,
    /// The oscillator of each carrier and its turn per sample.
    osc: [Complex64; CARRIERS],
    turn: [Complex64; CARRIERS],
    i: u64,
}

/// The order of the phase steps of `level`: 2 for level 1, 4 for level 2.
pub fn order_of(level: u8) -> u32 {
    if level >= 2 {
        4
    } else {
        2
    }
}

impl RprGenerator {
    /// A generator for `o`.
    pub fn new(o: RprGenOptions) -> Self {
        let sps = o.rate / BAUD;
        let n_sym = PREAMBLE + o.data_symbols.max(1);
        let burst_samples = (2 * RAMP + n_sym) as f64 * sps;
        let period = burst_samples + o.gap_s.max(0.0) * o.rate;
        let dial = test_dial(o.rate);
        let mut turn = [Complex64::new(1.0, 0.0); CARRIERS];
        for (c, t) in turn.iter_mut().enumerate() {
            let f = dial + carrier_hz(CENTRE_HZ, c);
            *t = Complex64::from_polar(1.0, TAU * f / o.rate);
        }
        RprGenerator {
            n_sym,
            sps,
            period,
            burst: None,
            osc: [Complex64::new(1.0, 0.0); CARRIERS],
            turn,
            i: 0,
            o,
        }
    }

    /// From the stack's generator settings: the speed level and the gap.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let level = match cfg.settings.get(&super::GEN_SPEED) {
            super::GenSpeed::Level1 => 1,
            super::GenSpeed::Level2 => 2,
        };
        RprGenerator::new(RprGenOptions {
            rate: cfg.rate_hz,
            level,
            gap_s: cfg.settings.get(&super::GEN_GAP_S),
            seed: cfg.seed,
            ..Default::default()
        })
    }

    /// The options.
    pub fn options(&self) -> &RprGenOptions {
        &self.o
    }

    /// The start of burst `b`, seconds from the first sample.
    pub fn burst_start_s(&self, b: u64) -> f64 {
        (self.o.lead_s * self.o.rate + b as f64 * self.period) / self.o.rate
    }

    /// The phase step indices of burst `b`: for each step (preamble and data, one fewer than the symbols), the index of each
    /// carrier, the step being `index x 2 pi / order`.
    pub fn steps(&self, b: u64) -> Vec<[u8; CARRIERS]> {
        let order = order_of(self.o.level);
        (1..self.n_sym)
            .map(|m| {
                let mut row = [0u8; CARRIERS];
                for (c, v) in row.iter_mut().enumerate() {
                    *v = step(self.o.seed, b, m, c, order);
                }
                row
            })
            .collect()
    }

    fn make_burst(&self, b: u64) -> Burst {
        let order = order_of(self.o.level);
        let mut phase = [0.0f64; CARRIERS];
        for (c, p) in phase.iter_mut().enumerate() {
            // A start phase of each carrier, fixed by the carrier number.
            *p = TAU * ((c * 37 + 11) % 64) as f64 / 64.0;
        }
        let mut phasors = Vec::with_capacity(self.n_sym);
        for m in 0..self.n_sym {
            let mut row = [Complex64::new(0.0, 0.0); CARRIERS];
            for c in 0..CARRIERS {
                if m > 0 {
                    phase[c] += TAU * step(self.o.seed, b, m, c, order) as f64 / order as f64;
                }
                row[c] = Complex64::from_polar(1.0, phase[c]);
            }
            phasors.push(row);
        }
        Burst { phasors }
    }

    /// The baseband phasor of carrier values at sample position `s` (samples from the burst start): ramps, guard fades, the
    /// active parts. `None` outside the burst.
    fn values(&mut self, b: u64, s: f64) -> Option<[Complex64; CARRIERS]> {
        let slots = (2 * RAMP + self.n_sym) as f64;
        let t = s / self.sps;
        if !(0.0..slots).contains(&t) {
            return None;
        }
        if self.burst.as_ref().is_none_or(|(n, _)| *n != b) {
            self.burst = Some((b, self.make_burst(b)));
        }
        let burst = &self.burst.as_ref()?.1;
        let slot = t.floor() as usize;
        let frac = t - slot as f64;
        let last = self.n_sym - 1;
        let mut out = [Complex64::new(0.0, 0.0); CARRIERS];
        if slot < RAMP {
            let a = 0.5 - 0.5 * (PI * t / RAMP as f64).cos();
            for (o, p) in out.iter_mut().zip(&burst.phasors[0]) {
                *o = p * a;
            }
        } else if slot >= RAMP + self.n_sym {
            let u = t - (RAMP + self.n_sym) as f64;
            let a = 0.5 + 0.5 * (PI * u / RAMP as f64).cos();
            for (o, p) in out.iter_mut().zip(&burst.phasors[last]) {
                *o = p * a;
            }
        } else {
            let m = slot - RAMP;
            let g = GUARD_S * BAUD;
            if m > 0 && frac < g {
                let w = 0.5 - 0.5 * (PI * frac / g).cos();
                for (c, o) in out.iter_mut().enumerate() {
                    *o = burst.phasors[m - 1][c] * (1.0 - w) + burst.phasors[m][c] * w;
                }
            } else {
                out = burst.phasors[m];
            }
        }
        Some(out)
    }
}

/// The audio frequency of carrier `c` (0 to 7) of a comb centred on `centre_hz`.
pub fn carrier_hz(centre_hz: f64, c: usize) -> f64 {
    centre_hz + (c as f64 - (CARRIERS as f64 - 1.0) / 2.0) * SPACING_HZ
}

/// The step index of symbol `m` (1 on) of carrier `c` in burst `b`: the preamble from a fixed seed, the data from `seed`.
fn step(seed: u64, b: u64, m: usize, c: usize, order: u32) -> u8 {
    let key = if m < PREAMBLE {
        0x5250_5250 ^ ((m as u64) << 8) ^ c as u64
    } else {
        seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b << 32) ^ ((m as u64) << 8) ^ c as u64
    };
    (Rng::new(key).next_u64() % u64::from(order)) as u8
}

impl Generator for RprGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the stack is stated in 2.5 kHz; eight carriers of 50 Bd.
        Some(SignalInfo::new(2500.0).with_symbol_rate(BAUD * CARRIERS as f64))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        test_dial(self.o.rate)
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let amp = 1.0 / (CARRIERS as f64).sqrt();
        let lead = self.o.lead_s * self.o.rate;
        for o in out.iter_mut() {
            let pos = self.i as f64 - lead;
            self.i += 1;
            let mut acc = Complex64::new(0.0, 0.0);
            if pos >= 0.0 {
                let b = (pos / self.period).floor() as u64;
                let s = pos - b as f64 * self.period;
                if let Some(v) = self.values(b, s) {
                    for (c, x) in v.iter().enumerate() {
                        acc += x * self.osc[c];
                    }
                }
            }
            for (c, osc) in self.osc.iter_mut().enumerate() {
                *osc *= self.turn[c];
            }
            if self.i.is_multiple_of(4096) {
                for osc in self.osc.iter_mut() {
                    *osc /= osc.norm();
                }
            }
            *o = Complex32::new((acc.re * amp) as f32, (acc.im * amp) as f32);
        }
        out.len()
    }
}
