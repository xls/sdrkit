//! The stacks as an engine sees them: descriptors, the generator through the receiver at the stack's default rate, refusal of
//! low rates, retunes, clear, saving and loading results.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Owner, Receiver};
use sdrkit_dev_test::Harness;
use sdrkit_stack_cell::lte::gen::GEN_SFN0;
use sdrkit_stack_cell::{descriptors, CELL, EBICAB, GSMR, LBJ, NBIOT, NR};

#[test]
fn descriptors_are_complete() {
    let d = descriptors();
    let ids: Vec<&str> = d.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        vec!["cell", "nr", "nbiot", "gsmr", "lbj", "ebicab", "eot", "umts", "cdma2000", "tdscdma", "cv2x", "railway_cir"]
    );
    for m in &d {
        assert_eq!(m.group, "Cellular and railway");
        assert!(
            m.tuning.default_hz == 0.0
                || (m.tuning.min_hz <= m.tuning.default_hz && m.tuning.default_hz <= m.tuning.max_hz),
            "{}",
            m.id
        );
        assert!(m.tuning.min_rate_hz <= m.tuning.rate_hz, "{}", m.id);
        assert!(m.make_generator.is_some());
        assert!(!(m.settings)().is_empty());
    }
    assert_eq!(CELL.tuning.min_rate_hz, 1.92e6);
    assert_eq!(NR.tuning.min_rate_hz, 3.84e6);
    assert_eq!(NBIOT.tuning.min_rate_hz, 1e6);
    assert_eq!(start_hz(&GSMR), sdrkit_stack_cell::gsm::gen::CENTRE_HZ);
    assert_eq!(LBJ.tuning.tune_offset_hz, 50e3);
    assert_eq!(EBICAB.tuning.tune_offset_hz, 1e6);
}

/// What a run through a descriptor leaves: the saved state of the graph and its results.
struct Through {
    state: serde_json::Value,
    results: sdrkit_core::Results,
}

/// The stack's generator through the harness at the stack's default rate and frequency, `seconds` long, with `extra` settings.
fn through(
    desc: &'static sdrkit_core::StackDescriptor,
    seconds: f64,
    extra: &[CaseSetting],
) -> Through {
    let h = Harness::new(desc)
        .seconds(seconds)
        .seed(5)
        .with_settings(extra);
    let f = start_hz(desc);
    let x = h.generate().expect("the test signal");
    let mut rx = stack_graph(desc, &[]);
    let input = InputInfo::new(desc.tuning.rate_hz, f + desc.tuning.tune_offset_hz, f).recording();
    let s = run_graph(&mut rx, input, &x, &[262_144]);
    let r = Owner::results(&rx).clone();
    assert!(!r.status().summary.is_empty(), "{}", desc.id);
    Through {
        state: s,
        results: r,
    }
}

#[test]
fn lte_through_the_descriptor() {
    let run = through(&CELL, 0.45, &[CaseSetting::new(&GEN_SFN0, 4)]);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    let c = &cells(&run.state)[0];
    assert_eq!(c["earfcn"], 6300);
    assert_eq!(c["mib"]["n_rb"], 50);
    assert_eq!(c["sib1"]["plmns"][0], "001-01");
    assert_eq!(c["sib1"]["enb"], 54321);
    assert_eq!(c["sib1"]["sector"], 2);
}

