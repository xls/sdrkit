//! Drivers for OpenHPSDR network transceivers: protocol 1 (Hermes-Lite 2, Red Pitaya, Metis, Hermes, Angelia, Orion) and
//! protocol 2 (ANAN, Orion firmware). Four kinds of radio share one implementation and differ by their [`Profile`]. A radio
//! sends one UDP stream. Each of its receivers is an open radio of its own, addressed as `host:port` (RX1) or `host:port/rxN`.

#![warn(missing_docs)]

mod discover;
mod hub;
pub mod p1;
pub mod p2;
pub mod plan;
pub mod profile;
mod rx;
mod stream1;
mod stream2;
mod tx;

#[cfg(test)]
mod fake1;
#[cfg(test)]
mod fake2;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_p2;

pub use discover::{Found, DEFAULT_PORT};
pub use profile::{Model, Profile, HERMES_LITE2, HPSDR1, HPSDR2, RED_PITAYA};

use hub::Hub;
use rx::{Place, RxRadio, RxShared};
use sdrkit_core::net::Transport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Argument, Command, CommandId, CommandSet, Declare, Field, Published, Record, Results, Setting,
    SettingSet, TableDef, Values,
};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, Doing, DriverDescriptor, DriverError, Link, LossReport, Polled, Retuned,
    RingWriter, Source, SourceDriver, Started, StreamFormat, Transmitter, TxState,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::sync::{Arc, Weak};
use std::time::Duration;

// ---- Settings ----

/// The frequency correction.
pub const PPM: Setting<field::F64> = Setting::new("ppm", "Frequency correction", "ppm", 0.0)
    .range(-200.0, 200.0, 0.1)
    .help(
        "The clock error of the radio in ppm, positive when its clock runs fast (signals then show a little low). 0: no \
         correction.",
    );
/// Swap I and Q.
pub const SWAP_IQ: Setting<field::Bool> = Setting::new("swapiq", "Swap I and Q", "", false).help(
    "Mirror the spectrum if signals show on the wrong side of the tuned frequency. The two words of each sample are taken the \
     other way round.",
);
/// The longest transmission.
pub const TX_TIMEOUT: Setting<field::F64> =
    Setting::new("txtimeout", "Transmit time limit", "s", 180.0)
        .range(10.0, 600.0, 10.0)
        .help("The longest time one transmission can last. Then sdrkit ends it.");
/// The N2ADR filter board of a Hermes-Lite 2.
pub const N2ADR: Setting<field::Bool> = Setting::new("n2adr", "N2ADR filter board", "", true).help(
    "Switch the low-pass filter for the band on the N2ADR filter board. Its relays are on the open-collector outputs. Above \
     30 MHz no filter is switched in. Without the board this changes nothing. With several receivers open, the filter for the \
     highest frequency among them is used.",
);
/// The power amplifier of a Hermes-Lite 2.
pub const HL2_PA: Setting<field::Bool> =
    Setting::new("hl2pa", "Hermes-Lite 2 power amplifier", "", false).help(
        "Switch its PA on while transmitting. Off: only the low-level exciter output is used.",
    );

sdrkit_core::settings! {
    /// The own settings of every HPSDR radio.
    pub enum HpsdrSetting {
        /// [`PPM`].
        Ppm = PPM,
        /// [`SWAP_IQ`].
        SwapIq = SWAP_IQ,
        /// [`TX_TIMEOUT`].
        TxTimeout = TX_TIMEOUT,
    }
}

sdrkit_core::settings! {
    /// The settings that only a Hermes-Lite 2 has. They follow [`HpsdrSetting`].
    pub enum Hl2Setting {
        /// [`N2ADR`].
        N2adr = N2ADR,
        /// [`HL2_PA`].
        Pa = HL2_PA,
    }
}

// ---- Readings ----

