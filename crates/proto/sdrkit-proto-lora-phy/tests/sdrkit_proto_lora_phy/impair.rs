//! A builder of test signals with the faults of real transmitters and radios, written from the definition of the signal:
//! a crystal error that moves the carrier and the chip clock together, a separate clock error, a carrier that drifts, frames
//! that follow each other or overlap, noise, a level step, a DC offset, IQ imbalance and a converter that clips.
//!
//! The defaults give the plain signal: one clean frame per [`Tx`], no fault of the radio.

use sdrkit_algo_sim::front::{add_dc, IqImbalance, Quantiser};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{LoraParams, TxFrame};

/// One transmitted frame.
#[derive(Clone, Debug)]
pub struct Tx {
    /// The payload bytes.
    pub payload: Vec<u8>,
    /// The start of the preamble, seconds.
    pub start_s: f64,
    /// The error of the transmitter crystal, ppm. A fast crystal (positive) raises the carrier by this share of the radio
    /// frequency and shortens the chips by the same share.
    pub crystal_ppm: f64,
    /// A further error of the chip clock alone, ppm (positive: longer chips).
    pub clock_ppm: f64,
    /// A further carrier offset, Hz.
    pub carrier_hz: f64,
    /// The change of the carrier during the frame, Hz/s.
    pub drift_hz_s: f64,
    /// The level relative to [`Radio::amp`], dB.
    pub level_db: f64,
    /// The start phase, radians.
    pub phase: f64,
}

impl Tx {
    /// A clean frame with `payload` that starts at `start_s`.
    pub fn new(payload: &[u8], start_s: f64) -> Self {
        Tx {
            payload: payload.to_vec(),
            start_s,
            crystal_ppm: 0.0,
            clock_ppm: 0.0,
            carrier_hz: 0.0,
            drift_hz_s: 0.0,
            level_db: 0.0,
            phase: 0.0,
        }
    }

    /// The frame on the air for a radio tuned to `rf_hz`.
    pub fn frame(&self, p: &LoraParams, rf_hz: f64, amp: f32) -> TxFrame {
        let mut f = TxFrame::new(p, &self.payload)
            .expect("a valid frame")
            .at(self.start_s)
            .carrier(self.carrier_hz + self.crystal_ppm * 1e-6 * rf_hz)
            .amplitude(amp * 10f32.powf(self.level_db as f32 / 20.0))
            .ppm(self.clock_ppm - self.crystal_ppm);
        f.phase = self.phase;
        f.drift_hz_s = self.drift_hz_s;
        f
    }

    /// The duration on the air, seconds.
    pub fn duration_s(&self, p: &LoraParams) -> f64 {
        self.frame(p, 0.0, 1.0).duration_s()
    }
}

/// The radio that hears the frames.
#[derive(Clone, Debug)]
pub struct Radio {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The frequency the radio is tuned to, Hz. The crystal error of a transmitter acts on it.
    pub rf_hz: f64,
    /// The amplitude of a frame at 0 dB. The converter is at full scale at 1.
    pub amp: f32,
    /// The signal to noise ratio of a frame at 0 dB in the LoRa bandwidth, dB. `None`: no noise.
    pub snr_db: Option<f64>,
    /// The seed of the noise.
    pub noise_seed: u64,
    /// A step of the gain: from this time (seconds) on, the level changes by this many dB.
    pub level_step: Option<(f64, f64)>,
    /// The DC offset of the converter.
    pub dc: Complex32,
    /// IQ imbalance: the gain error (dB) and the phase error (degrees).
    pub iq: Option<(f64, f64)>,
    /// The converter clips each of I and Q at full scale and rounds to this many bits. `None`: no converter.
    pub adc_bits: Option<u32>,
}

impl Radio {
    /// A radio at `rate` with no fault and no noise.
    pub fn new(rate: f64) -> Self {
        Radio {
            rate,
            rf_hz: 0.0,
            amp: 0.3,
            snr_db: None,
            noise_seed: 1,
            level_step: None,
            dc: Complex32::default(),
            iq: None,
            adc_bits: None,
        }
    }

    /// The samples from time 0 to `seconds` with these frames: the frames with their own offsets, then the noise, then the
    /// faults of the radio in the order gain step, IQ imbalance, DC offset, converter.
    pub fn render(&self, p: &LoraParams, frames: &[Tx], seconds: f64) -> Vec<Complex32> {
        let n = (seconds * self.rate).ceil().max(0.0) as usize;
        let mut x = vec![Complex32::default(); n];
        for t in frames {
            t.frame(p, self.rf_hz, self.amp)
                .add_into(&mut x, self.rate, 0);
        }
        if let Some(snr) = self.snr_db {
            let power = (self.amp as f64).powi(2) / 10f64.powf(snr / 10.0) * self.rate / p.bw;
            Awgn::with_power(power, self.noise_seed)
                .expect("a valid noise power")
                .process(&mut x);
        }
        if let Some((at, db)) = self.level_step {
            let from = ((at * self.rate).max(0.0) as usize).min(n);
            let g = 10f32.powf(db as f32 / 20.0);
            for v in &mut x[from..] {
                *v *= g;
            }
        }
        if let Some((gain_db, phase_deg)) = self.iq {
            IqImbalance::new(gain_db, phase_deg)
                .expect("a valid imbalance")
                .process(&mut x);
        }
        if self.dc != Complex32::default() {
            add_dc(&mut x, self.dc);
        }
        if let Some(bits) = self.adc_bits {
            Quantiser::new(bits).expect("valid bits").process(&mut x);
        }
        x
    }
}
