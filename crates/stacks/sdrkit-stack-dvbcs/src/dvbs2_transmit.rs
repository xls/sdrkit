//! The DVB-S2 chain of the sender of `dvbs` ([`crate::dvbs_transmit`], setting `tx_standard`): one transport stream with
//! constant coding and modulation.
//!
//! Each BBFRAME takes the next packets of the queue, a null packet in each packet slot that finds it empty, so the
//! transport stream keeps the useful bit rate and the PCRs keep their slot times; the sender never pads and sends no dummy
//! PLFRAME between frames of data. The coding is [`TsAdapter`] and [`S2Encoder`], the pulses root raised cosine with the
//! roll-off of the settings at the session's rate ([`TxShaper`]). A frame is coded once and shaped in blocks of
//! [`BLOCK`] symbols, so a pass makes no more samples than it writes. After `stop` the frame on air ends, and the fall
//! runs over dummy PLFRAMEs: no packet is cut. The level is [`rms`] of full scale, so that the peaks stay under the knee
//! of the soft limiter.
//!
//! Standard: ETSI EN 302 307-1.

use crate::dvbs_transmit::{TxError, RISE_S};
use sdrkit_components::tx::Ramp;
use sdrkit_core::Complex32;
use sdrkit_proto_dvbs2::modcod::{ModCod, Modulation};
use sdrkit_proto_dvbs2::s2encode::{roll_off_field, useful_bit_rate, S2Encoder, TsAdapter};
use sdrkit_proto_dvbs2::settings::{
    FecFrame, GenRollOff, TX_FRAME, TX_INVERSION, TX_MODCOD, TX_PILOTS, TX_PL_SCRAMBLING,
    TX_ROLLOFF, TX_SYMBOL_RATE,
};
use sdrkit_proto_dvbs2::transmit::TxShaper;
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::sendqueue::SendQueue;
use sdrkit_proto_mpegts::source::PacketSource;

/// The symbols shaped in one step.
pub const BLOCK: usize = 512;

/// The rms magnitude of the signal of `modulation` with `roll_off`, as a fraction of full scale. The highest peak of the
/// shaped signal grows with the rings of the constellation and as the roll-off falls (4.0 dB over the mean for QPSK at
/// 0.35, 7.2 dB for 32APSK at 0.20); each level keeps it under the knee of the soft limiter (0.9).
pub fn rms(modulation: Modulation, roll_off: GenRollOff) -> f32 {
    match (modulation, roll_off) {
        (Modulation::Qpsk | Modulation::Psk8, GenRollOff::R35) => 0.55,
        (Modulation::Qpsk | Modulation::Psk8, GenRollOff::R25) => 0.50,
        (Modulation::Qpsk | Modulation::Psk8, GenRollOff::R20) => 0.47,
        (Modulation::Apsk16, GenRollOff::R35) => 0.49,
        (Modulation::Apsk16, GenRollOff::R25) => 0.44,
        (Modulation::Apsk16, GenRollOff::R20) => 0.41,
        (Modulation::Apsk32, GenRollOff::R35) => 0.44,
        (Modulation::Apsk32, GenRollOff::R25) => 0.40,
        (Modulation::Apsk32, GenRollOff::R20) => 0.375,
    }
}

/// The values of the settings of a DVB-S2 transmission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct S2Config {
    /// The symbol rate, symbols per second.
    pub symbol_rate: f64,
    /// The MODCOD of every frame.
    pub modcod: ModCod,
    /// The FEC frame.
    pub frame: FecFrame,
    /// Pilot blocks.
    pub pilots: bool,
    /// The roll-off.
    pub roll_off: GenRollOff,
    /// The number of the Gold code of the PL scrambling.
    pub pl_scrambling: u32,
    /// Spectral inversion: Q negated.
    pub inversion: bool,
}

impl Default for S2Config {
    fn default() -> Self {
        S2Config {
            symbol_rate: TX_SYMBOL_RATE.default,
            modcod: TX_MODCOD.default.modcod(),
            frame: TX_FRAME.default,
            pilots: TX_PILOTS.default,
            roll_off: TX_ROLLOFF.default,
            pl_scrambling: TX_PL_SCRAMBLING.default as u32,
            inversion: TX_INVERSION.default,
        }
    }
}

impl S2Config {
    /// Whether the frames are short.
    pub fn short(&self) -> bool {
        self.frame == FecFrame::Short
    }

    /// The useful bit rate: the rate of the transport stream. 0 when the frame size has no code at the rate (9/10 short).
    pub fn bit_rate(&self) -> f64 {
        useful_bit_rate(self.symbol_rate, &self.modcod, self.short(), self.pilots).unwrap_or(0.0)
    }

    /// The lowest session rate for the symbol rate, Hz: the occupied band (1 + roll-off) with a margin of 1.3.
    pub fn min_rate(&self) -> f64 {
        self.symbol_rate * (1.0 + self.roll_off.value()) * 1.3
    }

    /// The rms magnitude of the signal, as a fraction of full scale.
    pub fn rms(&self) -> f32 {
        rms(self.modcod.modulation, self.roll_off)
    }
}

/// One DVB-S2 transmission: the coder and the pulse shaper at the output rate, with the level, the rise and fall and the
/// limiter.
pub struct S2Chain {
    cfg: S2Config,
    adapter: TsAdapter,
    coder: S2Encoder,
    data: usize,
    ro: u8,
    shaper: TxShaper,
    syms: Vec<Complex32>,
    at: usize,
    frames: u64,
    gain: f32,
    ramp: Ramp,
    stopping: bool,
}

