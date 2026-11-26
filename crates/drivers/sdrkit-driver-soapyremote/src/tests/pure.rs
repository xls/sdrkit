//! The rules that need no connection.

use super::fake::{Fake, Script};
use super::*;
use crate::format::{choose_format, cs16_shift, Format, WireChoice};
use crate::log::LogLimiter;
use crate::plan::*;
use crate::radio::Facts;
use crate::rpc::{self, ArgInfo, ArgType, Range};
use crate::stream::{ack, ack_trigger, seq_event, DgHeader, SeqEvent};
use sdrkit_core::{SettingSet, Value, ValueKind};
use sdrkit_driver_core::{DriverError, RadioSetting};
use sdrkit_graph::stream::SampleType;
use std::time::Instant;

const M: f64 = 1e6;

fn r(a: f64, b: f64, s: f64) -> Range {
    Range::new(a, b, s)
}

#[test]
fn step_grid_examples() {
    assert_eq!(force_onto(r(0.0, 40.0, 0.0), 50.0, Snap::Nearest), 40.0);
    assert_eq!(force_onto(r(0.0, 40.0, 0.0), -3.0, Snap::Nearest), 0.0);
    assert_eq!(force_onto(r(0.0, 40.0, 8.0), 13.0, Snap::Nearest), 16.0);
    assert_eq!(force_onto(r(0.0, 40.0, 8.0), 39.0, Snap::Nearest), 40.0);
    assert_eq!(force_onto(r(0.0, 38.0, 8.0), 41.0, Snap::Nearest), 32.0);
    assert_eq!(force_onto(r(0.0, 40.0, 8.0), 9.0, Snap::Up), 16.0);
    assert_eq!(force_onto(r(0.0, 38.0, 8.0), 37.0, Snap::Up), 32.0);
}

#[test]
fn rate_choice_examples() {
    let airspy = [r(2.5 * M, 2.5 * M, 0.0), r(10.0 * M, 10.0 * M, 0.0)];
    let c = choose_rate(&airspy, 4.0 * M);
    assert_eq!((c.rate, c.exact), (10.0 * M, false));
    assert_eq!(
        c.note.unwrap(),
        "this radio does not offer 4.000 Msps; 10.000 Msps is used"
    );
    let c = choose_rate(&airspy, 2.5 * M);
    assert_eq!((c.rate, c.exact, c.note), (2.5 * M, true, None));
    let c = choose_rate(&airspy, 20.0 * M);
    assert_eq!(c.rate, 10.0 * M);
    assert_eq!(
        c.note.unwrap(),
        "this radio only offers up to 10.000 Msps, 20.000 Msps were requested"
    );
    let rtl = [r(225_001.0, 300_000.0, 0.0), r(900_001.0, 3.2 * M, 0.0)];
    assert!(choose_rate(&rtl, 2.4 * M).exact);
    let c = choose_rate(&rtl, 500_000.0);
    assert_eq!((c.rate, c.exact), (900_001.0, false));
    assert!(c.note.is_some());
    let c = choose_rate(&[], 1.0 * M);
    assert_eq!((c.rate, c.exact), (1.0 * M, true));
    // Within 1 Hz of a grid value is exact.
    assert!(choose_rate(&[r(0.0, 10.0 * M, 1000.0)], 2_400_000.5).exact);
    // The transmit rule: the smallest offered value at or above, else the fastest.
    assert_eq!(choose_rate_up(&airspy, 3.0 * M), 10.0 * M);
    assert_eq!(choose_rate_up(&airspy, 30.0 * M), 10.0 * M);
}

#[test]
fn filter_choice_example() {
    let t = [r(1.75 * M, 1.75 * M, 0.0), r(5.0 * M, 5.0 * M, 0.0)];
    assert_eq!(choose_bandwidth(&t, 4.0 * M), Some(5.0 * M));
    assert_eq!(choose_bandwidth(&t, 2.0 * M), Some(1.75 * M));
    assert_eq!(choose_bandwidth(&[], 2.0 * M), None);
}

