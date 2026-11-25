//! EnOcean: the test telegrams of ERP1 and ERP2 at two rates, with an offset and noise; noise alone; the readings after a
//! teach-in.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, Generator as _, InputInfo, Owner, Receiver, SettingSet, StackConfig, Values};
use sdrkit_stack_ism::enocean::{self, scene, EnoceanStackSetting, GenRadio, ERP1_HZ};
use sdrkit_stack_ism::typed::{EnoceanDeviceRow, EnoceanFields, ENOCEAN_DEVICES};

/// The fields and the device rows after `seconds` of the test signal of `radio` at `rate`, moved by `offset_hz`, with complex
/// noise of `sigma` (none for `signal` false).
fn run(
    rate: f64,
    radio: GenRadio,
    seconds: f64,
    offset_hz: f64,
    sigma: f32,
    signal: bool,
) -> ([i64; 4], Vec<(String, String)>) {
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    if signal {
        scene(rate, radio).unwrap().generate(&mut x);
    }
    let mut r = Rng::new(7);
    for (i, v) in x.iter_mut().enumerate() {
        let ph = (std::f64::consts::TAU * offset_hz * i as f64 / rate) as f32;
        *v = *v * Complex32::from_polar(1.0, ph)
            + Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32());
    }
    let mut rx = enocean::receiver(&StackConfig::new(Values::defaults(
        &EnoceanStackSetting::schema(),
    )))
    .unwrap();
    rx.configure(&InputInfo::new(rate, ERP1_HZ, ERP1_HZ).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    for b in x.chunks(16_384) {
        rx.feed(b, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    let f = EnoceanFields::read(rx.results().fields()).expect("the EnOcean fields");
    let counts = [f.telegrams, f.subtelegrams, f.frames_bad, f.devices];
    let rows = ENOCEAN_DEVICES
        .rows::<EnoceanDeviceRow>(rx.results())
        .map(|(_, r)| (r.name.to_string(), r.reading.to_string()))
        .collect();
    (counts, rows)
}

fn reading<'a>(rows: &'a [(String, String)], name: &str) -> &'a str {
    rows.iter()
        .find(|r| r.0.contains(name))
        .map_or("", |r| r.1.as_str())
}

#[test]
fn erp1_every_telegram_once_with_its_reading() {
    // 12 telegrams of 3 subtelegrams every 80 ms: 0.965 s holds one round.
    let (f, rows) = run(1e6, GenRadio::Erp1, 0.965, 0.0, 0.003, true);
    assert_eq!(f, [12, 36, 0, 7]);
    assert_eq!(reading(&rows, "A5-02-05"), "21.0 °C");
    assert_eq!(reading(&rows, "A5-04-01"), "50.0 % humidity, 20.0 °C");
    assert_eq!(reading(&rows, "A5-07-01"), "motion, supply 3.30 V");
    assert_eq!(reading(&rows, "A5-06-01"), "30416 lx, supply 3.00 V");
    assert_eq!(reading(&rows, "D5-00-01"), "contact closed");
    assert_eq!(reading(&rows, "00291A01"), "no button released (as F6-02-01)");
    assert_eq!(reading(&rows, "01820001"), "encrypted, 6 bytes");
}

#[test]
fn erp2_and_other_rates_and_offsets() {
    for (rate, radio, off) in [
        (1e6, GenRadio::Erp2, 0.0),
        (1e6, GenRadio::Erp2, 15e3),
        (2e6, GenRadio::Erp1, 40e3),
        (500e3, GenRadio::Erp1, 0.0),
    ] {
        let (f, rows) = run(rate, radio, 0.965, off, 0.003, true);
        assert_eq!((f[0], f[2]), (12, 0), "{rate} {radio:?} {off}");
        assert_eq!(reading(&rows, "A5-02-05"), "21.0 °C");
        if radio == GenRadio::Erp2 {
            // No status in ERP2: the rocker byte is shown as it is.
            assert_eq!(reading(&rows, "00291A01"), "switch data 00");
        }
    }
}

#[test]
fn noise_alone_gives_nothing() {
    let (f, _) = run(1e6, GenRadio::Erp1, 2.0, 0.0, 0.1, false);
    assert_eq!(f[0], 0);
}


