//! A client for rsp_tcp servers: SDRplay RSP radios on the network. An rsp_tcp server speaks the rtl_tcp protocol. In its
//! extended mode it also takes commands for the controls of the RSP (antenna, LNA state, IF gain reduction, the AGC of the
//! radio, notches, bias-T, clock output).

#![warn(missing_docs)]

pub mod model;
pub mod proto;
mod radio;

use model::{Antenna, Controls, GainSplit, Model};
use proto::{SampleFormat, DEFAULT_PORT};
use radio::RspRadio;
use sdrkit_core::net::{host_port, show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Hz, Results, Setting, SettingSet, Values};
use sdrkit_driver_core::form;
use sdrkit_driver_core::rtl_tcp::{PinDetector, SERVER_RATE};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    DriverDescriptor, DriverError, RadioSetting, Source, SourceDriver, StreamFormat, RATE,
    RATE_EXACT,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The lowest rate an RSP returns through rsp_tcp (2 Msps decimated by 32).
pub const MIN_RATE: Hz = 62_500.0;
/// The highest rate.
pub const MAX_RATE: Hz = 10e6;
/// The rate sent when the stack requests none.
pub const DEFAULT_RATE: Hz = 2.048e6;
/// Above this link speed the radio writes a note to the log.
pub const LINK_WARN_BITS: f64 = 100e6;

// ---- Settings ----

sdrkit_core::choice_enum! {
    /// The commands the driver sends ([`PROTOCOL`]).
    pub enum CommandSet {
        /// The RSP commands if the server sends the extended greeting, else the rtl_tcp commands.
        Auto = ("auto", "Auto (from the greeting)"),
        /// The RSP commands. A server without the extended greeting is refused.
        Extended = ("extended", "Extended (RSP commands)"),
        /// The rtl_tcp commands only.
        Compat = ("compat", "rtl_tcp commands only"),
    }
}

/// The commands the driver sends.
pub const PROTOCOL: Setting<field::Choice<CommandSet>> =
    form::protocol(CommandSet::Auto)
        .restart()
        .help(
            "Auto uses the RSP commands when the server greets in extended mode (rsp_tcp started with its extended-mode option), \
             and plain rtl_tcp commands otherwise. Extended insists on the extended greeting. rtl_tcp commands only: for servers \
             that misbehave with the RSP commands.",
        );
/// The frequency correction.
pub const PPM: Setting<field::F64> = form::PPM
    .range(-200.0, 200.0, 1.0)
    .help("The clock error of the radio in ppm, positive when the clock runs fast.");
/// The antenna input.
pub const ANTENNA: Setting<field::Choice<Antenna>> = form::antenna(Antenna::First).help(
    "The input the antenna is on. Hi-Z inputs are for 1 kHz to 30 MHz. On the RSPduo, the tuner choice of the server can limit \
     this. A model that lacks the input refuses it.",
);
/// How one gain number is shared between the two gain stages.
pub const GAIN_SPLIT: Setting<field::Choice<GainSplit>> =
    Setting::choice("gainsplit", "Gain split", GainSplit::Sensitivity)
        .section(form::GAIN_SECTION)
        .help(
        "How one gain number is shared between the LNA and the IF stage. Sensitivity keeps the LNA gain (best noise figure). \
         Linearity gives up LNA gain first (better with strong neighbouring signals). The samples on the link are 8-bit, so set \
         the gain so that the signal neither clips nor sinks into the noise.",
    );
/// The LNA state by hand. -1 follows the gain number.
pub const LNA_STATE: Setting<field::I64> = form::lna_state(form::STAGE_FOLLOWS, model::MAX_LNA_STATE as f64)
    .help(
        "Set the LNA state by hand. -1 follows the gain number. The LNA steps depend on the model and the band. A state the band \
         does not have is refused.",
    );
