//! The sender of `dvbt` (`dvbt.transmit`): a DVB-T signal of the transport stream that the host feeds. The library has no
//! video encoder; the sender modulates the packets as they come.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session's rate
//! for `tx.sink`, the channel centred at 0 Hz on the dial. Output `keyed` is the keyed state ([`KeyedOut`]). Each pass
//! writes as many samples as the pass brings in.
//!
//! `append` and `append_lp` add whole 188-byte packets to a queue of
//! [`QUEUE_PACKETS`](sdrkit_proto_mpegts::sendqueue::QUEUE_PACKETS) packets for each stream ([`SendQueue`]); a
//! full queue refuses, so the host feeds at the useful bit rate, which the results state. `start` builds the
//! [`Modulator`] of the `tx_*` settings and sends from the start of a super-frame; each packet slot takes the next packet
//! of its queue, or a null packet. `stop` ends after the frame on air. The signal rises and falls in [`RISE_S`] ([`Ramp`]),
//! has an rms of [`RMS`] of full scale, and passes the soft limiter [`sdrkit_components::tx::soft_limit`]. Every
//! transmission ends with an end mark, also when the transmitter goes off or the format changes. On a live radio whose
//! host times bursts (`HostStreams::transmit_lead`), the first sample states its time on air (`TagKind::BurstStart`).
//!
//! Standard: ETSI EN 300 744.

use crate::generator::{Modulator, OutRate, MEAN_POWER};
use crate::params::{Bandwidth, CodeRate, Constellation, Guard, Hierarchy, Mode, TpsParams};
use crate::settings::{
    TX_BANDWIDTH, TX_CELL_ID, TX_CONSTELLATION, TX_GUARD, TX_HIERARCHY, TX_MODE, TX_RATE_HP,
    TX_RATE_LP,
};
use crate::typed::{DvbtTxFields, TX_APPEND, TX_APPEND_LP, TX_START, TX_STOP};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, Ramp};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use sdrkit_proto_mpegts::sendqueue::{QueueError, SendQueue, TxAppendArgs};

/// The name of the component.
pub const ID: &str = "dvbt.transmit";
/// The rms magnitude of the signal, as a fraction of full scale.
pub const RMS: f32 = 0.25;
/// The rise at the start and the fall at the end (raised cosine), seconds.
pub const RISE_S: f64 = 0.005;

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The channel width.
        Bandwidth = TX_BANDWIDTH,
        /// The FFT mode.
        Mode = TX_MODE,
        /// The guard interval.
        Guard = TX_GUARD,
        /// The constellation.
        Constellation = TX_CONSTELLATION,
        /// The hierarchy.
        Hierarchy = TX_HIERARCHY,
        /// The code rate.
        RateHp = TX_RATE_HP,
        /// The LP code rate.
        RateLp = TX_RATE_LP,
        /// The cell id.
        CellId = TX_CELL_ID,
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
        /// Adds packets to the low-priority stream.
        AppendLp = TX_APPEND_LP.command,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 4] = [
    TX_START.key,
    TX_STOP.key,
    TX_APPEND.command.key,
    TX_APPEND_LP.command.key,
];

/// The values of the settings of the sender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxConfig {
    /// The channel width.
    pub bandwidth: Bandwidth,
    /// The FFT mode.
    pub mode: Mode,
    /// The guard interval.
    pub guard: Guard,
    /// The constellation.
    pub constellation: Constellation,
    /// The hierarchy.
    pub hierarchy: Hierarchy,
    /// The code rate of the stream, or of the high-priority stream.
    pub rate_hp: CodeRate,
    /// The code rate of the low-priority stream.
    pub rate_lp: CodeRate,
    /// The cell id; 0 for none.
    pub cell_id: u16,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            bandwidth: TX_BANDWIDTH.default,
            mode: TX_MODE.default,
            guard: TX_GUARD.default,
            constellation: TX_CONSTELLATION.default,
            hierarchy: TX_HIERARCHY.default,
            rate_hp: TX_RATE_HP.default,
            rate_lp: TX_RATE_LP.default,
            cell_id: 0,
        }
    }
}

impl TxConfig {
    /// What TPS signals: the length indicator is 31 with a cell id and 23 without.
    pub fn tps(&self) -> TpsParams {
        TpsParams {
            mode: self.mode,
            guard: self.guard,
            constellation: self.constellation,
            hierarchy: self.hierarchy,
            rate_hp: self.rate_hp,
            rate_lp: self.rate_lp,
            cell_id: self.cell_id,
            length: if self.cell_id == 0 { 23 } else { 31 },
            dvbh: 0,
        }
    }

