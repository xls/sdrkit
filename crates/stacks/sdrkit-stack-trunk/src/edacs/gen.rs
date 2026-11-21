//! The EDACS test signal: a whole site in one capture, the control channel and the analog voice channels. Noise and the
//! receiver's impairments come from the framework's `gen_*` settings.

use super::code::frame_bits;
use super::msg::build;
use crate::fmtx::fm;
use num_complex::Complex32;
use sdrkit_algo_dsp::iir::OnePole;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{GenConfig, Generator, SignalInfo};
use std::f64::consts::{PI, TAU};

/// A call of the site.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteCall {
    /// LCN.
    pub lcn: u32,
    /// Group.
    pub group: u32,
    /// Unit.
    pub unit: u32,
    /// Emergency.
    pub emergency: bool,
    /// Start, s.
    pub start: f64,
    /// End, s.
    pub end: f64,
    /// The voice tone, Hz.
    pub tone_hz: f64,
    /// Call end messages after it.
    pub call_end: bool,
    /// A carrier only (never announced).
    pub carrier_only: bool,
}

/// The site.
#[derive(Clone, Debug)]
pub struct EdacsSite {
    /// Sample rate.
    pub rate: f64,
    /// The radio's centre, Hz.
    pub centre_hz: f64,
    /// The control channel, Hz.
    pub cc_hz: f64,
    /// Its LCN.
    pub cc_lcn: u32,
    /// Site number.
    pub site: u32,
    /// System (area, Extended Addressing).
    pub area: u32,
    /// Length, s.
    pub seconds: f64,
    /// Control channel magnitude.
    pub cc_amp: f32,
    /// Control channel deviation, Hz.
    pub dev: f64,
    /// Extended Addressing.
    pub extended: bool,
    /// The ESK byte.
    pub esk: bool,
    /// Inverted data polarity.
    pub invert: bool,
    /// The sender's bit clock (1 = 9600 bit/s).
    pub clock: f64,
    /// The calls.
    pub calls: Vec<SiteCall>,
    /// The voice channel table (LCN 1, 2 …), Hz.
    pub table: Vec<f64>,
    /// The seed of the filler bits and words.
    pub seed: u64,
    /// Explicit frames (A, B) instead of the site's traffic.
    pub frames: Option<Vec<(u32, u32)>>,
}

impl Default for EdacsSite {
    fn default() -> Self {
        EdacsSite {
            rate: 480e3,
            centre_hz: 856.1e6,
            cc_hz: 856.0e6,
            cc_lcn: 1,
            site: 5,
            area: 1234,
            seconds: 8.0,
            cc_amp: 0.5,
            dev: 1400.0,
            extended: false,
            esk: false,
            invert: false,
            clock: 1.0,
            calls: Vec::new(),
            table: (0..10).map(|i| 856.0e6 + 25e3 * i as f64).collect(),
            seed: 1,
            frames: None,
        }
    }
}

/// The calls of the test case D14.
pub fn d14_calls() -> Vec<SiteCall> {
    vec![
        SiteCall {
            lcn: 3,
            group: 777,
            unit: 4321,
            emergency: false,
            start: 1.0,
            end: 3.0,
            tone_hz: 1000.0,
            call_end: true,
            carrier_only: false,
        },
        SiteCall {
            lcn: 6,
            group: 888,
            unit: 5555,
            emergency: true,
            start: 3.6,
            end: 5.2,
            tone_hz: 1600.0,
            call_end: false,
            carrier_only: false,
        },
        SiteCall {
            lcn: 8,
            group: 0,
            unit: 0,
            emergency: false,
            start: 6.0,
            end: 7.0,
            tone_hz: 700.0,
            call_end: false,
            carrier_only: true,
        },
    ]
}

