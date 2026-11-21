//! New Packet Radio: every rate, network and offset found once, the offset measured, noise, noise alone, the rates setting and
//! the data file.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, InputInfo, Receiver, SettingSet, StackConfig, Value, Values};
use sdrkit_stack_ism::npr70::{
    self, frame, parse_rates, rates_searched, sync_levels, NprSetting, RATES, TEST_DIAL_HZ,
};
use sdrkit_stack_ism::typed::{NprFields, NprFrameRow, NprNetworkRow, NPR_FRAMES, NPR_NETWORKS};

fn receiver(rate: f64) -> Box<dyn Receiver> {
    let mut rx =
        npr70::receiver(&StackConfig::new(Values::defaults(&NprSetting::schema()))).unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording())
        .unwrap();
    Box::new(rx)
}

fn noise(x: &mut [Complex32], sigma: f32, seed: u64) {
    let mut r = Rng::new(seed);
    for v in x {
        *v += Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32());
    }
}

/// One frame at `symbol_rate` of `network`, moved by `offset_hz`, in `seconds` at `rate`.
fn one(rate: f64, symbol_rate: f64, network: usize, offset_hz: f64, seed: u64) -> Vec<Complex32> {
    let mut rng = Rng::new(seed);
    let f = frame(rate, symbol_rate, network, &mut rng).unwrap();
    let mut x = vec![Complex32::default(); 2000];
    x.extend(f);
    x.extend(vec![Complex32::default(); 2000]);
    for (i, v) in x.iter_mut().enumerate() {
        let ph = (std::f64::consts::TAU * offset_hz * i as f64 / rate) as f32;
        *v *= Complex32::from_polar(1.0, ph);
    }
    x
}

/// The (network, offset) of each frame found.
fn found(rate: f64, x: &[Complex32]) -> Vec<(String, f64)> {
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    for p in x.chunks(50_000) {
        rx.feed(p, &mut out);
    }
    rx.finish(&mut out);
    NPR_FRAMES
        .rows::<NprFrameRow>(rx.results())
        .map(|(_, r)| (r.network.to_string(), r.tuning_hz))
        .collect()
}

#[test]
fn every_rate_network_and_offset_gives_one_hit() {
    let rate = 4e6;
    for r in RATES {
        for n in [0usize, 5, 15] {
            for frac in [-0.03, 0.0, 0.04] {
                let mut x = one(rate, r, n, frac * r, (n as u64) * 7 + 1);
                noise(&mut x, 0.01, 3);
                let f = found(rate, &x);
                assert_eq!(f.len(), 1, "rate {r}, network {n}, offset {frac}: {f:?}");
                assert_eq!(f[0].0, format!("network {n}"));
                let want = TEST_DIAL_HZ + frac * r;
                assert!(
                    (f[0].1 - want).abs() < 0.05 * r,
                    "{} against {want}",
                    f[0].1
                );
            }
        }
    }
}

#[test]
fn noise_at_500_ksym_s_and_noise_alone() {
    let rate = 4e6;
    let mut hits = Vec::new();
    for sigma in [0.2f32, 0.3] {
        let mut n = 0;
        for k in 0..10 {
            let mut x = one(rate, 500e3, 3, 0.0, 100 + k);
            noise(&mut x, sigma, 200 + k);
            n += usize::from(found(rate, &x).len() == 1);
        }
        hits.push(n);
    }
    assert!(hits[0] >= 9, "{hits:?}");
    let mut x = vec![Complex32::default(); 4_000_000];
    noise(&mut x, 0.3, 9);
    assert!(found(rate, &x).is_empty());
}

#[test]
fn the_sync_words_the_rates_and_the_setting() {
    // Network 9, 45 D2 96: 01 00 01 01, 11 01 00 10, 10 01 01 10.
    assert_eq!(
        sync_levels(9),
        [-1.0, -3.0, -1.0, -1.0, 1.0, -1.0, -3.0, 3.0, 3.0, -1.0, -1.0, 3.0]
    );
    assert_eq!(rates_searched(1e6, &[]), [50e3, 100e3, 180e3]);
    assert_eq!(rates_searched(4e6, &[]).len(), 5);
    assert_eq!(parse_rates("100, 500"), Some(vec![100e3, 500e3]));
    assert_eq!(parse_rates("120"), None);
    let mut rx = receiver(4e6);
    let id = rx.schema().id("rates").unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), id, &Value::Text("500".into())).unwrap();
    assert!(sdrkit_core::owner::apply(rx.as_mut(), id, &Value::Text("7".into())).is_err());
    let mut out = sdrkit_core::Output::default();
    rx.feed(&vec![Complex32::default(); 400_000], &mut out);
    rx.finish(&mut out);
    let r = rx.results();
    assert_eq!(NprFields::read(r.fields()).unwrap().rates_searched, "500");
}

#[test]
fn a_low_rate_is_refused_and_the_data_file() {
    let mut rx =
        npr70::receiver(&StackConfig::new(Values::defaults(&NprSetting::schema()))).unwrap();
    assert!(rx
        .configure(&InputInfo::new(500e3, TEST_DIAL_HZ, TEST_DIAL_HZ))
        .is_err());
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/npr70.json");
    let text = std::fs::read_to_string(path).unwrap();
    let file = sdrkit_data_plans::data::parse("npr70.json", &text).unwrap();
    assert_eq!(file.groups[0].bands.len(), 1);
}

#[test]
fn a_change_of_frequency_keeps_the_logs_and_drops_the_level() {
    let rate = 4e6;
    let mut x = one(rate, 500e3, 3, 0.0, 5);
    noise(&mut x, 0.01, 3);
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let before = NprFields::read(rx.results().fields()).unwrap();
    assert_eq!(before.sync_words, 1);
    assert!(before.level_dbfs.is_some());
    rx.forget();
    let after = NprFields::read(rx.results().fields()).unwrap();
    assert_eq!(after.sync_words, 1);
    assert_eq!(after.level_dbfs, None);
    assert_eq!(NPR_FRAMES.rows::<NprFrameRow>(rx.results()).count(), 1);
    assert_eq!(NPR_NETWORKS.rows::<NprNetworkRow>(rx.results()).count(), 1);
}