#[test]
fn native_sample_types_and_the_cs16_shift() {
    let types: Vec<SampleType> = [Format::Cf32, Format::Cs16, Format::Cs8, Format::Cu8]
        .iter()
        .map(|f| f.sample_type())
        .collect();
    assert_eq!(
        types,
        [
            SampleType::Cf32,
            SampleType::Cs16,
            SampleType::Cs8,
            SampleType::Cu8
        ]
    );
    // CS16 native 2048 (Pluto): shifted by 4 to the full 16-bit scale.
    assert_eq!(cs16_shift(Format::Cs16, Some("CS16"), 2048.0), 4);
    // CS16 native at full scale, CS16 not native, and a scale that is no power of two: no shift.
    assert_eq!(cs16_shift(Format::Cs16, Some("CS16"), 32768.0), 0);
    assert_eq!(cs16_shift(Format::Cs16, Some("CS8"), 128.0), 0);
    assert_eq!(cs16_shift(Format::Cs16, Some("CS16"), 2000.0), 0);
    assert_eq!(cs16_shift(Format::Cs8, Some("CS8"), 8.0), 0);
    // The data datagram of the protocol example: four CS8 samples after the header.
    let d = hex(DATA0);
    let h = DgHeader::parse(&d).unwrap();
    assert_eq!((h.bytes, h.seq, h.elems), (32, 0, 4));
    assert_eq!(&d[24..], &[0x7f, 0x80, 0x00, 0x01, 0x40, 0xc0, 0xff, 0x00]);
}

#[test]
fn format_choice() {
    let listed = vec!["CS8".to_string(), "CS16".to_string(), "CF32".to_string()];
    assert_eq!(
        choose_format(WireChoice::Auto, Some("CS8"), Some(&listed)),
        (Format::Cs8, None)
    );
    assert_eq!(
        choose_format(WireChoice::Cs16, Some("CS8"), Some(&listed)).0,
        Format::Cs16
    );
    let only8 = vec!["CS8".to_string()];
    let (f, note) = choose_format(WireChoice::Cf32, Some("CS8"), Some(&only8));
    assert_eq!(f, Format::Cs8);
    assert_eq!(
        note.unwrap(),
        "the radio behind this server does not stream CF32; CS8 is used"
    );
    // A native format the driver does not take (CS12, CS32): CS16 if listed, else CF32.
    assert_eq!(
        choose_format(WireChoice::Auto, Some("CS12"), Some(&listed)).0,
        Format::Cs16
    );
    assert_eq!(
        choose_format(WireChoice::Auto, Some("CS32"), Some(&["CF32".to_string()])).0,
        Format::Cf32
    );
}

#[test]
fn datagram_sizes() {
    for (f, a, b) in [
        (Format::Cf32, 178, 1116),
        (Format::Cs16, 357, 2232),
        (Format::Cs8, 714, 4464),
        (Format::Cu8, 714, 4464),
    ] {
        assert_eq!(elements_per_datagram(1500, f.bytes()), a, "{f:?}");
        assert_eq!(elements_per_datagram(9000, f.bytes()), b, "{f:?}");
    }
    assert_eq!(window_packets(4_194_304, 1500), 2796);
    assert_eq!(window_packets(1 << 20, 1500), 699);
    assert_eq!(window_packets(100, 1500), 1);
    assert_eq!(ack_trigger(2796), 349);
    assert_eq!(ack_trigger(5), 1);
    assert_eq!(ack(0, 2796).to_vec(), hex(ACK0));
    assert_eq!(ack(349, 2796).to_vec(), hex(ACK349));
}

#[test]
fn sequences_wrap_without_loss() {
    let mut next = 0xffff_fffeu32;
    for s in [0xffff_fffeu32, 0xffff_ffff, 0, 1] {
        assert_eq!(seq_event(next, s), SeqEvent::InOrder);
        next = s.wrapping_add(1);
    }
    assert_eq!(seq_event(10, 13), SeqEvent::Lost(3));
    assert_eq!(seq_event(14, 11), SeqEvent::Late);
    assert_eq!(seq_event(0xffff_ffff, 2), SeqEvent::Lost(3));
}

#[test]
fn status_texts() {
    assert_eq!(status_text(0, 0, 0), "");
    assert_eq!(
        status_text(1, 2, 3),
        "1 overflow at the server (it or its network is too slow), 2 datagrams lost on the network, 3 reconnects"
    );
    assert_eq!(status_text(2, 1, 1), "2 overflows at the server (it or its network is too slow), 1 datagram lost on the network, 1 reconnect");
}

