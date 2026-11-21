//! Generic PSK reception: the BPSK, QPSK, offset QPSK and 8PSK point maps and labels, the front end, the coarse carrier searches,
//! the streaming demodulator, and the blind analyser that finds the symbol rate, the order, the pulse shape and the carrier.
//!
//! | Step | Type | What it does |
//! |---|---|---|
//! | points and labels | [`PskOrder`], [`PskPulse`], [`index_bits`], [`bits_index`] | the point of each symbol index, the nearest index without `atan2`, Gray labels |
//! | front end | [`PskFrontEnd`] | bad samples to 0, blind IQ correction, the mixer, the anti-alias decimation, the input level, and the band locator of a wide capture |
//! | coarse carrier | [`CarrierSearch`], [`BandLocator`], [`SpectrumSum`] | the raised-cosine template fit or the M-th power line over averaged spectra; the strongest band of a wide capture |
//! | demodulate | [`PskDemodulator`] | the mixer with hand-over, the matched filter at the symbol instants, run-wise symbol timing and carrier loop (Costas, decision-directed or ML), EVM |
//! | analyse | [`analyse`], [`find_region`], [`snap`] | the blind analysis of a capture: the signal's band, rate, order, shape and carrier |
//!
//! Point maps: BPSK at 0 and 180 degrees, QPSK and offset QPSK at 45 degrees plus multiples of 90, 8PSK at multiples of 45, each
//! counted counter-clockwise from index 0.

mod analyse;
mod demod;
mod front;
mod search;

pub use analyse::{
    analyse, find_region, snap, standard_rates, Analysis, AnalysisNote, Found, OrderTrial,
};
pub use demod::{PskCarrier, PskDemodConfig, PskDemodulator};
pub(crate) use demod::Decided;
pub use front::PskFrontEnd;
pub use search::{
    BandLocator, CarrierSearch, SearchConfig, SearchKind, SearchOutcome, SpectrumSum,
};

use crate::Complex32;
use sdrkit_algo_dsp::pulse::{self, Norm};
use sdrkit_util_bits::gray::{gray, ungray};
use std::f64::consts::PI;

/// tan(22.5 degrees): the edge between an axis point and a diagonal point of 8PSK.
const TAN_PI_8: f32 = 0.414_213_57;

/// The modulation order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PskOrder {
    /// Binary PSK: one bit per symbol.
    #[default]
    Bpsk,
    /// Quadrature PSK: I and Q at the same instant.
    Qpsk,
    /// Offset QPSK: Q half a symbol after I.
    Oqpsk,
    /// 8-ary PSK.
    Psk8,
}

impl PskOrder {
    /// Every order.
    pub const ALL: [PskOrder; 4] = [
        PskOrder::Bpsk,
        PskOrder::Qpsk,
        PskOrder::Oqpsk,
        PskOrder::Psk8,
    ];

    /// The number of points.
    pub fn m(self) -> usize {
        match self {
            PskOrder::Bpsk => 2,
            PskOrder::Qpsk | PskOrder::Oqpsk => 4,
            PskOrder::Psk8 => 8,
        }
    }

    /// Bits per symbol.
    pub fn bits(self) -> usize {
        match self {
            PskOrder::Bpsk => 1,
            PskOrder::Qpsk | PskOrder::Oqpsk => 2,
            PskOrder::Psk8 => 3,
        }
    }

