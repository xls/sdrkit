//! The sender of `dvbs` (`dvbs.transmit`): a DVB-S or DVB-S2 signal (setting `tx_standard`) of the transport stream that
//! the host feeds, with the same commands, queue and result fields for both. The library has no video encoder; the sender
//! codes the packets as they come. This module holds the component and the DVB-S chain; the DVB-S2 chain is
//! [`crate::dvbs2_transmit`].
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session's rate
//! for `tx.sink`, the carrier centred at 0 Hz on the dial. Output `keyed` is the keyed state ([`KeyedOut`]). Each pass
//! writes as many samples as the pass brings in.
//!
//! `append` adds whole 188-byte packets to a queue of
//! [`QUEUE_PACKETS`](sdrkit_proto_mpegts::sendqueue::QUEUE_PACKETS) packets ([`SendQueue`]); a full queue refuses, so the
//! host feeds at the useful bit rate, which the results state. `start` builds the chain of the `tx_*` settings: for
//! DVB-S the [`Chain`], which sends from the start of a group of eight packets; each packet slot takes the next packet of
//! the queue, or a null packet. Its coding is [`S1Encoder`], the pulses root raised cosine with roll-off 0.35 at the
//! session's rate ([`TxShaper`]), the rms [`RMS`] of full scale. `stop` ends after the group of eight packets on air, or
//! after the PLFRAME on air. The signal rises and falls in [`RISE_S`] ([`Ramp`]) and passes the soft limiter
//! [`sdrkit_components::tx::soft_limit`]. Every transmission ends with an end mark, also when the transmitter goes off or
//! the format changes. On a live radio whose host times bursts (`HostStreams::transmit_lead`), the first sample states
//! its time on air (`TagKind::BurstStart`).
//!
//! Standard: ETSI EN 300 421; ETSI EN 302 307-1.

use crate::dvbs2_transmit::{S2Chain, S2Config};
use crate::typed::{DvbsTxFields, TX_APPEND, TX_START, TX_STOP};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, Ramp};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use sdrkit_proto_dvbs2::inner::PunctureRate;
use sdrkit_proto_dvbs2::s1encode::{useful_bit_rate, S1Encoder};
use sdrkit_proto_dvbs2::settings::{
    GenStandardChoice, TX_FRAME, TX_INVERSION, TX_MODCOD, TX_PILOTS, TX_PL_SCRAMBLING, TX_RATE,
    TX_ROLLOFF, TX_STANDARD, TX_SYMBOL_RATE,
};
use sdrkit_proto_dvbs2::transmit::TxShaper;
use sdrkit_proto_mpegts::outer::{GROUP, TS};
use sdrkit_proto_mpegts::sendqueue::{QueueError, SendQueue, TxAppendArgs};
use sdrkit_proto_mpegts::source::PacketSource;

/// The name of the component.
pub const ID: &str = "dvbs.transmit";
/// The roll-off of DVB-S (EN 300 421 clause 4.5).
pub const ROLL_OFF: f64 = 0.35;
/// The session's rate over the symbol rate that the sender needs at least: the occupied band (1 + roll-off) with a margin
/// of 1.3.
pub const MIN_OVERSAMPLING: f64 = (1.0 + ROLL_OFF) * 1.3;
/// The rms magnitude of the signal, as a fraction of full scale.
pub const RMS: f32 = 0.55;
/// The rise at the start and the fall at the end (raised cosine), seconds.
pub const RISE_S: f64 = 0.005;

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The symbol rate.
        SymbolRate = TX_SYMBOL_RATE,
        /// The code rate.
        Rate = TX_RATE,
        /// Spectral inversion.
        Inversion = TX_INVERSION,
        /// The standard.
        Standard = TX_STANDARD,
        /// The MODCOD of DVB-S2.
        Modcod = TX_MODCOD,
        /// The FEC frame of DVB-S2.
        Frame = TX_FRAME,
        /// Pilots of DVB-S2.
        Pilots = TX_PILOTS,
        /// The roll-off of DVB-S2.
        RollOff = TX_ROLLOFF,
        /// The PL scrambling code of DVB-S2.
        PlScrambling = TX_PL_SCRAMBLING,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Starts sending.
        Start = TX_START,
        /// Stops sending.
        Stop = TX_STOP,
        /// Adds packets.
        Append = TX_APPEND.command,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 3] = [TX_START.key, TX_STOP.key, TX_APPEND.command.key];

