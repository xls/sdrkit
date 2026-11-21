//! The rtl_tcp client, on the shared base of the network radios. A server sends a 12-byte greeting (magic, tuner type, gain
//! count), then unsigned 8-bit I/Q without end. A command is 5 bytes: a command byte and a big-endian 32-bit value. Every change
//! is live, the rate too. Some servers stream at their own rate whatever rate is requested: the driver measures what arrives and
//! holds the radio to that rate.

use crate::net::{self, Abort};
use crate::runner::{
    Notices, Opener, Radio, RadioStream, RunnerConfig, Sink, StreamCtx, StreamRunner,
    NETWORK_ADVICE,
};
use crate::{
    own_id, Capabilities, DriverDescriptor, DriverError, Protocol, RadioSetting, RateLimits,
    Source, SourceDriver, StreamFormat, AGC, BIAS_TEE, CENTRE, GAIN, RATE, RATE_EXACT,
};
use sdrkit_core::net::{host_port, is_local, show, timed_out, Conn, NetError, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Hz, Setting, SettingSet, Value, Values};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The rtl_tcp default port.
pub const DEFAULT_PORT: u16 = 1234;

/// Tuner types from the greeting.
pub const TUNERS: [&str; 6] = ["E4000", "FC0012", "FC0013", "FC2580", "R820T", "R828D"];

const GAINS_E4000: &[i32] = &[
    -10, 15, 40, 65, 90, 115, 140, 165, 190, 215, 240, 290, 340, 420,
];
const GAINS_FC0012: &[i32] = &[-99, -40, 71, 179, 192];
const GAINS_FC0013: &[i32] = &[
    -99, -73, -65, -63, -60, -58, -54, 58, 61, 63, 65, 67, 68, 70, 71, 179, 181, 182, 184, 186,
    188, 191, 197,
];
const GAINS_FC2580: &[i32] = &[0];
const GAINS_R820T: &[i32] = &[
    0, 9, 14, 27, 37, 77, 87, 125, 144, 157, 166, 197, 207, 229, 254, 280, 297, 328, 338, 364, 372,
    386, 402, 421, 434, 439, 445, 480, 496,
];

/// The highest RTL-SDR rate without loss, and the top of what an RTL-SDR is allowed.
const RTL_STEADY: Hz = 2.56e6;
const RTL_MAX: Hz = 3.2e6;
/// The rates of an RTL-SDR that people normally use. An open radio offers those inside its rate ranges. 900 kHz itself
/// lies just below the upper range of the RTL2832U, so the list has the lowest rate of that range.
const COMMON_RATES: &[Hz] = &[900_001.0, 1.024e6, 1.4e6, 1.8e6, 2.048e6, 2.4e6, 2.56e6];
const OTHER_MAX: Hz = 20e6;
const ANY_MAX: Hz = 40e6;
const DEFAULT_RATE: Hz = 2.048e6;
/// The rtl_tcp protocol takes the frequency as 32-bit Hz.
const MAX_FREQ: Hz = 4.29e9;

/// The rtl_tcp commands the client sends.
pub mod cmd {
    /// Centre frequency, Hz.
    pub const FREQ: u8 = 0x01;
    /// Sample rate, sps.
    pub const RATE: u8 = 0x02;
    /// Gain mode (0 auto, 1 manual).
    pub const GAIN_MODE: u8 = 0x03;
    /// Tuner gain, tenths of a dB.
    pub const GAIN: u8 = 0x04;
    /// Frequency correction, ppm.
    pub const PPM: u8 = 0x05;
    /// RTL2832 digital AGC.
    pub const AGC: u8 = 0x08;
    /// Direct sampling (0 off, 1 I, 2 Q).
    pub const DIRECT: u8 = 0x09;
    /// Bias tee.
    pub const BIAS: u8 = 0x0e;
}

/// A command on the wire: the byte, then the value big endian (a negative value as its two's complement).
pub fn command(c: u8, value: u32) -> [u8; 5] {
    let v = value.to_be_bytes();
    [c, v[0], v[1], v[2], v[3]]
}

