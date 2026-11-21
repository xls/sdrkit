//! Acceptance tests of the wireless microphone finder: wide typed::WmicCandidateRow::FM carriers and a narrow AM carrier, the band names of a plan
//! file, the test signal, the pick that plays, and the rules of every stack.

use sdrkit_core::{InputInfo, LockState, MemStorage, Receiver, Storage, StorePath, Value};
use sdrkit_stack_voice::typed;
use sdrkit_stack_voice::scene::{self, Audio, Modulation, SceneConfig, Transmission};
use sdrkit_stack_voice::wmic;
use sdrkit_stack_voice::WMIC;
use std::sync::Arc;

mod common;
use common::{feed, feed_on, generated_of, plan_text, receiver_of, samples, stored};

const CENTRE: f64 = 590e6;
const EU_BAND: &str = "UHF 470 - 694 MHz";

/// A storage that holds the two band files of the crate.
fn storage() -> Arc<MemStorage> {
    let s = MemStorage::new();
    for file in ["microphones_eu", "microphones_us"] {
        s.write(
            &StorePath::new(&stored(file)).unwrap(),
            plan_text(file).as_bytes(),
        )
        .unwrap();
    }
    Arc::new(s)
}

/// The receiver with the band file `file` loaded (none when empty) and the settings `set`.
fn receiver(file: &str, set: &[(&str, Value)]) -> Box<dyn Receiver> {
    let plan = (!file.is_empty()).then(|| ("plan", Value::Text(stored(file))));
    let set: Vec<(&str, Value)> = plan.into_iter().chain(set.iter().cloned()).collect();
    receiver_of(&WMIC, None, &set, storage())
}

/// One row of the `candidates` table.
#[derive(Clone, Debug)]
struct Row {
    id: i64,
    hz: f64,
    width_hz: f64,
    deviation_hz: Option<f64>,
    fm: bool,
    listening: bool,
    band: String,
}

/// The candidates of `rx`, in the order the table has them: the row written first comes first.
fn candidates(rx: &dyn Receiver) -> Vec<Row> {
    let r = rx.results();
    let table = r
        .decl()
        .table_id("candidates")
        .and_then(|t| r.table(t))
        .unwrap();
    let mut rows: Vec<Row> = table
        .rows_newest_first()
        .map(|(_, rec)| Row {
            id: rec.at(&typed::WmicCandidateRow::ID).unwrap(),
            hz: rec.at(&typed::WmicCandidateRow::HZ).unwrap(),
            width_hz: rec.at(&typed::WmicCandidateRow::WIDTH_HZ).unwrap(),
            deviation_hz: rec.at(&typed::WmicCandidateRow::DEVIATION_HZ),
            fm: rec.at(&typed::WmicCandidateRow::FM).unwrap(),
            listening: rec.at(&typed::WmicCandidateRow::LISTENING).unwrap(),
            band: rec.at(&typed::WmicCandidateRow::BAND).unwrap().to_owned(),
        })
        .collect();
    rows.reverse();
    rows
}

#[test]
fn the_table_is_in_order_of_frequency() {
    // The carrier on the lowest frequency starts last and gets the highest id.
    let mut cfg = four_carriers(2.4);
    cfg.tx[0].start_s = 1.0;
    cfg.tx[0].dur_s = 1.4;
    let mut rx = receiver("", &[]);
    feed(rx.as_mut(), &samples(&cfg), 4e6, CENTRE, 65_536);
    let rows = candidates(rx.as_ref());
    assert_eq!(rows.len(), 4, "{rows:#?}");
    assert!(rows.windows(2).all(|w| w[0].hz < w[1].hz), "{rows:#?}");
    assert!((rows[0].hz - 589.4e6).abs() < 30e3);
    assert_eq!(rows[0].id, rows.iter().map(|r| r.id).max().unwrap());
}

/// The candidate within 30 kHz of `hz`.
fn near(rows: &[Row], hz: f64) -> Option<&Row> {
    rows.iter().find(|r| (r.hz - hz).abs() < 30e3)
}

