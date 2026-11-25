//! The address of a HackRF and the devices it selects: empty (the first free one), a serial number, `#<n>` (the n-th
//! HackRF of the bus listing, from 0) or `@<bus>-<port path>`.

use crate::error::HackrfError;
use crate::proto;
use sdrkit_driver_core::DriverError;
use sdrkit_driver_usb::UsbDeviceInfo;

/// Which device an address selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// The first HackRF that opens.
    First,
    /// The HackRF whose serial number is this or ends with it (lower case hexadecimal).
    Serial(String),
    /// The n-th HackRF of the bus listing, from 0.
    Index(usize),
    /// The HackRF at this bus position (`<bus>-<port>.<port>..`).
    Path(String),
}

impl Selector {
    /// Reads `address`, with or without the schemes `usb:` and `hackrf:`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] for a text that is none of the forms.
    pub fn parse(address: &str) -> Result<Selector, DriverError> {
        let mut a = address.trim();
        for scheme in ["usb:", "hackrf:"] {
            if let Some(rest) = a.strip_prefix(scheme) {
                a = rest.strip_prefix("//").unwrap_or(rest);
            }
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
            return if p.contains('-') && !p.contains(char::is_whitespace) {
                Ok(Selector::Path(p.to_string()))
            } else {
                Err(DriverError::Address)
            };
        }
        if a.len() <= 32 && a.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(Selector::Serial(a.to_ascii_lowercase()));
        }
        Err(DriverError::Address)
    }

    /// The address text of the selector, with the scheme `hackrf:`.
    pub fn address(&self) -> String {
        match self {
            Selector::First => "hackrf:".into(),
            Selector::Serial(s) => format!("hackrf:{s}"),
            Selector::Index(n) => format!("hackrf:#{n}"),
            Selector::Path(p) => format!("hackrf:@{p}"),
        }
    }

    /// The selector that finds `info` again after a re-plug: its serial number, else its bus position.
    pub fn pinned(info: &UsbDeviceInfo) -> Selector {
        match info.serial.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => Selector::Serial(s.to_ascii_lowercase()),
            None => Selector::Path(info.path()),
        }
    }
}

/// True for a HackRF board in its own firmware (not in DFU mode).
pub fn is_hackrf(info: &UsbDeviceInfo) -> bool {
    info.vendor_id == proto::VENDOR_ID
        && matches!(
            info.product_id,
            proto::PRODUCT_ONE | proto::PRODUCT_JAWBREAKER | proto::PRODUCT_RAD1O
        )
}

/// True for a device in the boot loader of its chip (DFU mode).
pub fn is_dfu(info: &UsbDeviceInfo) -> bool {
    info.vendor_id == proto::DFU_VENDOR_ID && info.product_id == proto::DFU_PRODUCT_ID
}

