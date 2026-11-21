//! The ocean radar test signal generator: the clean linear FM sawtooth sweeps of several radars, an optional
//! Morse ID keyed with the whole sweep width, and junk the detector must not count. Noise comes from the standard `gen_*` keys.

use super::detect::fft_size;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Sweep;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_text::morse::Keyer;

/// One radar of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct TestRadar {
    pub offset_hz: f64,
    pub width_hz: f64,
    pub repeat_s: f64,
    /// +1 rising, -1 falling.
    pub dir: f64,
    /// dB relative to the strongest.
    pub level_db: f64,
    pub start_s: f64,
    /// A callsign keyed in the second half of each ID cycle, its speed (wpm) and the cycle (s).
    pub id: Option<(String, f64, f64)>,
}

/// The built-in radars: A 50 kHz up at -300 kHz every 0.5 s; B 100 kHz down at +250 kHz every 1.0 s, -4 dB; C 25 kHz up at +550 kHz
/// every 0.5 s, -2 dB.
pub fn builtin() -> Vec<TestRadar> {
    vec![
        TestRadar {
            offset_hz: -300e3,
            width_hz: 50e3,
            repeat_s: 0.5,
            dir: 1.0,
            level_db: 0.0,
            start_s: 0.1,
            id: None,
        },
        TestRadar {
            offset_hz: 250e3,
            width_hz: 100e3,
            repeat_s: 1.0,
            dir: -1.0,
            level_db: -4.0,
            start_s: 0.35,
            id: None,
        },
        TestRadar {
            offset_hz: 550e3,
            width_hz: 25e3,
            repeat_s: 0.5,
            dir: 1.0,
            level_db: -2.0,
            start_s: 0.2,
            id: None,
        },
    ]
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct RadarGenOptions {
    /// Samples per second.
    pub rate: f64,
    /// The radars.
    pub radars: Vec<TestRadar>,
    /// A carrier at +500 kHz and an ionosonde-like 200 kHz/s chirp across 0.9 x rate.
    pub junk: bool,
}

impl Default for RadarGenOptions {
    fn default() -> Self {
        RadarGenOptions {
            rate: 2e6,
            radars: builtin(),
            junk: false,
        }
    }
}

struct State {
    /// The sawtooth of sweeps, started at the radar's start time.
    sweep: Sweep,
    /// The whole width every 5 ms, sent while the ID key is down.
    id_sweep: Sweep,
    key: Vec<(bool, f64)>,
}

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    pub enum RadarSignal {
        /// The built-in radars.
        Radars = ("radars", "Radars"),
        /// Noise only.
        Nothing = ("nothing", "Nothing"),
        /// The radars, the first keying the ID AB1CD at 15 words a minute.
        RadarsWithId = ("radars_with_id", "Radars with an ID"),
        /// Wideband chirps that are not radars.
        Junk = ("junk", "Junk"),
    }
}

/// The generator: the strongest radar (and each junk signal) at unit power while it sweeps.
pub struct RadarGenerator {
    o: RadarGenOptions,
    i: u64,
    st: Vec<State>,
    carrier: Nco,
    chirp: Sweep,
}

