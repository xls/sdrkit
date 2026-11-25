//! The USB devices that a driver of the engine takes: the engine's table `devices`, the events of an arrival and a
//! removal, and the choice of the device that a USB address opens.
//!
//! The control thread reads the watch of the bus at each tick without waiting, and lists the bus again only after an
//! arrival or a removal. Listing opens no device. The engine learns that a device is busy, or that the user may not
//! open it, from its own opens that fail.

use crate::engine_owner::EngineOwner;
use crate::feed::lock;
use crate::{radio_key, RadioId, Shared};
use sdrkit_core::interface::{EventKind, Fields, Table};
use sdrkit_driver_core::{DriverError, Unavailable};
use sdrkit_driver_registry::{usb_family, usb_radio, UsbRadio};
use sdrkit_driver_usb::{Availability, UsbBus, UsbEvent, UsbId, UsbWatch};
use std::collections::HashMap;
use std::time::Duration;

sdrkit_core::choice_enum! {
    /// What can be done with an attached USB device.
    pub enum DeviceState {
        /// No program has it open: the engine can open it.
        Free = ("free", "Free"),
        /// The engine has it open as a radio.
        Open = ("open", "Open"),
        /// The user may not open it (Linux: no udev rule; Android: no permission from the system).
        NoPermission = ("no_permission", "No permission"),
        /// The system has not bound it to a driver that programs can use (Windows: WinUSB), or a driver of the system
        /// holds it.
        NeedsDriver = ("needs_driver", "Needs a driver"),
        /// Another program has it open.
        Busy = ("busy", "Busy"),
    }
}

/// A row of the table `devices`. The row key is the bus position, `<bus>-<port path>`, which stays while the device
/// stays attached.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DeviceRow<'a> {
    /// The address that opens the device, `usb:<family>:<selector>`. It finds the same device after a re-plug.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The driver kind that takes the device.
    #[field(label = "Driver", refers_to = "drivers")]
    pub driver: &'a str,
    /// The manufacturer and product strings of the device, or the name of its family.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The serial number string.
    #[field(label = "Serial number")]
    pub serial: Option<&'a str>,
    /// The bus position, `<bus>-<port path>`.
    #[field(label = "USB port")]
    pub path: &'a str,
    /// What can be done with the device.
    #[field(label = "State")]
    pub state: DeviceState,
    /// The radio that the device is open as.
    #[field(label = "Radio", refers_to = "radios")]
    pub radio: Option<&'a str>,
}

/// The attached USB devices that a driver of the engine takes.
pub static DEVICES: Table<DeviceRow<'static>> =
    Table::new("devices", "USB devices", 256).label(&DeviceRow::NAME);

/// The fields of the event `device.arrived`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DeviceArrivedEvent<'a> {
    /// The address that opens the device.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The driver kind that takes the device.
    #[field(label = "Driver")]
    pub driver: &'a str,
    /// The name of the device.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The bus position: the row key of the device in the table `devices`.
    #[field(label = "USB port")]
    pub path: &'a str,
    /// What can be done with the device.
    #[field(label = "State")]
    pub state: DeviceState,
}

/// The fields of the event `device.left`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DeviceLeftEvent<'a> {
    /// The address that opened the device.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The driver kind that took the device.
    #[field(label = "Driver")]
    pub driver: &'a str,
    /// The name of the device.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The bus position: the row key that the device had in the table `devices`.
    #[field(label = "USB port")]
    pub path: &'a str,
}

/// A USB device that a driver of the engine takes was attached.
pub static DEVICE_ARRIVED: EventKind<DeviceArrivedEvent<'static>> =
    EventKind::new("device.arrived", "USB device arrived");
/// A USB device of the table `devices` was removed.
pub static DEVICE_LEFT: EventKind<DeviceLeftEvent<'static>> =
    EventKind::new("device.left", "USB device left");

