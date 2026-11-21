//! mioty: every pattern through the receiver at offsets, extended frames, two frames at once, noise, the test signal, noise
//! alone, the refused rate and the data file.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    Complex32, Generator, InputInfo, Owner, Receiver, SettingSet, StackConfig, Values,
};
use sdrkit_stack_ism::mioty::code::{PATTERNS, SYMBOL_RATE};
use sdrkit_stack_ism::mioty::gen::{frame, test_mpdu, MiotyGenerator};
use sdrkit_stack_ism::mioty::{self, MiotySetting, TEST_DIAL_HZ, WORK_RATE_HZ};
use sdrkit_stack_ism::typed::{
    MiotyDeviceRow, MiotyFields, MiotyFrameRow, MIOTY_DEVICES, MIOTY_FRAMES,
};

fn receiver(rate: f64) -> Box<dyn Receiver> {
    let mut rx =
        mioty::receiver(&StackConfig::new(Values::defaults(&MiotySetting::schema()))).unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording())
        .unwrap();
    Box::new(rx)
}

/// Adds frame `mpdu` of `pattern` to `x` at `start_s` with carrier 0 at `f0` Hz from the centre.
fn add(x: &mut [Complex32], rate: f64, mpdu: &[u8], pattern: usize, start_s: f64, f0: f64) {
    let lead = 18.5 / SYMBOL_RATE;
    for (dt, c, s) in frame(mpdu, pattern, rate) {
        let at = ((start_s + dt - lead) * rate).round() as usize;
        let f = f0 + f64::from(c) * SYMBOL_RATE;
        for (i, v) in s.iter().enumerate() {
            let ph = (std::f64::consts::TAU * f * (at + i) as f64 / rate) as f32;
            if let Some(o) = x.get_mut(at + i) {
                *o += v * Complex32::new(ph.cos(), ph.sin());
            }
        }
    }
}

fn noise(x: &mut [Complex32], sigma: f32, seed: u64) {
    let mut r = Rng::new(seed);
    for v in x {
        *v += Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32());
    }
}

/// Feeds `x` and returns the frames: (id, pattern, psi, frequency).
fn frames(rate: f64, x: &[Complex32]) -> Vec<(String, String, i64, f64)> {
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    for p in x.chunks(32_768) {
        rx.feed(p, &mut out);
    }
    rx.finish(&mut out);
    MIOTY_FRAMES
        .rows::<MiotyFrameRow>(rx.results())
        .map(|(_, r)| (r.id.to_string(), r.pattern.to_string(), r.psi, r.freq_hz.0))
        .collect()
}

#[test]
fn every_pattern_decodes_at_offsets_up_to_9_khz() {
    let rate = 250e3;
    let mut rng = Rng::new(3);
    let offsets = [-9e3, -4e3, 0.0, 5e3, 9e3];
    let mut x = vec![Complex32::default(); (rate * 24.0) as usize];
    let mut want = Vec::new();
    for (p, _) in PATTERNS.iter().enumerate() {
        let off = offsets[p % offsets.len()];
        let f0 = off - 12.0 * SYMBOL_RATE;
        let mpdu = test_mpdu(p as u32, 15, &mut rng);
        add(&mut x, rate, &mpdu, p, 0.3 + 1.1 * p as f64, f0);
        want.push((PATTERNS[p].name.to_string(), TEST_DIAL_HZ + off));
    }
    noise(&mut x, 0.02, 1);
    let got = frames(rate, &x);
    for (name, f) in &want {
        let hit = got.iter().find(|g| &g.1 == name);
        let Some(hit) = hit else {
            panic!("{name} not decoded: {got:?}");
        };
        assert!((hit.3 - f).abs() < 40.0, "{name}: {} against {f}", hit.3);
        assert_eq!(hit.0, "short 1234");
    }
    assert_eq!(got.len(), want.len());
}

#[test]
fn extended_frames_and_two_frames_at_once() {
    let rate = 250e3;
    let mut rng = Rng::new(5);
    let mut x = vec![Complex32::default(); (rate * 9.0) as usize];
    add(&mut x, rate, &test_mpdu(1, 21, &mut rng), 2, 0.2, -30e3);
    add(&mut x, rate, &test_mpdu(2, 30, &mut rng), 10, 0.25, 2e3);
    add(&mut x, rate, &test_mpdu(3, 30, &mut rng), 16, 4.5, -10e3);
    noise(&mut x, 0.02, 2);
    let mut got: Vec<i64> = frames(rate, &x).iter().map(|g| g.2).collect();
    got.sort_unstable();
    assert_eq!(got, [21, 30, 30]);
}

