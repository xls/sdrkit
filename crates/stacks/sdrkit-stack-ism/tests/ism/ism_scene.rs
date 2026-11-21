//! `ism`: the generator's scenes through the standard impairments and the receiver, the saved state, the settings and the
//! events.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, InputInfo, Output, Receiver, StackConfig, StateBlob, Value, Values,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_proto_ookdev::Modulation;
use sdrkit_stack_ism::ism::devices::{Devices, Store};
use sdrkit_stack_ism::ism::gen::{self, GenParams, Slot, TestDevice};
use sdrkit_stack_ism::ism::{self, ISM};
use serde_json::json;

fn defaults() -> Values {
    sdrkit_core::Values::defaults(&(ISM.settings)())
}

fn rx(rate: f64, centre: f64) -> StackGraph {
    let mut r = ism::receiver(&StackConfig::new(defaults())).unwrap();
    r.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    r
}

/// The stack's test signal through the standard impairments: noise at `snr_db` in the declared bandwidth, and more settings
/// as typed settings.
fn scene(rate: f64, seconds: f64, snr_db: f64, more: &[CaseSetting]) -> Vec<Complex32> {
    Harness::new(&ISM)
        .rate(rate)
        .seconds(seconds)
        .seed(7)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .with_settings(more)
        .generate()
        .unwrap()
}

/// The clean signal of `slots` with noise at 27 dB SNR in the declared bandwidth.
fn slots_signal(rate: f64, slots: Vec<Slot>, seconds: f64) -> Vec<Complex32> {
    let g = gen::generator(rate, &GenParams::default(), slots).unwrap();
    let schema = sdrkit_components::sim::with_generator_settings(&(ISM.settings)()).unwrap();
    let mut v = Values::defaults(&schema);
    v.update_from_json(&json!({"gen_noise": "snr", "gen_snr_db": 27.0, "gen_seed": 3}))
        .unwrap();
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &v).unwrap();
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

