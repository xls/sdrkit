//! COSPAS-SARSAT: the recorded vectors, the generator through the standard impairments, the bank, the saved state, the country
//! names, the BCH repairs and the environment.

#![allow(clippy::unusual_byte_groupings)] // the groups follow the message fields

use crate::common::{cell, cell_num, command, command_with, number, row_count, set, values};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, InputInfo, LockState, Output, Receiver, Results, StackConfig, StateBlob,
    Value,
};
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_stack_satcom::modes::sarsat::{self, t001, t018, Preset};
use serde_json::json;

const DIAL: f64 = 406.028e6;
const CENTRE: f64 = 406.048e6;

fn rx_for(rate: f64, centre: f64, dial: f64, settings: &[CaseSetting]) -> Box<dyn Receiver> {
    let v = values(&sarsat::DESCRIPTOR, settings);
    let mut rx = (sarsat::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, centre, dial).recording())
        .unwrap();
    rx
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.extend(
            out.events
                .drain(..)
                .filter(|e| e.is(&sdrkit_stack_satcom::typed::SARSAT_BEACON)),
        );
    }
    ev
}

fn int(r: &Results, key: &str) -> i64 {
    let id = r.decl().field_id(key).unwrap();
    match r.fields().value(id) {
        Some(Value::I64(v)) => *v,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(e: &Event, key: &str) -> String {
    cell(&e.fields, key)
}

fn num(e: &Event, key: &str) -> Option<f64> {
    cell_num(&e.fields, key)
}

fn bits_of_hex(h: &str, n: usize) -> Vec<bool> {
    let mut b: Vec<bool> = h
        .chars()
        .flat_map(|c| {
            let v = c.to_digit(16).unwrap();
            (0..4).rev().map(move |i| (v >> i) & 1 == 1)
        })
        .collect();
    b.truncate(n);
    b
}

fn sent_fgb() -> Vec<String> {
    t001::test_beacons()
        .iter()
        .map(|s| {
            t001::decode(&t001::encode(s), &mut t001::Codes::new())
                .unwrap()
                .hex_id
        })
        .collect()
}

fn sent_sgb() -> Vec<String> {
    t018::test_beacons()
        .iter()
        .map(|s| {
            t018::decode(&t018::encode(s), &mut t018::Code::new())
                .unwrap()
                .hex_id_23
        })
        .collect()
}

fn ids(ev: &[Event]) -> Vec<String> {
    ev.iter().map(|e| text(e, "hex_id").to_string()).collect()
}

/// The recorded first-generation bursts: everything but the fields of PDF-2 (the recording holds an earlier PDF-2 layout).
#[test]
fn vector_first_generation() {
    let rec = sdrkit_dev_test::read_recording("satcom/sarsat_fgb_128k")
        .expect("the recording satcom/sarsat_fgb_128k");
    first_generation_vector(rec.rate_hz, rec.centre_hz, &rec.samples, false);
}

/// The signal of the recording satcom/sarsat_fgb_128k made by the generator of the crate: 8.5 s at 128 kHz, the five test
/// beacons in turn at the start times of the recording, the carrier 20 kHz below the centre, 20 dB carrier to noise in
/// 2.4 kHz, noise seed 7.
fn first_generation_signal() -> Vec<Complex32> {
    use sdrkit_core::Generator;
    use sdrkit_stack_satcom::modes::sarsat::gen::{GenParams, SarsatGenerator};
    const RATE: f64 = 128e3;
    let mut x = vec![Complex32::default(); (8.5 * RATE) as usize];
    for (i, start_s) in [1.0, 2.54, 4.0, 5.54, 7.0].into_iter().enumerate() {
        let mut p = GenParams::new(RATE);
        p.beacon = i + 1;
        p.gap_s = start_s;
        // One burst: the generator's next one comes a gap later.
        let mut one = vec![Complex32::default(); ((start_s + 0.6) * RATE) as usize];
        SarsatGenerator::new(p).generate(&mut one);
        for (v, o) in x.iter_mut().zip(&one) {
            *v += *o;
        }
    }
    let mut rng = Rng::new(7);
    let noise = RATE / 2400.0 / 100.0;
    let w = std::f64::consts::TAU * (DIAL - CENTRE) / RATE;
    for (k, v) in x.iter_mut().enumerate() {
        let a = w * k as f64;
        *v = *v * Complex32::new(a.cos() as f32, a.sin() as f32) + rng.complex_normal(noise);
    }
    x
}

/// The fine positions and homing flags of the long location messages, with every other value of the vector, on the signal
/// of the recording made by the generator of the crate.
#[test]
fn vector_first_generation_pdf2() {
    first_generation_vector(128e3, CENTRE, &first_generation_signal(), true);
}

/// Checks the first-generation bursts of `samples` against the expected values of the recording; the fine positions and the
/// homing flags of long location messages only with `pdf2`.
fn first_generation_vector(rate_hz: f64, centre_hz: f64, samples: &[Complex32], pdf2: bool) {
    let exp = sigmf::read_expected("satcom").unwrap();
    let e = &exp["recordings"]["sarsat_fgb_128k"];
    let want = e["bursts"].as_array().unwrap();
    for piece in [65_536usize, 4_000, 777] {
        let mut rx = rx_for(rate_hz, centre_hz, DIAL, &[]);
        let ev = feed(rx.as_mut(), samples, piece);
        let r = rx.results();
        assert_eq!(ev.len(), want.len(), "piece {piece}: {:?}", ids(&ev));
        assert_eq!(int(r, "frames_ok"), 5);
        assert_eq!(int(r, "frames_bad"), 0);
        assert_eq!(int(r, "bursts_heard"), 5);
        assert_eq!(r.status().lock_state, LockState::Locked);
        for (w, b) in want.iter().zip(&ev) {
            assert_eq!(text(b, "hex_id"), w["hex_id"].as_str().unwrap());
            assert!(
                (b.time - w["start_s"].as_f64().unwrap()).abs() < 0.04,
                "{}",
                b.time
            );
            assert!((b.freq_hz - DIAL).abs() < 5.0, "{}", b.freq_hz);
            let snr = num(b, "snr_db").unwrap();
            assert!(snr > 30.0 && snr < 45.0, "C/N {snr}");
            // The message as received decodes again to every field of the vector.
            let long = w["long"].as_bool().unwrap();
            let bits = bits_of_hex(&text(b, "message_hex"), if long { 144 } else { 112 });
            let m = t001::decode(&bits, &mut t001::Codes::new()).unwrap();
            assert_eq!(m.hex_id, w["hex_id"].as_str().unwrap());
            assert_eq!(m.long, long);
            assert_eq!(m.self_test, w["self_test"].as_bool().unwrap());
            assert_eq!(m.protocol, w["protocol"].as_str().unwrap());
            assert_eq!(m.protocol_code as u64, w["protocol_code"].as_u64().unwrap());
            assert_eq!(text(b, "protocol"), w["protocol_name"].as_str().unwrap());
            assert_eq!(text(b, "beacon_type"), w["beacon_type"].as_str().unwrap());
            assert_eq!(num(b, "mid").map(|v| v as u64), w["mid"].as_u64());
            assert_eq!(text(b, "country"), w["country"].as_str().unwrap());
            assert_eq!(
                text(b, "identification"),
                w["identification"].as_str().unwrap()
            );
            let pdf2_field = long && w["protocol"].as_str() == Some("location");
            assert_eq!(m.position, w["position"].as_bool().unwrap());
            if pdf2_field && !pdf2 {
                // The coarse position of PDF-1 only.
                let (lat, lon) = b.get(&sdrkit_core::publish::keys::POSITION).unwrap();
                assert!((lat - w["lat"].as_f64().unwrap()).abs() < 0.25);
                assert!((lon - w["lon"].as_f64().unwrap()).abs() < 0.25);
            } else if m.position {
                let (lat, lon) = b.get(&sdrkit_core::publish::keys::POSITION).unwrap();
                assert!((lat - w["lat"].as_f64().unwrap()).abs() < 2e-4);
                assert!((lon - w["lon"].as_f64().unwrap()).abs() < 2e-4);
                assert_eq!(m.fine, w["fine"].as_bool().unwrap());
            } else {
                assert!(b.get(&sdrkit_core::publish::keys::POSITION).is_none());
            }
            if pdf2 || !pdf2_field {
                assert_eq!(
                    b.at(&sdrkit_stack_satcom::typed::SarsatBeaconEvent::HOMING),
                    w["homing_121_5"].as_bool()
                );
            }
            match w.get("bch2_ok") {
                Some(v) => assert_eq!(m.bch2_ok, Some(v.as_bool().unwrap())),
                None => assert_eq!(m.bch2_ok, None),
            }
            assert_eq!(
                num(b, "bits_corrected").map(|v| v as u64),
                w["bits_corrected"].as_u64()
            );
        }
        assert_eq!(
            r.status().summary,
            format!(
                "Beacon {} EPIRB, New Zealand",
                want[4]["hex_id"].as_str().unwrap()
            )
        );
        assert_eq!(row_count(r, "bursts"), 5);
        assert_eq!(row_count(r, "beacons"), 5);
        assert_eq!(r.points().len(), 4);
    }
}

#[test]
fn vector_second_generation() {
    let rec = sdrkit_dev_test::read_recording("satcom/sarsat_sgb_192k")
        .expect("the recording satcom/sarsat_sgb_192k");
    let exp = sigmf::read_expected("satcom").unwrap();
    let e = &exp["recordings"]["sarsat_sgb_192k"];
    let want = e["bursts"].as_array().unwrap();
    for piece in [50_000usize, 3_001] {
        let mut rx = rx_for(rec.rate_hz, rec.centre_hz, DIAL, &[]);
        let ev = feed(rx.as_mut(), &rec.samples, piece);
        assert_eq!(ev.len(), want.len(), "{:?}", ids(&ev));
        assert_eq!(int(rx.results(), "frames_bad"), 0);
        for (w, b) in want.iter().zip(&ev) {
            let bits = bits_of_hex(&text(b, "message_hex"), 250);
            let m = t018::decode(&bits, &mut t018::Code::new()).unwrap();
            assert_eq!(text(b, "hex_id"), w["hex_id_23"].as_str().unwrap());
            assert_eq!(m.hex_id_23, w["hex_id_23"].as_str().unwrap());
            assert!((b.time - w["start_s"].as_f64().unwrap()).abs() < 0.02);
            assert_eq!(m.tac as u64, w["tac"].as_u64().unwrap());
            assert_eq!(m.serial as u64, w["serial"].as_u64().unwrap());
            assert_eq!(num(b, "mid").map(|v| v as u64), w["mid"].as_u64());
            assert_eq!(text(b, "country"), w["country"].as_str().unwrap());
            assert_eq!(text(b, "beacon_type"), w["beacon_type"].as_str().unwrap());
            assert_eq!(m.position, w["position"].as_bool().unwrap());
            if m.position {
                assert!((m.lat.unwrap() - w["lat"].as_f64().unwrap()).abs() < 2e-4);
                assert!((m.lon.unwrap() - w["lon"].as_f64().unwrap()).abs() < 2e-4);
            }
            assert_eq!(m.homing_121_5, w["homing_121_5"].as_bool().unwrap());
            assert_eq!(m.rls, w["rls"].as_bool().unwrap());
            assert_eq!(m.test_protocol, w["test_protocol"].as_bool().unwrap());
            assert_eq!(
                m.rotating_field_id as u64,
                w["rotating_field_id"].as_u64().unwrap()
            );
            assert_eq!(num(b, "bits_corrected"), Some(0.0));
            let off = num(b, "offset_hz").unwrap();
            assert!((off - 22_000.0).abs() < 5.0, "carrier {off}");
        }
    }
}

fn harness(rate: f64) -> Harness {
    Harness::new(&sarsat::DESCRIPTOR)
        .rate(rate)
        .set(&sdrkit_stack_satcom::modes::sarsat::gen::GEN_GAP_S, 0.8)
}

#[test]
fn generator_round_trip_with_impairments() {
    let rate = 256e3;
    for (name, offset, bitrate, reverse, snr) in [
        ("plain", 0.0, 0.0, false, 25.0),
        ("offset", 3_200.0, 0.8, true, 25.0),
        ("low", -2_000.0, -0.8, false, 12.0),
    ] {
        let run = harness(rate)
            .seconds(7.2)
            .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset)
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_BITRATE_PCT,
                bitrate,
            )
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_REVERSE,
                reverse,
            )
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -10.0)
            // A small echo and an ADC of 8 bits.
            .set(&sdrkit_algo_sim::chain::GEN_ECHO_DELAY_S, 7.0 / rate)
            .set(&sdrkit_algo_sim::chain::GEN_ECHO_DB, -20.0)
            .set(&sdrkit_algo_sim::chain::GEN_ADC_BITS, 8)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Snr,
            )
            .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, snr)
            .run()
            .unwrap();
        let ev: Vec<Event> = run.events_of("sarsat.beacon").cloned().collect();
        assert_eq!(ids(&ev), sent_fgb(), "{name}");
        for b in &ev {
            let off = num(b, "offset_hz").unwrap();
            assert!((off - offset).abs() < 5.0, "{name}: {off}");
        }
        assert_eq!(int(&run.results, "frames_bad"), 0, "{name}");
    }
}