#[test]
fn remote_driver_texts_and_addresses() {
    let kw = |v: &[(&str, &str)]| {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect::<rpc::Kwargs>()
    };
    assert_eq!(parse_remote("rtlsdr"), kw(&[("driver", "rtlsdr")]));
    assert_eq!(
        parse_remote("driver=sdrplay, serial=7"),
        kw(&[("driver", "sdrplay"), ("serial", "7")])
    );
    assert_eq!(parse_remote("rtlsdr,,"), kw(&[("driver", "rtlsdr")]));
    assert_eq!(parse_remote(" \t"), kw(&[]));
    assert_eq!(
        clean_address("tcp://127.0.0.1:5000").unwrap(),
        ("127.0.0.1".into(), 5000)
    );
    assert_eq!(
        clean_address(" 10.0.0.2 ").unwrap(),
        ("10.0.0.2".into(), 55132)
    );
    assert_eq!(
        clean_address("[fe80::1]:55132").unwrap(),
        ("fe80::1".into(), 55132)
    );
    assert!(clean_address("h:0").is_err());
    assert_eq!(corrected(100e6, 3.0), 100e6 / 1.000003);
}

#[test]
fn arguments_and_busy_words() {
    let mut kw = rpc::Kwargs::new();
    kw.insert("remote:driver".into(), "lime".into());
    kw.insert("driver".into(), "remote".into());
    kw.insert("serial".into(), "9".into());
    assert_eq!(rpc::arg(&kw, rpc::DRIVER), Some("lime"));
    assert_eq!(rpc::arg(&kw, rpc::SERIAL), Some("9"));
    assert_eq!(rpc::arg(&kw, rpc::LABEL), None);
    assert!(
        is_busy("usb_claim_interface error -6")
            && is_busy("Device or resource BUSY")
            && !is_busy("no match")
    );
    assert!(is_bias_key("BiasTee") && is_bias_key("bias_tee") && !is_bias_key("bias"));
}

fn gr() -> Vec<Element> {
    vec![
        Element {
            name: "IFGR".into(),
            range: r(20.0, 59.0, 1.0),
        },
        Element {
            name: "RFGR".into(),
            range: r(0.0, 9.0, 1.0),
        },
    ]
}

#[test]
fn gain_plans() {
    let e = gr();
    let o = r(0.0, 48.0, 0.0);
    let els = |v: &[(&str, f64)]| {
        GainPlan::Elements(v.iter().map(|(a, b)| (a.to_string(), *b)).collect())
    };
    assert_eq!(
        gain_plan(o, &e, 48.0, None),
        els(&[("RFGR", 0.0), ("IFGR", 20.0)])
    );
    assert_eq!(
        gain_plan(o, &e, 0.0, None),
        els(&[("RFGR", 9.0), ("IFGR", 59.0)])
    );
    assert_eq!(
        gain_plan(o, &e, 24.0, None),
        els(&[("RFGR", 5.0), ("IFGR", 40.0)])
    );
    assert_eq!(
        gain_plan(o, &e, 48.0, Some(6)),
        els(&[("RFGR", 6.0), ("IFGR", 20.0)])
    );
    // An overall range of 0 to 1 dB with these elements produces 0 to 48.
    assert_eq!(overall_range(r(0.0, 1.0, 0.0), &e), r(0.0, 48.0, 0.0));
    // Another gain-reduction radio: max - (g - min). Others: g.
    let one = vec![Element {
        name: "IFGR".into(),
        range: r(20.0, 59.0, 1.0),
    }];
    assert_eq!(
        gain_plan(r(20.0, 59.0, 1.0), &one, 30.0, None),
        GainPlan::Overall(49.0)
    );
    let lv = vec![
        Element {
            name: "LNA".into(),
            range: r(0.0, 40.0, 8.0),
        },
        Element {
            name: "VGA".into(),
            range: r(0.0, 62.0, 2.0),
        },
    ];
    assert!(!is_gain_reduction(&lv));
    assert_eq!(
        gain_plan(r(0.0, 102.0, 0.0), &lv, 30.0, None),
        GainPlan::Overall(30.0)
    );
    assert_eq!(
        gain_plan(r(0.0, 102.0, 0.0), &lv, 300.0, None),
        GainPlan::Overall(102.0)
    );
    assert!(!is_gain_reduction(&[Element {
        name: "GR".into(),
        range: r(0.0, 1.0, 0.0)
    }]));
}

