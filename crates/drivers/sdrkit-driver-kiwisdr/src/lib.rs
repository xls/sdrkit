//! A client for KiwiSDR receivers: the IQ stream of one channel of a KiwiSDR over WebSocket, at the rate and centre of the
//! receiver, with its meters, its listener list and, when requested, its own waterfall.

#![warn(missing_docs)]

pub mod directory;
pub mod info;
pub mod proto;
mod radio;

#[cfg(test)]
mod tests;

pub use info::{Listener, ServerInfo, WaterfallLine};
pub use proto::Class;
pub use radio::Timing;

use info::Meters;
use proto::{smeter_text, Address};
use radio::{Ctx, KiwiRadio, Session};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Argument, Axis, Clock, Command, CommandId, CommandSet, Declare, Field, Published, RasterDef,
    Record, Results, Setting, SettingSet, TableDef, Transport, Values,
};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, RunnerHooks, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, Doing, DriverDescriptor, DriverError, RateLimits, Source, SourceDriver,
    StreamFormat,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The longest name shown to the owner, in characters.
pub const MAX_USER_NAME: usize = 32;
/// The longest location shown to the owner, in characters.
pub const MAX_LOCATION: usize = 64;

// ---- Settings ----

sdrkit_core::choice_enum! {
    /// The half-width of the IQ filter of the receiver around the tuned frequency ([`PASSBAND`]).
    pub enum Passband {
        /// 3.0 kHz.
        P3000 = ("3000", "±3.0 kHz"),
        /// 5.0 kHz.
        P5000 = ("5000", "±5.0 kHz"),
        /// 5.9 kHz.
        P5900 = ("5900", "±5.9 kHz"),
        /// The widest the receiver allows at its rate.
        Max = ("max", "Widest the receiver allows"),
    }
}

/// The password of the receiver.
pub const PASSWORD: Setting<field::Text> = Setting::new("password", "Password", "", "")
    .section(sdrkit_driver_core::form::LINK_SECTION)
    .restart()
    .help("Only if the owner set one. Never shown in names, logs or the status.");
/// The password that lifts the time limit of a session.
pub const TLIMIT_PASSWORD: Setting<field::Text> =
    Setting::new("tlimit_password", "Time-limit exemption password", "", "")
        .section(sdrkit_driver_core::form::LINK_SECTION)
        .restart()
        .help(
            "Some owners give regular listeners a password that lifts the time limit of a session.",
        );
/// The name shown to the owner.
pub const USER_NAME: Setting<field::Text> =
    Setting::new("user_name", "Your name, shown to the owner", "", "sdrkit")
        .section(sdrkit_driver_core::form::LINK_SECTION)
        .restart()
        .help("Shown in the listener list of the receiver, so the owner knows who listens. At most 32 characters.");
/// The location shown to the owner.
pub const LOCATION: Setting<field::Text> = Setting::new(
    "location",
    "Your location, shown to the owner",
    "",
    "",
)
.section(sdrkit_driver_core::form::LINK_SECTION)
.restart()
.help(
    "Shown to the owner and to other listeners. At most 64 characters. If empty, nothing is sent.",
);
/// The passband of the IQ filter.
pub const PASSBAND: Setting<field::Choice<Passband>> =
    Setting::choice("passband", "Passband", Passband::P5000)
        .section(sdrkit_driver_core::form::RATE_SECTION)
        .help(
        "The half-width of the IQ filter of the receiver around the tuned frequency. The receiver sends 12 kHz of spectrum. \
         A passband wider than the rate of the receiver allows is refused.",
    );
/// Swap I and Q.
pub const SWAP_IQ: Setting<field::Bool> = sdrkit_driver_core::form::SWAP_IQ.help(
    "Mirrors the spectrum. Turn it on if signals that belong above the frequency appear below it.",
);
/// Ask the receiver for its listener list.
pub const USERS: Setting<field::Bool> = Setting::new("users", "Show other listeners", "", true)
    .section(sdrkit_driver_core::form::OTHER_SECTION)
    .help("Asks the receiver every 10 s who else is connected.");