#[test]
fn other_rates_and_clock_error() {
    for rate in [128e3, 1.024e6] {
        let run = harness(rate)
            .seconds(4.0)
            .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 40.0)
            .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 500.0)
            .run()
            .unwrap();
        assert!(int(&run.results, "frames_ok") >= 2, "rate {rate}");
        assert_eq!(int(&run.results, "frames_bad"), 0);
    }
}

#[test]
fn noise_and_spurs_give_nothing() {
    let rate = 128e3;
    let mut rng = Rng::new(3);
    let x: Vec<Complex32> = (0..(6.0 * rate) as usize)
        .map(|i| {
            // A steady spur and a DC offset over the noise.
            let p = std::f64::consts::TAU * 7_000.0 * i as f64 / rate;
            rng.complex_normal(0.005)
                + Complex32::new(0.05 + 0.2 * p.cos() as f32, 0.2 * p.sin() as f32)
        })
        .collect();
    let mut rx = rx_for(rate, CENTRE, DIAL, &[]);
    let ev = feed(rx.as_mut(), &x, 5000);
    let r = rx.results();
    assert!(ev.is_empty());
    assert_eq!(int(r, "bursts_heard"), 0);
    assert_eq!(int(r, "frames_bad"), 0);
    assert_eq!(r.status().lock_state, LockState::Tracking);
    assert!(r.status().summary.starts_with("COSPAS-SARSAT: listening"));
}

