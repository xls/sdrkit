//! Radios: the contract every driver implements, the ring that carries native samples to the host, the owner form of a kind
//! of radio and of an open radio, and the drivers of this crate (test signal, file, rtl_tcp, iiod).

#![warn(missing_docs)]

pub mod address;
pub mod dc;
mod error;
pub mod file;
pub mod form;
pub mod gain;
pub mod iiod;
pub mod loss;
pub mod net;
pub mod paced;
mod radio;
pub mod rate;
pub mod ring;
pub mod rtl_tcp;
pub mod runner;
pub mod sample;
pub mod settle;
pub mod synth;
pub mod ws;

use sdrkit_core::{
    CommandId, Complex32, Hz, Published, Record, Results, Schema, SettingDecl, SettingId,
    StateBlob, StateError, Storage, Transport, Value, Values,
};
use sdrkit_graph::stream::SampleType;
use std::sync::Arc;
use std::time::Duration;

pub use address::{AddressPart, PartValue, Syntax};
pub use error::{Doing, DriverError, Protocol, Unavailable};
pub use radio::{
    own_id, own_setting, DriverDescriptor, DriverKind, KindDecl, Link, OpenRadio, Opens,
    RadioCommand, RadioSetting, StreamFormat, AGC, BANDWIDTH, BIAS_TEE, CENTRE, CENTRE_GIVEN,
    CONNECTED, GAIN, LOST, OVERFLOW, RADIO_EVENTS, RATE, RATE_EXACT, RATE_GIVEN, RESTART,
    STANDARD_FIELDS, STATE,
};
pub use radio::{tx_schema, TxSetting, TX_BANDWIDTH, TX_FREQUENCY, TX_POWER, TX_RATE};
pub use radio::{BUFFER_PCT, DELIVERED_PCT, LINK_DOWN, PROBLEM, RADIO_LOST, SAMPLES, TX_OUTPUT_DBM};
pub mod linkwatch;
pub use ring::{ring, Gap, Moves, RingReader, RingStats, RingWriter, RING_CAPACITY};
pub use settle::Settle;

/// The rates a radio offers: any rate between `min_hz` and `max_hz`, or only the listed ranges if there are some.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateLimits {
    /// The lowest rate. 0 if not known.
    pub min_hz: Hz,
    /// The highest rate. 0 if not known.
    pub max_hz: Hz,
    /// Inclusive ranges. A single rate is a range from it to itself.
    pub ranges: Vec<(Hz, Hz)>,
    /// What the link carries without loss, if that is below `max_hz` (a USB 2 cable, Wi-Fi).
    pub steady_hz: Option<Hz>,
}

/// What an open radio can do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capabilities {
    /// A name for the radio.
    pub name: String,
    /// The rates it offers.
    pub rates: RateLimits,
    /// The lowest frequency it is specified for.
    pub min_freq_hz: Hz,
    /// The highest frequency it is specified for.
    pub max_freq_hz: Hz,
    /// (min, max, step) in dB, if the gain can be set.
    pub gain_db: Option<(f64, f64, f64)>,
    /// (min, max) of the width of the analogue filter in Hz, if the width can be set. `None` for a filter that cannot be
    /// set or that the radio derives from the rate.
    pub bandwidth_hz: Option<(Hz, Hz)>,
    /// The radio has its own automatic gain control.
    pub has_agc: bool,
    /// The limits of its transmitter ([`Source::transmitter`]), known before the transmitter opens. `None` for a radio
    /// that cannot transmit.
    pub transmit: Option<TxLimits>,
    /// It has a bias tee.
    pub has_bias_tee: bool,
    /// The samples arrive in real time (a radio), so a slow reader loses them. A file or a generator waits instead.
    pub realtime: bool,
    /// The rates that people normally use, offered next to the rate range. Empty takes the single rates of
    /// [`RateLimits::ranges`] when it lists only such.
    pub common_rates_hz: Vec<Hz>,
    /// The widths of the analogue filter that people normally use, or the widths that the filter has.
    pub common_bandwidths_hz: Vec<Hz>,
    /// The gains in dB that the hardware has, for a gain set in steps. Empty for a gain without steps.
    pub gain_steps_db: Vec<f64>,
}

