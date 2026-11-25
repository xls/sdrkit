//! USB transport for the drivers of USB radios: a list of the attached devices, arrival and removal events, control
//! transfers, queued bulk IN streams and bulk OUT writes.
//!
//! A driver works against the traits [`UsbBus`], [`UsbDevice`], [`BulkIn`] and [`BulkOut`]. The backends:
//!
//! - [`platform_bus`] returns the bus of the operating system: the `nusb` backend on Linux, macOS, Windows and Android
//!   when the cargo feature `nusb` is on, else [`NoUsb`].
//! - [`NoUsb`] has no devices; every open fails with [`UsbError::NoUsbHere`]. It is the bus on iOS and on the web.
//! - `FakeUsb` (cargo feature `fake`) replays a [`Script`] and reports the first call that does not follow it.
//! - [`RecordingUsb`] wraps any bus and writes every exchange into a [`Script`].
//!
//! No call blocks on an async runtime. A call returns when the transfer completes or its timeout passes.
//!
//! # Script files
//!
//! A script is a text file, one item per line. Files for tests are kept under `tests/vectors/usb/<family>/`. A line
//! that starts with `#` and an empty line are ignored. Numbers are hexadecimal where the field is a byte or a word of
//! the USB protocol, decimal elsewhere. `<data>` is a run of hexadecimal digits, `-` for no bytes, or
//! `cut:<len>:<crc>:<head>` for a payload stored cut short: its length in bytes, the CRC-32/ISO-HDLC of all of it in 8
//! digits, and its first bytes. `<err>` is `!` and an error key of [`UsbError::key`] (`!timeout`, `!stall`,
//! `!unplugged`); `!other` and `!script` can carry a quoted text.
//!
//! ```text
//! sdrkit-usb 1
//! mode strict
//! device 1 0bda:2838 bus="1" ports=2.1 manufacturer="Maker" product="Dongle" serial="0001" release=0100 speed=high
//! open 1
//! claim 1 0
//! in 1 c0 00 0000 0010 1 -> 0a
//! out 1 40 00 0001 0010 01
//! out 1 40 00 0002 0010 01 -> !stall
//! bout 1 02 0102 -> 2
//! halt 1 81
//! reset 1
//! bulk 1 81 after=1000us 00ff00ff
//! bulk 1 81 rep=4 cut:16384:1c291ca3:0001
//! bulk 1 81 !unplug
//! unplug 1
//! plug 2 0bda:2838 bus="1" ports=2.1
//! ```
//!
//! - `sdrkit-usb 1` is the first line: the format and its version.
//! - `mode strict` or `mode lenient` (default strict). Strict: each call must be the next step. Lenient: a call takes
//!   the first unused step with the same fields after the last one taken, or repeats the reply of the last step that
//!   matched; an output call that matches no step succeeds; an input call that matches no step fails.
//! - `device <id> <vid>:<pid> bus=".." ports=a.b.c [manufacturer=".."] [product=".."] [serial=".."]
//!   [blocked=<reason>] [release=<bcd>] [speed=<speed>]` is a device attached at the start. `<reason>` is a key of
//!   [`Unavailable::key`], `<bcd>` the `bcdDevice` in four hexadecimal digits (0 when absent), `<speed>` a key of
//!   [`Speed::key`] (unknown when absent).
//! - `open <dev>`, `claim <dev> <interface>`, `halt <dev> <ep>`, `reset <dev>`: the call and, after `->`, its error.
//!   Without `->` the call succeeds.
//! - `in <dev> <type> <request> <value> <index> <length> -> <data>|<err>`: a control IN transfer and its reply.
//! - `out <dev> <type> <request> <value> <index> <data> [-> <err>]`: a control OUT transfer.
//! - `bout <dev> <ep> <data> [-> <count>|<err>]`: a bulk OUT transfer and the bytes the device took (the whole
//!   payload when `->` is absent). It is the step of [`UsbDevice::bulk_out`] and also of each
//!   [`BulkOut::submit`] of a queued writer; there the reply is what [`BulkOut::completed`] returns for it.
//! - `bulk <dev> <ep> [after=<n>us] [rep=<n>] <data>|<err>|!unplug`: bulk IN data that endpoint `<ep>` of `<dev>`
//!   gives in this order, apart from the steps. `after` is the time since the previous bulk item of the endpoint;
//!   `rep` gives the item that many times. One item fills at most one transfer; a longer item fills several.
//!   `!unplug` unplugs the device when the stream reaches it. A cut payload replays as its head and then zero bytes.
//! - `unplug <dev>` and `plug <id> ...` (the fields of `device`) take effect when the steps reach them.

