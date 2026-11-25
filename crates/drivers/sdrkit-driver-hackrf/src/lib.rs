//! The HackRF driver: the HackRF One, the HackRF Pro in its 8-bit sample mode, the Jawbreaker and the rad1o over USB.
//! It receives signed 8-bit I/Q at 2 to 20 MS/s and transmits half duplex on the same antenna port.
//!
//! Addresses: `hackrf:` (the first free HackRF), `hackrf:<serial>` (the serial number or its last digits),
//! `hackrf:#<n>` (the n-th HackRF of the bus listing, from 0), `hackrf:@<bus>-<port path>`. The scheme `usb:hackrf:`
//! is taken too.
//!
//! Source: the USB protocol of the HackRF family as the maker publishes it (firmware USB API 0x0110 of release
//! v2026.01.3 and 0x0113 of the development firmware, and older firmware by its version rules).

#![warn(missing_docs)]

mod ctl;
mod error;
mod link;
pub mod proto;
mod select;
mod tx;

pub use ctl::Facts;
pub use error::HackrfError;
pub use select::{is_dfu, is_hackrf, Selector};
pub use tx::{HackrfTxSetting, TX_AMP};

use crate::ctl::{identify, Ctl};
use crate::link::{lock, HackrfRadio, Link, RxConfig};
use sdrkit_core::command::{Command, CommandSet};
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandId, Declare, Field, Published, Record, Results, Setting, SettingDecl, SettingSet,
    TableDef, Values,
};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner};
use sdrkit_driver_core::{
    AddressPart, Capabilities, Doing, DriverDescriptor, DriverError, Link as LinkState, LossReport,
    Polled, Retuned, RingWriter, Source, SourceDriver, Started, StreamFormat, Syntax, Transmitter,
    TX_OUTPUT_DBM,
};
use sdrkit_driver_usb::{UsbBus, UsbDeviceInfo};
use sdrkit_graph::stream::{Kind, SampleType};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The timings and transfer sizes of the driver.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// The longest wait of one bulk read; the read loop looks for a stop between reads.
    pub read_slice: Duration,
    /// No receive block for this long ends the stream: "the device stopped streaming".
    pub watchdog: Duration,
    /// How often the streaming state is read (USB API 0x0106 or later). `None` never reads it.
    pub state_poll: Option<Duration>,
    /// The gap between attempts to open the device again after it left.
    pub reconnect_gap: Duration,
    /// The timeout of a control request.
    pub control: Duration,
    /// The timeout of mode off, which can wait for the device to drain a transmission.
    pub mode_off: Duration,
    /// The size of one bulk read, a multiple of 16 384 bytes.
    pub read_size: usize,
    /// The bulk reads kept queued.
    pub reads_queued: usize,
    /// The timeout of one bulk write.
    pub write_timeout: Duration,
}

impl Default for Timing {
    /// Reads of 50 ms, a watchdog of 1 s, the state every 250 ms, a reopen every 250 ms, requests of 100 ms, mode off of
    /// 1 s, 4 queued reads of 256 KiB, writes of at most 1 s.
    fn default() -> Self {
        Timing {
            read_slice: Duration::from_millis(50),
            watchdog: Duration::from_secs(1),
            state_poll: Some(Duration::from_millis(250)),
            reconnect_gap: Duration::from_millis(250),
            control: Duration::from_millis(100),
            mode_off: Duration::from_secs(1),
            read_size: 262_144,
            reads_queued: 4,
            write_timeout: Duration::from_secs(1),
        }
    }
}

// ---- Settings ----

/// The LNA gain by hand. -1 follows the gain.
pub const LNA: Setting<field::I64> = Setting::new("lna_db", "Gain stage: LNA", "dB", -1)
    .range(-1.0, 40.0, 1.0)
    .help(
        "The receive IF gain, 0 to 40 dB in 8 dB steps; a value between steps is rounded down. -1 follows the gain. Start \
         at 16 dB and raise LNA and VGA by about the same amount.",
    );
/// The VGA gain by hand. -1 follows the gain.
pub const VGA: Setting<field::I64> = Setting::new("vga_db", "Gain stage: VGA", "dB", -1)
    .range(-1.0, 62.0, 1.0)
    .help("The receive baseband gain, 0 to 62 dB in 2 dB steps; an odd value is rounded down. -1 follows the gain.");
