//! The OpenWebRX client driver. An OpenWebRX server shares one radio with many listeners over a WebSocket and sends demodulated
//! audio, never IQ. The driver returns that audio as real 16-bit samples: in the `iq` delivery the server demodulates USB, so the
//! stream holds the band from the dial frequency up to 5.4 kHz above it; in the `audio` delivery the stream is the audio of the
//! server's demodulator.
//!
//! Standard: the OpenWebRX WebSocket protocol, versions 0.18 and later.

#![warn(missing_docs)]

mod adpcm;
mod conn;
mod proto;
mod radio;
mod view;

use radio::OwrxRadio;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Clock, Declare, Field, RasterDef, Results, Setting, SettingSet, TableDef, Transport, Values,
};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, RunnerHooks, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, DriverDescriptor, DriverError, Source, SourceDriver, StreamFormat,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

// ---- Settings ----

sdrkit_core::choice_enum! {
    /// What the stream holds ([`DELIVERY`]).
    pub enum Delivery {
        /// The USB audio of the server from the dial frequency up to 5.4 kHz above it, for the decoders of sdrkit.
        Iq = ("iq", "IQ for sdrkit's decoders"),
        /// The audio of the demodulator of the server.
        Audio = ("audio", "Server demodulates"),
    }
}

sdrkit_core::choice_enum! {
    /// The DMR time slots the server decodes ([`DMR_SLOT`]).
    pub enum DmrSlot {
        /// Slot 1.
        One = ("1", "Slot 1"),
        /// Slot 2.
        Two = ("2", "Slot 2"),
        /// Both slots.
        Both = ("3", "Both"),
    }
}

/// What the stream holds.
pub const DELIVERY: Setting<field::Choice<Delivery>> =
    Setting::choice("delivery", "Delivery", Delivery::Iq)
        .restart()
        .help(
            "IQ for sdrkit's decoders receives 5.4 kHz of spectrum from the USB audio of the server, so the SSB, CW and data \
             decoders of sdrkit run on it. The AGC of the server sets the level. Server demodulates gives the FM, AM, SSB or \
             digital voice audio of the server.",
        );
/// The passband width of the demodulator of the server. 0 takes the width of the mode.
pub const PASSBAND: Setting<field::Hz> =
    Setting::new("passband", "Passband (0 = the mode's)", "Hz", 0.0)
        .range(0.0, 200_000.0, 50.0)
        .help(
            "The passband width of the demodulator of the server. 0 takes the width of the mode.",
        );
/// The squelch of the server. -150 is off.
pub const SQUELCH: Setting<field::F64> = Setting::new("squelch", "Server squelch", "dB", -150.0)
    .range(-150.0, 0.0, 1.0)
    .help("The server mutes its audio below this level. -150 is off.");
/// The audio frequency of a single-signal decoder. 0 lets the server choose.
pub const SECONDARY_OFFSET: Setting<field::Hz> = Setting::new(
    "secondary_offset",
    "Decoder audio frequency (0 = the server's)",
    "Hz",
    0.0,
)
.range(0.0, 6000.0, 10.0)
.help("The audio frequency of a single-signal decoder (BPSK31, RTTY).");
/// The DMR time slots the server decodes.
pub const DMR_SLOT: Setting<field::Choice<DmrSlot>> =
    Setting::choice("dmr_slot", "DMR time slot", DmrSlot::Both)
        .help("The DMR time slots that the server decodes.");
/// Moves the centre frequency of the shared radio for every listener.
pub const MOVE_CENTRE: Setting<field::Bool> =
    Setting::new("move_centre", "Move the server's centre frequency", "", false).help(
        "Retunes the shared radio for every listener, on servers that allow it. Some need the magic key the operator gave you.",
    );
/// The key some servers ask for before a listener can move the centre frequency.
pub const MAGIC_KEY: Setting<field::Text> = Setting::new("magic_key", "Magic key", "", "")
    .help("The key some servers ask for before a listener can move the centre frequency.");