impl S2Chain {
    /// The chain of `cfg` at `rate` Hz.
    ///
    /// # Errors
    ///
    /// [`TxError::NoShortFrame`] when the frame size has no code at the rate, [`TxError::RateTooLow`] when `rate` is below
    /// [`S2Config::min_rate`].
    pub fn new(cfg: &S2Config, rate: f64) -> Result<Self, TxError> {
        let coder = S2Encoder::new(cfg.modcod, cfg.short(), cfg.pilots, cfg.pl_scrambling);
        let code = coder.code().ok_or(TxError::NoShortFrame)?;
        let needs = cfg.min_rate();
        if rate.is_nan() || rate < needs {
            return Err(TxError::RateTooLow { rate, needs });
        }
        let shaper = TxShaper::new(rate, cfg.symbol_rate, cfg.roll_off.value())
            .map_err(|_| TxError::RateTooLow { rate, needs })?;
        Ok(S2Chain {
            cfg: *cfg,
            adapter: TsAdapter::default(),
            data: code.kbch / 8 - sdrkit_proto_ipcast::bbheader::BBHEADER_LEN,
            ro: roll_off_field(cfg.roll_off.value()),
            syms: Vec::with_capacity(coder.frame_symbols()),
            coder,
            shaper,
            at: 0,
            frames: 0,
            gain: cfg.rms(),
            ramp: Ramp::of_seconds(RISE_S, rate),
            stopping: false,
        })
    }

    /// The settings of the transmission.
    pub fn config(&self) -> &S2Config {
        &self.cfg
    }

    /// The fall starts at the end of the PLFRAME on air.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Whether `stop` was asked.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// The PLFRAMEs with data made so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Makes the symbols of the next PLFRAME: a frame of the packets of `queue`, or a dummy frame once the fall has
    /// started.
    fn frame(&mut self, queue: &mut SendQueue) {
        if self.stopping {
            self.ramp.fall();
            self.coder.dummy(&mut self.syms);
        } else {
            self.adapter
                .fill(self.coder.bbframe(), self.data, None, self.ro, &mut || {
                    let mut p = [0u8; TS];
                    queue.next_packet(&mut p);
                    p
                });
            self.coder.encode(&mut self.syms);
            self.frames += 1;
        }
        if self.cfg.inversion {
            for s in &mut self.syms {
                *s = s.conj();
            }
        }
        self.at = 0;
    }

    /// Makes the samples of the next [`BLOCK`] symbols at the output rate into `out` (cleared first), the packets of a new
    /// frame from `queue`. False when `out` holds the last samples of the transmission.
    pub fn block(&mut self, queue: &mut SendQueue, out: &mut Vec<Complex32>) -> bool {
        if self.at == self.syms.len() {
            self.frame(queue);
        }
        let n = (self.syms.len() - self.at).min(BLOCK);
        self.shaper.push(&self.syms[self.at..self.at + n]);
        self.at += n;
        out.clear();
        out.resize(self.shaper.ready(), Complex32::default());
        let n = self.shaper.pop(out);
        out.truncate(n);
        self.ramp.shape(self.gain, out)
    }
}

/// The sender of `cfg` at `rate` for 0.25 s of signal, the queue full of numbered packets (items: output samples; the CPU
/// share is of one core at that rate).
pub fn bench_sender(seconds: f64, cfg: &S2Config, rate: f64) -> sdrkit_core::BenchResult {
    use sdrkit_proto_mpegts::source::{Numbered, NumberedSource};
    let signal_s = 0.25;
    let packets = (cfg.bit_rate() * signal_s / (TS * 8) as f64).ceil() as usize;
    let mut src = NumberedSource::new(Numbered::SATELLITE_CABLE);
    let mut bytes = Vec::with_capacity(packets * TS);
    let mut p = [0u8; TS];
    for _ in 0..packets.min(sdrkit_proto_mpegts::sendqueue::QUEUE_PACKETS) {
        src.next_packet(&mut p);
        bytes.extend_from_slice(&p);
    }
    let samples = (signal_s * rate) as usize;
    let mut out = Vec::new();
    let Ok(mut chain) = S2Chain::new(cfg, rate) else {
        return sdrkit_core::BenchResult::default();
    };
    let mut queue = SendQueue::new(cfg.bit_rate());
    sdrkit_core::bench::measure(seconds, samples, signal_s, || {
        queue.clear();
        let _ = queue.append(&bytes);
        let mut made = 0;
        while made < samples {
            chain.block(&mut queue, &mut out);
            made += out.len();
        }
    })
}

/// The sender of the first benchmark: QPSK 3/4, normal frames, at 1 Msym/s.
pub fn bench_qpsk() -> S2Config {
    S2Config {
        symbol_rate: 1e6,
        modcod: ModCod::new(7).unwrap_or_else(|| unreachable!("MODCOD 7 exists")),
        ..S2Config::default()
    }
}

/// The sender of the second benchmark: 16APSK 3/4, normal frames, at 1 Msym/s.
pub fn bench_16apsk() -> S2Config {
    S2Config {
        symbol_rate: 1e6,
        modcod: ModCod::new(19).unwrap_or_else(|| unreachable!("MODCOD 19 exists")),
        ..S2Config::default()
    }
}
