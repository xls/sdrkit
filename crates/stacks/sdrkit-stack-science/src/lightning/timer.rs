//! The sferic timer: impulses over the noise floor, each timed at the half-peak point of its leading edge to a fraction of a
//! sample. A double notch on the carrier of a timing transmitter can run in front, so that the strong carrier does not lift
//! the floor.

use sdrkit_algo_dsp::impulse::{leading_edge, Impulse, ImpulseConfig, ImpulseDetector};
use sdrkit_algo_dsp::notch::CarrierNotch;
use sdrkit_core::Complex32;

/// A pulse whose edge lies less than this after the edge of the pulse before is that pulse's ringing or sky wave, and is
/// dropped, s.
pub const MERGE_S: f64 = 1e-3;
/// The width of the carrier notch, Hz.
pub const NOTCH_WIDTH_HZ: f64 = 300.0;
/// Input samples filtered at a time.
const PIECE: usize = 8192;

/// One timed sferic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    /// The half-peak point of the leading edge: an absolute input sample index with its fraction.
    pub edge: f64,
    /// The 1-sigma error of `edge`, s.
    pub sigma_s: f64,
    /// The peak envelope over the floor, dB.
    pub peak_db: f64,
    /// The peak envelope.
    pub amplitude: f64,
    /// The time the envelope stays at half its peak or above, µs.
    pub width_us: f64,
    /// From 10 % to 90 % of the peak envelope, µs.
    pub rise_us: f64,
    /// The mean frequency over the half-peak span, Hz from the tuned centre.
    pub mean_hz: f64,
    /// The absolute input sample index at which the pulse was known.
    pub given_at: u64,
}

/// The sferic timer (module documentation).
#[derive(Clone, Debug)]
pub struct SfericTimer {
    rate: f64,
    det: ImpulseDetector,
    notch: Option<CarrierNotch>,
    merge: f64,
    last_edge: Option<f64>,
    buf: Vec<Complex32>,
    imps: Vec<Impulse>,
    env: Vec<f64>,
    too_long: u64,
    merged: u64,
}

impl SfericTimer {
    /// A timer for input at `rate` with the start level `threshold_db` over the floor, and a notch on the carrier at
    /// `notch_hz` from the tuned centre when it is `Some`. Errors when the rate is not above 0 or the notch offset lies outside
    /// `+-rate/2`.
    ///
    /// # Errors
    ///
    /// [`sdrkit_algo_dsp::DspError`] of the refused value.
    pub fn new(
        rate: f64,
        threshold_db: f64,
        notch_hz: Option<f64>,
    ) -> Result<Self, sdrkit_algo_dsp::DspError> {
        let det = ImpulseDetector::new(ImpulseConfig::timer(rate, threshold_db))?;
        let notch = match notch_hz {
            Some(hz) => Some(CarrierNotch::new(rate, hz, NOTCH_WIDTH_HZ)?),
            None => None,
        };
        Ok(SfericTimer {
            rate,
            det,
            notch,
            merge: MERGE_S * rate,
            last_edge: None,
            buf: Vec::with_capacity(PIECE),
            imps: Vec::new(),
            env: Vec::new(),
            too_long: 0,
            merged: 0,
        })
    }

    /// The input rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The carrier notch offset, Hz, when a notch runs.
    pub fn notch_hz(&self) -> Option<f64> {
        self.notch.as_ref().map(CarrierNotch::offset_hz)
    }

    /// Sets the start level to `threshold_db` over the floor and the end level 3 dB lower, from the next sample on.
    pub fn set_threshold(&mut self, threshold_db: f64) {
        self.det.set_levels(threshold_db, threshold_db - 3.0);
    }

    /// The floor (mean power of the noise), once it has a first value.
    pub fn floor(&self) -> Option<f64> {
        self.det.floor()
    }

    /// Whether the floor is valid, so that pulses can start.
    pub fn is_valid(&self) -> bool {
        self.det.is_valid()
    }

    /// Impulses dropped as too long for lightning.
    pub fn too_long(&self) -> u64 {
        self.too_long
    }

    /// Pulses dropped as the ringing or sky wave of the pulse before.
    pub fn merged(&self) -> u64 {
        self.merged
    }

    /// Learns the floor again and forgets the impulse being measured; the counters start again.
    pub fn reset(&mut self) {
        self.det.reset();
        if let Some(n) = &mut self.notch {
            n.reset();
        }
        self.last_edge = None;
        self.too_long = 0;
        self.merged = 0;
    }

    /// Takes `x`, whose first sample has the absolute index `first`, and appends each pulse timed to `out`, in order.
    pub fn process(&mut self, x: &[Complex32], first: u64, out: &mut Vec<Pulse>) {
        for (k, piece) in x.chunks(PIECE).enumerate() {
            let at = first + (k * PIECE) as u64;
            self.imps.clear();
            match &mut self.notch {
                Some(n) => {
                    self.buf.clear();
                    self.buf.extend_from_slice(piece);
                    n.process(&mut self.buf);
                    self.det.process(&self.buf, at, &mut self.imps);
                }
                None => self.det.process(piece, at, &mut self.imps),
            }
            self.time(out);
        }
    }

    /// The input ended: times the impulse still being measured.
    pub fn flush(&mut self, out: &mut Vec<Pulse>) {
        self.imps.clear();
        self.det.flush(&mut self.imps);
        self.time(out);
    }

    /// Times the impulses in `self.imps`.
    fn time(&mut self, out: &mut Vec<Pulse>) {
        let us = 1e6 / self.rate;
        for imp in self.imps.drain(..) {
            if imp.too_long {
                self.too_long += 1;
                continue;
            }
            let Some(e) = leading_edge(&imp.samples, imp.floor, &mut self.env) else {
                continue;
            };
            let edge = imp.samples_from as f64 + e.edge;
            if self.last_edge.is_some_and(|last| edge - last < self.merge) {
                self.merged += 1;
                continue;
            }
            self.last_edge = Some(edge);
            out.push(Pulse {
                edge,
                sigma_s: e.sigma / self.rate,
                peak_db: e.peak_db,
                amplitude: e.amplitude,
                width_us: e.width * us,
                rise_us: e.rise * us,
                mean_hz: e.frequency * self.rate,
                given_at: imp.given_at,
            });
        }
    }
}