    /// The display name ("BPSK", "QPSK", "OQPSK", "8PSK").
    pub fn name(self) -> &'static str {
        match self {
            PskOrder::Bpsk => "BPSK",
            PskOrder::Qpsk => "QPSK",
            PskOrder::Oqpsk => "OQPSK",
            PskOrder::Psk8 => "8PSK",
        }
    }

    /// The unit point of symbol index `k` (taken modulo the order).
    pub fn point(self, k: usize) -> Complex32 {
        let a = match self {
            PskOrder::Bpsk => PI * (k % 2) as f64,
            PskOrder::Qpsk | PskOrder::Oqpsk => PI / 4.0 + PI / 2.0 * (k % 4) as f64,
            PskOrder::Psk8 => PI / 4.0 * (k % 8) as f64,
        };
        Complex32::new(a.cos() as f32, a.sin() as f32)
    }

    /// The unit point nearest to `y`, by sign and sector tests: [`PskOrder::point`] of [`PskOrder::decide`] without the
    /// angle. The signs are set as bits, with no branch on the value of `y`: the symbols of a signal are random, so such
    /// a branch is guessed wrong for every second symbol.
    #[inline]
    pub fn nearest(self, y: Complex32) -> Complex32 {
        const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
        // `size`, negative when `minus`.
        let signed = |size: f32, minus: bool| f32::from_bits(size.to_bits() | (u32::from(minus) << 31));
        // Below 0, or not a number.
        let under = |v: f32| (v < 0.0) | v.is_nan();
        let (left, low) = (under(y.re), under(y.im));
        match self {
            PskOrder::Bpsk => Complex32::new(signed(1.0, y.re < 0.0), 0.0),
            PskOrder::Qpsk | PskOrder::Oqpsk => Complex32::new(signed(S, left), signed(S, low)),
            PskOrder::Psk8 => {
                let (ax, ay) = (y.re.abs(), y.im.abs());
                // On the real axis, else on the imaginary axis, else on a diagonal.
                let real = ay <= TAN_PI_8 * ax;
                let imag = !real & (ax <= TAN_PI_8 * ay);
                let diagonal = !real & !imag;
                let pick = |on: bool, size: f32| size.to_bits() & u32::from(on).wrapping_neg();
                let re = f32::from_bits(pick(real, 1.0) | pick(diagonal, S));
                let im = f32::from_bits(pick(imag, 1.0) | pick(diagonal, S));
                Complex32::new(signed(re, left & !imag), signed(im, low & !real))
            }
        }
    }

    /// The index of the point nearest to `y`, by sign and sector tests.
    pub fn decide(self, y: Complex32) -> usize {
        match self {
            PskOrder::Bpsk => (y.re < 0.0) as usize,
            PskOrder::Qpsk | PskOrder::Oqpsk => match (y.re >= 0.0, y.im >= 0.0) {
                (true, true) => 0,
                (false, true) => 1,
                (false, false) => 2,
                (true, false) => 3,
            },
            PskOrder::Psk8 => {
                let (ax, ay) = (y.re.abs(), y.im.abs());
                if ay <= TAN_PI_8 * ax {
                    if y.re >= 0.0 {
                        0
                    } else {
                        4
                    }
                } else if ax <= TAN_PI_8 * ay {
                    if y.im >= 0.0 {
                        2
                    } else {
                        6
                    }
                } else {
                    match (y.re >= 0.0, y.im >= 0.0) {
                        (true, true) => 1,
                        (false, true) => 3,
                        (false, false) => 5,
                        (true, false) => 7,
                    }
                }
            }
        }
    }
}

/// The pulse shape of the transmitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PskPulse {
    /// Rectangular pulses, one symbol long.
    Rect,
    /// Root raised cosine with this roll-off.
    Rrc(f64),
}

impl PskPulse {
    /// The roll-off: that of the root raised cosine, 0 for rectangular pulses.
    pub fn roll_off(self) -> f64 {
        match self {
            PskPulse::Rect => 0.0,
            PskPulse::Rrc(b) => b,
        }
    }

    /// The matched filter at `sps` samples per symbol over `span` symbols (a rectangular pulse spans one), with unit energy. The
    /// roll-off is held within 0.01 to 1. Errors for `sps` not above 0 or a span too short for one tap.
    pub fn matched_filter(self, sps: f64, span: f64) -> Result<Vec<f32>, crate::ModemError> {
        let (shape, span) = match self {
            PskPulse::Rect => (pulse::Shape::Rect, 1.0),
            PskPulse::Rrc(b) => (
                pulse::Shape::Rrc {
                    beta: b.clamp(0.01, 1.0),
                },
                span,
            ),
        };
        Ok(pulse::taps(shape, sps, span, Norm::Energy)?)
    }

    /// The shape of the pulse through its matched filter, for the timing detector: a raised cosine of the same roll-off, or of
    /// roll-off 1 for rectangular pulses (whose triangle it is close to).
    pub fn overall(self) -> pulse::Shape {
        match self {
            PskPulse::Rect => pulse::Shape::Rc { beta: 1.0 },
            PskPulse::Rrc(b) => pulse::Shape::Rc {
                beta: b.clamp(0.01, 1.0),
            },
        }
    }
}

