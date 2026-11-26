//! The kind of radio: its settings, its descriptor, the address selectors, the `discover` command with its `devices`
//! table, and the open of a dongle on the shared runner with re-plug.

use crate::device::{probe_tuner, Rtl2832, TunerKind};
use crate::ids::is_rtl;
use crate::radio::{OpenSet, RtlRadio, PLL_LOCKED, TUNER};
use crate::RtlError;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Command, CommandId, CommandSet, Field, Published, Record, Setting, SettingSet, TableDef, Values,
};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner};
use sdrkit_driver_core::{
    AddressPart, DriverDescriptor, DriverError, Opens, Source, SourceDriver, StreamFormat, Syntax,
};
use sdrkit_driver_usb::{UsbBus, UsbDevice, UsbDeviceInfo, UsbError, UsbId};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

sdrkit_core::choice_enum! {
    /// Direct sampling ([`DIRECT_SAMPLING`]).
    pub enum DirectSampling {
        /// The tuner at every frequency.
        Off = ("off", "Off"),
        /// The I input below 24 MHz.
        I = ("i", "I input"),
        /// The Q input below 24 MHz.
        Q = ("q", "Q input"),
    }
}

/// The DC offset of the RTL2832U removed in software.
pub const DC_REMOVAL: Setting<field::Bool> = sdrkit_driver_core::form::DC_REMOVAL;
/// The frequency correction.
pub const PPM: Setting<field::F64> = sdrkit_driver_core::form::PPM
    .range(-200.0, 200.0, 0.1)
    .help(
        "The clock error of the dongle in ppm, positive if its clock runs fast (signals then show a little low). Dongles \
         without a TCXO are often 20 to 100 ppm off.",
    );
/// Direct sampling below 24 MHz.
pub const DIRECT_SAMPLING: Setting<field::Choice<DirectSampling>> =
    sdrkit_driver_core::form::direct_sampling(DirectSampling::Off).help(
        "Below 24 MHz, sample the antenna input on the I or the Q input of the ADC and leave the tuner off. An RTL-SDR Blog \
         V3 has its HF input on Q; other dongles need a change to the board. A Blog V4 receives HF through its own \
         upconverter and does not use this.",
    );
/// Offset tuning of a zero-IF tuner.
pub const OFFSET_TUNING: Setting<field::Bool> =
    Setting::new("offset_tuning", "Offset tuning", "", false)
        .section(sdrkit_driver_core::form::CORRECTIONS_SECTION)
        .help(
        "Tune an E4000 tuner 85 % of half the sample rate away and shift back in the demodulator, so that the DC spike of \
         the tuner is not at the centre. An R820T or R828D tuner has an IF of its own and does not use this.",
    );
/// The digital AGC of the RTL2832U.
pub const RTL_AGC: Setting<field::Bool> = Setting::new("rtl_agc", "RTL2832U digital gain", "", false)
    .section(sdrkit_driver_core::form::GAIN_SECTION)
    .advanced()
    .help("The digital gain stage of the RTL2832U after the ADC. Off: the samples keep the level that the tuner gives.");

/// The detector loops of the tuner as the automatic gain.
pub const TUNER_AGC: Setting<field::Bool> = Setting::new("tuner_agc", "Tuner AGC loops", "", false)
    .section(sdrkit_driver_core::form::GAIN_SECTION)
    .advanced()
    .help(
        "With automatic gain on, let the detectors of the tuner set its gain. Off: the driver holds a fixed tuner gain that it \
         chooses from the level of the samples, which keeps strong stations nearby from overloading the dongle.",
    );

sdrkit_core::settings! {
    /// The own settings of the RTL2832U driver.
    pub enum RtlSetting {
        /// [`RTL_AGC`].
        RtlAgc = RTL_AGC,
        /// [`TUNER_AGC`].
        TunerAgc = TUNER_AGC,
        /// [`PPM`].
        Ppm = PPM,
        /// [`DC_REMOVAL`](sdrkit_driver_core::form::DC_REMOVAL).
        DcRemoval = DC_REMOVAL,
        /// [`OFFSET_TUNING`].
        OffsetTuning = OFFSET_TUNING,
        /// [`DIRECT_SAMPLING`].
        DirectSampling = DIRECT_SAMPLING,
    }
}