#[test]
fn noise_at_7_8_db_es_n0_loses_no_frame() {
    // Unit amplitude and sigma 2 per part at the working rate: Es/N0 = 10 log10(48 / 8) = 7.8 dB.
    let rate = 250e3;
    let sigma = 2.0 * (rate / WORK_RATE_HZ).sqrt() as f32;
    let mut rng = Rng::new(9);
    let mut x = vec![Complex32::default(); (rate * 8.0) as usize];
    for k in 0..4 {
        add(
            &mut x,
            rate,
            &test_mpdu(k, 15, &mut rng),
            16,
            0.3 + 1.6 * f64::from(k),
            -28e3 + 15e3 * f64::from(k),
        );
    }
    noise(&mut x, sigma, 4);
    assert_eq!(frames(rate, &x).len(), 4);
}

#[test]
fn the_test_signal_and_noise_alone() {
    let rate = 1e6;
    let mut g = MiotyGenerator::from_config(&sdrkit_core::GenConfig::new(
        rate,
        Values::defaults(&MiotySetting::schema()),
    ))
    .unwrap();
    let mut x = vec![Complex32::default(); (rate * 6.0) as usize];
    g.generate(&mut x);
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    for p in x.chunks(65_536) {
        rx.feed(p, &mut out);
    }
    let r = rx.results();
    let frames_ok = MiotyFields::read(r.fields()).unwrap().frames_ok;
    assert!(frames_ok >= 2, "{frames_ok}");
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    let mut x = vec![Complex32::default(); (250e3 * 8.0) as usize];
    noise(&mut x, 0.3, 8);
    assert!(frames(250e3, &x).is_empty());
}

#[test]
fn a_low_rate_is_refused_and_the_data_file() {
    let mut rx =
        mioty::receiver(&StackConfig::new(Values::defaults(&MiotySetting::schema()))).unwrap();
    assert!(rx
        .configure(&InputInfo::new(200e3, TEST_DIAL_HZ, TEST_DIAL_HZ))
        .is_err());
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/mioty.json");
    let text = std::fs::read_to_string(path).unwrap();
    let file = sdrkit_data_plans::data::parse("mioty.json", &text).unwrap();
    assert_eq!(file.groups[0].bands.len(), 1);
}

#[test]
fn a_radio_off_the_dial_gives_the_frame_frequency() {
    let rate = 250e3;
    let mut rng = Rng::new(11);
    let mut x = vec![Complex32::default(); (rate * 1.6) as usize];
    // The radio sits 20 kHz below the dial: the frame, centred on the dial, is at +20 kHz in the samples.
    add(
        &mut x,
        rate,
        &test_mpdu(1, 15, &mut rng),
        16,
        0.2,
        20e3 - 12.0 * SYMBOL_RATE,
    );
    noise(&mut x, 0.02, 1);
    let mut rx =
        mioty::receiver(&StackConfig::new(Values::defaults(&MiotySetting::schema()))).unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ - 20e3, TEST_DIAL_HZ).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let f = MIOTY_FRAMES
        .rows::<MiotyFrameRow>(rx.results())
        .next()
        .unwrap()
        .1
        .freq_hz
        .0;
    assert!((f - TEST_DIAL_HZ).abs() < 40.0, "{f}");
}

#[test]
fn a_change_of_frequency_keeps_the_logs_and_drops_the_measurements() {
    let rate = 250e3;
    let mut rng = Rng::new(3);
    let mut x = vec![Complex32::default(); (rate * 6.0) as usize];
    add(&mut x, rate, &test_mpdu(0, 15, &mut rng), 0, 0.3, 0.0);
    noise(&mut x, 0.02, 1);
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let (frames_ok, devices) = {
        let f = MiotyFields::read(rx.results().fields()).unwrap();
        assert!(f.frames_ok > 0 && f.snr_db.is_some() && f.level_dbfs.is_some());
        (f.frames_ok, f.devices)
    };
    rx.forget();
    let f = MiotyFields::read(rx.results().fields()).unwrap();
    assert_eq!((f.frames_ok, f.devices), (frames_ok, devices));
    assert_eq!((f.snr_db, f.level_dbfs), (None, None));
    assert!(MIOTY_FRAMES.rows::<MiotyFrameRow>(rx.results()).count() > 0);
    assert!(MIOTY_DEVICES.rows::<MiotyDeviceRow>(rx.results()).count() > 0);
}