    /// The useful bit rate of the stream (`lp` false) or of the low-priority stream.
    pub fn bit_rate(&self, lp: bool) -> f64 {
        self.tps().net_bitrate(self.bandwidth, lp)
    }
}

/// Why a transmit command was refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TxError {
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The session's rate is below the rate that the channel width needs.
    RateTooLow {
        /// The session's rate, Hz.
        rate: f64,
        /// The lowest rate for the width, Hz.
        needs: f64,
    },
    /// The transmission is ending after `stop`.
    Ending,
    /// `append_lp` in a multiplex that is not hierarchical.
    NotHierarchical,
    /// The packets were refused.
    Queue(QueueError),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::NoInput => f.write_str("the stack has no input yet"),
            TxError::RateTooLow { rate, needs } => write!(
                f,
                "the sample rate {:.3} Msps is below the {:.3} Msps that the channel width needs",
                rate / 1e6,
                needs / 1e6
            ),
            TxError::Ending => f.write_str("the transmission is ending: start again after it ends"),
            TxError::NotHierarchical => {
                f.write_str("the multiplex is not hierarchical: it has no low-priority stream")
            }
            TxError::Queue(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TxError {}

/// One transmission: the modulator at the output rate, with the level, the rise and fall and the limiter.
pub struct Chain {
    m: Modulator,
    out: OutRate,
    gain: f32,
    ramp: Ramp,
    stopping: bool,
}

impl Chain {
    /// The chain of `cfg` at `rate` Hz.
    ///
    /// # Errors
    ///
    /// [`TxError::RateTooLow`] when `rate` is below the lowest rate of the channel width.
    pub fn new(cfg: &TxConfig, rate: f64) -> Result<Self, TxError> {
        let needs = cfg.bandwidth.min_rate();
        if rate.is_nan() || rate < needs {
            return Err(TxError::RateTooLow { rate, needs });
        }
        let m = Modulator::new(cfg.bandwidth, cfg.tps(), 0, 0, None);
        let out = OutRate::new(m.rate(), cfg.mode, rate)
            .map_err(|_| TxError::RateTooLow { rate, needs })?;
        Ok(Chain {
            m,
            out,
            gain: RMS / (MEAN_POWER as f32).sqrt(),
            ramp: Ramp::of_seconds(RISE_S, rate),
            stopping: false,
        })
    }

    /// What TPS signals.
    pub fn tps(&self) -> &TpsParams {
        &self.m.tps
    }

    /// The fall starts at the next frame boundary.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Whether `stop` was asked.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Makes the next symbol at the output rate into `out` (cleared first), the packets from `hp` and `lp`. False when
    /// `out` holds the last samples of the transmission.
    pub fn symbol(&mut self, hp: &mut SendQueue, lp: &mut SendQueue, out: &mut Vec<Complex32>) -> bool {
        if self.stopping && !self.ramp.falling() && self.m.position().0 == 0 {
            self.ramp.fall();
        }
        let s = self.m.next_symbol(hp, lp);
        self.out.push(s);
        out.clear();
        self.out.take_all(out);
        self.ramp.shape(self.gain, out)
    }
}

/// One transmission on the graph: the chain and the samples that wait for the output.
struct Burst {
    chain: Chain,
    out: BurstOut,
}

/// The sender (`dvbt.transmit`). See the module.
pub struct DvbtTransmit {
    cfg: TxConfig,
    rate: f64,
    dial: f64,
    transmit: bool,
    hp: SendQueue,
    lp: SendQueue,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    timing: BurstTiming,
    results: Published,
    base: Vec<Complex32>,
}

impl Default for DvbtTransmit {
    fn default() -> Self {
        let cfg = TxConfig::default();
        let mut t = DvbtTransmit {
            cfg,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            hp: SendQueue::new(cfg.bit_rate(false)),
            lp: SendQueue::new(cfg.bit_rate(true)),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            timing: BurstTiming::default(),
            results: Published::new(DvbtTxFields::SHAPE.fields),
            base: Vec::new(),
        };
        t.publish();
        t
    }
}

impl DvbtTransmit {
    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// The parameters of the transmission under way, else those of the settings.
    fn tps(&self) -> TpsParams {
        self.burst
            .as_ref()
            .map_or_else(|| self.cfg.tps(), |b| *b.chain.tps())
    }

    /// Starts a transmission with the settings; one under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`TxError`] when the stack has no input, the rate is too low or the
    /// transmission is ending.
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
        let chain = Chain::new(&self.cfg, self.rate).map_err(failed)?;
        let out = BurstOut::new(self.rate, self.rate, 1.0).map_err(|_| failed(TxError::NoInput))?;
        self.hp.restart(self.cfg.bit_rate(false));
        self.lp.restart(self.cfg.bit_rate(true));
        self.burst = Some(Burst { chain, out });
        Ok(())
    }

    /// Ends the transmission after the frame on air.
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.burst {
            b.chain.stop();
        }
    }

    /// Adds the packets of `bytes` to the stream, or to the low-priority stream with `lp`.
    ///
    /// # Errors
    ///
    /// [`TxError::Queue`] when the packets are refused, [`TxError::NotHierarchical`] for `lp` in a multiplex that is not
    /// hierarchical.
    pub fn append(&mut self, bytes: &[u8], lp: bool) -> Result<usize, TxError> {
        if lp && !self.tps().hierarchy.hierarchical() {
            return Err(TxError::NotHierarchical);
        }
        let feed = if lp { &mut self.lp } else { &mut self.hp };
        feed.append(bytes).map_err(TxError::Queue)
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if self.burst.take().is_some() {
            self.end_owed = true;
            self.hp.clear();
            self.lp.clear();
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let tps = self.tps();
        let bw = self
            .burst
            .as_ref()
            .map_or(self.cfg.bandwidth, |b| b.chain.m.bandwidth);
        let rate = tps.net_bitrate(bw, false);
        let hier = tps.hierarchy.hierarchical();
        let f = DvbtTxFields {
            transmitting: self.burst.is_some(),
            tx_freq_hz: self.burst.as_ref().map(|_| Hertz(self.dial)),
            tx_bitrate_bps: rate,
            tx_bitrate_lp_bps: hier.then(|| tps.net_bitrate(bw, true)),
            tx_packets: (self.hp.sent + self.lp.sent) as i64,
            tx_nulls: (self.hp.nulls + if hier { self.lp.nulls } else { 0 }) as i64,
            tx_queue: self.hp.len() as i64,
            tx_queue_lp: self.lp.len() as i64,
            tx_queue_s: Seconds(self.hp.len() as f64 * 1504.0 / rate),
            tx_pcr_restamped: (self.hp.restamped + self.lp.restamped) as i64,
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
                let more = b.chain.symbol(&mut self.hp, &mut self.lp, &mut self.base);
                b.out.push(&self.base);
                if !more {
                    b.out.finish(&mut self.base);
                }
            }
            if b.out.write(io, 0, room, &self.timing) {
                self.burst = None;
                self.hp.clear();
                self.lp.clear();
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

impl Component for DvbtTransmit {
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
        let c = &mut self.cfg;
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Bandwidth => c.bandwidth = TX_BANDWIDTH.read(id, v)?,
            TxSetting::Mode => c.mode = TX_MODE.read(id, v)?,
            TxSetting::Guard => c.guard = TX_GUARD.read(id, v)?,
            TxSetting::Constellation => c.constellation = TX_CONSTELLATION.read(id, v)?,
            TxSetting::Hierarchy => c.hierarchy = TX_HIERARCHY.read(id, v)?,
            TxSetting::RateHp => c.rate_hp = TX_RATE_HP.read(id, v)?,
            TxSetting::RateLp => c.rate_lp = TX_RATE_LP.read(id, v)?,
            TxSetting::CellId => {
                c.cell_id = u16::try_from(TX_CELL_ID.read(id, v)?)
                    .map_err(|_| SettingError::Malformed { id, at: None })?;
            }
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let c = &self.cfg;
        sdrkit_graph::values_of([
            (TxSetting::Bandwidth, Value::Choice(c.bandwidth.index())),
            (TxSetting::Mode, Value::Choice(c.mode.index())),
            (TxSetting::Guard, Value::Choice(c.guard.index())),
            (
                TxSetting::Constellation,
                Value::Choice(c.constellation.index()),
            ),
            (TxSetting::Hierarchy, Value::Choice(c.hierarchy.index())),
            (TxSetting::RateHp, Value::Choice(c.rate_hp.index())),
            (TxSetting::RateLp, Value::Choice(c.rate_lp.index())),
            (TxSetting::CellId, Value::I64(i64::from(c.cell_id))),
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
                self.append(&packets, false).map_err(failed)?;
            }
            TxCommand::AppendLp => {
                let TxAppendArgs { packets } = TX_APPEND_LP.args(args)?;
                self.append(&packets, true).map_err(failed)?;
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