/// Open the waterfall of the receiver.
pub const WATERFALL: Setting<field::Bool> = Setting::new(
    "waterfall",
    "Receiver's waterfall",
    "",
    false,
)
.section(sdrkit_driver_core::form::OTHER_SECTION)
.restart()
.help(
    "Also shows the wide waterfall of the receiver. It opens a second connection to the receiver.",
);
/// The zoom of the waterfall.
pub const WF_ZOOM: Setting<field::I64> = Setting::new("wf_zoom", "Waterfall zoom", "", 0)
    .range(0.0, 14.0, 1.0)
    .section(sdrkit_driver_core::form::OTHER_SECTION)
    .help("0 shows the whole range. Each step halves the span around the tuned frequency.");
sdrkit_core::settings! {
    /// The own settings of the KiwiSDR driver.
    pub enum KiwiSetting {
        /// [`PASSBAND`].
        Passband = PASSBAND,
        /// [`SWAP_IQ`].
        SwapIq = SWAP_IQ,
        /// [`PASSWORD`].
        Password = PASSWORD,
        /// [`TLIMIT_PASSWORD`].
        TlimitPassword = TLIMIT_PASSWORD,
        /// [`USER_NAME`].
        UserName = USER_NAME,
        /// [`LOCATION`].
        Location = LOCATION,
        /// [`USERS`].
        Users = USERS,
        /// [`WATERFALL`].
        Waterfall = WATERFALL,
        /// [`WF_ZOOM`].
        WfZoom = WF_ZOOM,
    }
}

// ---- Readings ----

/// The S-meter, dBm.
pub const RSSI: Field<field::F64> = Field::new("rssi", "S-meter", "dBm");
/// The highest S-meter reading of the last second, dBm.
pub const RSSI_MAX: Field<field::F64> =
    Field::new("rssi_max", "S-meter, highest of the last second", "dBm");
/// The S-meter in S-units ("S9+16").
pub const S_UNITS: Field<field::Text> = Field::new("s_units", "S-units", "");
/// The ADC of the receiver overloaded in the last second.
pub const OVERLOAD: Field<field::Bool> = Field::new("overload", "ADC overload", "");
/// The GPS time of the last frame is valid.
pub const GPS: Field<field::Bool> = Field::new("gps", "GPS time", "");
/// The GPS time put on the samples last.
pub const GPS_TIME: Field<field::Time> = Field::new("gps_time", "GPS time of the samples", "s");
/// IQ frames received.
pub const FRAMES: Field<field::I64> = Field::new("frames", "Frames received", "");
/// Frames lost (sequence gaps).
pub const MISSED: Field<field::I64> = Field::new("missed", "Frames missed", "");
/// Frames ignored as malformed.
pub const BAD: Field<field::I64> = Field::new("bad", "Bad frames", "");
/// Frames that were not IQ.
pub const NOT_IQ: Field<field::I64> = Field::new("not_iq", "Frames without IQ", "");
/// Gaps filled with zeros.
pub const GAPS: Field<field::I64> = Field::new("gaps", "Gaps filled", "");
/// The speed of the link over the last second.
pub const THROUGHPUT: Field<field::F64> = Field::new("throughput", "Throughput", "Mbit/s");
/// The IQ samples received as a share of the rate.
pub const IQ_PCT: Field<field::F64> = Field::new("iq_rate", "IQ samples received", "%");
/// The version of the server, "1.800".
pub const VERSION: Field<field::Text> = Field::new("version", "Version", "");
/// The channels of the receiver.
pub const CHANNELS: Field<field::I64> = Field::new("channels", "Channels", "");
/// The channels without a password.
pub const FREE_CHANNELS: Field<field::I64> =
    Field::new("free_channels", "Channels without a password", "");