mod error;
mod none;
#[cfg(all(
    feature = "nusb",
    any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "android"
    )
))]
mod nusb_bus;
mod record;
mod script;
mod text;

#[cfg(feature = "fake")]
pub mod fake;

pub use error::{Unavailable, UsbError};
pub use none::NoUsb;
#[cfg(all(
    feature = "nusb",
    any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "android"
    )
))]
pub use nusb_bus::NusbBus;
pub use record::{RecordOptions, RecordingUsb};
pub use script::{BulkItem, BulkKind, Entry, Mode, Payload, Script, Step};
pub use text::ParseError;

#[cfg(feature = "fake")]
pub use fake::{FakeUsb, Mismatch};

use std::sync::Arc;
use std::time::Duration;

/// The id of an attached device on one bus. It stays the same while the device stays attached; a device that is
/// plugged in again gets a new id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UsbId(pub u64);

/// An attached device as the bus lists it. Listing does no I/O on the device: the strings are those the system
/// keeps, and some systems keep none (Windows keeps no manufacturer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbDeviceInfo {
    /// The id to open it with.
    pub id: UsbId,
    /// The `idVendor` of its device descriptor.
    pub vendor_id: u16,
    /// The `idProduct` of its device descriptor.
    pub product_id: u16,
    /// The manufacturer string.
    pub manufacturer: Option<String>,
    /// The product string.
    pub product: Option<String>,
    /// The serial number string.
    pub serial: Option<String>,
    /// The bus or host controller, in the form of the system.
    pub bus: String,
    /// The hub ports from the root to the device.
    pub ports: Vec<u8>,
    /// A reason known at listing time why an open will fail (Windows: the device is not bound to WinUSB).
    pub blocked: Option<Unavailable>,
    /// The `bcdDevice` of its device descriptor (the release number); 0 where the system does not say (Android).
    pub release: u16,
    /// The speed of its link.
    pub speed: Speed,
}

impl UsbDeviceInfo {
    /// A device with no strings, no bus position, release 0, an unknown speed and nothing blocking it.
    pub fn new(id: UsbId, vendor_id: u16, product_id: u16) -> Self {
        UsbDeviceInfo {
            id,
            vendor_id,
            product_id,
            manufacturer: None,
            product: None,
            serial: None,
            bus: String::new(),
            ports: Vec::new(),
            blocked: None,
            release: 0,
            speed: Speed::Unknown,
        }
    }

    /// The bus position as `<bus>-<port>.<port>..`, the form of the `@` selector of a USB address.
    pub fn path(&self) -> String {
        let ports: Vec<String> = self.ports.iter().map(u8::to_string).collect();
        format!("{}-{}", self.bus, ports.join("."))
    }
}

/// The speed of the link of a device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Speed {
    /// The system does not say.
    #[default]
    Unknown,
    /// 1.5 Mbit/s.
    Low,
    /// 12 Mbit/s.
    Full,
    /// 480 Mbit/s.
    High,
    /// 5 Gbit/s.
    Super,
    /// 10 Gbit/s or more.
    SuperPlus,
}