/// The limits of a transmitter. A radio declares them when it opens ([`Capabilities::transmit`]); an open transmitter
/// declares those of its outputs ([`TxCapabilities::limits`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TxLimits {
    /// The lowest carrier it is specified for.
    pub min_freq_hz: Hz,
    /// The highest carrier it is specified for. 0 if not known.
    pub max_freq_hz: Hz,
    /// The rates it offers.
    pub rates: RateLimits,
    /// (min, max) of the width of the analogue filter in Hz, if the width can be set.
    pub bandwidth_hz: Option<(Hz, Hz)>,
    /// (min, max, step) of the output power in dB relative to the full output: max is 0 for a radio that reaches its full
    /// output, min is negative. The power cannot be set if max is not above min.
    pub power_db: (f64, f64, f64),
    /// The power the transmitter starts with, in dB relative to the full output. `None` is the bottom of
    /// [`TxLimits::power_db`]: no power leaves the radio until the host sets one. A driver sets a value only for a radio
    /// whose own output is weak (a few milliwatts) and that drives no amplifier as far as the driver knows.
    pub start_power_db: Option<f64>,
    /// The rates that people normally use.
    pub common_rates_hz: Vec<Hz>,
    /// The output in dBm at full power ([`TX_POWER`] 0 dB), at increasing carrier frequencies: typical figures of the
    /// device, an estimate, not measured. The board and its cable change it. Empty when not known.
    pub full_output_dbm: Vec<(Hz, f64)>,
}

impl TxLimits {
    /// The estimated output in dBm at `carrier_hz` with `power_db` relative to the full output, interpolated between the
    /// points of [`TxLimits::full_output_dbm`]. `None` outside the points or without them.
    pub fn output_dbm(&self, carrier_hz: Hz, power_db: f64) -> Option<f64> {
        let t = &self.full_output_dbm;
        let i = t.windows(2).position(|w| (w[0].0..=w[1].0).contains(&carrier_hz));
        let full = match (i, t.as_slice()) {
            (Some(i), _) => {
                let ((f0, p0), (f1, p1)) = (t[i], t[i + 1]);
                p0 + (p1 - p0) * (carrier_hz - f0) / (f1 - f0)
            }
            (None, [(f, p)]) if *f == carrier_hz => *p,
            _ => return None,
        };
        Some(full + power_db)
    }
}

/// What a start or a live change returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Started {
    /// The rate the radio provides. The host resamples if it differs from the rate it requested.
    pub rate_hz: Hz,
    /// The centre frequency of the samples.
    pub centre_hz: Hz,
}

/// What a change of the settings did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Retuned {
    /// Applied while streaming, or kept for the time the radio is back.
    Live(Started),
    /// The change needs the stream stopped and started again (a new rate on most radios).
    NeedsRestart,
}

/// What [`Source::poll`] found besides the results it wrote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polled {
    /// [`Source::capabilities`] changed.
    pub capabilities: bool,
    /// The stream must be stopped and started again.
    pub restart: bool,
    /// A setting whose value the device told the driver (the rate an rtl_tcp server holds to).
    pub store: Option<(SettingId, Value)>,
}

/// The sample-loss figures of a source. Seconds are samples divided by the rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LossReport {
    /// The rate the figures are counted at.
    pub rate_hz: Hz,
    /// Samples the radio or its link did not deliver, link-down gaps included.
    pub radio_lost: u64,
    /// How many radio-side losses.
    pub radio_events: u64,
    /// How long ago the last radio-side loss was found.
    pub since_radio_loss: Option<Duration>,
    /// Samples missed while a network radio was reconnecting (also counted in `radio_lost`).
    pub link_down: u64,
    /// The delivered rate over the last complete window, in percent of the set rate.
    pub delivered_pct: Option<f64>,
    /// The delivered rate is below 97 %.
    pub rate_low: bool,
}