fn wide(hz: f64, deviation: f64, audio: Audio, length_s: f64) -> Transmission {
    Transmission {
        modulation: Modulation::WideFm,
        amplitude: 0.1,
        deviation,
        audio,
        ..Transmission::on(hz).at(0.0, length_s)
    }
}

/// Three wide typed::WmicCandidateRow::FM carriers and a narrow AM carrier at 4 Msps around 590 MHz.
fn four_carriers(length_s: f64) -> SceneConfig {
    let tone = || Audio::Tones(vec![1000.0]);
    SceneConfig {
        rate_hz: 4e6,
        centre_hz: CENTRE,
        length_s,
        noise_rms: 0.01,
        tx: vec![
            wide(589.4e6, 1.0, tone(), length_s),
            wide(590.85e6, 0.8, Audio::Speech, length_s),
            wide(591.25e6, 0.5, tone(), length_s),
            Transmission {
                modulation: Modulation::Am,
                amplitude: 0.1,
                ..Transmission::on(591.7e6).at(0.0, length_s)
            },
        ],
        seed: 7,
    }
}

#[test]
fn three_wide_fm_carriers_and_a_narrow_am_carrier() {
    let cfg = four_carriers(3.0);
    let mut rx = receiver("microphones_eu", &[("carriers", Value::I64(8))]);
    let h = feed(rx.as_mut(), &samples(&cfg), 4e6, CENTRE, 65_536);
    let rows = candidates(rx.as_ref());
    assert_eq!(rows.len(), 4, "{rows:#?}");
    for hz in [589.4e6, 590.85e6, 591.25e6] {
        let r = near(&rows, hz).unwrap_or_else(|| panic!("{hz}: {rows:#?}"));
        assert!(r.width_hz > 15e3, "{hz}: width {}", r.width_hz);
        assert!(r.fm, "{hz}: {r:?}");
    }
    let am = near(&rows, 591.7e6).unwrap_or_else(|| panic!("{rows:#?}"));
    assert!(!am.fm, "{am:?}");
    assert!(rows.iter().all(|r| r.band == EU_BAND), "{rows:#?}");
    let strong = near(&rows, 589.4e6).unwrap().deviation_hz.unwrap();
    let weak = near(&rows, 591.25e6).unwrap().deviation_hz.unwrap();
    assert!(weak > 5e3, "{weak}");
    assert!(strong > 1.4 * weak, "{strong} and {weak}");
    // 24 kHz and 12 kHz of peak deviation were sent.
    assert!((strong - 24e3).abs() < 2e3, "{strong}");
    assert!((weak - 12e3).abs() < 2e3, "{weak}");
    // One event when a candidate is first reported, one when it first looks like typed::WmicCandidateRow::FM.
    let events: Vec<_> = h
        .events
        .iter()
        .filter(|e| e.is(&typed::WMIC_CANDIDATE))
        .collect();
    let fm = |want: bool| {
        events
            .iter()
            .filter(|e| e.fields.at(&typed::WmicCandidateRow::FM) == Some(want))
            .count()
    };
    assert_eq!((fm(false), fm(true)), (4, 3));
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert_eq!(
        rx.results().status().summary,
        "Wireless mic finder: 4 candidates"
    );
    // Nothing plays without a pick.
    assert!(h.audio.is_empty());
}

#[test]
fn the_band_names_come_from_the_plan_file() {
    let cfg = four_carriers(1.2);
    let x = samples(&cfg);
    // The same carriers are in the first band of the other file.
    let mut rx = receiver("microphones_us", &[]);
    feed(rx.as_mut(), &x, 4e6, CENTRE, 65_536);
    let rows = candidates(rx.as_ref());
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|r| r.band == "UHF 470 - 608 MHz"),
        "{rows:#?}"
    );
    // Outside every band of the plan the band is empty.
    let mut rx = receiver("microphones_eu", &[]);
    feed(rx.as_mut(), &x, 4e6, 800e6, 65_536);
    let rows = candidates(rx.as_ref());
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r.band.is_empty()), "{rows:#?}");
    // Without a plan the band is empty and the status says so.
    let mut rx = receiver("", &[]);
    feed(rx.as_mut(), &x, 4e6, CENTRE, 65_536);
    let rows = candidates(rx.as_ref());
    assert!(rows.iter().all(|r| r.band.is_empty()), "{rows:#?}");
    assert_eq!(
        rx.results().status().summary,
        format!(
            "Wireless mic finder: {} candidates; no channel plan loaded",
            rows.len()
        )
    );
    assert!(rx.schema().id("region").is_err());
}