#[test]
fn the_settings_the_radio_reveals_are_checked() {
    let f = Facts {
        elements: gr(),
        gain: Some(r(0.0, 1.0, 0.0)),
        antennas: vec!["A".into(), "B".into()],
        clocks: vec!["internal".into(), "external".into(), "gpsdo".into()],
        clock: Some("internal".into()),
        times: vec!["none".into()],
        rates: vec![r(2.5e6, 2.5e6, 0.0)],
        ..Default::default()
    };
    assert_eq!(f.capabilities("h:1").gain_db, Some((0.0, 48.0, 0.0)));
    let d = drv(&Fake::start(Script::rtl()));
    let ok = values(&d, &[]);
    assert_eq!(f.check(&ok), Ok(()));
    let refused = |key: &str, v: Value| f.check(&values(&d, &[(key, v)])).unwrap_err();
    assert_eq!(
        refused("bias_tee", Value::Bool(true)),
        DriverError::Refused {
            id: RadioSetting::BiasTee.id()
        }
    );
    let exact = values(
        &d,
        &[
            ("rate", Value::Hz(2.4e6)),
            ("rate_exact", Value::Bool(true)),
        ],
    );
    assert_eq!(
        f.check(&exact),
        Err(DriverError::Refused {
            id: RadioSetting::Rate.id()
        })
    );
    // A radio without an RF stage has no setting of an RF state.
    assert!(d.descriptor().decl().schema().id("rfgr").is_err());
    assert_eq!(Facts::default().rf_state(&ok), None);
}

#[test]
fn the_log_limit() {
    let mut l = LogLimiter::default();
    let t0 = Instant::now();
    let passed = (0..1000)
        .filter(|i| l.admit(t0 + Duration::from_micros(*i)))
        .count();
    assert_eq!(passed, 20);
    assert_eq!(l.summary(t0 + Duration::from_secs(1), false), None);
    assert_eq!(
        l.summary(t0 + Duration::from_secs(11), false).unwrap(),
        "980 more server log lines"
    );
    assert_eq!(l.summary(t0 + Duration::from_secs(30), true), None);
    // A new second admits again.
    assert!(l.admit(t0 + Duration::from_secs(2)));
}

#[test]
fn default_timings() {
    let t = Timing::default();
    assert_eq!(t.call, Duration::from_secs(10));
    assert_eq!(t.make, Duration::from_secs(20));
    assert_eq!(t.setup, Duration::from_secs(10));
    assert_eq!(t.close_call, Duration::from_secs(1));
    assert_eq!(t.log_start, Duration::from_secs(5));
    assert_eq!(
        (t.first_data, t.stall, t.slice),
        (
            Duration::from_secs(3),
            Duration::from_secs(3),
            Duration::from_millis(100)
        )
    );
    assert_eq!(t.connect_timeout("10.0.0.1"), Duration::from_secs(8));
    assert_eq!(t.connect_timeout("127.0.0.1"), Duration::from_secs(10));
    assert_eq!(t.tx_waits, 50);
    assert_eq!(show_version(0x400), "4.0");
    assert_eq!(error_name(-7), "UNDERFLOW");
}

