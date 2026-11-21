//! The sender of `dvbt2` (`dvbt2.transmit`): a DVB-T2 signal of the transport stream that the host feeds, one data PLP in
//! normal mode, T2-base, one transmitter. The library has no video encoder; the sender modulates the packets as they come.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session's rate
//! for `tx.sink`, the channel centred at 0 Hz on the dial. Output `keyed` is the keyed state ([`KeyedOut`]). Each pass
//! writes as many samples as the pass brings in.
//!
//! `append` adds whole 188-byte packets to a queue of
//! [`QUEUE_PACKETS`](sdrkit_proto_mpegts::sendqueue::QUEUE_PACKETS) packets ([`SendQueue`]); a full queue refuses, so the
//! host feeds at the useful bit rate, which the results state. `start` builds the [`Transmitter`] of the `tx_*` settings
//! with the frame of [`plan_frame`] and sends from the start of a T2-frame; each packet slot takes the next packet of the
//! queue, or a null packet. The FEC blocks of a frame are coded while the frame before it is sent, a share with each
//! symbol. `stop` ends after the T2-frame on air. The signal is made at the elementary rate of the channel width and
//! changed to the session's rate. It rises and falls in [`RISE_S`] ([`Ramp`]), has an rms of [`RMS`] of full scale, and
//! passes the soft limiter [`sdrkit_components::tx::soft_limit`]. Every transmission ends with an end mark, also when the
//! transmitter goes off or the format changes. On a live radio whose host times bursts (`HostStreams::transmit_lead`), the
//! first sample states its time on air (`TagKind::BurstStart`).
//!
//! Standard: ETSI EN 302 755 V1.4.1.

use crate::bicm::Coding;
use crate::error::T2Error;
use crate::params::{Bandwidth, Fft, Guard, Modulation, PilotPattern, Profile};
use crate::pilots::FrameShape;
use crate::settings::{
    TX_BANDWIDTH, TX_CELL_ID, TX_EXTENDED, TX_FEC_FRAME, TX_FFT, TX_FRAME_MS, TX_GUARD, TX_L1_MOD,
    TX_MODULATION, TX_NETWORK_ID, TX_PILOTS, TX_RATE, TX_ROTATION, TX_SYSTEM_ID, TX_TI_BLOCKS,
};
use crate::tx::{self, plan_frame, FramePlan, PlpKind, PlpSpec, Transmitter};
use crate::typed::{Dvbt2TxFields, TX_APPEND, TX_START, TX_STOP};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, Ramp};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use sdrkit_proto_mpegts::sendqueue::{QueueError, SendQueue, TxAppendArgs};
use sdrkit_proto_mpegts::source::{NullSource, PacketSource};

/// The name of the component.
pub const ID: &str = "dvbt2.transmit";
/// The rms magnitude of the signal, as a fraction of full scale.
pub const RMS: f32 = 0.25;
/// The rise at the start and the fall at the end (raised cosine), seconds.
pub const RISE_S: f64 = 0.005;
/// The occupied width over the session's rate that the sender takes at most.
pub const MOST_OCCUPIED: f64 = 0.9;

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The channel width.
        Bandwidth = TX_BANDWIDTH,
        /// The FFT size.
        Fft = TX_FFT,
        /// The guard interval.
        Guard = TX_GUARD,
        /// The pilot pattern.
        Pilots = TX_PILOTS,
        /// Extended carriers.
        Extended = TX_EXTENDED,
        /// The constellation.
        Modulation = TX_MODULATION,
        /// The code rate.
        Rate = TX_RATE,
        /// A rotated constellation.
        Rotation = TX_ROTATION,
        /// The FEC frame.
        FecFrame = TX_FEC_FRAME,
        /// The TI-blocks of a T2-frame.
        TiBlocks = TX_TI_BLOCKS,
        /// The longest T2-frame.
        FrameMs = TX_FRAME_MS,
        /// The L1-post constellation.
        L1Mod = TX_L1_MOD,
        /// CELL_ID.
        CellId = TX_CELL_ID,
        /// NETWORK_ID.
        NetworkId = TX_NETWORK_ID,
        /// T2_SYSTEM_ID.
        SystemId = TX_SYSTEM_ID,
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