impl RadarGenerator {
    /// The generator of `o`.
    pub fn new(o: RadarGenOptions) -> Self {
        let st = o
            .radars
            .iter()
            .map(|r| State {
                sweep: radar_sweep(o.rate, r, 0.9 * r.repeat_s, r.repeat_s),
                id_sweep: radar_sweep(o.rate, r, 0.005, 0.005),
                key: r.id.as_ref().map_or(Vec::new(), |(call, wpm, _)| {
                    let Ok(keyer) = Keyer::new(*wpm) else {
                        return Vec::new();
                    };
                    let mut k: Vec<(bool, f64)> =
                        keyer.key(call).map(|k| (k.on, k.seconds)).collect();
                    k.push((false, 7.0 * keyer.dot()));
                    k
                }),
            })
            .collect();
        RadarGenerator {
            st,
            carrier: Nco::new(o.rate, 500e3),
            chirp: {
                let span = 0.9 * o.rate;
                Sweep::new(o.rate, -span / 2.0, span / 2.0, span / 200e3)
                    .and_then(|s| s.repeating(span / 200e3))
                    .unwrap_or_else(|_| unreachable!("the junk chirp fits the rate"))
            },
            i: 0,
            o,
        }
    }
    /// From the stack's generator settings: `gen_signal`.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let signal = cfg.settings.get(&super::GEN_SIGNAL);
        let mut radars = builtin();
        if signal == RadarSignal::RadarsWithId {
            radars[0].id = Some(("AB1CD".into(), 15.0, 14.0));
        }
        RadarGenerator::new(RadarGenOptions {
            rate: cfg.rate_hz,
            radars: match signal {
                RadarSignal::Nothing | RadarSignal::Junk => Vec::new(),
                RadarSignal::Radars | RadarSignal::RadarsWithId => radars,
            },
            junk: signal == RadarSignal::Junk,
        })
    }

    /// What radar `k` sends at time `t`: its sweep, its ID sweep while the key is down, or nothing.
    fn regime(&self, k: usize, t: f64) -> Regime {
        let r = &self.o.radars[k];
        if t < r.start_s {
            return Regime::Off;
        }
        let local = t - r.start_s;
        if let Some((_, _, cycle)) = &r.id {
            let ct = local % cycle;
            if ct >= cycle / 2.0 {
                // The ID half: 1.2 s of silence, then the key.
                let mut kt = ct - cycle / 2.0 - 1.2;
                if kt < 0.0 {
                    return Regime::Quiet;
                }
                for &(on, d) in &self.st[k].key {
                    if kt < d {
                        return if on { Regime::Id } else { Regime::Quiet };
                    }
                    kt -= d;
                }
                return Regime::Quiet;
            }
        }
        Regime::Sweep
    }
}

/// What a radar sends now.
#[derive(Clone, Copy, PartialEq)]
enum Regime {
    /// Not started.
    Off,
    /// The ID half, key up.
    Quiet,
    /// The ID half, key down.
    Id,
    /// Sweeping.
    Sweep,
}

/// The sawtooth of radar `r`: across its width in its direction, `sweep_s` long, every `period_s`.
fn radar_sweep(rate: f64, r: &TestRadar, sweep_s: f64, period_s: f64) -> Sweep {
    let lo = r.offset_hz - r.dir * 0.5 * r.width_hz;
    let hi = r.offset_hz + r.dir * 0.5 * r.width_hz;
    Sweep::new(rate, lo, hi, sweep_s)
        .and_then(|s| s.repeating(period_s))
        .unwrap_or_else(|_| {
            Sweep::new(rate, 0.0, 0.0, 1.0)
                .unwrap_or_else(|_| unreachable!("a still sweep is valid"))
        })
}

impl Generator for RadarGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is that of the strongest radar in one bin of the receiver's 1 ms Hann FFT: the noise
        // bandwidth of that bin is 1.5 bins.
        Some(SignalInfo::new(
            1.5 * self.o.rate / fft_size(self.o.rate) as f64,
        ))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.o.rate;
        for o in out.iter_mut() {
            let t = self.i as f64 / rate;
            self.i += 1;
            let mut s = Complex32::new(0.0, 0.0);
            for k in 0..self.o.radars.len() {
                let regime = self.regime(k, t);
                if regime == Regime::Off {
                    continue;
                }
                let a = 10f64.powf(self.o.radars[k].level_db / 20.0) as f32;
                let st = &mut self.st[k];
                // The sawtooth runs on through the ID half, so the sweeps after it keep their timing.
                let mut v = [Complex32::default()];
                st.sweep.generate(&mut v);
                match regime {
                    Regime::Sweep => s += v[0] * a,
                    Regime::Id => {
                        st.id_sweep.generate(&mut v);
                        s += v[0] * a;
                    }
                    _ => {}
                }
            }
            if self.o.junk {
                s += self.carrier.next_sample();
                let mut v = [Complex32::default()];
                self.chirp.generate(&mut v);
                s += v[0];
            }
            *o = s;
        }
        out.len()
    }
}