/// The RF amplifier while receiving.
pub const AMP: Setting<field::Bool> = Setting::new("amp", "RF amplifier", "", false).help(
    "The front-end amplifier, about 11 dB, less at high frequencies. The antenna port takes at most -5 dBm: more can \
     damage the device. Use an attenuator for strong signals.",
);
/// The 10 MHz clock output.
pub const CLOCK_OUT: Setting<field::Bool> = Setting::new("clock_out", "Clock output", "", false)
    .section("advanced")
    .help(
        "A 10 MHz, 3.3 V square wave on CLKOUT for a high-impedance load, to lock another device.",
    );

sdrkit_core::settings! {
    /// The own receive settings of the HackRF driver.
    pub enum HackrfSetting {
        /// [`LNA`].
        Lna = LNA,
        /// [`VGA`].
        Vga = VGA,
        /// [`AMP`].
        Amp = AMP,
        /// [`CLOCK_OUT`].
        ClockOut = CLOCK_OUT,
    }
}

// ---- Readings ----

/// The board and its revision.
pub const BOARD: Field<field::Text> = Field::new("board", "Board", "");
/// The board id.
pub const BOARD_ID: Field<field::I64> = Field::new("board_id", "Board id", "");
/// The platforms the firmware was built for.
pub const PLATFORMS: Field<field::Text> = Field::new("platforms", "Firmware platforms", "");
/// The firmware version string.
pub const FIRMWARE: Field<field::Text> = Field::new("firmware", "Firmware", "");
/// The USB API version of the firmware.
pub const USB_API: Field<field::Text> = Field::new("usb_api", "USB API", "");
/// The part id of the chip.
pub const PART_ID: Field<field::Text> = Field::new("part_id", "Part id", "");
/// The serial number.
pub const SERIAL: Field<field::Text> = Field::new("serial", "Serial number", "");
/// The clock the device uses: the external 10 MHz or its own.
pub const CLOCK_IN: Field<field::Text> = Field::new("clock_in", "Clock", "");
/// The board shows a DC spike at the tuned centre.
pub const DC_SPIKE: Field<field::Bool> = Field::new("dc_spike", "DC spike at the centre", "");
/// The tune offset the maker advises at the present rate: a quarter of the rate on boards with a DC spike, 0 on the
/// HackRF Pro. The driver itself tunes exactly where it is told.
pub const TUNE_OFFSET: Field<field::Hz> = Field::new("tune_offset_hz", "Advised tune offset", "Hz");
/// The last switch from receive to transmit.
pub const RX_TO_TX: Field<field::F64> = Field::new("rx_to_tx_ms", "Receive to transmit", "ms");
/// The last switch from transmit to receive.
pub const TX_TO_RX: Field<field::F64> = Field::new("tx_to_rx_ms", "Transmit to receive", "ms");
/// The rate the device samples at, from its streaming state (USB API 0x0106 or later).
pub const MEASURED_RATE: Field<field::Hz> =
    Field::new("measured_rate_hz", "Measured sample rate", "Hz");
/// Shortfalls the device reported: samples dropped in receive, underruns in transmit.
pub const SHORTFALLS: Field<field::I64> = Field::new("shortfalls", "Device shortfalls", "");

/// The readings of a HackRF.
pub const READINGS: &[&dyn Declare] = &[
    &BOARD,
    &BOARD_ID,
    &PLATFORMS,
    &FIRMWARE,
    &USB_API,
    &PART_ID,
    &SERIAL,
    &CLOCK_IN,
    &DC_SPIKE,
    &TUNE_OFFSET,
    &RX_TO_TX,
    &TX_TO_RX,
    &SHORTFALLS,
    &MEASURED_RATE,
    &TX_OUTPUT_DBM,
];