/// The board id of the discovery reply.
pub const BOARD: Field<field::I64> = Field::new("board", "Board", "");
/// The name of the board.
pub const BOARD_NAME: Field<field::Text> = Field::new("board_name", "Board name", "");
/// The MAC address, `00:1C:C0:A2:12:34`.
pub const MAC: Field<field::Text> = Field::new("mac", "MAC", "");
/// The gateware (protocol 1) or firmware (protocol 2) version.
pub const VERSION: Field<field::Text> = Field::new("version", "Version", "");
/// The protocol version a protocol 2 radio supports, `3.8`.
pub const PROTOCOL: Field<field::Text> = Field::new("protocol", "Protocol", "");
/// The receivers the radio says it has.
pub const RADIO_RECEIVERS: Field<field::I64> =
    Field::new("radio_receivers", "Receivers it has", "");
/// The radio was streaming to another program at discovery.
pub const BUSY: Field<field::Bool> = Field::new("busy", "Busy at discovery", "");
/// The receivers in the stream (protocol 1) or the DDCs enabled (protocol 2).
pub const RECEIVERS: Field<field::I64> = Field::new("receivers", "Receivers in the stream", "");
/// The number of this receiver, from 1.
pub const RECEIVER: Field<field::I64> = Field::new("receiver", "This receiver", "");
/// The frequency sent to the radio, after the clock correction.
pub const FREQUENCY_SENT: Field<field::Hz> = Field::new("frequency_sent", "Frequency sent", "Hz");
/// An ADC overload in the last 500 ms.
pub const OVERLOAD: Field<field::Bool> = Field::new("overload", "ADC overload", "");
/// The PLL of a protocol 2 radio is locked.
pub const PLL: Field<field::Bool> = Field::new("pll", "PLL locked", "");
/// The temperature of a Hermes-Lite 2.
pub const TEMPERATURE: Field<field::F64> = Field::new("temperature", "Temperature", "°C");
/// The exciter power, as the radio reads it.
pub const EXCITER: Field<field::I64> = Field::new("exciter", "Exciter power (raw)", "");
/// The forward power, as the radio reads it.
pub const FORWARD: Field<field::I64> = Field::new("fwd", "Forward power (raw)", "");
/// The reverse power, as the radio reads it.
pub const REVERSE: Field<field::I64> = Field::new("rev", "Reverse power (raw)", "");
/// The supply voltage, as the radio reads it.
pub const SUPPLY: Field<field::I64> = Field::new("supply", "Supply voltage (raw)", "");
/// The standing wave ratio from the forward and reverse power.
pub const SWR: Field<field::F64> = Field::new("swr", "SWR", "");
/// Samples lost in sequence gaps.
pub const SEQ_LOST: Field<field::I64> = Field::new("seq_lost", "Lost in sequence gaps", "samples");
/// Samples lost to packets dropped while the layout of a protocol 1 stream changed.
pub const SETTLE_LOST: Field<field::I64> =
    Field::new("settle_lost", "Lost while settling", "samples");
/// Samples dropped because this receiver read too slowly.
pub const QUEUE_DROPPED: Field<field::I64> =
    Field::new("queue_dropped", "Dropped (receiver behind)", "samples");
/// Transmit samples the radio did not get in time.
pub const UNDERRUNS: Field<field::I64> = Field::new("underruns", "Transmit underrun", "samples");
/// The speed of the sample packets over the last window.
pub const THROUGHPUT: Field<field::F64> = Field::new("mbps", "Throughput", "Mbit/s");
/// Sample packets per second over the last window.
pub const PACKETS: Field<field::F64> = Field::new("pps", "Packets", "/s");
/// The samples per second this receiver got or lost over the last window.
pub const ACHIEVED: Field<field::F64> = Field::new("achieved", "Achieved rate", "sps");
/// Sequence gaps over the last window.
pub const GAPS: Field<field::I64> = Field::new("gaps", "Sequence gaps", "");
/// Packets dropped while the layout settled over the last window (protocol 1).
pub const SETTLE_DROPS: Field<field::I64> =
    Field::new("settle_drops", "Dropped while settling", "packets");