impl EdacsSite {
    fn assignment(&self, c: &SiteCall) -> (u32, u32) {
        if self.extended {
            build::ea_group(c.lcn, c.group, c.unit, c.emergency, false, false, false)
        } else {
            build::group(c.lcn, c.group, c.unit, c.emergency, false, false)
        }
    }
    /// The frames (A, B) and their start times after the lead-in.
    pub fn frame_words(&self, rng: &mut Rng) -> Vec<(u32, u32)> {
        if let Some(f) = &self.frames {
            return f.clone();
        }
        let lead = 400.0 / (9600.0 * self.clock);
        let frame_s = 288.0 / (9600.0 * self.clock);
        let n = ((self.seconds - lead - 80.0 / 9600.0) / frame_s)
            .floor()
            .max(0.0) as usize;
        let mut out = Vec::with_capacity(n);
        let mut rr = 0usize;
        for k in 0..n {
            let t = lead + k as f64 * frame_s;
            let rand = (rng.next_u64() & 0xFFF_FFFF) as u32;
            let mut w = if k % 10 == 0 {
                let (a, _) = if self.extended {
                    build::ea_site(self.site, self.area)
                } else {
                    build::site_id(self.site, self.cc_lcn, 1, false, false, false)
                };
                (a, rand)
            } else {
                let active: Vec<&SiteCall> = self
                    .calls
                    .iter()
                    .filter(|c| !c.carrier_only && t >= c.start - 0.05 && t <= c.end)
                    .collect();
                let ending: Vec<&SiteCall> = self
                    .calls
                    .iter()
                    .filter(|c| {
                        c.call_end && !c.carrier_only && t > c.end && t <= c.end + 3.0 * frame_s
                    })
                    .collect();
                if !active.is_empty() && k % 3 == 0 {
                    rr += 1;
                    self.assignment(active[rr % active.len()])
                } else if let Some(c) = ending.first() {
                    if self.extended {
                        // Extended Addressing has no call end command in this layout: send the filler.
                        build::filler_ea(rand)
                    } else {
                        build::call_end(c.lcn, 0)
                    }
                } else if self.extended {
                    build::filler_ea(rand)
                } else {
                    build::filler_std(rand)
                }
            };
            if self.esk {
                w = build::esk(w);
            }
            out.push(w);
        }
        out
    }
    /// The capture.
    pub fn iq(&self) -> Vec<Complex32> {
        let mut rng = Rng::new(self.seed);
        let words = self.frame_words(&mut rng);
        let mut bits: Vec<bool> = (0..400).map(|_| rng.bit() == 1).collect();
        for &(a, b) in &words {
            bits.extend(frame_bits(a, b));
        }
        bits.extend((0..80).map(|_| rng.bit() == 1));
        let n = (self.seconds * self.rate) as usize;
        let mut out = vec![Complex32::new(0.0, 0.0); n];
        // The control channel: NRZ smoothed by a one-pole low-pass at 0.75 of the bit rate.
        let spb = self.rate / (9600.0 * self.clock);
        let mut lp = OnePole::new(self.rate, 0.75 * 9600.0);
        let s = if self.invert { -1.0 } else { 1.0 };
        let audio: Vec<f32> = (0..n)
            .map_while(|i| {
                let bi = (i as f64 / spb) as usize;
                let b = *bits.get(bi)?;
                Some(lp.process_sample(if b { s } else { -s }))
            })
            .collect();
        if let Ok(cc) = fm(&audio, self.rate, self.dev, self.cc_hz - self.centre_hz) {
            for (o, z) in out.iter_mut().zip(cc) {
                *o += z * self.cc_amp;
            }
        }
        // The voice channels: a tone of 3 kHz deviation with 5 ms raised-cosine ramps.
        for c in &self.calls {
            let Some(&f) = self.table.get(c.lcn as usize - 1) else {
                continue;
            };
            let a0 = ((c.start * self.rate) as usize).min(n);
            let a1 = ((c.end * self.rate) as usize).min(n);
            let tone: Vec<f32> = (0..a1 - a0)
                .map(|k| (TAU * c.tone_hz * k as f64 / self.rate).sin() as f32)
                .collect();
            let Ok(voice) = fm(&tone, self.rate, 3000.0, f - self.centre_hz) else {
                continue;
            };
            let ramp = 0.005 * self.rate;
            for (k, (o, z)) in out[a0..a1].iter_mut().zip(voice).enumerate() {
                let r = (k as f64 / ramp).min((a1 - a0 - k) as f64 / ramp).min(1.0);
                let g = 0.5 - 0.5 * (PI * r).cos();
                *o += z * (0.4 * g) as f32;
            }
        }
        out
    }
    /// The table as the setting's text (MHz per line).
    pub fn table_text(&self) -> String {
        self.table
            .iter()
            .map(|f| format!("{:.4}\n", f / 1e6))
            .collect()
    }
}

/// The demonstration generator: the D14 site at the given rate, looping, the control channel at 0 Hz.
pub struct EdacsGenerator {
    rate: f64,
    cycle: Vec<Complex32>,
    pos: usize,
}

impl EdacsGenerator {
    /// From the stack's generator config.
    pub fn from_config(cfg: &GenConfig) -> EdacsGenerator {
        // The control channel at 0 Hz, as for every generator of the crate.
        let s = EdacsSite {
            rate: cfg.rate_hz,
            seed: cfg.seed,
            calls: d14_calls(),
            centre_hz: 856.0e6,
            ..Default::default()
        };
        EdacsGenerator {
            rate: cfg.rate_hz,
            cycle: s.iq(),
            pos: 0,
        }
    }
}

impl Generator for EdacsGenerator {
    /// The station is made at 0 Hz. The framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.cycle.is_empty() {
            return 0;
        }
        for z in out.iter_mut() {
            *z = self.cycle[self.pos];
            self.pos = (self.pos + 1) % self.cycle.len();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            symbol_rate_hz: Some(9600.0),
            bit_rate_hz: Some(9600.0),
            power: 0.25,
            ..SignalInfo::new(25e3)
        })
    }
}
