//! The test signal parts of the TV generators: the clean pulse shaper, the payload and the test signal settings.

use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::SimError;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Setting, SignalInfo};
use sdrkit_proto_mpegts::source::{Numbered, NumberedSource, PacketSource, TestMux};
use std::collections::VecDeque;

/// The pulse span of the generators, symbols (12 each side of the centre).
pub const SPAN: f64 = 24.0;

/// The clean back end of every TV generator: root raised cosine pulses at any rate, at unit mean power, with a queue
/// of finished samples. Impairments (noise, offsets, echoes, inversion, level) come from the framework's `gen_*` settings.
#[derive(Clone, Debug)]
pub struct TxShaper {
    m: Linear,
    queue: VecDeque<Complex32>,
    tmp: Vec<Complex32>,
}

impl TxShaper {
    /// A shaper from symbols at `rs` per second to samples at `fs` per second with roll-off `alpha`.
    ///
    /// # Errors
    ///
    /// The [`SimError`] of the pulse if a rate is not positive or the roll-off is not in 0 to 1.
    pub fn new(fs: f64, rs: f64, alpha: f64) -> Result<TxShaper, SimError> {
        let m = Linear::rrc(alpha, SPAN, fs / rs)?;
        Ok(TxShaper {
            m,
            queue: VecDeque::new(),
            tmp: Vec::new(),
        })
    }
    /// Samples ready.
    pub fn ready(&self) -> usize {
        self.queue.len()
    }
    /// Shapes symbols of unit mean power and queues the samples.
    pub fn push(&mut self, syms: &[Complex32]) {
        self.tmp.clear();
        self.m.push(syms, &mut self.tmp);
        self.queue.extend(self.tmp.iter().copied());
    }
    /// Moves up to `out.len()` samples out and returns how many it moved.
    pub fn pop(&mut self, out: &mut [Complex32]) -> usize {
        let n = out.len().min(self.queue.len());
        for (o, s) in out.iter_mut().zip(self.queue.drain(..n)) {
            *o = s;
        }
        n
    }
}

/// What a TV generator sends: the test multiplex (PSI, SI and the numbered packets of
/// [`Numbered::SATELLITE_CABLE`]) or the numbered packets alone (as in the recordings).
pub fn payload(numbered_only: bool) -> Box<dyn PacketSource> {
    if numbered_only {
        return Box::new(NumberedSource::new(Numbered::SATELLITE_CABLE));
    }
    match TestMux::satellite_cable(100) {
        Ok(m) => Box::new(m),
        Err(_) => Box::new(NumberedSource::new(Numbered::SATELLITE_CABLE)),
    }
}

sdrkit_core::choice_enum! {
    /// What the test signal of a TV stack carries.
    pub enum Payload {
        /// The test multiplex.
        Programme = ("programme", "Test multiplex"),
        /// The numbered packets alone.
        Numbered = ("numbered", "Numbered packets"),
    }
}

/// The payload of the test signal.
pub const GEN_PAYLOAD: Setting<field::Choice<Payload>> =
    Setting::choice("gen_payload", "Payload", Payload::Programme)
        .section(TEST_SIGNAL)
        .advanced();

/// The section of the test signal settings.
pub const TEST_SIGNAL: &str = "Test signal";

/// The signal of a generator: a linear carrier of `rs` symbols per second, `alpha` roll-off, `bits` information bits per second.
pub fn signal_info(rs: f64, alpha: f64, bits: f64) -> SignalInfo {
    SignalInfo {
        bandwidth_hz: rs * (1.0 + alpha),
        symbol_rate_hz: Some(rs),
        bit_rate_hz: Some(bits),
        power: 1.0,
    }
}
