//! End of train: the data file, the test signal through the receiver over rates, carriers, clocks and noise, and the table
//! commands.

use crate::common::*;
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Owner, Receiver, StackDescriptor};
use sdrkit_dev_test::{Awgn, ClockError, FrequencyOffset};
use sdrkit_graph::StackGraph;
use sdrkit_stack_cell::eot::frame::{Direction, EotData};
use sdrkit_stack_cell::eot::gen::{example, EotGenOptions, EotGenerator};
use sdrkit_stack_cell::typed::{EotFields, EOT_FRAMES};
use sdrkit_stack_cell::EOT;

const DIAL: f64 = 457.9375e6;

/// Frames of `frames` at `rate`, the channel `channel_hz` from the centre, sent once with `tail_s` of silence after them.
fn frames(
    rate: f64,
    channel_hz: f64,
    frames: Vec<EotData>,
    gap_s: f64,
    tail_s: f64,
) -> Vec<Complex32> {
    let n = frames.len();
    let mut g = EotGenerator::new(
        rate,
        EotGenOptions {
            frames,
            channel_hz,
            gap_s,
            lead_s: 0.05,
            repeat: false,
        },
    );
    let len = ((0.05 + n as f64 * (143.0 / 1200.0 + gap_s) + tail_s) * rate) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); len];
    let got = g.generate(&mut x);
    x[got..].fill(Complex32::new(0.0, 0.0));
    x
}

/// A new receiver fed `x` at `rate` with the station `station_hz` from the centre. Returns it and its events.
fn receive(x: &[Complex32], rate: f64, station_hz: f64) -> (StackGraph, Output) {
    let mut rx = stack_graph(&EOT, &[]);
    rx.configure(&InputInfo::new(rate, DIAL - station_hz, DIAL).recording())
        .expect("configure");
    let mut all = Output::default();
    let mut out = Output::default();
    for c in x.chunks(4096) {
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        out.clear();
    }
    rx.finish(&mut out);
    all.events.append(&mut out.events);
    (rx, all)
}

fn good(rx: &StackGraph) -> f64 {
    value(rx.results(), "frames_ok").unwrap_or(0.0)
}

fn texts(rx: &StackGraph) -> Vec<String> {
    let r = rx.results();
    let id = r.decl().table_id(EOT_FRAMES.key).unwrap();
    let n = r.table(id).map_or(0, |t| t.len());
    (0..n)
        .filter_map(|i| cell_text(r, EOT_FRAMES.key, &format!("{i:06}"), "text"))
        .collect()
}

fn units(rx: &StackGraph) -> Vec<String> {
    let r = rx.results();
    (0..64)
        .filter_map(|i| cell_text(r, EOT_FRAMES.key, &format!("{i:06}"), "unit"))
        .collect()
}

#[test]
fn the_data_file_has_both_links() {
    let file = plan_file("eot.json");
    assert_eq!(file.groups.len(), 1);
    let band = &file.groups[0].bands[0];
    assert_eq!(band.channels.len(), 2);
    assert_eq!(band.channels[0].centre_hz, 457.9375e6);
    assert_eq!(band.channels[1].centre_hz, 452.9375e6);
    assert_eq!((band.start_hz, band.end_hz), (150e6, 520e6));
}

#[test]
fn the_test_signal_locks_with_the_default_settings() {
    let d: &'static StackDescriptor = &EOT;
    let rate = d.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(d).values().unwrap();
    let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 2.0) as usize];
    g.generate(&mut x);
    let (rx, out) = receive(&x, rate, -d.tuning.tune_offset_hz);
    assert!(good(&rx) >= 10.0, "{}", good(&rx));
    assert_eq!(value(rx.results(), "frames_bad"), Some(0.0));
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    let t = texts(&rx);
    assert_eq!(
        t[0],
        "brake pipe 90 psi, battery OK 78%, moving, normal, valve open, marker light on"
    );
    assert_eq!(units(&rx)[0], "12345");
    assert!(out.events.iter().any(|e| e.kind() == "eot.frame"));
    let note_id = rx.results().decl().field_id(EotFields::NOTE.key).unwrap();
    assert!(matches!(
        rx.results().fields().value(note_id),
        Some(sdrkit_core::Value::Text(t)) if t.contains("Not confirmed by a public document")
    ));
}

#[test]
fn every_pressure_type_and_motion() {
    for psi in [0u8, 1, 45, 90, 127] {
        for kind in [0u8, 7] {
            for moving in [false, true] {
                let unit = 1 + 1000 * u32::from(psi);
                let x = frames(48e3, 0.0, vec![example(unit, psi, kind, moving)], 0.05, 0.1);
                let (rx, _) = receive(&x, 48e3, 0.0);
                assert_eq!(good(&rx), 1.0, "{psi} {kind} {moving}");
                assert!(texts(&rx)[0].contains(&format!("{psi} psi")));
                assert_eq!(units(&rx)[0], format!("{unit:05}"));
            }
        }
    }
}

#[test]
fn rates_carriers_and_clocks() {
    for rate in [48e3, 250e3, 1e6, 2.4e6] {
        for carrier in [-4000.0, -1000.0, 0.0, 2500.0] {
            for ppm in [-200.0, 200.0] {
                let x = frames(rate, 0.0, vec![example(12345, 90, 0, true)], 0.05, 0.1);
                let mut y = Vec::new();
                ClockError::new(rate, ppm).process(&x, &mut y);
                FrequencyOffset::new(rate, carrier).process(&mut y);
                let (rx, _) = receive(&y, rate, 0.0);
                assert_eq!(
                    good(&rx),
                    1.0,
                    "{rate} {carrier} {ppm} {:?} {:?}",
                    units(&rx),
                    rx.results().log_lines()
                );
            }
        }
    }
}

