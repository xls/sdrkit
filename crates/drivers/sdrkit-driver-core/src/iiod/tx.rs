//! The AD936x transmitter over iiod, behind [`crate::Transmitter`]: TX1, TX2, or both outputs at once. It has its own control
//! connection and, while an output is on air, one stream connection for the transmit device. A worker thread feeds that
//! stream with whole buffers that carry every output of the layout. Open, the start of a transmission and its end write
//! the largest attenuation to every attenuator of the board; only an output on air is unmuted. Open leaves the board as
//! it is while another client transmits on it. One stream has the samples in the scan channels of one output and its
//! attenuation in every attenuator of the board: a driver cannot know which attenuator of a board those channels run
//! through.

use super::context::Device;
use super::proto::{IioError, IiodConn, Target};
use super::samples::{mask_of, tx_layout, LayoutError, TxLayout};
use super::{
    buffer_samples, connect_ctl, lock, rf_bandwidth, Address, Board, BoardFacts, PHY, TX_LO,
};
use crate::net::Abort;
use crate::runner::Notices;
use crate::{
    fmt_m, tx_schema, Doing, DriverError, RateLimits, StreamLayout, Transmitter, TxCapabilities,
    TxLimits, TxSetting, TxState, TxStats, TX_BANDWIDTH, TX_FREQUENCY, TX_POWER, TX_RATE,
};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Hz, Results, Setting, SettingId, SettingSet, Values};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Reads the limits of the transmitter with `outputs` outputs in use: the range of the transmit LO and of the
/// attenuation from the board (as the power range), the rates, the filter widths and the output of the AD936x. A range
/// that the board does not report has the limits of the AD9364. It only reads.
pub(super) fn read_limits(ctl: &mut IiodConn, phy: &str, outputs: usize) -> TxLimits {
    let (min_freq_hz, max_freq_hz) = ctl
        .read_attr(phy, Target::Channel(true, TX_LO), "frequency_available")
        .ok()
        .and_then(|s| super::parse_range(&s))
        .map(|(a, _, b)| (a, b))
        .unwrap_or((47e6, 6e9));
    let (least, most, step) = ctl
        .read_attr(
            phy,
            Target::Channel(true, "voltage0"),
            "hardwaregain_available",
        )
        .ok()
        .and_then(|s| super::parse_range(&s))
        .map(|(lo, step, hi)| ((-hi).max(0.0), -lo, step.abs().max(0.01)))
        .unwrap_or((0.0, MAX_ATTENUATION, 0.25));
    let power_db = (0.0 - most, 0.0 - least, step);
    let max_rate = if outputs > 1 {
        super::MAX_RATE_2R2T
    } else {
        super::MAX_RATE
    };
    TxLimits {
        min_freq_hz,
        max_freq_hz,
        rates: RateLimits {
            min_hz: super::MIN_RATE,
            max_hz: max_rate,
            ranges: Vec::new(),
            steady_hz: None,
        },
        bandwidth_hz: Some(super::FILTER_WIDTH),
        power_db,
        start_power_db: Some(START_POWER),
        common_rates_hz: super::COMMON_RATES.to_vec(),
        full_output_dbm: FULL_OUTPUT_DBM.to_vec(),
    }
}

/// The power an AD936x transmitter starts with, in dB relative to its full output. Its output at full power is a few
/// milliwatts and the driver knows of no amplifier after it, so at -10 dB a nearby receiver reads it.
pub const START_POWER: f64 = -10.0;

/// The output of an AD936x at full power (0 dB attenuation) in dBm, at carrier frequencies in Hz: typical values of the
/// maximum output power of its data sheet, not checked against the data sheet or measured. An estimate: the balun, the
/// board and the cable of a radio take part of it.
pub const FULL_OUTPUT_DBM: &[(Hz, f64)] = &[(800e6, 8.0), (2.4e9, 7.5), (3.5e9, 6.5), (5.5e9, 6.0)];

/// The kernel buffers the transmit stream keeps queued.
pub const TX_KERNEL_BUFFERS: u32 = 12;
/// Each output's queue holds this many buffers.
const QUEUE_BUFFERS: usize = 4;
/// The time added to the estimate of when the radio has played a burst: the transfer over the network and the start of the
/// radio's own buffer are not in the estimate. A longer carrier of zeros is harmless; a cut burst is not read.
const PLAY_MARGIN: Duration = Duration::from_millis(150);
/// The AD936x's largest transmit attenuation.
pub const MAX_ATTENUATION: f64 = 89.75;
/// The zeros in front of a burst.
const LEAD_IN: Duration = Duration::from_millis(250);
/// A gap shorter than this is not an underrun (the clock estimate's slack).
const UNDERRUN_SLACK: Duration = Duration::from_millis(2);
/// The converter's lowest rate without the FIR: 25 MHz / 12.
const CONVERTER_MIN_RATE: Hz = 25e6 / 12.0;
sdrkit_core::choice_enum! {
    /// The transmit outputs that run ([`TRANSMITTERS`]). They are fixed from open to the next open.
    pub enum Transmitters {
        /// One, the one of [`TX_OUTPUT`].
        One = ("one", "One (the output)"),
        /// Both, each pushed on its own.
        Both = ("both", "Both, as two streams"),
        /// Both, interleaved in one stream.
        Interleaved = ("interleaved", "Both, interleaved in one stream"),
    }
}

sdrkit_core::choice_enum! {
    /// The scan channels that carry the one stream ([`TX_OUTPUT`]).
    pub enum TxOutput {
        /// Those of TX1 (`voltage0`, `voltage1`).
        Tx1 = ("tx1", "TX1: first output, the only one of a Pluto"),
        /// Those of TX2 (`voltage2`, `voltage3`).
        Tx2 = ("tx2", "TX2: second output, on AD9361 boards"),
    }
}

sdrkit_core::choice_enum! {
    /// The transmit port of the AD936x ([`TX_PORT`]).
    pub enum TxPort {
        /// As the radio is.
        Auto = ("auto", "As the radio is set"),
        /// Port A.
        A = ("A", "Port A"),
        /// Port B.
        B = ("B", "Port B"),
    }
}

impl TxPort {
    fn attr(self) -> Option<&'static str> {
        match self {
            TxPort::Auto => None,
            TxPort::A => Some("A"),
            TxPort::B => Some("B"),
        }
    }
}

/// The transmit outputs that run.
pub const TRANSMITTERS: Setting<field::Choice<Transmitters>> =
    Setting::choice("transmitters", "Outputs", Transmitters::One).help(
        "Both outputs at once, on one clock and one TX LO (AD9361 boards: ANTSDR E310, Pluto+). Two streams: each output takes \
         its own samples. Interleaved: one stream of TX1, TX2, TX1... At most 30.72 Msps with both.",
    );
/// The scan channels that carry the one stream.
pub const TX_OUTPUT: Setting<field::Choice<TxOutput>> =
    Setting::choice("txoutput", "Output", TxOutput::Tx1)
        .help(
            "The pair of converter channels that carries the samples if one output runs; the power setting goes to every \
             output. Choose the other one if the antenna is on the other connector and nothing is heard (TX2 needs an \
             AD9361 board: ANTSDR E310, Pluto+).",
        )
        .advanced();
/// The transmit port.
pub const TX_PORT: Setting<field::Choice<TxPort>> =
    Setting::choice("txport", "TX port", TxPort::Auto)
        .help("The AD936x output port (A or B) that the transmitter uses. Both outputs share it.");

sdrkit_core::settings! {
    /// The own transmit settings of the iiod driver. They follow [`TxSetting`].
    pub enum IiodTxSetting {
        /// [`TRANSMITTERS`].
        Transmitters = TRANSMITTERS,
        /// [`TX_OUTPUT`].
        Output = TX_OUTPUT,
        /// [`TX_PORT`].
        Port = TX_PORT,
    }
}