/// The lowest frequency the receiver covers.
pub const LOWEST: Field<field::Hz> = Field::new("lowest_hz", "Covered from", "Hz");
/// The highest frequency the receiver covers.
pub const HIGHEST: Field<field::Hz> = Field::new("highest_hz", "Covered to", "Hz");
/// The time limit of a session.
pub const SESSION_LIMIT: Field<field::I64> = Field::new("session_limit", "Session limit", "min");
/// The name of the receiver.
pub const RX_NAME: Field<field::Text> = Field::new("rx_name", "Receiver", "");
/// The location of the receiver.
pub const RX_LOCATION: Field<field::Text> = Field::new("rx_location", "Location", "");
/// The antenna of the receiver.
pub const RX_ANTENNA: Field<field::Text> = Field::new("rx_antenna", "Antenna", "");
/// The grid square of the receiver.
pub const RX_GRID: Field<field::Text> = Field::new("rx_grid", "Grid", "");
/// The position of the receiver.
pub const RX_POSITION: Field<field::Position> = Field::new("rx_position", "Position", "");
/// Why the radio does not stream.
pub const REASON: Field<field::Text> = Field::new("reason", "Not streaming", "");
/// Why the waterfall is not available.
pub const WATERFALL_PROBLEM: Field<field::Text> =
    Field::new("waterfall_problem", "Waterfall not available", "");

/// The readings of a KiwiSDR.
pub const READINGS: &[&dyn Declare] = &[
    &RSSI,
    &RSSI_MAX,
    &S_UNITS,
    &OVERLOAD,
    &GPS,
    &GPS_TIME,
    &FRAMES,
    &MISSED,
    &BAD,
    &NOT_IQ,
    &GAPS,
    &THROUGHPUT,
    &IQ_PCT,
    &VERSION,
    &CHANNELS,
    &FREE_CHANNELS,
    &LOWEST,
    &HIGHEST,
    &SESSION_LIMIT,
    &RX_NAME,
    &RX_LOCATION,
    &RX_ANTENNA,
    &RX_GRID,
    &RX_POSITION,
    &REASON,
    &WATERFALL_PROBLEM,
];

/// The channel of a listener.
pub const L_CHANNEL: Field<field::I64> = Field::new("channel", "Channel", "");
/// The name of a listener. Empty for a free channel.
pub const L_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The location of a listener.
pub const L_LOCATION: Field<field::Text> = Field::new("location", "Location", "");
/// The frequency a listener is tuned to.
pub const L_FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
/// The demodulator of a listener, as the receiver names it.
pub const L_DEMOD: Field<field::Text> = Field::new("demod", "Demodulator", "");
/// How long a listener is connected.
pub const L_CONNECTED: Field<field::Duration> = Field::new("connected", "Connected", "s");

/// The listener list of the receiver, one row per channel (key: the channel number).
pub static LISTENERS: TableDef = TableDef {
    key: "listeners",
    label: "Listeners",
    columns: &[
        &L_CHANNEL,
        &L_NAME,
        &L_LOCATION,
        &L_FREQ,
        &L_DEMOD,
        &L_CONNECTED,
    ],
    cap: info::MAX_LISTENERS,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The waterfall of the receiver: one row per line, the newest last, in dB.
pub static WATERFALL_RASTER: RasterDef = RasterDef {
    key: "waterfall",
    label: "Waterfall",
    x_unit: "Hz",
    y_unit: "line",
    unit: "dB",
};

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "kiwisdr",
    name: "KiwiSDR",
    opens: sdrkit_driver_core::Opens::Many,
    addresses: &[
        "kiwisdr:<host> (port 8073)",
        "kiwisdr:<host>:<port>",
        "kiwisdr:http://<host>:<port>/",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(proto::DEFAULT_PORT)),
    ],
    settings: &[KiwiSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cs16,
        min_rate_hz: proto::MIN_FS,
        max_rate_hz: proto::MAX_FS,
        channels: 1,
    }],
    accepts: &[],
    tables: &[&LISTENERS],
    rasters: &[&WATERFALL_RASTER],
    series: &[],
    points: "",
    kind_commands: KiwiKindCommand::COMMANDS,
    kind_tables: &[&RECEIVERS],
};

// ---- Errors ----

/// What the handshake waited for and did not get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Awaited {
    /// The `sample_rate` message of the IQ connection.
    SampleRate,
    /// The `wf_setup` message of the waterfall connection.
    WfSetup,
}

/// What a stream that stalled still sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stall {
    /// Nothing.
    Silent,
    /// Audio frames, but no IQ: the receiver did not take the IQ demodulator.
    AudioOnly,
    /// Messages, but no IQ.
    MessagesOnly,
}