/// Lists the attached dongles in [`DEVICES`].
pub const DISCOVER: Command = Command::new("discover", "Find dongles").help(
    "Lists the RTL2832U dongles on USB. A dongle that no program has open is opened for a moment to read which tuner it \
     has.",
);

sdrkit_core::commands! {
    /// The commands of the driver kind.
    pub enum RtlKindCommand {
        /// [`DISCOVER`].
        Discover = DISCOVER,
    }
}

/// The address that opens the dongle.
pub const D_ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
/// The manufacturer and product strings.
pub const D_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The serial number string.
pub const D_SERIAL: Field<field::Text> = Field::new("serial", "Serial number", "");
/// The tuner, when it is known.
pub const D_TUNER: Field<field::Text> = Field::new("tuner", "Tuner", "");
/// The bus position, `<bus>-<port>.<port>`.
pub const D_PATH: Field<field::Text> = Field::new("path", "USB port", "");
/// `free`, `open` (by this driver), `busy` (another program), or a key of the reason it cannot be opened.
pub const D_STATE: Field<field::Text> = Field::new("state", "State", "");

/// The dongles that [`DISCOVER`] found, one row per USB port.
pub static DEVICES: TableDef = TableDef {
    key: "devices",
    label: "Dongles",
    columns: &[&D_ADDRESS, &D_NAME, &D_SERIAL, &D_TUNER, &D_PATH, &D_STATE],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The driver key, the scheme of its addresses.
pub const KEY: &str = "rtlusb";

const ADDRESS: &[AddressPart] = &[AddressPart::text("device", "Dongle", "", Syntax::Name).help(
    "Empty for the first free dongle, a serial number, #<n> for the n-th dongle from 0, or @<bus>-<ports> for the dongle at \
     a USB port.",
)];

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: KEY,
    name: "RTL-SDR dongle (USB)",
    opens: Opens::Once,
    addresses: &[
        "rtlusb: (the first free dongle)",
        "rtlusb:<serial> (the dongle with this serial number)",
        "rtlusb:#<n> (the n-th dongle, from 0)",
        "rtlusb:@<bus>-<port>.<port> (the dongle at this USB port)",
    ],
    address: ADDRESS,
    settings: &[RtlSetting::SETTINGS],
    commands: &[],
    readings: &[&TUNER, &PLL_LOCKED],
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cu8,
        min_rate_hz: 225_001.0,
        max_rate_hz: 3.2e6,
        channels: 1,
    }],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: RtlKindCommand::COMMANDS,
    kind_tables: &[&DEVICES],
};

/// Which dongle an address names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// The first dongle that is free.
    First,
    /// The dongle with this serial number.
    Serial(String),
    /// The n-th dongle in the order of the bus, from 0.
    Index(usize),
    /// The dongle at this bus position, `<bus>-<port>.<port>`.
    Path(String),
}

impl Selector {
    /// Reads the selector of an address: the driver's own part, with or without a leading `usb:`, `rtl:` or
    /// `rtlusb:`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] for `#` without a number, or `@` without a position.
    pub fn parse(address: &str) -> Result<Selector, DriverError> {
        let mut a = address.trim();
        for p in ["usb:", "rtlusb:", "rtl:"] {
            a = a.strip_prefix(p).unwrap_or(a);
        }
        if a.is_empty() {
            return Ok(Selector::First);
        }
        if let Some(n) = a.strip_prefix('#') {
            return n
                .parse()
                .map(Selector::Index)
                .map_err(|_| DriverError::Address);
        }
        if let Some(p) = a.strip_prefix('@') {
            if p.is_empty() || p.contains(char::is_whitespace) {
                return Err(DriverError::Address);
            }
            return Ok(Selector::Path(p.to_string()));
        }
        Ok(Selector::Serial(a.to_string()))
    }
}

/// The attached dongles, in the order of the bus.
///
/// # Errors
///
/// What the bus returns.
pub fn dongles(bus: &dyn UsbBus) -> Result<Vec<UsbDeviceInfo>, RtlError> {
    Ok(bus
        .devices()?
        .into_iter()
        .filter(|d| is_rtl(d.vendor_id, d.product_id))
        .collect())
}