/// The id of an own transmit setting in the transmit schema.
fn tx_id(s: IiodTxSetting) -> SettingId {
    SettingId(TxSetting::SETTINGS.len() as u16 + s.id().0)
}

/// What two transmit outputs of one board share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedPart {
    /// The TX LO: both outputs have one carrier.
    Lo,
    /// The sample clock.
    Clock,
    /// The analogue filter.
    Filter,
    /// The transmit port.
    Port,
}

/// Why the iiod transmitter refused, where [`DriverError`] has no variant. It reaches the host as [`DriverError::Driver`].
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum TxError {
    /// Another client transmits on this board.
    #[error("another client transmits on this board")]
    OtherClient,
    /// Receive and transmit at once need firmware in FDD mode.
    #[error("receive and transmit at once need firmware in FDD mode")]
    NeedsFdd,
    /// The receiver uses the low-rate FIR or CS8, which transmit cannot share.
    #[error("the receiver uses the low-rate FIR or CS8")]
    ReceiverHoldsClock,
    /// While the receiver streams, the transmit rate must be the receive rate.
    #[error(
        "the transmit rate must be the receive rate, {rx_rate_hz} Hz, while the receiver streams"
    )]
    RateOfReceiver {
        /// The receive rate.
        rx_rate_hz: f64,
    },
    /// Transmit runs at the lowest converter rate or above.
    #[error("transmit runs at {min_hz} Hz or above")]
    BelowConverter {
        /// The lowest rate.
        min_hz: f64,
    },
    /// With two outputs, the data port carries half its rate.
    #[error("with two outputs the rate is at most {max_hz} Hz")]
    TwoOutputs {
        /// The highest rate with two outputs.
        max_hz: f64,
    },
    /// The transmit FIR stays on after it was switched off.
    #[error("the transmit FIR stays on")]
    FirStaysOn,
    /// The outputs share a part, and the other output on air uses it in another way.
    #[error("the outputs share the {0:?}")]
    Shared(SharedPart),
    /// The outputs are interleaved in one stream: only TX1 takes samples.
    #[error("the outputs are interleaved: give the samples of both to TX1")]
    Interleaved,
    /// An interleaved cyclic buffer does not hold whole instants, or a cyclic buffer on two outputs differs in length.
    #[error("the cyclic buffer has a length that does not fit")]
    CyclicLength,
    /// A cyclic buffer needs samples.
    #[error("a cyclic buffer needs samples")]
    EmptyCyclic,
    /// The transmit buffer does not carry a channel of the output.
    #[error("the transmit buffer does not fit: {0:?}")]
    Layout(LayoutError),
}

impl From<TxError> for DriverError {
    fn from(e: TxError) -> Self {
        DriverError::driver(e)
    }
}

/// The target state of one output, read from the transmit values.
#[derive(Clone, Debug, PartialEq)]
struct TxTune {
    centre_hz: Hz,
    rate_hz: Hz,
    bandwidth_hz: Hz,
    attenuation_db: f64,
    output: TxOutput,
    port: TxPort,
    transmitters: Transmitters,
}

impl TxTune {
    fn of(v: &Values) -> TxTune {
        TxTune {
            centre_hz: v.get(&TX_FREQUENCY),
            rate_hz: v.get(&TX_RATE),
            bandwidth_hz: v.get(&TX_BANDWIDTH),
            attenuation_db: 0.0 - v.get(&TX_POWER),
            output: v.get(&TX_OUTPUT),
            port: v.get(&TX_PORT),
            transmitters: v.get(&TRANSMITTERS),
        }
    }
}

/// The layout that `transmitters` requires.
fn layout_of(t: Transmitters) -> StreamLayout {
    match t {
        Transmitters::One => StreamLayout::Single,
        Transmitters::Both => StreamLayout::Separate(2),
        Transmitters::Interleaved => StreamLayout::Interleaved(2),
    }
}

/// The scan channels (I, Q) of the transmit device that carry phy channel `chan`.
fn scan_pair(chan: usize) -> (&'static str, &'static str) {
    if chan == 1 {
        ("voltage2", "voltage3")
    } else {
        ("voltage0", "voltage1")
    }
}

fn chan_name(chan: usize) -> &'static str {
    if chan == 1 {
        "voltage1"
    } else {
        "voltage0"
    }
}

fn tx_name(chan: usize) -> String {
    format!("TX{}", chan + 1)
}

#[derive(Debug, Default)]
struct Counters {
    sent: AtomicU64,
    buffers: AtomicU64,
    padded: AtomicU64,
    underruns: AtomicU64,
    underrun_samples: AtomicU64,
    /// When the radio has played every transfer it was given.
    played_by: Mutex<Option<Instant>>,
}

/// One output: its tune state, its state, its queue while a stream runs, its counters.
struct Out {
    tune: TxTune,
    /// The phy channel whose scan channels carry its samples: 0 for TX1, 1 for TX2.
    chan: usize,
    /// The phy channels of the attenuators that take its attenuation: every one of the board for the one stream, else
    /// its own.
    atts: Vec<usize>,
    state: TxState,
    since: Option<Instant>,
    counters: Arc<Counters>,
    /// The worker takes this output's samples (else it sends zeros for it and drops what is queued).
    armed: Arc<AtomicBool>,
    flush: Arc<AtomicBool>,
    prod: Option<rtrb::Producer<Complex32>>,
    /// The samples it repeats while cyclic.
    cyclic: Vec<Complex32>,
    /// The attenuation that every one of its attenuators holds, while it is unmuted.
    written_att: Option<f64>,
}

impl Out {
    fn new(tune: TxTune, chan: usize, atts: Vec<usize>) -> Out {
        Out {
            tune,
            chan,
            atts,
            state: TxState::Disarmed,
            since: None,
            counters: Arc::default(),
            armed: Arc::default(),
            flush: Arc::default(),
            prod: None,
            cyclic: Vec::new(),
            written_att: None,
        }
    }
}

struct Worker {
    stop: Arc<AtomicBool>,
    abort: Arc<Abort>,
    thread: Option<JoinHandle<Result<(), IioError>>>,
}

impl Worker {
    /// Stops the worker after its current transfer (it closes the buffer), shutting the socket if it does not end within 2 s.
    fn stop(mut self) -> Option<IioError> {
        self.stop.store(true, Ordering::Release);
        let t = self.thread.take()?;
        let end = Instant::now() + Duration::from_secs(2);
        while !t.is_finished() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(1));
        }
        if !t.is_finished() {
            self.abort.fire();
        }
        t.join().ok().and_then(|r| r.err())
    }

    fn failed(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| t.is_finished())
    }
}

struct Core {
    ctl: Option<IiodConn>,
    at: Address,
    label: String,
    phy: String,
    dds: Device,
    board: Board,
    notices: Notices,
    caps: TxCapabilities,
    layout: StreamLayout,
    /// The setting that chose the outputs at open.
    open_transmitters: Transmitters,
    outs: Vec<Out>,
    /// The phy channels of every transmit attenuator of the board, those of outputs not in use too.
    attenuators: Vec<usize>,
    ports: Vec<String>,
    closed: bool,
    /// The shared rate while an output is on air.
    rate: Hz,
    buffer: usize,
    mask: Vec<u32>,
    /// The sample layout of each output in the stream.
    lays: Vec<TxLayout>,
    worker: Option<Worker>,
    cyclic_conn: Option<IiodConn>,
    written_lo: Option<Hz>,
    written_bw: Option<Hz>,
    lo_powerdown: bool,
    /// The FIR switches the phy lists that touch transmit: (output channel, channel, attribute).
    fir_switches: Vec<(bool, &'static str, &'static str)>,
}

/// What the transmitter handle and the source share (the source closes it on stop).
pub struct TxShared {
    core: Mutex<Core>,
    closed: AtomicBool,
}

