//! The transport stream monitor.

use sdrkit_core::{Choice, SettingSet, Value};
use sdrkit_graph::{Component, Registry};
use sdrkit_proto_mpegts::monitor::{
    results_decl, CheckFill, TsMonitor, TsMonitorComponent, TsMonitorSetting,
};
use sdrkit_proto_mpegts::source::{Numbered, PacketSource, TestMux};

#[test]
fn counts_names_and_bit_rate() {
    let mut mux = TestMux::satellite_cable(50).unwrap();
    let mut mon = TsMonitor::new(mux.numbered());
    let mut p = [0u8; 188];
    // 2,000 packets over 1 s of signal: 2,000 * 1,504 bit/s.
    for i in 0..2000 {
        mux.next_packet(&mut p);
        if i == 700 {
            p[1] |= 0x80;
        }
        if i == 900 {
            p[3] |= 0x80;
        }
        mon.push(&p, 10.0 + i as f64 * 5e-4);
    }
    let s = mon.stats();
    assert_eq!((s.packets, s.flagged, s.scrambled), (2000, 1, 1));
    assert!(s.pat_seen);
    assert_eq!(s.services, ["sdrkit test"]);
    assert!(
        (s.bitrate_bps - 2000.0 * 1504.0).abs() < 0.02 * 2000.0 * 1504.0,
        "{}",
        s.bitrate_bps
    );
    assert_eq!(s.numbered.gaps, 0);
    // 47 packets in each period of 50 are data packets.
    assert_eq!(
        s.numbered.good + s.numbered.flagged + s.numbered.wrong,
        1880
    );
    mon.clear();
    assert_eq!(mon.stats().packets, 0);
    assert_eq!(mon.pattern(), Some(Numbered::SATELLITE_CABLE));
}

#[test]
fn the_component_is_registered_and_strict() {
    let mut r = Registry::new();
    sdrkit_proto_mpegts::register(&mut r);
    let c = r.make("mpegts.ts_monitor").unwrap();
    assert_eq!(c.info().name, "mpegts.ts_monitor");
    let mut c = TsMonitorComponent::default();
    let s = TsMonitorSetting::schema();
    let (pid, fill) = (
        TsMonitorSetting::CheckPid.id(),
        TsMonitorSetting::CheckFill.id(),
    );
    assert!(s.check(pid, &Value::I64(9000)).is_err());
    assert!(s
        .setting(fill)
        .unwrap()
        .field
        .choice_index("other")
        .is_none());
    assert!(c
        .apply(fill, &Value::Choice(CheckFill::I31N7.index()))
        .is_ok());
    assert!(c.apply(pid, &Value::I64(257)).is_ok());
    assert_eq!(c.monitor().pattern().unwrap().pid, 257);
    assert!(s.id("nothing").is_err());
    assert!(results_decl().field_id("bitrate_bps").is_some());
}