/// The address that names `info` among `all`: its serial number when no other dongle has it, else its USB port.
pub fn address_of(info: &UsbDeviceInfo, all: &[UsbDeviceInfo]) -> String {
    match info.serial.as_deref() {
        Some(s)
            if !s.is_empty()
                && all
                    .iter()
                    .filter(|d| d.serial.as_deref() == Some(s))
                    .count()
                    == 1 =>
        {
            format!("{KEY}:{s}")
        }
        _ => format!("{KEY}:@{}", info.path()),
    }
}

/// The selector that finds `info` again after a re-plug.
fn lasting(info: &UsbDeviceInfo, all: &[UsbDeviceInfo]) -> Selector {
    match Selector::parse(&address_of(info, all)) {
        Ok(s) => s,
        Err(_) => Selector::Path(info.path()),
    }
}

/// Opens the dongle that `sel` names. `First` takes the first one that this driver does not have open and that opens.
///
/// # Errors
///
/// [`RtlError::NotFound`] when no dongle matches, else the error of the open.
pub fn open_selected(
    bus: &dyn UsbBus,
    sel: &Selector,
    open: &OpenSet,
) -> Result<(Box<dyn UsbDevice>, Vec<UsbDeviceInfo>), RtlError> {
    let all = dongles(bus)?;
    let pick = |info: &UsbDeviceInfo| bus.open(info.id).map_err(RtlError::from);
    let found = match sel {
        Selector::First => {
            let held = open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let free: Vec<&UsbDeviceInfo> = all
                .iter()
                .filter(|d| !held.contains_key(&d.path()))
                .collect();
            drop(held);
            let mut last = RtlError::NotFound;
            for d in free {
                match pick(d) {
                    Ok(dev) => return Ok((dev, all)),
                    Err(e) => last = e,
                }
            }
            return Err(last);
        }
        Selector::Index(n) => all.get(*n),
        Selector::Path(p) => all.iter().find(|d| d.path() == *p),
        Selector::Serial(s) => all.iter().find(|d| d.serial.as_deref() == Some(s.as_str())),
    };
    let info = found.ok_or(RtlError::NotFound)?;
    let dev = pick(info)?;
    Ok((dev, all))
}

/// The RTL2832U driver. It reaches its dongles through a USB bus.
#[derive(Clone)]
pub struct RtlUsbDriver {
    bus: Arc<dyn UsbBus>,
    open: Arc<OpenSet>,
    tuners: Arc<Mutex<HashMap<UsbId, Option<TunerKind>>>>,
    counter: bool,
    bytes: Arc<AtomicU64>,
}

impl std::fmt::Debug for RtlUsbDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RtlUsbDriver")
    }
}

impl RtlUsbDriver {
    /// A driver for the dongles on `bus`.
    pub fn new(bus: Arc<dyn UsbBus>) -> RtlUsbDriver {
        RtlUsbDriver {
            bus,
            open: Arc::default(),
            tuners: Arc::default(),
            counter: false,
            bytes: Arc::default(),
        }
    }

    /// The same driver, whose dongles stream the counter of the test mode in place of samples: each byte is the
    /// previous byte plus 1 ([`crate::diag::CounterCheck`]). A diagnostic for sample-loss tests; hosts do not see it.
    pub fn with_counter_test(mut self, on: bool) -> RtlUsbDriver {
        self.counter = on;
        self
    }