/// The columns of the table `drivers` that say whether the radios of a kind can be reached on this system.
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct DriverAvailability {
    /// False if no radio of the kind can be opened on this system.
    #[field(label = "Available")]
    pub available: bool,
    /// Why no radio of the kind can be opened. No value for a kind that is available.
    #[field(label = "Unavailable because")]
    pub unavailable_why: Option<Unavailable>,
}

/// The availability of the driver kind `driver` on `bus`: a kind of USB radio is as available as the bus.
pub(crate) fn availability(driver: &str, bus: &dyn UsbBus) -> DriverAvailability {
    let why = match (usb_family(driver), bus.availability()) {
        (Some(_), Availability::Unavailable(u)) => {
            <Unavailable as sdrkit_core::Choice>::from_key(u.key())
        }
        _ => None,
    };
    DriverAvailability {
        available: why.is_none(),
        unavailable_why: why,
    }
}

/// How often the bus is listed while it has no watch.
const RELIST: Duration = Duration::from_secs(2);

/// One attached device of the table.
struct Listed {
    id: UsbId,
    path: String,
    serial: Option<String>,
    /// Why the system lets no program open the device, as the listing says it.
    blocked: Option<Unavailable>,
    radio: UsbRadio,
}

/// What the engine knows of the USB bus between two ticks.
#[derive(Default)]
pub(crate) struct UsbState {
    started: bool,
    watch: Option<Box<dyn UsbWatch>>,
    listed: Vec<Listed>,
    /// What an open of the engine that failed said of the device at a bus position.
    notes: HashMap<String, DeviceState>,
    /// The bus position of the device of each open USB radio.
    open: HashMap<u64, String>,
    /// The rows are out of date.
    dirty: bool,
    last_list: Option<std::time::Instant>,
}

/// The state that the reason `why` gives a device. `None` for a reason that is not one of a device that is attached.
fn blocked_state(why: Unavailable) -> Option<DeviceState> {
    match why {
        Unavailable::NoPermission => Some(DeviceState::NoPermission),
        Unavailable::KernelDriver | Unavailable::NeedsWinUsb => Some(DeviceState::NeedsDriver),
        Unavailable::NoUsbHere | Unavailable::NotFound => None,
    }
}

/// What a failed open says of the device, if the failure is one that stays with the device.
fn failed_state(e: &DriverError) -> Option<DeviceState> {
    match e {
        DriverError::Busy => Some(DeviceState::Busy),
        DriverError::Unavailable { why } => blocked_state(*why),
        _ => None,
    }
}

/// The devices of `bus` that a driver takes, in the order of the bus.
fn list(bus: &dyn UsbBus) -> Vec<Listed> {
    let all = bus.devices().unwrap_or_default();
    all.iter()
        .filter_map(|d| {
            Some(Listed {
                id: d.id,
                path: d.path(),
                serial: d.serial.clone().filter(|s| !s.is_empty()),
                blocked: d
                    .blocked
                    .and_then(|u| <Unavailable as sdrkit_core::Choice>::from_key(u.key())),
                radio: usb_radio(d, &all)?,
            })
        })
        .collect()
}

impl UsbState {
    fn state_of(&self, d: &Listed) -> (DeviceState, Option<RadioId>) {
        let radio = self
            .open
            .iter()
            .find(|(_, path)| **path == d.path)
            .map(|(r, _)| RadioId::from_u64(*r));
        let state = match (radio, d.blocked.and_then(blocked_state)) {
            (Some(_), _) => DeviceState::Open,
            (None, Some(b)) => b,
            (None, None) => self
                .notes
                .get(&d.path)
                .copied()
                .unwrap_or(DeviceState::Free),
        };
        (state, radio)
    }