#[test]
fn second_generation_from_the_generator() {
    for (rate, ppm, snr) in [
        (192e3, 0.0, 25.0),
        (1.024e6, 50.0, 20.0),
        (2.4e6, 0.0, 18.0),
    ] {
        let run = Harness::new(&sarsat::DESCRIPTOR)
            .rate(rate)
            .seconds(7.2)
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_SECOND_GENERATION,
                true,
            )
            .set(&sdrkit_stack_satcom::modes::sarsat::gen::GEN_GAP_S, 0.6)
            .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, ppm)
            .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 1500.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Snr,
            )
            .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, snr)
            .run()
            .unwrap();
        let ev: Vec<Event> = run.events_of("sarsat.beacon").cloned().collect();
        assert_eq!(ids(&ev), sent_sgb(), "rate {rate}");
        assert_eq!(int(&run.results, "frames_bad"), 0);
    }
}

#[test]
fn wide_search_bank_clear_and_saved_state() {
    let rate = 1.2e6;
    let x = harness(rate)
        .seconds(3.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 250e3) // far from the dial: only the bank finds it
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 20.0)
        .generate()
        .unwrap();
    let mut rx = rx_for(
        rate,
        DIAL,
        DIAL,
        &[CaseSetting::new(
            &sdrkit_stack_satcom::modes::sarsat::WIDE_SEARCH,
            true,
        )],
    );
    let ev = feed(rx.as_mut(), &x, 30_000);
    let r = rx.results();
    assert!(int(r, "finders") >= 9);
    let span = r
        .decl()
        .field_id("span_hz")
        .and_then(|id| r.fields().value(id));
    assert!(span.and_then(number).is_some_and(|s| s >= 800e3));
    let n = int(r, "frames_ok");
    assert!((1..=4).contains(&n), "{n}");
    let off = num(&ev[0], "offset_hz").unwrap();
    assert!((off - 250e3).abs() < 3e3, "{off}");
    // Silence ends any burst still open, so the log stays as it is from here on.
    let mut rng = Rng::new(9);
    let quiet: Vec<Complex32> = (0..(0.3 * rate) as usize)
        .map(|_| rng.complex_normal(1e-4))
        .collect();
    feed(rx.as_mut(), &quiet, 30_000);
    let rows = row_count(rx.results(), "bursts");
    assert!(rows >= 1);
    // Save, clear, restore.
    let state = rx.save_state().expect("a saved state");
    command(rx.as_mut(), "clear").unwrap();
    feed(rx.as_mut(), &quiet, 30_000);
    assert_eq!(row_count(rx.results(), "bursts"), 0);
    assert!(rx
        .restore_state(&StateBlob::new("bad", 1, Vec::new()))
        .is_err());
    rx.restore_state(&state).unwrap();
    feed(rx.as_mut(), &quiet, 30_000);
    assert_eq!(row_count(rx.results(), "bursts"), rows);
    // A retune keeps the log.
    rx.retuned(&InputInfo::new(rate, 406.050e6, 406.030e6).recording());
    feed(rx.as_mut(), &quiet, 30_000);
    assert_eq!(row_count(rx.results(), "bursts"), rows);
    // Remove forgets one beacon. A row that is not there is refused.
    let beacons = row_count(rx.results(), "beacons");
    assert!(beacons >= 1);
    let row = crate::common::rows(rx.results(), "beacons")[0].0.to_owned();
    command_with(rx.as_mut(), "remove", &[("row", Value::Text(row))]).unwrap();
    assert_eq!(row_count(rx.results(), "beacons"), beacons - 1);
    let none = Value::Text("none".into());
    assert!(matches!(
        command_with(rx.as_mut(), "remove", &[("row", none)]),
        Err(sdrkit_core::CommandError::NoSuchRow { .. })
    ));
}

