//! Argos: the A2 layer, the generator's uplink bursts and downlink through the standard impairments into the receiver.

use crate::common::{cell, cell_num, command, field_text, row_count, rows, set};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, Event, LockState, Record, Results, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_satcom::modes::argos::{self, a2, gen};
use sdrkit_stack_satcom::typed::{ArgosDownlinkFrameEvent, ArgosPttEvent};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::{hdlc_encode, HdlcDeframer};
use serde_json::json;

const RATE: f64 = 256e3;
const DOWNLINK_HZ: f64 = 465.9875e6;

fn harness() -> Harness {
    Harness::new(&argos::DESCRIPTOR).rate(RATE)
}

fn int(r: &Results, key: &str) -> i64 {
    let id = r.decl().field_id(key).unwrap();
    match r.fields().value(id) {
        Some(Value::I64(v)) => *v,
        other => panic!("{key}: {other:?}"),
    }
}

/// The row of table `table` with key `key`.
fn row<'a>(r: &'a Results, table: &str, key: &str) -> Option<&'a Record> {
    r.table(r.decl().table_id(table)?)?.row(key)
}

fn ptt(run: &Run) -> Vec<Event> {
    run.events_of("argos.ptt").cloned().collect()
}

/// Every platform heard at least `min_each` times, with its exact data.
fn check_platforms(run: &Run, min_each: usize) {
    let ev = ptt(run);
    for (i, (id, _, k)) in gen::PLATFORMS.iter().enumerate() {
        let hex = format!("{id:07X}");
        let mine: Vec<&Event> = ev
            .iter()
            .filter(|e| e.at(&ArgosPttEvent::PLATFORM) == Some(hex.as_str()))
            .collect();
        assert!(
            mine.len() >= min_each,
            "{hex}: {} of {}",
            mine.len(),
            ev.len()
        );
        let want = a2::parse(&a2::build(*id, *k, &gen::platform_data(i))).unwrap();
        for e in mine {
            assert_eq!(e.get(&keys::TEXT), Some(want.data_hex.as_str()));
            assert_eq!(e.at(&ArgosPttEvent::DATA_BITS), Some(want.data_bits as i64));
        }
        assert!(row(&run.results, "platforms", &hex).is_some());
    }
}

#[test]
fn v1_hdlc_frames_round_trip() {
    let frames: Vec<Vec<u8>> = vec![
        vec![0x7E, 0xFF, 0x00, 0x1F],
        (0..40).collect(),
        vec![0x55; 7],
    ];
    let refs: Vec<&[u8]> = frames.iter().map(Vec::as_slice).collect();
    let mut bits = Vec::new();
    hdlc_encode(&refs, &CRC16_IBM_SDLC, 1, &mut bits);
    let mut d = HdlcDeframer::x25(1, 398);
    let mut got = Vec::new();
    for &b in &bits {
        if d.push(b).is_some_and(|f| f.fcs_ok) {
            got.push(d.frame().to_vec());
        }
    }
    assert_eq!(got, frames);
    let mut bad = Vec::new();
    hdlc_encode(&[&frames[1]], &CRC16_IBM_SDLC, 1, &mut bad);
    bad[60] ^= 1;
    let mut d = HdlcDeframer::x25(1, 398);
    assert!(bad.iter().all(|&b| !d.push(b).is_some_and(|f| f.fcs_ok)));
}

#[test]
fn v2_uplink_platforms() {
    // Three platforms and the other burst, 2 s apart: 3 cycles.
    let run = harness()
        .seconds(3.0 * 4.0 * 2.7 + 1.0)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_OTHER, true)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .run()
        .unwrap();
    check_platforms(&run, 3);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    let kinds: Vec<String> = rows(&run.results, "bursts")
        .into_iter()
        .map(|(_, r)| cell(r, "kind"))
        .collect();
    // The QPSK burst is listed as another modulation and never decoded.
    let other = kinds.iter().filter(|k| *k == "other").count();
    assert!(other >= 1, "{kinds:?}");
    assert_eq!(int(&run.results, "frames_bad") as usize, other);
    for e in ptt(&run) {
        assert!(e.at(&ArgosPttEvent::QUALITY).unwrap() > 0.8);
    }
}

