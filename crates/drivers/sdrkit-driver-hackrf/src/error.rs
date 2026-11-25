//! The errors of the HackRF driver.

use crate::proto::api_text;
use sdrkit_core::NetError;
use sdrkit_driver_core::DriverError;
use sdrkit_driver_usb::{Unavailable, UsbError};

/// A failure of the HackRF driver.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum HackrfError {
    /// A USB transfer failed.
    #[error("{0}")]
    Usb(#[from] UsbError),
    /// The firmware of the device is too old for the request; it was not sent.
    #[error(
        "request {request} is not supported by this firmware (USB API {}, it needs {})",
        api_text(*have),
        api_text(*need)
    )]
    NotSupported {
        /// The request number.
        request: u8,
        /// The lowest USB API version that has the request.
        need: u16,
        /// The USB API version of the device.
        have: u16,
    },
    /// The driver never sends this request (it writes firmware, flash, logic or raw registers, or it is out of scope).
    #[error("the driver never sends request {request} with value {value}")]
    Forbidden {
        /// The request number.
        request: u8,
        /// The value field.
        value: u16,
    },
    /// The request exists only on the HackRF Pro.
    #[error("request {request} exists only on the HackRF Pro")]
    ProOnly {
        /// The request number.
        request: u8,
    },
    /// The device replied 0 to a gain request: it did not take the value.
    #[error("the device rejected the value of request {request}")]
    Rejected {
        /// The request number.
        request: u8,
    },
    /// A reply was shorter than the request needs.
    #[error("request {request} gave {got} of {want} bytes")]
    Short {
        /// The request number (6 for the device descriptor).
        request: u8,
        /// The bytes needed.
        want: usize,
        /// The bytes received.
        got: usize,
    },
    /// Board id 0: the early prototype, which the driver does not drive.
    #[error("board id 0 (an early prototype) is not supported")]
    Prototype,
    /// The device is in the boot loader of its chip (firmware update mode).
    #[error("the HackRF is in DFU mode (firmware update) and cannot be used")]
    Dfu,
    /// No attached HackRF matches the address.
    #[error("no HackRF matches the address")]
    NotFound,
    /// The serial number in the address matches more than one device.
    #[error("the serial number in the address matches {0} HackRF devices")]
    Ambiguous(usize),
    /// No receive block arrived for the time of the watchdog.
    #[error("the device stopped streaming")]
    StoppedStreaming,
    /// The device stalled the transmit mode request: its transmitter is disabled.
    #[error("transmit is disabled on this device")]
    TransmitDisabled,
    /// The board has no antenna power (Jawbreaker, rad1o).
    #[error("this board has no antenna power")]
    NoAntennaPower,
    /// The frequency lies outside what the firmware takes (0 to 7250 MHz).
    #[error("{0} Hz is outside 0 to 7250 MHz")]
    Frequency(u64),
    /// The device cannot be used on this system for a lasting reason.
    #[error("{}", .0.text())]
    Unavailable(Unavailable),
}

impl HackrfError {
    /// The reason to report to the host when this error stops a device from opening, or `None` when the error is not
    /// one of the lasting reasons of [`Unavailable`].
    pub fn unavailable(&self) -> Option<Unavailable> {
        match self {
            HackrfError::Usb(e) => e.unavailable(),
            HackrfError::NotFound => Some(Unavailable::NotFound),
            HackrfError::Unavailable(u) => Some(*u),
            _ => None,
        }
    }

    /// True if the device left the bus.
    pub fn unplugged(&self) -> bool {
        matches!(
            self,
            HackrfError::Usb(UsbError::Unplugged | UsbError::NoDevice)
        )
    }
}

impl From<HackrfError> for DriverError {
    /// An unplug is a closed link and a timeout a timed-out link, so the runner opens the device again; a busy device is
    /// [`DriverError::Busy`]; every other error keeps its type in [`DriverError::Driver`].
    fn from(e: HackrfError) -> DriverError {
        match e {
            HackrfError::Usb(UsbError::Unplugged | UsbError::NoDevice) => {
                DriverError::Net(NetError::Closed)
            }
            HackrfError::Usb(UsbError::Timeout) | HackrfError::StoppedStreaming => {
                DriverError::Net(NetError::TimedOut)
            }
            HackrfError::Usb(UsbError::Busy) => DriverError::Busy,
            e => DriverError::driver(e),
        }
    }
}