/// Appends the bits of symbol index `k`, first bit first. BPSK: the index. QPSK and offset QPSK: bit 0 from I (I below 0 is 1)
/// and bit 1 from Q, so indexes 0 to 3 are 00, 10, 11, 01. 8PSK: the Gray code of the index, most significant bit first.
pub fn index_bits(order: PskOrder, k: usize, out: &mut Vec<u8>) {
    match order {
        PskOrder::Bpsk => out.push((k & 1) as u8),
        PskOrder::Qpsk | PskOrder::Oqpsk => {
            let (b0, b1) = [(0, 0), (1, 0), (1, 1), (0, 1)][k & 3];
            out.push(b0);
            out.push(b1);
        }
        PskOrder::Psk8 => {
            let g = gray(k as u32 & 7);
            out.push((g >> 2) as u8 & 1);
            out.push((g >> 1) as u8 & 1);
            out.push(g as u8 & 1);
        }
    }
}

/// The symbol index of the first [`PskOrder::bits`] bits of `b` (the inverse of [`index_bits`]). Errors when `b` is too short.
pub fn bits_index(order: PskOrder, b: &[u8]) -> Result<usize, crate::ModemError> {
    if b.len() < order.bits() {
        return Err(crate::ModemError::InputTooShort { need: order.bits() });
    }
    Ok(match order {
        PskOrder::Bpsk => (b[0] & 1) as usize,
        PskOrder::Qpsk | PskOrder::Oqpsk => match (b[0] & 1, b[1] & 1) {
            (0, 0) => 0,
            (1, 0) => 1,
            (1, 1) => 2,
            _ => 3,
        },
        PskOrder::Psk8 => {
            let g = ((b[0] & 1) as u32) << 2 | ((b[1] & 1) as u32) << 1 | (b[2] & 1) as u32;
            ungray(g) as usize
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_sit_at_their_angles() {
        let deg = |o: PskOrder, k| {
            let p = o.point(k);
            (p.im as f64)
                .atan2(p.re as f64)
                .to_degrees()
                .rem_euclid(360.0)
        };
        assert!((deg(PskOrder::Bpsk, 1) - 180.0).abs() < 1e-4);
        for k in 0..4 {
            assert!((deg(PskOrder::Qpsk, k) - (45.0 + 90.0 * k as f64)).abs() < 1e-4);
        }
        for k in 0..8 {
            assert!((deg(PskOrder::Psk8, k) - 45.0 * k as f64).abs() < 1e-4);
        }
    }

    #[test]
    fn decisions_are_nearest_points() {
        for o in PskOrder::ALL {
            for k in 0..o.m() {
                for d in [-0.35f32, 0.0, 0.35] {
                    let y = o.point(k)
                        * Complex32::from_polar(0.7, d * std::f32::consts::PI / o.m() as f32);
                    assert_eq!(o.decide(y), k, "{o:?} {k} {d}");
                }
            }
        }
    }

    #[test]
    fn labels_match_the_table_and_invert() {
        let mut v = Vec::new();
        for k in 0..4 {
            index_bits(PskOrder::Qpsk, k, &mut v);
        }
        assert_eq!(v, [0, 0, 1, 0, 1, 1, 0, 1]);
        // 8PSK: neighbours differ in one bit (Gray), 0 to 7 give 000 001 011 010 110 111 101 100.
        let mut v = Vec::new();
        for k in 0..8 {
            index_bits(PskOrder::Psk8, k, &mut v);
        }
        assert_eq!(
            v,
            [0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 0, 0]
        );
        for o in PskOrder::ALL {
            for k in 0..o.m() {
                let mut b = Vec::new();
                index_bits(o, k, &mut b);
                assert_eq!(bits_index(o, &b), Ok(k));
            }
        }
        assert!(bits_index(PskOrder::Psk8, &[1, 0]).is_err());
    }

    #[test]
    fn matched_filter_has_unit_energy() {
        let h = PskPulse::Rrc(0.5).matched_filter(4.0, 12.0).unwrap();
        let e: f32 = h.iter().map(|v| v * v).sum();
        assert!((e - 1.0).abs() < 1e-4);
        assert_eq!(h.len(), 49);
        let r = PskPulse::Rect.matched_filter(8.0, 8.0).unwrap();
        assert!((r.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-4);
    }
}