fn info(kind: ArgType, value: &str, range: Range, options: &[&str], names: &[&str]) -> ArgInfo {
    ArgInfo {
        key: "k".into(),
        value: value.into(),
        name: "Name".into(),
        description: String::new(),
        units: "dB".into(),
        kind: Some(kind),
        range,
        options: options.iter().map(|s| s.to_string()).collect(),
        option_names: names.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn arg_info_to_settings() {
    // Bool, info true, radio false: default off.
    let s = module_setting(
        &info(ArgType::Bool, "true", Range::default(), &[], &[]),
        Some("false"),
    )
    .unwrap();
    assert_eq!(s.decl.field.key, "soapy.k");
    assert_eq!(s.decl.field.kind, ValueKind::Bool);
    assert_eq!(s.decl.default, Value::Bool(false));
    assert_eq!(s.wire(&Value::Bool(true)).unwrap(), "true");
    // Free text: nothing.
    assert!(module_setting(
        &info(ArgType::String, "x", Range::default(), &[], &[]),
        None
    )
    .is_none());
    // Int 0..10 step 0: step 1, empty value 0.
    let s = module_setting(
        &info(ArgType::Int, "", r(0.0, 10.0, 0.0), &[], &[]),
        Some(""),
    )
    .unwrap();
    let range = s.decl.range.unwrap();
    assert_eq!(
        (range.min, range.max, range.step, s.decl.field.unit.as_ref()),
        (0.0, 10.0, 1.0, "dB")
    );
    assert_eq!(s.decl.default, Value::I64(0));
    assert_eq!(s.wire(&Value::I64(7)).unwrap(), "7");
    // Float with step 0 stays continuous.
    let s = module_setting(
        &info(ArgType::Float, "1.5", r(0.0, 3.0, 0.0), &[], &[]),
        None,
    )
    .unwrap();
    assert_eq!(s.decl.range.unwrap().step, 0.0);
    assert_eq!(s.decl.default, Value::F64(1.5));
    // Option names of the wrong count fall back to the options; a value outside them becomes the first.
    let s = module_setting(
        &info(ArgType::String, "9", Range::default(), &["a", "b"], &["A"]),
        None,
    )
    .unwrap();
    let labels: Vec<_> = s
        .decl
        .field
        .choices
        .iter()
        .map(|c| c.label.as_ref())
        .collect();
    assert_eq!(labels, ["a", "b"]);
    assert_eq!(s.decl.default, Value::Choice(0));
    assert_eq!(s.wire(&Value::Choice(1)).unwrap(), "b");
    assert_eq!(s.wire(&Value::Choice(2)), None);
    // Help texts; the empty key and the bias key are skipped.
    assert_eq!(s.decl.help, "A setting of the radio's SoapySDR driver (k).");
    let mut i = info(ArgType::Bool, "0", Range::default(), &[], &[]);
    i.description = "Turns it on".into();
    assert_eq!(
        module_setting(&i, None).unwrap().decl.help,
        "Turns it on (SoapySDR driver setting k)."
    );
    i.key = "biasT_ctrl".into();
    assert!(module_setting(&i, None).is_none());
    i.key = String::new();
    assert!(module_setting(&i, None).is_none());
    assert!(is_bias_key("BiasTee") && is_bias_key("bias_tee") && !is_bias_key("bias"));
}

#[test]
fn gain_stages_and_their_labels() {
    // A gain-reduction radio: its RF stage has a setting of its own, so it has no stage settings.
    assert_eq!(stages(&gr()).count(), 0);
    let lna_vga = vec![
        Element {
            name: "LNA".into(),
            range: r(0.0, 40.0, 8.0),
        },
        Element {
            name: "VGA".into(),
            range: r(0.0, 62.0, 2.0),
        },
    ];
    let v: Vec<RunTime> = stages(&lna_vga)
        .zip([Some(16.0), None])
        .map(|(e, g)| stage_setting(e, g))
        .collect();
    let labels: Vec<_> = v.iter().map(|s| s.decl.field.label.as_ref()).collect();
    assert_eq!(labels, ["LNA gain", "VGA gain"]);
    let keys: Vec<_> = v.iter().map(|s| s.decl.field.key.as_ref()).collect();
    assert_eq!(keys, ["lna_gain", "vga_gain"]);
    // Each starts following the gain.
    assert!(v.iter().all(|s| s.decl.default == Value::I64(-1)));
    assert_eq!(v[0].wire(&Value::I64(8)), None);
    // A stage with another name keeps it; an amplifier of one step is on or off, as it was at open.
    let other = Element {
        name: "TIA".into(),
        range: r(0.0, 12.0, 3.0),
    };
    assert_eq!(stage_setting(&other, None).decl.field.key, "gain.TIA");
    let amp = Element {
        name: "AMP".into(),
        range: r(0.0, 14.0, 14.0),
    };
    let a = stage_setting(&amp, Some(14.0));
    assert_eq!(
        (a.decl.field.key.as_ref(), a.decl.default.clone()),
        ("amp", Value::Bool(true))
    );
    // One element: the gain setting drives it, no stage settings.
    assert_eq!(stages(&lna_vga[..1]).count(), 0);
}
