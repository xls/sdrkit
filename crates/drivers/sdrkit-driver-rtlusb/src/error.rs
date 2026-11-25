//! The errors of the RTL2832U driver.

use sdrkit_driver_core::DriverError;
use sdrkit_driver_usb::{Unavailable, UsbError};

/// A failure of the RTL2832U driver.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RtlError {
    /// A USB transfer failed.
    #[error("{0}")]
    Usb(#[from] UsbError),
    /// An I2C read returned fewer bytes than asked for.
    #[error("the I2C read of device {addr:#04x} returned {got} of {want} bytes")]
    ShortRead {
        /// The 8-bit I2C address.
        addr: u8,
        /// The bytes asked for.
        want: usize,
        /// The bytes returned.
        got: usize,
    },
    /// The dongle has a tuner that this driver does not drive.
    #[error("tuner {0} not supported yet")]
    TunerNotSupported(&'static str),
    /// No tuner answered the probe.
    #[error("no tuner answered on the I2C bus of the dongle")]
    NoTuner,
    /// The tuner has no PLL setting for this local oscillator frequency.
    #[error("the tuner cannot tune its oscillator to {hz} Hz")]
    NoPllSetting {
        /// The local oscillator frequency.
        hz: u64,
    },
    /// The PLL of the tuner did not lock.
    #[error("the PLL of the tuner did not lock at {hz} Hz")]
    PllNotLocked {
        /// The local oscillator frequency.
        hz: u64,
    },
    /// The sample rate is not one that the RTL2832U makes.
    #[error(
        "{0} Hz is not a sample rate of the RTL2832U (225 001 to 300 000 or 900 001 to 3 200 000)"
    )]
    Rate(u32),
    /// The tuner has no setting for this gain.
    #[error("the tuner has no gain of {0} tenth dB")]
    Gain(i32),
    /// The operation needs another tuner or another mode.
    #[error("{0}")]
    NotHere(&'static str),
    /// No attached dongle matches the address.
    #[error("no RTL2832U dongle matches the address")]
    NotFound,
}

impl RtlError {
    /// The reason to report to the host when this error stops a dongle from opening, or `None` when the error is not
    /// one of the lasting reasons of [`Unavailable`].
    pub fn unavailable(&self) -> Option<Unavailable> {
        match self {
            RtlError::Usb(e) => e.unavailable(),
            RtlError::NotFound => Some(Unavailable::NotFound),
            _ => None,
        }
    }

    /// True if the dongle was unplugged.
    pub fn unplugged(&self) -> bool {
        matches!(self, RtlError::Usb(UsbError::Unplugged))
    }
}

impl From<RtlError> for DriverError {
    /// A busy device is [`DriverError::Busy`], so an open is tried again; a device that cannot be used here is
    /// [`DriverError::Unavailable`] with its reason; every other error keeps its type in [`DriverError::Driver`].
    fn from(e: RtlError) -> DriverError {
        match e {
            RtlError::Usb(UsbError::Busy) => DriverError::Busy,
            e => e
                .unavailable()
                .and_then(|u| sdrkit_driver_core::Unavailable::error_of_key(u.key()))
                .unwrap_or_else(|| DriverError::driver(e)),
        }
    }
}
