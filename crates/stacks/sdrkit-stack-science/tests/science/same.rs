//! NOAA SAME through the receiver: the demo scene, the vote over damaged repeats, the events and the county table.

use sci::same::{GenMessage, MessageKind, SameGenerator};
use sci::typed::{
    SameAlertEvent, SameAlertLocationRow, SameAlertRow, SameFields, SAME_ALERT, SAME_ALERTS,
    SAME_ALERT_LOCATIONS, SAME_END, SAME_TONES,
};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::{Complex32, Generator};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

#[test]
fn the_demo_scene_returns_both_messages() {
    let r = Harness::new(&sci::SAME)
        .rate(250e3)
        .seconds(30.0)
        .set(&sdrkit_stack_science::same::GEN_LOOP, false)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .run()
        .unwrap();
    let ok = r.results.fields().at(&SameFields::MESSAGES_OK).unwrap();
    assert!(ok >= 2, "{}", r.results.status().summary);
    let snr = r.results.fields().get(&keys::SNR_DB).unwrap();
    assert!(snr > 10.0, "SNR {snr} dB");
    let alerts: Vec<_> = r.events.iter().filter(|e| e.is(&SAME_ALERT)).collect();
    assert_eq!(alerts.len(), 2);
    for e in &r.events {
        assert!(e.is(&SAME_ALERT) || e.is(&SAME_END), "{}", e.kind());
        assert!(e.freq_hz != 0.0);
        assert_eq!(e.get(&keys::FAMILY), Some(Family::Alert));
    }
    assert_eq!(alerts[0].at(&SameAlertEvent::EVENT), Some("TOR"));
    assert_eq!(alerts[0].get(&keys::URGENCY), Some(Urgency::Emergency));
    assert_eq!(alerts[0].get(&keys::SOURCE), Some("KHGX/NWS"));
}

#[test]
fn the_vote_mends_damaged_repeats() {
    let m = GenMessage {
        flips: Some((1, 3)),
        eom: false,
        ..GenMessage::new(sci::same::TORNADO)
    };
    let mut g = SameGenerator::new(64e3, &[m], 7, false);
    let mut x = vec![Complex32::new(0.0, 0.0); (g.duration() * 64e3) as usize + 3 * 64_000];
    g.generate(&mut x);
    let r = Harness::new(&sci::SAME)
        .rate(64e3)
        .freq(162.55e6)
        .feed(&x)
        .unwrap();
    assert_eq!(
        r.results.fields().at(&SameFields::MESSAGES_OK),
        Some(1),
        "{}",
        r.results.status().summary
    );
    assert_eq!(r.results.fields().at(&SameFields::MESSAGES_BAD), Some(0));
}

#[test]
fn a_county_table_names_the_locations() {
    let r = Harness::new(&sci::SAME)
        .rate(64e3)
        .seconds(20.0)
        .set(
            &sdrkit_stack_science::same::GEN_SCENE,
            sdrkit_stack_science::same::Scene::Tornado,
        )
        .set(&sdrkit_stack_science::same::GEN_LOOP, false)
        .set(&sdrkit_stack_science::same::COUNTY_TEXT, "48201,Harris")
        .run()
        .unwrap();
    assert_eq!(r.results.fields().at(&SameFields::COUNTIES), Some(1));
    let e = r.events.iter().find(|e| e.is(&SAME_ALERT)).unwrap();
    let text = e.get(&keys::TEXT).unwrap();
    assert!(text.contains("Harris"), "{text}");
    let id = r.results.decl().table_id(SAME_ALERT_LOCATIONS.key).unwrap();
    let places = r.results.table(id).unwrap();
    assert!(places
        .rows_newest_first()
        .any(|(_, row)| row.at(&SameAlertLocationRow::PLACE).is_some_and(|p| p.contains("Harris"))));
    assert!(!r.results.points().is_empty());
    points_show_rows(&r.results, SAME_ALERT_LOCATIONS.key);
}

/// Asserts that every map point of `r` shows a row of table `table` that is there.
fn points_show_rows(r: &sdrkit_core::Results, table: &str) {
    let want = r.decl().table_id(table);
    for p in r.points().values() {
        let link = p.row.as_ref().expect("a row");
        assert_eq!(Some(link.table), want, "{}", p.id);
        let t = r.table(link.table).expect("the table");
        assert!(t.row(&link.key).is_some(), "{}: no row {}", p.id, link.key);
    }
}

#[test]
fn no_message_returns_nothing() {
    let r = Harness::new(&sci::SAME)
        .rate(64e3)
        .seconds(10.0)
        .set(
            &sdrkit_stack_science::same::GEN_SCENE,
            sdrkit_stack_science::same::Scene::Noise,
        )
        .run()
        .unwrap();
    assert_eq!(r.results.fields().at(&SameFields::MESSAGES_OK), Some(0));
    assert!(r.events.is_empty());
}

