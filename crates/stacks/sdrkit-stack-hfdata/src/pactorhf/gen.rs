//! The PACTOR-II/III/IV physical-layer test signal. The symbols are not PACTOR frames: every frame is a fixed 40-symbol pattern
//! followed by pseudo-random symbols, every 362 symbols, so the clock, the carriers, the steps and the frame finder have a known
//! answer.

use super::analyse::grid_of;
use sdrkit_algo_dsp::pulse::rrc;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use std::f64::consts::{PI, TAU};

/// The frame period in symbols.
pub const FRAME: usize = 362;

/// Which signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variant {
    /// Two D-PSK carriers at centre +-100 Hz; the order of the phase steps.
    P2 { order: u32 },
    /// The carriers of a PACTOR-III level (1 to 6).
    P3 { level: u8 },
    /// One carrier at 1500 Hz, 1800 baud: PSK of order 2, 4, 8, or 16 for 16-QAM.
    P4 { constellation: u32 },
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct PactorHfGenOptions {
    /// The sample rate, Hz.
    pub rate: f64,
    pub variant: Variant,
    /// The PACTOR-II pair centre, Hz above the dial.
    pub centre_hz: f64,
    /// Complex output (the upper sideband of a dial at 0 Hz), or its real part.
    pub complex: bool,
}

impl Default for PactorHfGenOptions {
    fn default() -> Self {
        PactorHfGenOptions {
            rate: 8000.0,
            variant: Variant::P2 { order: 4 },
            centre_hz: 885.0,
            complex: true,
        }
    }
}

/// The phase step (in units of 2 pi / order) of symbol `k` of carrier `c`.
pub fn step(k: usize, c: usize, order: u32) -> u32 {
    let pos = k % FRAME;
    let seed = if pos < 40 {
        0x5EED + c as u64 * 7919 + pos as u64
    } else {
        0xF00D ^ ((k as u64) << 8) ^ c as u64
    };
    let mut r = Rng::new(seed);
    (r.next_u64() % order as u64) as u32
}

/// The generator: the clean signal at unit mean power (complex output).
pub struct PactorHfGenerator {
    o: PactorHfGenOptions,
    freqs: Vec<f64>,
    order: u32,
    baud: f64,
    amplitude: f64,
    /// Per carrier: the absolute phase of each symbol (filled as needed).
    phases: Vec<Vec<f64>>,
    p4: Vec<(f64, f64)>,
    i: u64,
}

impl PactorHfGenerator {
    /// A generator for `o`.
    pub fn new(o: PactorHfGenOptions) -> Self {
        let (freqs, order, baud) = match o.variant {
            Variant::P2 { order } => (vec![o.centre_hz - 100.0, o.centre_hz + 100.0], order, 100.0),
            Variant::P3 { level } => (grid_of(level), if level <= 3 { 2 } else { 4 }, 100.0),
            Variant::P4 { constellation } => (vec![1500.0], constellation, 1800.0),
        };
        let n = freqs.len();
        PactorHfGenerator {
            freqs,
            order,
            baud,
            amplitude: 1.0 / (n.max(1) as f64).sqrt(),
            phases: vec![Vec::new(); n],
            p4: Vec::new(),
            i: 0,
            o,
        }
    }
    /// From the stack's generator settings: `gen_level` for PACTOR-III, `gen_constellation` for PACTOR-IV.
    pub fn from_config(cfg: &GenConfig, variant: u8) -> Self {
        let v = match variant {
            3 => Variant::P3 {
                level: cfg.settings.get(&super::GEN_LEVEL).clamp(1, 6) as u8,
            },
            4 => Variant::P4 {
                constellation: cfg.settings.get(&super::GEN_CONSTELLATION).points(),
            },
            _ => Variant::P2 { order: 4 },
        };
        PactorHfGenerator::new(PactorHfGenOptions {
            rate: cfg.rate_hz,
            variant: v,
            ..Default::default()
        })
    }
    /// The sent phase steps of carrier `c`, symbols 0 .. n.
    pub fn sent(&self, c: usize, n: usize) -> Vec<u32> {
        (0..n).map(|k| step(k, c, self.order)).collect()
    }
    /// The carrier frequencies.
    pub fn freqs(&self) -> &[f64] {
        &self.freqs
    }