#[test]
fn a_signal_off_the_centre_with_the_offset_set() {
    let x = frames(1e6, 100e3, vec![example(12345, 90, 0, true)], 0.05, 0.1);
    let (rx, _) = receive(&x, 1e6, 100e3);
    assert_eq!(good(&rx), 1.0);
}

#[test]
fn noise_sweep() {
    let mut lines = Vec::new();
    for snr in [30.0, 20.0, 15.0, 12.0, 10.0, 8.0, 6.0] {
        let mut ok = 0;
        for seed in 1..=10u64 {
            let mut x = frames(48e3, 0.0, vec![example(12345, 90, 0, true)], 0.05, 0.1);
            Awgn::new(1.0, snr, 12.5e3, 48e3, seed).process(&mut x);
            let (rx, _) = receive(&x, 48e3, 0.0);
            if good(&rx) == 1.0 && units(&rx).first().map(String::as_str) == Some("12345") {
                ok += 1;
            }
        }
        lines.push(format!("C/N {snr} dB in 12.5 kHz: {ok} of 10"));
        if snr >= 12.0 {
            assert_eq!(ok, 10, "{lines:?}");
        }
    }
    println!("{}", lines.join("\n"));
}

#[test]
fn frames_close_together() {
    let list: Vec<EotData> = (100..104).map(|u| example(u, 90, 0, true)).collect();
    let x = frames(48e3, 0.0, list, 0.01, 0.1);
    let (rx, _) = receive(&x, 48e3, 0.0);
    assert_eq!(good(&rx), 4.0);
    let mut u = units(&rx);
    u.sort();
    assert_eq!(u, ["00100", "00101", "00102", "00103"]);
}

#[test]
fn noise_gives_no_frame() {
    let rate = 48e3;
    let x = Awgn::with_sigma(0.1, 7).noise((rate * 5.0) as usize);
    let (rx, out) = receive(&x, rate, 0.0);
    assert_eq!(good(&rx), 0.0);
    assert!(rx.results().status().lock_state != sdrkit_core::LockState::Locked);
    assert!(out.events.is_empty());
}

/// 600 s of noise through the stack, made and fed a second at a time: no frame, and the lock never set. The false-frame
/// rates over many runs are tested on the modem.
#[test]
fn ten_minutes_of_noise_give_no_frame() {
    let rate = 48e3;
    let mut rx = stack_graph(&EOT, &[]);
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .expect("configure");
    let mut noise = Awgn::with_sigma(0.1, 13);
    let mut out = Output::default();
    let (mut frames, mut locked) = (0, 0);
    for _ in 0..600 {
        let x = noise.noise(rate as usize);
        for c in x.chunks(4096) {
            rx.feed(c, &mut out);
            frames += out
                .events
                .iter()
                .filter(|e| e.kind() == "eot.frame")
                .count();
            out.clear();
        }
        locked += usize::from(rx.results().status().lock_state == sdrkit_core::LockState::Locked);
    }
    rx.finish(&mut out);
    let bad = value(rx.results(), "frames_bad").unwrap_or(0.0);
    assert_eq!((good(&rx), frames, locked), (0.0, 0, 0), "bad {bad}");
}

#[test]
fn a_rate_below_the_least_is_refused() {
    let mut rx = stack_graph(&EOT, &[]);
    assert!(rx
        .configure(&InputInfo::new(24e3, DIAL, DIAL).recording())
        .is_err());
}

#[test]
fn head_of_train_frames() {
    let x = frames(48e3, 0.0, vec![example(4321, 0, 3, false)], 0.05, 0.1);
    let mut rx = stack_graph(&EOT, &[("direction", serde_json::json!("hot"))]);
    rx.configure(&InputInfo::new(48e3, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let r = rx.results();
    assert_eq!(
        cell_text(r, "frames", "000000", "link").as_deref(),
        Some("Head of train")
    );
    assert_eq!(
        cell_text(r, "frames", "000000", "text").as_deref(),
        Some("command, message type 3")
    );
    let e = out.events.iter().find(|e| e.kind() == "eot.frame").unwrap();
    assert_eq!(e.get(&sdrkit_core::publish::keys::SOURCE), Some("04321"));
    let _ = Direction::Hot;
}

#[test]
fn retune_keeps_clear_empties_and_the_state_round_trips() {
    let x = frames(48e3, 0.0, vec![example(12345, 90, 0, true)], 0.05, 0.1);
    let (mut rx, _) = receive(&x, 48e3, 0.0);
    assert_eq!(good(&rx), 1.0);
    let saved = rx.save_state().expect("a saved state");
    // A retune keeps the table.
    rx.retuned(&InputInfo::new(48e3, DIAL + 1e6, DIAL + 1e6).recording());
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 4800], &mut out);
    assert_eq!(good(&rx), 1.0);
    clear(&mut rx);
    rx.feed(&vec![Complex32::new(0.0, 0.0); 4800], &mut out);
    assert_eq!(good(&rx), 0.0);
    assert_eq!(texts(&rx).len(), 0);
    let mut back = stack_graph(&EOT, &[]);
    back.configure(&InputInfo::new(48e3, DIAL, DIAL).recording())
        .unwrap();
    back.restore_state(&saved).expect("the state is taken");
    back.feed(&vec![Complex32::new(0.0, 0.0); 4800], &mut out);
    assert_eq!(good(&back), 1.0);
    assert_eq!(units(&back)[0], "12345");
}