impl std::fmt::Display for Awaited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Awaited::SampleRate => "sample_rate",
            Awaited::WfSetup => "wf_setup",
        })
    }
}

impl std::fmt::Display for Stall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Stall::Silent => "no data at all",
            Stall::AudioOnly => "audio frames but no IQ (it did not take the IQ demodulator)",
            Stall::MessagesOnly => "messages but no IQ samples",
        })
    }
}

/// The errors of the KiwiSDR driver that [`DriverError`] has no variant for. They reach the host inside
/// [`DriverError::Driver`].
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum KiwiError {
    /// The address is an `https://` or `wss://` address.
    #[error(
        "secure (https) addresses are not supported: use the http address and port of the receiver"
    )]
    SecureAddress,
    /// `badp=1`: a wrong password, or every channel without a password is busy.
    #[error("{}", if *set {
        "the KiwiSDR refused the connection: wrong password, or all its channels without a password are busy"
    } else {
        "this KiwiSDR needs a password, or all its free channels are busy"
    })]
    Password {
        /// A password was sent.
        set: bool,
    },
    /// `badp=2`: the receiver is starting up.
    #[error("the KiwiSDR is still starting up")]
    Starting,
    /// `badp=5`: one connection per address, and another one is open.
    #[error("this KiwiSDR allows only one connection from an address, and another one is open")]
    OneConnection,
    /// `badp=6`: the receiver is updating its database.
    #[error("the KiwiSDR is updating its database")]
    Updating,
    /// Another `badp` code.
    #[error("the KiwiSDR refused the connection (code {})", code.map_or_else(|| "?".to_string(), |c| c.to_string()))]
    Refused {
        /// The code, if it is a number.
        code: Option<i64>,
    },
    /// `too_busy`: every channel is in use.
    #[error("all {} channels of the KiwiSDR are in use", channels.map_or_else(String::new, |c| c.to_string()))]
    AllBusy {
        /// The channel count, if the message has it.
        channels: Option<u32>,
    },
    /// `down`: the receiver is down.
    #[error("the KiwiSDR is down (maintenance or restarting)")]
    Down,
    /// `redirect`: the receiver redirects the client to another address.
    #[error("the KiwiSDR asks to use another address: {to}")]
    Redirect {
        /// The other address.
        to: String,
    },
    /// The time limit of the receiver for one listener ran out.
    #[error("the time limit of the KiwiSDR for one listener ran out")]
    TimeLimit,
    /// The receiver closed the connection.
    #[error("the KiwiSDR closed the connection{}", limit_min.map_or_else(String::new, |m| format!(" (it ends a session after {m} minutes)")))]
    Closed {
        /// The session limit of the receiver, minutes, if it told it.
        limit_min: Option<u32>,
    },
    /// The reported IQ rate is not one of a KiwiSDR.
    #[error("the server reports an IQ rate of {} Hz, not the rate of a KiwiSDR", reported.map_or_else(|| "?".to_string(), |r| r.to_string()))]
    NotKiwiRate {
        /// The rate, if it is a number.
        reported: Option<f64>,
    },
    /// The hand-shake did not finish in time.
    #[error("the KiwiSDR did not finish the hand-shake (no {awaited})")]
    Incomplete {
        /// What did not come.
        awaited: Awaited,
    },
    /// No IQ came for the stall time.
    #[error("no IQ from the KiwiSDR for {seconds} s: {kind}")]
    Stalled {
        /// What came instead.
        kind: Stall,
        /// The stall time, seconds.
        seconds: u64,
    },
}

impl KiwiError {
    /// How the driver waits before it connects again after this error.
    pub fn class(&self) -> Class {
        match self {
            KiwiError::Starting | KiwiError::Updating | KiwiError::AllBusy { .. } => Class::Busy,
            KiwiError::OneConnection | KiwiError::Down | KiwiError::TimeLimit => Class::Hold,
            KiwiError::Password { .. } | KiwiError::Refused { .. } => Class::Password,
            KiwiError::SecureAddress
            | KiwiError::Redirect { .. }
            | KiwiError::NotKiwiRate { .. } => Class::Final,
            KiwiError::Closed { .. } | KiwiError::Incomplete { .. } | KiwiError::Stalled { .. } => {
                Class::Retry
            }
        }
    }
}

