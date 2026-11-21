//! The occupied bandwidth test signal: PSK with root-raised-cosine pulses of unit power, and optionally a neighbour of the
//! same shape at a lower level, offset by a number of occupied bandwidths. The power spectrum of the signal is the
//! raised-cosine spectrum of the symbol rate `Rs` and the roll-off `a` (flat to `(1 - a) Rs / 2`, zero from `(1 + a) Rs /
//! 2`). Noise is stated as Es/N0 by the standard impairments; the carrier offset comes from them too.

use sdrkit_algo_dsp::pulse::rrc;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::{PI, TAU};

const SECTION: &str = "Test signal";

/// The symbol rate.
pub const GEN_SYMBOL_RATE: Setting<field::F64> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Bd", 25e3)
        .range(100.0, 1e7, 0.0)
        .section(SECTION);
/// The roll-off of the pulses.
pub const GEN_ROLLOFF: Setting<field::F64> = Setting::new("gen_rolloff", "Roll-off", "", 0.35)
    .range(0.05, 1.0, 0.0)
    .section(SECTION);
/// The points of the PSK constellation.
pub const GEN_PSK: Setting<field::I64> = Setting::new("gen_psk", "PSK points", "", 4)
    .range(2.0, 16.0, 1.0)
    .section(SECTION);
/// A neighbour of the same shape.
pub const GEN_NEIGHBOUR: Setting<field::Bool> =
    Setting::new("gen_neighbour", "Neighbour", "", false).section(SECTION);
/// The level of the neighbour.
pub const GEN_NEIGHBOUR_DB: Setting<field::F64> =
    Setting::new("gen_neighbour_db", "Neighbour level", "dB", -40.0)
        .range(-100.0, 20.0, 0.0)
        .section(SECTION);
/// The offset of the neighbour, in occupied (99 %) bandwidths.
pub const GEN_NEIGHBOUR_OFFSET: Setting<field::F64> =
    Setting::new("gen_neighbour_offset", "Neighbour offset", "", 1.25)
        .range(-10.0, 10.0, 0.0)
        .section(SECTION);

/// The pulse spans this many symbols on each side.
const SPAN: f64 = 8.0;
/// Pulse table points per symbol.
const TABLE: usize = 1024;

/// The power density of the raised-cosine spectrum at `f` symbol rates from the centre (1 in the flat part).
pub fn rc_density(f: f64, rolloff: f64) -> f64 {
    let f = f.abs();
    let a = (1.0 - rolloff) / 2.0;
    let b = (1.0 + rolloff) / 2.0;
    if f <= a {
        1.0
    } else if f >= b {
        0.0
    } else {
        0.5 * (1.0 + (PI / rolloff * (f - a)).cos())
    }
}

/// The width, in symbol rates, of the band that holds the share `share` of the power of the raised-cosine spectrum, with
/// half the rest on each side.
pub fn rc_share_width(rolloff: f64, share: f64) -> f64 {
    let steps = 200_000;
    let top = (1.0 + rolloff) / 2.0;
    let df = top / steps as f64;
    let total: f64 = (0..steps)
        .map(|k| rc_density((k as f64 + 0.5) * df, rolloff))
        .sum();
    let mut tail = 0.0;
    for k in (0..steps).rev() {
        tail += rc_density((k as f64 + 0.5) * df, rolloff);
        // Half the rest of the two-sided power, (1 - share) total, lies above the edge.
        if tail >= 0.5 * (1.0 - share) * 2.0 * total {
            return 2.0 * k as f64 * df;
        }
    }
    0.0
}

/// The width, in symbol rates, between the points `x_db` under the flat part of the raised-cosine spectrum.
pub fn rc_x_db_width(rolloff: f64, x_db: f64) -> f64 {
    let level = 10f64.powf(-x_db / 10.0);
    let theta = (2.0 * level - 1.0).acos();
    2.0 * ((1.0 - rolloff) / 2.0 + theta / PI * rolloff)
}