#[test]
fn every_event_is_declared_with_its_frequency() {
    for second in [false, true] {
        let run = harness(256e3)
            .seconds(4.0)
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_SECOND_GENERATION,
                second,
            )
            .run()
            .unwrap();
        assert!(!run.events.is_empty());
        for e in &run.events {
            assert!(e.is(&sdrkit_stack_satcom::typed::SARSAT_BEACON), "{e:?}");
            assert!(e.freq_hz > 400e6, "{}", e.freq_hz);
            let test =
                e.at(&sdrkit_stack_satcom::typed::SarsatBeaconEvent::SELF_TEST) == Some(true);
            let urgency = e.get(&sdrkit_core::publish::keys::URGENCY);
            assert_eq!(
                urgency,
                Some(if test {
                    sdrkit_core::publish::keys::Urgency::None
                } else {
                    sdrkit_core::publish::keys::Urgency::Emergency
                })
            );
        }
    }
}

#[test]
fn a_preset_asks_the_host_to_retune() {
    let mut rx = rx_for(256e3, CENTRE, DIAL, &[]);
    let downlink = Value::Choice(sdrkit_core::Choice::index(Preset::Downlink));
    command_with(rx.as_mut(), "preset.apply", &[("preset", downlink)]).unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 25_600], &mut out);
    assert!(matches!(
        out.requests.as_slice(),
        [sdrkit_core::HostRequest::Retune { tuned_hz, .. }] if *tuned_hz == 1544.5e6
    ));
}