/// The readings of an HPSDR receiver.
pub const READINGS: &[&dyn Declare] = &[
    &BOARD,
    &BOARD_NAME,
    &MAC,
    &VERSION,
    &PROTOCOL,
    &RADIO_RECEIVERS,
    &BUSY,
    &RECEIVERS,
    &RECEIVER,
    &FREQUENCY_SENT,
    &OVERLOAD,
    &PLL,
    &TEMPERATURE,
    &EXCITER,
    &FORWARD,
    &REVERSE,
    &SUPPLY,
    &SWR,
    &SEQ_LOST,
    &SETTLE_LOST,
    &QUEUE_DROPPED,
    &UNDERRUNS,
    &THROUGHPUT,
    &PACKETS,
    &ACHIEVED,
    &GAPS,
    &SETTLE_DROPS,
];

// ---- Descriptors ----

const P1_PRODUCES: &[StreamFormat] = &[StreamFormat {
    kind: Kind::Iq,
    sample: SampleType::Cs32,
    min_rate_hz: 48_000.0,
    max_rate_hz: 384_000.0,
    channels: 1,
}];
const P2_PRODUCES: &[StreamFormat] = &[StreamFormat {
    kind: Kind::Iq,
    sample: SampleType::Cs32,
    min_rate_hz: 48_000.0,
    max_rate_hz: 1_536_000.0,
    channels: 1,
}];
const P1_ACCEPTS: &[StreamFormat] = &[StreamFormat {
    kind: Kind::Iq,
    sample: SampleType::Cf32,
    min_rate_hz: p1::TX_RATE as f64,
    max_rate_hz: p1::TX_RATE as f64,
    channels: 1,
}];
const P2_ACCEPTS: &[StreamFormat] = &[StreamFormat {
    kind: Kind::Iq,
    sample: SampleType::Cf32,
    min_rate_hz: p2::TX_RATE as f64,
    max_rate_hz: p2::TX_RATE as f64,
    channels: 1,
}];

static HERMES_LITE2_KIND: DriverDescriptor = DriverDescriptor {
    key: "hermeslite2",
    name: "Hermes Lite 2",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "hermeslite2:<host>:<port> (receiver 1, the port is required)",
        "hermeslite2:<host>:<port>/rx<n> (receiver n)",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(false),
        sdrkit_driver_core::AddressPart::port(None),
        sdrkit_driver_core::AddressPart::receiver(HERMES_LITE2.max_receivers),
    ],
    settings: &[HpsdrSetting::SETTINGS, Hl2Setting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: P1_PRODUCES,
    accepts: P1_ACCEPTS,
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: HpsdrKindCommand::COMMANDS,
    kind_tables: &[&RADIOS],
};
static RED_PITAYA_KIND: DriverDescriptor = DriverDescriptor {
    key: "redpitaya",
    name: "Red Pitaya",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "redpitaya:<host>:<port> (receiver 1, the port is required)",
        "redpitaya:<host>:<port>/rx<n> (receiver n)",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(false),
        sdrkit_driver_core::AddressPart::port(None),
        sdrkit_driver_core::AddressPart::receiver(RED_PITAYA.max_receivers),
    ],
    settings: &[HpsdrSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: P1_PRODUCES,
    accepts: P1_ACCEPTS,
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: HpsdrKindCommand::COMMANDS,
    kind_tables: &[&RADIOS],
};
static HPSDR1_KIND: DriverDescriptor = DriverDescriptor {
    key: "hpsdr1",
    name: "HPSDR (protocol 1)",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "hpsdr1:<host>:<port> (receiver 1, the port is required)",
        "hpsdr1:<host>:<port>/rx<n> (receiver n)",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(false),
        sdrkit_driver_core::AddressPart::port(None),
        sdrkit_driver_core::AddressPart::receiver(HPSDR1.max_receivers),
    ],
    settings: &[HpsdrSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: P1_PRODUCES,
    accepts: P1_ACCEPTS,
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: HpsdrKindCommand::COMMANDS,
    kind_tables: &[&RADIOS],
};
static HPSDR2_KIND: DriverDescriptor = DriverDescriptor {
    key: "hpsdr2",
    name: "ANAN / HPSDR (protocol 2)",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "hpsdr2:<host>:<port> (receiver 1, the port is required)",
        "hpsdr2:<host>:<port>/rx<n> (receiver n)",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(false),
        sdrkit_driver_core::AddressPart::port(None),
        sdrkit_driver_core::AddressPart::receiver(HPSDR2.max_receivers),
    ],
    settings: &[HpsdrSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: P2_PRODUCES,
    accepts: P2_ACCEPTS,
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: HpsdrKindCommand::COMMANDS,
    kind_tables: &[&RADIOS],
};