/// The messages (alerts and ends) of the two-alert scene with the carrier at `offset_hz` and noise at `cn_db` carrier to noise in
/// 25 kHz, the noise drawn from `seed`: those heard with the text sent, and those taken as whole with another text.
fn messages_heard(offset_hz: f64, cn_db: Option<f64>, seed: u64) -> (usize, usize) {
    let rate = 64e3;
    let msgs = [
        GenMessage {
            tone_s: 1.5,
            ..GenMessage::new(sci::same::TORNADO)
        },
        GenMessage::new(sci::same::WEEKLY),
    ];
    let mut g = SameGenerator::new(rate, &msgs, 7, false);
    let mut x = vec![Complex32::new(0.0, 0.0); ((g.duration() + 2.0) * rate) as usize];
    g.generate(&mut x);
    for (n, v) in x.iter_mut().enumerate() {
        *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * offset_hz * n as f64 / rate) as f32);
    }
    if let Some(cn) = cn_db {
        let n0 = 10f64.powf(-cn / 10.0) / 25e3;
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, seed).unwrap();
        awgn.process(&mut x);
    }
    let r = Harness::new(&sci::SAME)
        .rate(rate)
        .freq(162.55e6)
        .feed(&x)
        .unwrap();
    let id = r.results.decl().table_id(SAME_ALERTS.key).unwrap();
    let sent = [sci::same::TORNADO, sci::same::WEEKLY, "NNNN"];
    let (mut right, mut wrong) = (0, 0);
    for (_, row) in r.results.table(id).unwrap().rows_newest_first() {
        if row.at(&SameAlertRow::KIND) == Some(MessageKind::Damaged) {
            continue;
        }
        if row.at(&SameAlertRow::RAW).is_some_and(|t| sent.contains(&t)) {
            right += 1;
        } else {
            wrong += 1;
        }
    }
    (right, wrong)
}

/// Noise seeds for each point.
const SEEDS: u64 = 16;

/// The mean of the messages heard with the text sent (of 4) over [`SEEDS`] noise seeds, and the number taken as whole with
/// another text.
fn mean_heard(offset_hz: f64, cn_db: f64) -> (f64, usize) {
    let all: Vec<(usize, usize)> = (0..SEEDS).map(|s| messages_heard(offset_hz, Some(cn_db), 40 + s)).collect();
    let right = all.iter().map(|a| a.0).sum::<usize>() as f64 / SEEDS as f64;
    (right, all.iter().map(|a| a.1).sum())
}

/// The row of the specification: the messages heard with the text sent (of 4: two alerts, two ends) at each carrier to noise
/// ratio in 25 kHz, centred and with +3 and -6 kHz of carrier error; no text other than the one sent is taken as whole, and
/// no message is counted twice.
#[test]
fn the_reader_reaches_its_sensitivity() {
    assert_eq!(messages_heard(0.0, None, 0), (4, 0));
    let levels = [6.0, 3.0, 2.0, 1.0, 0.0, -1.0];
    let rows = [
        (0.0, [4.0, 4.0, 4.0, 2.9, 2.25, 0.9]),
        (3e3, [4.0, 4.0, 4.0, 3.4, 1.6, 0.0]),
        (-6e3, [4.0, 4.0, 4.0, 3.25, 2.1, 0.0]),
    ];
    let failed: Vec<String> = std::thread::scope(|s| {
        let runs: Vec<_> = rows
            .iter()
            .map(|&(offset, required)| {
                s.spawn(move || {
                    let mut failed = Vec::new();
                    for (&cn, &need) in levels.iter().zip(&required) {
                        let (right, wrong) = mean_heard(offset, cn);
                        if right < need || right > 4.0 || wrong > 0 {
                            failed.push(format!("{offset} Hz {cn} dB: {right} of {need}, {wrong} wrong"));
                        }
                    }
                    failed
                })
            })
            .collect();
        runs.into_iter().flat_map(|r| r.join().unwrap()).collect()
    });
    assert!(failed.is_empty(), "{failed:?}");
}

/// Noise alone through the narrow channel: no message and no tone.
#[test]
fn noise_alone_gives_no_same_message_or_tone() {
    for seed in 0..4 {
        let rate = 64e3;
        let mut x = vec![Complex32::new(0.0, 0.0); (20.0 * rate) as usize];
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(1.0, 70 + seed).unwrap();
        awgn.process(&mut x);
        let r = Harness::new(&sci::SAME).rate(rate).freq(162.55e6).feed(&x).unwrap();
        assert_eq!(r.results.fields().at(&SameFields::MESSAGES_OK), Some(0), "seed {seed}");
        assert_eq!(r.results.fields().at(&SameFields::MESSAGES_BAD), Some(0), "seed {seed}");
        let id = r.results.decl().table_id(SAME_TONES.key).unwrap();
        assert_eq!(r.results.table(id).unwrap().len(), 0, "seed {seed}");
        assert!(r.events.is_empty(), "seed {seed}");
    }
}