impl TxShared {
    /// Disarms every output and closes.
    pub fn close(&self) {
        lock(&self.core).close();
        self.closed.store(true, Ordering::Release);
    }
    /// Closed already.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// The iiod transmitter.
pub struct IiodTransmitter {
    shared: Arc<TxShared>,
    caps: TxCapabilities,
    layout: StreamLayout,
}

impl IiodTransmitter {
    /// Opens the transmitter of the board at `at` (its own control connection), every output off air, and makes sure that
    /// `values` fit the board. [`TRANSMITTERS`] and [`TX_OUTPUT`] choose the outputs. It writes the largest attenuation
    /// to every attenuator of the board, unless another client transmits on it; a FIR that was left on is switched off.
    ///
    /// # Errors
    ///
    /// [`DriverError::CannotTransmit`] for a board without a transmitter, [`DriverError::Refused`] for a setting that the
    /// board cannot take, else what the board returns.
    pub(crate) fn open(
        at: &Address,
        board: Board,
        values: &Values,
    ) -> Result<IiodTransmitter, DriverError> {
        let label = at.label.as_str();
        if lock(&board).tx_open {
            return Err(DriverError::NotNow {
                state: Doing::Running,
            });
        }
        let (mut ctl, ctx, _) = connect_ctl(at, &Abort::new())?;
        let facts = BoardFacts::from_context(&ctx, label)?;
        let Some(dds) = facts.tx.clone().filter(|_| facts.transmitters > 0) else {
            ctl.shutdown();
            return Err(DriverError::CannotTransmit);
        };
        let tune = TxTune::of(values);
        let layout = layout_of(tune.transmitters);
        if layout.count() > facts.transmitters {
            ctl.shutdown();
            return Err(DriverError::Refused {
                id: tx_id(IiodTxSetting::Transmitters),
            });
        }
        let phy = ctx.device(PHY).map(|d| d.id.clone()).unwrap_or_default();
        let limits = read_limits(&mut ctl, &phy, layout.count());
        let ports = if facts.phy_has("voltage0", true, "rf_port_select_available") {
            ctl.read_attr(
                &phy,
                Target::Channel(true, "voltage0"),
                "rf_port_select_available",
            )
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Ok(m) = ctl.read_attr(&phy, Target::Device, "ensm_mode") {
            lock(&board).fdd = Some(m == "fdd");
        }
        let caps = TxCapabilities {
            name: format!("{} TX", facts.name),
            settings: tx_schema(IiodTxSetting::SETTINGS, &limits),
            limits,
            outputs: (1..=facts.transmitters).map(|i| format!("tx{i}")).collect(),
            cyclic: true,
            full_duplex: lock(&board).fdd.unwrap_or(true),
            buffer_samples: 0,
        };
        let attenuators: Vec<usize> = (0..2)
            .filter(|&c| facts.phy_has(chan_name(c), true, "hardwaregain"))
            .collect();
        let outs = match layout {
            StreamLayout::Single => {
                let chan = usize::from(tune.output == TxOutput::Tx2);
                vec![Out::new(tune.clone(), chan, attenuators.clone())]
            }
            _ => (0..layout.count())
                .map(|k| Out::new(tune.clone(), k, vec![k]))
                .collect(),
        };
        let mut core = Core {
            ctl: Some(ctl),
            at: at.clone(),
            label: label.to_string(),
            phy,
            dds,
            board: board.clone(),
            notices: Notices::default(),
            caps,
            layout,
            open_transmitters: tune.transmitters,
            outs,
            attenuators,
            ports,
            closed: false,
            rate: 0.0,
            buffer: 0,
            mask: Vec::new(),
            lays: Vec::new(),
            worker: None,
            cyclic_conn: None,
            written_lo: None,
            written_bw: None,
            lo_powerdown: facts.phy_has(TX_LO, true, "powerdown"),
            fir_switches: [
                (true, "voltage0", "filter_fir_en"),
                (false, "out", "voltage_filter_fir_en"),
            ]
            .into_iter()
            .filter(|(out, ch, attr)| facts.phy_has(ch, *out, attr))
            .collect(),
        };
        for k in 0..core.outs.len() {
            if let Err(e) = core.check(k, &tune) {
                core.close();
                return Err(e);
            }
        }
        if let Err(e) = core.fir_off() {
            core.close();
            return Err(e);
        }
        // The firmware or an earlier program can have left any attenuation, also on an output that this layout does not
        // use. The outputs of another client that transmits now stay as they are.
        if !core.other_client() {
            if let Err(e) = core.mute_board() {
                core.close();
                return Err(e);
            }
        }
        lock(&board).tx_open = true;
        core.notices.log(format!(
            "PlutoSDR {label}: transmitter open ({}), disarmed",
            (0..core.outs.len())
                .map(|k| core.name(k))
                .collect::<Vec<_>>()
                .join(" and ")
        ));
        let caps = core.caps.clone();
        Ok(IiodTransmitter {
            shared: Arc::new(TxShared {
                core: Mutex::new(core),
                closed: AtomicBool::new(false),
            }),
            caps,
            layout,
        })
    }

    /// The part the source keeps, to close the transmitter on stop.
    pub fn shared(&self) -> Arc<TxShared> {
        self.shared.clone()
    }

    fn core(&self) -> std::sync::MutexGuard<'_, Core> {
        let mut c = lock(&self.shared.core);
        c.reap();
        c
    }
}

fn closed_error() -> DriverError {
    DriverError::NotNow {
        state: Doing::Closed,
    }
}

fn not_armed() -> DriverError {
    DriverError::NotNow {
        state: Doing::Disarmed,
    }
}

fn not_while_transmitting() -> DriverError {
    DriverError::NotNow {
        state: Doing::Transmitting,
    }
}

impl Core {
    fn ctl(&mut self) -> Result<&mut IiodConn, DriverError> {
        self.ctl.as_mut().ok_or_else(closed_error)
    }

    fn write(&mut self, target: Target, attr: &str, value: &str) -> Result<(), DriverError> {
        let phy = self.phy.clone();
        self.ctl()?
            .write_attr(&phy, target, attr, value)
            .map(|_| ())
            .map_err(DriverError::from)
    }

    /// Writes an attribute that setting `id` sets. A refusal of the board is [`DriverError::Refused`] for it.
    fn write_for(
        &mut self,
        id: SettingId,
        target: Target,
        attr: &str,
        value: &str,
    ) -> Result<(), DriverError> {
        let phy = self.phy.clone();
        match self.ctl()?.write_attr(&phy, target, attr, value) {
            Ok(_) => Ok(()),
            Err(IioError::Errno(_)) => Err(DriverError::Refused { id }),
            Err(e) => Err(e.into()),
        }
    }

    /// The phy channel that takes what the outputs share (the filter, the port) for output `k`.
    fn out_target(&self, k: usize) -> Target<'static> {
        Target::Channel(true, chan_name(self.outs[k].chan))
    }

    fn name(&self, k: usize) -> String {
        tx_name(self.outs[k].chan)
    }

    /// Another client has the transmit buffer of the board enabled.
    fn other_client(&mut self) -> bool {
        let dds = self.dds.id.clone();
        self.ctl.as_mut().is_some_and(|c| {
            c.read_attr(&dds, Target::Buffer, "enable")
                .is_ok_and(|v| v == "1")
        })
    }

    /// Writes the gain `text` to every attenuator of output `k`.
    fn write_gain(&mut self, k: usize, text: &str) -> Result<(), DriverError> {
        for chan in self.outs[k].atts.clone() {
            self.write(Target::Channel(true, chan_name(chan)), "hardwaregain", text)?;
        }
        Ok(())
    }

    /// Unmutes output `k`: every one of its attenuators takes `att` dB.
    fn unmute(&mut self, k: usize, att: f64) -> Result<(), DriverError> {
        self.write_gain(k, &gain_text(-att))?;
        self.outs[k].written_att = Some(att);
        Ok(())
    }