/// The IF gain reduction by hand. -1 follows the gain number.
pub const IFGR: Setting<field::I64> =
    form::if_reduction(form::STAGE_FOLLOWS, model::IFGR_MAX as f64)
        .help("Set the IF gain reduction by hand (20 to 59 dB). -1 follows the gain number.");
/// The level the AGC of the radio holds.
pub const AGC_SETPOINT: Setting<field::I64> = Setting::new(
    "agcsetpoint",
    "AGC set point",
    "dBFS",
    -30,
)
.range(-60.0, -20.0, 1.0)
.section(form::GAIN_SECTION)
.help(
    "The level the AGC of the radio holds the signal at. The AGC runs when automatic gain is on.",
);
const NOTCH_HELP: &str =
    "Cut strong broadcast (FM, MW) or DAB band III signals that overload the radio. Leave off to receive those bands.";
/// The FM/MW broadcast notch.
pub const RF_NOTCH: Setting<field::Bool> =
    Setting::new("rfnotch", "FM/MW notch", "", false)
        .section(form::ANTENNA_SECTION)
        .help(NOTCH_HELP);
/// The DAB notch.
pub const DAB_NOTCH: Setting<field::Bool> =
    Setting::new("dabnotch", "DAB notch", "", false)
        .section(form::ANTENNA_SECTION)
        .help(NOTCH_HELP);
/// The MW notch of the Hi-Z input of the RSPduo.
pub const AM_NOTCH: Setting<field::Bool> = Setting::new("amnotch", "Hi-Z MW notch", "", false)
    .section(form::ANTENNA_SECTION)
    .help(
    "Cuts strong MW broadcast signals on the Hi-Z input of the RSPduo. Leave off to receive MW.",
);
/// The clock output.
pub const REF_OUT: Setting<field::Bool> = form::CLOCK_OUT
    .help("The 24 MHz clock of the radio on its output socket, to lock another radio.");

sdrkit_core::settings! {
    /// The own settings of the rsp_tcp driver. A server that lacks a control refuses a value that needs it.
    pub enum RspSetting {
        /// [`GAIN_SPLIT`].
        GainSplit = GAIN_SPLIT,
        /// [`LNA_STATE`].
        LnaState = LNA_STATE,
        /// [`IFGR`].
        Ifgr = IFGR,
        /// [`AGC_SETPOINT`].
        AgcSetpoint = AGC_SETPOINT,
        /// [`PPM`].
        Ppm = PPM,
        /// [`ANTENNA`].
        Antenna = ANTENNA,
        /// [`RF_NOTCH`].
        RfNotch = RF_NOTCH,
        /// [`DAB_NOTCH`].
        DabNotch = DAB_NOTCH,
        /// [`AM_NOTCH`].
        AmNotch = AM_NOTCH,
        /// [`REF_OUT`].
        RefOut = REF_OUT,
        /// [`PROTOCOL`].
        Protocol = PROTOCOL,
        /// The rate the server keeps whatever rate is requested ([`SERVER_RATE`]).
        ServerRate = SERVER_RATE,
    }
}

// ---- Readings ----

/// The server takes the RSP commands.
pub const EXTENDED: Field<field::Bool> = Field::new("extended", "RSP commands", "");
/// The version of the server, "major.minor" (extended mode only).
pub const VERSION: Field<field::Text> = Field::new("version", "Version", "");
/// The model of the radio (extended mode only).
pub const MODEL: Field<field::Text> = Field::new("model", "Model", "");
/// The sample format on the link.
pub const FORMAT: Field<field::Choice<SampleFormat>> = Field::new("format", "Sample format", "");
/// The controls the server offers (extended mode only).
pub const CONTROLS: Field<field::Text> = Field::new("controls", "Controls", "");
/// The tuner type of the greeting (rtl_tcp commands only).
pub const TUNER: Field<field::I64> = Field::new("tuner", "Tuner type", "");
/// The gain count of the greeting (rtl_tcp commands only).
pub const GAINS: Field<field::I64> = Field::new("gains", "Gain count", "");
/// The speed of the link over the last window.
pub const THROUGHPUT: Field<field::F64> = Field::new("throughput", "Link", "Mbit/s");