    /// The row of every listed device, as owned values: the engine owner is written after the lock of this state is
    /// released, because the lock of the engine owner comes first in the order of the locks.
    fn rows(&self) -> Vec<OwnedRow> {
        self.listed
            .iter()
            .map(|d| {
                let (state, radio) = self.state_of(d);
                OwnedRow {
                    path: d.path.clone(),
                    address: d.radio.address.clone(),
                    driver: d.radio.family.driver,
                    name: d.radio.name.clone(),
                    serial: d.serial.clone(),
                    state,
                    radio: radio.map(radio_key),
                }
            })
            .collect()
    }
}

/// One row of the table `devices`, with its key `path`.
struct OwnedRow {
    path: String,
    address: String,
    driver: &'static str,
    name: String,
    serial: Option<String>,
    state: DeviceState,
    radio: Option<String>,
}

/// Writes `rows` into the table `devices` of `e` and removes the rows with the keys `gone`.
fn write_rows(e: &mut EngineOwner, rows: &[OwnedRow], gone: &[String]) {
    for path in gone {
        e.results.remove_row(&DEVICES, path);
    }
    for r in rows {
        DEVICES.put(
            &mut e.results,
            &r.path,
            &DeviceRow {
                address: &r.address,
                driver: r.driver,
                name: &r.name,
                serial: r.serial.as_deref(),
                path: &r.path,
                state: r.state,
                radio: r.radio.as_deref(),
            },
        );
    }
}

/// A device that a USB address selects: its bus position, the selector that names it alone, and why the system lets no
/// program open it, if the listing says so.
pub(crate) struct Picked {
    pub(crate) path: String,
    pub(crate) selector: String,
    pub(crate) blocked: Option<Unavailable>,
}

impl Shared {
    /// Brings the table `devices` up to date at a tick: reads the arrivals and removals that the watch of the bus holds,
    /// without waiting, lists the bus again if there were any, and writes the rows and the events `device.arrived` and
    /// `device.left`. The first call lists the devices that are attached already; they raise no event.
    pub(crate) fn step_devices(&self, now: f64) {
        let mut st = lock(&self.usb_state);
        let first = !std::mem::replace(&mut st.started, true);
        if first {
            st.watch = self.usb.watch().ok();
        }
        let mut changed = first;
        let mut stopped = false;
        if let Some(w) = st.watch.as_mut() {
            loop {
                match w.next(Duration::ZERO) {
                    Ok(Some(UsbEvent::Arrived(_) | UsbEvent::Left(_))) => changed = true,
                    Ok(None) => break,
                    Err(_) => {
                        stopped = true;
                        break;
                    }
                }
            }
        } else if self.usb.availability() == Availability::Available {
            // A bus that cannot be watched is listed now and then.
            changed |= st.last_list.is_none_or(|t| t.elapsed() >= RELIST);
        }
        if stopped {
            st.watch = None;
        }
        if !changed && !st.dirty {
            return;
        }
        let mut arrived = Vec::new();
        let mut left = Vec::new();
        if changed {
            st.last_list = Some(std::time::Instant::now());
            let new = list(self.usb.as_ref());
            let old = std::mem::take(&mut st.listed);
            for d in &old {
                if !new.iter().any(|n| n.id == d.id) {
                    st.notes.remove(&d.path);
                    left.push((
                        d.path.clone(),
                        d.radio.address.clone(),
                        d.radio.family.driver,
                        d.radio.name.clone(),
                    ));
                }
            }
            if !first {
                arrived.extend(
                    new.iter()
                        .filter(|n| !old.iter().any(|d| d.id == n.id))
                        .map(|n| n.id),
                );
            }
            st.listed = new;
        }
        st.dirty = false;
        // A device that came back at the position of one that left keeps the row.
        let gone: Vec<String> = left
            .iter()
            .map(|(path, ..)| path.clone())
            .filter(|p| !st.listed.iter().any(|d| d.path == *p))
            .collect();
        let mut events = Vec::new();
        for (path, address, driver, name) in &left {
            events.push(DEVICE_LEFT.event(
                now,
                0.0,
                &DeviceLeftEvent {
                    address,
                    driver,
                    name,
                    path,
                },
            ));
        }
        for d in st.listed.iter().filter(|d| arrived.contains(&d.id)) {
            events.push(DEVICE_ARRIVED.event(
                now,
                0.0,
                &DeviceArrivedEvent {
                    address: &d.radio.address,
                    driver: d.radio.family.driver,
                    name: &d.radio.name,
                    path: &d.path,
                    state: st.state_of(d).0,
                },
            ));
        }
        let rows = st.rows();
        drop(st);
        let mut e = self.engine.lock();
        write_rows(&mut e, &rows, &gone);
        for ev in events {
            e.note_event(ev, &self.events);
        }
    }