/// The values of the settings of the sender.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    /// The channel width.
    pub bandwidth: Bandwidth,
    /// The FFT size.
    pub fft: Fft,
    /// The guard interval.
    pub guard: Guard,
    /// The pilot pattern.
    pub pilots: PilotPattern,
    /// Extended carriers.
    pub extended: bool,
    /// The coding and the constellation of the PLP.
    pub coding: Coding,
    /// The TI-blocks of a T2-frame; 0 for no time interleaving.
    pub ti_blocks: u8,
    /// The longest T2-frame, ms.
    pub frame_ms: u16,
    /// The L1-post constellation.
    pub l1_mod: Modulation,
    /// CELL_ID.
    pub cell_id: u16,
    /// NETWORK_ID.
    pub network_id: u16,
    /// T2_SYSTEM_ID.
    pub system_id: u16,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            bandwidth: TX_BANDWIDTH.default,
            fft: TX_FFT.default,
            guard: TX_GUARD.default,
            pilots: TX_PILOTS.default,
            extended: TX_EXTENDED.default,
            coding: Coding {
                frame: TX_FEC_FRAME.default,
                rate: TX_RATE.default.rate(),
                modulation: TX_MODULATION.default.modulation(),
                rotated: TX_ROTATION.default,
            },
            ti_blocks: TX_TI_BLOCKS.default as u8,
            frame_ms: TX_FRAME_MS.default as u16,
            l1_mod: TX_L1_MOD.default.modulation(),
            cell_id: 0,
            network_id: 0,
            system_id: 0,
        }
    }
}

impl TxConfig {
    /// The frame shape, without its data symbols.
    pub fn shape(&self) -> FrameShape {
        FrameShape {
            fft: self.fft,
            extended: self.extended,
            pp: self.pilots,
            guard: self.guard,
            miso: false,
            tr: false,
            l_data: 0,
        }
    }

    /// The T2-frame of the settings: its data symbols, its FEC blocks and the useful bit rate.
    ///
    /// # Errors
    ///
    /// [`T2Error::Rule`] when EN 302 755 does not allow the combination ([`plan_frame`]).
    pub fn plan(&self) -> Result<FramePlan, T2Error> {
        plan_frame(
            self.bandwidth,
            self.shape(),
            self.coding,
            self.l1_mod,
            self.ti_blocks,
            f64::from(self.frame_ms) / 1000.0,
        )
    }

    /// The occupied width of the signal, Hz: (K_total - 1) carrier spacings.
    pub fn occupied_hz(&self) -> f64 {
        let k = self.fft.k_total(self.extended && self.fft.has_extended());
        (k - 1) as f64 * self.bandwidth.rate() / self.fft.size() as f64
    }

    /// The lowest session rate for the settings, Hz.
    pub fn min_rate(&self) -> f64 {
        self.occupied_hz() / MOST_OCCUPIED
    }

