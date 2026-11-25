//! The bus of a platform without USB access for programs (iOS, the web), or of a build without a USB backend.

use crate::{Availability, Unavailable, UsbBus, UsbDevice, UsbError, UsbId, UsbWatch};

/// A bus with no devices. Every open fails with [`UsbError::NoUsbHere`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoUsb;

impl UsbBus for NoUsb {
    fn availability(&self) -> Availability {
        Availability::Unavailable(Unavailable::NoUsbHere)
    }

    fn devices(&self) -> Result<Vec<crate::UsbDeviceInfo>, UsbError> {
        Err(UsbError::NoUsbHere)
    }

    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        Err(UsbError::NoUsbHere)
    }

    fn open(&self, _id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError> {
        Err(UsbError::NoUsbHere)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_call_says_no_usb() {
        let b = NoUsb;
        assert_eq!(
            b.availability(),
            Availability::Unavailable(Unavailable::NoUsbHere)
        );
        assert_eq!(b.availability().text(), "no USB access on this platform");
        assert_eq!(b.devices(), Err(UsbError::NoUsbHere));
        assert!(matches!(b.open(UsbId(1)), Err(UsbError::NoUsbHere)));
        assert!(b.watch().is_err());
    }
}