/// The class of `e`: the class of a [`KiwiError`], else [`Class::Retry`].
pub fn class_of(e: &DriverError) -> Class {
    match e {
        DriverError::Driver(s) => s
            .downcast_ref::<KiwiError>()
            .map_or(Class::Retry, KiwiError::class),
        _ => Class::Retry,
    }
}

// ---- The driver ----

// ---- Commands of the kind ----

/// The receiver to test, `host:port` or `host`.
pub const ADDRESS: Argument<field::Text> = Argument::new(Field::new("address", "Address", ""))
    .help("The receiver to test, as host:port or host.");
/// The password of the receiver, for a test. Empty for none.
pub const TEST_PASSWORD: Setting<field::Text> = Setting::new("password", "Password", "", "")
    .help("The password of the receiver, if it asks for one.");

/// Tests the connection to the receiver at [`ADDRESS`]: connects, logs in, reads up to the IQ rate and closes at once. The
/// receiver is listed in [`RECEIVERS`].
pub const PROBE: Command = Command::new("probe", "Test the connection")
    .help("Connects to the receiver, logs in and closes at once. It streams nothing. A receiver that answers is listed.")
    .args(&[&ADDRESS, &TEST_PASSWORD]);

sdrkit_core::commands! {
    /// The commands of the KiwiSDR driver kind.
    pub enum KiwiKindCommand {
        /// [`PROBE`].
        Probe = PROBE,
    }
}

/// The address of a receiver that a test reached, `host:port`.
pub const R_ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
/// The version of its server, `major.minor`.
pub const R_VERSION: Field<field::Text> = Field::new("version", "Version", "");
/// Its channels.
pub const R_CHANNELS: Field<field::I64> = Field::new("channels", "Channels", "");

/// The receivers that a test reached, one row per address.
pub static RECEIVERS: TableDef = TableDef {
    key: "receivers",
    label: "Receivers tested",
    columns: &[&R_ADDRESS, &R_VERSION, &R_CHANNELS],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

/// What a test of the connection found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Probe {
    /// The version of the server (major, minor).
    pub version: Option<(u32, u32)>,
    /// The channels of the receiver.
    pub channels: Option<u32>,
}

/// What is known per receiver address, shared by every open of it.
#[derive(Debug, Default)]
pub(crate) struct Known {
    pub info: Mutex<ServerInfo>,
    pub meters: Meters,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The KiwiSDR driver. It keeps what it learnt per receiver address.
#[derive(Clone)]
pub struct KiwiDriver {
    transport: Arc<dyn Transport>,
    clock: Clock,
    known: Arc<Mutex<HashMap<String, Arc<Known>>>>,
    timing: Timing,
}

impl KiwiDriver {
    /// A driver that reaches its receivers through `transport` and reads the time of day from `clock`.
    pub fn new(transport: Arc<dyn Transport>, clock: Clock) -> Self {
        Self::with_timing(transport, clock, Timing::default())
    }

    /// A driver with its own timings.
    pub fn with_timing(transport: Arc<dyn Transport>, clock: Clock, timing: Timing) -> Self {
        KiwiDriver {
            transport,
            clock,
            known: Arc::default(),
            timing,
        }
    }