/// The server's greeting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Greeting {
    /// 4 characters (`RTL0` for a real rtl_tcp).
    pub magic: [u8; 4],
    /// The tuner type (1 to 6 known).
    pub tuner: u32,
    /// The number of gains.
    pub gains: u32,
}

impl Greeting {
    /// Reads the 12 bytes. Not printable magic, or a tuner type or gain count above 0xFFFF, is not an rtl_tcp server.
    pub fn parse(b: &[u8; 12]) -> Option<Greeting> {
        let magic = [b[0], b[1], b[2], b[3]];
        let tuner = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
        let gains = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
        (magic.iter().all(|c| (0x20..=0x7E).contains(c)) && tuner <= 0xFFFF && gains <= 0xFFFF)
            .then_some(Greeting {
                magic,
                tuner,
                gains,
            })
    }
    /// Another radio behind an rtl_tcp server: the magic is not `RTL0`, or the tuner is not 1 to 6.
    pub fn is_other(&self) -> bool {
        &self.magic != b"RTL0" || !(1..=6).contains(&self.tuner)
    }
    /// The tuner's name.
    pub fn tuner_name(&self) -> &'static str {
        TUNERS
            .get((self.tuner as usize).wrapping_sub(1))
            .copied()
            .unwrap_or("unknown tuner")
    }
    /// The tuner's gain list, tenths of a dB.
    pub fn gain_table(&self) -> &'static [i32] {
        match self.tuner {
            1 => GAINS_E4000,
            2 => GAINS_FC0012,
            3 => GAINS_FC0013,
            4 => GAINS_FC2580,
            _ => GAINS_R820T,
        }
    }
}

/// The list entry nearest `target` (tenths of a dB); on a tie the earlier (lower) one.
pub fn nearest_gain(table: &[i32], target: i32) -> i32 {
    let mut best = table[0];
    for &g in table {
        if (g - target).abs() < (best - target).abs() {
            best = g;
        }
    }
    best
}

sdrkit_core::choice_enum! {
    /// How the server is treated ([`SERVER`]).
    pub enum ServerMode {
        /// Like an RTL-SDR unless the greeting or the requested rate says otherwise.
        Auto = ("auto", "Auto"),
        /// A real RTL-SDR.
        Rtl = ("rtl", "RTL-SDR (up to 2.56 Msps)"),
        /// Another radio behind rtl_tcp.
        Other = ("other", "Other radio (up to 20 Msps)"),
    }
}

sdrkit_core::choice_enum! {
    /// Direct sampling ([`DIRECT_SAMPLING`]).
    pub enum DirectSampling {
        /// Never sent: the library of the server decides.
        Auto = ("auto", "Auto"),
        /// Off.
        Off = ("off", "Off"),
        /// The I input below 24 MHz.
        I = ("i", "I input"),
        /// The Q input below 24 MHz.
        Q = ("q", "Q input"),
    }
}

/// How the server is treated.
pub const SERVER: Setting<field::Choice<ServerMode>> =
    Setting::choice("server", "Server radio", ServerMode::Auto).help(
        "RTL-SDR: a real RTL-SDR, which drops samples above 2.56 Msps. Other radio: any rate from 0.25 to 20 Msps. Auto: an \
         RTL-SDR unless the server greets as another radio or the stack needs more than 2.56 Msps.",
    );
/// Allow any rate from 0.1 to 40 Msps.
pub const ANY_RATE: Setting<field::Bool> = Setting::new("anyrate", "Allow any sample rate", "", false)
    .section("advanced")
    .help(
        "Only limits of 0.1 to 40 Msps. 8-bit I/Q at 10 Msps is 160 Mbit/s, more than most Wi-Fi gives. Lost samples show in \
         the status with the measured speed.",
    );
/// The rate with [`ANY_RATE`]. 0 takes the rate of the stack.
pub const CUSTOM_RATE: Setting<field::Hz> =
    Setting::new("customrate", "Custom sample rate", "Hz", 0.0)
        .range(0.0, 40e6, 1e5)
        .help("Applies only with Allow any sample rate. 0 takes the rate the stack needs.");