    fn phase(&mut self, c: usize, k: usize) -> f64 {
        while self.phases[c].len() <= k {
            let j = self.phases[c].len();
            let prev = if j == 0 { 0.0 } else { self.phases[c][j - 1] };
            self.phases[c].push(prev + TAU * step(j, c, self.order) as f64 / self.order as f64);
        }
        self.phases[c][k]
    }

    fn p4_symbol(&mut self, k: usize) -> (f64, f64) {
        while self.p4.len() <= k {
            let j = self.p4.len();
            let s = step(j, 0, self.order.max(2));
            let v = if self.order == 16 {
                let lv = [-3.0, -1.0, 1.0, 3.0];
                (
                    lv[(s & 3) as usize] / 10f64.sqrt(),
                    lv[(s >> 2) as usize & 3] / 10f64.sqrt(),
                )
            } else {
                let off = if self.order == 2 {
                    0.0
                } else {
                    PI / self.order as f64
                };
                let p = TAU * s as f64 / self.order as f64 + off;
                (p.cos(), p.sin())
            };
            self.p4.push(v);
        }
        self.p4[k]
    }

    fn baseband(&mut self, t: f64) -> Complex32 {
        let ts = t * self.baud;
        let k = ts.floor().max(0.0) as usize;
        let frac = ts - k as f64;
        let mut s = Complex32::new(0.0, 0.0);
        match self.o.variant {
            Variant::P4 { .. } => {
                let (mut re, mut im) = (0.0, 0.0);
                for j in k.saturating_sub(4)..=k + 4 {
                    let (a, b) = self.p4_symbol(j);
                    let h = rrc(ts - j as f64 - 0.5, 0.35);
                    re += a * h;
                    im += b * h;
                }
                let f = self.freqs[0];
                let p = TAU * f * t;
                let (sn, cs) = p.sin_cos();
                let a = self.amplitude;
                s = Complex32::new(
                    (a * (re * cs - im * sn)) as f32,
                    (a * (re * sn + im * cs)) as f32,
                );
            }
            _ => {
                let p2 = matches!(self.o.variant, Variant::P2 { .. });
                for c in 0..self.freqs.len() {
                    let cur = self.phase(c, k);
                    let prev = if k == 0 { cur } else { self.phase(c, k - 1) };
                    // P-II: raised-cosine cross-fade over the whole symbol; P-III: flat top, +-0.18 symbol cross-fades.
                    let w = if p2 {
                        0.5 - 0.5 * (PI * frac).cos()
                    } else if frac < 0.18 {
                        0.5 + 0.5 * (PI * frac / 0.36).sin()
                    } else {
                        1.0
                    };
                    let w_next = if !p2 && frac > 0.82 {
                        0.5 + 0.5 * (PI * (frac - 1.0) / 0.36).sin()
                    } else {
                        0.0
                    };
                    let next = self.phase(c, k + 1);
                    let (re, im) = if p2 || frac < 0.18 {
                        (
                            (1.0 - w) * prev.cos() + w * cur.cos(),
                            (1.0 - w) * prev.sin() + w * cur.sin(),
                        )
                    } else if frac > 0.82 {
                        let ww = 1.0 - w_next;
                        (
                            ww * cur.cos() + (1.0 - ww) * next.cos(),
                            ww * cur.sin() + (1.0 - ww) * next.sin(),
                        )
                    } else {
                        (cur.cos(), cur.sin())
                    };
                    let f = self.freqs[c];
                    let p = TAU * f * t;
                    let (sn, cs) = p.sin_cos();
                    let a = self.amplitude;
                    s += Complex32::new(
                        (a * (re * cs - im * sn)) as f32,
                        (a * (re * sn + im * cs)) as f32,
                    );
                }
            }
        }
        s
    }
}

impl Generator for PactorHfGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the PACTOR stacks is stated in 2.5 kHz.
        Some(SignalInfo::new(2500.0).with_symbol_rate(self.baud))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.i as f64 / self.o.rate;
            self.i += 1;
            let s = self.baseband(t);
            *o = if self.o.complex {
                s
            } else {
                Complex32::new(s.re, 0.0)
            };
        }
        out.len()
    }
}
