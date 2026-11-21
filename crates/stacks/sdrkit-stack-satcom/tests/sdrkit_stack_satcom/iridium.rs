//! Iridium: the physical layer and RAW lines from the generator's bursts.

use crate::common::{int, row_count, set};
use sdrkit_core::{Complex32, Generator, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satcom::modes::iridium::{self, gen, phy};
use serde_json::json;

const RATE: f64 = 4e6;

#[test]
fn r1_unique_words_and_mapping() {
    assert_eq!(phy::UW_DOWN, "001100000011000011110011");
    assert_eq!(phy::UW_UP, "110011000011110011111100");
    for k in 0..4u8 {
        let b = phy::step_bits(k);
        assert_eq!(phy::bits_step(b[0], b[1]), k);
    }
    assert_eq!(
        [
            phy::step_bits(0),
            phy::step_bits(1),
            phy::step_bits(2),
            phy::step_bits(3)
        ],
        [[false, false], [true, false], [true, true], [false, true]]
    );
    assert_eq!(phy::uw_steps(true), &[0, 2, 0, 0, 0, 2, 0, 0, 2, 2, 0, 2]);
    assert_eq!(phy::uw_steps(false), &[2, 0, 2, 0, 0, 2, 2, 0, 2, 2, 2, 0]);
}

fn harness(centre: f64, seconds: f64) -> Harness {
    Harness::new(&iridium::DESCRIPTOR)
        .rate(RATE)
        .freq(centre)
        .seconds(seconds)
        .piece(100_000)
        .set(
            &sdrkit_stack_satcom::modes::iridium::gen::GEN_CENTRE_HZ,
            centre,
        )
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Cn0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_CN0_DBHZ, 64.0)
}

/// The bursts the harness's generator sends (the same seed and settings).
fn sent(centre: f64, seconds: f64) -> Vec<gen::Sent> {
    let mut p = gen::GenParams::new(RATE);
    p.centre_hz = centre;
    let mut g = gen::IrGenerator::new(p);
    let mut x = vec![Complex32::new(0.0, 0.0); (seconds * RATE) as usize];
    g.generate(&mut x);
    g.sent
}

#[test]
fn r6_r8_bursts_found_and_bits_exact() {
    for centre in [1626.25e6, 1622.0e6] {
        check(centre);
    }
}

fn check(centre: f64) {
    let r = harness(centre, 1.0)
        .set(&sdrkit_stack_satcom::modes::iridium::RAW_OUTPUT, true)
        .run()
        .expect("the run");
    let g = sent(centre, 1.0);
    let uw_ok = int(&r, "frames_ok");
    // The detector's noise floor settles in its first frames: bursts from 5 ms on.
    let n = g
        .iter()
        .filter(|s| s.start_s > 0.005 && s.start_s < 0.95)
        .count() as u64;
    assert!(uw_ok * 100 >= n * 96, "{uw_ok} of {n}");
    assert!(uw_ok <= n + 2);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    // Every RAW line holds the bits sent after the unique word.
    let mut exact = 0;
    let lines: Vec<_> = r.events_of("iridium.raw").collect();
    assert_eq!(lines.len() as u64, uw_ok);
    for e in &lines {
        let line = e.get(&sdrkit_core::publish::keys::TEXT).unwrap();
        let bits = line.split_whitespace().last().unwrap();
        if g.iter().any(|s| {
            let b: String = s.bits.iter().map(|&x| if x { '1' } else { '0' }).collect();
            let n = (bits.len() - 24).min(b.len());
            n + 20 >= b.len() && bits[24..24 + n] == b[..n]
        }) {
            exact += 1;
        }
        // The fields of the RAW line.
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts[0], "RAW:");
        assert!(parts[1].starts_with("i-") && parts[1].ends_with("-t1"));
        assert!(parts[2].parse::<f64>().is_ok() && parts[3].parse::<u64>().is_ok());
        assert!(parts[4].starts_with("N:") && parts[5].starts_with("I:"));
        assert!(parts[6].ends_with('%') || parts[7] == "%");
        assert!(e.freq_hz > 1.61e9, "{e:?}");
    }
    assert!(exact * 100 >= uw_ok * 95, "{exact} of {uw_ok}");
    assert_eq!(row_count(&r.results, "raw") as u64, uw_ok.min(200));
}

#[test]
fn noise_has_no_unique_word() {
    let mut x = vec![Complex32::new(0.0, 0.0); (0.5 * RATE) as usize];
    let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(2e-4, 2).unwrap();
    awgn.process(&mut x);
    let r = harness(1626.25e6, 0.5).feed(&x).unwrap();
    assert_eq!(int(&r, "frames_ok"), 0);
    assert_eq!(r.results.status().lock_state, LockState::Searching);
}

#[test]
fn cut_independence() {
    let h = harness(1626.25e6, 0.6).set(&sdrkit_stack_satcom::modes::iridium::RAW_OUTPUT, true);
    let a = h.clone().piece(4_099).run().unwrap();
    let b = h.piece(400_000).run().unwrap();
    assert!(int(&a, "frames_ok") > 10);
    assert_eq!(a.field("frames_ok"), b.field("frames_ok"));
    let raw = |r: &sdrkit_dev_test::Run| {
        let id = r.results.decl().table_id("raw").unwrap();
        r.results.table(id).unwrap().clone()
    };
    assert_eq!(raw(&a), raw(&b));
}

#[test]
fn every_event_is_declared() {
    let r = harness(1626.25e6, 0.5)
        .set(&sdrkit_stack_satcom::modes::iridium::RAW_OUTPUT, true)
        .run()
        .unwrap();
    assert!(!r.events.is_empty());
    assert!(r
        .events
        .iter()
        .all(|e| e.is(&sdrkit_stack_satcom::typed::IRIDIUM_RAW_EVENT)));
}

#[test]
fn bad_settings_are_refused() {
    let mut rx = Harness::new(&iridium::DESCRIPTOR).receiver().unwrap();
    assert!(set(rx.as_mut(), "threshold_db", json!(30.0)).is_err());
    assert!(set(rx.as_mut(), "threshold_db", json!(12.0)).is_ok());
    assert!(set(rx.as_mut(), "raw_output", json!("sometimes")).is_err());
}