/// The readings of an rsp_tcp radio.
pub const READINGS: &[&dyn Declare] = &[
    &EXTENDED,
    &VERSION,
    &MODEL,
    &FORMAT,
    &CONTROLS,
    &TUNER,
    &GAINS,
    &THROUGHPUT,
];

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "rsp_tcp",
    name: "SDRplay RSP (rsp_tcp server)",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &["rsp_tcp:<host> (port 1234)", "rsp_tcp:<host>:<port>"],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(DEFAULT_PORT)),
    ],
    settings: &[RspSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[
        StreamFormat {
            kind: Kind::Iq,
            sample: SampleType::Cu8,
            min_rate_hz: MIN_RATE,
            max_rate_hz: MAX_RATE,
            channels: 1,
        },
        StreamFormat {
            kind: Kind::Iq,
            sample: SampleType::Cs16,
            min_rate_hz: MIN_RATE,
            max_rate_hz: MAX_RATE,
            channels: 1,
        },
    ],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The errors of the rsp_tcp driver that [`DriverError`] has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RspError {
    /// [`PROTOCOL`] is [`CommandSet::Extended`] but the server did not send the extended greeting.
    #[error("the server did not send the extended rsp_tcp greeting")]
    NotExtended,
    /// The server sends another sample format than at open.
    #[error("the server changed its sample format from {was:?} to {now:?}")]
    FormatChanged {
        /// The format at open.
        was: SampleFormat,
        /// The format now.
        now: SampleFormat,
    },
}

/// The rsp_tcp settings of a radio, read from its values.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// The commands the driver sends.
    pub protocol: CommandSet,
    /// Frequency correction, ppm.
    pub ppm: i64,
    /// The antenna input.
    pub antenna: Antenna,
    /// The gain split.
    pub split: GainSplit,
    /// The LNA state by hand. -1 follows the gain number.
    pub lnastate: i64,
    /// The IF gain reduction by hand. -1 follows the gain number.
    pub ifgr: i64,
    /// The AGC set point, dBFS.
    pub agcsetpoint: i64,
    /// The FM/MW notch.
    pub rfnotch: bool,
    /// The DAB notch.
    pub dabnotch: bool,
    /// The Hi-Z MW notch.
    pub amnotch: bool,
    /// The clock output.
    pub refout: bool,
    /// The rate the server holds to, whole Hz.
    pub pinned: Option<u32>,
}

impl Options {
    /// Reads the settings from `v`.
    pub fn from_values(v: &Values) -> Options {
        let pinned = Some(v.get(&SERVER_RATE))
            .filter(|r| *r >= 1.0)
            .map(|r| r.round().min(f64::from(u32::MAX)) as u32);
        Options {
            protocol: v.get(&PROTOCOL),
            ppm: v.get(&PPM).round() as i64,
            antenna: v.get(&ANTENNA),
            split: v.get(&GAIN_SPLIT),
            lnastate: v.get(&LNA_STATE),
            ifgr: v.get(&IFGR),
            agcsetpoint: v.get(&AGC_SETPOINT),
            rfnotch: v.get(&RF_NOTCH),
            dabnotch: v.get(&DAB_NOTCH),
            amnotch: v.get(&AM_NOTCH),
            refout: v.get(&REF_OUT),
            pinned,
        }
    }

    /// The value of command 0x1F for `model`: 0 for [`Antenna::First`], else the position of the input in the inputs of the
    /// model. `None` if the model lacks the input.
    pub fn antenna_index(&self, model: Model) -> Option<u32> {
        match self.antenna {
            Antenna::First => Some(0),
            a => model
                .inputs()
                .iter()
                .position(|i| *i == a)
                .map(|i| i as u32),
        }
    }
}

// ---- The rate ----