#[test]
fn v2_uplink_noise_offsets_drift_and_rate() {
    // 16 dB in 2.4 kHz, a few kHz off, 40 Hz/s drift, a 150 Hz step from burst to burst.
    let run = harness()
        .seconds(3.0 * 3.0 * 1.8 + 1.0)
        .piece(7_777)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 16.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 2_500.0)
        .set(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, 40.0)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_STEP_HZ, 150.0)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_GAP_S, 1.0)
        .run()
        .unwrap();
    check_platforms(&run, 3);
    for (id, r) in rows(&run.results, "platforms") {
        let d = cell_num(r, "drift_hz_s").unwrap();
        assert!(d > 0.0, "{id}: {d}");
    }
    // Another sample rate and a clock error.
    let run = Harness::new(&argos::DESCRIPTOR)
        .rate(192e3)
        .seconds(3.0 * 3.0 * 1.8)
        .piece(4_096)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_GAP_S, 1.0)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 30.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .run()
        .unwrap();
    check_platforms(&run, 2);
}

#[test]
fn v2_drift_matches_the_step() {
    let run = harness()
        .seconds(3.0 * 6.0 * 1.8)
        .piece(50_000)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_STEP_HZ, 100.0)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_GAP_S, 1.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .run()
        .unwrap();
    let ev = ptt(&run);
    for (id, _, _) in gen::PLATFORMS {
        let hex = format!("{id:07X}");
        let ts: Vec<f64> = ev
            .iter()
            .filter(|e| e.at(&ArgosPttEvent::PLATFORM) == Some(hex.as_str()))
            .map(|e| e.time)
            .collect();
        let period = (ts.last().unwrap() - ts[0]) / (ts.len() - 1) as f64;
        let want = 100.0 / period;
        let d = row(&run.results, "platforms", &hex)
            .and_then(|r| cell_num(r, "drift_hz_s"))
            .unwrap();
        assert!((d - want).abs() < 0.25 * want, "{hex}: {d} vs {want}");
    }
}

#[test]
fn v2_noise_lists_nothing() {
    let mut rng = Rng::new(9);
    let x: Vec<Complex32> = (0..(8.0 * RATE) as usize)
        .map(|_| rng.complex_normal(0.005))
        .collect();
    let run = harness().piece(32_768).feed(&x).unwrap();
    assert_eq!(row_count(&run.results, "bursts"), 0);
    assert!(run.events.is_empty());
}