    /// The bytes that the streams of this driver have read from USB.
    pub fn bytes_read(&self) -> u64 {
        self.bytes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Writes the rows of [`DEVICES`]. A dongle that this driver does not have open and that no probe has seen is
    /// opened for a moment to read its tuner.
    fn discover(&self, results: &mut Published) -> Result<(), RtlError> {
        let all = dongles(self.bus.as_ref())?;
        results.clear_rows(&DEVICES);
        let held = self
            .open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut known = self
            .tuners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        known.retain(|id, _| all.iter().any(|d| d.id == *id));
        for d in &all {
            let path = d.path();
            let (tuner, state) = if let Some(t) = held.get(&path) {
                (Some(*t), "open".to_string())
            } else if let Some(why) = d.blocked {
                (None, why.key().to_string())
            } else if let Some(t) = known.get(&d.id) {
                (*t, "free".to_string())
            } else {
                match self
                    .bus
                    .open(d.id)
                    .map_err(RtlError::from)
                    .and_then(probe_tuner)
                {
                    Ok(t) => {
                        known.insert(d.id, t);
                        (t, "free".to_string())
                    }
                    Err(RtlError::Usb(UsbError::Busy)) => (None, "busy".to_string()),
                    Err(e) => (
                        None,
                        e.unavailable()
                            .map_or_else(|| "busy".to_string(), |u| u.key().to_string()),
                    ),
                }
            };
            let Some(mut row) = results.row_mut(&DEVICES, &path) else {
                continue;
            };
            let name = [d.manufacturer.as_deref(), d.product.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            row.set(&D_ADDRESS, &address_of(d, &all));
            row.set(&D_NAME, &name);
            row.set(&D_SERIAL, d.serial.as_deref().unwrap_or(""));
            row.set(&D_TUNER, tuner.map_or("", TunerKind::name));
            row.set(&D_PATH, &path);
            row.set(&D_STATE, &state);
        }
        Ok(())
    }
}

impl SourceDriver for RtlUsbDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn invoke_kind(
        &self,
        id: CommandId,
        _args: &Values,
        results: &mut Published,
    ) -> Result<Option<Record>, DriverError> {
        match RtlKindCommand::from_id(id) {
            Some(RtlKindCommand::Discover) => {
                self.discover(results)?;
                Ok(None)
            }
            None => Err(DriverError::NotNow {
                state: sdrkit_driver_core::Doing::Closed,
            }),
        }
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let first = Selector::parse(address)?;
        let sel = Arc::new(Mutex::new(first));
        let (bus, open, counter, bytes) = (
            self.bus.clone(),
            self.open.clone(),
            self.counter,
            self.bytes.clone(),
        );
        let opener: Opener<RtlRadio> = Arc::new(move |_values, _abort, notices| {
            let s = sel
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let (dev, all) = open_selected(bus.as_ref(), &s, &open)?;
            let info = dev.info().clone();
            let d = Rtl2832::open(dev)?;
            // A re-plugged dongle has a new id: find it again by serial number or port.
            *sel.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = lasting(&info, &all);
            notices.log(format!(
                "RTL2832U at USB {}: tuner {}",
                info.path(),
                d.tuner().name()
            ));
            Ok(RtlRadio::new(d, open.clone(), counter, bytes.clone()))
        });
        let mut cfg = RunnerConfig::new(
            format!("RTL-SDR {address}"),
            "use a USB 2 port without a hub, or a lower sample rate",
        );
        cfg.open_attempts = 3;
        let runner = StreamRunner::open(cfg, opener, values)?;
        Ok(Box::new(runner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_read_every_form() {
        assert_eq!(Selector::parse("").unwrap(), Selector::First);
        assert_eq!(Selector::parse("usb:rtl:").unwrap(), Selector::First);
        assert_eq!(Selector::parse("#2").unwrap(), Selector::Index(2));
        assert_eq!(Selector::parse("rtl:#0").unwrap(), Selector::Index(0));
        assert_eq!(
            Selector::parse("@1-2.1").unwrap(),
            Selector::Path("1-2.1".into())
        );
        assert_eq!(
            Selector::parse("usb:rtl:00000001").unwrap(),
            Selector::Serial("00000001".into())
        );
        assert!(Selector::parse("#x").is_err());
        assert!(Selector::parse("@").is_err());
    }

    #[test]
    fn an_address_uses_a_unique_serial_or_the_port() {
        let mut a = UsbDeviceInfo::new(UsbId(1), 0x0bda, 0x2838);
        a.bus = "1".into();
        a.ports = vec![2];
        a.serial = Some("00000001".into());
        let mut b = a.clone();
        b.id = UsbId(2);
        b.ports = vec![3];
        assert_eq!(address_of(&a, &[a.clone()]), "rtlusb:00000001");
        assert_eq!(address_of(&a, &[a.clone(), b.clone()]), "rtlusb:@1-2");
    }
}
