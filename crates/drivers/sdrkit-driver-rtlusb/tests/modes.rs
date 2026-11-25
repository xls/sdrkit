//! Direct sampling and the bias tee of an R820T dongle on a scripted bus: what the driver writes.

use sdrkit_core::{Choice, SettingSet, Value, Values};
use sdrkit_driver_core::{DriverKind, RadioSetting, SourceDriver};
use sdrkit_driver_rtlusb::r82xx::pll_for;
use sdrkit_driver_rtlusb::{DirectSampling, RtlError, RtlSetting, RtlUsbDriver};
use sdrkit_driver_usb::{FakeUsb, RecordOptions, RecordingUsb, Step};
use std::sync::Arc;

const BUS: &str = r#"sdrkit-usb 1
mode lenient
device 1 0bda:2838 bus="1" ports=2 manufacturer="Realtek" product="RTL2838UHIDIR" serial="0001"
out 1 40 00 00c8 0610 02 -> !stall
out 1 40 00 00c6 0610 00 -> !stall
in 1 c0 00 0120 000a 1 -> 09
in 1 c0 00 0034 0600 1 -> 69
in 1 c0 00 0034 0600 5 -> 69005a0014
in 1 c0 00 0034 0600 3 -> 69005a
in 1 c0 00 0034 0600 4 -> 69005a00
in 1 c0 00 3004 0200 1 -> 0e
in 1 c0 00 3003 0200 1 -> 19
in 1 c0 00 3001 0200 1 -> 18
"#;

/// The control OUT transfers as `<wValue> <wIndex> <data>`, from step `from` on.
fn outs(rec: &RecordingUsb, from: usize) -> Vec<String> {
    rec.script()
        .steps()
        .skip(from)
        .filter_map(|(_, s)| match s {
            Step::ControlOut { setup, data, .. } => Some(format!(
                "{:04x} {:04x} {}",
                setup.value,
                setup.index,
                data.iter().map(|b| format!("{b:02x}")).collect::<String>()
            )),
            _ => None,
        })
        .collect()
}

fn setup() -> (RecordingUsb, RtlUsbDriver, Values) {
    let rec = RecordingUsb::new(
        Arc::new(FakeUsb::from_text(BUS).unwrap()),
        RecordOptions::default(),
    );
    let d = RtlUsbDriver::new(Arc::new(rec.clone()));
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Rate.id(), &Value::Hz(2.4e6)).unwrap();
    (rec, d, v)
}

fn set(v: &mut Values, s: impl SettingSet, value: Value) {
    v.set(sdrkit_driver_core::own_id(s), &value).unwrap();
}

#[test]
fn direct_sampling_on_the_q_input_below_24_mhz() {
    let (rec, d, mut v) = setup();
    set(
        &mut v,
        RtlSetting::DirectSampling,
        Value::Choice(DirectSampling::Q.index()),
    );
    v.set(RadioSetting::Centre.id(), &Value::Hz(7.1e6)).unwrap();
    let mut src = d.open("", &v).unwrap();
    let o = outs(&rec, 0);
    let at = |w: &str| {
        o.iter()
            .rposition(|x| x == w)
            .unwrap_or_else(|| panic!("no `{w}`"))
    };
    // The tuner goes to standby, then the demodulator takes the Q input with the IF at 7.1 MHz.
    let standby = at("0034 0610 06b1");
    assert!(standby < at("b120 0011 1a"));
    assert!(at("0620 0010 90") > standby);
    // IF register for 7.1 MHz: -floor(7.1e6 * 2^22 / 28.8e6) mod 2^22 = 0x3038E4.
    assert!(o
        .windows(3)
        .any(|w| w == ["1920 0011 30", "1a20 0011 38", "1b20 0011 e4"]));
    // Above 24 MHz the tuner is back: initialised again and tuned.
    let n = rec.script().steps().count();
    v.set(RadioSetting::Centre.id(), &Value::Hz(100e6)).unwrap();
    src.apply(&v).unwrap();
    let o = outs(&rec, n);
    assert!(
        o.contains(&"0034 0610 05833275c040d66c".to_string()),
        "no tuner initialisation"
    );
    assert!(o.contains(&"0620 0010 80".to_string()));
    assert!(o.contains(&"1520 0011 01".to_string()));
    assert!(o.iter().any(|x| x.starts_with("0034 0610 10")), "no tune");
    src.stop();
}

#[test]
fn the_bias_tee_stays_off_unless_it_is_set() {
    let (rec, d, mut v) = setup();
    let mut src = d.open("", &v).unwrap();
    let gpo = |o: &[String]| -> Vec<String> {
        o.iter()
            .filter(|x| x.starts_with("3001 0210"))
            .cloned()
            .collect()
    };
    assert!(
        gpo(&outs(&rec, 0)).is_empty(),
        "GPIO written without the bias tee set"
    );
    let n = rec.script().steps().count();
    v.set(RadioSetting::BiasTee.id(), &Value::Bool(true))
        .unwrap();
    src.apply(&v).unwrap();
    // GPIO 0 an output, then driven high: 0x18 read back, 0x19 written.
    let o = outs(&rec, n);
    assert!(o.contains(&"3004 0210 0e".to_string()));
    assert!(o.contains(&"3003 0210 19".to_string()));
    assert_eq!(gpo(&o), ["3001 0210 19"]);
    // The stop switches it off again.
    let n = rec.script().steps().count();
    src.stop();
    assert_eq!(gpo(&outs(&rec, n)), ["3001 0210 18"]);
}