/// Writes the readings of `link` into `results`.
pub(crate) fn write_readings(link: &Link, results: &mut Results) {
    let f = &link.facts;
    let rate = lock(&link.rx).rate;
    let s = lock(&link.stats).clone();
    let mut r = results.fields_mut();
    r.set(&BOARD, &f.name());
    r.set(&BOARD_ID, i64::from(f.board_id));
    r.opt(
        &PLATFORMS,
        f.platforms.map(proto::platforms_text).as_deref(),
    );
    r.set(&FIRMWARE, &f.version);
    r.set(&USB_API, &proto::api_text(f.api));
    r.opt(&PART_ID, f.part_serial.map(|p| p.part_text()).as_deref());
    r.set(&SERIAL, &f.serial());
    r.opt(
        &CLOCK_IN,
        s.clock_in.map(|on| {
            if on {
                "external 10 MHz in use"
            } else {
                "own clock"
            }
        }),
    );
    r.set(&DC_SPIKE, f.board.dc_spike());
    r.set(
        &TUNE_OFFSET,
        if f.board.dc_spike() { rate / 4.0 } else { 0.0 },
    );
    r.opt(&RX_TO_TX, s.rx_to_tx.map(|d| d.as_secs_f64() * 1e3));
    r.opt(&TX_TO_RX, s.tx_to_rx.map(|d| d.as_secs_f64() * 1e3));
    r.set(&SHORTFALLS, i64::try_from(s.shortfalls).unwrap_or(i64::MAX));
    r.opt(&MEASURED_RATE, s.measured_rate);
}

// ---- The kind: the devices table and its command ----

/// Open each free HackRF for a moment to read its board and firmware.
pub const PROBE: Setting<field::Bool> = Setting::new("probe", "Read board and firmware", "", true)
    .help("Opens each free HackRF for a moment to read its board and firmware. Off lists what the bus shows.");

/// Lists the HackRF devices on the bus in [`DEVICES`].
pub const DISCOVER: Command = Command::new("discover", "Find HackRF devices")
    .help(
        "Lists the HackRF devices on the USB bus, with a device in DFU mode marked as not usable.",
    )
    .args(&[&PROBE]);

sdrkit_core::commands! {
    /// The commands of the HackRF kind.
    pub enum HackrfKindCommand {
        /// [`DISCOVER`].
        Discover = DISCOVER,
    }
}

/// The address that opens the device.
pub const D_ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
/// The name of the device.
pub const D_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The board, if the device was opened.
pub const D_BOARD: Field<field::Text> = Field::new("board", "Board", "");
/// The firmware version string, if the device was opened.
pub const D_FIRMWARE: Field<field::Text> = Field::new("firmware", "Firmware", "");
/// The serial number.
pub const D_SERIAL: Field<field::Text> = Field::new("serial", "Serial number", "");
/// The bus position, `<bus>-<port path>`.
pub const D_PATH: Field<field::Text> = Field::new("path", "Bus position", "");
/// free, busy, dfu, or the key of the reason it cannot be opened.
pub const D_STATE: Field<field::Text> = Field::new("state", "State", "");