#[test]
fn bad_settings_are_refused() {
    let mut rx = rx_for(256e3, CENTRE, DIAL, &[]);
    assert!(set(rx.as_mut(), "wide_search", json!("maybe")).is_err());
}

/// The country of a MID comes from the ITU MID table of sdrkit-util-ids.
#[test]
fn mid_374_and_550_name_their_countries() {
    for (mid, country) in [(374, "Panama"), (550, "Timor-Leste")] {
        let mut s = t001::test_beacons()[0].clone();
        s.mid = mid;
        let d = t001::decode(&t001::encode(&s), &mut t001::Codes::new()).unwrap();
        assert_eq!(d.country, country);
    }
}

/// The BCH decoders repair in place with no allocation.
#[test]
fn bch_repairs_allocate_nothing() {
    let mut codes = t001::Codes::new();
    let mut code = t018::Code::new();
    let good1 = t001::encode(&t001::test_beacons()[0]);
    let good2 = t018::encode(&t018::test_beacons()[0]);
    let mut f1 = good1[24..106].to_vec();
    let mut f2 = good1[106..144].to_vec();
    let mut w = good2.clone();
    // Warm up: the working buffers reach their size.
    codes.repair1(&mut f1);
    code.repair(&mut w);
    let n = alloc_count::count(|| {
        for k in 0..100 {
            f1.copy_from_slice(&good1[24..106]);
            f1[k % 82] = !f1[k % 82];
            f1[(k * 7 + 3) % 82] = !f1[(k * 7 + 3) % 82];
            assert!(codes.repair1(&mut f1).is_some());
            f2.copy_from_slice(&good1[106..144]);
            f2[k % 38] = !f2[k % 38];
            assert!(codes.repair2(&mut f2).is_some());
            w.copy_from_slice(&good2);
            for e in 0..6 {
                w[(k + 41 * e) % 250] = !w[(k + 41 * e) % 250];
            }
            assert_eq!(code.repair(&mut w), Some(6));
        }
    });
    assert_eq!(n, 0);
    assert_eq!(f1, good1[24..106]);
    assert_eq!(w, good2);
}