/// The HTTP user of a password-protected web proxy.
pub const USER: Setting<field::Text> = Setting::new("user", "HTTP user (proxy login)", "", "")
    .restart()
    .help("Only for a receiver behind a password-protected web proxy.");
/// The HTTP password of a password-protected web proxy.
pub const PASSWORD: Setting<field::Text> =
    Setting::new("password", "HTTP password (proxy login)", "", "")
        .restart()
        .help(
            "Only for a receiver behind a password-protected web proxy. Over ws:// or http:// the password goes unencrypted.",
        );

sdrkit_core::settings! {
    /// The own settings of the OpenWebRX driver.
    pub enum OwrxSetting {
        /// [`DELIVERY`].
        Delivery = DELIVERY,
        /// [`PASSBAND`].
        Passband = PASSBAND,
        /// [`SQUELCH`].
        Squelch = SQUELCH,
        /// [`SECONDARY_OFFSET`].
        SecondaryOffset = SECONDARY_OFFSET,
        /// [`DMR_SLOT`].
        DmrSlot = DMR_SLOT,
        /// [`MOVE_CENTRE`].
        MoveCentre = MOVE_CENTRE,
        /// [`MAGIC_KEY`].
        MagicKey = MAGIC_KEY,
        /// [`USER`].
        User = USER,
        /// [`PASSWORD`].
        Password = PASSWORD,
    }
}

// ---- Readings ----

/// The name and version of the server.
pub const SERVER: Field<field::Text> = Field::new("server", "Server", "");
/// The id of the profile in use, `<sdr id>|<profile id>`.
pub const PROFILE_IN_USE: Field<field::Text> = Field::new("profile_id", "Profile id", "");
/// The name of the profile in use.
pub const PROFILE_NAME: Field<field::Text> = Field::new("profile_name", "Profile", "");
/// The low edge of the band of the server.
pub const BAND_LOW: Field<field::Hz> = Field::new("band_low_hz", "Band from", "Hz");
/// The high edge of the band of the server.
pub const BAND_HIGH: Field<field::Hz> = Field::new("band_high_hz", "Band to", "Hz");
/// The listeners of the server.
pub const LISTENERS: Field<field::I64> = Field::new("listeners", "Listeners", "");
/// The most listeners the server takes.
pub const MAX_LISTENERS: Field<field::I64> = Field::new("max_listeners", "Most listeners", "");
/// The S-meter of the server.
pub const SMETER: Field<field::F64> = Field::new("smeter_db", "S-meter", "dB");
/// The CPU load of the server.
pub const CPU: Field<field::F64> = Field::new("cpu_pct", "Server CPU", "%");
/// The temperature of the server.
pub const TEMPERATURE: Field<field::F64> = Field::new("temperature_c", "Temperature", "°C");
/// The name of the receiver.
pub const RECEIVER: Field<field::Text> = Field::new("receiver", "Receiver", "");
/// Where the receiver is, as its operator writes it.
pub const LOCATION: Field<field::Text> = Field::new("location", "Location", "");
/// The height of the receiver above mean sea level.
pub const HEIGHT: Field<field::F64> = Field::new("height_m", "Height", "m");
/// The operator of the receiver.
pub const ADMIN: Field<field::Text> = Field::new("admin", "Admin", "");
/// The id of the map point of the receiver.
pub const RECEIVER_POINT: &str = "receiver";
/// The line that the decoder of the server writes now.
pub const DECODER_LINE: Field<field::Text> = Field::new("decoder_line", "Server decoder", "");
/// The last error that the server reported about its radio or its demodulator.
pub const SERVER_ERROR: Field<field::Text> = Field::new("server_error", "Server error", "");
/// The link speed over the last throughput window, all WebSocket bytes.
pub const KBIT_S: Field<field::F64> = Field::new("kbit_s", "Link speed", "kbit/s");
/// Audio samples per second over the last throughput window.
pub const AUDIO_SPS: Field<field::F64> = Field::new("audio_sps", "Audio", "samples/s");
/// Waterfall lines per second over the last throughput window.
pub const LINES_S: Field<field::F64> = Field::new("lines_s", "Waterfall", "lines/s");
/// Bytes received on this connection.
pub const BYTES_IN: Field<field::I64> = Field::new("bytes_in", "Bytes received", "");
/// The audio is ADPCM.
pub const AUDIO_ADPCM: Field<field::Bool> = Field::new("audio_adpcm", "Audio ADPCM", "");
/// The ADPCM audio carries sync headers.
pub const AUDIO_SYNC: Field<field::Bool> = Field::new("audio_sync", "Audio sync headers", "");
/// The waterfall is ADPCM.
pub const WATERFALL_ADPCM: Field<field::Bool> =
    Field::new("waterfall_adpcm", "Waterfall ADPCM", "");