    fn ctx(&self, addr: Address) -> Ctx {
        let label = addr.show();
        let known = lock(&self.known).entry(label.clone()).or_default().clone();
        Ctx {
            addr,
            label,
            transport: self.transport.clone(),
            clock: self.clock.clone(),
            known,
            session: Arc::new(Session::new(self.timing.clone())),
        }
    }
}

impl SourceDriver for KiwiDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn invoke_kind(
        &self,
        id: CommandId,
        args: &Values,
        results: &mut Published,
    ) -> Result<Option<Record>, DriverError> {
        let Some(KiwiKindCommand::Probe) = KiwiKindCommand::from_id(id) else {
            return Err(DriverError::NotNow {
                state: Doing::Closed,
            });
        };
        let address = args.arg(&ADDRESS).map_err(|_| DriverError::Address)?;
        let ctx = self.ctx(proto::parse_address(address)?);
        let p = radio::probe(&ctx, args.get(&TEST_PASSWORD))?;
        if let Some(mut row) = results.row_mut(&RECEIVERS, &ctx.label) {
            row.set(&R_ADDRESS, &ctx.label);
            row.opt(
                &R_VERSION,
                p.version.map(|(a, b)| format!("{a}.{b}")).as_deref(),
            );
            row.opt(&R_CHANNELS, p.channels.map(i64::from));
        }
        Ok(None)
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let ctx = Arc::new(self.ctx(proto::parse_address(address)?));
        let restart = radio::Options::from_values(values).restart_part();
        let c = ctx.clone();
        let opener: Opener<KiwiRadio> =
            Arc::new(move |values, abort, notices| radio::open_radio(&c, values, abort, notices));
        let mut cfg = RunnerConfig::new(format!("KiwiSDR {}", ctx.label), NETWORK_ADVICE);
        cfg.open_attempts = 1;
        cfg.reconnect_gap = self.timing.reconnect_gap;
        cfg.hooks = Some(Arc::new(KiwiHooks {
            ctx,
            restart: Mutex::new(restart),
        }));
        Ok(Box::new(StreamRunner::open(cfg, opener, values)?))
    }
}

/// The capabilities of a receiver at rate `fs` that covers `lo` to `hi`.
pub fn capabilities(label: &str, fs: f64, lo: f64, hi: f64) -> Capabilities {
    Capabilities {
        name: format!("KiwiSDR {label}"),
        rates: RateLimits {
            min_hz: fs,
            max_hz: fs,
            ranges: vec![(fs, fs)],
            steady_hz: None,
        },
        min_freq_hz: lo,
        max_freq_hz: hi,
        gain_db: Some((0.0, 120.0, 1.0)),
        bandwidth_hz: None,
        has_agc: true,
        transmit: None,
        has_bias_tee: false,
        realtime: true,
        common_rates_hz: Vec::new(),
        common_bandwidths_hz: Vec::new(),
        gain_steps_db: Vec::new(),
    }
}

/// The status line of a connected receiver: version, rate, S-meter, overload, GPS and link speed.
fn status_line(i: &ServerInfo, m: &Meters) -> String {
    let mut parts = vec![match i.version() {
        Some(v) => format!("KiwiSDR {v}"),
        None => "KiwiSDR".into(),
    }];
    let (mbit, iq) = m.link();
    if let Some(fs) = i.sample_rate {
        let mut s = format!("{:.1} ksps IQ", fs / 1e3);
        if let Some(f) = iq.filter(|f| *f < 0.97) {
            s.push_str(&format!(" ({:.0} %)", f * 100.0));
        }
        parts.push(s);
    }
    if let Some(d) = m.rssi().0 {
        parts.push(smeter_text(d));
    }
    if m.overload() {
        parts.push("ADC overload".into());
    }
    let gps = m.gps_valid.load(Ordering::Relaxed);
    parts.push(if gps { "GPS time" } else { "no GPS" }.into());
    if let Some(v) = mbit {
        parts.push(format!("{v:.2} Mbit/s"));
    }
    parts.join(", ")
}