/// No source of the stack reads the environment.
#[test]
fn the_decoders_read_no_environment() {
    for (name, src) in [
        ("fgb.rs", include_str!("../../src/modes/sarsat/fgb.rs")),
        ("sgb.rs", include_str!("../../src/modes/sarsat/sgb.rs")),
        ("mod.rs", include_str!("../../src/modes/sarsat/mod.rs")),
    ] {
        assert!(!src.contains(concat!("env", "::var")), "{name}");
    }
}

mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct Counting;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            COUNT.with(|c| c.set(c.get() + 1));
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            COUNT.with(|c| c.set(c.get() + 1));
            unsafe { System.realloc(p, l, n) }
        }
    }

    /// Allocations made by this thread while `f` runs (other test threads do not count).
    pub fn count(f: impl FnOnce()) -> usize {
        let before = COUNT.with(|c| c.get());
        f();
        COUNT.with(|c| c.get()) - before
    }

    #[global_allocator]
    static A: Counting = Counting;
}

// A capture with no noise at all (silence between bursts): a window mostly in silence before a burst held a few of its chips with
// little energy of its own, so the second-generation detector mistook it for a preamble and counted a failed burst.
#[test]
fn a_capture_without_noise_has_no_failed_bursts() {
    for second in [false, true] {
        let run = Harness::new(&sarsat::DESCRIPTOR)
            .seconds(4.0)
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_SECOND_GENERATION,
                second,
            )
            .set(
                &sdrkit_stack_satcom::modes::sarsat::gen::GEN_GAP_S,
                if second { 0.6 } else { 0.8 },
            )
            .run()
            .unwrap();
        assert_eq!(
            int(&run.results, "frames_ok"),
            2,
            "second generation {second}"
        );
        assert_eq!(
            int(&run.results, "frames_bad"),
            0,
            "second generation {second}"
        );
    }
}

/// A long first-generation message: bits 1-15 ones, the normal frame sync, bit 25 set, the fields of `fill`, both BCH codes.
fn t001_message(fill: impl Fn(&mut [bool])) -> Vec<bool> {
    let mut b = vec![false; 144];
    put_bits(&mut b, 1, 15, 0x7FFF);
    put_bits(&mut b, 16, 24, 0b000101111);
    b[24] = true;
    fill(&mut b);
    t001::seal(&mut b);
    b
}

/// Sets bits `first` to `last` (numbered from 1) to `v`, most significant bit first.
fn put_bits(b: &mut [bool], first: usize, last: usize, v: u64) {
    let n = last - first + 1;
    for i in 0..n {
        b[first - 1 + i] = (v >> (n - 1 - i)) & 1 == 1;
    }
}

fn read_t001(b: &[bool]) -> t001::Fgb {
    t001::decode(b, &mut t001::Codes::new()).expect("a valid message")
}

/// The identity of the ELT(DT) vectors: country 226, protocol 9, bits 26-85 of hex ID 1C538006253FDFF.
fn elt_dt_identity(b: &mut [bool]) {
    put_bits(b, 26, 85, 0x1C5_3800_6253_FDFF);
}