    /// Writes the largest attenuation to every attenuator of the board, one write each, those of outputs not in use too.
    /// It tries every one and returns the first error.
    fn mute_board(&mut self) -> Result<(), DriverError> {
        let mute = self.mute_text();
        let mut first = Ok(());
        for chan in self.attenuators.clone() {
            let r = self.write(
                Target::Channel(true, chan_name(chan)),
                "hardwaregain",
                &mute,
            );
            if first.is_ok() {
                first = r;
            }
        }
        for o in &mut self.outs {
            o.written_att = None;
        }
        first
    }

    fn output(&self, k: usize) -> Result<(), DriverError> {
        if self.closed {
            return Err(closed_error());
        }
        if k >= self.outs.len() {
            return Err(crate::no_output(k));
        }
        Ok(())
    }

    fn mute_text(&self) -> String {
        gain_text(-MAX_ATTENUATION.max(-self.caps.limits.power_db.0))
    }

    /// The attenuation on the step of the board. The schema holds it to the range of the board.
    fn attenuation(&self, tune: &TxTune) -> f64 {
        let (_, _, step) = self.caps.limits.power_db;
        (tune.attenuation_db / step).round() * step
    }

    /// The transmit rate `tune` requests: its own, else the receiver's.
    fn wanted_rate(&self, tune: &TxTune) -> Hz {
        if tune.rate_hz > 0.0 {
            tune.rate_hz
        } else {
            lock(&self.board).rx_rate
        }
    }

    /// The samples of a transfer and the rate that plays them: those of the buffer on the air, else those that the
    /// settings of the first output give when it is armed.
    fn transfer(&self) -> (usize, Hz) {
        match self.outs.first() {
            Some(o) if o.state.transmitting() && self.buffer > 0 => (self.buffer, self.rate),
            Some(o) => {
                let rate = self.wanted_rate(&o.tune);
                (buffer_samples(rate), rate)
            }
            None => (0, 0.0),
        }
    }

    fn bandwidth(&self, tune: &TxTune, rate: Hz) -> Hz {
        rf_bandwidth(tune.bandwidth_hz, rate)
    }

    /// Checks that the target state of output `k` fits the board. Returns the phy channel that it requires.
    fn check(&self, k: usize, tune: &TxTune) -> Result<usize, DriverError> {
        if tune.transmitters != self.open_transmitters {
            // The outputs are chosen when the transmitter opens.
            return Err(DriverError::Refused {
                id: tx_id(IiodTxSetting::Transmitters),
            });
        }
        let chan = match self.layout {
            StreamLayout::Single => usize::from(tune.output == TxOutput::Tx2),
            _ => k,
        };
        if chan + 1 > self.caps.outputs.len() {
            return Err(DriverError::Refused {
                id: tx_id(IiodTxSetting::Output),
            });
        }
        if let Some(port) = tune.port.attr() {
            if !self.ports.is_empty() && !self.ports.iter().any(|p| p == port) {
                return Err(DriverError::Refused {
                    id: tx_id(IiodTxSetting::Port),
                });
            }
        }
        if tune.centre_hz.is_nan() || tune.centre_hz <= 0.0 {
            return Err(DriverError::Refused {
                id: TxSetting::Frequency.id(),
            });
        }
        let rate = self.wanted_rate(tune);
        if self.layout.count() > 1 && rate > super::MAX_RATE_2R2T {
            return Err(TxError::TwoOutputs {
                max_hz: super::MAX_RATE_2R2T,
            }
            .into());
        }
        Ok(chan)
    }

    /// Puts the transmit FIR in its known state, off: transmit runs at the converter's rates (2.083 Msps and up), which need no
    /// interpolating filter. A switch that reads on (another program left it so) is written off and read back. If it stays on,
    /// the transmitter is refused. When the receiver uses the low-rate FIR, the shared switch belongs to it, and arm refuses.
    fn fir_off(&mut self) -> Result<(), DriverError> {
        if lock(&self.board).rx_fir {
            return Ok(());
        }
        let phy = self.phy.clone();
        for (out, ch, attr) in self.fir_switches.clone() {
            let target = Target::Channel(out, ch);
            let read = |c: &mut Core| -> Result<String, DriverError> {
                c.ctl()?
                    .read_attr(&phy, target, attr)
                    .map(|v| v.trim().to_string())
                    .map_err(DriverError::from)
            };
            if read(self)? == "0" {
                continue;
            }
            self.write(target, attr, "0")?;
            let now = read(self)?;
            if now != "0" {
                return Err(TxError::FirStaysOn.into());
            }
            self.notices.log(format!(
                "PlutoSDR {}: the FIR was left on ({attr}). It is off now, as transmit needs",
                self.label
            ));
        }
        if let Ok(c) = self
            .ctl()?
            .read_attr(&phy, Target::Device, "filter_fir_config")
        {
            self.notices.log(format!(
                "PlutoSDR {}: FIR configuration: {} (not used: the FIR is off)",
                self.label,
                c.trim()
            ));
        }
        Ok(())
    }

    /// Why output `k` cannot run `tune` beside the outputs on air: they share the TX LO, the rate, the filter and the port.
    fn shared_conflict(&self, k: usize, tune: &TxTune) -> Option<DriverError> {
        let other = (0..self.outs.len()).find(|&j| j != k && self.outs[j].state.transmitting())?;
        let o = &self.outs[other].tune;
        let part = if (tune.centre_hz.round() - o.centre_hz.round()).abs() >= 1.0 {
            // The outputs share one TX LO: move the signal inside the band with the digital offset of the output.
            SharedPart::Lo
        } else if (self.wanted_rate(tune) - self.rate).abs() > 1.0 {
            SharedPart::Clock
        } else if self
            .written_bw
            .is_some_and(|w| w != self.bandwidth(tune, self.rate))
        {
            SharedPart::Filter
        } else if tune.port != o.port {
            SharedPart::Port
        } else {
            return None;
        };
        Some(TxError::Shared(part).into())
    }

    /// Fails when the worker died (the link dropped): every output goes off air.
    fn reap(&mut self) {
        if self.worker.as_ref().is_some_and(|w| w.failed()) {
            let why = self.worker.take().and_then(|w| w.stop());
            self.notices.warn(format!(
                "PlutoSDR {}: the transmit stream stopped ({}); the transmitter is off air",
                self.label,
                why.map_or_else(|| "it ended".to_owned(), |e| e.to_string())
            ));
            self.disarm_all();
        }
    }

    fn configure(&mut self, k: usize, tune: &TxTune) -> Result<String, DriverError> {
        self.output(k)?;
        let chan = self.check(k, tune)?;
        let mut notes = Vec::new();
        let (a, b) = (self.caps.limits.min_freq_hz, self.caps.limits.max_freq_hz);
        if tune.centre_hz < a || tune.centre_hz > b {
            notes.push(format!(
                "this transmitter is specified for {}-{} MHz",
                fmt_m(a),
                fmt_m(b)
            ));
        }
        if self.outs[k].state.transmitting() {
            if chan != self.outs[k].chan || (self.wanted_rate(tune) - self.rate).abs() > 1.0 {
                // A new output or rate needs the output off air.
                return Err(not_while_transmitting());
            }
            if let Some(e) = self.shared_conflict(k, tune) {
                return Err(e);
            }
            self.outs[k].tune = tune.clone();
            self.apply_live(k)?;
        } else {
            self.outs[k].tune = tune.clone();
            self.outs[k].chan = chan;
            self.caps.buffer_samples = self.transfer().0;
        }
        Ok(notes.join("; "))
    }

    /// The carrier, the filter (both shared) and the attenuation of output `k`, where they changed.
    fn apply_live(&mut self, k: usize) -> Result<(), DriverError> {
        let tune = self.outs[k].tune.clone();
        let lo = tune.centre_hz.round();
        if self.written_lo != Some(lo) {
            self.write(Target::Channel(true, TX_LO), "frequency", &format!("{lo}"))?;
            self.written_lo = Some(lo);
        }
        let bw = self.bandwidth(&tune, self.rate);
        if self.written_bw != Some(bw) {
            self.write(self.out_target(k), "rf_bandwidth", &format!("{bw}"))?;
            self.written_bw = Some(bw);
        }
        let att = self.attenuation(&tune);
        if self.outs[k].written_att != Some(att) {
            self.unmute(k, att)?;
        }
        Ok(())
    }