/// The bins of a waterfall line.
pub const FFT_BINS: Field<field::I64> = Field::new("fft_bins", "Waterfall bins", "");
/// The bottom of the display range of the waterfall that the server suggests.
pub const WATERFALL_MIN: Field<field::F64> = Field::new("waterfall_min_db", "Waterfall from", "dB");
/// The top of the display range of the waterfall that the server suggests.
pub const WATERFALL_MAX: Field<field::F64> = Field::new("waterfall_max_db", "Waterfall to", "dB");
/// Audio samples lost when the ADPCM stream lost its sync.
pub const RESYNC_LOST: Field<field::I64> = Field::new("resync_lost", "Lost to resyncs", "");

/// The readings of an OpenWebRX radio.
pub const READINGS: &[&dyn Declare] = &[
    &SERVER,
    &PROFILE_IN_USE,
    &PROFILE_NAME,
    &BAND_LOW,
    &BAND_HIGH,
    &LISTENERS,
    &MAX_LISTENERS,
    &SMETER,
    &CPU,
    &TEMPERATURE,
    &RECEIVER,
    &LOCATION,
    &HEIGHT,
    &ADMIN,
    &DECODER_LINE,
    &SERVER_ERROR,
    &KBIT_S,
    &AUDIO_SPS,
    &LINES_S,
    &BYTES_IN,
    &AUDIO_ADPCM,
    &AUDIO_SYNC,
    &WATERFALL_ADPCM,
    &FFT_BINS,
    &WATERFALL_MIN,
    &WATERFALL_MAX,
    &RESYNC_LOST,
];

// ---- Tables and rasters ----

/// The name of a bookmark.
pub const B_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The frequency of a bookmark, a demodulator frequency.
pub const B_FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
/// The mode of a bookmark, as the server names it.
pub const B_MODE: Field<field::Text> = Field::new("mode", "Mode", "");
/// The radio frequency that puts a bookmark in the stream: B/2 above it in the `iq` delivery, the bookmark itself in the
/// `audio` delivery.
pub const B_TUNE: Field<field::Hz> = Field::new("tune_hz", "Tune to", "Hz");