/// The HackRF devices the last search found, one row per address.
pub static DEVICES: TableDef = TableDef {
    key: "devices",
    label: "HackRF devices",
    columns: &[
        &D_ADDRESS,
        &D_NAME,
        &D_BOARD,
        &D_FIRMWARE,
        &D_SERIAL,
        &D_PATH,
        &D_STATE,
    ],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

const fn iq(sample: SampleType, min: f64) -> StreamFormat {
    StreamFormat {
        kind: Kind::Iq,
        sample,
        min_rate_hz: min,
        max_rate_hz: proto::MAX_RATE_HZ,
        channels: 1,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "hackrf",
    name: "HackRF",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "hackrf: (the first free HackRF)",
        "hackrf:<serial> (or its last digits)",
        "hackrf:#<n> (the n-th HackRF, from 0)",
        "hackrf:@<bus>-<port path>",
    ],
    address: &[AddressPart::text("device", "Device", "", Syntax::Name).help(
        "Empty for the first free HackRF, a serial number or its last digits, #n for the n-th HackRF from 0, or \
         @bus-ports for a bus position.",
    )],
    settings: &[HackrfSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[iq(SampleType::Cs8, 2e6)],
    accepts: &[iq(SampleType::Cf32, 2e6)],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: HackrfKindCommand::COMMANDS,
    kind_tables: &[&DEVICES],
};

/// Advice shown with a loss of samples.
pub const USB_ADVICE: &str =
    "use a USB 2.0 port of the computer itself, without a hub, or a lower sample rate";

/// The HackRF driver.
#[derive(Clone)]
pub struct HackrfDriver {
    bus: Arc<dyn UsbBus>,
    timing: Timing,
}

impl HackrfDriver {
    /// A driver for the devices of `bus`, with the default timings.
    pub fn new(bus: Arc<dyn UsbBus>) -> Self {
        Self::with_timing(bus, Timing::default())
    }

    /// A driver with its own timings, for tests.
    pub fn with_timing(bus: Arc<dyn UsbBus>, timing: Timing) -> Self {
        HackrfDriver { bus, timing }
    }

    /// A driver for the USB bus of this system.
    pub fn platform() -> Self {
        Self::new(sdrkit_driver_usb::platform_bus())
    }

    /// The HackRF devices on the bus, with board and firmware read where `probe` is set and the device is free.
    pub fn devices(&self, probe: bool) -> Vec<DeviceRow> {
        let Ok(list) = self.bus.devices() else {
            return Vec::new();
        };
        list.iter()
            .filter(|d| is_hackrf(d) || is_dfu(d))
            .map(|d| self.row(d, probe))
            .collect()
    }

    fn row(&self, d: &UsbDeviceInfo, probe: bool) -> DeviceRow {
        let address = Selector::pinned(d).address();
        if is_dfu(d) {
            return DeviceRow {
                address,
                name: "HackRF in DFU mode, not usable".into(),
                board: None,
                firmware: None,
                serial: d.serial.clone(),
                path: d.path(),
                state: "dfu".into(),
            };
        }
        let mut row = DeviceRow {
            address,
            name: d.product.clone().unwrap_or_else(|| "HackRF".into()),
            board: None,
            firmware: None,
            serial: d.serial.clone(),
            path: d.path(),
            state: d.blocked.map_or("free", |u| u.key()).into(),
        };
        if probe && d.blocked.is_none() {
            let read = self
                .bus
                .open(d.id)
                .map_err(HackrfError::from)
                .and_then(|mut usb| {
                    usb.claim(proto::INTERFACE)?;
                    identify(usb.as_mut(), d, self.timing.control)
                });
            match read {
                Ok(f) => {
                    row.board = Some(f.name());
                    row.firmware = Some(f.version.clone());
                    row.serial = Some(f.serial());
                }
                Err(e) => {
                    row.state = match (&e, e.unavailable()) {
                        (HackrfError::Usb(sdrkit_driver_usb::UsbError::Busy), _) => "busy".into(),
                        (_, Some(u)) => u.key().into(),
                        _ => "error".into(),
                    }
                }
            }
        }
        row
    }
}

/// One row of [`DEVICES`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    /// The address that opens the device.
    pub address: String,
    /// The product name, or the note of a device in DFU mode.
    pub name: String,
    /// The board, if it was read.
    pub board: Option<String>,
    /// The firmware version, if it was read.
    pub firmware: Option<String>,
    /// The serial number.
    pub serial: Option<String>,
    /// The bus position.
    pub path: String,
    /// free, busy, dfu, error, or the key of the reason it cannot be opened.
    pub state: String,
}

impl SourceDriver for HackrfDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let sel = Selector::parse(address)?;
        let ctx = Arc::new(Ctx {
            bus: self.bus.clone(),
            timing: self.timing.clone(),
            wanted: Mutex::new(sel),
            current: Mutex::new(None),
        });
        let c = ctx.clone();
        let opener: Opener<HackrfRadio> =
            Arc::new(move |values, _abort, notices| open_radio(&c, values, notices));
        let cfg = RunnerConfig {
            reconnect_gap: self.timing.reconnect_gap,
            ..RunnerConfig::new(format!("HackRF {address}"), USB_ADVICE)
        };
        let runner = StreamRunner::open(cfg, opener, values)?;
        Ok(Box::new(HackrfSource {
            runner,
            ctx,
            tx: None,
        }))
    }

    fn invoke_kind(
        &self,
        id: CommandId,
        args: &Values,
        results: &mut Published,
    ) -> Result<Option<Record>, DriverError> {
        match HackrfKindCommand::from_id(id) {
            Some(HackrfKindCommand::Discover) => {
                results.clear_rows(&DEVICES);
                for d in self.devices(args.get(&PROBE)) {
                    let Some(mut row) = results.row_mut(&DEVICES, &d.address) else {
                        continue;
                    };
                    row.set(&D_ADDRESS, &d.address);
                    row.set(&D_NAME, &d.name);
                    row.opt(&D_BOARD, d.board.as_deref());
                    row.opt(&D_FIRMWARE, d.firmware.as_deref());
                    row.opt(&D_SERIAL, d.serial.as_deref());
                    row.set(&D_PATH, &d.path);
                    row.set(&D_STATE, &d.state);
                }
                Ok(None)
            }
            None => Err(DriverError::NotNow {
                state: Doing::Closed,
            }),
        }
    }

    fn settings_at(&self, address: &str) -> Result<Vec<SettingDecl>, DriverError> {
        Selector::parse(address).map(|_| Vec::new())
    }
}

