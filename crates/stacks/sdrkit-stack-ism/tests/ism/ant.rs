//! `ant`: the frame vectors, the generator round trip, bad CRCs, other networks, noise, and the declared events.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Receiver, SettingSet, StackConfig, Value,
};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_graph::{DecoderComponent, StackGraph};
use sdrkit_stack_ism::ant::{self, AntSetting, Demod, GenParams, Sensors, TestSensor, ANT};
use sdrkit_stack_ism::family::from_hex;

/// Feeds `x` to a fresh `ant` receiver with the harness's settings, and ends the stream.
fn feed(h: &Harness, x: &[Complex32]) -> StackGraph {
    let mut r = ant::receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    r.configure(&InputInfo::new(4e6, 2457e6, 2457e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        out.clear();
    }
    r.finish(&mut out);
    r
}

/// The signal of `p` for `seconds`, through the harness's standard impairments.
fn signal(h: &Harness, p: GenParams, seconds: f64) -> Vec<Complex32> {
    let g: Box<dyn Generator> = Box::new(ant::generator(4e6, p));
    let mut g = sdrkit_components::sim::impaired(g, &h.values().unwrap()).unwrap();
    let mut x = vec![Complex32::default(); (4e6 * seconds) as usize];
    g.generate(&mut x);
    x
}

fn sensors(r: &StackGraph) -> &ant::Store {
    r.graph()
        .component_as::<Sensors>("sensors")
        .unwrap()
        .store()
}

fn field(r: &StackGraph, k: &str) -> f64 {
    crate::util::number(r.results(), k)
}

fn noisy(snr_db: f64) -> Harness {
    Harness::new(&ANT)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .seed(4)
}

fn sensor(dev: u16, marker: u16, bad_every: u64) -> TestSensor {
    TestSensor {
        device: dev,
        dtype: 120,
        tx: 1,
        period: 0.25,
        offset: 0.0,
        level_db: 0.0,
        bad_every,
        marker,
    }
}

#[test]
fn frame_vectors() {
    let exp = read_expected("ism").unwrap();
    for v in exp["byte_vectors"]["ant_frames_17_bytes"]
        .as_array()
        .unwrap()
    {
        let b = from_hex(v["hex"].as_str().unwrap()).unwrap();
        assert!(ant::crc_ok(&b));
        assert_eq!(
            format!("{:02X}{:02X}", b[0], b[1]),
            v["marker"].as_str().unwrap()
        );
        assert_eq!(
            u16::from_le_bytes([b[2], b[3]]) as u64,
            v["device_number"].as_u64().unwrap()
        );
        assert_eq!((b[4] & 0x7F) as u64, v["device_type"].as_u64().unwrap());
        assert_eq!(b[5] as u64, v["transmission_type"].as_u64().unwrap());
        let mut p = [0u8; 8];
        p.copy_from_slice(&b[7..15]);
        let marker = u16::from_be_bytes([b[0], b[1]]);
        assert_eq!(ant::build(marker, 49, 120, 1, &p).to_vec(), b);
        for i in 0..17 * 8 {
            let mut x = b.clone();
            x[i / 8] ^= 0x80 >> (i % 8);
            assert!(!ant::crc_ok(&x));
        }
    }
}

#[test]
fn generator_round_trip() {
    let secs = 20.0;
    // SNR 25 dB in 1 MHz: the -6 dB sensor is at 19 dB.
    let h = noisy(25.0)
        .set(&GEN_OFFSET_HZ, 30e3)
        .set(&GEN_CLOCK_PPM, 50.0)
        .seconds(secs);
    let r = feed(&h, &h.generate().unwrap());
    let st = sensors(&r);
    let find = |dev: u16, t: u8| {
        st.sensors
            .get(&ant::sensor_key(dev, t, if t == 11 { 5 } else { 1 }))
            .unwrap_or_else(|| panic!("sensor {dev}"))
    };
    let hr = find(26945, 120);
    let pm = find(12345, 11);
    let sc = find(777, 121);
    assert_eq!(hr.manufacturer.as_deref(), Some("garmin"));
    assert_eq!(hr.serial, Some(12345));
    assert_eq!(pm.power_w, Some(180));
    assert_eq!(pm.cadence.map(|c| c.round()), Some(85.0));
    assert!(
        (sc.speed_kmh.unwrap() - 25.0).abs() <= 0.5,
        "{:?}",
        sc.speed_kmh
    );
    assert!(
        (sc.cadence.unwrap() - 85.0).abs() <= 1.0,
        "{:?}",
        sc.cadence
    );
    let p = 8070.0 / 32768.0;
    assert!(
        (hr.period_s.unwrap() - p).abs() <= 0.01 * p,
        "{:?}",
        hr.period_s
    );
    let sent =
        (secs / p) as u64 + (secs / (8182.0 / 32768.0)) as u64 + (secs / (8086.0 / 32768.0)) as u64;
    let good = field(&r, "frames_ok");
    assert!(good >= 0.98 * sent as f64 - 3.0, "{good} of {sent}");
    assert!(hr.history.iter().all(|v| (60.0..=99.5).contains(&v[1])));
    let rr = hr.rr_ms.unwrap();
    assert!((300.0..1100.0).contains(&rr));
    // The offset is the median of the sensors' decision levels.
    let off = r.results().status().offset_hz.unwrap();
    assert!((off - 30e3).abs() < 15e3, "{off}");
}

#[test]
fn bad_crc_every_tenth_frame() {
    let h = noisy(20.0);
    let x = signal(
        &h,
        GenParams {
            sensors: vec![sensor(99, ant::ANT_PLUS, 10)],
            seed: 4,
        },
        10.0,
    );
    let r = feed(&h, &x);
    assert!(field(&r, "frames_bad") >= 3.0, "{:?}", r.results().fields());
    let s = sensors(&r).sensors.iter().next().unwrap().record;
    assert!(s.history.iter().all(|v| (59.0..=100.0).contains(&v[1])));
}

#[test]
fn other_networks() {
    let h = noisy(20.0);
    let p = || GenParams {
        sensors: vec![sensor(49, 0x5B25, 0)],
        seed: 4,
    };
    let r = feed(&h, &signal(&h, p(), 3.0));
    assert!(sensors(&r)
        .sensors
        .iter()
        .any(|s| s.record.device == 49 && s.record.marker == 0x5B25));
    let other = AntSetting::SETTINGS[AntSetting::OtherNetworks.id().index()].key();
    let off = h.clone().with_settings(&[CaseSetting {
        key: other,
        value: Value::Bool(false),
    }]);
    let r = feed(&off, &signal(&off, p(), 3.0));
    assert!(sensors(&r).sensors.is_empty());
}

#[test]
fn no_sensor_from_noise() {
    let h = noisy(10.0)
        .set(&ant::GEN_SENSORS, ant::GenSensors::None)
        .seconds(20.0);
    let r = feed(&h, &h.generate().unwrap());
    assert!(sensors(&r).sensors.is_empty());
}

#[test]
fn saved_state_comes_back() {
    let h = noisy(25.0)
        .set(&ant::GEN_SENSORS, ant::GenSensors::Strap)
        .seconds(3.0);
    let r = feed(&h, &h.generate().unwrap());
    let state = r.save_state().expect("a saved state");
    let mut back = ant::receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    back.configure(&InputInfo::new(4e6, 2457e6, 2457e6).recording())
        .unwrap();
    back.restore_state(&state).unwrap();
    assert_eq!(sensors(&back).sensors.len(), 1);
    crate::util::clear(&mut back);
    assert!(sensors(&back).sensors.is_empty());
}

/// A value of the wrong kind or out of range is refused.
#[test]
fn a_bad_setting_value_is_refused() {
    use crate::util::refused;
    assert!(refused(
        &Sensors::default(),
        "wheel_mm",
        Value::Text("big".into())
    ));
    let d = DecoderComponent::new(Demod::default());
    assert!(refused(&d, "other_networks", Value::Text("maybe".into())));
    let mut r = ant::receiver(&StackConfig::new(Harness::new(&ANT).values().unwrap())).unwrap();
    assert!(crate::util::set(&mut r, "wheel_mm", Value::F64(9999.0)).is_err());
}

/// The events of new sensors carry the frequency of the channel.
#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let run = Harness::new(&ANT).seconds(3.0).run().unwrap();
    let decl = sdrkit_stack_ism::typed::shape("ant").results_decl();
    assert!(run.events.len() >= 3, "{:?}", run.events);
    assert!(crate::util::declared(&decl, &run.events));
    for e in &run.events {
        assert!(
            (e.freq_hz - 2457e6).abs() < 50e3,
            "{} at {} Hz",
            e.kind(),
            e.freq_hz
        );
    }
}