    fn arm(&mut self, k: usize) -> Result<(), DriverError> {
        self.output(k)?;
        if self.outs[k].state.transmitting() {
            return Ok(());
        }
        let tune = self.outs[k].tune.clone();
        self.outs[k].chan = self.check(k, &tune)?;
        if self.outs.iter().any(|o| o.state.transmitting()) {
            if let Some(e) = self.shared_conflict(k, &tune) {
                return Err(e);
            }
            return self.join_transmitting(k);
        }
        // Nothing of ours is on air: the board must be free.
        self.ctl()?;
        if self.other_client() {
            return Err(TxError::OtherClient.into());
        }
        let (rx_streaming, rx_rate, rx_fir, rx_cs8, fdd) = {
            let b = lock(&self.board);
            (b.rx_streaming, b.rx_rate, b.rx_fir, b.rx_cs8, b.fdd)
        };
        if rx_streaming && fdd == Some(false) {
            return Err(TxError::NeedsFdd.into());
        }
        if rx_fir || (rx_streaming && rx_cs8) {
            return Err(TxError::ReceiverHoldsClock.into());
        }
        let rate = self.wanted_rate(&tune);
        if rate.is_nan() || rate <= 0.0 {
            return Err(DriverError::Refused {
                id: TxSetting::Rate.id(),
            });
        }
        if rx_streaming && (rate - rx_rate).abs() > 1.0 {
            return Err(TxError::RateOfReceiver {
                rx_rate_hz: rx_rate,
            }
            .into());
        }
        if rate < CONVERTER_MIN_RATE {
            return Err(TxError::BelowConverter {
                min_hz: CONVERTER_MIN_RATE,
            }
            .into());
        }
        self.fir_off()?;
        let r = self.start_transmitting(k, rate);
        if let Err(e) = &r {
            self.notices.warn(format!(
                "PlutoSDR {}: transmit did not start: {}",
                self.label,
                e.text(&self.caps.settings)
            ));
            self.outs[k].state = TxState::Armed; // so disarm undoes what was done
            self.disarm(k);
        }
        r
    }

    /// Output `k` joins the outputs on air: the stream already carries its channels (as zeros), so it only unmutes.
    fn join_transmitting(&mut self, k: usize) -> Result<(), DriverError> {
        let att = self.attenuation(&self.outs[k].tune);
        let o = &mut self.outs[k];
        o.flush.store(false, Ordering::Release);
        o.armed.store(true, Ordering::Release);
        o.state = TxState::Armed;
        o.since = Some(Instant::now());
        if let Err(e) = self.unmute(k, att) {
            self.disarm(k);
            return Err(e);
        }
        self.notices.log(format!(
            "PlutoSDR {}: {} on air beside the other output, {} dB attenuation",
            self.label,
            self.name(k),
            att
        ));
        Ok(())
    }

    fn start_transmitting(&mut self, k: usize, rate: Hz) -> Result<(), DriverError> {
        let out = self.out_target(k);
        // 0. Every attenuator of the board muted first: another program can have changed any of them since open.
        self.mute_board()?;
        // 1. The DDS tone generators off: only our samples go out.
        let tones: Vec<String> = self
            .dds
            .channels
            .iter()
            .filter(|c| c.output && c.scan.is_none() && c.has("raw"))
            .map(|c| c.id.clone())
            .collect();
        let dds = self.dds.id.clone();
        for id in tones {
            self.ctl()?
                .write_attr(&dds, Target::Channel(true, &id), "raw", "0")
                .map_err(DriverError::from)?;
        }
        // 2. The shared rate, when the receiver does not hold it.
        if !lock(&self.board).rx_streaming {
            self.write(
                Target::Channel(true, "voltage0"),
                "sampling_frequency",
                &format!("{}", rate.round()),
            )?;
        }
        let phy = self.phy.clone();
        self.rate = self
            .ctl()?
            .read_attr(
                &phy,
                Target::Channel(true, "voltage0"),
                "sampling_frequency",
            )
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(rate);
        // 3. Port, 4. filter, 5. carrier, 6. the LO powered up. The outputs share all of them.
        let tune = self.outs[k].tune.clone();
        if let Some(port) = tune.port.attr() {
            self.write_for(tx_id(IiodTxSetting::Port), out, "rf_port_select", port)?;
        }
        let bw = self.bandwidth(&tune, self.rate);
        self.write(out, "rf_bandwidth", &format!("{bw}"))?;
        self.written_bw = Some(bw);
        let lo = tune.centre_hz.round();
        self.write(Target::Channel(true, TX_LO), "frequency", &format!("{lo}"))?;
        self.written_lo = Some(lo);
        if self.lo_powerdown {
            self.write(Target::Channel(true, TX_LO), "powerdown", "0")?;
        }
        // 7. The buffer, with the scan channels of every output of the layout.
        self.buffer = buffer_samples(self.rate);
        self.caps.buffer_samples = self.buffer;
        let pos = |d: &Device, id: &str| d.channel_index(id, true).unwrap_or(0);
        let pairs: Vec<(usize, usize)> = self
            .outs
            .iter()
            .map(|o| {
                let (i, q) = scan_pair(o.chan);
                (pos(&self.dds, i), pos(&self.dds, q))
            })
            .collect();
        let bits: Vec<usize> = pairs.iter().flat_map(|p| [p.0, p.1]).collect();
        self.mask = mask_of(&bits, self.dds.channels.len());
        self.lays = pairs
            .iter()
            .map(|&p| tx_layout(&self.dds, &self.mask, p))
            .collect::<Result<_, _>>()
            .map_err(TxError::Layout)?;
        let o = &mut self.outs[k];
        o.armed.store(true, Ordering::Release);
        o.flush.store(false, Ordering::Release);
        o.state = TxState::Armed;
        self.start_stream()?;
        {
            let mut b = lock(&self.board);
            b.tx_armed = true;
            b.tx_rate = self.rate;
        }
        // 8. Unmuted last: every attenuator of the output.
        let att = self.attenuation(&tune);
        self.unmute(k, att)?;
        self.outs[k].since = Some(Instant::now());
        self.notices.log(format!(
            "PlutoSDR {}: on air at {} MHz, {} Msps, {} dB attenuation, {}",
            self.label,
            fmt_m(lo),
            fmt_m(self.rate),
            att,
            self.name(k)
        ));
        Ok(())
    }

    /// Opens the streaming buffer and starts the worker, with a queue per output (one queue in the interleaved layout).
    fn start_stream(&mut self) -> Result<(), DriverError> {
        let dds = self.dds.id.clone();
        let mut conn = self.stream_conn()?;
        conn.set_buffers_count(&dds, TX_KERNEL_BUFFERS)
            .map_err(DriverError::from)?;
        conn.open_buffer(&dds, self.buffer, &self.mask, false)
            .map_err(DriverError::from)?;
        let interleaved = matches!(self.layout, StreamLayout::Interleaved(_));
        let mut lanes = Vec::new();
        for (k, o) in self.outs.iter_mut().enumerate() {
            o.prod = None;
            if interleaved && k > 0 {
                continue;
            }
            let carried: Vec<usize> = if interleaved {
                (0..self.lays.len()).collect()
            } else {
                vec![k]
            };
            let (prod, cons) = rtrb::RingBuffer::new(self.buffer * QUEUE_BUFFERS * carried.len());
            o.prod = Some(prod);
            lanes.push(Lane {
                cons,
                flush: o.flush.clone(),
                counters: o.counters.clone(),
                outputs: carried,
                idle: true,
                sending: false,
            });
        }
        let armed = self.outs.iter().map(|o| o.armed.clone()).collect();
        let stride = self.lays.first().map_or(4, |l| l.stride);
        self.worker = Some(spawn_worker(
            conn,
            Feed {
                dev: dds,
                buffer: self.buffer,
                stride,
                lays: self.lays.clone(),
                armed,
                rate: self.rate,
            },
            lanes,
        )?);
        Ok(())
    }

