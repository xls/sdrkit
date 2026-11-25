//! Every radio driver of sdrkit in one list: the drivers of `sdrkit-driver-core` (test signal, file, rtl_tcp, iiod),
//! the network driver crates and the drivers of the USB radios. A host that needs fewer builds its own list.
//!
//! The USB radios have one more address form, `usb:<family>:<selector>` ([`USB_FAMILIES`]). [`usb_radio`] says which
//! driver takes an attached USB device.

#![warn(missing_docs)]

use sdrkit_core::{Clock, Storage, Transport};
use sdrkit_driver_core::synth::GeneratorFactory;
use sdrkit_driver_core::SourceDriver;
use sdrkit_driver_usb::{UsbBus, UsbDeviceInfo};
use std::sync::Arc;

/// Every driver: the drivers of the core first, then the network driver crates, then the USB radios. Keys are unique.
/// The test signal plays what `generator` makes, files come from `storage`, network radios are reached through
/// `transport`, `clock` returns the time to the drivers that stamp what they receive, and the USB radios are those of
/// `usb` (`sdrkit_driver_usb::platform_bus()` for the bus of the system, `NoUsb` for none).
pub fn all(
    generator: Arc<dyn GeneratorFactory>,
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
    clock: Clock,
    usb: Arc<dyn UsbBus>,
) -> Vec<Box<dyn SourceDriver>> {
    let t = &transport;
    let mut v = sdrkit_driver_core::drivers(generator, storage, t.clone());
    v.push(Box::new(sdrkit_driver_spyserver::SpyServerDriver::new(
        t.clone(),
    )));
    v.push(Box::new(sdrkit_driver_sdrpp::SdrppDriver::new(t.clone())));
    v.push(Box::new(sdrkit_driver_soapyremote::SoapyRemoteDriver::new(
        t.clone(),
    )));
    v.push(Box::new(sdrkit_driver_kiwisdr::KiwiDriver::new(
        t.clone(),
        clock.clone(),
    )));
    v.push(Box::new(sdrkit_driver_openwebrx::OpenWebRxDriver::new(
        t.clone(),
        clock,
    )));
    v.push(Box::new(sdrkit_driver_rsptcp::RspTcpDriver::new(t.clone())));
    // One driver per HPSDR radio family, sharing what they learn.
    v.extend(sdrkit_driver_hpsdr::drivers(t.clone()));
    v.push(Box::new(sdrkit_driver_tci::TciDriver::new(t.clone())));
    v.push(Box::new(sdrkit_driver_sdrconnect::SdrconnectDriver::new(
        t.clone(),
    )));
    v.push(Box::new(sdrkit_driver_rtlusb::RtlUsbDriver::new(
        usb.clone(),
    )));
    v.push(Box::new(sdrkit_driver_hackrf::HackrfDriver::new(usb)));
    v
}

/// A family of USB radios: the word after `usb:` in an address, and the key of the driver that takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbFamily {
    /// The family word of the address `usb:<family>:<selector>`.
    pub family: &'static str,
    /// The key of the driver kind.
    pub driver: &'static str,
    /// The driver takes the last digits of a serial number as a selector.
    pub serial_suffix: bool,
}

/// The families of USB radios, in the order of [`all`].
pub const USB_FAMILIES: [UsbFamily; 2] = [
    UsbFamily {
        family: "rtl",
        driver: sdrkit_driver_rtlusb::KEY,
        serial_suffix: false,
    },
    UsbFamily {
        family: "hackrf",
        driver: "hackrf",
        serial_suffix: true,
    },
];

/// Splits the part of an address after `usb:` into the key of the driver and its selector: `rtl:0001` returns
/// (`rtlusb`, `0001`). `None` for a family that is not in [`USB_FAMILIES`].
pub fn split_usb(rest: &str) -> Option<(&'static str, &str)> {
    let (family, selector) = rest.split_once(':').unwrap_or((rest, ""));
    USB_FAMILIES
        .iter()
        .find(|f| f.family == family)
        .map(|f| (f.driver, selector))
}