/// The bookmarks of the server (key: the frequency and the name).
pub static BOOKMARKS: TableDef = TableDef {
    key: "bookmarks",
    label: "Bookmarks",
    columns: &[&B_NAME, &B_FREQ, &B_MODE, &B_TUNE],
    cap: view::MAX_LIST,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The mode of a dial frequency.
pub const D_MODE: Field<field::Text> = Field::new("mode", "Mode", "");
/// A dial frequency.
pub const D_FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");

/// The dial frequencies of the digital modes of the server (key: the mode and the frequency).
pub static DIAL_FREQUENCIES: TableDef = TableDef {
    key: "dial_frequencies",
    label: "Dial frequencies",
    columns: &[&D_MODE, &D_FREQ],
    cap: view::MAX_LIST,
    marks: sdrkit_core::RowMarks::NONE,
};

/// When a decode came.
pub const C_TIME: Field<field::Time> = Field::new("time", "Time", "");
/// The mode of a decode.
pub const C_MODE: Field<field::Text> = Field::new("mode", "Mode", "");
/// The frequency of a decode, as the decoder produces it.
pub const C_FREQ: Field<field::F64> = Field::new("freq", "Frequency", "");
/// The level of a decode.
pub const C_LEVEL: Field<field::F64> = Field::new("level_db", "Level", "dB");
/// The time offset of a decode.
pub const C_OFFSET: Field<field::Duration> = Field::new("offset", "Offset", "s");
/// The message of a decode.
pub const C_MESSAGE: Field<field::Text> = Field::new("message", "Message", "");
/// The other keys of a decode, as a JSON object.
pub const C_REST: Field<field::Text> = Field::new("rest", "More", "");

/// The decodes of the decoders of the server, the newest last (key: a number that counts up).
pub static DECODES: TableDef = TableDef {
    key: "decodes",
    label: "Server decodes",
    columns: &[
        &C_TIME, &C_MODE, &C_FREQ, &C_LEVEL, &C_OFFSET, &C_MESSAGE, &C_REST,
    ],
    cap: view::MAX_DECODES,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The id of a profile, `<sdr id>|<profile id>`.
pub const P_ID: Field<field::Text> = Field::new("id", "Id", "");
/// The name of a profile.
pub const P_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The low edge of the band of a profile, once the driver has seen it.
pub const P_LOW: Field<field::Hz> = Field::new("low_hz", "From", "Hz");
/// The high edge of the band of a profile, once the driver has seen it.
pub const P_HIGH: Field<field::Hz> = Field::new("high_hz", "To", "Hz");

/// The profiles of the server (key: the id).
pub static PROFILES: TableDef = TableDef {
    key: "profiles",
    label: "Profiles",
    columns: &[&P_ID, &P_NAME, &P_LOW, &P_HIGH],
    cap: view::MAX_LIST,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The id of a mode.
pub const M_ID: Field<field::Text> = Field::new("id", "Id", "");
/// The name of a mode.
pub const M_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The mode is a decoder that runs on the audio of a demodulator.
pub const M_DECODER: Field<field::Bool> = Field::new("decoder", "Decoder", "");
/// The demodulators a decoder runs on, by id, separated by commas.
pub const M_UNDERLYING: Field<field::Text> = Field::new("underlying", "Runs on", "");
/// The server can run the mode now.
pub const M_AVAILABLE: Field<field::Bool> = Field::new("available", "Available", "");

/// The demodulators and decoders of the server (key: the id).
pub static MODES: TableDef = TableDef {
    key: "modes",
    label: "Modes",
    columns: &[&M_ID, &M_NAME, &M_DECODER, &M_UNDERLYING, &M_AVAILABLE],
    cap: view::MAX_LIST,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The name of a digital voice value.
pub const V_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// A digital voice value.
pub const V_VALUE: Field<field::Text> = Field::new("value", "Value", "");

/// What the digital voice decoder of the server reports (key: the name).
pub static METADATA: TableDef = TableDef {
    key: "metadata",
    label: "Digital voice",
    columns: &[&V_NAME, &V_VALUE],
    cap: view::MAX_METADATA,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The waterfall of the band of the server: one row per line, the newest last, in dB.
pub static WATERFALL: RasterDef = RasterDef {
    key: "waterfall",
    label: "Waterfall",
    x_unit: "Hz",
    y_unit: "line",
    unit: "dB",
};

/// The spectrum of the decoder of the server: one row per line, the newest last, in dB, about the audio frequency 0.
pub static DECODER_WATERFALL: RasterDef = RasterDef {
    key: "decoder_waterfall",
    label: "Decoder spectrum",
    x_unit: "Hz",
    y_unit: "line",
    unit: "dB",
};

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "openwebrx",
    name: "OpenWebRX receiver",
    opens: sdrkit_driver_core::Opens::Many,
    addresses: &[
        "openwebrx:<host> (port 8073)",
        "openwebrx:<host>:<port>",
        "openwebrx:https://<host>/<path>",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(conn::DEFAULT_PORT)),
        sdrkit_driver_core::AddressPart::text(
            "path",
            "Path",
            "/",
            sdrkit_driver_core::Syntax::UrlPath,
        )
        .help("The path of the receiver on its web server. Empty for a receiver at the root."),
        sdrkit_driver_core::AddressPart::switch("tls", "Secure connection", "https://")
            .help("Connects with TLS, as an https link does."),
    ],
    settings: &[OwrxSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[StreamFormat {
        kind: Kind::Real,
        sample: SampleType::I16,
        min_rate_hz: proto::OUTPUT_RATE as f64,
        max_rate_hz: proto::HD_OUTPUT_RATE as f64,
        channels: 1,
    }],
    accepts: &[],
    tables: &[
        &BOOKMARKS,
        &DIAL_FREQUENCIES,
        &DECODES,
        &PROFILES,
        &MODES,
        &METADATA,
    ],
    rasters: &[&WATERFALL, &DECODER_WATERFALL],
    series: &[],
    points: "Receiver",
    kind_commands: &[],
    kind_tables: &[],
};

// ---- Errors ----

/// The errors of an OpenWebRX server that the driver core has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OwrxError {
    /// The web server answered, but has no OpenWebRX WebSocket at the path.
    #[error("the web server has no OpenWebRX WebSocket at this path (HTTP {status})")]
    NoWebSocket {
        /// The HTTP status.
        status: u16,
    },
    /// The web server requires a login and none is set.
    #[error("the web server asks for a login: set the HTTP user and password")]
    LoginNeeded,
    /// The web server refused the user or the password.
    #[error("the web server refused the HTTP user or password")]
    LoginRefused,
    /// The web server answered with a status that refuses the connection.
    #[error("the web server refused the connection (HTTP {status})")]
    Http {
        /// The HTTP status.
        status: u16,
    },
    /// More than two redirects in a row.
    #[error("too many redirects")]
    TooManyRedirects,
}

// ---- Timing ----

/// Timings, for tests and slow links.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// The TCP connect wait for a remote host (8 s).
    pub connect_remote: Duration,
    /// The TCP connect wait for this device (10 s).
    pub connect_local: Duration,
    /// The wait for the answer of the upgrade (10 s).
    pub upgrade: Duration,
    /// The wait for the greeting of the server (10 s).
    pub greeting: Duration,
    /// The wait for the first usable `config` after the greeting (15 s).
    pub config: Duration,
    /// The wait for `config` after `selectprofile` (5 s).
    pub profile_wait: Duration,
    /// The wait for `config` after `setfrequency` (3 s).
    pub centre_wait: Duration,
    /// The wait for the profile list when a named profile is set (1 s).
    pub profiles_wait: Duration,
    /// No frame for this long ends the read loop (10 s).
    pub stall: Duration,
    /// A waterfall but no audio for this long after the start goes to the log once (5 s).
    pub no_audio: Duration,
    /// A ping after this long without a send (15 s).
    pub keepalive: Duration,
    /// How long a close waits for the close of the server (1 s).
    pub close_wait: Duration,
    /// The first backoff wait (16 s).
    pub backoff_base: Duration,
    /// The longest backoff wait (512 s).
    pub backoff_max: Duration,
    /// The read timeout of the connection, in which the read loop looks for a stop (100 ms).
    pub tick: Duration,
    /// The throughput window (1 s).
    pub window: Duration,
    /// The gap between reconnect attempts (1 s).
    pub reconnect_gap: Duration,
    /// The gap between open attempts at the start (500 ms).
    pub retry_gap: Duration,
    /// A fixed masking key for every client frame (tests only).
    pub fixed_mask: Option<[u8; 4]>,
    /// A fixed `Sec-WebSocket-Key` (tests only).
    pub fixed_key: Option<[u8; 16]>,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect_remote: Duration::from_secs(8),
            connect_local: Duration::from_secs(10),
            upgrade: Duration::from_secs(10),
            greeting: Duration::from_secs(10),
            config: Duration::from_secs(15),
            profile_wait: Duration::from_secs(5),
            centre_wait: Duration::from_secs(3),
            profiles_wait: Duration::from_secs(1),
            stall: Duration::from_secs(10),
            no_audio: Duration::from_secs(5),
            keepalive: Duration::from_secs(15),
            close_wait: Duration::from_secs(1),
            backoff_base: Duration::from_secs(16),
            backoff_max: Duration::from_secs(512),
            tick: Duration::from_millis(100),
            window: Duration::from_secs(1),
            reconnect_gap: Duration::from_secs(1),
            retry_gap: Duration::from_millis(500),
            fixed_mask: None,
            fixed_key: None,
        }
    }
}

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---- The driver ----