impl Speed {
    /// Every speed, in declaration order.
    pub const ALL: [Speed; 6] = [
        Speed::Unknown,
        Speed::Low,
        Speed::Full,
        Speed::High,
        Speed::Super,
        Speed::SuperPlus,
    ];

    /// The fixed text key of the speed.
    pub fn key(self) -> &'static str {
        match self {
            Speed::Unknown => "unknown",
            Speed::Low => "low",
            Speed::Full => "full",
            Speed::High => "high",
            Speed::Super => "super",
            Speed::SuperPlus => "super_plus",
        }
    }

    /// The speed for the text key `key`.
    pub fn from_key(key: &str) -> Option<Speed> {
        Speed::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// A change in the set of attached devices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsbEvent {
    /// A device was attached.
    Arrived(UsbDeviceInfo),
    /// The device with this id left.
    Left(UsbId),
}

/// Whether the bus can reach USB devices at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Devices can be listed and opened, subject to the reasons of each device.
    Available,
    /// No device can be opened on this bus, for this reason.
    Unavailable(Unavailable),
}

impl Availability {
    /// The state as a sentence for a person.
    pub fn text(self) -> &'static str {
        match self {
            Availability::Available => "USB devices can be opened",
            Availability::Unavailable(u) => u.text(),
        }
    }
}

/// The setup fields of a control transfer apart from the length. The direction bit (0x80) of `request_type` is set
/// by [`UsbDevice::control_in`] and cleared by [`UsbDevice::control_out`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Setup {
    /// `bmRequestType`: type in bits 5 and 6, recipient in bits 0 to 4.
    pub request_type: u8,
    /// `bRequest`.
    pub request: u8,
    /// `wValue`.
    pub value: u16,
    /// `wIndex`.
    pub index: u16,
}

impl Setup {
    /// A vendor request to the device.
    pub const fn vendor(request: u8, value: u16, index: u16) -> Setup {
        Setup {
            request_type: 0x40,
            request,
            value,
            index,
        }
    }

    /// The same fields with the direction bit set for IN.
    pub const fn input(self) -> Setup {
        Setup {
            request_type: self.request_type | 0x80,
            ..self
        }
    }

    /// The same fields with the direction bit cleared for OUT.
    pub const fn output(self) -> Setup {
        Setup {
            request_type: self.request_type & 0x7f,
            ..self
        }
    }
}

/// A source of USB devices.
pub trait UsbBus: Send + Sync {
    /// Whether this bus can open devices at all.
    fn availability(&self) -> Availability;

    /// The devices attached now.
    ///
    /// # Errors
    ///
    /// [`UsbError::NoUsbHere`] on a bus without USB, else what the system returns.
    fn devices(&self) -> Result<Vec<UsbDeviceInfo>, UsbError>;

    /// Starts to watch for arrivals and removals. Events from this moment on are delivered.
    ///
    /// # Errors
    ///
    /// When the system cannot watch the bus.
    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError>;

    /// Opens the device with id `id`. The device is not claimed; see [`UsbDevice::claim`].
    ///
    /// # Errors
    ///
    /// [`UsbError::NoDevice`], [`UsbError::NoPermission`], [`UsbError::NeedsWinUsb`], [`UsbError::Busy`],
    /// [`UsbError::NoUsbHere`].
    fn open(&self, id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError>;
}

/// The arrivals and removals on a bus since [`UsbBus::watch`].
pub trait UsbWatch: Send {
    /// The next event, waiting up to `timeout`; `None` if none came in that time.
    ///
    /// # Errors
    ///
    /// When the watch stopped.
    fn next(&mut self, timeout: Duration) -> Result<Option<UsbEvent>, UsbError>;
}

/// An open device.
pub trait UsbDevice: Send {
    /// The device as it was listed.
    fn info(&self) -> &UsbDeviceInfo;

