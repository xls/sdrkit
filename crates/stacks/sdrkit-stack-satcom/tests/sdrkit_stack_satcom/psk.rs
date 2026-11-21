//! PSK demodulator: blind analysis and demodulation of the generator's signals through the standard
//! impairments, the bit chain with the ambiguity search, signals that must never lock, and the declared results and events.

use crate::common::{cell_num, field_text, number, rows, set};
use sdrkit_algo_modem::psk::{PskOrder, PskPulse};
use sdrkit_core::{Choice, Complex32, Event, InputInfo, Output, Results};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satcom::modes::psk::bits::{Descrambler, Differential};
use sdrkit_stack_satcom::modes::psk::{self, gen};
use serde_json::json;
use std::collections::HashSet;

const CENTRE: f64 = 435.25e6;

/// The results of a run.
struct Got {
    r: Results,
    events: Vec<Event>,
}

impl Got {
    fn num(&self, key: &str) -> f64 {
        self.r
            .decl()
            .field_id(key)
            .and_then(|id| self.r.fields().value(id))
            .and_then(number)
            .unwrap_or(f64::NAN)
    }
    fn text(&self, key: &str) -> String {
        field_text(&self.r, key)
    }
    fn syncs(&self) -> Vec<u64> {
        let mut v: Vec<u64> = rows(&self.r, "syncs")
            .iter()
            .filter_map(|(_, r)| cell_num(r, "bit"))
            .map(|b| b as u64)
            .collect();
        v.sort_unstable();
        v
    }
    fn locked(&self) -> bool {
        self.r.status().lock_state == sdrkit_core::LockState::Locked
    }
    fn log(&self) -> Vec<String> {
        self.r.log_lines().iter().map(|l| l.to_string()).collect()
    }
}

fn order_name(o: PskOrder) -> sdrkit_stack_satcom::modes::psk::gen::GenOrder {
    match o {
        PskOrder::Bpsk => sdrkit_stack_satcom::modes::psk::gen::GenOrder::Bpsk,
        PskOrder::Qpsk => sdrkit_stack_satcom::modes::psk::gen::GenOrder::Qpsk,
        PskOrder::Oqpsk => sdrkit_stack_satcom::modes::psk::gen::GenOrder::Oqpsk,
        PskOrder::Psk8 => sdrkit_stack_satcom::modes::psk::gen::GenOrder::Psk8,
    }
}

fn coding_name(
    diff: Differential,
    descr: Descrambler,
) -> sdrkit_stack_satcom::modes::psk::gen::GenCoding {
    match (diff, descr) {
        (Differential::Dpsk, _) => sdrkit_stack_satcom::modes::psk::gen::GenCoding::Dpsk,
        (Differential::NrzM, _) => sdrkit_stack_satcom::modes::psk::gen::GenCoding::NrzM,
        (Differential::NrzS, _) => sdrkit_stack_satcom::modes::psk::gen::GenCoding::NrzS,
        (_, Descrambler::G3ruh) => sdrkit_stack_satcom::modes::psk::gen::GenCoding::G3ruh,
        (_, Descrambler::Ccsds) => sdrkit_stack_satcom::modes::psk::gen::GenCoding::Ccsds,
        _ => sdrkit_stack_satcom::modes::psk::gen::GenCoding::None,
    }
}

/// The harness for the test signal of `p`, with the receiver's sync word set.
fn harness(p: &gen::GenParams, seconds: f64) -> Harness {
    let rolloff = match p.shape {
        PskPulse::Rect => 0.0,
        PskPulse::Rrc(b) => b,
    };
    Harness::new(&psk::DESCRIPTOR)
        .rate(p.rate)
        .freq(CENTRE)
        .seconds(seconds)
        .seed(p.seed)
        .set(
            &sdrkit_stack_satcom::modes::psk::gen::GEN_ORDER,
            order_name(p.order),
        )
        .set(&sdrkit_stack_satcom::modes::psk::gen::GEN_BAUD, p.baud)
        .set(&sdrkit_stack_satcom::modes::psk::gen::GEN_ROLLOFF, rolloff)
        .set(
            &sdrkit_stack_satcom::modes::psk::gen::GEN_CODING,
            coding_name(p.diff, p.descr),
        )
        .set(
            &sdrkit_stack_satcom::modes::psk::gen::GEN_CONJUGATE,
            p.conjugate,
        )
        .set(&sdrkit_stack_satcom::modes::psk::bits::SYNC, "1ACFFC1D")
}