#[test]
fn v3_downlink_through_a_pass() {
    for (bpsk, nrzi) in [(true, true), (false, false)] {
        let h = Harness::new(&argos::DESCRIPTOR)
            .rate(RATE)
            .freq(DOWNLINK_HZ)
            .seconds(30.0)
            .piece(65_536)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_DOWNLINK, true)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_BPSK, bpsk)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_NRZI, nrzi)
            .set(
                &sdrkit_stack_satcom::modes::argos::gen::GEN_PASS_TCA_S,
                15.0,
            )
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Snr,
            )
            .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 30.0);
        let run = h.run().unwrap();
        let r = &run.results;
        assert_eq!(field_text(r, "link"), "downlink");
        assert_eq!(run.field("locked"), Some(&Value::Bool(true)));
        let good: Vec<String> = run
            .events_of("argos.downlink_frame")
            .map(|e| {
                let mut s = String::new();
                sdrkit_core::base64::hex_into(
                    e.at(&ArgosDownlinkFrameEvent::FRAME).unwrap(),
                    &mut s,
                );
                s
            })
            .collect();
        let mut g =
            gen::ArgosGenerator::new(gen::GenParams::from_values(RATE, &h.values().unwrap(), 1));
        let mut x = vec![Complex32::default(); (30.0 * RATE) as usize];
        sdrkit_core::Generator::generate(&mut g, &mut x);
        let sent: Vec<String> = g
            .frames_sent
            .iter()
            .map(|f| {
                let mut s = String::new();
                sdrkit_core::base64::hex_into(f, &mut s);
                s
            })
            .collect();
        // Every frame sent after the lock (the first second or so) comes back.
        let after: Vec<&String> = sent
            .iter()
            .skip(4)
            .take(sent.len().saturating_sub(6))
            .collect();
        let missing = after.iter().filter(|s| !good.contains(s)).count();
        assert!(
            missing <= 1,
            "bpsk {bpsk} nrzi {nrzi}: {missing} of {} missing",
            after.len()
        );
        let coding = field_text(r, "coding");
        if nrzi {
            assert_eq!(coding, "nrzi");
        } else {
            assert!(coding == "nrz" || coding == "inverted_nrz", "{coding}");
        }
        // The Doppler is followed.
        let p = gen::GenParams::from_values(RATE, &h.values().unwrap(), 1);
        let pts: Vec<&[f32; 2]> = r
            .series(r.decl().series_id("doppler").unwrap())
            .unwrap()
            .points
            .iter()
            .filter(|q| q[0] > 3.0)
            .collect();
        assert!(pts.len() > 20);
        for q in pts {
            let want = gen::pass_doppler(&p, q[0] as f64);
            assert!(
                (q[1] as f64 - want).abs() < 40.0,
                "t {}: {} vs {want}",
                q[0],
                q[1]
            );
        }
        for e in run.events_of("argos.downlink_frame") {
            assert!((e.freq_hz - DOWNLINK_HZ).abs() < 20e3, "{}", e.freq_hz);
        }
    }
    // Noise produces no frame.
    let mut rng = Rng::new(2);
    let x: Vec<Complex32> = (0..(6.0 * RATE) as usize)
        .map(|_| rng.complex_normal(0.005))
        .collect();
    let run = Harness::new(&argos::DESCRIPTOR)
        .rate(RATE)
        .freq(DOWNLINK_HZ)
        .piece(65_536)
        .feed(&x)
        .unwrap();
    assert_eq!(run.events_of("argos.downlink_frame").count(), 0);
}

#[test]
fn every_event_is_declared_with_its_frequency() {
    for (freq, down) in [(401.65e6, false), (DOWNLINK_HZ, true)] {
        let run = Harness::new(&argos::DESCRIPTOR)
            .rate(RATE)
            .freq(freq)
            .seconds(8.0)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_DOWNLINK, down)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_PASS_TCA_S, 3.0)
            .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_GAP_S, 1.0)
            .run()
            .unwrap();
        assert!(!run.events.is_empty(), "downlink {down}");
        for e in &run.events {
            assert!(
                e.is(&sdrkit_stack_satcom::typed::ARGOS_PTT)
                    || e.is(&sdrkit_stack_satcom::typed::ARGOS_DOWNLINK_FRAME),
                "{e:?}"
            );
            assert!((e.freq_hz - freq).abs() < 30e3, "{}", e.freq_hz);
        }
    }
}

