//! The DTMB test signal: the transmitter of GB 20600-2006 from transport packets to frames (bit chain, system information,
//! frequency interleaver and inverse transform for C = 3780, the symbols themselves for C = 1, headers PN420, PN595 or PN945,
//! rotating or fixed), at 7.56 or 5.67 Msym/s, shaped by the square-root raised-cosine pulse at the output rate. It makes the
//! clean signal at unit body power, and the framework adds the impairments.
//!
//! Standard: GB 20600-2006 clause 4.

use super::chain::TxChain;
use super::pn::{Header, HeaderSeq};
use super::profile::{
    carriers, si_chips, si_symbol, Carriers, CodeRate, Interleaver, Mapping, Profile, DATA, SI,
};
use super::{Width, BODY, FSYM_6, FSYM_8, ROLLOFF, WIDTH};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::SimError;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_mpegts::source::{Numbered, NumberedSource};

/// The span of the pulse, symbols.
const PULSE_SPAN: f64 = 64.0;

/// Options.
#[derive(Clone, Debug, PartialEq)]
pub struct DtmbGenParams {
    /// Output rate.
    pub rate_hz: Hz,
    /// Header (C = 1 always uses PN595).
    pub header: Header,
    /// The header phase stays fixed.
    pub fixed_phase: bool,
    /// Single carrier (C = 1).
    pub single_carrier: bool,
    /// 6 MHz channel.
    pub width6: bool,
    /// The mapping; 4QAM-NR and 32QAM force rate 0.8.
    pub mapping: Mapping,
    /// The LDPC rate.
    pub code_rate: CodeRate,
    /// The time interleaver.
    pub interleaver: Interleaver,
    /// The numbered packets of the payload.
    pub packets: Numbered,
}

impl Default for DtmbGenParams {
    fn default() -> Self {
        DtmbGenParams {
            rate_hz: 10e6,
            header: Header::Pn945,
            fixed_phase: false,
            single_carrier: false,
            width6: false,
            mapping: Mapping::Qam64,
            code_rate: CodeRate::R06,
            interleaver: Interleaver::Mode1,
            packets: Numbered::TERRESTRIAL,
        }
    }
}

impl DtmbGenParams {
    /// The profile sent (rate 0.8 for 4QAM-NR and 32QAM).
    pub fn profile(&self) -> Profile {
        let rate = match self.mapping {
            Mapping::Qam4Nr | Mapping::Qam32 => CodeRate::R08,
            _ => self.code_rate,
        };
        Profile {
            mapping: self.mapping,
            rate,
            interleaver: self.interleaver,
        }
    }
    /// The carrier form sent.
    pub fn carriers(&self) -> Carriers {
        if self.single_carrier {
            Carriers::Single
        } else {
            Carriers::Multi
        }
    }
    /// The header sent.
    pub fn header_sent(&self) -> Header {
        if self.single_carrier {
            Header::Pn595
        } else {
            self.header
        }
    }
    /// The symbol rate.
    pub fn symbol_rate(&self) -> f64 {
        if self.width6 {
            FSYM_6
        } else {
            FSYM_8
        }
    }
}

/// The generator.
pub struct DtmbGenerator {
    p: DtmbGenParams,
    seq: HeaderSeq,
    chain: TxChain,
    si: [Complex32; SI],
    k: usize,
    fsym: f64,
    pulse: Linear,
    ifft: Fft,
    data: Vec<Complex32>,
    body: Vec<Complex32>,
    pending: Vec<Complex32>,
    at: usize,
}

impl DtmbGenerator {
    /// With these options.
    ///
    /// # Errors
    ///
    /// [`SimError`] if no pulse serves the output rate.
    pub fn new(p: DtmbGenParams) -> Result<DtmbGenerator, SimError> {
        let fsym = p.symbol_rate();
        let header = p.header_sent();
        let profile = p.profile();
        let chips = si_chips(profile.index()).unwrap_or([-1.0; SI]);
        let mut chain = TxChain::new(profile, Box::new(NumberedSource::new(p.packets)))
            .unwrap_or_else(|e| unreachable!("the DTMB codes are fixed: {e}"));
        // Warm-up: the interleaver runs until it holds no fill, so the first frame sent carries data in every branch.
        let mut data = vec![Complex32::new(0.0, 0.0); DATA];
        for _ in 0..profile.interleaver.delay_frames() {
            chain
                .frame(&mut data)
                .unwrap_or_else(|e| unreachable!("the DTMB codes are fixed: {e}"));
        }
        Ok(DtmbGenerator {
            seq: HeaderSeq::new(header, p.fixed_phase || header == Header::Pn595),
            chain,
            si: chips.map(si_symbol),
            k: 0,
            fsym,
            pulse: Linear::rrc(ROLLOFF, PULSE_SPAN, p.rate_hz / fsym)?,
            ifft: Fft::inverse(BODY),
            data,
            body: vec![Complex32::new(0.0, 0.0); BODY],
            pending: Vec::new(),
            at: 0,
            p,
        })
    }
    /// From the engine's configuration.
    ///
    /// # Errors
    ///
    /// As [`DtmbGenerator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<DtmbGenerator, SimError> {
        DtmbGenerator::new(DtmbGenParams {
            rate_hz: cfg.rate_hz,
            width6: cfg.settings.get(&WIDTH) == Width::W6,
            ..Default::default()
        })
    }
    /// The symbols of the next frame (header then body) at one sample per symbol.
    pub fn frame_symbols(&mut self) -> Vec<Complex32> {
        let mut syms = self.seq.header(self.k);
        self.k += 1;
        self.chain
            .frame(&mut self.data)
            .unwrap_or_else(|e| unreachable!("the DTMB codes are fixed: {e}"));
        if self.p.single_carrier {
            self.body[..SI].copy_from_slice(&self.si);
            self.body[SI..].copy_from_slice(&self.data);
        } else {
            let map = carriers();
            for (s, &c) in self.si.iter().zip(&map.si) {
                self.body[c as usize] = *s;
            }
            for (d, &c) in self.data.iter().zip(&map.data) {
                self.body[c as usize] = *d;
            }
            self.ifft.process(&mut self.body);
            let s = 1.0 / (BODY as f32).sqrt();
            self.body.iter_mut().for_each(|v| *v *= s);
        }
        syms.extend_from_slice(&self.body);
        syms
    }
    /// The data symbols of the last frame made, in stream order.
    pub fn data(&self) -> &[Complex32] {
        &self.data
    }
    fn frame(&mut self) {
        let syms = self.frame_symbols();
        self.pending.clear();
        self.pulse.push(&syms, &mut self.pending);
        self.at = 0;
    }
}

impl Generator for DtmbGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // C/N is stated in the symbol bandwidth.
        Some(SignalInfo::new(self.fsym).with_symbol_rate(self.fsym))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                self.frame();
                continue;
            }
            let n = (out.len() - w).min(self.pending.len() - self.at);
            out[w..w + n].copy_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            w += n;
        }
        w
    }
}