/// The offset of the dial from the radio's centre: the tune offset below it, or an eighth of the rate at low rates.
fn dial(rate: f64) -> f64 {
    -psk::DESCRIPTOR.tuning.tune_offset_hz.min(rate / 8.0)
}

/// Feeds the test signal `x` (its carrier at 0 Hz) to the harness's receiver with the signal and the dial `dial(rate)` from the
/// centre, in pieces of `piece`.
fn feed(h: &Harness, p: &gen::GenParams, x: &[Complex32], piece: usize) -> Got {
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(p.rate, dial(p.rate));
    let x: Vec<Complex32> = x.iter().map(|v| v * nco.next_sample()).collect();
    let mut rx = h.receiver().expect("the settings");
    rx.configure(&InputInfo::new(p.rate, CENTRE, CENTRE + dial(p.rate)).recording())
        .expect("the rate is accepted");
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(piece) {
        out.clear();
        rx.feed(c, &mut out);
        events.append(&mut out.events);
    }
    let mut r = Results::default();
    r.clone_from(rx.results());
    Got { r, events }
}

fn run(h: Harness, p: &gen::GenParams) -> Got {
    let x = h.generate().expect("the test signal");
    feed(&h, p, &x, 32_768)
}

/// The payloads the receiver returned after its sync words that match frames the generator sent: (good, all).
fn payloads_ok(p: &gen::GenParams, g: &Got) -> (usize, usize) {
    let sent: HashSet<Vec<u8>> = (0..20_000u64)
        .map(|k| gen::payload(p.seed, k, p.payload_bits))
        .collect();
    let bits: Vec<u8> = g.text("bit_stream").bytes().map(|c| c - b'0').collect();
    let first = g.num("bits_first") as u64;
    let (mut ok, mut total) = (0, 0);
    for s in g.syncs() {
        if s < first {
            continue;
        }
        let i = (s - first) as usize;
        if i + p.payload_bits > bits.len() {
            continue;
        }
        total += 1;
        if sent.contains(&bits[i..i + p.payload_bits]) {
            ok += 1;
        }
    }
    (ok, total)
}

#[test]
fn p1_blind_analysis_over_orders_rates_and_shapes() {
    let cases = [
        (2e6, PskOrder::Bpsk, 1200.0, PskPulse::Rrc(0.35), 4.0),
        (2e6, PskOrder::Qpsk, 4800.0, PskPulse::Rrc(0.35), 2.0),
        (2e6, PskOrder::Oqpsk, 2400.0, PskPulse::Rrc(0.5), 3.0),
        (2e6, PskOrder::Psk8, 9600.0, PskPulse::Rrc(0.35), 2.0),
        (2e6, PskOrder::Bpsk, 1200.0, PskPulse::Rect, 4.0),
        (2e6, PskOrder::Qpsk, 9600.0, PskPulse::Rect, 2.0),
        (48e3, PskOrder::Qpsk, 300.0, PskPulse::Rrc(0.35), 12.0),
    ];
    for (rate, order, baud, shape, secs) in cases {
        let mut p = gen::GenParams::new(rate);
        p.order = order;
        p.baud = baud;
        p.shape = shape;
        let esn0 = if order == PskOrder::Psk8 { 21.0 } else { 16.0 };
        let h = harness(&p, secs)
            .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 777.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EsN0,
            )
            .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, esn0);
        let x = h.generate().expect("the test signal");
        let g = feed(&h, &p, &x, 32_768);
        let what = format!("{order:?} {baud} {shape:?} at {rate}");
        assert!(g.locked(), "{what}: {:?}", g.log());
        assert_eq!(g.text("order"), order_name(order).key(), "{what}");
        let b = g.num("symbol_rate_hz");
        assert!((b - baud).abs() < 0.005 * baud, "{what}: {b}");
        let c = g.num("carrier_hz");
        assert!((c - 777.0).abs() < (3.0f64).max(0.01 * baud), "{what}: {c}");
        if shape == PskPulse::Rect && order != PskOrder::Oqpsk {
            assert_eq!(g.text("shape"), "rect", "{what}");
        }
        let (ok, total) = payloads_ok(&p, &g);
        assert!(ok >= 1 && total - ok <= 2, "{what}: {ok} of {total}");
    }
}