#[test]
fn saved_state_and_clear() {
    let h = harness()
        .seconds(6.0)
        .set(&sdrkit_stack_satcom::modes::argos::gen::GEN_GAP_S, 1.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(RATE, 401.65e6, 401.65e6).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    let n = row_count(rx.results(), "platforms");
    assert!(n >= 2);
    let state = rx.save_state().unwrap();
    command(rx.as_mut(), "clear").unwrap();
    rx.feed(&vec![Complex32::default(); 25_600], &mut out);
    assert_eq!(row_count(rx.results(), "platforms"), 0);
    rx.restore_state(&state).unwrap();
    rx.feed(&vec![Complex32::default(); 25_600], &mut out);
    assert_eq!(row_count(rx.results(), "platforms"), n);
    assert!(set(rx.as_mut(), "link", json!("sideways")).is_err());
}

/// Adds an A2 burst at sample `at` of a stream at [`RATE`]: 160 ms of carrier, then `bits` (a 1 goes from -1.1 to +1.1 rad),
/// unit power, carrier phase `phase`, `hz` from the dial.
fn add_a2(x: &mut [Complex32], at: usize, bits: &[bool], phase: f64, hz: f64) {
    let spb = RATE / a2::BIT_RATE;
    let nc = (0.160 * RATE) as usize;
    let total = nc + (bits.len() as f64 * spb) as usize;
    for (s, v) in x[at..].iter_mut().take(total).enumerate() {
        let m = if s < nc {
            0.0
        } else {
            let b = (s - nc) as f64 / spb;
            let first = if b.fract() < 0.5 { -a2::DEV } else { a2::DEV };
            if bits[(b as usize).min(bits.len() - 1)] {
                first
            } else {
                -first
            }
        };
        let a = phase + std::f64::consts::TAU * hz * s as f64 / RATE + m;
        *v += Complex32::from_polar(1.0, a as f32);
    }
}

/// The bursts of 8 trials identified (the platform and the message length; A2 has no checksum, so bit errors in the data
/// cannot be told), at `snr_db` in 5 kHz with the carrier `hz` from the dial: each trial sends an 80-bit and a 304-bit message.
fn a2_decoded(snr_db: f64, hz: f64) -> usize {
    let mut rng = Rng::new(40 + (10.0 * snr_db).abs() as u64);
    let noise = RATE / 5000.0 / 10f64.powf(snr_db / 10.0);
    let mut ok = 0;
    for trial in 0..8u64 {
        let mut x: Vec<Complex32> = (0..(3.2 * RATE) as usize)
            .map(|_| rng.complex_normal(noise))
            .collect();
        let mut want = Vec::new();
        for (i, (at, k)) in [(0.6, 0usize), (1.8, 7)].into_iter().enumerate() {
            let id = 0x0A2C000 + 16 * trial as u32 + i as u32;
            let data: Vec<bool> = (0..24 + 32 * k).map(|_| rng.bit() == 1).collect();
            let bits = a2::build(id, k, &data);
            want.push(a2::parse(&bits).unwrap());
            add_a2(
                &mut x,
                (at * RATE) as usize,
                &bits,
                std::f64::consts::TAU * rng.uniform(),
                hz,
            );
        }
        let run = harness().piece(32_768).feed(&x).unwrap();
        for w in &want {
            ok += usize::from(ptt(&run).iter().any(|e| {
                e.at(&ArgosPttEvent::PLATFORM) == Some(w.id_hex.as_str())
                    && e.at(&ArgosPttEvent::DATA_BITS) == Some(w.data_bits as i64)
            }));
        }
    }
    ok
}

/// A2 bursts of 80 and 304 bits at 4 to -6 dB in 5 kHz, still and with a carrier 3.1 kHz off: at least the share of 16
/// bursts that the specification requires (100, 100, 94, 63, 63, 25 % and 100, 94, 94, 44, 38, 19 %).
#[test]
fn a2_sensitivity() {
    let snrs = [4.0, 2.0, 0.0, -2.0, -4.0, -6.0];
    let need = [
        (0.0, [16, 16, 15, 10, 10, 4]),
        (3100.0, [16, 15, 15, 7, 6, 3]),
    ];
    let mut short = Vec::new();
    for (hz, need) in need {
        let got: Vec<usize> = snrs.iter().map(|&s| a2_decoded(s, hz)).collect();
        if got.iter().zip(&need).any(|(g, n)| g < n) {
            short.push(format!("{hz} Hz: {got:?} of 16, need {need:?}"));
        }
    }
    assert!(short.is_empty(), "{short:?}");
}

/// Noise alone, 150 runs of 6 s, gives no platform and lists no burst.
#[test]
fn a2_noise_gives_no_platform() {
    let mut rng = Rng::new(77);
    let mut bursts = 0;
    for run in 0..150 {
        let x: Vec<Complex32> = (0..(6.0 * RATE) as usize)
            .map(|_| rng.complex_normal(1.0))
            .collect();
        let got = harness().piece(32_768).feed(&x).unwrap();
        assert!(ptt(&got).is_empty(), "run {run}: {:?}", ptt(&got));
        bursts += row_count(&got.results, "bursts");
    }
    assert_eq!(bursts, 0, "bursts listed in noise");
}