/// The frequency correction.
pub const PPM: Setting<field::F64> = Setting::new("ppm", "Frequency correction", "ppm", 0.0)
    .range(-200.0, 200.0, 1.0)
    .help(
        "The clock error in ppm, positive if the clock runs fast (signals show up a little low). Dongles without a TCXO are often \
         20 to 100 ppm off.",
    );
/// Direct sampling below 24 MHz.
pub const DIRECT_SAMPLING: Setting<field::Choice<DirectSampling>> =
    Setting::choice("ds", "Direct sampling (HF)", DirectSampling::Auto).help(
        "I or Q samples HF directly below 24 MHz (a V3 has its HF input on Q). Auto leaves it to the server: a Blog V4 upconverts \
         HF itself.",
    );
/// The rate a server keeps whatever rate is requested. The driver sets it. 0: none found.
pub const SERVER_RATE: Setting<field::Hz> = Setting::new(
    "server_rate",
    "Rate of the server",
    "Hz",
    0.0,
)
.range(0.0, 40e6, 1.0)
.section("advanced")
.help(
    "The rate the server sends whatever it is asked. The driver finds it. 0 means none was found.",
);

sdrkit_core::settings! {
    /// The own settings of the rtl_tcp driver.
    pub enum RtlSetting {
        /// [`SERVER`].
        Server = SERVER,
        /// [`ANY_RATE`].
        AnyRate = ANY_RATE,
        /// [`CUSTOM_RATE`].
        CustomRate = CUSTOM_RATE,
        /// [`PPM`].
        Ppm = PPM,
        /// [`DIRECT_SAMPLING`].
        DirectSampling = DIRECT_SAMPLING,
        /// [`SERVER_RATE`].
        ServerRate = SERVER_RATE,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "rtl_tcp",
    name: "rtl_tcp server",
    opens: crate::Opens::Once,
    addresses: &["rtl_tcp:<host> (port 1234)", "rtl_tcp:<host>:<port>"],
    address: &[
        crate::AddressPart::host(true),
        crate::AddressPart::port(Some(DEFAULT_PORT)),
    ],
    settings: &[RtlSetting::SETTINGS],
    commands: &[],
    readings: &[],
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cu8,
        min_rate_hz: 225_001.0,
        max_rate_hz: ANY_MAX,
        channels: 1,
    }],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The rtl_tcp settings of a radio, read from its values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The `server` setting.
    pub server: ServerMode,
    /// Allow any sample rate.
    pub anyrate: bool,
    /// The custom rate in Hz (0: the rate of the stack).
    pub customrate_hz: Hz,
    /// Frequency correction.
    pub ppm: f64,
    /// Direct sampling.
    pub ds: DirectSampling,
    /// The rate the server holds to, whole Hz.
    pub pinned: Option<u32>,
}

impl Options {
    /// Reads the settings from `v`.
    pub fn from_values(v: &Values) -> Options {
        let pinned = Some(v.get(&SERVER_RATE))
            .filter(|r| *r >= 1.0)
            .map(|r| r.round() as u32);
        Options {
            server: v.get(&SERVER),
            anyrate: v.get(&ANY_RATE),
            customrate_hz: v.get(&CUSTOM_RATE),
            ppm: v.get(&PPM),
            ds: v.get(&DIRECT_SAMPLING),
            pinned,
        }
    }
}

/// How the rate is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateClass {
    /// Allow any sample rate.
    Any,
    /// Another radio.
    Other,
    /// A real RTL-SDR.
    Rtl,
}

/// The rate to send, and how the server is treated.
#[derive(Clone, Debug, PartialEq)]
pub struct RatePlan {
    /// Sent with command 0x02.
    pub rate: u32,
    /// How the server is treated.
    pub class: RateClass,
    /// A one-time log note (Auto taken as other because of the requested rate).
    pub note: Option<String>,
}

