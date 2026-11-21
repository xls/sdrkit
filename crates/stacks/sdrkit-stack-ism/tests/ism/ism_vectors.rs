//! `ism`: the recorded scenes give exactly the devices of expected.json. A field of expected.json in another unit or kind than
//! the measurement catalogue is converted first.

use sdrkit_core::{Choice, Complex32, InputInfo, Output, Receiver, StackConfig, Value};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_proto_ookdev::Check;
use sdrkit_stack_ism::ism::devices::{Devices, Store};
use sdrkit_stack_ism::ism::{self, ISM};
use serde_json::Value as Json;

fn run(name: &str, chunk: usize, check: impl FnOnce(&Store)) {
    let rec = sdrkit_dev_test::read_recording(&format!("ism/{name}"))
        .unwrap_or_else(|| panic!("the recording ism/{name}"));
    let mut rx = ism::receiver(&StackConfig::new(sdrkit_core::Values::defaults(&(ISM
        .settings)(
    ))))
    .unwrap();
    rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording())
        .unwrap();
    let mut out = Output::default();
    let mut x = rec.samples.clone();
    x.extend(std::iter::repeat_n(
        Complex32::default(),
        (rec.rate_hz * 0.1) as usize,
    ));
    for c in x.chunks(chunk) {
        rx.feed(c, &mut out);
        out.clear();
    }
    check(
        rx.graph()
            .component_as::<Devices>("devices")
            .expect("the devices component")
            .store(),
    );
}

/// A field of expected.json as the catalogue holds it: (key, value, scale of the tolerance).
fn catalogue(key: &str, v: &Json) -> (&'static str, Json, f64) {
    let n = || v.as_f64().unwrap_or(f64::NAN);
    match key {
        "temperature_f" => ("temperature_c", Json::from((n() - 32.0) * 5.0 / 9.0), 1.0),
        "pressure_psi" => ("pressure_kpa", Json::from(n() * 6.894_757), 6.894_757),
        "wind_speed_km_h" => ("wind_speed_m_s", Json::from(n() / 3.6), 1.0),
        "wind_gust_km_h" => ("wind_gust_m_s", Json::from(n() / 3.6), 1.0),
        "interval_s" => ("interval", v.clone(), 1.0),
        "learn" => ("learning", Json::Bool(n() != 0.0), 1.0),
        "moving" => ("moving", Json::Bool(n() != 0.0), 1.0),
        "flags" => (
            "flags",
            Json::from(i64::from_str_radix(v.as_str().unwrap_or(""), 16).unwrap_or(-1)),
            1.0,
        ),
        "command" if v.is_number() => ("command", Json::from(v.to_string()), 1.0),
        "source" => ("source_node", v.clone(), 1.0),
        "destination" => ("destination", Json::from(v.to_string()), 1.0),
        "value" => ("switch_on", Json::Bool(v.as_str() == Some("on")), 1.0),
        "power_w" => ("power_0_w", v.clone(), 1.0),
        k => (
            sdrkit_proto_ookdev::reading::measure::ALL
                .iter()
                .map(|f| f.key())
                .find(|x| *x == k)
                .unwrap_or_else(|| panic!("no measurement {k}")),
            v.clone(),
            1.0,
        ),
    }
}

fn check(name: &str) {
    let exp = read_expected("ism").unwrap();
    let e = &exp["recordings"][name]["expect"];
    run(name, 65536, |s| {
        for d in s.devices.iter() {
            eprintln!(
                "{} {} {:?} {:?} {}",
                d.record.model, d.record.id, d.record.channel, d.record.battery_ok, d.record.line
            );
        }
        for u in s.unknown.iter() {
            eprintln!("unknown: {} at {:.0}", u.line(), u.offset_hz);
        }
        let tol = e["value_tolerance"].as_f64().unwrap_or(0.051);
        let ftol = e["freq_tolerance_hz"].as_f64().unwrap();
        for w in e["devices"].as_array().unwrap() {
            let model = w["model"].as_str().unwrap();
            let id = w["id"].as_str().unwrap();
            let dev = s
                .devices
                .iter()
                .find(|x| x.record.model == model && x.record.id == id)
                .unwrap_or_else(|| panic!("{model} {id} missing"))
                .record;
            assert_eq!(dev.kind, w["kind"].as_str().unwrap(), "{model} kind");
            let check = &Check::CHOICES[usize::from(dev.check.index())].key;
            assert!(
                check.eq_ignore_ascii_case(w["check"].as_str().unwrap()),
                "{model} check {check}"
            );
            if let Some(c) = w.get("channel").or_else(|| w.get("channel_name")) {
                assert_eq!(
                    serde_json::to_value(&dev.channel).unwrap(),
                    *c,
                    "{model} channel"
                );
            }
            if let Some(b) = w.get("battery_ok") {
                assert_eq!(dev.battery_ok, b.as_bool(), "{model} battery");
            }
            let sent = w["sent_offset_hz"].as_f64().unwrap();
            assert!(
                (dev.offset_hz - sent).abs() <= ftol,
                "{model}: frequency {} vs {sent}",
                dev.offset_hz
            );
            for (k, v) in w["fields"].as_object().unwrap() {
                let (k, v, scale) = catalogue(k, v);
                let f = dev
                    .fields
                    .iter()
                    .find(|f| f.key() == k)
                    .unwrap_or_else(|| panic!("{model}: no field {k} in {}", dev.line));
                match (f.value(), &v) {
                    (Value::F64(value) | Value::Duration(value), Json::Number(n)) => {
                        assert!(
                            (value - n.as_f64().unwrap()).abs() <= tol * scale,
                            "{model} {k}: {value} vs {n}"
                        )
                    }
                    (Value::I64(value), Json::Number(n)) => {
                        assert_eq!(Some(*value), n.as_i64(), "{model} {k}")
                    }
                    (Value::Text(t), Json::String(x)) => assert_eq!(t, x, "{model} {k}"),
                    (Value::Bool(a), Json::Bool(b)) => assert_eq!(a, b, "{model} {k}"),
                    (a, b) => panic!("{model} {k}: {a:?} vs {b}"),
                }
            }
        }
        if let Some(n) = e["devices_exactly"].as_u64() {
            assert_eq!(s.devices.len() as u64, n, "device count");
        }
        if let Some(n) = e["unknown_bursts_max"].as_u64() {
            assert!(s.unknown_bursts <= n, "unknown bursts {}", s.unknown_bursts);
        }
    });
}

#[test]
fn scene_433() {
    check("ism_433_scene_1msps");
}

#[test]
fn scene_868() {
    check("ism_868_scene_1msps");
}

#[test]
fn zwave_by_ism() {
    check("zwave_868_4_1msps");
}
