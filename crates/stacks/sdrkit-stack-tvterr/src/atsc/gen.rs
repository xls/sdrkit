//! The ATSC 1.0 test signal: numbered transport packets, the field encoder, 8 levels plus the 1.25 pilot, the VSB baseband centred
//! on the channel (symbols turned by (−j)^k and shaped by the root-raised-cosine pulse at one sample per symbol), resampled to
//! the output rate. It makes the clean signal at unit mean power, and the framework adds the impairments.
//!
//! Standard: ATSC A/53 Part 2 clause 6.

use super::fec::{FieldEncoder, DATA_SEGS};
use super::rx::{vsb_pulse, FSYM};
use crate::band::resampler;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::source::Numbered;

/// The bandwidth the SNR of the test signal is measured in.
pub const NOISE_BANDWIDTH_HZ: f64 = 5.38e6;

/// The net bit rate: 312 packets of 188 bytes per field.
pub const BIT_RATE: f64 = 312.0 * 188.0 * 8.0 * FSYM / (313.0 * 832.0);

/// The generator's options.
#[derive(Clone, Debug, PartialEq)]
pub struct AtscGenParams {
    /// Output rate.
    pub rate_hz: Hz,
    /// The packets: numbered packets of this pattern.
    pub packets: Numbered,
}

impl Default for AtscGenParams {
    fn default() -> Self {
        AtscGenParams {
            rate_hz: 8e6,
            packets: Numbered::TERRESTRIAL,
        }
    }
}

/// The generator.
pub struct AtscGenerator {
    p: AtscGenParams,
    enc: FieldEncoder,
    n: u32,
    k: usize,
    pulse: Fir<Complex32>,
    rs: FractionalResampler<Complex32>,
    pending: Vec<Complex32>,
    at: usize,
    lv: Vec<f32>,
    tmp: Vec<Complex32>,
    pk: Vec<[u8; TS]>,
}

impl AtscGenerator {
    /// A generator with these options.
    ///
    /// # Errors
    ///
    /// [`DspError`] if no resampler serves the output rate.
    pub fn new(p: AtscGenParams) -> Result<AtscGenerator, DspError> {
        Ok(AtscGenerator {
            // The pulse leaves ±3.0 MHz: flat to there, cut where the first alias or image of that band begins.
            rs: resampler(FSYM, p.rate_hz, 3.0e6, p.rate_hz.min(FSYM) - 3.0e6)?,
            p,
            enc: FieldEncoder::new(),
            n: 0,
            k: 0,
            pulse: Fir::new(&vsb_pulse()),
            pending: Vec::new(),
            at: 0,
            lv: Vec::new(),
            tmp: Vec::new(),
            pk: Vec::with_capacity(DATA_SEGS),
        })
    }

    /// From the engine's configuration (the stack has no test-signal options of its own).
    ///
    /// # Errors
    ///
    /// As [`AtscGenerator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<AtscGenerator, DspError> {
        AtscGenerator::new(AtscGenParams {
            rate_hz: cfg.rate_hz,
            ..Default::default()
        })
    }

    fn next_field(&mut self) {
        self.pk.clear();
        for _ in 0..DATA_SEGS {
            self.pk.push(self.p.packets.packet(self.n));
            self.n = self.n.wrapping_add(1);
        }
        self.lv.clear();
        self.enc.encode_field(&self.pk, &mut self.lv);
        self.tmp.clear();
        // Unit power at the output: levels power 21 + pilot 1.5625, through a pulse of unit energy.
        let g = (1.0f32 / (21.0 + 1.5625)).sqrt();
        let rot = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, -1.0),
            Complex32::new(-1.0, 0.0),
            Complex32::new(0.0, 1.0),
        ];
        for &l in &self.lv {
            let b = (l + 1.25) * g;
            let s = rot[self.k & 3] * b;
            self.k = self.k.wrapping_add(1);
            self.tmp.push(s);
        }
        self.pulse.process_in_place(&mut self.tmp);
        self.pending.clear();
        self.rs.process(&self.tmp, &mut self.pending);
        self.at = 0;
    }

    /// Seconds of one field.
    pub fn field_seconds() -> f64 {
        313.0 * 832.0 / FSYM
    }
}

impl Generator for AtscGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                self.next_field();
                continue;
            }
            let n = (out.len() - w).min(self.pending.len() - self.at);
            out[w..w + n].copy_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            w += n;
        }
        w
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(NOISE_BANDWIDTH_HZ)
                .with_symbol_rate(FSYM)
                .with_bit_rate(BIT_RATE),
        )
    }
}