// ---- Commands of the kind ----

/// How long a search or a test waits for answers.
pub const WAIT: Setting<field::F64> = Setting::new("wait_s", "Wait", "s", 1.5)
    .range(0.05, 10.0, 0.05)
    .help("How long to wait for answers. 1 to 2 s is usual.");
/// The address to test, `host:port` or `host`.
pub const ADDRESS: Argument<field::Text> = Argument::new(Field::new("address", "Address", ""))
    .help("The radio to test, as host:port or host.");

/// Asks every radio of the kind on the local networks to answer, and lists those that answer in [`RADIOS`].
pub const DISCOVER: Command = Command::new("discover", "Find radios")
    .help("Asks every radio of this kind on the local networks to answer. The radios that answer are listed.")
    .args(&[&WAIT]);
/// Asks the radio at [`ADDRESS`] to answer, and lists it in [`RADIOS`].
pub const PROBE: Command = Command::new("probe", "Test an address")
    .help("Asks the radio at the address to answer. A radio that answers is listed.")
    .args(&[&ADDRESS, &WAIT]);

sdrkit_core::commands! {
    /// The commands of an HPSDR driver kind.
    pub enum HpsdrKindCommand {
        /// [`DISCOVER`].
        Discover = DISCOVER,
        /// [`PROBE`].
        Probe = PROBE,
    }
}

/// The address a radio answered from, `host:port`.
pub const R_ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
/// The radio streams to some program now.
pub const R_BUSY: Field<field::Bool> = Field::new("busy", "Busy", "");
/// The board id of the radio.
pub const R_BOARD: Field<field::I64> = Field::new("board", "Board", "");
/// The receivers the radio says it has.
pub const R_RECEIVERS: Field<field::I64> = Field::new("receivers", "Receivers", "");
/// The MAC address of the radio.
pub const R_MAC: Field<field::Text> = Field::new("mac", "MAC address", "");
/// The gateware or firmware version of the radio.
pub const R_VERSION: Field<field::Text> = Field::new("version", "Version", "");
/// The protocol version that a protocol 2 radio supports, times ten.
pub const R_PROTOCOL: Field<field::I64> = Field::new("protocol", "Protocol", "");

/// The radios that a search or a test found, one row per address.
pub static RADIOS: TableDef = TableDef {
    key: "radios",
    label: "Radios found",
    columns: &[
        &R_ADDRESS,
        &R_BUSY,
        &R_BOARD,
        &R_RECEIVERS,
        &R_MAC,
        &R_VERSION,
        &R_PROTOCOL,
    ],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

/// Writes `found` as a row of [`RADIOS`].
fn write_found(found: &Found, results: &mut Published) {
    let key = found.address.to_string();
    let Some(mut row) = results.row_mut(&RADIOS, &key) else {
        return;
    };
    let mac = p1::mac_text(&found.mac);
    row.set(&R_ADDRESS, &key);
    row.set(&R_BUSY, found.busy);
    row.set(&R_BOARD, i64::from(found.board));
    row.opt(&R_RECEIVERS, found.receivers.map(i64::from));
    row.set(&R_MAC, &mac);
    row.set(&R_VERSION, &found.version);
    row.opt(&R_PROTOCOL, found.protocol.map(i64::from));
}

/// The descriptor of the driver kind for `model`.
fn descriptor(model: Model) -> &'static DriverDescriptor {
    match model {
        Model::HermesLite2 => &HERMES_LITE2_KIND,
        Model::RedPitaya => &RED_PITAYA_KIND,
        Model::Hpsdr1 => &HPSDR1_KIND,
        Model::Hpsdr2 => &HPSDR2_KIND,
    }
}