/// What the openings of one source share: the bus, the selector (pinned to the device after the first open) and the
/// link that is open now.
struct Ctx {
    bus: Arc<dyn UsbBus>,
    timing: Timing,
    wanted: Mutex<Selector>,
    current: Mutex<Option<Arc<Link>>>,
}

/// Opens the device that the selector of `ctx` names: claim, identification, mode off. The selector is then pinned to
/// the serial number (or the bus position) of the device, so a re-plug finds the same device.
fn open_radio(
    ctx: &Ctx,
    values: &Values,
    notices: &sdrkit_driver_core::runner::Notices,
) -> Result<HackrfRadio, DriverError> {
    let list = ctx.bus.devices().map_err(HackrfError::from)?;
    let sel = lock(&ctx.wanted).clone();
    let first = sel == Selector::First;
    let mut last = HackrfError::NotFound;
    for info in select::find(&list, &sel)? {
        match open_link(ctx, &info, values) {
            Ok(link) => {
                let link = Arc::new(link);
                *lock(&ctx.wanted) = Selector::pinned(&info);
                *lock(&ctx.current) = Some(link.clone());
                let f = &link.facts;
                notices.log(format!(
                    "HackRF: {}, firmware {} (USB API {}), serial {}",
                    f.name(),
                    f.version,
                    proto::api_text(f.api),
                    f.serial()
                ));
                return Ok(HackrfRadio { link });
            }
            Err(OpenFail::Hackrf(e @ HackrfError::Usb(sdrkit_driver_usb::UsbError::Busy)))
                if first =>
            {
                last = e;
            }
            Err(OpenFail::Hackrf(e)) => return Err(e.into()),
            Err(OpenFail::Driver(e)) => return Err(e),
        }
    }
    Err(last.into())
}

enum OpenFail {
    Hackrf(HackrfError),
    Driver(DriverError),
}

impl From<HackrfError> for OpenFail {
    fn from(e: HackrfError) -> Self {
        OpenFail::Hackrf(e)
    }
}

fn open_link(ctx: &Ctx, info: &UsbDeviceInfo, values: &Values) -> Result<Link, OpenFail> {
    let t = &ctx.timing;
    let mut usb = ctx.bus.open(info.id).map_err(HackrfError::from)?;
    usb.claim(proto::INTERFACE).map_err(HackrfError::from)?;
    let facts = identify(usb.as_mut(), info, t.control)?;
    let rx = RxConfig::from_values(values, &facts).map_err(OpenFail::Driver)?;
    let mut ctl = Ctl::new(usb, facts.api, facts.board, t.control, t.mode_off);
    ctl.set_mode(proto::Mode::Off)?;
    Ok(Link::new(ctl, facts, t.clone(), rx))
}

/// An open HackRF: the runner of its receive side and its transmitter.
struct HackrfSource {
    runner: StreamRunner<HackrfRadio>,
    ctx: Arc<Ctx>,
    tx: Option<Arc<tx::TxShared>>,
}

impl Source for HackrfSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.runner.start(sink)
    }
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        self.runner.apply(values)
    }
    /// Closes the transmitter first, then stops receive and closes the device.
    fn stop(&mut self) {
        if let Some(t) = self.tx.take() {
            t.close();
        }
        self.runner.stop();
        *lock(&self.ctx.current) = None;
    }
    fn sample_type(&self) -> SampleType {
        SampleType::Cs8
    }
    fn link(&self) -> LinkState {
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
        Some(sdrkit_driver_core::tx_schema(
            HackrfTxSetting::SETTINGS,
            limits,
        ))
    }
    /// The transmitter of the device open now. The engine setting `transmit_allowed` is the only gate; the driver adds
    /// none.
    fn transmitter(&mut self, values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        if self.tx.as_ref().is_some_and(|t| !t.is_closed()) {
            return Err(DriverError::NotNow {
                state: Doing::Running,
            });
        }
        let link = lock(&self.ctx.current)
            .clone()
            .filter(|l| !l.gone())
            .ok_or(DriverError::NotNow {
                state: Doing::Stopped,
            })?;
        let (t, shared) = tx::HackrfTransmitter::open(link, values)?;
        self.tx = Some(shared);
        Ok(Box::new(t))
    }
}

impl Drop for HackrfSource {
    fn drop(&mut self) {
        self.stop();
    }
}
