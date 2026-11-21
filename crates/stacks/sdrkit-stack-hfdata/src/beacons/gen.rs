//! The HF beacon test signal generator: clean keyed carriers at known RF frequencies, levels and times, with
//! slow fading per source. Noise comes from the standard `gen_*` keys.

use super::list::{self, NCDXF};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Morse;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_text::morse::Keyer;
use std::f64::consts::TAU;

/// The centre of the test signal when `gen_centre_hz` is 0, Hz: the single-letter beacons near 3.6 MHz.
pub const DEFAULT_CENTRE_HZ: f64 = 3.65e6;

/// How a source is keyed.
#[derive(Clone, Debug, PartialEq)]
pub enum Keying {
    /// A steady carrier.
    Carrier,
    /// A marker letter: dot length (s) and the gap after each letter (units).
    Letter {
        /// The letter.
        letter: char,
        /// The dot length, seconds.
        unit: f64,
        /// The gap after each letter, dots.
        gap_units: f64,
    },
    /// A Long Dashes place in the 27 s ladder.
    Dashes {
        /// The place (0 also sends at place 1).
        place: u8,
    },
    /// An NCDXF beacon: index, band, the gap between its four dashes (s), the first dash at `first_dash` s into the slot.
    Ncdxf {
        /// The beacon's place in the sequence (0 - 17).
        index: u8,
        /// The band (0 - 4).
        band: u8,
        /// The gap between the dashes, seconds.
        gap_s: f64,
        /// The start of the first dash in the slot, seconds.
        first_dash: f64,
    },
}

/// One source. Its slow fading is part of the scene, a level schedule of this source only: the standard fading of the `gen_*`
/// keys acts on the whole signal.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// The frequency, kHz.
    pub khz: f64,
    /// The keying.
    pub keying: Keying,
    /// The level of the key-down carrier (for NCDXF the 100 W dash), dB: 0 is unit power.
    pub level_db: f64,
    /// A raised-cosine dip: depth (dB) and period (s).
    pub fade: Option<(f64, f64)>,
    /// Active from .. to (s).
    pub window: Option<(f64, f64)>,
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct BeaconGenOptions {
    /// Samples per second.
    pub rate: f64,
    /// The radio centre, Hz.
    pub centre_hz: f64,
    /// The sources.
    pub sources: Vec<Source>,
    /// UTC (seconds) of sample 0, for the NCDXF schedule.
    pub utc_start: f64,
}

impl Default for BeaconGenOptions {
    fn default() -> Self {
        BeaconGenOptions {
            rate: 192_000.0,
            centre_hz: 7.0e6,
            sources: Vec::new(),
            utc_start: 0.0,
        }
    }
}

/// The built-in source: 'D' on 3594.7 kHz and 'K' on 3595.3 kHz (unit 0.1 s, gap 7), a carrier on 3595.45 kHz, all at the same
/// level.
pub fn builtin_sources() -> Vec<Source> {
    let l = |khz: f64, c: char| Source {
        khz,
        keying: Keying::Letter {
            letter: c,
            unit: 0.1,
            gap_units: 7.0,
        },
        level_db: 0.0,
        fade: None,
        window: None,
    };
    vec![
        l(3594.7, 'D'),
        l(3595.3, 'K'),
        Source {
            khz: 3595.45,
            keying: Keying::Carrier,
            level_db: 0.0,
            fade: None,
            window: None,
        },
    ]
}

/// The carrier of one source: a keyed Morse letter, or an oscillator keyed by the source's schedule.
enum Carrier {
    Morse(Box<Morse>),
    Nco(Nco),
}

struct Src {
    s: Source,
    amp: f64,
    carrier: Carrier,
    key: Vec<(bool, f64)>,
}

/// The generator: a source at level 0 dB has unit power while keyed.
pub struct BeaconGenerator {
    o: BeaconGenOptions,
    src: Vec<Src>,
    i: u64,
    scratch: Vec<Complex32>,
}