/// Chooses the rate to send from the settings, the values of the radio and whether the greeting said "other".
///
/// # Errors
///
/// [`DriverError::Refused`] for [`RATE`] if the rate is above what this kind of server returns and [`RATE_EXACT`] is off.
pub fn plan_rate(o: &Options, v: &Values, greeting_other: bool) -> Result<RatePlan, DriverError> {
    let rate_chosen = v.get(&RATE_EXACT);
    let mode_rate = Some(v.get(&RATE)).filter(|r| *r > 0.0);
    let wanted = if o.anyrate && o.customrate_hz > 0.0 {
        Some(o.customrate_hz)
    } else {
        mode_rate
    };
    let mut note = None;
    let class = if o.anyrate {
        RateClass::Any
    } else {
        match o.server {
            ServerMode::Rtl => RateClass::Rtl,
            ServerMode::Other => RateClass::Other,
            ServerMode::Auto if greeting_other => RateClass::Other,
            ServerMode::Auto => match wanted {
                Some(w) if w > RTL_STEADY * 1.0001 => {
                    note = Some(format!(
                        "the server greets as an RTL-SDR but {} Msps was asked for; samples may drop if this really is an RTL-SDR",
                        crate::fmt_m(w)
                    ));
                    RateClass::Other
                }
                _ => RateClass::Rtl,
            },
        }
    };
    if let Some(p) = o.pinned {
        return Ok(RatePlan {
            rate: p,
            class,
            note: None,
        });
    }
    let max = match class {
        RateClass::Any => ANY_MAX,
        RateClass::Other => OTHER_MAX,
        RateClass::Rtl => RTL_MAX,
    };
    if let Some(w) = wanted {
        if w > max * 1.0001 && !rate_chosen {
            return Err(DriverError::Refused {
                id: RadioSetting::Rate.id(),
            });
        }
    }
    let rate = match class {
        RateClass::Any => wanted.unwrap_or(DEFAULT_RATE).clamp(0.1e6, ANY_MAX),
        RateClass::Other => mode_rate.unwrap_or(DEFAULT_RATE).clamp(0.25e6, OTHER_MAX),
        RateClass::Rtl if rate_chosen && mode_rate.is_some() => mode_rate.unwrap_or(DEFAULT_RATE),
        RateClass::Rtl => {
            let r = mode_rate.unwrap_or(DEFAULT_RATE).min(RTL_STEADY);
            if r < 900_001.0 && (r > 300_000.0 || r <= 225_000.0) {
                900_001.0
            } else {
                r
            }
        }
    };
    Ok(RatePlan {
        rate: rate.round().clamp(1.0, u32::MAX as f64) as u32,
        class,
        note,
    })
}

/// The rates a server that ignores the rate command is snapped to (Msps).
pub const COMMON_RATES_MSPS: [f64; 25] = [
    0.25, 0.9, 1.0, 1.024, 1.4, 1.536, 1.792, 1.92, 2.0, 2.048, 2.16, 2.4, 2.5, 2.56, 2.88, 3.0,
    3.2, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0,
];

/// Snaps a measured rate: the nearest common rate within 2 %, else rounded to 10 kHz.
pub fn snap_server_rate(measured: Hz) -> Hz {
    let near = COMMON_RATES_MSPS
        .iter()
        .map(|m| m * 1e6)
        .min_by(|a, b| (a - measured).abs().total_cmp(&(b - measured).abs()));
    match near {
        Some(c) if (c - measured).abs() <= 0.02 * c => c,
        _ => (measured / 1e4).round() * 1e4,
    }
}

/// Decides, from throughput readings, whether a server streams at its own rate whatever rate is requested.
#[derive(Clone, Debug)]
pub struct PinDetector {
    skip: u32,
    readings: Vec<Hz>,
    last_total: u64,
    last_asked: Hz,
}

impl Default for PinDetector {
    fn default() -> Self {
        PinDetector {
            skip: 2,
            readings: Vec::new(),
            last_total: 0,
            last_asked: 0.0,
        }
    }
}