/// The values of the settings of a DVB-S transmission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    /// The symbol rate, symbols per second.
    pub symbol_rate: f64,
    /// The code rate.
    pub rate: PunctureRate,
    /// Spectral inversion: Q negated.
    pub inversion: bool,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            symbol_rate: TX_SYMBOL_RATE.default,
            rate: TX_RATE.default,
            inversion: TX_INVERSION.default,
        }
    }
}

impl TxConfig {
    /// The useful bit rate: the rate of the transport stream.
    pub fn bit_rate(&self) -> f64 {
        useful_bit_rate(self.symbol_rate, self.rate)
    }

    /// The lowest session rate for the symbol rate, Hz.
    pub fn min_rate(&self) -> f64 {
        self.symbol_rate * MIN_OVERSAMPLING
    }
}

/// Why a transmit command was refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TxError {
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The session's rate is below the rate that the symbol rate needs.
    RateTooLow {
        /// The session's rate, Hz.
        rate: f64,
        /// The lowest rate for the symbol rate, Hz.
        needs: f64,
    },
    /// The transmission is ending after `stop`.
    Ending,
    /// DVB-S2 has no short frame at code rate 9/10.
    NoShortFrame,
    /// The packets were refused.
    Queue(QueueError),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::NoInput => f.write_str("the stack has no input yet"),
            TxError::RateTooLow { rate, needs } => write!(
                f,
                "the sample rate {:.3} Msps is below the {:.3} Msps that the symbol rate needs",
                rate / 1e6,
                needs / 1e6
            ),
            TxError::Ending => f.write_str("the transmission is ending: start again after it ends"),
            TxError::NoShortFrame => {
                f.write_str("DVB-S2 has no short frame at code rate 9/10: use normal frames")
            }
            TxError::Queue(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TxError {}

/// One DVB-S transmission: the coder and the pulse shaper at the output rate, with the level, the rise and fall and the
/// limiter.
pub struct Chain {
    cfg: TxConfig,
    coder: S1Encoder,
    shaper: TxShaper,
    syms: Vec<Complex32>,
    packets: u64,
    gain: f32,
    ramp: Ramp,
    stopping: bool,
}

impl Chain {
    /// The chain of `cfg` at `rate` Hz.
    ///
    /// # Errors
    ///
    /// [`TxError::RateTooLow`] when `rate` is below [`TxConfig::min_rate`].
    pub fn new(cfg: &TxConfig, rate: f64) -> Result<Self, TxError> {
        let needs = cfg.min_rate();
        if rate.is_nan() || rate < needs {
            return Err(TxError::RateTooLow { rate, needs });
        }
        let shaper = TxShaper::new(rate, cfg.symbol_rate, ROLL_OFF)
            .map_err(|_| TxError::RateTooLow { rate, needs })?;
        Ok(Chain {
            cfg: *cfg,
            coder: S1Encoder::new(cfg.rate),
            shaper,
            syms: Vec::with_capacity(TS * 16),
            packets: 0,
            gain: RMS,
            ramp: Ramp::of_seconds(RISE_S, rate),
            stopping: false,
        })
    }

    /// The settings of the transmission.
    pub fn config(&self) -> &TxConfig {
        &self.cfg
    }

    /// The fall starts at the start of the next group of eight packets.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Whether `stop` was asked.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Makes the samples of the next packet slot at the output rate into `out` (cleared first), the packet from `queue`.
    /// False when `out` holds the last samples of the transmission.
    pub fn packet(&mut self, queue: &mut SendQueue, out: &mut Vec<Complex32>) -> bool {
        if self.stopping && !self.ramp.falling() && self.packets.is_multiple_of(GROUP as u64) {
            self.ramp.fall();
        }
        let mut p = [0u8; TS];
        queue.next_packet(&mut p);
        self.packets += 1;
        self.coder.packet(&p, &mut self.syms);
        if self.cfg.inversion {
            for s in &mut self.syms {
                *s = s.conj();
            }
        }
        self.shaper.push(&self.syms);
        out.clear();
        out.resize(self.shaper.ready(), Complex32::default());
        let n = self.shaper.pop(out);
        out.truncate(n);
        self.ramp.shape(self.gain, out)
    }
}

/// The chain of a transmission, of the standard that it started with.
enum Coder {
    S(Box<Chain>),
    S2(Box<S2Chain>),
}

impl Coder {
    /// The fall starts at the next boundary of the standard.
    fn stop(&mut self) {
        match self {
            Coder::S(c) => c.stop(),
            Coder::S2(c) => c.stop(),
        }
    }

    /// Whether `stop` was asked.
    fn stopping(&self) -> bool {
        match self {
            Coder::S(c) => c.stopping(),
            Coder::S2(c) => c.stopping(),
        }
    }

    /// The useful bit rate of the transmission.
    fn bit_rate(&self) -> f64 {
        match self {
            Coder::S(c) => c.config().bit_rate(),
            Coder::S2(c) => c.config().bit_rate(),
        }
    }

    /// Makes the next samples at the output rate into `out`: a packet slot of DVB-S, a block of symbols of DVB-S2. False
    /// when `out` holds the last samples of the transmission.
    fn make(&mut self, queue: &mut SendQueue, out: &mut Vec<Complex32>) -> bool {
        match self {
            Coder::S(c) => c.packet(queue, out),
            Coder::S2(c) => c.block(queue, out),
        }
    }
}

/// One transmission on the graph: the chain and the samples that wait for the output.
struct Burst {
    chain: Coder,
    out: BurstOut,
}

/// The sender (`dvbs.transmit`). See the module.
pub struct DvbsTransmit {
    standard: GenStandardChoice,
    cfg: TxConfig,
    s2: S2Config,
    rate: f64,
    dial: f64,
    transmit: bool,
    queue: SendQueue,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    timing: BurstTiming,
    results: Published,
    base: Vec<Complex32>,
}

impl Default for DvbsTransmit {
    fn default() -> Self {
        let cfg = TxConfig::default();
        let mut t = DvbsTransmit {
            standard: TX_STANDARD.default,
            cfg,
            s2: S2Config::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            queue: SendQueue::new(cfg.bit_rate()),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            timing: BurstTiming::default(),
            results: Published::new(DvbsTxFields::SHAPE.fields),
            base: Vec::new(),
        };
        t.publish();
        t
    }
}

impl DvbsTransmit {
    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// The useful bit rate of the transmission under way, else that of the settings.
    fn bit_rate(&self) -> f64 {
        match (&self.burst, self.standard) {
            (Some(b), _) => b.chain.bit_rate(),
            (None, GenStandardChoice::Dvbs) => self.cfg.bit_rate(),
            (None, GenStandardChoice::Dvbs2) => self.s2.bit_rate(),
        }
    }

    /// Starts a transmission with the settings; one under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`TxError`] when the stack has no input, the rate is too low, the transmission
    /// is ending or DVB-S2 has no frame of the settings.
    pub fn start(&mut self) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if let Some(b) = &self.burst {
            return if b.chain.stopping() {
                Err(failed(TxError::Ending))
            } else {
                Ok(())
            };
        }
        if self.rate <= 0.0 {
            return Err(failed(TxError::NoInput));
        }
        let chain = match self.standard {
            GenStandardChoice::Dvbs => Coder::S(Box::new(Chain::new(&self.cfg, self.rate).map_err(failed)?)),
            GenStandardChoice::Dvbs2 => {
                Coder::S2(Box::new(S2Chain::new(&self.s2, self.rate).map_err(failed)?))
            }
        };
        let out = BurstOut::new(self.rate, self.rate, 1.0).map_err(|_| failed(TxError::NoInput))?;
        self.queue.restart(chain.bit_rate());
        self.burst = Some(Burst { chain, out });
        Ok(())
    }

    /// Ends the transmission after the group of eight packets on air (DVB-S) or the PLFRAME on air (DVB-S2).
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.burst {
            b.chain.stop();
        }
    }

    /// Adds the packets of `bytes` to the queue and returns how many it took (null packets are left out).
    ///
    /// # Errors
    ///
    /// [`TxError::Queue`] when the packets are refused.
    pub fn append(&mut self, bytes: &[u8]) -> Result<usize, TxError> {
        self.queue.append(bytes).map_err(TxError::Queue)
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if self.burst.take().is_some() {
            self.end_owed = true;
            self.queue.clear();
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let rate = self.bit_rate();
        let q = &self.queue;
        let f = DvbsTxFields {
            transmitting: self.burst.is_some(),
            tx_freq_hz: self.burst.as_ref().map(|_| Hertz(self.dial)),
            tx_bitrate_bps: rate,
            tx_packets: q.sent as i64,
            tx_nulls: q.nulls as i64,
            tx_queue: q.len() as i64,
            tx_queue_s: Seconds(if rate > 0.0 {
                q.len() as f64 * (TS * 8) as f64 / rate
            } else {
                0.0
            }),
            tx_pcr_restamped: q.restamped as i64,
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark, and the keyed state.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some(b) = &mut self.burst {
            while b.out.wants(room) {
                let more = b.chain.make(&mut self.queue, &mut self.base);
                b.out.push(&self.base);
                if !more {
                    b.out.finish(&mut self.base);
                }
            }
            if b.out.write(io, 0, room, &self.timing) {
                self.burst = None;
                self.queue.clear();
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        if busy || ended || io.tick() {
            self.publish();
        }
    }
}

/// A refusal of the sender as a command error.
fn failed(e: TxError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

impl Component for DvbsTransmit {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            ID,
            vec![Port::iq("in")],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.cut();
        self.keyed.reset();
        self.publish();
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            KeyedOut::format(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit {
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let (c, s2) = (&mut self.cfg, &mut self.s2);
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::SymbolRate => {
                c.symbol_rate = TX_SYMBOL_RATE.read(id, v)?;
                s2.symbol_rate = c.symbol_rate;
            }
            TxSetting::Rate => c.rate = TX_RATE.read(id, v)?,
            TxSetting::Inversion => {
                c.inversion = TX_INVERSION.read(id, v)?;
                s2.inversion = c.inversion;
            }
            TxSetting::Standard => self.standard = TX_STANDARD.read(id, v)?,
            TxSetting::Modcod => s2.modcod = TX_MODCOD.read(id, v)?.modcod(),
            TxSetting::Frame => s2.frame = TX_FRAME.read(id, v)?,
            TxSetting::Pilots => s2.pilots = TX_PILOTS.read(id, v)?,
            TxSetting::RollOff => s2.roll_off = TX_ROLLOFF.read(id, v)?,
            TxSetting::PlScrambling => s2.pl_scrambling = TX_PL_SCRAMBLING.read(id, v)? as u32,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let (c, s2) = (&self.cfg, &self.s2);
        sdrkit_graph::values_of([
            (TxSetting::SymbolRate, Value::Hz(c.symbol_rate)),
            (TxSetting::Rate, Value::Choice(c.rate.index())),
            (TxSetting::Inversion, Value::Bool(c.inversion)),
            (TxSetting::Standard, Value::Choice(self.standard.index())),
            (TxSetting::Modcod, Value::Choice(u16::from(s2.modcod.id) - 1)),
            (TxSetting::Frame, Value::Choice(s2.frame.index())),
            (TxSetting::Pilots, Value::Bool(s2.pilots)),
            (TxSetting::RollOff, Value::Choice(s2.roll_off.index())),
            (TxSetting::PlScrambling, Value::I64(i64::from(s2.pl_scrambling))),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Start => self.start()?,
            TxCommand::Stop => self.stop(),
            TxCommand::Append => {
                let TxAppendArgs { packets } = TX_APPEND.args(args)?;
                self.append(&packets).map_err(failed)?;
            }
        }
        self.publish();
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// The sender of `cfg` at `rate` for 0.25 s of signal, the queue full of numbered packets (items: output samples; the CPU
/// share is of one core at that rate).
pub fn bench_sender(seconds: f64, cfg: &TxConfig, rate: f64) -> sdrkit_core::BenchResult {
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
    sdrkit_core::bench::measure(seconds, samples, signal_s, || {
        let Ok(mut chain) = Chain::new(cfg, rate) else {
            return;
        };
        let mut queue = SendQueue::new(cfg.bit_rate());
        let _ = queue.append(&bytes);
        let mut made = 0;
        while made < samples {
            chain.packet(&mut queue, &mut out);
            made += out.len();
        }
    })
}

/// The sender of the first benchmark: QPSK 3/4 at 1 Msym/s.
pub const BENCH_1M: TxConfig = TxConfig {
    symbol_rate: 1e6,
    rate: PunctureRate::R34,
    inversion: false,
};
/// The sender of the second benchmark: QPSK 3/4 at 2 Msym/s.
pub const BENCH_2M: TxConfig = TxConfig {
    symbol_rate: 2e6,
    ..BENCH_1M
};