/// The offsets of the three carriers of the test signal from the centre, Hz.
const DEMO: [f64; 3] = [-0.6e6, 0.25e6, 0.85e6];

#[test]
fn the_test_signal_gives_three_fm_candidates_at_default_settings() {
    let x = generated_of(&WMIC, 4e6, 3.0, 1);
    let mut rx = receiver("", &[]);
    feed(rx.as_mut(), &x, 4e6, common::dial("wmic", None), 65_536);
    let rows = candidates(rx.as_ref());
    assert_eq!(rows.len(), 3, "{rows:#?}");
    for off in DEMO {
        let r = near(&rows, common::dial("wmic", None) + off).unwrap_or_else(|| panic!("{rows:#?}"));
        assert!(r.fm, "{r:?}");
    }
}

/// The carriers of the test signal that `rx` lists after `seconds` of the scene `cfg` at 10 Msps, and the row count.
fn swept(cfg: &SceneConfig, seconds: f64) -> (usize, usize) {
    let mut cfg = cfg.clone();
    cfg.length_s = seconds;
    for t in &mut cfg.tx {
        t.dur_s = seconds;
    }
    let mut rx = receiver("", &[]);
    feed(rx.as_mut(), &samples(&cfg), 10e6, CENTRE, 65_536);
    let rows = candidates(rx.as_ref());
    let found = DEMO
        .iter()
        .filter(|off| near(&rows, CENTRE + **off).is_some())
        .count();
    (found, rows.len())
}

#[test]
fn sweep_of_carrier_and_clock_errors_at_10_msps() {
    for (offset, ppm) in [(2000.0, 50.0), (-2000.0, -50.0)] {
        let mut cfg = scene::wide_demo(10e6, 30.0, 3);
        for t in &mut cfg.tx {
            t.offset_hz = offset;
        }
        // The radio's clock is off: it samples at another rate than it states.
        cfg.rate_hz = 10e6 * (1.0 + ppm * 1e-6);
        assert_eq!(swept(&cfg, 1.2), (3, 3), "{offset:+} Hz, {ppm:+} ppm");
    }
}

#[test]
fn sweep_of_noise_twelve_db_below_the_signal_at_10_msps() {
    // Noise 12 dB below a carrier in the 160 kHz of a candidate's channel.
    let mut cfg = scene::wide_demo(10e6, 30.0, 3);
    let power = f64::from(scene::WIDE_AMPLITUDE * scene::WIDE_AMPLITUDE);
    cfg.noise_rms = (power / 10f64.powf(1.2) * 10e6 / wmic::CHANNEL_WIDTH_HZ / 2.0).sqrt();
    assert_eq!(swept(&cfg, 1.5), (3, 3));
}

#[test]
fn noise_alone_gives_no_candidate() {
    for (rate, rms) in [(4e6, 0.05), (10e6, 0.085)] {
        let cfg = SceneConfig {
            rate_hz: rate,
            centre_hz: CENTRE,
            length_s: 20e6 / rate,
            noise_rms: rms,
            tx: Vec::new(),
            seed: 11,
        };
        let mut rx = receiver("microphones_eu", &[]);
        let h = feed(rx.as_mut(), &samples(&cfg), rate, CENTRE, 65_536);
        assert!(candidates(rx.as_ref()).is_empty(), "{rate}");
        assert!(!h.events.iter().any(|e| e.is(&typed::WMIC_CANDIDATE)));
        assert_eq!(rx.results().status().lock_state, LockState::Searching);
    }
}