impl LossReport {
    /// Radio-side loss in seconds.
    pub fn radio_seconds(&self) -> f64 {
        if self.rate_hz > 0.0 {
            self.radio_lost as f64 / self.rate_hz
        } else {
            0.0
        }
    }
}

/// A kind of radio. The host keeps one per kind in a [`DriverKind`] and opens radios through it.
pub trait SourceDriver: Send + Sync {
    /// The descriptor of the kind.
    fn descriptor(&self) -> &'static DriverDescriptor;
    /// Opens the radio at `address` with the settings `values`, of the schema of the kind or of
    /// [`DriverKind::schema_at`] for `address`. It does not stream yet.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the radio cannot be reached or refuses.
    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError>;
    /// The settings that a radio at `address` adds ([`Source::settings`]) that are known before it opens, so that an
    /// open call can set them. The open radio adds these first, in this order, and no setting whose key the kind has.
    /// The default knows none.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] if the address names nothing that the driver can open.
    fn settings_at(&self, address: &str) -> Result<Vec<SettingDecl>, DriverError> {
        let _ = address;
        Ok(Vec::new())
    }
    /// Runs command `id` of the kind (counted in [`DriverDescriptor::kind_commands`]) with arguments that fit its
    /// declaration. It writes what it finds into `results`, the results of the kind ([`DriverDescriptor::kind_tables`]).
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the command fails. A driver with kind commands must implement it.
    fn invoke_kind(
        &self,
        id: CommandId,
        args: &Values,
        results: &mut Published,
    ) -> Result<Option<Record>, DriverError> {
        let _ = (id, args, results);
        Err(DriverError::NotNow {
            state: Doing::Closed,
        })
    }
}