    /// Claims interface `interface` for this program. Where the system has kernel drivers that can be detached
    /// (Linux), a driver that holds the interface is detached first. Control transfers on Windows need a claimed
    /// interface.
    ///
    /// # Errors
    ///
    /// [`UsbError::KernelDriver`] if a driver of the system holds it, [`UsbError::Busy`] if another program does,
    /// [`UsbError::NeedsWinUsb`], [`UsbError::Unplugged`].
    fn claim(&mut self, interface: u8) -> Result<(), UsbError>;

    /// A control IN transfer of `buf.len()` bytes at most; returns the count the device sent.
    ///
    /// # Errors
    ///
    /// [`UsbError::Timeout`], [`UsbError::Stall`] (the device refused the request), [`UsbError::Unplugged`].
    fn control_in(
        &mut self,
        setup: Setup,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError>;

    /// A control OUT transfer of `data`.
    ///
    /// # Errors
    ///
    /// As [`UsbDevice::control_in`].
    fn control_out(&mut self, setup: Setup, data: &[u8], timeout: Duration)
        -> Result<(), UsbError>;

    /// Starts a bulk IN stream on endpoint `endpoint` (address with bit 0x80) of a claimed interface, with
    /// `queued` transfers of `transfer_size` bytes kept in flight. The size is rounded up to a whole number of
    /// packets. The endpoint stays with the stream until it is dropped.
    ///
    /// # Errors
    ///
    /// When no claimed interface has the endpoint, or the endpoint is in use.
    fn bulk_in(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkIn>, UsbError>;

    /// Writes `data` to bulk OUT endpoint `endpoint` of a claimed interface; returns the count the device took.
    ///
    /// # Errors
    ///
    /// [`UsbError::Timeout`], [`UsbError::Stall`], [`UsbError::Unplugged`].
    fn bulk_out(&mut self, endpoint: u8, data: &[u8], timeout: Duration)
        -> Result<usize, UsbError>;

    /// Starts a queued bulk OUT writer on endpoint `endpoint` of a claimed interface, with `queued` buffers of
    /// `transfer_size` bytes (rounded up to whole packets) that can be in flight together. The endpoint stays with
    /// the writer until it is dropped. A device that has no such writer returns [`UsbError::Other`].
    ///
    /// # Errors
    ///
    /// When no claimed interface has the endpoint, or the endpoint is in use.
    fn bulk_out_queue(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkOut>, UsbError> {
        let _ = (endpoint, transfer_size, queued);
        Err(UsbError::Other(
            "this device has no queued bulk OUT writer".into(),
        ))
    }

    /// Clears the halt of endpoint `endpoint` when no stream holds it. A stream clears its own endpoint with
    /// [`BulkIn::clear_halt`].
    ///
    /// # Errors
    ///
    /// [`UsbError::Busy`] if a stream holds the endpoint, else what the device returns.
    fn clear_halt(&mut self, endpoint: u8) -> Result<(), UsbError>;

    /// Resets the device. The device may then show itself as a new device that must be opened again.
    ///
    /// # Errors
    ///
    /// When the system cannot reset (WinUSB), or the device left.
    fn reset(&mut self) -> Result<(), UsbError>;
}

/// A bulk IN stream with several transfers in flight.
pub trait BulkIn: Send {
    /// The endpoint address.
    fn endpoint(&self) -> u8;

    /// The size of one transfer in bytes, after rounding to whole packets.
    fn transfer_size(&self) -> usize;

    /// The data of the next completed transfer, waiting up to `timeout`. The slice is the transfer buffer itself
    /// and stays valid until the next call, which gives the buffer back to the queue. A timeout leaves the
    /// transfers queued, so no data is lost by a short timeout.
    ///
    /// # Errors
    ///
    /// [`UsbError::Timeout`], [`UsbError::Stall`] (clear it with [`BulkIn::clear_halt`]), [`UsbError::Unplugged`].
    fn next(&mut self, timeout: Duration) -> Result<&[u8], UsbError>;

    /// Stops the transfers in flight, clears the halt of the endpoint and queues the transfers again. Data in
    /// flight is lost.
    ///
    /// # Errors
    ///
    /// What the device returns.
    fn clear_halt(&mut self) -> Result<(), UsbError>;
}

/// A bulk OUT writer with a pool of transfer buffers, several of them in flight.
///
/// The caller takes a free buffer with [`BulkOut::buffer`], fills it in place and sends it with
/// [`BulkOut::submit`]. [`BulkOut::completed`] gives the result of each transfer in the order of submission and
/// returns its buffer to the pool. When every buffer is in flight, `buffer` returns `None` until a result is taken.
pub trait BulkOut: Send {
    /// The endpoint address.
    fn endpoint(&self) -> u8;

    /// The size of each buffer in bytes, after rounding to whole packets.
    fn transfer_size(&self) -> usize;

    /// The count of transfers submitted whose result has not been taken.
    fn in_flight(&self) -> usize;

    /// A free buffer of [`BulkOut::transfer_size`] bytes to fill, or `None` when all are in flight. Until
    /// [`BulkOut::submit`], further calls return the same buffer with its contents.
    fn buffer(&mut self) -> Option<&mut [u8]>;

    /// Sends the first `len` bytes of the buffer of [`BulkOut::buffer`]. A transfer shorter than the buffer costs
    /// one copy inside the writer; a full one costs none.
    ///
    /// # Errors
    ///
    /// When no buffer was taken, `len` is larger than the buffer, or the device left. The outcome of the transfer
    /// itself comes from [`BulkOut::completed`].
    fn submit(&mut self, len: usize) -> Result<(), UsbError>;

    /// The result of the oldest transfer in flight, waiting up to `timeout`: the count of bytes the device took,
    /// or its error. `None` when no transfer is in flight. `Some(Err(UsbError::Timeout))` when it did not finish in
    /// time; it stays in flight.
    fn completed(&mut self, timeout: Duration) -> Option<Result<usize, UsbError>>;

    /// Waits up to `timeout` for every transfer in flight and returns the bytes they sent together.
    ///
    /// # Errors
    ///
    /// The first error of a transfer (the later ones are still waited for), or [`UsbError::Timeout`] if transfers
    /// are still in flight at the end of `timeout`.
    fn flush(&mut self, timeout: Duration) -> Result<u64, UsbError> {
        let deadline = std::time::Instant::now() + timeout;
        let mut sent = 0u64;
        let mut first = None;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.completed(left) {
                None => break,
                Some(Ok(n)) => sent += n as u64,
                Some(Err(UsbError::Timeout)) => return Err(first.unwrap_or(UsbError::Timeout)),
                Some(Err(e)) => {
                    first.get_or_insert(e);
                }
            }
        }
        match first {
            Some(e) => Err(e),
            None => Ok(sent),
        }
    }

    /// Stops every transfer in flight and returns their buffers to the pool; their results are dropped. A
    /// cancelled transfer may have sent part of its bytes.
    fn cancel(&mut self);
}

/// The USB bus of this system: the `nusb` backend where the platform and the build have it, else [`NoUsb`].
pub fn platform_bus() -> Arc<dyn UsbBus> {
    #[cfg(all(
        feature = "nusb",
        any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "android"
        )
    ))]
    {
        Arc::new(NusbBus::new())
    }
    #[cfg(not(all(
        feature = "nusb",
        any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "android"
        )
    )))]
    {
        Arc::new(NoUsb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_joins_bus_and_ports() {
        let mut d = UsbDeviceInfo::new(UsbId(1), 0x1234, 0x5678);
        d.bus = "2".into();
        d.ports = vec![1, 4];
        assert_eq!(d.path(), "2-1.4");
    }

    #[test]
    fn setup_direction() {
        let s = Setup::vendor(1, 2, 3);
        assert_eq!(s.input().request_type, 0xc0);
        assert_eq!(s.input().output().request_type, 0x40);
    }
}