#[test]
fn other_modes_through_their_descriptors() {
    let run = through(&NR, 0.09, &[]);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    assert!(cells(&run.state).iter().any(|c| c["pci"] == 501));
    let run = through(
        &NBIOT,
        1.05,
        &[
            CaseSetting::new(&GEN_NOISE, NoiseKind::Snr),
            CaseSetting::new(&GEN_SNR_DB, 20.0),
        ],
    );
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    assert_eq!(cells(&run.state)[0]["pci"], 100);
    let run = through(&LBJ, 3.0, &[]);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    let m = entries(&data(&run.state, "messages")["messages"]);
    assert_eq!(m.last().expect("a message")["train"], "10482");
    let run = through(&EBICAB, 0.1, &[]);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    assert!(
        data(&run.state, "telegrams")["counters"]["telegrams"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn low_rates_are_refused() {
    for (desc, msg) in [
        (&CELL, "LTE cells needs a sample rate of at least 1.92 Msps"),
        (&NR, "5G NR cells needs a sample rate of at least 3.84 Msps"),
        (&NBIOT, "NB-IoT needs a sample rate of at least 1 Msps"),
        (&GSMR, "GSM / GSM-R needs a sample rate of at least 1 Msps"),
    ] {
        let mut rx = stack_graph(desc, &[]);
        assert!(rx.configure(&InputInfo::new(0.5e6, 900e6, 900e6)).is_err());
        assert_eq!(rx.results().status().lock_state, LockState::Off);
        assert_eq!(rx.results().status().summary, msg);
    }
}

#[test]
fn gsmr_through_the_descriptor() {
    let run = Harness::new(&GSMR).seconds(2.3).run().expect("a run");
    let r = &run.results;
    assert_eq!(r.status().lock_state, LockState::Locked);
    let cells = r.table(r.decl().table_id("cells").unwrap()).unwrap();
    assert_eq!(cells.len(), 2);
    assert!(cells.rows_newest_first().any(|(_, row)| {
        row.at(&sdrkit_stack_cell::typed::GsmrCellRow::NCC) == Some(1)
            && row.at(&sdrkit_stack_cell::typed::GsmrCellRow::BCC) == Some(2)
    }));
}

/// A retune keeps the table, and a retune in the middle of a capture drops that capture. Clear empties the table.
#[test]
fn gsmr_retune_and_clear() {
    let x = Harness::new(&GSMR)
        .seconds(2.3)
        .generate()
        .expect("the test signal");
    let input = |c: f64| InputInfo::new(5e6, c, c).recording();
    let mut rx = stack_graph(&GSMR, &[]);
    rx.configure(&input(922.8e6)).unwrap();
    let mut out = Output::default();
    rx.feed(&x[..5_000_000], &mut out);
    rx.retuned(&input(925e6));
    rx.feed(&x[5_000_000..], &mut out);
    assert_eq!(value(rx.results(), "retunes"), Some(1.0));
    assert!(cells(&state(&rx)).is_empty());
    let mut rx = stack_graph(&GSMR, &[]);
    let s = run_graph(&mut rx, input(922.8e6), &x, &[]);
    assert_eq!(cells(&s).len(), 2);
    rx.retuned(&input(925e6));
    rx.feed(&x[..1000], &mut out);
    assert_eq!(cells(&state(&rx)).len(), 2);
    clear(&mut rx);
    assert!(cells(&state(&rx)).is_empty());
    assert_eq!(table_len(rx.results(), "cells"), 0);
}

/// A change of frequency by the host keeps the cells and their counts, and forgets the level, the capture centre and
/// the carriers of the newest capture.
#[test]
fn gsmr_forget_keeps_the_cells() {
    let x = Harness::new(&GSMR)
        .seconds(2.3)
        .generate()
        .expect("the test signal");
    let mut rx = stack_graph(&GSMR, &[]);
    run_graph(&mut rx, InputInfo::new(5e6, 922.8e6, 922.8e6).recording(), &x, &[]);
    let r = rx.results();
    assert_eq!(table_len(r, "cells"), 2);
    assert!(table_len(r, "carriers") >= 2);
    assert!(value(r, "level_dbfs").is_some());
    assert!(value(r, "capture_centre").is_some());
    let identified = value(r, "identified");
    rx.forget();
    let r = rx.results();
    assert_eq!(table_len(r, "cells"), 2);
    assert_eq!(value(r, "cell_count"), Some(2.0));
    assert_eq!(value(r, "identified"), identified);
    assert_eq!(table_len(r, "carriers"), 0);
    assert_eq!(value(r, "level_dbfs"), None);
    assert_eq!(value(r, "capture_centre"), None);
}

/// The NB-IoT test signal (scene 0) with noise for 20 dB in the carrier.
fn nbiot_signal(seconds: f64) -> Vec<Complex32> {
    let g = sdrkit_stack_cell::nbiot::gen::NbGenerator::new(
        1.92e6,
        sdrkit_stack_cell::nbiot::gen::scene(0),
        Default::default(),
    );
    impaired(g, &snr(20.0), seconds)
}

#[test]
fn retune_clear_save_and_load() {
    let x = nbiot_signal(1.0);
    let mut noise = sdrkit_algo_sim::rng::Rng::new(4);
    let quiet: Vec<Complex32> = (0..1_920_000)
        .map(|_| noise.complex_normal(2.0 * 0.1 * 0.1))
        .collect();
    let at = |f: f64| InputInfo::new(1.92e6, f, f).recording();
    // A retune in the middle of a capture drops it: nothing is placed from the half capture.
    let mut rx = stack_graph(&NBIOT, &[]);
    rx.configure(&at(945e6)).unwrap();
    let mut out = Output::default();
    rx.feed(&x[..1_000_000], &mut out);
    rx.retuned(&at(946e6));
    rx.feed(&quiet, &mut out);
    assert!(cells(&state(&rx)).is_empty());
    assert_eq!(value(rx.results(), "retunes"), Some(1.0));
    // The table survives a retune; Clear empties it.
    let mut rx = stack_graph(&NBIOT, &[]);
    rx.configure(&at(945e6)).unwrap();
    rx.feed(&x, &mut out);
    assert_eq!(cells(&state(&rx)).len(), 1);
    rx.retuned(&at(950e6));
    rx.feed(&quiet[..1000], &mut out);
    assert_eq!(cells(&state(&rx)).len(), 1);
    // Save, clear, restore.
    let saved = rx.save_state().expect("a saved state");
    clear(&mut rx);
    assert!(cells(&state(&rx)).is_empty());
    rx.restore_state(&saved).unwrap();
    let cs = cells(&state(&rx));
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["pci"], 100);
    // A state of another stack is refused and changes nothing.
    let mut nr = stack_graph(&NR, &[]);
    nr.configure(&InputInfo::new(7.68e6, 3541.44e6, 3541.44e6).recording())
        .unwrap();
    assert!(nr.restore_state(&saved).is_err());
    assert!(cells(&state(&nr)).is_empty());
}

/// A file source is never skipped: a recording fed in pieces of any size has every capture searched and returns the same table
/// as one piece.
#[test]
fn recordings_never_skip_a_capture() {
    let x = nbiot_signal(3.0);
    let input = InputInfo::new(1.92e6, 945e6, 945e6).recording();
    let one = run_mode(&NBIOT, &[], input, &x, &[]);
    assert_eq!(one["captures"], 3);
    for seed in [1u64, 2] {
        let d = run_mode(&NBIOT, &[], input, &x, &random_pieces(seed, x.len()));
        assert_eq!(d["captures"], 3, "seed {seed}");
        assert_same(&d, &one);
    }
}