#[test]
fn p1_doppler_ramp_clock_error_and_off_centre() {
    let mut p = gen::GenParams::new(2e6);
    p.order = PskOrder::Qpsk;
    p.baud = 4800.0;
    let h = harness(&p, 4.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, -3000.0)
        .set(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, 300.0)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 40.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EsN0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 15.0);
    let g = run(h, &p);
    assert!(g.locked(), "{:?}", g.log());
    // The carrier at the end: the offset and the ramp over 4 s.
    let want = -3000.0 + 300.0 * 4.0;
    let c = g.num("carrier_hz");
    assert!((c - want).abs() < 60.0, "{c} vs {want}");
    let d = g.num("drift_hz_s");
    assert!((d - 300.0).abs() < 0.35 * 300.0, "{d}");
    let (ok, total) = payloads_ok(&p, &g);
    assert!(ok >= 5 && total - ok <= 2, "{ok} of {total}");
}

/// A symbol clock 3 % off the rate set is followed.
#[test]
fn a_symbol_clock_three_percent_off_is_followed() {
    let mut p = gen::GenParams::new(2e6);
    p.order = PskOrder::Bpsk;
    p.baud = 2472.0; // 3 % above the rate the receiver is told
    let h = harness(&p, 4.0)
        .set(&sdrkit_stack_satcom::modes::psk::rx::BAUD, 2400.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EsN0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 18.0);
    let g = run(h, &p);
    assert!(g.locked(), "{:?}", g.log());
    let b = g.num("symbol_rate_hz");
    assert!((b - 2472.0).abs() < 5.0, "{b}");
    let (ok, total) = payloads_ok(&p, &g);
    assert!(ok >= 3 && total - ok <= 2, "{ok} of {total}");
}

#[test]
fn p2_every_order_with_every_coding() {
    for order in [PskOrder::Bpsk, PskOrder::Qpsk, PskOrder::Psk8] {
        for (diff, descr) in [
            (Differential::None, Descrambler::None),
            (Differential::Dpsk, Descrambler::None),
            (Differential::NrzM, Descrambler::None),
            (Differential::NrzS, Descrambler::None),
            (Differential::None, Descrambler::G3ruh),
            (Differential::None, Descrambler::Ccsds),
        ] {
            let mut p = gen::GenParams::new(2e6);
            p.order = order;
            p.baud = 9600.0;
            p.diff = diff;
            p.descr = descr;
            p.conjugate = order == PskOrder::Qpsk;
            let c = coding_name(diff, descr).key();
            let mut h = harness(&p, 1.5)
                .set(
                    &sdrkit_algo_sim::chain::GEN_NOISE,
                    sdrkit_algo_sim::chain::NoiseKind::EsN0,
                )
                .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 20.0);
            if diff != Differential::None {
                h = h.set(&sdrkit_stack_satcom::modes::psk::bits::DIFFERENTIAL, diff);
            }
            if descr != Descrambler::None {
                h = h.set(&sdrkit_stack_satcom::modes::psk::bits::DESCRAMBLER, descr);
            }
            let g = run(h, &p);
            let (ok, total) = payloads_ok(&p, &g);
            assert!(
                ok >= 3 && total - ok <= 2,
                "{order:?} {c}: {ok} of {total}, {:?}",
                g.log()
            );
        }
    }
}