    fn stream_conn(&self) -> Result<IiodConn, DriverError> {
        let abort = Abort::new();
        let at = &self.at;
        let mut conn = IiodConn::connect(
            &at.transport,
            &at.host,
            at.port,
            Duration::from_secs(8),
            &abort,
        )?;
        conn.recv_buffer_size(&[4 << 20]);
        conn.timeout(super::CONTEXT_TIMEOUT_MS)?;
        Ok(conn)
    }

    fn close_cyclic(&mut self) {
        if let Some(mut c) = self.cyclic_conn.take() {
            c.set_read_timeout(Duration::from_millis(500));
            let dds = self.dds.id.clone();
            let _ = c.close_buffer(&dds);
            c.shutdown();
        }
    }

    fn stop_worker(&mut self) {
        if let Some(w) = self.worker.take() {
            if let Some(e) = w.stop() {
                self.notices
                    .log(format!("PlutoSDR {}: transmit stream: {e}", self.label));
            }
        }
        for o in &mut self.outs {
            o.prod = None;
        }
    }

    fn disarm_all(&mut self) {
        for k in 0..self.outs.len() {
            self.disarm(k);
        }
    }

    fn disarm(&mut self, k: usize) {
        if k >= self.outs.len() || !self.outs[k].state.transmitting() {
            return;
        }
        // 1. Mute. The last output on air leaves every attenuator of the board muted.
        let last = !(0..self.outs.len()).any(|j| j != k && self.outs[j].state.transmitting());
        let muted = if last {
            self.mute_board()
        } else {
            let mute = self.mute_text();
            self.write_gain(k, &mute)
        };
        if let Err(e) = muted {
            self.notices.warn(format!(
                "PlutoSDR {}: {} did not mute: {e}",
                self.label,
                self.name(k)
            ));
        }
        // 2. Its samples stop: the worker drops what is queued and sends zeros for it.
        let was_cyclic = self.outs[k].state == TxState::Cyclic;
        let o = &mut self.outs[k];
        o.armed.store(false, Ordering::Release);
        o.written_att = None;
        o.state = TxState::Disarmed;
        o.since = None;
        o.cyclic.clear();
        self.notices
            .log(format!("PlutoSDR {}: {} off air", self.label, self.name(k)));
        if self.outs.iter().any(|o| o.state.transmitting()) {
            // The others stay on air. A cyclic buffer is made again without this output.
            if was_cyclic {
                if let Err(e) = self.settle_cyclic() {
                    self.notices.warn(format!(
                        "PlutoSDR {}: the other outputs stopped: {e}",
                        self.label
                    ));
                    self.disarm_all();
                }
            }
            return;
        }
        // 3. The last output: the buffer closes and the LO powers down.
        self.stop_worker();
        self.close_cyclic();
        if self.lo_powerdown {
            if let Err(e) = self.write(Target::Channel(true, TX_LO), "powerdown", "1") {
                self.notices.log(format!(
                    "PlutoSDR {}: the TX LO could not be powered down: {e}",
                    self.label
                ));
            }
        }
        self.written_lo = None;
        self.written_bw = None;
        let mut b = lock(&self.board);
        b.tx_armed = false;
        b.tx_rate = 0.0;
    }

    /// After a change of the cyclic outputs: repeats what the cyclic outputs hold, or streams again when none is left.
    fn settle_cyclic(&mut self) -> Result<(), DriverError> {
        self.close_cyclic();
        if self.outs.iter().any(|o| o.state == TxState::Cyclic) {
            return self.open_cyclic();
        }
        self.start_stream()
    }

    /// Opens the cyclic buffer with every cyclic output's samples (zeros for the others) and writes it once.
    fn open_cyclic(&mut self) -> Result<(), DriverError> {
        let interleaved = matches!(self.layout, StreamLayout::Interleaved(_));
        let n = self.lays.len();
        let mut per: Vec<Vec<Complex32>> = vec![Vec::new(); n];
        if interleaved {
            for (i, s) in self.outs[0].cyclic.iter().enumerate() {
                per[i % n].push(*s);
            }
        } else {
            for (k, o) in self.outs.iter().enumerate() {
                if o.state == TxState::Cyclic {
                    per[k].clone_from(&o.cyclic);
                }
            }
        }
        let len = per.iter().map(Vec::len).max().unwrap_or(0);
        let stride = self.lays.first().map_or(4, |l| l.stride);
        let mut raw = vec![0u8; len * stride];
        for (k, s) in per.iter().enumerate() {
            if self.outs[k].state == TxState::Cyclic
                || (interleaved && self.outs[k].state.transmitting())
            {
                self.lays[k].write(s, &mut raw);
            }
        }
        let dds = self.dds.id.clone();
        let mut conn = self.stream_conn()?;
        conn.open_buffer(&dds, len, &self.mask, true)
            .map_err(DriverError::from)?;
        conn.write_buffer(&dds, &raw).map_err(DriverError::from)?;
        for o in self.outs.iter().filter(|o| o.state == TxState::Cyclic) {
            o.counters.sent.fetch_add(len as u64, Ordering::Relaxed);
            o.counters.buffers.fetch_add(1, Ordering::Relaxed);
        }
        self.cyclic_conn = Some(conn);
        Ok(())
    }

    /// Queues what fits of `iq` for output `k` now, without waiting. Returns how many were taken.
    fn push_once(&mut self, k: usize, iq: &[Complex32]) -> Result<usize, DriverError> {
        self.output(k)?;
        let interleaved = matches!(self.layout, StreamLayout::Interleaved(_));
        if interleaved && k > 0 {
            return Err(TxError::Interleaved.into());
        }
        let state = if interleaved {
            self.outs
                .iter()
                .map(|o| o.state)
                .find(|s| s.transmitting())
                .unwrap_or(TxState::Disarmed)
        } else {
            self.outs[k].state
        };
        match state {
            TxState::Armed if self.worker.is_some() => {}
            TxState::Armed | TxState::Cyclic => {
                return Err(DriverError::NotNow {
                    state: Doing::Cyclic,
                })
            }
            TxState::Disarmed => return Err(not_armed()),
            TxState::Closed => return Err(closed_error()),
        }
        let Some(p) = self.outs[k].prod.as_mut() else {
            return Ok(0);
        };
        let n = iq.len().min(p.slots());
        if n > 0 {
            let mut chunk = p.write_chunk_uninit(n).expect("n slots are free");
            let (a, b) = chunk.as_mut_slices();
            for (d, s) in a.iter_mut().chain(b.iter_mut()).zip(&iq[..n]) {
                d.write(*s);
            }
            // SAFETY: every one of the n slots was just written.
            unsafe { chunk.commit_all() };
        }
        Ok(n)
    }

    fn flush(&mut self, k: usize) -> Result<(), DriverError> {
        self.output(k)?;
        match (&self.outs[k].prod, self.outs[k].state) {
            (Some(_), _) if matches!(self.layout, StreamLayout::Interleaved(_)) => {}
            (Some(_), TxState::Armed) => {}
            _ => return Err(not_armed()),
        }
        self.outs[k].flush.store(true, Ordering::Release);
        Ok(())
    }

