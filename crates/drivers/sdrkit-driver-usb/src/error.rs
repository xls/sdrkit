//! The errors of USB access, and the reasons why a device cannot be used at all.

/// Why a USB device cannot be used on this system, as a driver reports it to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unavailable {
    /// The platform gives programs no USB access (iPhone, a web page) or the library was built without it.
    NoUsbHere,
    /// The user may not open the device (Linux: no udev rule; Android: no permission from the system).
    NoPermission,
    /// A driver of the operating system holds the device and cannot be detached.
    KernelDriver,
    /// Windows: the device is not bound to the WinUSB driver.
    NeedsWinUsb,
    /// The device is not attached.
    NotFound,
}

impl Unavailable {
    /// Every reason, in declaration order.
    pub const ALL: [Unavailable; 5] = [
        Unavailable::NoUsbHere,
        Unavailable::NoPermission,
        Unavailable::KernelDriver,
        Unavailable::NeedsWinUsb,
        Unavailable::NotFound,
    ];

    /// The fixed text key of the reason.
    pub fn key(self) -> &'static str {
        match self {
            Unavailable::NoUsbHere => "no_usb_here",
            Unavailable::NoPermission => "no_permission",
            Unavailable::KernelDriver => "kernel_driver",
            Unavailable::NeedsWinUsb => "needs_winusb",
            Unavailable::NotFound => "not_found",
        }
    }

    /// The reason for the text key `key`.
    pub fn from_key(key: &str) -> Option<Unavailable> {
        Unavailable::ALL.into_iter().find(|u| u.key() == key)
    }

    /// The reason as a sentence for a person.
    pub fn text(self) -> &'static str {
        match self {
            Unavailable::NoUsbHere => "no USB access on this platform",
            Unavailable::NoPermission => "no permission to open the USB device",
            Unavailable::KernelDriver => "a driver of the system holds the USB device",
            Unavailable::NeedsWinUsb => "the USB device needs the WinUSB driver",
            Unavailable::NotFound => "the USB device is not attached",
        }
    }
}

/// The failure of a USB call.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UsbError {
    /// No device has this id, or it left before the call.
    #[error("no such USB device")]
    NoDevice,
    /// The user may not open the device.
    #[error("no permission to open the USB device")]
    NoPermission,
    /// A driver of the operating system holds the interface and could not be detached.
    #[error("a driver of the system holds the USB device")]
    KernelDriver,
    /// Windows: the device or interface is not bound to WinUSB.
    #[error("the USB device needs the WinUSB driver")]
    NeedsWinUsb,
    /// The platform gives no USB access.
    #[error("no USB access on this platform")]
    NoUsbHere,
    /// Another program or handle holds the device, interface or endpoint.
    #[error("another program uses the USB device")]
    Busy,
    /// The device was unplugged while open.
    #[error("the USB device was unplugged")]
    Unplugged,
    /// The transfer did not complete in time.
    #[error("the USB transfer timed out")]
    Timeout,
    /// The device stalled the endpoint.
    #[error("the USB endpoint stalled")]
    Stall,
    /// A call that does not follow the script of a fake bus. The text names the step and the call.
    #[error("script: {0}")]
    Script(String),
    /// Any other failure, with the text of the system.
    #[error("{0}")]
    Other(String),
}

impl UsbError {
    /// The reason to report to the host when this error stops a device from opening, or `None` when the error is
    /// not one of the lasting reasons of [`Unavailable`].
    pub fn unavailable(&self) -> Option<Unavailable> {
        match self {
            UsbError::NoDevice => Some(Unavailable::NotFound),
            UsbError::NoPermission => Some(Unavailable::NoPermission),
            UsbError::KernelDriver => Some(Unavailable::KernelDriver),
            UsbError::NeedsWinUsb => Some(Unavailable::NeedsWinUsb),
            UsbError::NoUsbHere => Some(Unavailable::NoUsbHere),
            _ => None,
        }
    }

    /// True if the same call can work later without a change by the user (a timeout, a busy device, a re-plug).
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            UsbError::Busy | UsbError::Unplugged | UsbError::Timeout | UsbError::NoDevice
        )
    }

    /// The text key of the error in a script (`timeout`, `stall`, `other`). [`UsbError::Script`] and
    /// [`UsbError::Other`] keep their text separately.
    pub fn key(&self) -> &'static str {
        match self {
            UsbError::NoDevice => "no_device",
            UsbError::NoPermission => "no_permission",
            UsbError::KernelDriver => "kernel_driver",
            UsbError::NeedsWinUsb => "needs_winusb",
            UsbError::NoUsbHere => "no_usb_here",
            UsbError::Busy => "busy",
            UsbError::Unplugged => "unplugged",
            UsbError::Timeout => "timeout",
            UsbError::Stall => "stall",
            UsbError::Script(_) => "script",
            UsbError::Other(_) => "other",
        }
    }

    /// The error for a text key and, for `script` and `other`, its text.
    pub fn from_key(key: &str, text: &str) -> Option<UsbError> {
        Some(match key {
            "no_device" => UsbError::NoDevice,
            "no_permission" => UsbError::NoPermission,
            "kernel_driver" => UsbError::KernelDriver,
            "needs_winusb" => UsbError::NeedsWinUsb,
            "no_usb_here" => UsbError::NoUsbHere,
            "busy" => UsbError::Busy,
            "unplugged" => UsbError::Unplugged,
            "timeout" => UsbError::Timeout,
            "stall" => UsbError::Stall,
            "script" => UsbError::Script(text.to_owned()),
            "other" => UsbError::Other(text.to_owned()),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for u in Unavailable::ALL {
            assert_eq!(Unavailable::from_key(u.key()), Some(u));
        }
        for e in [
            UsbError::NoDevice,
            UsbError::NoPermission,
            UsbError::KernelDriver,
            UsbError::NeedsWinUsb,
            UsbError::NoUsbHere,
            UsbError::Busy,
            UsbError::Unplugged,
            UsbError::Timeout,
            UsbError::Stall,
            UsbError::Other("x".into()),
        ] {
            assert_eq!(UsbError::from_key(e.key(), "x"), Some(e));
        }
    }

    #[test]
    fn lasting_reasons_map_to_unavailable() {
        assert_eq!(
            UsbError::NeedsWinUsb.unavailable(),
            Some(Unavailable::NeedsWinUsb)
        );
        assert_eq!(
            UsbError::NoDevice.unavailable(),
            Some(Unavailable::NotFound)
        );
        assert_eq!(UsbError::Timeout.unavailable(), None);
        assert!(UsbError::Timeout.retryable());
        assert!(!UsbError::NoPermission.retryable());
    }
}