/// An open radio. `start` passes it the writer of a ring. The radio's own thread fills the ring until `stop`.
pub trait Source: Send {
    /// What the radio can do.
    fn capabilities(&self) -> &Capabilities;
    /// Starts streaming into `sink`.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the stream cannot start.
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError>;
    /// Applies `values`, the whole state of the settings after one change. The driver sends only what changed. [`OpenRadio`]
    /// has made sure that every value fits the schema.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] if the device refuses a value, or another [`DriverError`]. The values then stay as they were.
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError>;
    /// Stops streaming and joins its thread. Safe to call twice.
    fn stop(&mut self);
    /// How one sample is stored in the ring. Known once the radio is open, fixed until it is opened again.
    fn sample_type(&self) -> SampleType;
    /// Where the link is.
    fn link(&self) -> Link;
    /// Writes the device's readings and log lines into `results` and says what changed. It never waits on the radio.
    fn poll(&mut self, results: &mut Results) -> Polled {
        let _ = results;
        Polled::default()
    }
    /// The settings that the device reveals only when it opens (gains of each stage, module settings, antenna lists), with
    /// their ranges and choices. [`OpenRadio`] adds them after the settings of the kind. The `i`-th has the id
    /// [`DriverDescriptor::run_time_id`] of `i`. The default adds none.
    fn settings(&self) -> Vec<SettingDecl> {
        Vec::new()
    }
    /// What the radio keeps across a restart (what it learned about a server). The default keeps nothing.
    fn save_state(&self) -> Option<StateBlob> {
        None
    }
    /// Restores the radio from what [`Source::save_state`] kept.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the state is not the radio's. The default returns [`StateError::NoState`].
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let _ = state;
        Err(StateError::NoState)
    }
    /// The sample-loss figures, for a source that counts them.
    fn loss(&self) -> Option<LossReport> {
        None
    }
    /// Ends the connection and connects again with the same settings.
    ///
    /// # Errors
    ///
    /// [`DriverError::NotNow`] for a radio that has no connection.
    fn reconnect(&mut self) -> Result<(), DriverError> {
        Err(DriverError::NotNow {
            state: Doing::Running,
        })
    }
    /// Runs device command `id` (counted in [`DriverDescriptor::commands`]) with arguments that fit its declaration.
    /// [`OpenRadio`] calls it only for an id of the descriptor's commands.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the device refuses. A driver with commands must implement it.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, DriverError> {
        let _ = (id, args);
        Err(DriverError::NotNow {
            state: Doing::Running,
        })
    }
    /// How the samples lie in the rings. Known once the radio is open, fixed until it is opened again.
    fn layout(&self) -> StreamLayout {
        StreamLayout::Single
    }
    /// Starts streaming into several rings, one per receiver, for a [`StreamLayout::Separate`] radio. Ring `i` gets receiver
    /// `i`. Fewer rings than receivers is allowed: the receivers without a ring are read and discarded.
    ///
    /// # Errors
    ///
    /// [`DriverError::Streams`] for a radio with one stream, or what [`Source::start`] returns.
    fn start_streams(&mut self, mut sinks: Vec<RingWriter>) -> Result<Started, DriverError> {
        match (sinks.pop(), sinks.is_empty()) {
            (Some(s), true) => self.start(s),
            _ => Err(DriverError::Streams {
                given: sinks.len() + 1,
                has: 1,
            }),
        }
    }
    /// Opens the transmitter with every output off air. Nothing goes on air until [`Transmitter::arm`]. `values` holds the
    /// transmit settings ([`TxCapabilities::settings`]). Stopping the source disarms and closes it.
    ///
    /// # Errors
    ///
    /// [`DriverError::CannotTransmit`] by default.
    fn transmitter(&mut self, values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        let _ = values;
        Err(DriverError::CannotTransmit)
    }
    /// The transmit settings of the radio, known before its transmitter opens: [`TxSetting`] then the driver's own, with
    /// the limits of [`Capabilities::transmit`] ([`tx_schema`]). [`OpenRadio`] adds them after its receive settings. `None`
    /// (the default) for a radio that cannot transmit.
    fn tx_schema(&self) -> Option<Arc<Schema>> {
        None
    }
}

/// How a source lays its samples out in the rings ([`Source::layout`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamLayout {
    /// One receiver, one ring.
    Single,
    /// `n` receivers sharing one clock, each in its own ring, sample for sample in step ([`Source::start_streams`]).
    Separate(usize),
    /// `n` receivers in one ring, interleaved per sample instant: RX1, RX2, RX1, RX2... The ring carries `n` x the rate in samples;
    /// [`Started::rate_hz`] is the rate of each receiver.
    Interleaved(usize),
}

impl StreamLayout {
    /// How many receivers deliver samples.
    pub fn receivers(&self) -> usize {
        self.count()
    }

    /// How many receivers or transmit outputs the layout has.
    pub fn count(&self) -> usize {
        match *self {
            StreamLayout::Single => 1,
            StreamLayout::Separate(n) | StreamLayout::Interleaved(n) => n,
        }
    }

    /// The layout's name: `"single"`, `"separate"` or `"interleaved"`.
    pub fn name(&self) -> &'static str {
        match self {
            StreamLayout::Single => "single",
            StreamLayout::Separate(_) => "separate",
            StreamLayout::Interleaved(_) => "interleaved",
        }
    }
}

/// What a transmitter can do, known once it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct TxCapabilities {
    /// A name for the transmitter ("ANTSDR E310 TX1").
    pub name: String,
    /// The limits of the outputs it opened. They lie inside [`Capabilities::transmit`] of the radio (two outputs can
    /// halve the highest rate).
    pub limits: TxLimits,
    /// The transmit outputs it has ("tx1", "tx2").
    pub outputs: Vec<String>,
    /// It can repeat a buffer by itself ([`Transmitter::play_cyclic`]).
    pub cyclic: bool,
    /// It can transmit while the receiver streams.
    pub full_duplex: bool,
    /// The samples it sends per transfer; a burst is padded with zeros to a whole number of these.
    pub buffer_samples: usize,
    /// The transmit settings: [`TxSetting`] followed by the driver's own.
    pub settings: Arc<Schema>,
}