    fn play_cyclic(&mut self, k: usize, iq: &[Complex32]) -> Result<(), DriverError> {
        self.output(k)?;
        if !self.outs[k].state.transmitting() {
            return Err(not_armed());
        }
        if iq.is_empty() {
            return Err(TxError::EmptyCyclic.into());
        }
        let n = self.outs.len();
        if let StreamLayout::Interleaved(_) = self.layout {
            if k > 0 {
                return Err(TxError::Interleaved.into());
            }
            if !iq.len().is_multiple_of(n) {
                return Err(TxError::CyclicLength.into());
            }
        }
        for j in (0..n).filter(|&j| j != k) {
            let o = &self.outs[j];
            if o.state == TxState::Armed && self.worker.is_some() {
                // A cyclic buffer repeats every output of the board: the streaming output goes off air first.
                return Err(not_while_transmitting());
            }
            if o.state == TxState::Cyclic && o.cyclic.len() != iq.len() {
                return Err(TxError::CyclicLength.into());
            }
        }
        // The streaming buffer closes first (one buffer per device).
        self.stop_worker();
        self.outs[k].cyclic = iq.to_vec();
        self.outs[k].state = TxState::Cyclic;
        if matches!(self.layout, StreamLayout::Interleaved(_)) {
            for o in self.outs.iter_mut().filter(|o| o.state.transmitting()) {
                o.state = TxState::Cyclic;
            }
        }
        self.settle_cyclic()
    }

    fn stats(&self, k: usize) -> TxStats {
        let Some(o) = self.outs.get(k) else {
            return TxStats::default();
        };
        let c = &o.counters;
        TxStats {
            sent: c.sent.load(Ordering::Relaxed),
            buffers: c.buffers.load(Ordering::Relaxed),
            queued: o
                .prod
                .as_ref()
                .map_or(0, |p| p.buffer().capacity() - p.slots()),
            padded: c.padded.load(Ordering::Relaxed),
            underruns: c.underruns.load(Ordering::Relaxed),
            underrun_samples: c.underrun_samples.load(Ordering::Relaxed),
            transmitting_since: o.since,
        }
    }

    fn state(&self, k: usize) -> TxState {
        match self.outs.get(k) {
            _ if self.closed => TxState::Closed,
            Some(o) => o.state,
            None => TxState::Closed,
        }
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.disarm_all();
        if let Some(mut c) = self.ctl.take() {
            c.shutdown();
        }
        self.closed = true;
        for o in &mut self.outs {
            o.state = TxState::Closed;
        }
        lock(&self.board).tx_open = false;
    }
}

/// `hardwaregain` text for a gain in dB (two decimals, the AD936x steps by 0.25 dB).
fn gain_text(db: f64) -> String {
    let s = format!("{db:.2}");
    if s == "-0.00" {
        "0.00".into()
    } else {
        s
    }
}

/// One queue the worker drains: an output's samples, or (interleaved) the samples of every output.
struct Lane {
    cons: rtrb::Consumer<Complex32>,
    flush: Arc<AtomicBool>,
    counters: Arc<Counters>,
    /// The outputs it carries, in the order of its samples.
    outputs: Vec<usize>,
    /// Not in a burst: nothing sent since the last flush, or since arm.
    idle: bool,
    /// It was in a burst before the transfer that the worker makes now: a late transfer is its underrun.
    sending: bool,
}

/// What the worker needs besides its queues.
struct Feed {
    dev: String,
    buffer: usize,
    stride: usize,
    lays: Vec<TxLayout>,
    /// Per output: it is armed.
    armed: Vec<Arc<AtomicBool>>,
    rate: Hz,
}

/// The buffers of zeros that go out before the first samples of a burst, for buffers that play for `buffer_time`: those
/// of [`LEAD_IN`], two or more.
pub(super) fn lead_in_buffers(buffer_time: Duration) -> usize {
    (LEAD_IN.as_secs_f64() / buffer_time.as_secs_f64())
        .ceil()
        .max(2.0) as usize
}

/// The time that the zeros in front of a burst play, for buffers that play for `buffer_time`: the first sample of
/// the burst leaves the radio this long after its first transfer was ready.
pub(super) fn lead_in(buffer_time: Duration) -> Duration {
    buffer_time * lead_in_buffers(buffer_time) as u32
}

fn spawn_worker(conn: IiodConn, feed: Feed, lanes: Vec<Lane>) -> Result<Worker, DriverError> {
    let stop = Arc::new(AtomicBool::new(false));
    let abort = Abort::new();
    conn.register(&abort);
    let s = stop.clone();
    let thread = std::thread::Builder::new()
        .name("sdrkit-iiod-tx".into())
        .spawn(move || worker(conn, feed, lanes, s))
        .map_err(|_| DriverError::Thread)?;
    Ok(Worker {
        stop,
        abort,
        thread: Some(thread),
    })
}

impl Lane {
    fn armed(&self, feed: &Feed) -> bool {
        self.outputs
            .iter()
            .any(|&k| feed.armed[k].load(Ordering::Acquire))
    }
    /// Samples per transfer: one buffer of each output it carries.
    fn need(&self, feed: &Feed) -> usize {
        feed.buffer * self.outputs.len()
    }
    fn ready(&self, feed: &Feed) -> bool {
        let avail = self.cons.slots();
        avail >= self.need(feed) || (self.flush.load(Ordering::Acquire) && avail > 0)
    }
    /// In a burst: it sent before and was not flushed since, or it has samples queued.
    fn in_burst(&self) -> bool {
        !self.idle || !self.cons.is_empty()
    }
}