/// The devices of `list` that `sel` selects, in the order to try them.
///
/// # Errors
///
/// [`HackrfError::NotFound`] if none matches ([`HackrfError::Dfu`] if the only candidates are in DFU mode),
/// [`HackrfError::Ambiguous`] for a serial number that matches several.
pub fn find(list: &[UsbDeviceInfo], sel: &Selector) -> Result<Vec<UsbDeviceInfo>, HackrfError> {
    let boards: Vec<&UsbDeviceInfo> = list.iter().filter(|d| is_hackrf(d)).collect();
    let found: Vec<UsbDeviceInfo> = match sel {
        Selector::First => boards.into_iter().cloned().collect(),
        Selector::Index(n) => boards.get(*n).map(|d| (*d).clone()).into_iter().collect(),
        Selector::Path(p) => boards
            .into_iter()
            .filter(|d| d.path() == *p)
            .cloned()
            .collect(),
        Selector::Serial(s) => {
            let m: Vec<UsbDeviceInfo> = boards
                .into_iter()
                .filter(|d| {
                    d.serial
                        .as_deref()
                        .is_some_and(|x| x.to_ascii_lowercase().ends_with(s.as_str()))
                })
                .cloned()
                .collect();
            if m.len() > 1 {
                let exact: Vec<UsbDeviceInfo> = m
                    .iter()
                    .filter(|d| {
                        d.serial
                            .as_deref()
                            .is_some_and(|x| x.eq_ignore_ascii_case(s))
                    })
                    .cloned()
                    .collect();
                if exact.len() != 1 {
                    return Err(HackrfError::Ambiguous(m.len()));
                }
                exact
            } else {
                m
            }
        }
    };
    if found.is_empty() {
        let dfu_only = matches!(sel, Selector::First) && list.iter().any(is_dfu);
        return Err(if dfu_only {
            HackrfError::Dfu
        } else {
            HackrfError::NotFound
        });
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_driver_usb::UsbId;

    fn dev(id: u64, pid: u16, serial: &str, ports: &[u8]) -> UsbDeviceInfo {
        let mut d = UsbDeviceInfo::new(UsbId(id), proto::VENDOR_ID, pid);
        d.serial = Some(serial.into());
        d.bus = "1".into();
        d.ports = ports.to_vec();
        d
    }

    #[test]
    fn addresses_parse_in_every_form() {
        assert_eq!(Selector::parse(""), Ok(Selector::First));
        assert_eq!(Selector::parse("usb:hackrf:"), Ok(Selector::First));
        assert_eq!(Selector::parse("hackrf:#1"), Ok(Selector::Index(1)));
        assert_eq!(
            Selector::parse("@2-1.4"),
            Ok(Selector::Path("2-1.4".into()))
        );
        assert_eq!(
            Selector::parse("hackrf:00000000A06063C8"),
            Ok(Selector::Serial("00000000a06063c8".into()))
        );
        assert_eq!(Selector::parse("#x"), Err(DriverError::Address));
        assert_eq!(Selector::parse("not a serial"), Err(DriverError::Address));
        assert_eq!(Selector::Index(2).address(), "hackrf:#2");
    }

    #[test]
    fn selectors_find_their_devices() {
        let list = vec![
            UsbDeviceInfo::new(UsbId(9), 0x0bda, 0x2838),
            dev(
                1,
                proto::PRODUCT_ONE,
                "0000000000000000a06063c8234e925f",
                &[2],
            ),
            dev(
                2,
                proto::PRODUCT_ONE,
                "0000000000000000a06063c8234e1234",
                &[3, 1],
            ),
        ];
        let ids =
            |s: &Selector| find(&list, s).map(|v| v.iter().map(|d| d.id.0).collect::<Vec<_>>());
        assert_eq!(ids(&Selector::First), Ok(vec![1, 2]));
        assert_eq!(ids(&Selector::Index(1)), Ok(vec![2]));
        assert_eq!(ids(&Selector::Index(2)), Err(HackrfError::NotFound));
        assert_eq!(ids(&Selector::Path("1-3.1".into())), Ok(vec![2]));
        assert_eq!(ids(&Selector::Serial("925f".into())), Ok(vec![1]));
        assert_eq!(
            ids(&Selector::Serial("a06063c8234e".into())),
            Err(HackrfError::NotFound)
        );
        assert_eq!(ids(&Selector::Serial("4e925f".into())), Ok(vec![1]));
        let both = vec![list[1].clone(), list[1].clone()];
        assert_eq!(
            find(&both, &Selector::Serial("925f".into())).err(),
            Some(HackrfError::Ambiguous(2))
        );
        let dfu = vec![UsbDeviceInfo::new(
            UsbId(4),
            proto::DFU_VENDOR_ID,
            proto::DFU_PRODUCT_ID,
        )];
        assert_eq!(find(&dfu, &Selector::First).err(), Some(HackrfError::Dfu));
        assert_eq!(
            Selector::pinned(&list[1]),
            Selector::Serial("0000000000000000a06063c8234e925f".into())
        );
    }
}