impl PinDetector {
    /// One reading (a new throughput window): the achieved rate, the requested rate, and the bytes since connect. Returns the
    /// rate to pin when the server is found not to obey.
    pub fn reading(
        &mut self,
        achieved: Hz,
        asked: Hz,
        total_bytes: u64,
        pinned: Option<Hz>,
    ) -> Option<Hz> {
        if total_bytes < self.last_total || (asked - self.last_asked).abs() > 0.5 {
            // A fresh start: skip the backlog and the old rate.
            self.skip = 2;
            self.readings.clear();
        }
        self.last_total = total_bytes;
        self.last_asked = asked;
        if self.skip > 0 {
            self.skip -= 1;
            return None;
        }
        self.readings.push(achieved);
        if self.readings.len() < 3 {
            return None;
        }
        let mut r = std::mem::take(&mut self.readings);
        r.sort_by(f64::total_cmp);
        let median = r[1];
        if asked > 0.0 && (median - asked).abs() <= 0.04 * asked {
            return None;
        }
        let snapped = snap_server_rate(median);
        if pinned == Some(snapped) && snapped == asked {
            return None;
        }
        Some(snapped)
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throughput {
    /// Bits per second, in millions.
    pub mbit_s: f64,
    /// Samples per second that arrived (bytes / 2 / seconds).
    pub achieved_hz: Hz,
    /// The requested rate.
    pub asked_hz: Hz,
    /// Bytes since connect.
    pub total_bytes: u64,
    /// Samples lost on the radio side, x 2.
    pub lost_bytes: u64,
}

/// Per server address: what the driver learnt, shared by every connection to it.
#[derive(Debug, Default)]
struct AddrState {
    found_other: AtomicBool,
    pinned: Mutex<Option<u32>>,
    last: Mutex<Option<Throughput>>,
    window: Mutex<Window>,
    detector: Mutex<PinDetector>,
}

#[derive(Debug, Default)]
struct Window {
    start: Option<Instant>,
    bytes: u64,
    total: u64,
    asked: Hz,
}

/// The rtl_tcp driver. It remembers, per server address, servers found to be "other", the last throughput, and held rates.
#[derive(Clone)]
pub struct RtlTcpDriver {
    transport: Arc<dyn Transport>,
    addrs: Arc<Mutex<HashMap<String, Arc<AddrState>>>>,
    window: Duration,
    stall: Duration,
}

impl RtlTcpDriver {
    /// A driver that reaches its servers through `transport`, with a throughput window of 1 s and a stall timeout of 3 s.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Duration::from_secs(1), Duration::from_secs(3))
    }

    fn addr_state(&self, key: &str) -> Arc<AddrState> {
        self.addrs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(key.to_string())
            .or_default()
            .clone()
    }

    /// The last throughput window of a server address, if any.
    pub fn throughput(&self, address: &str) -> Option<Throughput> {
        let (h, p) = host_port(address, Some(DEFAULT_PORT)).ok()?;
        let state = self
            .addrs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&show(h, p))?
            .clone();
        let last = *lock(&state.last);
        last
    }

    /// A driver with its own throughput window and stall timeout of a remote server, for tests and for links that need other
    /// timings. The detection of a server that holds its own rate counts in windows.
    pub fn with_timing(transport: Arc<dyn Transport>, window: Duration, stall: Duration) -> Self {
        RtlTcpDriver {
            transport,
            addrs: Arc::default(),
            window,
            stall,
        }
    }
}

impl SourceDriver for RtlTcpDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }
    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) =
            host_port(address, Some(DEFAULT_PORT)).map_err(|_| DriverError::Address)?;
        let label = show(host, port);
        let state = self.addr_state(&label);
        plan_rate(
            &Options::from_values(values),
            values,
            state.found_other.load(Ordering::Relaxed),
        )?;
        let link = Link {
            transport: self.transport.clone(),
            host: host.to_owned(),
            port,
            label: label.clone(),
            window: self.window,
            stall: self.stall,
        };
        let opener: Opener<RtlRadio> = Arc::new(move |values, abort, notices| {
            RtlRadio::connect(&link, state.clone(), values, abort, notices)
        });
        let runner = StreamRunner::open(
            RunnerConfig::new(format!("rtl_tcp {label}"), NETWORK_ADVICE),
            opener,
            values,
        )?;
        Ok(Box::new(runner))
    }
}

/// Where a server is and how the driver waits on it.
struct Link {
    transport: Arc<dyn Transport>,
    host: String,
    port: u16,
    label: String,
    window: Duration,
    stall: Duration,
}