fn feed(r: &mut StackGraph, x: &[Complex32], chunk: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(chunk) {
        r.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    ev
}

fn store(r: &StackGraph) -> &Store {
    r.graph()
        .component_as::<Devices>("devices")
        .expect("the devices component")
        .store()
}

fn has(s: &Store, t: TestDevice) -> bool {
    s.devices
        .iter()
        .any(|x| x.record.model == t.model() && x.record.id == t.id())
}

fn expect_all(s: &Store, devs: &[TestDevice]) {
    for &t in devs {
        let have: Vec<_> = s
            .devices
            .iter()
            .map(|x| (&x.record.model, &x.record.id))
            .collect();
        assert!(has(s, t), "{t:?} missing; have {have:?}");
    }
}

fn no_wrong(s: &Store, devs: &[TestDevice]) {
    for x in s.devices.iter() {
        let d = x.record;
        assert!(
            devs.iter().any(|t| d.model == t.model() && d.id == t.id()),
            "wrong reading {} {} {}",
            d.model,
            d.id,
            d.line
        );
    }
}

fn devices_of(scene: u32) -> Vec<TestDevice> {
    gen::scene_slots(scene).iter().map(|s| s.dev).collect()
}

#[test]
fn scene0_end_to_end_2msps() {
    let rate = 2e6;
    let x = scene(rate, 12.0, 27.0, &[]);
    let mut r = rx(rate, 433.92e6);
    feed(&mut r, &x, 65536);
    let s = store(&r);
    expect_all(s, &devices_of(0));
    no_wrong(s, &devices_of(0));
    let unknown: Vec<String> = s.unknown.iter().map(|u| u.line()).collect();
    assert_eq!(s.unknown_bursts, 0, "{unknown:?}");
    for sl in gen::scene_slots(0) {
        let d = s
            .devices
            .iter()
            .find(|x| x.record.model == sl.dev.model())
            .unwrap()
            .record;
        let tol = if gen::device_train(sl.dev, 0).modulation == Modulation::Fsk {
            40e3
        } else {
            12e3
        };
        assert!(
            (d.offset_hz - sl.offset_hz).abs() <= tol,
            "{:?} at {}",
            sl.dev,
            d.offset_hz
        );
        assert!(
            (d.freq_hz - (433.92e6 + sl.offset_hz)).abs() <= tol,
            "{:?} at {} Hz",
            sl.dev,
            d.freq_hz
        );
        // Every transmission is counted once: two in 12 s.
        assert!(
            d.count >= 1 && d.count <= 2,
            "{:?} count {}",
            sl.dev,
            d.count
        );
    }
}

#[test]
fn scenes_1_and_2_at_1msps() {
    for (sc, centre) in [(1u32, 868.3e6), (2, 868.4e6)] {
        let x = scene(
            1e6,
            5.0,
            27.0,
            &[CaseSetting::new(
                &ism::GEN_SCENE,
                if sc == 1 {
                    ism::GenScene::Sensors868
                } else {
                    ism::GenScene::Zwave
                },
            )],
        );
        let mut r = rx(1e6, centre);
        feed(&mut r, &x, 65536);
        let s = store(&r);
        let devs: Vec<TestDevice> = devices_of(sc)
            .into_iter()
            .filter(|&d| d != TestDevice::WmbusT)
            .collect();
        expect_all(s, &devs);
        no_wrong(s, &devices_of(sc));
    }
}

#[test]
fn weak_signals_never_wrong() {
    let rate = 2e6;
    let x = scene(rate, 30.0, 12.0, &[]);
    let mut r = rx(rate, 433.92e6);
    feed(&mut r, &x, 65536);
    let s = store(&r);
    no_wrong(s, &devices_of(0));
    let found = devices_of(0).iter().filter(|&&t| has(s, t)).count();
    assert!(found >= 12, "only {found} of 14 at 12 dB");
}

#[test]
fn carrier_rejection() {
    let rate = 2e6;
    let x = scene(
        rate,
        12.0,
        27.0,
        &[CaseSetting::new(&ism::GEN_CARRIER, true)],
    );
    let mut r = rx(rate, 433.92e6);
    feed(&mut r, &x, 65536);
    let s = store(&r);
    expect_all(s, &devices_of(0));
    no_wrong(s, &devices_of(0));
    for u in s.unknown.iter() {
        assert!(
            u.time_s < 1.2 || (u.offset_hz - 300e3).abs() > 20e3,
            "carrier reported as unknown at {}",
            u.time_s
        );
    }
}

#[test]
fn frequency_error_follows() {
    for err in [-20e3, 20e3] {
        let rate = 2e6;
        let x = scene(rate, 6.0, 27.0, &[CaseSetting::new(&GEN_OFFSET_HZ, err)]);
        let mut r = rx(rate, 433.92e6);
        feed(&mut r, &x, 65536);
        let s = store(&r);
        expect_all(s, &devices_of(0));
        for sl in gen::scene_slots(0) {
            let d = s
                .devices
                .iter()
                .find(|x| x.record.model == sl.dev.model())
                .unwrap()
                .record;
            assert!(
                (d.offset_hz - (sl.offset_hz + err)).abs() <= 40e3,
                "{:?}",
                sl.dev
            );
        }
    }
}

#[test]
fn truncated_burst_returns_no_reading() {
    let slots = vec![Slot {
        dev: TestDevice::Wh1080,
        offset_hz: 20e3,
        phase_s: 0.1,
        period_s: 0.5,
        level_db: 0.0,
        truncate: 0.5,
    }];
    let x = slots_signal(1e6, slots, 2.1);
    let mut r = rx(1e6, 433.92e6);
    feed(&mut r, &x, 65536);
    let s = store(&r);
    assert!(s.devices.is_empty());
    assert!(s.unknown_bursts <= 4, "{}", s.unknown_bursts);
}

#[test]
fn chunking_returns_the_same_readings() {
    let slots: Vec<Slot> = gen::scene_slots(0)
        .into_iter()
        .filter(|s| {
            matches!(
                s.dev,
                TestDevice::Ev1527 | TestDevice::Schrader | TestDevice::Toyota
            )
        })
        .map(|mut s| {
            s.phase_s -= 3.3;
            s
        })
        .collect();
    let x = slots_signal(1e6, slots, 1.2);
    let mut lines = Vec::new();
    for chunk in [1usize, 1000, 65536] {
        let mut r = rx(1e6, 433.92e6);
        feed(&mut r, &x, chunk);
        let mut v: Vec<String> = store(&r)
            .log
            .iter()
            .map(|l| format!("{} {} {} {:.3}", l.model, l.id, l.text, l.time_s))
            .collect();
        v.sort();
        assert_eq!(v.len(), 3, "{v:?}");
        lines.push(v);
    }
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[1], lines[2]);
}

#[test]
fn nan_changes_nothing_after_it() {
    let slots: Vec<Slot> = gen::scene_slots(0)
        .into_iter()
        .filter(|s| s.dev == TestDevice::Schrader)
        .map(|mut s| {
            s.phase_s = 0.5;
            s
        })
        .collect();
    let mut x = slots_signal(1e6, slots, 1.0);
    x[1000] = Complex32::new(f32::NAN, f32::INFINITY);
    let mut r = rx(1e6, 433.92e6);
    feed(&mut r, &x, 4096);
    assert_eq!(store(&r).devices.len(), 1);
}