/// One PSK stream with root-raised-cosine pulses.
#[derive(Clone, Debug)]
struct Stream {
    symbols: Vec<Complex32>,
    first: i64,
    rng: Rng,
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct ObwGenerator {
    rate: f64,
    sps: f64,
    psk: u32,
    table: Vec<f32>,
    main: Stream,
    neighbour: Option<(Stream, f32, f64)>,
    n: u64,
}

impl ObwGenerator {
    /// The signal of the settings `v` at `rate_hz`, its symbols from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> ObwGenerator {
        let rs = v.get(&GEN_SYMBOL_RATE);
        let a = v.get(&GEN_ROLLOFF);
        let table = (0..=(2.0 * SPAN) as usize * TABLE)
            .map(|k| rrc(k as f64 / TABLE as f64 - SPAN, a) as f32)
            .collect();
        let stream = |s: u64| Stream {
            symbols: Vec::new(),
            first: 0,
            rng: Rng::new(s),
        };
        let neighbour = v.get(&GEN_NEIGHBOUR).then(|| {
            let amp = 10f64.powf(v.get(&GEN_NEIGHBOUR_DB) / 20.0) as f32;
            let off = v.get(&GEN_NEIGHBOUR_OFFSET) * rc_share_width(a, 0.99) * rs;
            (stream(seed ^ 0x6e65_6967_6862), amp, off)
        });
        ObwGenerator {
            rate: rate_hz,
            sps: rate_hz / rs,
            psk: v.get(&GEN_PSK).clamp(2, 16) as u32,
            table,
            main: stream(seed ^ 0x6f62_7773_796d),
            neighbour,
            n: 0,
        }
    }

    /// The pulse-shaped value of `s` at sample `n`: the symbols within the span, each its pulse at the distance in symbols.
    fn value(&self, s: &mut Stream, n: u64) -> Complex32 {
        let t = n as f64 / self.sps;
        let lo = (t - SPAN).ceil() as i64;
        let hi = (t + SPAN).floor() as i64;
        while s.first + (s.symbols.len() as i64) <= hi {
            let k = (s.rng.uniform() * f64::from(self.psk)) as u32 % self.psk;
            let ph = TAU * f64::from(k) / f64::from(self.psk) + PI / f64::from(self.psk);
            s.symbols.push(Complex32::from_polar(1.0, ph as f32));
        }
        let drop = (lo - s.first).max(0) as usize;
        if drop > 64 {
            s.symbols.drain(..drop);
            s.first += drop as i64;
        }
        let mut z = Complex32::default();
        for k in lo.max(s.first)..=hi {
            let x = (t - k as f64 + SPAN) * TABLE as f64;
            let i = x.floor() as usize;
            let w = (x - i as f64) as f32;
            let p = self.table[i.min(self.table.len() - 1)] * (1.0 - w)
                + self.table[(i + 1).min(self.table.len() - 1)] * w;
            z += s.symbols[(k - s.first) as usize] * p;
        }
        z
    }
}

impl Generator for ObwGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut main = std::mem::replace(
            &mut self.main,
            Stream {
                symbols: Vec::new(),
                first: 0,
                rng: Rng::new(0),
            },
        );
        let mut nb = self.neighbour.take();
        for z in out.iter_mut() {
            *z = self.value(&mut main, self.n);
            if let Some((s, amp, off)) = nb.as_mut() {
                let ph = TAU * (*off * self.n as f64 / self.rate).fract();
                *z += self.value(s, self.n) * Complex32::from_polar(*amp, ph as f32);
            }
            self.n += 1;
        }
        self.main = main;
        self.neighbour = nb;
        out.len()
    }
    /// Unit power at the symbol rate: the impairments state the noise as Es/N0.
    fn signal_info(&self) -> Option<SignalInfo> {
        let rs = self.rate / self.sps;
        Some(SignalInfo::new(rs).with_symbol_rate(rs).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_raised_cosine_widths_of_a_roll_off_of_0_35() {
        assert!(
            (rc_share_width(0.35, 0.99) - 1.167).abs() < 0.002,
            "{}",
            rc_share_width(0.35, 0.99)
        );
        assert!(
            (rc_x_db_width(0.35, 26.0) - 1.328).abs() < 0.002,
            "{}",
            rc_x_db_width(0.35, 26.0)
        );
    }
}
