//! The LoRa modulator: data symbol values to complex samples at any rate, from the exact phase of the chirps.

use crate::coding;
use crate::error::Result;
use crate::params::LoraParams;
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_algo_sim::scene::Burst;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// `e^(j 2 pi cycles)`.
#[inline]
pub(crate) fn phasor(cycles: f64) -> Complex32 {
    let (s, c) = sin_cos(TAU * (cycles - cycles.floor()));
    Complex32::new(c, s)
}

/// One frame on the air: its setting, its data symbols, and how the transmitter sends it. The frame is the preamble, two
/// sync symbols, 2.25 down-chirps, two more up-chirps at SF5 and SF6, then the data symbols.
#[derive(Clone, Debug, PartialEq)]
pub struct TxFrame {
    /// The setting (SF, bandwidth, preamble, sync word, inverted IQ).
    pub params: LoraParams,
    /// The data symbol values.
    pub symbols: Vec<u16>,
    /// The start of the preamble, seconds of output time.
    pub start_s: f64,
    /// The carrier, Hz from 0 Hz of the output.
    pub carrier_hz: f64,
    /// The amplitude.
    pub amplitude: f32,
    /// The start phase, radians.
    pub phase: f64,
    /// The transmitter clock error, ppm. If it is positive, the chips are that much longer.
    pub ppm: f64,
    /// The carrier drift, Hz/s.
    pub drift_hz_s: f64,
}

impl TxFrame {
    /// A frame that carries `payload` with the setting `params`, at time 0, carrier 0 Hz, amplitude 1. If the setting is not
    /// valid, or the payload is longer than 255 bytes, this returns an error.
    pub fn new(params: &LoraParams, payload: &[u8]) -> Result<Self> {
        Ok(TxFrame::from_symbols(
            params,
            coding::encode(payload, params)?,
        ))
    }

    /// A frame of given data symbols.
    pub fn from_symbols(params: &LoraParams, symbols: Vec<u16>) -> Self {
        TxFrame {
            params: params.clone(),
            symbols,
            start_s: 0.0,
            carrier_hz: 0.0,
            amplitude: 1.0,
            phase: 0.0,
            ppm: 0.0,
            drift_hz_s: 0.0,
        }
    }

    /// Sets the start time, seconds.
    pub fn at(mut self, start_s: f64) -> Self {
        self.start_s = start_s;
        self
    }

    /// Sets the carrier, Hz.
    pub fn carrier(mut self, hz: f64) -> Self {
        self.carrier_hz = hz;
        self
    }

    /// Sets the amplitude.
    pub fn amplitude(mut self, a: f32) -> Self {
        self.amplitude = a;
        self
    }

    /// Sets the transmitter clock error, ppm.
    pub fn ppm(mut self, ppm: f64) -> Self {
        self.ppm = ppm;
        self
    }

    /// The length of the frame in chips, without the clock error.
    pub fn chips(&self) -> f64 {
        let n = self.params.n() as f64;
        (self.params.header_symbols() + self.symbols.len() as f64) * n
    }

    /// The duration of the frame with the clock error, seconds.
    pub fn duration_s(&self) -> f64 {
        self.chips() / self.params.bw * (1.0 + self.ppm * 1e-6)
    }

    /// The end time, seconds.
    pub fn end_s(&self) -> f64 {
        self.start_s + self.duration_s()
    }

    /// The baseband chirp phase (cycles) at chip position `u` from the frame start. Past the end, this returns `None`.
    #[inline]
    fn chirp_cycles(&self, u: f64, sync: [u16; 2]) -> Option<f64> {
        let p = &self.params;
        let n = p.n() as f64;
        let pre = p.preamble as f64;
        let up = |s: f64, x: f64| {
            let w = n - s;
            if x < w {
                (s * x + 0.5 * x * x) / n - 0.5 * x
            } else {
                (s * w + 0.5 * w * w) / n + 0.5 * (x - w) * (x - w) / n - 0.5 * x
            }
        };
        let k = (u / n).floor();
        let x = u - k * n;
        if k < pre {
            return Some(up(0.0, x));
        }
        if k < pre + 2.0 {
            return Some(up(sync[(k - pre) as usize] as f64, x));
        }
        let sfd_end = (pre + 4.25) * n;
        if u < sfd_end {
            let xs = u - (pre + 2.0) * n;
            let xx = xs - (xs / n).floor() * n;
            return Some(0.5 * xx - 0.5 * xx * xx / n);
        }
        let mut rest = u - sfd_end;
        if p.sf < 7 {
            if rest < 2.0 * n {
                let xx = rest - (rest / n).floor() * n;
                return Some(up(0.0, xx));
            }
            rest -= 2.0 * n;
        }
        let j = (rest / n).floor() as usize;
        let x = rest - j as f64 * n;
        let s = *self.symbols.get(j)? as f64;
        Some(up(s, x))
    }

    /// Adds the samples of the frame that fall in `out` to it. `out[0]` is output sample `first` at `rate` samples a second.
    pub fn add_into(&self, out: &mut [Complex32], rate: f64, first: u64) {
        if !(rate > 0.0 && rate.is_finite() && self.params.bw > 0.0) {
            return;
        }
        let t0 = self.start_s;
        let t1 = self.end_s();
        let n_out = out.len() as u64;
        let i0 = ((t0 * rate).ceil().max(0.0) as u64).max(first);
        let i1 = ((t1 * rate).floor().max(0.0) as u64 + 1).min(first + n_out);
        if i1 <= i0 {
            return;
        }
        let sync = self.params.sync_symbols();
        let chip_rate = self.params.bw / (1.0 + self.ppm * 1e-6);
        let conj = self.params.invert_iq;
        let ph0 = self.phase / TAU;
        let amp = self.amplitude;
        for i in i0..i1 {
            let t = i as f64 / rate - t0;
            let Some(c) = self.chirp_cycles(t * chip_rate, sync) else {
                continue;
            };
            let c = if conj { -c } else { c };
            let cyc = c + self.carrier_hz * t + 0.5 * self.drift_hz_s * t * t + ph0;
            out[(i - first) as usize] += phasor(cyc) * amp;
        }
    }

    /// The whole frame as a burst of a `sdrkit_algo_sim::scene::Scene` at `rate`. The burst starts at the sample nearest the start
    /// time, and the samples keep the exact start time, carrier and phase. The burst has no offset and a level of 0 dB.
    pub fn burst(&self, rate: f64) -> Burst {
        let i0 = (self.start_s * rate).round().max(0.0) as u64;
        let i1 = (self.end_s() * rate).floor().max(0.0) as u64 + 1;
        let mut samples = vec![Complex32::default(); i1.saturating_sub(i0) as usize];
        self.add_into(&mut samples, rate, i0);
        Burst {
            start_s: i0 as f64 / rate,
            offset_hz: 0.0,
            level_db: 0.0,
            samples,
        }
    }

    /// The frame from time 0 to its end and `tail_s` more, at `rate`.
    pub fn render(&self, rate: f64, tail_s: f64) -> Vec<Complex32> {
        let n = ((self.end_s() + tail_s.max(0.0)) * rate).ceil().max(0.0) as usize;
        let mut out = vec![Complex32::default(); n];
        self.add_into(&mut out, rate, 0);
        out
    }
}