fn count(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Writes the readings of `i` and `m` into `r`.
fn write_readings(i: &ServerInfo, m: &Meters, r: &mut Results) {
    let mut f = r.fields_mut();
    let (rssi, max) = m.rssi();
    f.opt(&RSSI, rssi);
    f.opt(&RSSI_MAX, max);
    let units = rssi.map(proto::s_units);
    f.opt(&S_UNITS, units.as_deref());
    f.set(&OVERLOAD, m.overload());
    f.set(&GPS, m.gps_valid.load(Ordering::Relaxed));
    f.opt(&GPS_TIME, m.gps_unix());
    f.set(&FRAMES, count(m.frames.load(Ordering::Relaxed)));
    f.set(&MISSED, count(m.missed.load(Ordering::Relaxed)));
    f.set(&BAD, count(m.bad.load(Ordering::Relaxed)));
    f.set(&NOT_IQ, count(m.not_iq.load(Ordering::Relaxed)));
    f.set(&GAPS, count(m.discontinuities.load(Ordering::Relaxed)));
    let (mbit, iq) = m.link();
    f.opt(&THROUGHPUT, mbit);
    f.opt(&IQ_PCT, iq.map(|x| x * 100.0));
    let version = i.version();
    f.opt(&VERSION, version.as_deref());
    f.opt(&CHANNELS, i.rx_chans.map(i64::from));
    f.opt(&FREE_CHANNELS, i.chan_no_pwd.map(i64::from));
    let (lo, hi) = i.covered();
    f.set(&LOWEST, lo);
    f.set(&HIGHEST, hi);
    f.opt(&SESSION_LIMIT, i.limit_min().map(i64::from));
    for (field, text) in [
        (&RX_NAME, &i.rx_name),
        (&RX_LOCATION, &i.rx_location),
        (&RX_ANTENNA, &i.rx_antenna),
        (&RX_GRID, &i.rx_grid),
    ] {
        f.opt(field, Some(text.as_str()).filter(|t| !t.is_empty()));
    }
    f.opt(&RX_POSITION, i.rx_gps);
    let reason = i.reason.as_ref().map(ToString::to_string);
    f.opt(&REASON, reason.as_deref());
    let wf = i.waterfall_problem.as_ref().map(ToString::to_string);
    f.opt(&WATERFALL_PROBLEM, wf.as_deref());
}

/// Writes the waterfall lines of `i` into the raster of `r`, the newest last.
fn write_waterfall(i: &ServerInfo, r: &mut Results) {
    let Some(id) = r.decl().raster_id(WATERFALL_RASTER.key) else {
        return;
    };
    let Some(newest) = i.waterfall.back() else {
        return;
    };
    let width = newest.levels_db.len();
    r.update_raster(id, |ras| {
        ras.width = width;
        ras.height = i.waterfall.len();
        let step = if width > 0 {
            newest.span_hz / width as f64
        } else {
            0.0
        };
        ras.x = Axis::new(newest.start_hz + step / 2.0, step, WATERFALL_RASTER.x_unit);
        ras.y = Axis::new(0.0, 1.0, WATERFALL_RASTER.y_unit);
        WATERFALL_RASTER.unit.clone_into(&mut ras.unit);
        ras.values.clear();
        for line in &i.waterfall {
            let n = line.levels_db.len().min(width);
            ras.values.extend_from_slice(&line.levels_db[..n]);
            ras.values.resize(ras.values.len() + width - n, f32::NAN);
        }
        true
    });
}

/// Writes the listener list into the table of `r`.
fn write_listeners(list: &[Listener], r: &mut Results) {
    let Some(id) = r.decl().table_id(LISTENERS.key) else {
        return;
    };
    let Some(mut t) = r.table_mut(id) else {
        return;
    };
    t.clear();
    for l in list {
        let mut row = t.row_mut(&l.channel.to_string());
        row.set(&L_CHANNEL, i64::from(l.channel));
        row.set(&L_NAME, &l.name);
        row.set(&L_LOCATION, &l.location);
        row.opt(&L_FREQ, l.freq_hz);
        row.set(&L_DEMOD, &l.demod);
        row.opt(&L_CONNECTED, l.connected_s);
    }
}

/// What the runner requests from the driver when no radio is open: a restart check, the status of a lost link, and the
/// state at a stop.
struct KiwiHooks {
    ctx: Arc<Ctx>,
    restart: Mutex<radio::RestartPart>,
}

impl std::fmt::Debug for KiwiHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KiwiHooks")
            .field("label", &self.ctx.label)
            .finish()
    }
}

impl RunnerHooks for KiwiHooks {
    fn restart_needed(&self, values: &Values) -> bool {
        let new = radio::Options::from_values(values).restart_part();
        let mut old = lock(&self.restart);
        if *old == new {
            return false;
        }
        *old = new;
        true
    }
    fn status(&self) -> Option<String> {
        let i = lock(&self.ctx.known.info);
        if i.connected {
            return None;
        }
        Some(match &i.reason {
            Some(e) => format!("KiwiSDR: {e}"),
            None => "KiwiSDR: not connected".into(),
        })
    }
    fn stopped(&self) {
        let mut i = lock(&self.ctx.known.info);
        i.connected = false;
        i.reason = None;
    }
}