/// ELT(DT) vector 1: a position with offsets, manual activation, above 10000 m, current.
#[test]
fn elt_dt_position_with_offsets() {
    let b = t001_message(|b| {
        elt_dt_identity(b);
        b[66] = false;
        put_bits(b, 68, 75, 87);
        b[75] = false;
        put_bits(b, 77, 85, 3);
        put_bits(b, 107, 108, 0);
        put_bits(b, 109, 112, 14);
        put_bits(b, 113, 114, 0b11);
        put_bits(b, 115, 132, 0b1_0011_1001_0_0001_0010);
    });
    let d = read_t001(&b);
    assert_eq!((d.bch2_ok, d.bits_corrected), (Some(true), 0));
    assert_eq!(d.protocol_code, 9);
    assert_eq!(d.beacon_type, "ELT-DT");
    assert_eq!(d.protocol_name, "ELT-DT location");
    assert_eq!(d.mid, 226);
    assert_eq!(d.hex_id, "1C538006253FDFF");
    assert!((d.lat.unwrap() - 43.56).abs() < 1e-4, "{:?}", d.lat);
    assert!((d.lon.unwrap() - 1.481111).abs() < 1e-4, "{:?}", d.lon);
    assert!(d.fine && !d.homing_121_5);
    for t in ["manual activation", ">10000 m", "current", "type 11"] {
        assert!(d.identification.contains(t), "{t}: {}", d.identification);
    }
}

/// ELT(DT) vector 2: the default offsets leave the PDF-1 position, and the hex ID does not follow it.
#[test]
fn elt_dt_default_offsets() {
    let b = t001_message(|b| {
        elt_dt_identity(b);
        b[66] = true;
        put_bits(b, 68, 75, 20);
        b[75] = true;
        put_bits(b, 77, 85, 40);
        put_bits(b, 107, 108, 0b01);
        put_bits(b, 109, 112, 15);
        put_bits(b, 113, 114, 0b01);
        put_bits(b, 115, 132, 0b1_0000_1111_1_0000_1111);
    });
    let d = read_t001(&b);
    assert_eq!((d.lat, d.lon), (Some(-10.0), Some(-20.0)));
    assert!(!d.fine);
    assert_eq!(d.hex_id, "1C538006253FDFF");
    assert!(
        d.identification.contains("by the beacon"),
        "{}",
        d.identification
    );
    assert!(
        d.identification.contains("not available"),
        "{}",
        d.identification
    );
}

/// ELT(DT) protocol 9 of country 226 with identity type `t` and identity `ident`, no position, and PDF-2 bits 107-132 `pdf2`.
fn elt_dt_plain(t: u64, ident: u64, pdf2: u64) -> Vec<bool> {
    t001_message(move |b| {
        put_bits(b, 27, 36, 226);
        put_bits(b, 37, 40, 9);
        put_bits(b, 41, 42, t);
        put_bits(b, 43, 66, ident);
        put_bits(b, 67, 75, 0x0FF);
        put_bits(b, 76, 85, 0x1FF);
        put_bits(b, 107, 132, pdf2);
    })
}

/// ELT(DT) vectors 3 to 6: the aircraft address with rotating field type 000, the operator designator, the location test and
/// the cancellation message.
#[test]
fn elt_dt_identities_test_and_cancellation() {
    let rot = 0b10_0011_00_000_10001_01001_01010;
    let d = read_t001(&elt_dt_plain(0, 0x3C6444, rot));
    assert_eq!(d.bch2_ok, Some(true));
    assert!(!d.position);
    for t in ["aircraft address 3C6444", "operator ZLR", "external means"] {
        assert!(d.identification.contains(t), "{t}: {}", d.identification);
    }
    let op = (0b11000_10110_01010 << 9) | 77;
    let d = read_t001(&elt_dt_plain(1, op, 0b00_0000_11_1_0000_1111_1_0000_1111));
    assert!(
        d.identification.contains("operator AFR, serial 77"),
        "{}",
        d.identification
    );
    let d = read_t001(&elt_dt_plain(0, 0, 0b00_1111_01_1_0000_1111_1_0000_1111));
    assert_eq!(d.beacon_type, "Test");
    assert!(
        d.identification.contains("location test"),
        "{}",
        d.identification
    );
    let b = t001_message(|b| {
        put_bits(b, 27, 36, 226);
        put_bits(b, 37, 40, 9);
        put_bits(b, 43, 66, 0x3C6444);
        put_bits(b, 67, 75, 0x1FA);
        put_bits(b, 76, 85, 0x3FA);
        put_bits(b, 107, 114, 0x3C);
        put_bits(b, 115, 123, 0x0F0);
        put_bits(b, 124, 132, 0x0F0);
    });
    let d = read_t001(&b);
    assert!(!d.position);
    assert!(
        d.identification.contains("cancellation"),
        "{}",
        d.identification
    );
}