#[test]
fn the_rate_below_4_msps_is_refused() {
    let mut r = ant::receiver(&StackConfig::new(Harness::new(&ANT).values().unwrap())).unwrap();
    assert!(r.configure(&InputInfo::new(2e6, 2457e6, 2457e6)).is_err());
    assert_eq!(
        r.results().status().summary,
        "ANT+ sensors needs a sample rate of at least 4 Msps"
    );
}

/// The demodulator benchmark runs (`ism.ant_demod`; the pending candidates are a queue, not a `Vec` with `remove(0)`).
#[test]
fn the_demodulator_benchmark_runs() {
    let b = ant::benchmark();
    assert_eq!(b.name, "ism.ant_demod");
    let r = (b.run)(0.01);
    assert!(r.items_per_second > 0.0);
}

/// A change of frequency keeps the sensors, the frame log and the frame count, and drops the deviation of the present
/// signal.
#[test]
fn a_change_of_frequency_keeps_the_sensors_and_drops_the_deviation() {
    let h = Harness::new(&ANT).seconds(3.0);
    let mut r = feed(&h, &h.generate().unwrap());
    let sensors = crate::util::table(r.results(), "sensors").len();
    let frames = field(&r, "frames_ok");
    assert!(sensors >= 1 && frames > 0.0);
    assert!(crate::util::has(r.results(), "deviation"));
    r.forget();
    assert_eq!(crate::util::table(r.results(), "sensors").len(), sensors);
    assert_eq!(field(&r, "frames_ok"), frames);
    assert!(!crate::util::has(r.results(), "deviation"));
    assert_eq!(r.results().status().snr_db, None);
}