#[test]
fn duplicates_count_once_and_lists_survive_retune_and_restart() {
    let one = |dev, phase_s, period_s| Slot {
        dev,
        offset_hz: 20e3,
        phase_s,
        period_s,
        level_db: 0.0,
        truncate: 0.0,
    };
    // Readings differ between transmissions (pressure steps), so 3 transmissions count 3 times.
    let x = slots_signal(1e6, vec![one(TestDevice::Schrader, 0.2, 0.6)], 1.8);
    let mut r = rx(1e6, 433.92e6);
    feed(&mut r, &x, 65536);
    assert_eq!(store(&r).devices.iter().next().unwrap().record.count, 3);
    // The same reading twice within 2 s counts once.
    let x = slots_signal(
        1e6,
        vec![
            one(TestDevice::Ev1527, 0.1, 10.0),
            one(TestDevice::Ev1527, 0.6, 10.0),
        ],
        1.2,
    );
    let mut r = rx(1e6, 433.92e6);
    feed(&mut r, &x, 65536);
    assert_eq!(store(&r).devices.iter().next().unwrap().record.count, 1);
    assert_eq!(store(&r).log.len(), 1);
    r.retuned(&InputInfo::new(1e6, 434e6, 434e6).recording());
    feed(&mut r, &vec![Complex32::default(); 300_000], 65536);
    assert_eq!(store(&r).devices.len(), 1, "survives a retune");
    // The saved state comes back in a new receiver, and a state of another stack is refused.
    let state = r.save_state().expect("a saved state");
    let mut back = rx(1e6, 433.92e6);
    back.restore_state(&state).unwrap();
    assert_eq!(store(&back).devices.len(), 1);
    assert_eq!(store(&back).log.len(), 1);
    let other = StateBlob::new("zwave", state.version(), state.bytes().to_vec());
    assert!(back.restore_state(&other).is_err());
    crate::util::clear(&mut r);
    assert!(store(&r).devices.is_empty() && store(&r).log.is_empty());
}

#[test]
fn rate_too_low_is_refused() {
    let mut r = ism::receiver(&StackConfig::new(defaults())).unwrap();
    assert!(r
        .configure(&InputInfo::new(500e3, 433.92e6, 433.92e6))
        .is_err());
    assert_eq!(
        r.results().status().summary,
        "ISM sensors needs a sample rate of at least 1 Msps"
    );
}

#[test]
fn descriptor_generator_runs() {
    let mut g =
        (ISM.make_generator.unwrap())(&crate::util::gen_defaults(&ISM, 2e6).with_seed(1)).unwrap();
    let mut x = vec![Complex32::default(); 4096];
    assert_eq!(g.generate(&mut x), 4096);
    assert!(g.signal_info().is_some());
}

/// A value out of range or of the wrong kind is refused.
#[test]
fn a_bad_setting_value_is_refused() {
    let mut r = ism::receiver(&StackConfig::new(defaults())).unwrap();
    assert!(crate::util::set(&mut r, "threshold", Value::F64(99.0)).is_err());
    assert!(crate::util::set(&mut r, "threshold", Value::Text("loud".into())).is_err());
    assert!(crate::util::set(&mut r, "threshold", Value::F64(10.0)).is_ok());
}

/// Every event the test signal makes is declared, and none is sent at frequency 0.
#[test]
fn every_event_is_declared_with_its_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("ism").results_decl();
    let x = scene(1e6, 6.0, 27.0, &[]);
    let mut r = rx(1e6, 433.92e6);
    let ev = feed(&mut r, &x, 65536);
    assert!(ev.len() >= 10, "{} events", ev.len());
    assert!(crate::util::declared(&decl, &ev));
    for e in &ev {
        assert!(e.freq_hz > 400e6, "{} sent at {} Hz", e.kind(), e.freq_hz);
    }
}

/// The readings of one 8 s trial of scene 0 (14 devices at 433.92 MHz) at `snr_100k` dB in 100 kHz, with `more` settings.
fn scene_readings(snr_100k: f64, seed: u64, more: &[CaseSetting]) -> f64 {
    let rate = 2e6;
    let x = Harness::new(&ISM)
        .rate(rate)
        .seconds(8.0)
        .seed(seed)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_100k - 10.0 * 2f64.log10())
        .with_settings(more)
        .generate()
        .unwrap();
    let mut r = rx(rate, 433.92e6);
    feed(&mut r, &x, 65536);
    crate::util::number(Owner::results(&r), "readings")
}