/// Vector 7: standard location offsets of 5 minute bits and 4 second bits, the fixed bits 1101, the position source and the
/// homing bit; seconds 1111 seal the offset.
#[test]
fn standard_location_offsets_and_homing() {
    let make = |lat_sec: u64| {
        t001_message(move |b| {
            put_bits(b, 27, 36, 211);
            put_bits(b, 37, 40, 0b0011);
            put_bits(b, 41, 64, 0x3C6444);
            b[64] = false;
            put_bits(b, 66, 74, 216);
            b[74] = false;
            put_bits(b, 76, 85, 48);
            put_bits(b, 107, 110, 0b1101);
            b[110] = true;
            b[111] = true;
            b[112] = true;
            put_bits(b, 114, 118, 20);
            put_bits(b, 119, 122, lat_sec);
            b[122] = false;
            put_bits(b, 124, 128, 3);
            put_bits(b, 129, 132, 2);
        })
    };
    let d = read_t001(&make(9));
    assert_eq!(d.bch2_ok, Some(true));
    assert!(d.fine && d.homing_121_5);
    assert!((d.lat.unwrap() - (54.0 + 20.0 / 60.0 + 36.0 / 3600.0)).abs() < 1e-6);
    assert!((d.lon.unwrap() - (12.0 - 3.0 / 60.0 - 8.0 / 3600.0)).abs() < 1e-6);
    let d = read_t001(&make(0b1111));
    assert!(!d.fine);
    assert_eq!(d.lat, Some(54.0));
}

/// Vector 8: a user protocol with bits 84-85 = 01 has homing; the orbitography protocol has no homing bits.
#[test]
fn user_protocol_homing() {
    let spec = t001::FgbSpec {
        mid: 219,
        long: false,
        self_test: false,
        id: t001::FgbId::UserMaritime {
            text: "OXTEST".into(),
            beacon: '1',
        },
        position: None,
        homing: true,
    };
    assert!(read_t001(&t001::encode(&spec)).homing_121_5);
    let orbit = t001::FgbSpec {
        id: t001::FgbId::UserOther { code: 0, bits: 1 },
        ..spec
    };
    assert!(!read_t001(&t001::encode(&orbit)).homing_121_5);
}

/// The ELT(DT) fields of the generator's encoder decode again.
#[test]
fn elt_dt_encoder_round_trip() {
    let spec = t001::FgbSpec {
        mid: 226,
        long: true,
        self_test: false,
        id: t001::FgbId::EltDt {
            id_type: 0,
            addr: 0x3C6444,
            activation: 2,
            altitude: 3,
            freshness: 3,
            operator: None,
            cancel: false,
        },
        position: Some((43.56, 1.481111)),
        homing: false,
    };
    let d = read_t001(&t001::encode(&spec));
    assert!(d.fine && !d.homing_121_5);
    assert!((d.lat.unwrap() - 43.56).abs() < 6e-4 && (d.lon.unwrap() - 1.481111).abs() < 6e-4);
    let rot = t001::FgbSpec {
        id: t001::FgbId::EltDt {
            id_type: 0,
            addr: 0x3C6444,
            activation: 2,
            altitude: 3,
            freshness: 0,
            operator: Some("ZLR".into()),
            cancel: false,
        },
        position: None,
        ..spec.clone()
    };
    assert!(read_t001(&t001::encode(&rot))
        .identification
        .contains("operator ZLR"));
    let cancel = t001::FgbSpec {
        id: t001::FgbId::EltDt {
            id_type: 0,
            addr: 0x3C6444,
            activation: 0,
            altitude: 0,
            freshness: 0,
            operator: None,
            cancel: true,
        },
        ..spec
    };
    let d = read_t001(&t001::encode(&cancel));
    assert!(!d.position && d.identification.contains("cancellation"));
}