impl BeaconGenerator {
    /// The generator of `o`.
    pub fn new(o: BeaconGenOptions) -> Self {
        let src = o
            .sources
            .iter()
            .map(|s| {
                let f = s.khz * 1e3 - o.centre_hz;
                let nco = || Carrier::Nco(Nco::new(o.rate, f));
                let (carrier, key) = match &s.keying {
                    Keying::Letter {
                        letter,
                        unit,
                        gap_units,
                    } => {
                        let m = Morse::new(&letter.to_string(), 1.2 / unit, o.rate, f, 0.0)
                            .and_then(|m| m.repeating(gap_units * unit));
                        match m {
                            Ok(m) => (Carrier::Morse(Box::new(m)), Vec::new()),
                            Err(_) => (nco(), vec![(false, 1.0)]),
                        }
                    }
                    Keying::Ncdxf { index, .. } => {
                        let call = NCDXF[*index as usize % 18].0;
                        let k = Keyer::new(22.0)
                            .map(|k| k.key(call).map(|k| (k.on, k.seconds)).collect())
                            .unwrap_or_default();
                        (nco(), k)
                    }
                    _ => (nco(), Vec::new()),
                };
                Src {
                    amp: 10f64.powf(s.level_db / 20.0),
                    carrier,
                    key,
                    s: s.clone(),
                }
            })
            .collect();
        BeaconGenerator {
            src,
            i: 0,
            scratch: Vec::new(),
            o,
        }
    }
    /// From the stack's generator settings: `gen_signal` and `gen_centre_hz`. At 0 the centre is the frequency where the
    /// session listens ([`GenConfig::tuned_hz`]), else [`DEFAULT_CENTRE_HZ`].
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let c = match s.get(&super::GEN_CENTRE) {
            c if c > 0.0 => c,
            _ => cfg.tuned_hz().filter(|f| *f > 0.0).unwrap_or(DEFAULT_CENTRE_HZ),
        };
        BeaconGenerator::new(BeaconGenOptions {
            rate: cfg.rate_hz,
            centre_hz: c,
            sources: match s.get(&super::GEN_SIGNAL) {
                BeaconSignal::Nothing => Vec::new(),
                BeaconSignal::Beacons => builtin_sources(),
            },
            ..Default::default()
        })
    }
}

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    pub enum BeaconSignal {
        /// The built-in beacons.
        Beacons = ("beacons", "Beacons"),
        /// Noise only.
        Nothing = ("nothing", "Nothing"),
    }
}

fn keyed(key: &[(bool, f64)], mut t: f64) -> bool {
    for &(on, d) in key {
        if t < d {
            return on;
        }
        t -= d;
    }
    false
}

/// The keying level (0 to 1) of a scheduled source at signal time `t`.
fn level(x: &Src, utc_start: f64, t: f64) -> f64 {
    match &x.s.keying {
        Keying::Carrier | Keying::Letter { .. } => {
            if x.key.is_empty() {
                1.0
            } else {
                0.0
            }
        }
        Keying::Dashes { place } => {
            let c = t % 27.0;
            let on = |p: f64| c >= 3.0 * p && c < 3.0 * p + 2.9;
            if on(*place as f64) || (*place == 0 && on(1.0)) {
                1.0
            } else {
                0.0
            }
        }
        Keying::Ncdxf {
            index,
            band,
            gap_s,
            first_dash,
        } => {
            let u = utc_start + t;
            if list::ncdxf_on(*band, u) != *index {
                return 0.0;
            }
            let st = u - (u / 10.0).floor() * 10.0;
            if st >= 0.1 && keyed(&x.key, st - 0.1) {
                return 1.0;
            }
            let mut l = 0.0;
            for k in 0..4 {
                let a = first_dash + k as f64 * (1.0 + gap_s);
                if st >= a && st < a + 1.0 {
                    l = 10f64.powf(-10.0 * k as f64 / 20.0);
                }
            }
            l
        }
    }
}

impl Generator for BeaconGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of a beacon is stated in 100 Hz, for the key-down carrier at level 0 dB.
        Some(SignalInfo::new(100.0))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.o.rate;
        out.fill(Complex32::new(0.0, 0.0));
        self.scratch.resize(out.len(), Complex32::new(0.0, 0.0));
        for x in self.src.iter_mut() {
            match &mut x.carrier {
                Carrier::Morse(m) => {
                    let n = m.generate(&mut self.scratch);
                    self.scratch[n..].fill(Complex32::new(0.0, 0.0));
                }
                Carrier::Nco(n) => n.fill(&mut self.scratch),
            }
            for (k, (o, &c)) in out.iter_mut().zip(self.scratch.iter()).enumerate() {
                let t = (self.i + k as u64) as f64 / rate;
                if let Some((a, b)) = x.s.window {
                    if t < a || t >= b {
                        continue;
                    }
                }
                let l = match x.carrier {
                    Carrier::Morse(_) => 1.0,
                    Carrier::Nco(_) => level(x, self.o.utc_start, t),
                };
                if l == 0.0 {
                    continue;
                }
                let fade = x.s.fade.map_or(1.0, |(depth, period)| {
                    10f64.powf(-depth / 20.0 * (0.5 - 0.5 * (TAU * t / period).cos()))
                });
                *o += c * (x.amp * l * fade) as f32;
            }
        }
        self.i += out.len() as u64;
        out.len()
    }
}