#[test]
fn a_rate_below_the_lowest_is_refused_with_a_message() {
    let mut rx = receiver("", &[]);
    let at = |rx: &mut dyn Receiver, rate: f64| {
        rx.configure(&InputInfo::new(rate, CENTRE, CENTRE).recording())
    };
    assert!(at(rx.as_mut(), 0.9e6).is_err());
    assert_eq!(
        rx.results().status().summary,
        "Wireless mic finder: the sample rate is too low (1 MHz at least)"
    );
    assert!(at(rx.as_mut(), 1e6).is_ok());
}

#[test]
fn each_setting_refuses_a_value_outside_its_range() {
    let mut rx = receiver("", &[]);
    let mut set = |key: &str, v: Value| {
        let id = rx.schema().id(key).unwrap();
        sdrkit_core::owner::apply(rx.as_mut(), id, &v)
    };
    for (key, bad, good) in [
        ("squelch", Value::F64(101.0), Value::F64(100.0)),
        ("squelch", Value::F64(-1.0), Value::F64(0.0)),
        ("carriers", Value::I64(0), Value::I64(1)),
        ("carriers", Value::I64(25), Value::I64(24)),
        ("listen", Value::I64(-1), Value::I64(0)),
        ("raster_step_hz", Value::Hz(-1.0), Value::Hz(100e3)),
        ("raster_step_hz", Value::Hz(250e3), Value::Hz(200e3)),
        ("raster_origin_hz", Value::Hz(-1.0), Value::Hz(470e6)),
        ("gen_cnr_db", Value::F64(-1.0), Value::F64(0.0)),
        // A text that is not a path of the storage.
        (
            "plan",
            Value::Text("/plans/microphones_eu.json".into()),
            Value::Text(stored("microphones_eu")),
        ),
    ] {
        assert!(set(key, bad.clone()).is_err(), "{key} takes {bad:?}");
        assert!(set(key, good.clone()).is_ok(), "{key} refuses {good:?}");
    }
}

#[test]
fn clear_empties_the_list() {
    let cfg = four_carriers(1.2);
    let mut rx = receiver("microphones_eu", &[]);
    feed(rx.as_mut(), &samples(&cfg), 4e6, CENTRE, 65_536);
    assert_eq!(candidates(rx.as_ref()).len(), 4);
    let id = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    assert!(candidates(rx.as_ref()).is_empty());
    assert_eq!(rx.results().fields().at(&typed::WmicFields::CANDIDATES), Some(0));
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

#[test]
fn the_picked_candidate_plays_and_a_second_pick_stops_it() {
    let cfg = four_carriers(3.0);
    let x = samples(&cfg);
    let mut rx = receiver("microphones_eu", &[]);
    let third = x.len() / 3;
    let h = feed(rx.as_mut(), &x[..third], 4e6, CENTRE, 65_536);
    assert!(h.audio.is_empty());
    let rows = candidates(rx.as_ref());
    let pick = near(&rows, 591.25e6).unwrap().id;
    // The setting picks the candidate.
    let listen = rx.schema().id("listen").unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), listen, &Value::I64(pick)).unwrap();
    let h = feed_on(rx.as_mut(), &x[third..2 * third], 65_536);
    let rows = candidates(rx.as_ref());
    assert!(near(&rows, 591.25e6).unwrap().listening);
    assert_eq!(rows.iter().filter(|r| r.listening).count(), 1);
    // 1 s of audio at 48 kHz: the 1000 Hz tone at 12 kHz of deviation is 12 / 75 x 3 of full scale at its peak.
    assert!(
        (h.audio.len() as f64 - 48e3).abs() < 2e3,
        "{}",
        h.audio.len()
    );
    let level = rms(&h.audio[4800..]);
    assert!((level - 0.48 / 2f64.sqrt()).abs() < 0.05, "{level}");
    // The command with the id of the candidate that plays stops it.
    typed::WMIC_LISTEN
        .call(rx.as_mut(), &typed::ListenArgs { id: pick })
        .unwrap();
    let h = feed_on(rx.as_mut(), &x[2 * third..], 65_536);
    assert!(h.audio.is_empty());
    assert!(candidates(rx.as_ref()).iter().all(|r| !r.listening));
}
