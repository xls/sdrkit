//! The kind of radio on a scripted bus: its declarations, the `discover` command with the `devices` table, the
//! address forms, and the refusal of a tuner that the driver does not drive.

use sdrkit_core::{CommandSet, Owner, Record};
use sdrkit_driver_core::{DriverError, DriverKind, SourceDriver};
use sdrkit_driver_rtlusb::{
    RtlError, RtlKindCommand, RtlUsbDriver, DEVICES, D_ADDRESS, D_NAME, D_PATH, D_SERIAL, D_STATE,
    D_TUNER,
};
use sdrkit_driver_usb::{FakeUsb, Script};
use std::sync::Arc;

/// Two dongles: one with an R820T and a unique serial, one with an FC0013 and a serial that a third device shares.
const BUS: &str = r#"sdrkit-usb 1
mode lenient
device 1 0bda:2838 bus="1" ports=2 manufacturer="Realtek" product="RTL2838UHIDIR" serial="0001"
device 2 0bda:2838 bus="1" ports=3 manufacturer="Generic" product="RTL2832U DVB-T" serial="0"
device 3 0bda:2832 bus="2" ports=1.4 serial="0"
device 4 1d50:6089 bus="2" ports=2 serial="hackrf"
in 1 c0 00 0120 000a 1 -> 09
out 1 40 00 00c8 0610 02 -> !stall
out 1 40 00 00c6 0610 00 -> !stall
in 1 c0 00 0034 0600 1 -> 69
in 2 c0 00 0120 000a 1 -> 09
out 2 40 00 00c8 0610 02 -> !stall
in 2 c0 00 00c6 0600 1 -> a3
open 3 -> !busy
"#;

fn rows(k: &DriverKind) -> Vec<Record> {
    let id = k.results_decl().table_id(DEVICES.key).unwrap();
    let t = k.results().table(id).unwrap();
    let mut v: Vec<Record> = t.rows_newest_first().map(|(_, r)| r.clone()).collect();
    v.sort_by_key(|r| r.get(&D_PATH).unwrap().to_string());
    v
}

#[test]
fn the_declarations_build() {
    let d = RtlUsbDriver::new(Arc::new(FakeUsb::new(Script::new())));
    let decl = d.descriptor().decl();
    for key in [
        "centre_hz",
        "rate",
        "gain",
        "agc",
        "bias_tee",
        "ppm",
        "ds",
        "offset_tuning",
        "rtl_agc",
    ] {
        assert!(decl.schema().id(key).is_ok(), "no setting {key}");
    }
    let k = DriverKind::new(Box::new(d));
    assert!(k.commands().id("discover").is_ok());
    assert!(k.commands().id("address").is_ok());
    assert!(k.results_decl().table_id("devices").is_some());
}

#[test]
fn discover_lists_each_dongle_with_its_tuner_and_state() {
    let bus = FakeUsb::from_text(BUS).unwrap();
    let mut k = DriverKind::new(Box::new(RtlUsbDriver::new(Arc::new(bus))));
    let c = RtlKindCommand::Discover;
    let commands = RtlKindCommand::commands();
    let args = commands
        .check(c.id(), &commands.def(c.id()).unwrap().new_args())
        .unwrap();
    k.invoke(c.id(), &args).unwrap();
    let cells: Vec<[String; 6]> = rows(&k)
        .iter()
        .map(|r| {
            [&D_ADDRESS, &D_NAME, &D_SERIAL, &D_TUNER, &D_PATH, &D_STATE]
                .map(|f| r.get(f).unwrap_or_default().to_string())
        })
        .collect();
    assert_eq!(
        cells,
        [
            [
                "rtlusb:0001",
                "Realtek RTL2838UHIDIR",
                "0001",
                "R820T",
                "1-2",
                "free"
            ],
            [
                "rtlusb:@1-3",
                "Generic RTL2832U DVB-T",
                "0",
                "FC0013",
                "1-3",
                "free"
            ],
            ["rtlusb:@2-1.4", "", "0", "", "2-1.4", "busy"],
        ]
        .map(|r| r.map(String::from))
    );
}

#[test]
fn a_tuner_that_is_not_driven_is_refused_clearly() {
    let bus = FakeUsb::from_text(BUS).unwrap();
    let d = RtlUsbDriver::new(Arc::new(bus));
    let schema = d.descriptor().decl().schema().clone();
    let v = sdrkit_core::Values::defaults(&schema);
    let e = d.open("#1", &v).err().unwrap();
    let DriverError::Driver(s) = &e else {
        panic!("{e}")
    };
    assert_eq!(
        s.downcast_ref::<RtlError>(),
        Some(&RtlError::TunerNotSupported("FC0013"))
    );
    assert_eq!(e.to_string(), "tuner FC0013 not supported yet");
    // A dongle that is not on the bus is unavailable, with the reason "not found".
    assert!(matches!(d.open("@9-9", &v), Err(DriverError::Unavailable { .. })));
    assert!(matches!(d.open("#x", &v), Err(DriverError::Address)));
}