    /// The devices that the selector `selector` of the USB driver kind `driver` names, in the order to try them: for the
    /// empty selector the devices of the family that the engine does not have open, those that the system blocks last;
    /// else the one device with the serial number, the place in the list (`#<n>`) or the bus position
    /// (`@<bus>-<port path>`). Empty for a kind that is no USB radio, a bus that
    /// lists nothing, and a selector that names no listed device; the driver then reads the selector itself.
    pub(crate) fn usb_candidates(&self, driver: &str, selector: &str) -> Vec<Picked> {
        let Some(family) = usb_family(driver) else {
            return Vec::new();
        };
        let all: Vec<Listed> = list(self.usb.as_ref())
            .into_iter()
            .filter(|d| d.radio.family.driver == family.driver)
            .collect();
        let st = lock(&self.usb_state);
        let free = |d: &&Listed| !st.open.values().any(|p| *p == d.path);
        let sel = selector.trim();
        let found: Vec<&Listed> = if sel.is_empty() {
            let mut free: Vec<&Listed> = all.iter().filter(free).collect();
            free.sort_by_key(|d| d.blocked.is_some());
            free
        } else if let Some(n) = sel.strip_prefix('#') {
            n.parse::<usize>()
                .ok()
                .and_then(|n| all.get(n))
                .into_iter()
                .collect()
        } else if let Some(p) = sel.strip_prefix('@') {
            all.iter().filter(|d| d.path == p).collect()
        } else {
            let with = |end: bool| -> Vec<&Listed> {
                let sel = sel.to_ascii_lowercase();
                all.iter()
                    .filter(|d| {
                        d.serial.as_deref().is_some_and(|s| {
                            let s = s.to_ascii_lowercase();
                            s == sel || (end && s.ends_with(&sel))
                        })
                    })
                    .collect()
            };
            // A family can take the last digits of a serial number. Two devices that match: the driver decides.
            match (with(false), family.serial_suffix) {
                (exact, _) if exact.len() == 1 => exact,
                (exact, true) if exact.is_empty() => Some(with(true))
                    .filter(|m| m.len() == 1)
                    .unwrap_or_default(),
                _ => Vec::new(),
            }
        };
        found
            .into_iter()
            .map(|d| Picked {
                path: d.path.clone(),
                selector: d.radio.selector.clone(),
                blocked: d.blocked,
            })
            .collect()
    }

    /// Notes that radio `radio` is the device at the bus position `path`.
    pub(crate) fn usb_opened(&self, radio: RadioId, path: &str) {
        let mut st = lock(&self.usb_state);
        st.notes.remove(path);
        st.open.insert(radio.to_u64(), path.to_owned());
        st.dirty = true;
    }

    /// Notes what the failed open `error` says of the device at the bus position `path`.
    pub(crate) fn usb_failed(&self, path: &str, error: &DriverError) {
        if let Some(state) = failed_state(error) {
            let mut st = lock(&self.usb_state);
            st.notes.insert(path.to_owned(), state);
            st.dirty = true;
        }
    }

    /// Notes that radio `radio` is closed: its device is free again.
    pub(crate) fn usb_closed(&self, radio: RadioId) {
        let mut st = lock(&self.usb_state);
        if st.open.remove(&radio.to_u64()).is_some() {
            st.dirty = true;
        }
    }
}
