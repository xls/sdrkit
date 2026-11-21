//! Every radio driver of sdrkit in one list: the drivers of `sdrkit-driver-core` (test signal, file, rtl_tcp, iiod) and
//! the network driver crates. A host that needs fewer builds its own list.

#![warn(missing_docs)]

use sdrkit_core::{Clock, Storage, Transport};
use sdrkit_driver_core::synth::GeneratorFactory;
use sdrkit_driver_core::SourceDriver;
use std::sync::Arc;

/// Every driver: the drivers of the core first, then the network driver crates. Keys are unique. The test signal plays
/// what `generator` makes, files come from `storage`, network radios are reached through `transport`, and `clock` returns
/// the time to the drivers that stamp what they receive.
pub fn all(
    generator: Arc<dyn GeneratorFactory>,
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
    clock: Clock,
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
    v
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
        ] {
            assert!(keys.contains(&key), "{key} missing from {keys:?}");
        }
        assert!(keys.len() >= 4 + 9, "{keys:?}");
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
            other => panic!("no example for the part {other}"),
        }
    }

    /// The address that the `address` command of each kind returns, in the order of [`all`]. Every required part has its
    /// [`example`]. The other parts have theirs if `all_parts` is set, else their defaults. Each address must be one
    /// that the driver reads: the driver opens it, or fails for another cause than the address. The HPSDR kinds are
    /// not opened, because they wait some seconds for a radio to answer. Their crate checks their addresses.
    fn addresses(all_parts: bool) -> Vec<String> {
        let list = all(
            Arc::new(|_: &str, _: &Values, _| None),
            Arc::new(MemStorage::new()),
            Arc::new(FakeTransport::new()),
            Clock::new(|| 0.0),
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
        ] {
            assert!(
                built.iter().any(|a| a == expected),
                "{expected} not in {built:?}"
            );
        }
    }
}