    /// The transmitter of the settings with the frame `plan`: one type 1 data PLP with PLP_ID 0, two T2-frames a
    /// super-frame. Its packets come from the caller of [`Transmitter::part`].
    ///
    /// # Errors
    ///
    /// [`T2Error`] when the transmitter refuses the configuration.
    pub fn transmitter(&self, plan: &FramePlan) -> Result<Transmitter, T2Error> {
        Transmitter::new(tx::TxConfig {
            bandwidth: self.bandwidth,
            shape: FrameShape {
                l_data: plan.l_data,
                ..self.shape()
            },
            profile: Profile::Base,
            l1_mod: self.l1_mod,
            num_t2_frames: 2,
            plps: vec![PlpSpec {
                id: 0,
                kind: PlpKind::Type1,
                group: 0,
                coding: self.coding,
                blocks: plan.blocks,
                time_il: plan.time_il,
                source: Box::new(NullSource),
            }],
            sub_slices: 1,
            cell_id: self.cell_id,
            network_id: self.network_id,
            t2_system_id: self.system_id,
            repetition: false,
            post_scrambled: false,
            miso_group2: Complex32::default(),
            fef: None,
        })
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
        /// The lowest rate for the settings, Hz.
        needs: f64,
    },
    /// The transmission is ending after `stop`.
    Ending,
    /// EN 302 755 does not allow the combination of the settings.
    Settings(T2Error),
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
            TxError::Settings(e) => write!(f, "the tx_* settings cannot be sent: {e}"),
            TxError::Queue(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TxError {}

/// One transmission at the elementary rate: the transmitter with the level, the rise and fall and the limiter.
pub struct Chain {
    plan: FramePlan,
    t: Transmitter,
    gain: f32,
    ramp: Ramp,
    stopping: bool,
}

impl Chain {
    /// The chain of `cfg`.
    ///
    /// # Errors
    ///
    /// [`TxError::Settings`] when EN 302 755 does not allow the combination.
    pub fn new(cfg: &TxConfig) -> Result<Self, TxError> {
        let plan = cfg.plan().map_err(TxError::Settings)?;
        let mut t = cfg.transmitter(&plan).map_err(TxError::Settings)?;
        t.code_ahead(true);
        Ok(Chain {
            plan,
            gain: RMS / t.mean_power().sqrt(),
            ramp: Ramp::of_seconds(RISE_S, t.rate()),
            t,
            stopping: false,
        })
    }

    /// The frame of the transmission.
    pub fn plan(&self) -> &FramePlan {
        &self.plan
    }

    /// The transmitter.
    pub fn transmitter(&self) -> &Transmitter {
        &self.t
    }

    /// The fall starts at the start of the next T2-frame.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Whether `stop` was asked.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Makes the next part of the signal (the P1 symbol or one symbol of the frame) at the elementary rate into `out`
    /// (cleared first), the packets from `queue`. False when `out` holds the last samples of the transmission.
    pub fn part(&mut self, queue: &mut SendQueue, out: &mut Vec<Complex32>) -> bool {
        if self.stopping && !self.ramp.falling() && self.t.at_frame_start() {
            self.ramp.fall();
        }
        out.clear();
        let feed: &mut dyn PacketSource = queue;
        if self.t.part(Some(feed), out).is_err() {
            out.clear();
            return false;
        }
        self.ramp.shape(self.gain, out)
    }
}

/// One transmission on the graph: the chain and the samples that wait for the output.
struct Burst {
    chain: Chain,
    out: BurstOut,
}

/// The sender (`dvbt2.transmit`). See the module.
pub struct Dvbt2Transmit {
    cfg: TxConfig,
    plan: Result<FramePlan, T2Error>,
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

impl Default for Dvbt2Transmit {
    fn default() -> Self {
        let cfg = TxConfig::default();
        let plan = cfg.plan();
        let mut t = Dvbt2Transmit {
            cfg,
            plan,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            queue: SendQueue::new(plan.map_or(0.0, |p| p.bit_rate)),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            timing: BurstTiming::default(),
            results: Published::new(Dvbt2TxFields::SHAPE.fields),
            base: Vec::new(),
        };
        t.publish();
        t
    }
}

impl Dvbt2Transmit {
    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// Starts a transmission with the settings; one under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`TxError`] when the stack has no input, the rate is too low, the settings cannot
    /// be sent or the transmission is ending.
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
        let needs = self.cfg.min_rate();
        if self.rate.is_nan() || self.rate < needs {
            return Err(failed(TxError::RateTooLow {
                rate: self.rate,
                needs,
            }));
        }
        let chain = Chain::new(&self.cfg).map_err(failed)?;
        // The rate change is flat over the occupied band.
        let natural = self.cfg.bandwidth.rate();
        // A session at the elementary rate to within 1 ppm takes the samples as they are.
        let to = if (self.rate - natural).abs() <= 1e-6 * natural {
            natural
        } else {
            self.rate
        };
        let pass = (0.51 * self.cfg.occupied_hz() / natural.min(to)).clamp(0.3, 0.47);
        let out = BurstOut::with_quality(natural, to, pass, 80.0, 1.0)
            .map_err(|_| failed(TxError::NoInput))?;
        self.queue.restart(chain.plan().bit_rate);
        self.burst = Some(Burst { chain, out });
        Ok(())
    }

    /// Ends the transmission after the T2-frame on air.
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

    /// Writes the result fields: the frame of the transmission under way, else that of the settings.
    fn publish(&mut self) {
        let plan = self
            .burst
            .as_ref()
            .map_or(self.plan, |b| Ok(*b.chain.plan()));
        let rate = plan.map_or(0.0, |p| p.bit_rate);
        let q = &self.queue;
        let problem = plan.err().map(|e| e.to_string());
        let f = Dvbt2TxFields {
            transmitting: self.burst.is_some(),
            tx_freq_hz: self.burst.as_ref().map(|_| Hertz(self.dial)),
            tx_bitrate_bps: rate,
            tx_packets: q.sent as i64,
            tx_nulls: q.nulls as i64,
            tx_queue: q.len() as i64,
            tx_queue_s: Seconds(if rate > 0.0 {
                q.len() as f64 * 1504.0 / rate
            } else {
                0.0
            }),
            tx_pcr_restamped: q.restamped as i64,
            tx_frame_s: plan.ok().map(|p| Seconds(p.frame_s)),
            tx_fec_blocks: plan.ok().map(|p| i64::from(p.blocks)),
            tx_problem: problem.as_deref(),
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
                let more = b.chain.part(&mut self.queue, &mut self.base);
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

/// The index of the choice of `C` that `is` accepts, else 0.
fn choice_index<C: Choice>(is: impl Fn(C) -> bool) -> u16 {
    (0..C::CHOICES.len() as u16)
        .find(|&i| C::from_index(i).is_some_and(&is))
        .unwrap_or(0)
}

/// The 16-bit value of setting `id` in `v`.
fn id16(
    s: &sdrkit_core::Setting<sdrkit_core::publish::field::I64>,
    id: SettingId,
    v: &Value,
) -> Result<u16, SettingError> {
    u16::try_from(s.read(id, v)?).map_err(|_| SettingError::Malformed { id, at: None })
}

impl Component for Dvbt2Transmit {
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
            TxSetting::Fft => c.fft = TX_FFT.read(id, v)?,
            TxSetting::Guard => c.guard = TX_GUARD.read(id, v)?,
            TxSetting::Pilots => c.pilots = TX_PILOTS.read(id, v)?,
            TxSetting::Extended => c.extended = TX_EXTENDED.read(id, v)?,
            TxSetting::Modulation => c.coding.modulation = TX_MODULATION.read(id, v)?.modulation(),
            TxSetting::Rate => c.coding.rate = TX_RATE.read(id, v)?.rate(),
            TxSetting::Rotation => c.coding.rotated = TX_ROTATION.read(id, v)?,
            TxSetting::FecFrame => c.coding.frame = TX_FEC_FRAME.read(id, v)?,
            TxSetting::TiBlocks => {
                c.ti_blocks = u8::try_from(TX_TI_BLOCKS.read(id, v)?)
                    .map_err(|_| SettingError::Malformed { id, at: None })?;
            }
            TxSetting::FrameMs => c.frame_ms = id16(&TX_FRAME_MS, id, v)?,
            TxSetting::L1Mod => c.l1_mod = TX_L1_MOD.read(id, v)?.modulation(),
            TxSetting::CellId => c.cell_id = id16(&TX_CELL_ID, id, v)?,
            TxSetting::NetworkId => c.network_id = id16(&TX_NETWORK_ID, id, v)?,
            TxSetting::SystemId => c.system_id = id16(&TX_SYSTEM_ID, id, v)?,
        }
        self.plan = self.cfg.plan();
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        use crate::settings::{L1Modulation, PlpModulation, TxRate};
        let c = &self.cfg;
        let modulation = choice_index(|x: PlpModulation| x.modulation() == c.coding.modulation);
        let rate = choice_index(|x: TxRate| x.rate() == c.coding.rate);
        let l1 = choice_index(|x: L1Modulation| x.modulation() == c.l1_mod);
        sdrkit_graph::values_of([
            (TxSetting::Bandwidth, Value::Choice(c.bandwidth.index())),
            (TxSetting::Fft, Value::Choice(c.fft.index())),
            (TxSetting::Guard, Value::Choice(c.guard.index())),
            (TxSetting::Pilots, Value::Choice(c.pilots.index())),
            (TxSetting::Extended, Value::Bool(c.extended)),
            (TxSetting::Modulation, Value::Choice(modulation)),
            (TxSetting::Rate, Value::Choice(rate)),
            (TxSetting::Rotation, Value::Bool(c.coding.rotated)),
            (TxSetting::FecFrame, Value::Choice(c.coding.frame.index())),
            (TxSetting::TiBlocks, Value::I64(i64::from(c.ti_blocks))),
            (TxSetting::FrameMs, Value::I64(i64::from(c.frame_ms))),
            (TxSetting::L1Mod, Value::Choice(l1)),
            (TxSetting::CellId, Value::I64(i64::from(c.cell_id))),
            (TxSetting::NetworkId, Value::I64(i64::from(c.network_id))),
            (TxSetting::SystemId, Value::I64(i64::from(c.system_id))),
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

/// The part of a T2-frame that takes longest to make.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorstPart {
    /// The time that the part took, seconds: the shortest of the runs, so that other work on the device does not count.
    pub took_s: f64,
    /// The signal time of the part, seconds.
    pub signal_s: f64,
}

/// The sender of `cfg` at the elementary rate, `frames` T2-frames a run after the first frame, numbered packets in the
/// queue (items: output samples; the CPU share is of one core at that rate), and the part of a frame that takes longest.
pub fn bench_sender(
    seconds: f64,
    cfg: &TxConfig,
    frames: usize,
) -> (sdrkit_core::BenchResult, WorstPart) {
    use sdrkit_proto_mpegts::source::NumberedSource;
    let (Ok(plan), Ok(mut chain)) = (cfg.plan(), Chain::new(cfg)) else {
        return Default::default();
    };
    let rate = cfg.bandwidth.rate();
    let samples = frames * plan.frame_samples;
    let mut src = NumberedSource::new(crate::generator::PATTERN);
    let mut queue = SendQueue::new(plan.bit_rate);
    let mut p = [0u8; 188];
    let mut out = Vec::new();
    // The first frame is coded at once; the frames after it are coded while the frame before is made.
    let parts = 1 + chain.transmitter().shape().l_f();
    for _ in 0..parts {
        chain.part(&mut queue, &mut out);
    }
    let mut fastest = vec![(f64::INFINITY, 0usize); parts];
    let r = sdrkit_core::bench::measure(seconds, samples, samples as f64 / rate, || {
        for at in 0..frames * parts {
            while queue.room() > 0 {
                src.next_packet(&mut p);
                let _ = queue.append(&p);
            }
            let t = std::time::Instant::now();
            chain.part(&mut queue, &mut out);
            let took_s = t.elapsed().as_secs_f64();
            let f = &mut fastest[at % parts];
            if took_s < f.0 {
                *f = (took_s, out.len());
            }
        }
    });
    let worst = fastest
        .iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map_or_else(WorstPart::default, |&(took_s, n)| WorstPart {
            took_s,
            signal_s: n as f64 / rate,
        });
    (r, worst)
}

/// The sender of the first benchmark: the defaults (8 MHz, 8K, guard 1/128, PP7, rotated 64-QAM 2/3, 64 800-bit frames).
pub fn bench_default() -> TxConfig {
    TxConfig::default()
}

/// The sender of the second benchmark: 1.7 MHz, 2K, guard 1/8, PP2, rotated QPSK 1/2, 16 200-bit frames.
pub fn bench_narrow() -> TxConfig {
    TxConfig {
        bandwidth: Bandwidth::B1_7,
        fft: Fft::K2,
        guard: Guard::G1_8,
        pilots: PilotPattern::Pp2,
        coding: Coding {
            frame: crate::params::FecFrame::Short,
            rate: crate::params::CodeRate::R1_2,
            modulation: Modulation::Qpsk,
            rotated: true,
        },
        ..TxConfig::default()
    }
}