/// Chooses the rate to send: the rate the server holds to, else the exact rate as it is, else the rate of the stack (or
/// 2.048 Msps) held to 62.5 ksps to 10 Msps.
///
/// # Errors
///
/// [`DriverError::Refused`] for [`RATE`] if the stack requests more than 10 Msps and [`RATE_EXACT`] is off.
pub fn plan_rate(o: &Options, v: &Values) -> Result<u32, DriverError> {
    if let Some(p) = o.pinned {
        return Ok(p);
    }
    let mode_rate = Some(v.get(&RATE)).filter(|r| *r > 0.0);
    let rate = match mode_rate {
        Some(r) if v.get(&RATE_EXACT) => r,
        Some(r) if r > MAX_RATE * 1.0001 => {
            return Err(DriverError::Refused {
                id: RadioSetting::Rate.id(),
            })
        }
        r => r.unwrap_or(DEFAULT_RATE).clamp(MIN_RATE, MAX_RATE),
    };
    Ok(rate.round().clamp(1.0, f64::from(u32::MAX)) as u32)
}

// ---- What is kept per server ----

/// What a greeting said.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerInfo {
    /// The connection uses the RSP commands.
    pub extended: bool,
    /// The magic of the greeting.
    pub magic: [u8; 4],
    /// The version (extended).
    pub version: Option<(u16, u16)>,
    /// The model (extended).
    pub model: Option<Model>,
    /// The capability word (extended, else 0).
    pub caps: u32,
    /// The sample format on the link.
    pub format: SampleFormat,
    /// The first word of the greeting (the tuner type for the rtl_tcp commands).
    pub tuner: u32,
    /// The second word (the gain count for the rtl_tcp commands).
    pub gains: u32,
}

impl ServerInfo {
    /// The controls the model and the capability word allow. None for the rtl_tcp commands.
    pub fn controls(&self) -> Controls {
        match (self.extended, self.model) {
            (true, Some(m)) => Controls::of(m, self.caps),
            _ => Controls::default(),
        }
    }

    /// A name for the radio at `label`.
    pub fn list_name(&self, label: &str) -> String {
        match (self.extended, self.model, self.version) {
            (true, Some(m), Some((ma, mi))) => {
                format!("SDRplay {} at {label} (rsp_tcp v{ma}.{mi})", m.name())
            }
            _ => format!("rsp_tcp {label} (rtl_tcp commands)"),
        }
    }