/// The family of the driver kind `driver`, if it is a kind of USB radio.
pub fn usb_family(driver: &str) -> Option<&'static UsbFamily> {
    USB_FAMILIES.iter().find(|f| f.driver == driver)
}

/// An attached USB device that a driver of [`all`] takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbRadio {
    /// Its family.
    pub family: &'static UsbFamily,
    /// The address that opens it and finds it again after a re-plug: `usb:<family>:<serial>`, or
    /// `usb:<family>:@<bus>-<port path>` for a device without a serial number of its own.
    pub address: String,
    /// The selector alone, as the driver stores it in the address of an open radio.
    pub selector: String,
    /// A name for people: the product string of the device, or the name of the family.
    pub name: String,
}

/// The radio that the attached device `info` is, among the devices `all` of its bus. `None` for a device that no driver
/// of USB radios takes. The call reads only the listing; it opens no device.
pub fn usb_radio(info: &UsbDeviceInfo, all: &[UsbDeviceInfo]) -> Option<UsbRadio> {
    let rtl = sdrkit_driver_rtlusb::ids::is_rtl(info.vendor_id, info.product_id);
    let (family, address, fallback) = if rtl {
        let dongles: Vec<UsbDeviceInfo> = all
            .iter()
            .filter(|d| sdrkit_driver_rtlusb::ids::is_rtl(d.vendor_id, d.product_id))
            .cloned()
            .collect();
        (
            &USB_FAMILIES[0],
            sdrkit_driver_rtlusb::address_of(info, &dongles),
            "RTL-SDR dongle",
        )
    } else if sdrkit_driver_hackrf::is_hackrf(info) {
        (
            &USB_FAMILIES[1],
            sdrkit_driver_hackrf::Selector::pinned(info).address(),
            "HackRF",
        )
    } else {
        return None;
    };
    // Each driver writes its address with its own key before the selector.
    let selector = address
        .strip_prefix(family.driver)
        .and_then(|a| a.strip_prefix(':'))
        .unwrap_or(&address)
        .to_owned();
    let name = match (info.manufacturer.as_deref(), info.product.as_deref()) {
        (Some(m), Some(p)) if !m.is_empty() && !p.is_empty() => format!("{m} {p}"),
        (_, Some(p)) if !p.is_empty() => p.to_owned(),
        _ => fallback.to_owned(),
    };
    Some(UsbRadio {
        family,
        address: format!("usb:{}:{selector}", family.family),
        selector,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::net::FakeTransport;
    use sdrkit_core::{MemStorage, Owner, Value, Values};
    use sdrkit_driver_core::address::ADDRESS;
    use sdrkit_driver_core::{DriverError, DriverKind};

    #[test]
    fn every_driver_is_listed_once() {
        let list = all(
            Arc::new(|_: &str, _: &Values, _| None),
            Arc::new(MemStorage::new()),
            Arc::new(FakeTransport::new()),
            Clock::new(|| 0.0),
            Arc::new(sdrkit_driver_usb::NoUsb),
        );
        let keys: Vec<&str> = list.iter().map(|d| d.descriptor().key).collect();
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), keys.len(), "duplicate keys in {keys:?}");
        for key in [
            "synth",
            "file",
            "rtl_tcp",
            "iiod",
            "sdrconnect",
            "tci",
            "sdrpp",
            "spyserver",
            "rtlusb",
            "hackrf",
        ] {
            assert!(keys.contains(&key), "{key} missing from {keys:?}");
        }
        assert!(keys.len() >= 4 + 9 + 2, "{keys:?}");
        for d in &list {
            assert!(!d.descriptor().name.is_empty());
            let _ = d.descriptor().decl();
        }
    }

    /// A value for the part `key` of an address: for a required part, and for an optional part that is set.
    fn example(key: &str) -> Value {
        match key {
            "host" => Value::Text("192.0.2.1".into()),
            "port" => Value::I64(5555),
            "receiver" => Value::I64(2),
            "path" => Value::Text("x.wav".into()),
            "stack" => Value::Text("fm".into()),
            "submode" => Value::Text("wide".into()),
            "tls" => Value::Bool(true),
            "device" => Value::Text("0001".into()),
            other => panic!("no example for the part {other}"),
        }
    }

    /// The address that the `address` command of each kind returns, in the order of [`all`]. Every required part has its
    /// [`example`]. The other parts have theirs if `all_parts` is set, else their defaults. Each address must be one
    /// that the driver reads: the driver opens it, or fails for another cause than the address. The HPSDR kinds are
    /// not opened, because they wait some seconds for a radio to answer. Their crate checks their addresses. The USB
    /// kinds are on a bus without USB, so no device is opened.
    fn addresses(all_parts: bool) -> Vec<String> {
        let list = all(
            Arc::new(|_: &str, _: &Values, _| None),
            Arc::new(MemStorage::new()),
            Arc::new(FakeTransport::new()),
            Clock::new(|| 0.0),
            Arc::new(sdrkit_driver_usb::NoUsb),
        );
        let mut built = Vec::new();
        std::thread::scope(|scope| {
            for driver in list {
                let mut kind = DriverKind::new(driver);
                let key = kind.descriptor().key;
                let id = kind.commands().id("address").unwrap();
                let def = kind.commands().def(id).unwrap();
                let mut args = def.new_args();
                let mut receivers = false;
                for part in def.args.settings() {
                    receivers |= part.field.key == "receiver";
                    if part.required || all_parts {
                        let slot = args.decl().id(&part.field.key).unwrap();
                        args.set_value(slot, &example(&part.field.key)).unwrap();
                    }
                }
                let reply = sdrkit_core::owner::invoke(&mut kind, id, &args)
                    .unwrap()
                    .unwrap();
                let text = reply.get(&ADDRESS).unwrap().to_string();
                built.push(text.clone());
                if receivers {
                    continue;
                }
                scope.spawn(move || {
                    // A radio is opened with the text after the driver key.
                    let rest = text.strip_prefix(key).and_then(|r| r.strip_prefix(':'));
                    let rest = rest.unwrap_or_else(|| panic!("{text} does not start with {key}:"));
                    let opened = kind.open(rest, &Values::defaults(kind.schema()));
                    assert!(
                        !matches!(opened, Err(DriverError::Address)),
                        "{key} refuses {text}"
                    );
                });
            }
        });
        built
    }

    #[test]
    fn every_kind_builds_an_address_that_its_driver_reads() {
        assert_eq!(
            addresses(false),
            [
                "synth:fm",
                "file:x.wav",
                "rtl_tcp:192.0.2.1:1234",
                "iiod:192.0.2.1:30431",
                "spyserver:192.0.2.1:5555",
                "sdrpp:192.0.2.1:5259",
                "soapyremote:192.0.2.1:55132",
                "kiwisdr:192.0.2.1:8073",
                "openwebrx:192.0.2.1:8073",
                "rsp_tcp:192.0.2.1:1234",
                "hermeslite2:192.0.2.1:5555",
                "redpitaya:192.0.2.1:5555",
                "hpsdr1:192.0.2.1:5555",
                "hpsdr2:192.0.2.1:5555",
                "tci:192.0.2.1:50001",
                "sdrconnect:192.0.2.1:5454",
                "rtlusb:",
                "hackrf:",
            ]
        );
    }

    #[test]
    fn every_kind_reads_an_address_with_every_part_set() {
        let built = addresses(true);
        for expected in [
            "synth:fm/wide",
            "rtl_tcp:192.0.2.1:5555",
            "openwebrx:https://192.0.2.1:5555/x.wav",
            "hpsdr1:192.0.2.1:5555/rx2",
            "rtlusb:0001",
            "hackrf:0001",
        ] {
            assert!(
                built.iter().any(|a| a == expected),
                "{expected} not in {built:?}"
            );
        }
    }

    #[test]
    fn a_usb_kind_without_usb_says_why_it_is_unavailable() {
        let list = all(
            Arc::new(|_: &str, _: &Values, _| None),
            Arc::new(MemStorage::new()),
            Arc::new(FakeTransport::new()),
            Clock::new(|| 0.0),
            Arc::new(sdrkit_driver_usb::NoUsb),
        );
        let mut opened = 0;
        for driver in list {
            let key = driver.descriptor().key;
            if usb_family(key).is_none() {
                continue;
            }
            let kind = DriverKind::new(driver);
            let values = Values::defaults(kind.schema());
            let e = kind.open("", &values).err();
            // The driver of the dongles keeps the error of the bus with its own type; the reason is in its text.
            match e {
                Some(DriverError::Unavailable { why }) => {
                    assert_eq!(why, sdrkit_driver_core::Unavailable::NoUsbHere, "{key}");
                }
                Some(other) => {
                    assert_eq!(other.to_string(), "no USB access on this platform", "{key}")
                }
                None => panic!("{key} opened a radio on a bus without USB"),
            }
            opened += 1;
        }
        assert_eq!(opened, USB_FAMILIES.len());
    }

    #[test]
    fn a_usb_address_names_its_family_and_an_attached_device_its_driver() {
        assert_eq!(split_usb("rtl:0001"), Some(("rtlusb", "0001")));
        assert_eq!(split_usb("hackrf:@1-2.3"), Some(("hackrf", "@1-2.3")));
        assert_eq!(split_usb("rtl"), Some(("rtlusb", "")));
        assert_eq!(split_usb("other:1"), None);
        assert_eq!(usb_family("hackrf").map(|f| f.family), Some("hackrf"));
        assert_eq!(usb_family("rtl_tcp"), None);
        let bus = sdrkit_driver_usb::FakeUsb::from_text(
            "sdrkit-usb 1\n\
             device 1 0bda:2838 bus=\"1\" ports=3 manufacturer=\"Maker\" product=\"Dongle\" serial=\"0001\"\n\
             device 2 0bda:2838 bus=\"1\" ports=4.1 serial=\"0002\"\n\
             device 3 0bda:2838 bus=\"1\" ports=4.2 serial=\"0002\"\n\
             device 4 1d50:6089 bus=\"2\" ports=1 product=\"HackRF One\" serial=\"00000000000000000123ABCD\"\n\
             device 5 046d:c52b bus=\"2\" ports=2 product=\"Receiver\"\n",
        )
        .unwrap();
        let listed = sdrkit_driver_usb::UsbBus::devices(&bus).unwrap();
        let radios: Vec<_> = listed.iter().map(|d| usb_radio(d, &listed)).collect();
        let text: Vec<_> = radios
            .iter()
            .map(|r| r.as_ref().map(|r| (r.address.as_str(), r.name.as_str())))
            .collect();
        assert_eq!(
            text,
            [
                Some(("usb:rtl:0001", "Maker Dongle")),
                // Two dongles with one serial number are named by their ports.
                Some(("usb:rtl:@1-4.1", "RTL-SDR dongle")),
                Some(("usb:rtl:@1-4.2", "RTL-SDR dongle")),
                Some(("usb:hackrf:00000000000000000123abcd", "HackRF One")),
                None,
            ]
        );
        // The address of a listed device splits into its driver and the selector that the driver reads.
        let r = radios[0].as_ref().unwrap();
        let rest = r.address.strip_prefix("usb:").unwrap();
        assert_eq!(
            split_usb(rest),
            Some((r.family.driver, r.selector.as_str()))
        );
        // Listing opened nothing.
        bus.check().unwrap();
    }
}