/// One connection to an rtl_tcp server.
pub struct RtlRadio {
    label: String,
    sock: Box<dyn Conn>,
    greeting: Greeting,
    local: bool,
    sent: HashMap<u8, u32>,
    bias_on: bool,
    class: RateClass,
    opts: Options,
    rate: u32,
    centre: Hz,
    noted_auto: bool,
    state: Arc<AddrState>,
    notices: Notices,
    window: Duration,
    stall: Duration,
    closed: bool,
}

impl RtlRadio {
    fn connect(
        link: &Link,
        state: Arc<AddrState>,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<RtlRadio, DriverError> {
        let label = &link.label;
        let local = is_local(&link.host);
        let timeout = Duration::from_secs(if local { 10 } else { 8 });
        let mut sock = net::connect(
            &link.transport,
            &link.host,
            link.port,
            false,
            timeout,
            abort,
        )?;
        net::recv_buffer(sock.as_mut(), &[16 << 20, 8 << 20, 4 << 20]);
        let mut g = [0u8; 12];
        let _ = sock.set_read_timeout(Some(Duration::from_secs(10)));
        // A server that sends no greeting serves another client.
        sock.read_exact(&mut g).map_err(|_| DriverError::Busy)?;
        let greeting = Greeting::parse(&g).ok_or(DriverError::Protocol(Protocol::NotThisRadio))?;
        if greeting.is_other() {
            state.found_other.store(true, Ordering::Relaxed);
        }
        notices.log(format!(
            "rtl_tcp {label}: {} ({}, {} gains)",
            String::from_utf8_lossy(&greeting.magic),
            greeting.tuner_name(),
            greeting.gains
        ));
        // A new connection: the throughput totals start again.
        *lock(&state.window) = Window::default();
        let opts = Options::from_values(values);
        let mut r = RtlRadio {
            label: label.to_string(),
            sock,
            greeting,
            local,
            sent: HashMap::new(),
            bias_on: false,
            class: RateClass::Rtl,
            opts,
            rate: 0,
            centre: values.get(&CENTRE),
            noted_auto: false,
            state,
            notices: notices.clone(),
            window: link.window,
            stall: link.stall,
            closed: false,
        };
        // The engine runs its own gain control.
        r.send(cmd::GAIN_MODE, 1)?;
        r.send(cmd::AGC, 0)?;
        Ok(r)
    }

    fn send(&mut self, c: u8, v: u32) -> Result<(), DriverError> {
        self.sock
            .write_all(&command(c, v))
            .map_err(|_| DriverError::Net(NetError::Closed))?;
        self.sent.insert(c, v);
        Ok(())
    }

    fn send_changed(&mut self, c: u8, v: u32) -> Result<(), DriverError> {
        if self.sent.get(&c) != Some(&v) {
            self.send(c, v)?;
        }
        Ok(())
    }

    fn is_other(&self) -> bool {
        self.class != RateClass::Rtl || self.greeting.is_other()
    }
}

impl Radio for RtlRadio {
    type Stream = RtlStream;

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        let mut opts = Options::from_values(values);
        if opts.pinned.is_none() {
            opts.pinned = *lock(&self.state.pinned);
        }
        let plan = plan_rate(&opts, values, self.greeting.is_other())?;
        let mut notes = Vec::new();
        if let Some(n) = plan.note {
            if !self.noted_auto {
                self.noted_auto = true;
                self.notices.log(format!("rtl_tcp {}: {n}", self.label));
            }
        }
        self.opts = opts;
        self.class = plan.class;
        if plan.class != RateClass::Rtl {
            self.state.found_other.store(true, Ordering::Relaxed);
        }
        // Frequency.
        let mut f = values.get(&CENTRE).max(0.0).round();
        if f > MAX_FREQ {
            f = MAX_FREQ;
            notes.push("the rtl_tcp protocol takes the frequency as 32-bit Hz (at most 4.29 GHz): it was cut to that".to_string());
        }
        // In this order: rate, direct sampling, ppm, frequency, bias tee, gain.
        self.send_changed(cmd::RATE, plan.rate)?;
        if self.rate != plan.rate {
            self.rate = plan.rate;
            let mut w = lock(&self.state.window);
            (w.start, w.bytes, w.asked) = (None, 0, f64::from(plan.rate));
        }
        let below_hf = f < 24e6;
        match opts.ds {
            DirectSampling::Auto => {
                if below_hf && matches!(self.greeting.tuner, 5 | 6) && !self.is_other() {
                    notes.push(
                        "below 24 MHz this tuner cannot receive on its own: a Blog V4 server upconverts HF itself; otherwise pick \
                         direct sampling I or Q (a V3 has its HF input on Q)"
                            .to_string(),
                    );
                }
            }
            DirectSampling::Off => self.send_changed(cmd::DIRECT, 0)?,
            DirectSampling::I => self.send_changed(cmd::DIRECT, if below_hf { 1 } else { 0 })?,
            DirectSampling::Q => self.send_changed(cmd::DIRECT, if below_hf { 2 } else { 0 })?,
        }
        let ppm = opts.ppm.round();
        let ppm = if (-1000.0..=1000.0).contains(&ppm) {
            ppm as i32
        } else {
            0
        };
        self.send_changed(cmd::PPM, ppm as u32)?;
        self.send_changed(cmd::FREQ, f as u32)?;
        self.centre = f;
        let bias = values.get(&BIAS_TEE);
        if bias || self.bias_on {
            self.send_changed(cmd::BIAS, bias as u32)?;
            self.bias_on = bias;
        }
        if !values.get(&AGC) {
            let g = values.get(&GAIN);
            let target = (g * 10.0).round() as i32;
            let v = if self.is_other() {
                target.max(0)
            } else {
                nearest_gain(self.greeting.gain_table(), target)
            };
            self.send_changed(cmd::GAIN, v as u32)?;
        }
        Ok(notes.join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<RtlStream, DriverError> {
        let mut sock = self.sock.try_clone()?;
        abort.register(sock.as_ref());
        let _ = sock.set_read_timeout(Some(self.stall));
        Ok(RtlStream {
            sock,
            label: self.label.clone(),
            local: self.local,
            stall: self.stall,
            state: self.state.clone(),
            window: self.window,
            spare: vec![0u8; READ],
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if self.bias_on {
            let _ = self.sock.write_all(&command(cmd::BIAS, 0));
            self.bias_on = false;
        }
        self.sock.shutdown();
    }

    fn rate_hz(&self) -> Hz {
        Hz::from(self.rate)
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cu8
    }

    fn capabilities(&self) -> Capabilities {
        let pinned = self.opts.pinned.or(*lock(&self.state.pinned));
        let rates = if let Some(p) = pinned {
            let p = Hz::from(p);
            RateLimits {
                min_hz: p,
                max_hz: p,
                ranges: vec![(p, p)],
                steady_hz: None,
            }
        } else if self.opts.anyrate {
            RateLimits {
                min_hz: 0.1e6,
                max_hz: ANY_MAX,
                ..Default::default()
            }
        } else if self.opts.server == ServerMode::Other
            || (self.opts.server == ServerMode::Auto
                && self.state.found_other.load(Ordering::Relaxed))
        {
            RateLimits {
                min_hz: 0.25e6,
                max_hz: OTHER_MAX,
                ..Default::default()
            }
        } else {
            RateLimits {
                min_hz: 225_001.0,
                max_hz: RTL_STEADY,
                ranges: vec![(225_001.0, 300_000.0), (900_001.0, RTL_STEADY)],
                steady_hz: None,
            }
        };
        let name = if self.greeting.is_other() {
            format!(
                "rtl_tcp {} ({})",
                self.label,
                String::from_utf8_lossy(&self.greeting.magic)
            )
        } else {
            format!("RTL-SDR {} at {}", self.greeting.tuner_name(), self.label)
        };
        Capabilities {
            name,
            rates,
            min_freq_hz: 0.0,
            max_freq_hz: MAX_FREQ,
            gain_db: Some((0.0, 49.6, 0.1)),
            bandwidth_hz: None,
            has_agc: false,
            transmit: None,
            has_bias_tee: true,
            realtime: true,
            common_rates_hz: COMMON_RATES.to_vec(),
            common_bandwidths_hz: Vec::new(),
            gain_steps_db: if self.greeting.is_other() {
                Vec::new()
            } else {
                self.greeting
                    .gain_table()
                    .iter()
                    .map(|&g| f64::from(g) / 10.0)
                    .collect()
            },
        }
    }

    fn status(&self) -> String {
        match *lock(&self.state.last) {
            Some(t) => format!(
                "{} Msps from {}, {:.1} Mbit/s",
                crate::fmt_m(t.achieved_hz),
                self.label,
                t.mbit_s
            ),
            None => String::new(),
        }
    }
}

/// The most bytes one read takes.
const READ: usize = 1 << 20;

/// The read loop of one rtl_tcp connection. It reads the bytes, unsigned 8-bit I/Q pairs, straight into the ring.
pub struct RtlStream {
    sock: Box<dyn Conn>,
    label: String,
    local: bool,
    stall: Duration,
    state: Arc<AddrState>,
    window: Duration,
    /// Where a read goes when the ring has no room for it.
    spare: Vec<u8>,
}

impl RtlStream {
    /// Counts bytes into the throughput window; at the end of a window, publishes it and runs the pin detector.
    fn count(&mut self, n: usize, sink: &Sink, ctx: &StreamCtx) {
        let now = Instant::now();
        let reading = {
            let mut w = lock(&self.state.window);
            w.total += n as u64;
            match w.start {
                None => {
                    (w.start, w.bytes) = (Some(now), 0);
                    None
                }
                Some(s) => {
                    w.bytes += n as u64;
                    let secs = now.duration_since(s).as_secs_f64();
                    if secs >= self.window.as_secs_f64() {
                        let t = Throughput {
                            mbit_s: w.bytes as f64 * 8.0 / secs / 1e6,
                            achieved_hz: w.bytes as f64 / 2.0 / secs,
                            asked_hz: w.asked,
                            total_bytes: w.total,
                            lost_bytes: sink.radio_lost() * 2,
                        };
                        (w.start, w.bytes) = (Some(now), 0);
                        Some(t)
                    } else {
                        None
                    }
                }
            }
        };
        let Some(t) = reading else { return };
        *lock(&self.state.last) = Some(t);
        let pinned = *lock(&self.state.pinned);
        let pin = lock(&self.state.detector).reading(
            t.achieved_hz,
            t.asked_hz,
            t.total_bytes,
            pinned.map(Hz::from),
        );
        if let Some(p) = pin {
            let p = p.round() as u32;
            *lock(&self.state.pinned) = Some(p);
            let m = crate::fmt_m(Hz::from(p));
            ctx.notices.warn(format!(
                "the rtl_tcp server sends {m} Msps whatever it is asked: the radio is set to that rate"
            ));
            ctx.notices
                .store(own_id(RtlSetting::ServerRate), Value::Hz(Hz::from(p)));
            ctx.notices.restart();
        }
    }
}

impl RadioStream for RtlStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let lost = DriverError::Net(NetError::Closed);
        let mut silent_since: Option<Instant> = None;
        let mut said_waiting = false;
        loop {
            if ctx.stopping() {
                return Ok(());
            }
            match sink.receive(READ, &mut self.spare, |dst| self.sock.read(dst)) {
                Ok(0) => return Err(lost),
                Ok(n) => {
                    silent_since = None;
                    said_waiting = false;
                    self.count(n, sink, ctx);
                }
                Err(e) if timed_out(&e) => {
                    if !self.local {
                        return Err(DriverError::Net(NetError::TimedOut));
                    }
                    // A driver app on this device may be paused by the phone: wait up to 15 s.
                    let since = *silent_since.get_or_insert_with(|| Instant::now() - self.stall);
                    if !said_waiting {
                        said_waiting = true;
                        ctx.notices.log(format!(
                            "rtl_tcp {}: no samples for {} s, still waiting (the driver app may be paused by the phone)",
                            self.label,
                            self.stall.as_secs()
                        ));
                    }
                    if since.elapsed() >= self.stall * 5 {
                        return Err(DriverError::Net(NetError::TimedOut));
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Err(lost),
            }
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A fake rtl_tcp server (on the fake network), for the tests of this crate and, with the `fake` feature, of the crates above
/// it. Never a real radio.
#[cfg(any(test, feature = "fake"))]
pub mod fake;

#[cfg(test)]
mod tests;