#[test]
fn p3_radio_error_recording_rate_and_cut_independence() {
    // A 48 kHz recording with the radio 1.5 kHz off.
    let mut p = gen::GenParams::new(48e3);
    p.order = PskOrder::Bpsk;
    p.baud = 1200.0;
    let h = harness(&p, 4.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 1500.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EsN0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 14.0);
    let x = h.generate().expect("the test signal");
    let a = feed(&h, &p, &x, 1_000);
    let b = feed(&h, &p, &x, 48_000);
    assert!(a.locked());
    let c = a.num("carrier_hz");
    assert!((c - 1500.0).abs() < 12.0, "{c}");
    assert_eq!(a.text("bit_stream"), b.text("bit_stream"));
    assert_eq!(a.syncs(), b.syncs());
    let (ok, total) = payloads_ok(&p, &a);
    assert!(ok >= 5 && total - ok <= 2, "{ok} of {total}");
    // A clock error at 2 Msps.
    let mut p = gen::GenParams::new(2e6);
    p.order = PskOrder::Qpsk;
    p.baud = 2400.0;
    let g = run(
        harness(&p, 3.0)
            .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, 100.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EsN0,
            )
            .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 20.0),
        &p,
    );
    assert!(g.locked());
    let b = g.num("symbol_rate_hz");
    assert!((b - 2400.0).abs() < 2.4, "{b}");
}

#[test]
fn noise_and_an_unmodulated_carrier_never_lock() {
    let rate = 2e6;
    let p = gen::GenParams::new(rate);
    // Noise alone: the generator's signal 100 dB under the floor.
    let h = harness(&p, 3.0)
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -100.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -26.0);
    let mut x = h.generate().expect("the noise");
    let g = feed(&h, &p, &x, 65_536);
    assert!(!g.locked());
    assert_eq!(g.num("frames_ok"), 0.0);
    // A carrier 700 Hz above the dial (`feed` moves 0 Hz to the dial).
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(rate, 700.0);
    for v in x.iter_mut() {
        *v += nco.next_sample() * 0.2;
    }
    let g = feed(&h, &p, &x, 65_536);
    assert!(!g.locked(), "{:?}", g.log());
}

#[test]
fn results_and_events_are_declared() {
    let p = gen::GenParams::new(2e6);
    let g = run(
        harness(&p, 2.0)
            .set(&sdrkit_stack_satcom::modes::psk::display::SAVE_BITS, true)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EsN0,
            )
            .set(&sdrkit_algo_sim::chain::GEN_ESN0_DB, 20.0),
        &p,
    );
    use sdrkit_stack_satcom::typed::{
        PSK_DEMOD_BYTES as BYTES_EVENT, PSK_DEMOD_LOCK as LOCK_EVENT,
    };
    assert!(g.events.iter().any(|e| e.is(&LOCK_EVENT)));
    assert!(g.events.iter().any(|e| e.is(&BYTES_EVENT)));
    for e in &g.events {
        assert!(e.is(&LOCK_EVENT) || e.is(&BYTES_EVENT), "{e:?}");
        assert!(e.freq_hz > 0.0, "{e:?}");
    }
}

#[test]
fn bad_settings_are_refused() {
    let mut rx = Harness::new(&psk::DESCRIPTOR).receiver().unwrap();
    let rx = rx.as_mut();
    assert!(set(rx, "order", json!("16qam")).is_err());
    assert!(set(rx, "span", json!("1000")).is_err());
    assert!(set(rx, "span", json!("wide")).is_err());
    assert!(set(rx, "click_hz", json!("here")).is_err());
    assert!(set(rx, "sync", json!("1ACFZZ")).is_err());
    assert!(set(rx, "differential", json!("manchester")).is_err());
    assert!(set(rx, "span", json!("50000")).is_ok());
    assert!(set(rx, "click_hz", json!("-1200")).is_ok());
}