// ---- Errors ----

/// The errors of the HPSDR drivers that [`DriverError`] has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HpsdrError {
    /// The receiver is open already.
    #[error("RX{receiver} is open already")]
    ReceiverInUse {
        /// The receiver number, from 1.
        receiver: usize,
    },
    /// Another transmitter of the radio is open: a radio has one.
    #[error("the radio has another transmitter open")]
    TransmitterTaken,
}

// ---- The drivers ----

/// The four HPSDR drivers, sharing the streams and what they learn per address. They reach their radios through `transport`.
pub fn drivers(transport: Arc<dyn Transport>) -> Vec<Box<dyn SourceDriver>> {
    let hub: Arc<Hub> = Arc::default();
    profile::ALL
        .iter()
        .map(|p| {
            Box::new(HpsdrDriver::with_hub(*p, transport.clone(), hub.clone()))
                as Box<dyn SourceDriver>
        })
        .collect()
}

/// The timings of the driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// Open attempts at start (1 and the retries).
    pub open_attempts: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing { open_attempts: 4 }
    }
}

/// One kind of HPSDR radio.
pub struct HpsdrDriver {
    p: Profile,
    transport: Arc<dyn Transport>,
    hub: Arc<Hub>,
    timing: Timing,
}

impl HpsdrDriver {
    /// A driver for radios of the profile `p`, reached through `transport`.
    pub fn new(p: Profile, transport: Arc<dyn Transport>) -> HpsdrDriver {
        HpsdrDriver::with_hub(p, transport, Arc::default())
    }

    fn with_hub(p: Profile, transport: Arc<dyn Transport>, hub: Arc<Hub>) -> HpsdrDriver {
        HpsdrDriver {
            p,
            transport,
            hub,
            timing: Timing::default(),
        }
    }

    /// The same driver with other timings.
    pub fn with_timing(mut self, timing: Timing) -> HpsdrDriver {
        self.timing = timing;
        self
    }

    #[cfg(test)]
    fn live_streams(&self) -> usize {
        self.hub.live_streams()
    }
}

/// A parsed address: `host:port[/rxN]`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Address {
    host: String,
    port: u16,
    receiver: usize,
}

/// Reads `host:port` or `host:port/rxN`. The port must be given.
///
/// # Errors
///
/// [`DriverError::Address`] for another text, [`DriverError::Streams`] for a receiver above the most the profile has.
fn parse_address(p: &Profile, address: &str) -> Result<Address, DriverError> {
    let a = address.trim();
    let (hp, n) = match a.to_ascii_lowercase().rfind("/rx") {
        Some(i) => {
            let n: usize = a[i + 3..].parse().map_err(|_| DriverError::Address)?;
            if n == 0 {
                return Err(DriverError::Address);
            }
            (&a[..i], n)
        }
        None => (a, 1),
    };
    let (host, port) = hp.rsplit_once(':').ok_or(DriverError::Address)?;
    if host.is_empty() || host.contains(':') || host.contains('/') {
        return Err(DriverError::Address);
    }
    let port: u16 = port
        .parse()
        .ok()
        .filter(|&p| p > 0)
        .ok_or(DriverError::Address)?;
    if n > usize::from(p.max_receivers) {
        return Err(DriverError::Streams {
            given: n,
            has: usize::from(p.max_receivers),
        });
    }
    Ok(Address {
        host: host.to_string(),
        port,
        receiver: n,
    })
}

