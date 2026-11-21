//! The serial-tone front: the 24 kHz channel of the family front end mixed down by the 1800 Hz carrier, through a root-raised
//! cosine matched filter (roll-off 0.35, 12 symbols, unit energy) and decimated to 2 samples a symbol or kept at 10.

use super::{BAUD, CARRIER_HZ};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_core::Complex32;

/// The input rate, Hz.
pub const RATE_IN: f64 = 24_000.0;
/// The roll-off of the pulse.
pub const ROLL_OFF: f64 = 0.35;
/// The span of the matched filter, symbols.
pub const SPAN: f64 = 12.0;

/// The matched filter's taps at 10 samples a symbol (121 taps).
pub fn matched_taps() -> Vec<f32> {
    taps(
        Shape::Rrc { beta: ROLL_OFF },
        RATE_IN / BAUD,
        SPAN,
        Norm::Energy,
    )
    .expect("the pulse is valid")
}

/// The front: mixer, matched filter, decimator.
pub struct SerialFront {
    nco: Nco,
    fir: FirDecimator<Complex32>,
    mixed: Vec<Complex32>,
}

impl SerialFront {
    /// A front whose output has `RATE_IN / decim` samples a second (`decim` 5 gives 2 samples a symbol).
    pub fn new(decim: usize) -> Self {
        SerialFront {
            nco: Nco::new(RATE_IN, -CARRIER_HZ),
            fir: FirDecimator::new(&matched_taps(), decim.max(1)),
            mixed: Vec::new(),
        }
    }
    /// Processes 24 kHz samples (dial at 0 Hz), appending output samples (carrier at 0 Hz). A sample that is not finite becomes 0.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.clear();
        self.mixed.extend(x.iter().map(|v| {
            if v.re.is_finite() && v.im.is_finite() {
                *v
            } else {
                Complex32::default()
            }
        }));
        self.nco.mix_in_place(&mut self.mixed);
        self.fir.process(&self.mixed, out);
    }
    /// Starts again from silence.
    pub fn reset(&mut self) {
        self.nco = Nco::new(RATE_IN, -CARRIER_HZ);
        self.fir.reset();
    }
}

/// `exp(j phase)`.
pub fn cis(phase: f64) -> Complex32 {
    let (s, c) = phase.sin_cos();
    Complex32::new(c as f32, s as f32)
}
