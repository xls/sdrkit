//! The slot clock on the stack's own test signal, faster than real time. The receiver reads the wall clock of its run
//! context once, at the first sample, and counts samples after it, so a fixed clock stands for the time the stream starts.
//! A live test signal starts at that time; the default one starts at a slot start and is found by slot search.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Clock, Complex32, Event, Generator, InputInfo, Output, Receiver, StackConfig, Value, Values,
};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_stack_wsjt::generator::{stations, WsjtGenerator};
use sdrkit_stack_wsjt::typed::{DECODE, JS8_FRAME};
use sdrkit_stack_wsjt::{receiver, Mode, Sub};

/// The rate of the tests.
const RATE: f64 = 12000.0;
/// 2026-01-01T00:00:00Z, a start of every slot length of the family.
const BASE: f64 = 1_767_225_600.0;

/// The receiver of `mode` with default settings and `gen_live` as given, on a recording whose first sample is at `utc`.
fn receiver_at(mode: Mode, live: bool, utc: f64) -> StackGraph {
    let schema = sdrkit_stack_wsjt::settings::schema_for(Some(mode));
    let mut s = Values::defaults(&schema);
    s.set(schema.id("gen_live").unwrap(), &Value::Bool(live))
        .unwrap();
    let mut rx = receiver::receiver(Some(mode), &StackConfig::new(s)).unwrap();
    rx.set_context(RunContext {
        clock: Clock::fixed(utc),
        ..RunContext::default()
    });
    rx.configure(&InputInfo::new(RATE, 14e6, 14e6).recording())
        .unwrap();
    rx
}

/// Feeds `g` to `rx` for up to `seconds`, and returns the decode or JS8 frame events of the first block that has any.
fn first_decodes(rx: &mut StackGraph, g: &mut dyn Generator, seconds: f64) -> Option<Vec<Event>> {
    let piece = 1200;
    let mut buf = vec![Complex32::default(); piece];
    let mut out = Output::default();
    for _ in 0..(seconds * RATE / piece as f64) as usize {
        g.generate(&mut buf);
        rx.feed(&buf, &mut out);
        let ev: Vec<Event> = out
            .events
            .drain(..)
            .filter(|e| e.is(&DECODE) || e.is(&JS8_FRAME))
            .collect();
        out.clear();
        if !ev.is_empty() {
            return Some(ev);
        }
    }
    None
}

/// The generator of `mode` with the default list, its first sample at `utc` when given.
fn generator(mode: Mode, utc: Option<f64>) -> WsjtGenerator {
    let mut g = if mode == Mode::Js8 {
        WsjtGenerator::js8(RATE, &sdrkit_stack_wsjt::generator::js8_stations()).unwrap()
    } else {
        WsjtGenerator::new(Sub::new(mode), RATE, false, &Default::default()).unwrap()
    };
    if let Some(t) = utc {
        g.start_at(t);
    }
    g
}

/// Every decode lies in one of `slots`; with `exact`, the DT of each decode of a listed station is its DT in the list within
/// 0.2 s. Returns the DT of each such decode less its DT in the list.
fn check_slot(mode: Mode, ev: &[Event], slots: &[f64], exact: bool, what: &str) -> Vec<f64> {
    let mut errors = Vec::new();
    for e in ev {
        assert!(
            slots.iter().any(|s| (e.time - s).abs() < 1e-6),
            "{mode:?} {what}: slot {} where {slots:?}",
            e.time
        );
        if !e.is(&DECODE) {
            continue;
        }
        let text = e
            .get(&sdrkit_core::publish::keys::TEXT)
            .unwrap_or_default()
            .to_string();
        if let Some(st) = stations(mode).iter().find(|s| s.0 == text) {
            let dt = num(e, "dt_s");
            assert!(
                !exact || (dt - st.3).abs() < 0.2,
                "{mode:?} {what}: {text} DT {dt} where {}",
                st.3
            );
            errors.push(dt - st.3);
        }
    }
    errors
}

/// The periods of the stations of `mode`'s test signal (JS8: its three speeds).
fn periods(mode: Mode) -> Vec<f64> {
    if mode == Mode::Js8 {
        (0..3).map(|s| Sub::new(mode).with_sub(s).period).collect()
    } else {
        vec![Sub::new(mode).period]
    }
}

/// A live test signal of `mode` from a stream that starts at a slot start, in the middle of a slot and 0.3 s before
/// one: the stations decode in the first slot whose transmissions lie whole in the stream, at their DT.
fn live(mode: Mode) {
    let period = Sub::new(mode).period;
    for (what, into) in [
        ("at the slot start", 0.0),
        ("in the middle", period / 2.0),
        ("0.3 s before a slot", period - 0.3),
    ] {
        let t = BASE + 2.0 * period + into;
        let slot = (t / period).ceil() * period;
        let firsts: Vec<f64> = periods(mode).iter().map(|p| (t / p).ceil() * p).collect();
        let mut rx = receiver_at(mode, true, t);
        let mut g = generator(mode, Some(t));
        let ev = first_decodes(&mut rx, &mut g, slot - t + period + 5.0)
            .unwrap_or_else(|| panic!("{mode:?} {what}: no decode"));
        let matched = check_slot(mode, &ev, &firsts, true, what);
        assert!(
            mode == Mode::Js8 || !matched.is_empty(),
            "{mode:?} {what}: no listed station"
        );
        assert_eq!(choice_key(rx.results(), "time_source"), "system_clock");
    }
}

/// The default test signal of `mode`, whose first sample is a slot start, from a stream whose wall clock is not one: slot
/// search finds the slots, and the first decodes shown come within `limit_s` of signal. Slot search puts the median start of
/// the decodes it found on the slot start, so every DT moves by one shared amount.
fn default(mode: Mode, limit_s: f64) {
    let period = Sub::new(mode).period;
    let mut rx = receiver_at(mode, false, BASE + 0.37 * period);
    let mut g = generator(mode, None);
    let ev = first_decodes(&mut rx, &mut g, limit_s)
        .unwrap_or_else(|| panic!("{mode:?}: no decode in {limit_s} s"));
    if mode == Mode::Js8 {
        // JS8 frames do not move the slot clock: the decoder searches its whole DT range on any grid.
        return;
    }
    assert_eq!(
        choice_key(rx.results(), "time_source"),
        "found_from_signals",
        "{mode:?}"
    );
    let errors = check_slot(mode, &ev, &[ev[0].time], false, "default");
    let (lo, hi) = errors
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &e| (a.min(e), b.max(e)));
    assert!(
        !errors.is_empty() && hi - lo < 0.3 && lo > -0.6 && hi < 0.6,
        "{mode:?}: DT errors {errors:?}"
    );
}

macro_rules! modes {
    ($($live:ident, $default:ident, $mode:expr, $limit_s:expr;)*) => {$(
        #[test]
        fn $live() {
            live($mode);
        }
        #[test]
        fn $default() {
            default($mode, $limit_s);
        }
    )*};
}

// The limits: one slot search window of the mode (a slot and a half for the slow modes) and its next slot.
modes! {
    ft8_live, ft8_default, Mode::Ft8, 35.0;
    ft4_live, ft4_default, Mode::Ft4, 18.0;
    ft2_live, ft2_default, Mode::Ft2, 10.0;
    js8_live, js8_default, Mode::Js8, 35.0;
    jt65_live, jt65_default, Mode::Jt65, 125.0;
    jt9_live, jt9_default, Mode::Jt9, 125.0;
    jt4_live, jt4_default, Mode::Jt4, 125.0;
    wspr_live, wspr_default, Mode::Wspr, 260.0;
}