impl SourceDriver for HpsdrDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        descriptor(self.p.model)
    }

    fn invoke_kind(
        &self,
        id: CommandId,
        args: &Values,
        results: &mut Published,
    ) -> Result<Option<Record>, DriverError> {
        let wait = Duration::from_secs_f64(args.get(&WAIT));
        match HpsdrKindCommand::from_id(id) {
            Some(HpsdrKindCommand::Discover) => {
                results.clear_rows(&RADIOS);
                for f in discover::discover(&self.p, &self.hub, self.transport.as_ref(), wait) {
                    write_found(&f, results);
                }
            }
            Some(HpsdrKindCommand::Probe) => {
                let address = args.arg(&ADDRESS).map_err(|_| DriverError::Address)?;
                let a = parse_address(&self.p, address)?;
                let f = discover::probe(
                    &self.p,
                    &self.hub,
                    self.transport.as_ref(),
                    &a.host,
                    a.port,
                    wait,
                )?;
                write_found(&f, results);
            }
            None => {
                return Err(DriverError::NotNow {
                    state: Doing::Closed,
                })
            }
        }
        Ok(None)
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let a = parse_address(&self.p, address)?;
        let p = self.p;
        hub::RxParams::from_values(&p, self.hub.known(&a.label()).board, values)?;
        let pl = Place {
            p,
            label: a.label(),
            host: a.host,
            port: a.port,
            n: a.receiver,
        };
        let shared: Arc<RxShared> = Arc::default();
        let (pl2, hub, sh2, net) = (
            pl.clone(),
            self.hub.clone(),
            shared.clone(),
            self.transport.clone(),
        );
        let opener: Opener<RxRadio> = Arc::new(move |values, abort, notices| {
            RxRadio::open(&pl2, &hub, &net, &sh2, values, abort, notices)
        });
        let mut cfg = RunnerConfig::new(
            format!("{} RX{} {}", p.name, pl.n, pl.label),
            NETWORK_ADVICE,
        );
        cfg.open_attempts = self.timing.open_attempts;
        let runner = StreamRunner::open(cfg, opener, values)?;
        Ok(Box::new(HpsdrSource {
            runner,
            pl,
            shared,
            tx: None,
        }))
    }
}

impl Address {
    fn label(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// One open receiver of an HPSDR radio: the shared runner and the transmitter.
struct HpsdrSource {
    runner: StreamRunner<RxRadio>,
    pl: Place,
    shared: Arc<RxShared>,
    tx: Option<Weak<hub::TxQueue>>,
}

impl HpsdrSource {
    fn stream(&self) -> Option<Arc<dyn hub::Stream>> {
        hub::lock(&self.shared.current).clone()
    }
}

impl Source for HpsdrSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }

    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.runner.start(sink)
    }

    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        self.runner.apply(values)
    }

    fn stop(&mut self) {
        if let Some(q) = self.tx.take().and_then(|w| w.upgrade()) {
            if q.state().transmitting() {
                if let Some(s) = self.stream() {
                    s.tx_end("the receiver stopped: transmit ended");
                }
            }
            q.set_state(TxState::Closed);
        }
        self.runner.stop();
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cs32
    }

    fn link(&self) -> Link {
        self.runner.link()
    }

    fn poll(&mut self, results: &mut Results) -> Polled {
        self.runner.poll(results)
    }

    fn loss(&self) -> Option<LossReport> {
        self.runner.loss()
    }

    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }

    fn tx_schema(&self) -> Option<Arc<sdrkit_core::Schema>> {
        let limits = self.runner.capabilities().transmit.as_ref()?;
        Some(sdrkit_driver_core::tx_schema(&[], limits))
    }

    fn transmitter(&mut self, values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        let s = self.stream().ok_or(DriverError::NotNow {
            state: Doing::Stopped,
        })?;
        let t = tx::HpsdrTransmitter::open(self.pl.p, &self.pl.label, &s, values)?;
        self.tx = Some(Arc::downgrade(t.queue()));
        Ok(Box::new(t))
    }
}

impl Drop for HpsdrSource {
    fn drop(&mut self) {
        self.stop();
    }
}