#[test]
fn a_pll_that_does_not_lock_fails_the_open() {
    let text = BUS.replace(
        "in 1 c0 00 0034 0600 3 -> 69005a",
        "in 1 c0 00 0034 0600 3 -> 690000",
    );
    let rec = RecordingUsb::new(
        Arc::new(FakeUsb::from_text(&text).unwrap()),
        RecordOptions::default(),
    );
    let d = RtlUsbDriver::new(Arc::new(rec.clone()));
    let v = Values::defaults(d.descriptor().decl().schema());
    let e = d.open("", &v).err().unwrap();
    let sdrkit_driver_core::DriverError::Driver(s) = &e else {
        panic!("{e}")
    };
    assert_eq!(
        s.downcast_ref::<RtlError>(),
        Some(&RtlError::PllNotLocked { hz: 56_000_000 })
    );
    // The second try with more VCO current was made.
    assert!(outs(&rec, 0).contains(&"0034 0610 1260".to_string()));
}

const BLOG_V4: &str = r#"sdrkit-usb 1
mode lenient
device 1 0bda:2838 bus="1" ports=2 manufacturer="RTLSDRBlog" product="Blog V4" serial="0001"
out 1 40 00 00c8 0610 02 -> !stall
out 1 40 00 00c6 0610 00 -> !stall
out 1 40 00 0034 0610 00 -> !stall
in 1 c0 00 0120 000a 1 -> 09
in 1 c0 00 0074 0600 1 -> 69
in 1 c0 00 0074 0600 5 -> 69005a0014
in 1 c0 00 0074 0600 3 -> 69005a
in 1 c0 00 0074 0600 4 -> 69005a00
in 1 c0 00 3004 0200 1 -> 0e
in 1 c0 00 3003 0200 1 -> 19
in 1 c0 00 3001 0200 1 -> 18
"#;

#[test]
fn a_blog_v4_upconverts_hf_and_switches_its_inputs_once_per_band() {
    let rec = RecordingUsb::new(
        Arc::new(FakeUsb::from_text(BLOG_V4).unwrap()),
        RecordOptions::default(),
    );
    let d = RtlUsbDriver::new(Arc::new(rec.clone()));
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Rate.id(), &Value::Hz(2.4e6)).unwrap();
    v.set(RadioSetting::Centre.id(), &Value::Hz(7e6)).unwrap();
    let mut src = d.open("", &v).unwrap();
    assert!(src.capabilities().name.contains("R828D"));
    assert_eq!(src.capabilities().min_freq_hz, 0.5e6);
    // HF: the tuner is at 7 + 28.8 MHz plus the IF of 1.815 MHz, from a 28.8 MHz reference with p = 1.
    let p = pll_for(37_615_000, 28_800_000, 1).unwrap();
    let tail = format!("{:02x}{:02x}{:02x}", p.r14, p.sdm & 0xFF, p.sdm >> 8);
    let o = outs(&rec, 0);
    assert!(
        o.iter()
            .any(|x| x.starts_with("0074 0610 10") && x.ends_with(&tail)),
        "no PLL write for 35.8 MHz"
    );
    let gpo = |o: &[String]| -> Vec<String> {
        o.iter()
            .filter(|x| x.starts_with("3001 0210"))
            .cloned()
            .collect()
    };
    // GPIO 5 low for HF.
    assert_eq!(gpo(&o), ["3001 0210 18"]);
    // VHF: GPIO 5 high, the cable 1 input; then a tune in the same band switches nothing.
    let n = rec.script().steps().count();
    v.set(RadioSetting::Centre.id(), &Value::Hz(100e6)).unwrap();
    src.apply(&v).unwrap();
    assert_eq!(gpo(&outs(&rec, n)), ["3001 0210 38"]);
    let n = rec.script().steps().count();
    v.set(RadioSetting::Centre.id(), &Value::Hz(101e6)).unwrap();
    src.apply(&v).unwrap();
    assert!(gpo(&outs(&rec, n)).is_empty());
    src.stop();
}

#[test]
fn the_kind_opens_a_dongle_with_the_default_settings() {
    let k = DriverKind::new(Box::new(RtlUsbDriver::new(Arc::new(
        FakeUsb::from_text(BUS).unwrap(),
    ))));
    let v = Values::defaults(k.decl().schema());
    let mut radio = k.open("", &v).unwrap();
    assert_eq!(radio.capabilities().rates.max_hz, 3.2e6);
    let mut events = Vec::new();
    radio.poll(0.0, &mut events);
    radio.stop();
}