/// The OpenWebRX driver. It keeps, per server address, what it learnt from the server: the bands of the profiles, the
/// lists of the server and the backoff wait.
#[derive(Clone)]
pub struct OpenWebRxDriver {
    transport: Arc<dyn Transport>,
    clock: Clock,
    addrs: Arc<Mutex<HashMap<String, Arc<view::AddrState>>>>,
    timing: Timing,
}

impl OpenWebRxDriver {
    /// A driver that reaches its servers through `transport` and reads the time of day from `clock`. For `https` and
    /// `wss` addresses, the transport must give TLS connections.
    pub fn new(transport: Arc<dyn Transport>, clock: Clock) -> Self {
        Self::with_timing(transport, clock, Timing::default())
    }

    /// A driver with its own timings.
    pub fn with_timing(transport: Arc<dyn Transport>, clock: Clock, timing: Timing) -> Self {
        OpenWebRxDriver {
            transport,
            clock,
            addrs: Arc::default(),
            timing,
        }
    }

    fn addr_state(&self, key: &str) -> Arc<view::AddrState> {
        lock(&self.addrs)
            .entry(key.to_string())
            .or_default()
            .clone()
    }
}

impl SourceDriver for OpenWebRxDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let target = conn::parse_address(address)?;
        let state = self.addr_state(&target.key());
        let opts = {
            let v = lock(&state.view);
            let opts = radio::Opts::from_values(values, &v.lists());
            v.check(&opts)?;
            opts
        };
        let label = target.label_at(target.port());
        let at = radio::At {
            transport: self.transport.clone(),
            clock: self.clock.clone(),
            target,
            timing: self.timing.clone(),
        };
        let st = state.clone();
        let opener: Opener<OwrxRadio> = Arc::new(move |values, abort, notices| {
            radio::connect(&at, &st, values, abort, notices)
        });
        let mut cfg = RunnerConfig::new(format!("OpenWebRX {label}"), NETWORK_ADVICE);
        cfg.reconnect_gap = self.timing.reconnect_gap;
        cfg.retry_gap = self.timing.retry_gap;
        cfg.hooks = Some(Arc::new(OwrxHooks { state, opts }));
        Ok(Box::new(StreamRunner::open(cfg, opener, values)?))
    }
}

/// What the runner requests from the driver when no radio is open: a restart check and the resync losses.
#[derive(Debug)]
struct OwrxHooks {
    state: Arc<view::AddrState>,
    opts: radio::Opts,
}

impl RunnerHooks for OwrxHooks {
    fn restart_needed(&self, values: &Values) -> bool {
        let o = radio::Opts::from_values(values, &lock(&self.state.view).lists());
        o.restart_part() != self.opts.restart_part()
    }
    fn extra_loss(&self) -> (u64, u64) {
        self.state.resync_lost()
    }
}

#[cfg(test)]
mod tests;