/// Where a transmitter is. Only `Armed` and `Cyclic` are on air.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxState {
    /// Configured, muted, its sample buffer closed: nothing is sent, and pushed samples are refused.
    Disarmed,
    /// On air: pushed samples are sent; between bursts the radio sends nothing, or zeros.
    Armed,
    /// On air: one buffer is repeated by the radio until disarmed.
    Cyclic,
    /// Closed (by [`Transmitter::close`], a stop of the source, or a lost link). Nothing more is accepted.
    Closed,
}

impl TxState {
    /// The radio is transmitting (or ready to, the instant samples arrive).
    pub fn transmitting(&self) -> bool {
        matches!(self, TxState::Armed | TxState::Cyclic)
    }
}

/// A transmitter's counters ([`Transmitter::stats`]). Samples are counted at the transmit rate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TxStats {
    /// Samples passed to the radio (padding included).
    pub sent: u64,
    /// Transfers passed to the radio.
    pub buffers: u64,
    /// Samples waiting in the transmitter's queue.
    pub queued: usize,
    /// Zeros added to complete a burst ([`Transmitter::flush`]).
    pub padded: u64,
    /// Times the radio ran out of samples in the middle of a burst (it had sent everything before the next samples came).
    pub underruns: u64,
    /// How long those gaps were, in samples.
    pub underrun_samples: u64,
    /// The time the transmitter was last switched on air, if it is on air now.
    pub transmitting_since: Option<std::time::Instant>,
}