/// The worker: one transfer carries a buffer of every output. It goes when an output has a whole buffer (or ends a burst) and
/// every other output in a burst has one too. An output in a burst that lags behind holds the transfer until the radio nears
/// the end of what it was given; then its missing samples go out as zeros and count as its underrun. At the start of a burst,
/// the transfer waits up to one buffer's time for the other armed outputs, and buffers of zeros go out first
/// ([`lead_in_buffers`]) when no output was in a burst.
fn worker(
    mut conn: IiodConn,
    feed: Feed,
    mut lanes: Vec<Lane>,
    stop: Arc<AtomicBool>,
) -> Result<(), IioError> {
    let most = lanes.iter().map(|l| l.need(&feed)).max().unwrap_or(0);
    let mut chunk = vec![Complex32::default(); most];
    let mut per = vec![Complex32::default(); feed.buffer];
    let mut raw = vec![0u8; feed.buffer * feed.stride];
    let buffer_time = Duration::from_secs_f64(feed.buffer as f64 / feed.rate);
    let mut due: Option<Instant> = None;
    let mut ready_since: Option<Instant> = None;
    let result = loop {
        if stop.load(Ordering::Acquire) {
            break Ok(());
        }
        // A disarmed output's queue is dropped.
        for l in &mut lanes {
            if !l.armed(&feed) {
                let n = l.cons.slots();
                if n > 0 {
                    if let Ok(r) = l.cons.read_chunk(n) {
                        r.commit_all();
                    }
                }
                l.idle = true;
                l.flush.store(false, Ordering::Release);
            }
        }
        let now = Instant::now();
        if !lanes.iter().any(|l| l.ready(&feed)) {
            for l in &mut lanes {
                if l.flush.load(Ordering::Acquire) && l.cons.is_empty() {
                    l.flush.store(false, Ordering::Release);
                    l.idle = true;
                }
            }
            ready_since = None;
            std::thread::sleep(Duration::from_micros(200));
            continue;
        }
        let since = *ready_since.get_or_insert(now);
        // At the start of a burst, an armed output without samples yet is waited for too, so outputs started together go
        // out together.
        let mid_burst = lanes.iter().any(|l| !l.idle);
        let lagging = lanes.iter().any(|l| l.in_burst() && !l.ready(&feed))
            || (!mid_burst && lanes.iter().any(|l| l.armed(&feed) && !l.in_burst()));
        if lagging {
            let deadline = match due {
                Some(d) if mid_burst => d.checked_sub(buffer_time).unwrap_or(d),
                _ => since + buffer_time,
            };
            if now < deadline {
                std::thread::sleep(Duration::from_micros(200));
                continue;
            }
        }
        ready_since = None;
        // A burst starts with zeros. The radio plays a buffer as soon as it has it, and the first transfers after a pause
        // are slower than the radio plays: without the zeros the radio runs dry between the first buffers of the burst.
        // The zeros are not samples of an output: no counter has them, and a late one is no underrun.
        let starting = lanes.iter().all(|l| l.idle);
        if starting {
            raw.fill(0);
            let mut failed = None;
            for _ in 0..lead_in_buffers(buffer_time) {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                if let Err(e) = conn.write_buffer(&feed.dev, &raw) {
                    failed = Some(e);
                    break;
                }
                let arrived = Instant::now();
                due = Some(due.map_or(arrived, |d| d.max(arrived)) + buffer_time);
            }
            if let Some(e) = failed {
                break Err(e);
            }
        }
        raw.fill(0);
        for l in &mut lanes {
            l.sending = !l.idle;
            if !l.in_burst() {
                continue;
            }
            let need = l.need(&feed);
            let k = need / feed.buffer;
            let n = l.cons.slots().min(need);
            if let Ok(r) = l.cons.read_chunk(n) {
                let (a, b) = r.as_slices();
                chunk[..a.len()].copy_from_slice(a);
                chunk[a.len()..n].copy_from_slice(b);
                r.commit_all();
            }
            chunk[n..need].fill(Complex32::default());
            let flushing = l.flush.load(Ordering::Acquire);
            let c = &l.counters;
            if n < need {
                let missing = ((need - n) / k) as u64;
                if flushing {
                    c.padded.fetch_add(missing, Ordering::Relaxed);
                } else {
                    c.underruns.fetch_add(1, Ordering::Relaxed);
                    c.underrun_samples.fetch_add(missing, Ordering::Relaxed);
                }
            }
            // The samples go into the channels of each output it carries; a disarmed output keeps zeros.
            for (j, &out) in l.outputs.iter().enumerate() {
                if !feed.armed[out].load(Ordering::Acquire) {
                    continue;
                }
                if k == 1 {
                    feed.lays[out].write(&chunk[..feed.buffer], &mut raw);
                } else {
                    for (t, p) in per.iter_mut().enumerate() {
                        *p = chunk[t * k + j];
                    }
                    feed.lays[out].write(&per, &mut raw);
                }
            }
            c.sent.fetch_add(feed.buffer as u64, Ordering::Relaxed);
            c.buffers.fetch_add(1, Ordering::Relaxed);
            l.idle = n < need && flushing && l.cons.is_empty();
            if l.idle {
                l.flush.store(false, Ordering::Release);
            }
        }
        match conn.write_buffer(&feed.dev, &raw) {
            Ok(_) => {}
            Err(e) => break Err(e),
        }
        // The radio plays a buffer once it has all of it, after what it had before. A buffer that arrives after the radio
        // played the one before left the radio dry for the time between: an underrun of every output that was in a burst
        // before it. The first buffer of a burst follows zeros or a pause, where a dry radio loses nothing.
        let arrived = Instant::now();
        if let Some(gap) = due
            .filter(|d| arrived > *d + UNDERRUN_SLACK)
            .map(|d| arrived - d)
        {
            for l in lanes.iter().filter(|l| l.sending) {
                let c = &l.counters;
                c.underruns.fetch_add(1, Ordering::Relaxed);
                c.underrun_samples.fetch_add(
                    (gap.as_secs_f64() * feed.rate).round() as u64,
                    Ordering::Relaxed,
                );
            }
        }
        let start = due.map_or(arrived, |d| d.max(arrived));
        due = Some(start + buffer_time);
        for l in &lanes {
            *l.counters
                .played_by
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = due;
        }
    };
    conn.set_read_timeout(Duration::from_millis(500));
    if result.is_ok() {
        let _ = conn.close_buffer(&feed.dev);
    }
    conn.shutdown();
    result
}

impl IiodTransmitter {
    fn run<R>(&mut self, f: impl FnOnce(&mut Core) -> R) -> R {
        let (r, caps) = {
            let mut c = self.core();
            (f(&mut c), c.caps.clone())
        };
        self.caps = caps;
        r
    }
}

impl Transmitter for IiodTransmitter {
    fn capabilities(&self) -> &TxCapabilities {
        &self.caps
    }

    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        self.configure_output(0, values)
    }

    fn arm(&mut self) -> Result<(), DriverError> {
        self.arm_output(0)
    }

    fn disarm(&mut self) {
        self.disarm_output(0);
    }

    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError> {
        self.push_output(0, iq, wait)
    }

    fn flush(&mut self) -> Result<(), DriverError> {
        self.flush_output(0)
    }

    /// The time that the zeros in front of a burst play ([`lead_in`]), at the rate on the air or, off the air, at
    /// the rate of the settings.
    fn start_latency(&self) -> Duration {
        let (buffer, rate) = self.core().transfer();
        if rate <= 0.0 {
            return Duration::ZERO;
        }
        lead_in(Duration::from_secs_f64(buffer as f64 / rate))
    }

    /// Waits until the worker has passed the end of the burst to the radio, then until the radio has played it. The time
    /// the radio needs is an estimate from the rate, so the wait adds [`PLAY_MARGIN`].
    fn drain(&mut self, wait: Duration) -> bool {
        let end = Instant::now() + wait;
        let (flush, counters) = {
            let c = self.core();
            (c.outs[0].flush.clone(), c.outs[0].counters.clone())
        };
        loop {
            let (queued, failed) = {
                let c = self.core();
                (
                    c.stats(0).queued,
                    c.worker.as_ref().is_none_or(|w| w.failed()),
                )
            };
            if failed {
                return false;
            }
            if queued == 0 && !flush.load(Ordering::Acquire) {
                break;
            }
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let played = *counters
            .played_by
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(at) = played.map(|t| t + PLAY_MARGIN) else {
            return true;
        };
        if at > end {
            std::thread::sleep(end.saturating_duration_since(Instant::now()));
            return false;
        }
        std::thread::sleep(at.saturating_duration_since(Instant::now()));
        true
    }

    fn play_cyclic(&mut self, iq: &[Complex32]) -> Result<(), DriverError> {
        self.play_cyclic_output(0, iq)
    }

    fn state(&self) -> TxState {
        self.state_output(0)
    }

    fn stats(&self) -> TxStats {
        self.stats_output(0)
    }

    fn poll(&mut self, results: &mut Results) {
        self.core().notices.drain_into(results);
    }

    fn close(&mut self) {
        self.shared.close();
    }

    fn layout(&self) -> StreamLayout {
        self.layout
    }

    fn configure_output(&mut self, output: usize, values: &Values) -> Result<(), DriverError> {
        let tune = TxTune::of(values);
        let note = self.run(|c| c.configure(output, &tune))?;
        if !note.is_empty() {
            self.core().notices.log(note);
        }
        Ok(())
    }

    fn arm_output(&mut self, output: usize) -> Result<(), DriverError> {
        self.run(|c| c.arm(output))
    }

    fn disarm_output(&mut self, output: usize) {
        self.core().disarm(output);
    }

    fn push_output(
        &mut self,
        output: usize,
        iq: &[Complex32],
        wait: Duration,
    ) -> Result<usize, DriverError> {
        // The core is locked only while samples go into the queue, never while waiting for room, so the other output and a
        // disarm are never held up.
        let end = Instant::now() + wait;
        let mut done = 0;
        loop {
            let (n, failed) = {
                let mut c = self.core();
                let n = c.push_once(output, &iq[done..])?;
                (n, c.worker.as_ref().is_none_or(|w| w.failed()))
            };
            done += n;
            if done == iq.len() || Instant::now() >= end || failed {
                return Ok(done);
            }
            std::thread::sleep(Duration::from_micros(300));
        }
    }

    fn flush_output(&mut self, output: usize) -> Result<(), DriverError> {
        self.core().flush(output)
    }

    fn play_cyclic_output(&mut self, output: usize, iq: &[Complex32]) -> Result<(), DriverError> {
        self.run(|c| c.play_cyclic(output, iq))
    }

    fn state_output(&self, output: usize) -> TxState {
        self.core().state(output)
    }

    fn stats_output(&self, output: usize) -> TxStats {
        self.core().stats(output)
    }
}

impl Drop for IiodTransmitter {
    fn drop(&mut self) {
        self.shared.close();
    }
}