    /// Writes the readings of the greeting into `r`.
    fn write(&self, r: &mut Results) {
        let mut f = r.fields_mut();
        f.set(&EXTENDED, self.extended);
        f.set(&FORMAT, self.format);
        if self.extended {
            let version = self.version.map(|(ma, mi)| format!("{ma}.{mi}"));
            f.opt(&VERSION, version.as_deref());
            let model = self.model.map(|m| m.name());
            f.opt(&MODEL, model.as_deref());
            let mut names = self.controls().names();
            if self.model == Some(Model::RspDuo) {
                for n in names.iter_mut().filter(|n| **n == "bias-T") {
                    *n = "bias-T (Tuner 2 input only)";
                }
            }
            let names = if names.is_empty() {
                "none".to_string()
            } else {
                names.join(", ")
            };
            f.set(&CONTROLS, &names);
        } else {
            f.set(&TUNER, i64::from(self.tuner));
            f.set(&GAINS, i64::from(self.gains));
        }
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throughput {
    /// Bits per second, in millions.
    pub mbit_s: f64,
    /// Samples per second that arrived (bytes / bytes per sample / seconds).
    pub achieved_hz: Hz,
    /// The requested rate.
    pub asked_hz: Hz,
    /// Bytes since connect.
    pub total_bytes: u64,
    /// Lost samples x bytes per sample.
    pub lost_bytes: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Window {
    pub start: Option<Instant>,
    pub bytes: u64,
    pub total: u64,
    pub asked: Hz,
}

/// Per server address: what the driver learnt, shared by every connection to it.
#[derive(Debug, Default)]
pub(crate) struct AddrState {
    pub info: Mutex<Option<ServerInfo>>,
    pub pinned: Mutex<Option<u32>>,
    pub last: Mutex<Option<Throughput>>,
    pub window: Mutex<Window>,
    pub detector: Mutex<PinDetector>,
    pub fallback_logged: AtomicBool,
}

/// Per open radio: the notes already written, and the sample format of the first connection.
#[derive(Debug, Default)]
pub(crate) struct OpenState {
    pub noted: Mutex<Vec<&'static str>>,
    pub format: Mutex<Option<SampleFormat>>,
}

impl OpenState {
    /// True the first time `what` is noted in this open.
    pub fn first(&self, what: &'static str) -> bool {
        let mut n = lock(&self.noted);
        if n.contains(&what) {
            false
        } else {
            n.push(what);
            true
        }
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- The driver ----

/// The timings of the driver.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// The throughput window (1 s). The detection of a server that holds its own rate counts in windows.
    pub window: Duration,
    /// Silence that ends the read loop for a remote server (3 s).
    pub stall: Duration,
    /// Silence that ends it for a local server (15 s).
    pub local_stall: Duration,
    /// The wait for the greeting (10 s).
    pub greeting: Duration,
    /// Open attempts at start (4).
    pub open_attempts: u32,
    /// Treat a local server as a remote one.
    pub local_as_remote: bool,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            window: Duration::from_secs(1),
            stall: Duration::from_secs(3),
            local_stall: Duration::from_secs(15),
            greeting: Duration::from_secs(10),
            open_attempts: 4,
            local_as_remote: false,
        }
    }
}

/// The rsp_tcp driver. It remembers, per server address, what the greeting said, the last throughput, and held rates.
#[derive(Clone)]
pub struct RspTcpDriver {
    transport: Arc<dyn Transport>,
    addrs: Arc<Mutex<HashMap<String, Arc<AddrState>>>>,
    timing: Timing,
}

impl RspTcpDriver {
    /// A driver that reaches its servers through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with its own timings.
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        RspTcpDriver {
            transport,
            addrs: Arc::default(),
            timing,
        }
    }

    fn addr_state(&self, key: &str) -> Arc<AddrState> {
        lock(&self.addrs)
            .entry(key.to_string())
            .or_default()
            .clone()
    }

    fn known(&self, address: &str) -> Option<Arc<AddrState>> {
        let (h, p) = host_port(address, Some(DEFAULT_PORT)).ok()?;
        lock(&self.addrs).get(&show(h, p)).cloned()
    }

    /// What the last greeting of the server at `address` said.
    pub fn server_info(&self, address: &str) -> Option<ServerInfo> {
        lock(&self.known(address)?.info).clone()
    }

    /// The last throughput window of the server at `address`, if any.
    pub fn throughput(&self, address: &str) -> Option<Throughput> {
        *lock(&self.known(address)?.last)
    }
}

impl SourceDriver for RspTcpDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) =
            host_port(address, Some(DEFAULT_PORT)).map_err(|_| DriverError::Address)?;
        let label = show(host, port);
        let state = self.addr_state(&label);
        plan_rate(&Options::from_values(values), values)?;
        let at = radio::At {
            transport: self.transport.clone(),
            host: host.to_owned(),
            port,
            label,
            timing: self.timing,
        };
        let open = Arc::new(OpenState::default());
        let st = state.clone();
        let opener: Opener<RspRadio> = Arc::new(move |values, abort, notices| {
            RspRadio::connect(&at, st.clone(), open.clone(), values, abort, notices)
        });
        let mut cfg = RunnerConfig::new("rsp_tcp", NETWORK_ADVICE);
        cfg.open_attempts = self.timing.open_attempts;
        Ok(Box::new(StreamRunner::open(cfg, opener, values)?))
    }
}

#[cfg(test)]
mod tests;