/// A radio's transmitter, from [`Source::transmitter`]. Every call returns quickly except where it says it waits.
///
/// The contract: nothing reaches the antenna unless [`Transmitter::arm`] succeeded and [`Transmitter::disarm`] has not been
/// called since. Disarm mutes first, then stops the samples, then powers the output down. Dropping a transmitter disarms and
/// closes it.
pub trait Transmitter: Send {
    /// What it can do.
    fn capabilities(&self) -> &TxCapabilities;
    /// Applies `values`, the whole state of the transmit settings. While on air, the carrier, the power and the filter
    /// change live. A new rate or output is refused until disarmed. While disarmed, the settings are stored and checked, and the
    /// radio stays muted.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the device refuses. Nothing changes.
    fn configure(&mut self, values: &Values) -> Result<(), DriverError>;
    /// Goes on air: unmutes to the configured power and opens the sample buffer. Refused when the radio is busy (another
    /// client transmits, the receiver holds the shared clock in a way transmit cannot share).
    fn arm(&mut self) -> Result<(), DriverError>;
    /// Goes off air: mutes, stops the samples (queued samples are dropped), closes the buffer, powers the output down. Safe to
    /// call at any time and twice.
    fn disarm(&mut self);
    /// Queues samples for transmission, waiting up to `wait` for room (zero: never waits). Returns how many were taken; the rest
    /// did not fit. Refused unless armed (not cyclic). Samples beyond full scale (magnitude 1) are clipped.
    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError>;
    /// Ends a burst: what is queued is sent, padded with zeros to a whole transfer, and the gap that follows is not an underrun.
    fn flush(&mut self) -> Result<(), DriverError>;
    /// The time from the push that completes the first transfer of a burst ([`TxCapabilities::buffer_samples`], or
    /// [`Transmitter::flush`] for a shorter burst) to the first sample of the burst at the antenna, at the configured
    /// rate: what the driver puts in front of a burst and the buffers it knows of. The network and what the radio
    /// holds unknown to the driver are not in it. A caller that wants a sample on the air at a given time pushes it
    /// this much early. The default is zero: the driver adds nothing that it knows of.
    fn start_latency(&self) -> Duration {
        Duration::ZERO
    }
    /// Waits until the radio has played the burst that [`Transmitter::flush`] ended, at most `wait`. Returns true if it
    /// has. [`Transmitter::disarm`] drops what is queued, so a caller that ends a burst calls flush, then this, then disarm.
    /// The default waits until the queue of the transmitter is empty and 50 ms more; a driver that knows when the radio
    /// has played its last transfer waits for that.
    fn drain(&mut self, wait: Duration) -> bool {
        let end = std::time::Instant::now() + wait;
        while self.stats().queued > 0 {
            if std::time::Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // The last samples are on their way to the radio and in its own buffer.
        std::thread::sleep(
            Duration::from_millis(50).min(end.saturating_duration_since(std::time::Instant::now())),
        );
        true
    }
    /// Repeats `iq` until disarmed (a test tone). Must be armed; replaces any streaming. `iq` should hold whole cycles.
    fn play_cyclic(&mut self, iq: &[Complex32]) -> Result<(), DriverError>;
    /// Where it is.
    fn state(&self) -> TxState;
    /// The counters.
    fn stats(&self) -> TxStats;
    /// Writes the log lines since the last call into `results`.
    fn poll(&mut self, results: &mut Results) {
        let _ = results;
    }
    /// Disarms every output and closes the connections. Nothing is accepted afterwards. Safe to call twice.
    fn close(&mut self);

    // ---- Several outputs. The calls above act on output 0. ----

    /// How the outputs take their samples: one output (the default), `n` outputs each pushed on its own
    /// ([`StreamLayout::Separate`]), or `n` outputs pushed together on output 0, interleaved per sample instant
    /// ([`StreamLayout::Interleaved`]: TX1, TX2, TX1...). The driver chooses it from its settings at open. It is fixed until the
    /// transmitter is opened again.
    fn layout(&self) -> StreamLayout {
        StreamLayout::Single
    }
    /// Applies `values` to output `output` (0 is the first), as [`Transmitter::configure`] does for output 0. The default has
    /// output 0 only.
    fn configure_output(&mut self, output: usize, values: &Values) -> Result<(), DriverError> {
        match output {
            0 => self.configure(values),
            n => Err(no_output(n)),
        }
    }
    /// Puts output `output` on air, as [`Transmitter::arm`] does for output 0.
    fn arm_output(&mut self, output: usize) -> Result<(), DriverError> {
        match output {
            0 => self.arm(),
            n => Err(no_output(n)),
        }
    }
    /// Takes output `output` off air, as [`Transmitter::disarm`] does for output 0. Safe at any time, also for an output the
    /// transmitter does not have.
    fn disarm_output(&mut self, output: usize) {
        if output == 0 {
            self.disarm();
        }
    }
    /// Queues samples for output `output`, as [`Transmitter::push`] does for output 0. In the interleaved layout, output 0 takes
    /// the samples of every output, interleaved, and the other outputs refuse samples.
    fn push_output(
        &mut self,
        output: usize,
        iq: &[Complex32],
        wait: Duration,
    ) -> Result<usize, DriverError> {
        match output {
            0 => self.push(iq, wait),
            n => Err(no_output(n)),
        }
    }
    /// Ends the burst of output `output`, as [`Transmitter::flush`] does for output 0.
    fn flush_output(&mut self, output: usize) -> Result<(), DriverError> {
        match output {
            0 => self.flush(),
            n => Err(no_output(n)),
        }
    }
    /// Repeats `iq` on output `output` until it is disarmed, as [`Transmitter::play_cyclic`] does for output 0.
    fn play_cyclic_output(&mut self, output: usize, iq: &[Complex32]) -> Result<(), DriverError> {
        match output {
            0 => self.play_cyclic(iq),
            n => Err(no_output(n)),
        }
    }
    /// Where output `output` is. An output the transmitter does not have is [`TxState::Closed`].
    fn state_output(&self, output: usize) -> TxState {
        match output {
            0 => self.state(),
            _ => TxState::Closed,
        }
    }
    /// The counters of output `output`. An output the transmitter does not have has all counters at zero.
    fn stats_output(&self, output: usize) -> TxStats {
        match output {
            0 => self.stats(),
            _ => TxStats::default(),
        }
    }
}

/// The error for an output a transmitter does not have (`output` counts from 0).
pub fn no_output(output: usize) -> DriverError {
    DriverError::Streams {
        given: output + 1,
        has: 1,
    }
}

/// Every driver of this crate: the test signal (playing what `generator` makes), files in `storage`, and rtl_tcp and iiod
/// radios reached through `transport`.
pub fn drivers(
    generator: Arc<dyn synth::GeneratorFactory>,
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
) -> Vec<Box<dyn SourceDriver>> {
    vec![
        Box::new(synth::SynthDriver::new(generator)),
        Box::new(file::FileDriver::new(storage)),
        Box::new(rtl_tcp::RtlTcpDriver::new(transport.clone())),
        Box::new(iiod::IiodDriver::new(transport)),
    ]
}

/// Formats a rate or frequency in millions with up to 3 decimals, trailing zeros dropped ("2.4", "0.9", "61.44").
pub fn fmt_m(hz: Hz) -> String {
    let s = format!("{:.3}", hz / 1e6);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// How much of a stream arrives, for people: the arriving rate, the asked rate and the percentage, "14 of 20 Msps arrive
/// (70 %)". Every driver words a shortfall of a stream in this form.
pub fn arrive_text(arriving_hz: Hz, asked_hz: Hz) -> String {
    let pct = if asked_hz > 0.0 {
        (100.0 * arriving_hz / asked_hz).clamp(0.0, 100.0)
    } else {
        0.0
    };
    format!("{} of {} Msps arrive ({pct:.0} %)", fmt_m(round_rate(arriving_hz)), fmt_m(asked_hz))
}

/// `hz` to two significant figures above 1 Msps and to 0.01 Msps below: a measured rate shows no false precision.
fn round_rate(hz: Hz) -> Hz {
    let step = if hz >= 10e6 { 1e6 } else if hz >= 1e6 { 0.1e6 } else { 0.01e6 };
    (hz / step).round() * step
}

/// Helpers for the tests of this crate.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use sdrkit_core::Owner;

    /// The values of a radio of `driver`'s kind: the defaults, then `set` by key.
    pub fn values(driver: &dyn SourceDriver, set: &[(&str, Value)]) -> Values {
        let schema = driver.descriptor().decl().schema().clone();
        let mut v = Values::defaults(&schema);
        for (key, value) in set {
            let id = schema.id(key).unwrap();
            v.set(id, value).unwrap();
        }
        v
    }

    /// Empty results of the declaration of `kind`.
    pub fn results(kind: &DriverDescriptor) -> Results {
        Results::new(kind.decl().results().clone())
    }

    /// The kind of `driver` as an owner, to open radios through.
    pub fn kind(driver: impl SourceDriver + 'static) -> DriverKind {
        let k = DriverKind::new(Box::new(driver));
        let _ = k.schema();
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millions_print_with_up_to_three_decimals() {
        assert_eq!(fmt_m(2.4e6), "2.4");
        assert_eq!(fmt_m(900_001.0), "0.9");
        assert_eq!(fmt_m(61.44e6), "61.44");
        assert_eq!(fmt_m(12.345e6), "12.345");
        assert_eq!(fmt_m(20e6), "20");
    }

    #[test]
    fn the_list_has_all_four_drivers() {
        let list = drivers(
            Arc::new(|_: &str, _: &Values, _| None),
            Arc::new(sdrkit_core::MemStorage::new()),
            Arc::new(sdrkit_core::net::FakeTransport::new()),
        );
        let ids: Vec<_> = list.iter().map(|d| d.descriptor().key).collect();
        assert_eq!(ids, ["synth", "file", "rtl_tcp", "iiod"]);
    }
}