/// The trials of 20 (seeds 40 to 59) that keep at least half the clean run's readings at each level.
fn scene_passes(levels: &[f64], more: &[CaseSetting]) -> Vec<usize> {
    let clean = scene_readings(40.0, 40, more);
    levels
        .iter()
        .map(|&snr| {
            (0..20)
                .filter(|&k| 2.0 * scene_readings(snr, 40 + k, more) >= clean)
                .count()
        })
        .collect()
}

/// The narrow-envelope retry and the FSK passes (scene 0, 8 s trials, 20 trials, SNR in 100 kHz): at 12, 10 and 8 dB at least
/// 20, 20 and 19 trials keep half the clean run's readings; noise alone gives no reading in 30 trials.
#[test]
fn scene_near_the_threshold() {
    let got = scene_passes(&[12.0, 10.0, 8.0], &[]);
    println!("passes {got:?}");
    assert!(
        got.iter().zip([20, 20, 19]).all(|(&g, n)| g >= n),
        "{got:?}"
    );
    let noise: f64 = (0..30)
        .map(|k| {
            let mut rng = sdrkit_algo_sim::rng::Rng::new(500 + k);
            let x: Vec<Complex32> = (0..16_000_000).map(|_| rng.complex_normal(1e-3)).collect();
            let mut r = rx(2e6, 433.92e6);
            feed(&mut r, &x, 65536);
            crate::util::number(Owner::results(&r), "readings")
        })
        .sum();
    assert_eq!(noise, 0.0);
}

/// The row's figures at the lower levels: at 6, 5, 4, 3 and 2 dB at least 12, 14, 11, 10 and 0 trials of 20.
#[test]
fn scene_reaches_the_row() {
    let got = scene_passes(&[6.0, 5.0, 4.0, 3.0, 2.0], &[]);
    println!("passes {got:?}");
    assert!(
        got.iter().zip([12, 14, 11, 10, 0]).all(|(&g, n)| g >= n),
        "{got:?}"
    );
}

/// The row's figures with a carrier error of 20 ppm (8.7 kHz at 433.92 MHz), with a clock error of 50 ppm and 300 kHz off
/// centre: at 5 and 3 dB the trials that pass are within 2 of the row's 14 and 10.
#[test]
fn scene_with_carrier_error_clock_error_and_off_centre() {
    let cases = [
        ("20 ppm carrier", CaseSetting::new(&GEN_OFFSET_HZ, 8.7e3)),
        ("50 ppm clock", CaseSetting::new(&GEN_CLOCK_PPM, 50.0)),
        ("off centre", CaseSetting::new(&GEN_OFFSET_HZ, 300e3)),
    ];
    for (name, case) in cases {
        let got = scene_passes(&[5.0, 3.0], &[case]);
        println!("{name}: passes {got:?}");
        assert!(
            got.iter().zip([12, 8]).all(|(&g, n)| g >= n),
            "{name}: {got:?}"
        );
    }
}

/// A change of frequency keeps the devices, the logs and the counts, and drops the noise floor, the input level and the
/// signal to noise ratio of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_devices_and_drops_the_levels() {
    let x = scene(1e6, 6.0, 27.0, &[]);
    let mut r = rx(1e6, 433.92e6);
    feed(&mut r, &x, 65536);
    let devices = crate::util::table(r.results(), "devices").len();
    let log = crate::util::table(r.results(), "log").len();
    let packets = crate::util::number(r.results(), "packets_decoded");
    let readings = crate::util::number(r.results(), "readings");
    assert!(devices >= 10 && log >= 10 && packets >= 10.0 && readings >= 10.0);
    assert!(crate::util::has(r.results(), "noise_floor_dbfs"));
    assert!(crate::util::has(r.results(), "input_level_dbfs"));
    assert!(r.results().status().snr_db.is_some());
    r.forget();
    assert_eq!(crate::util::table(r.results(), "devices").len(), devices);
    assert_eq!(crate::util::table(r.results(), "log").len(), log);
    assert_eq!(crate::util::number(r.results(), "packets_decoded"), packets);
    assert_eq!(crate::util::number(r.results(), "readings"), readings);
    assert!(!crate::util::has(r.results(), "noise_floor_dbfs"));
    assert!(!crate::util::has(r.results(), "input_level_dbfs"));
    assert_eq!(r.results().status().snr_db, None);
}
