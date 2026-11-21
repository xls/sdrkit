//! The DVB-C test-signal generator: transport packets through the outer code, the byte to m-tuple
//! mapping with differential quadrant coding, and root raised cosine pulses. The signal is clean: the framework adds the
//! impairments.

use super::qam::{Mapper, Qam};
use super::{GEN_ANNEX_C, GEN_QAM, GEN_SYMBOL_RATE};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo, Values};
use sdrkit_proto_dvbs2::transmit::{payload, signal_info, Payload, TxShaper, GEN_PAYLOAD};
use sdrkit_proto_mpegts::outer::{Dispersal, OuterEncoder, RS_N as RSW};
use sdrkit_proto_mpegts::source::PacketSource;

/// The generator's parameters.
pub struct DvbcGenParams {
    /// Sample rate.
    pub fs: f64,
    /// The order.
    pub qam: Qam,
    /// Annex C (roll-off 0.13).
    pub annex_c: bool,
    /// Symbol rate, Hz (0: 6.952 Msym/s, 5.274 for annex C).
    pub symbol_rate: f64,
    /// What it sends.
    pub source: Box<dyn PacketSource>,
}

impl DvbcGenParams {
    /// 64-QAM at 6.952 Msym/s with the test multiplex, at `fs`.
    pub fn new(fs: f64) -> Self {
        DvbcGenParams {
            fs,
            qam: Qam::Q64,
            annex_c: false,
            symbol_rate: 0.0,
            source: payload(false),
        }
    }
    /// The parameters of the stack's `gen_` settings. The schema of `s` must be that of [`super::DvbcSetting`].
    pub fn from_settings(fs: f64, s: &Values) -> Self {
        let mut p = DvbcGenParams::new(fs);
        p.qam = s.get(&GEN_QAM);
        p.annex_c = s.get(&GEN_ANNEX_C);
        p.symbol_rate = s.get(&GEN_SYMBOL_RATE);
        p.source = payload(s.get(&GEN_PAYLOAD) == Payload::Numbered);
        p
    }
    /// The symbol rate used: the setting, else the default of the annex, at most `fs / 1.2`.
    pub fn rate(&self) -> f64 {
        let r = if self.symbol_rate > 0.0 {
            self.symbol_rate
        } else if self.annex_c {
            5.274e6
        } else {
            6.952e6
        };
        r.min(self.fs / 1.2)
    }
    /// The roll-off of the annex.
    pub fn alpha(&self) -> f64 {
        if self.annex_c {
            0.13
        } else {
            0.15
        }
    }
}

/// The DVB-C generator.
pub struct DvbcGenerator {
    fs: f64,
    rs: f64,
    alpha: f64,
    bits: usize,
    source: Box<dyn PacketSource>,
    outer: OuterEncoder,
    mapper: Mapper,
    shaper: Option<TxShaper>,
    syms: Vec<Complex32>,
}

impl DvbcGenerator {
    /// A generator with these parameters.
    pub fn new(p: DvbcGenParams) -> Self {
        let rs = p.rate();
        let alpha = p.alpha();
        DvbcGenerator {
            fs: p.fs,
            rs,
            alpha,
            bits: p.qam.bits(),
            shaper: TxShaper::new(p.fs, rs, alpha).ok(),
            source: p.source,
            outer: OuterEncoder::new(Dispersal::DvbGroups),
            mapper: Mapper::new(p.qam),
            syms: Vec::new(),
        }
    }
    /// The generator of the stack's generator configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(DvbcGenParams::from_settings(cfg.rate_hz, &cfg.settings))
    }
}

impl Generator for DvbcGenerator {
    fn rate_hz(&self) -> f64 {
        self.fs
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let Some(shaper) = self.shaper.as_mut() else {
            return 0;
        };
        let mut p = [0u8; 188];
        while shaper.ready() < out.len() {
            self.source.next_packet(&mut p);
            let mut w = [0u8; RSW];
            self.outer.encode(&p, &mut w);
            self.syms.clear();
            self.mapper.push(&w, &mut self.syms);
            shaper.push(&self.syms);
        }
        shaper.pop(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let net = self.rs * self.bits as f64 * 188.0 / 204.0;
        Some(signal_info(self.rs, self.alpha, net))
    }
}
