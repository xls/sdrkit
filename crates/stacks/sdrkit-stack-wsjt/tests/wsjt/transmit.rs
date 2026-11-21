//! The transmit side through the stack's own transmitter: what the transmit sink receives decodes as what was sent, it
//! starts at the start of its slot, and `halt` ends it.

use crate::common::{feed, receiver, shift, texts};
use sdrkit_core::publish::keys;
use sdrkit_core::{owner, Choice, Complex32, Output, Owner, Receiver, Station, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_stack_wsjt::components::Js8Speed;
use sdrkit_stack_wsjt::typed::{JS8_FRAME, JS8_MESSAGE};
use sdrkit_stack_wsjt::{Mode, Sub};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One write of the transmit sink: the input samples fed before it, the samples, the centre and the end mark.
type Write = (usize, Vec<Complex32>, f64, bool);

/// One burst of the transmit sink: the input samples fed before it, its samples, and whether its end mark came.
type Burst = (usize, Vec<Complex32>, bool);

/// The transmit stream of a test: each write with the number of input samples fed when it came.
#[derive(Default)]
struct Sink {
    fed: AtomicUsize,
    writes: Mutex<Vec<Write>>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                centre_hz,
                end,
                ..
            },
        ) = (port, data)
        {
            let at = self.fed.load(Ordering::Relaxed);
            self.writes
                .lock()
                .unwrap()
                .push((at, samples.to_vec(), centre_hz, end));
        }
    }
}

/// A transmitting stack of `mode` at `rate` for the station K1ABC in FN42, with extra settings, and its transmit stream.
fn station_stack(mode: Mode, rate: f64, extra: &[(&str, Value)]) -> (StackGraph, Arc<Sink>) {
    station_stack_at(mode, rate, 0.0, extra)
}

/// As [`station_stack`], with the dial `offset` Hz above the centre of the samples.
fn station_stack_at(
    mode: Mode,
    rate: f64,
    offset: f64,
    extra: &[(&str, Value)],
) -> (StackGraph, Arc<Sink>) {
    station_stack_of(Some(mode), rate, offset, extra)
}

/// As [`station_stack_at`], for `mode` or (`None`) the combined stack.
fn station_stack_of(
    mode: Option<Mode>,
    rate: f64,
    offset: f64,
    extra: &[(&str, Value)],
) -> (StackGraph, Arc<Sink>) {
    let mut rx = receiver(mode, extra, rate, offset);
    let sink = Arc::new(Sink::default());
    rx.set_context(RunContext {
        streams: Some(sink.clone()),
        ..RunContext::default()
    });
    rx.set_station(&Station {
        callsign: "K1ABC".into(),
        locator: "FN42".into(),
        ..Station::default()
    });
    rx.set_transmit(true);
    (rx, sink)
}

/// Runs command `key` without arguments.
fn call(rx: &mut StackGraph, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    owner::invoke(rx, id, &args).unwrap();
}

/// Feeds silence from `from` to `to` seconds at `rate` in pieces of 0.1 s, counting the samples fed in `sink`.
fn silence(rx: &mut StackGraph, sink: &Sink, rate: f64, from: f64, to: f64) {
    let piece = (rate / 10.0) as usize;
    let zeros = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut t = from;
    while t < to {
        sink.fed
            .store((t * rate).round() as usize, Ordering::Relaxed);
        rx.feed(&zeros, &mut out);
        out.clear();
        t += 0.1;
    }
}

/// The bursts of the transmit stream: where each starts in the input, its samples, and whether its end mark came.
fn bursts(sink: &Sink) -> Vec<Burst> {
    let mut out: Vec<Burst> = Vec::new();
    let mut open = false;
    for (at, samples, _, end) in sink.writes.lock().unwrap().iter() {
        if !open {
            out.push((*at, Vec::new(), false));
        }
        let b = out.last_mut().unwrap();
        b.1.extend_from_slice(samples);
        b.2 = *end;
        open = !end;
    }
    out
}

#[test]
fn ft8_cq_goes_on_air_in_its_slot_decodes_and_halt_ends_it() {
    let rate = 48_000.0;
    let (mut rx, sink) = station_stack(Mode::Ft8, rate, &[]);
    assert!(rx.transmits());
    call(&mut rx, "cq");
    silence(&mut rx, &sink, rate, 0.0, 45.0);
    let first = bursts(&sink);
    assert_eq!(first.len(), 1, "one CQ in the first even slot");
    let (at, samples, ended) = &first[0];
    assert!(ended, "the burst has its end mark");
    // The burst starts at 30 s plus the start offset of FT8, within one piece of input.
    let start = 30.0 + Sub::new(Mode::Ft8).t0();
    let at_s = *at as f64 / rate;
    assert!((at_s - start).abs() <= 0.15, "starts at {at_s} s");
    let len_s = samples.len() as f64 / rate;
    assert!((len_s - 12.64).abs() < 0.05, "lasts {len_s} s");
    let peak = samples.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
    assert!(peak <= 0.51 && peak > 0.45, "peak {peak}");
    for (_, _, centre, _) in sink.writes.lock().unwrap().iter() {
        assert_eq!(*centre, 14e6, "centred on the dial");
    }
    // The samples of the transmit sink, where they were sent, decode with the stack's own receiver.
    let mut air = vec![Complex32::default(); (45.0 * rate) as usize];
    air[*at..*at + samples.len()].copy_from_slice(samples);
    let mut listener = receiver(Some(Mode::Ft8), &[], rate, 0.0);
    let heard = texts(&feed(&mut listener, &air, 4800));
    assert_eq!(heard, ["CQ K1ABC FN42"]);
    // The next CQ starts at 60.5 s; halt at 62 s ends it at once.
    silence(&mut rx, &sink, rate, 45.0, 62.0);
    call(&mut rx, "halt");
    silence(&mut rx, &sink, rate, 62.0, 80.0);
    let all = bursts(&sink);
    assert_eq!(all.len(), 2, "no CQ after halt");
    let (at, samples, ended) = &all[1];
    assert!(ended);
    let stop_s = (*at + samples.len()) as f64 / rate;
    assert!(
        (62.0..62.25).contains(&stop_s),
        "the second burst ends at {stop_s} s"
    );
}

#[test]
fn the_wspr_beacon_sends_in_its_slot_until_halt() {
    let rate = 12_000.0;
    let (mut rx, sink) = station_stack(Mode::Wspr, rate, &[("beacon_every", Value::I64(1))]);
    call(&mut rx, "beacon");
    silence(&mut rx, &sink, rate, 0.0, 125.0);
    call(&mut rx, "halt");
    silence(&mut rx, &sink, rate, 125.0, 250.0);
    let b = bursts(&sink);
    assert_eq!(b.len(), 1, "one beacon, then halt");
    let (at, samples, ended) = &b[0];
    assert!(ended);
    let at_s = *at as f64 / rate;
    assert!(
        (at_s - (120.0 + Sub::new(Mode::Wspr).t0())).abs() <= 0.15,
        "starts at {at_s} s"
    );
    let stop_s = (*at + samples.len()) as f64 / rate;
    assert!(
        (125.0..125.25).contains(&stop_s),
        "halt ends it at {stop_s} s"
    );
}

#[test]
fn a_stack_without_a_transmitter_on_has_nothing_on_air() {
    let rate = 12_000.0;
    let (mut rx, sink) = station_stack(Mode::Ft8, rate, &[]);
    call(&mut rx, "cq");
    rx.set_transmit(false);
    silence(&mut rx, &sink, rate, 0.0, 45.0);
    assert!(sink.writes.lock().unwrap().is_empty());
}

#[test]
fn each_stack_has_the_transmit_commands_of_its_modes() {
    let keys = |mode: Option<Mode>| -> Vec<String> {
        let rx = sdrkit_stack_wsjt::receiver::receiver(
            mode,
            &sdrkit_core::StackConfig::new(crate::common::settings(mode, &[])),
        )
        .unwrap();
        let defs = rx.commands();
        let mut k: Vec<String> = defs
            .defs()
            .iter()
            .filter(|d| d.transmits.is_some() || d.key == "halt")
            .map(|d| {
                format!(
                    "{}{}",
                    d.key,
                    d.stopped_by
                        .as_deref()
                        .map_or(String::new(), |s| format!(">{s}"))
                )
            })
            .collect();
        k.sort();
        assert_eq!(rx.transmits(), !k.is_empty(), "{mode:?}");
        k
    };
    assert_eq!(keys(Some(Mode::Ft8)), ["cq>halt", "halt"]);
    assert_eq!(keys(Some(Mode::Jt65)), ["cq>halt", "halt"]);
    assert_eq!(keys(Some(Mode::Wspr)), ["beacon>halt", "halt"]);
    assert_eq!(keys(Some(Mode::Js8)), ["halt", "send>halt"]);
    assert_eq!(keys(None), ["beacon>halt", "cq>halt", "halt", "send>halt"]);
    for m in [Mode::Q65, Mode::Fst4, Mode::Fst4w, Mode::Msk144] {
        assert!(keys(Some(m)).is_empty(), "{m:?}");
    }
}

/// The rate of the round trips: the lowest common rate of the decoders, to keep the tests short.
const TRIP_RATE: f64 = 12_000.0;

/// Starts a transmission with `start` on a stack of `mode` (settings `extra`, dial `offset` Hz above the centre), feeds
/// silence to `run_s` seconds in pieces of 0.1 s, puts each burst of the transmit sink where it was sent, and returns the
/// bursts, the centres of the sink and the texts that a new stack of `mode` with the same settings decodes from them.
fn round_trip(
    mode: Mode,
    extra: &[(&str, Value)],
    offset: f64,
    run_s: f64,
    start: impl FnOnce(&mut StackGraph),
) -> (Vec<Burst>, Vec<f64>, Vec<String>) {
    let rate = TRIP_RATE;
    let (mut rx, sink) = station_stack_at(mode, rate, offset, extra);
    start(&mut rx);
    silence(&mut rx, &sink, rate, 0.0, run_s);
    let b = bursts(&sink);
    let centres = sink.writes.lock().unwrap().iter().map(|w| w.2).collect();
    let mut air = vec![Complex32::default(); (run_s * rate) as usize + 1];
    for (at, samples, _) in &b {
        let n = samples.len().min(air.len() - at);
        air[*at..*at + n].copy_from_slice(&samples[..n]);
    }
    // The sink's samples are centred on the dial; the listener's centre is `offset` below it.
    shift(&mut air, rate, offset);
    let mut listener = receiver(Some(mode), extra, rate, offset);
    let ev = feed(&mut listener, &air, 1200);
    let mut heard = texts(&ev);
    heard.extend(
        ev.iter()
            .filter(|e| e.is(&JS8_FRAME) || e.is(&JS8_MESSAGE))
            .map(|e| e.get(&keys::TEXT).unwrap_or_default().to_string()),
    );
    (b, centres, heard)
}

/// Each burst ended with its end mark and started at the start of a slot of `sub` plus its start offset.
fn on_slot(b: &[Burst], sub: Sub) {
    assert!(!b.is_empty(), "nothing sent");
    for (at, samples, ended) in b {
        assert!(ended, "a burst without its end mark");
        let t = *at as f64 / TRIP_RATE - sub.t0();
        let off = t - (t / sub.period).round() * sub.period;
        assert!(off.abs() <= 0.15, "starts {off} s from its slot");
        let len_s = samples.len() as f64 / TRIP_RATE;
        assert!(
            (len_s - sub.tx_s()).abs() < 0.1,
            "lasts {len_s} s, {} s expected",
            sub.tx_s()
        );
    }
}

/// The CQ of each mode of the sequencer decodes with the stack's own receive chain.
#[test]
fn the_cq_of_each_sequencer_mode_decodes_with_its_own_stack() {
    for (mode, run_s) in [
        (Mode::Ft8, 45.0),
        (Mode::Ft4, 25.0),
        (Mode::Ft2, 15.0),
        (Mode::Jt65, 175.0),
        (Mode::Jt9, 175.0),
        (Mode::Jt4, 175.0),
    ] {
        let (b, centres, heard) = round_trip(mode, &[], 0.0, run_s, |rx| call(rx, "cq"));
        on_slot(&b, Sub::new(mode));
        assert!(centres.iter().all(|c| *c == 14e6), "{mode:?}: {centres:?}");
        assert!(
            heard.iter().any(|t| t == "CQ K1ABC FN42"),
            "{mode:?}: {heard:?}"
        );
    }
}

/// A stack set to another sub-mode sends that sub-mode, and the dial away from the centre moves the burst with it.
#[test]
fn a_cq_follows_the_sub_mode_and_the_dial_of_the_stack() {
    for (mode, key) in [(Mode::Jt65, "jt65_submode"), (Mode::Jt4, "jt4_submode")] {
        let extra = [(key, Value::Choice(1))];
        let (b, centres, heard) = round_trip(mode, &extra, 2000.0, 175.0, |rx| call(rx, "cq"));
        on_slot(&b, Sub::new(mode).with_sub(1));
        assert!(
            centres.iter().all(|c| *c == 14e6 + 2000.0),
            "{mode:?}: {centres:?}"
        );
        assert!(
            heard.iter().any(|t| t == "CQ K1ABC FN42"),
            "{mode:?}: {heard:?}"
        );
    }
}

/// The WSPR beacon decodes as the call, locator and power of the station.
#[test]
fn the_wspr_beacon_decodes_with_its_own_stack() {
    let extra = [
        ("beacon_every", Value::I64(1)),
        ("beacon_dbm", Value::I64(23)),
    ];
    let (b, _, heard) = round_trip(Mode::Wspr, &extra, 0.0, 235.0, |rx| call(rx, "beacon"));
    on_slot(&b, Sub::new(Mode::Wspr));
    assert!(heard.iter().any(|t| t == "K1ABC FN42 23"), "{heard:?}");
}

/// A JS8 message goes out frame by frame at each speed and decodes as sent.
#[test]
fn a_js8_message_decodes_with_its_own_stack_at_each_speed() {
    for (speed, run_s) in [(Js8Speed::Normal, 75.0), (Js8Speed::Turbo, 40.0)] {
        let extra = [("js8_speed", Value::Choice(speed.index()))];
        let (b, _, heard) = round_trip(Mode::Js8, &extra, 0.0, run_s, |rx| {
            let id = rx.commands().id("send").unwrap();
            let mut args = rx.commands().def(id).unwrap().new_args();
            let text = args.decl().id("text").unwrap();
            args.set_value(text, &Value::Text("W9XYZ HELLO WORLD".into()))
                .unwrap();
            owner::invoke(rx, id, &args).unwrap();
        });
        assert!(
            b.iter().all(|x| x.2),
            "{speed:?}: a burst without its end mark"
        );
        assert!(
            heard.iter().any(|t| t.contains("HELLO WORLD")),
            "{speed:?}: {heard:?}"
        );
    }
}

/// Switching the transmitter off in a burst, or a new format, ends the burst with its end mark at once.
#[test]
fn a_burst_cut_by_the_transmitter_or_a_retune_ends_with_its_mark() {
    let rate = TRIP_RATE;
    let (mut rx, sink) = station_stack(Mode::Ft8, rate, &[]);
    call(&mut rx, "cq");
    silence(&mut rx, &sink, rate, 0.0, 35.0);
    rx.set_transmit(false);
    silence(&mut rx, &sink, rate, 35.0, 36.0);
    let b = bursts(&sink);
    assert_eq!(b.len(), 1);
    assert!(b[0].2, "the cut burst has its end mark");
    let stop_s = (b[0].0 + b[0].1.len()) as f64 / rate;
    assert!((35.0..35.25).contains(&stop_s), "ends at {stop_s} s");
    rx.set_transmit(true);
    call(&mut rx, "cq");
    silence(&mut rx, &sink, rate, 36.0, 65.0);
    rx.configure(&sdrkit_core::InputInfo::new(rate, 7e6, 7e6).recording())
        .unwrap();
    silence(&mut rx, &sink, rate, 65.0, 66.0);
    let b = bursts(&sink);
    assert_eq!(b.len(), 2);
    assert!(b[1].2, "the burst cut by the new format has its end mark");
}

/// A transmission that the transmitter's switch ended does not start again when the switch comes back on.
#[test]
fn switching_the_transmitter_on_again_starts_nothing() {
    let rate = TRIP_RATE;
    for (mode, cmd) in [
        (Mode::Ft8, "cq"),
        (Mode::Wspr, "beacon"),
        (Mode::Js8, "send"),
    ] {
        let mut extra = Vec::new();
        if mode == Mode::Wspr {
            extra.push(("beacon_every", Value::I64(1)));
        }
        let (mut rx, sink) = station_stack(mode, rate, &extra);
        let id = rx.commands().id(cmd).unwrap();
        let mut args = rx.commands().def(id).unwrap().new_args();
        if let Some(t) = args.decl().id("text") {
            args.set_value(t, &Value::Text("W9XYZ HELLO".into()))
                .unwrap();
        }
        owner::invoke(&mut rx, id, &args).unwrap();
        silence(&mut rx, &sink, rate, 0.0, 1.0);
        rx.set_transmit(false);
        silence(&mut rx, &sink, rate, 1.0, 2.0);
        rx.set_transmit(true);
        silence(&mut rx, &sink, rate, 2.0, 250.0);
        assert!(sink.writes.lock().unwrap().is_empty(), "{mode:?}");
    }
}

/// At a rate that is no multiple of the base rate, fed in odd pieces, the sender writes as many samples as it is fed.
#[test]
fn the_sender_keeps_the_pace_of_the_session() {
    let rate = 44_100.0;
    let (mut rx, sink) = station_stack(Mode::Ft8, rate, &[]);
    call(&mut rx, "cq");
    let piece = 337;
    let zeros = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut fed = 0usize;
    let mut worst = 0i64;
    while fed < (45.0 * rate) as usize {
        sink.fed.store(fed, Ordering::Relaxed);
        rx.feed(&zeros, &mut out);
        out.clear();
        fed += piece;
        let b = bursts(&sink);
        if let Some((at, samples, false)) = b.first() {
            // Ahead of the input by the resampler's delay at most.
            let ahead = (*at + samples.len()) as i64 - fed as i64;
            worst = worst.max(ahead);
        }
    }
    assert!(
        worst < 64,
        "the burst ran {worst} samples ahead of the input"
    );
    let b = bursts(&sink);
    assert_eq!(b.len(), 1);
    assert!(b[0].2);
}

/// At the rate of the stack's descriptor (2 MHz, the dial 20 kHz above the centre) the CQ starts on its slot, lasts its
/// time, keeps its level and sits 1500 to 1544 Hz above the dial.
#[test]
fn a_cq_at_the_rate_of_the_stack() {
    let rate = 2e6;
    let start = [("time_start", Value::Text("2026-01-01T00:00:27Z".into()))];
    let (mut rx, sink) = station_stack_at(Mode::Ft8, rate, 20e3, &start);
    call(&mut rx, "cq");
    silence(&mut rx, &sink, rate, 0.0, 17.0);
    let b = bursts(&sink);
    assert_eq!(b.len(), 1);
    let (at, samples, ended) = &b[0];
    assert!(ended);
    let at_s = *at as f64 / rate;
    assert!(
        (at_s - (3.0 + Sub::new(Mode::Ft8).t0())).abs() <= 0.15,
        "starts at {at_s} s"
    );
    let len_s = samples.len() as f64 / rate;
    assert!((len_s - 12.64).abs() < 0.05, "lasts {len_s} s");
    let peak = samples.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
    assert!(peak <= 0.51 && peak > 0.45, "peak {peak}");
    for (_, _, centre, _) in sink.writes.lock().unwrap().iter() {
        assert_eq!(*centre, 14e6 + 20e3, "centred on the dial");
    }
    // The mean frequency of the middle of the burst.
    let mid = &samples[samples.len() / 4..3 * samples.len() / 4];
    let turn: f64 = mid
        .windows(2)
        .map(|w| f64::from((w[1] * w[0].conj()).arg()))
        .sum::<f64>()
        / (mid.len() - 1) as f64;
    let hz = turn * rate / std::f64::consts::TAU;
    assert!((1500.0..1544.0).contains(&hz), "at {hz} Hz above the dial");
}

/// The combined stack calls CQ in the mode of `qso_mode`, and `halt` drops the JS8 frames that wait.
#[test]
fn the_combined_stack_sends_its_qso_mode_and_halt_drops_waiting_frames() {
    let rate = TRIP_RATE;
    let ft4 = sdrkit_stack_wsjt::components::QsoMode::Ft4.index();
    let (mut rx, sink) = station_stack_of(None, rate, 0.0, &[("qso_mode", Value::Choice(ft4))]);
    call(&mut rx, "cq");
    silence(&mut rx, &sink, rate, 0.0, 25.0);
    let b = bursts(&sink);
    on_slot(&b, Sub::new(Mode::Ft4));
    let mut air = vec![Complex32::default(); (25.0 * rate) as usize];
    for (at, samples, _) in &b {
        air[*at..*at + samples.len()].copy_from_slice(samples);
    }
    let mut listener = receiver(Some(Mode::Ft4), &[], rate, 0.0);
    assert!(texts(&feed(&mut listener, &air, 1200)).contains(&"CQ K1ABC FN42".to_string()));
    call(&mut rx, "halt");
    silence(&mut rx, &sink, rate, 25.0, 30.0);
    let before = bursts(&sink).len();
    // A long JS8 text is several frames, one a slot; halt in the first frame drops the rest.
    let id = rx.commands().id("send").unwrap();
    let mut args = rx.commands().def(id).unwrap().new_args();
    let t = args.decl().id("text").unwrap();
    args.set_value(
        t,
        &Value::Text("W9XYZ THIS IS A LONG MESSAGE OF MANY FRAMES TO SEND".into()),
    )
    .unwrap();
    owner::invoke(&mut rx, id, &args).unwrap();
    silence(&mut rx, &sink, rate, 30.0, 47.0);
    assert_eq!(bursts(&sink).len(), before + 1, "the first frame");
    call(&mut rx, "halt");
    silence(&mut rx, &sink, rate, 47.0, 120.0);
    let b = bursts(&sink);
    assert_eq!(b.len(), before + 1, "no frame after halt");
    assert!(b.last().unwrap().2);
}

/// FNV-1a over the whole bursts of `sink` joined across passes: per burst where it starts in the input, its length, the
/// bits of every sample and its end mark.
fn digest(sink: &Sink) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (at, samples, ended) in bursts(sink) {
        put(at as u64);
        put(samples.len() as u64);
        for z in &samples {
            put(u64::from(z.re.to_bits()) << 32 | u64::from(z.im.to_bits()));
        }
        put(u64::from(ended));
    }
    h
}

/// The samples of the sender for fixed messages: an FT8 CQ at 48 kHz (a rate change), a WSPR beacon at the base rate
/// (none), and an FT8 CQ at 44.1 kHz fed in pieces of 337 samples.
#[test]
fn the_samples_of_the_sender_keep_their_digest() {
    let mut got = Vec::new();
    let (mut rx, sink) = station_stack(Mode::Ft8, 48_000.0, &[]);
    call(&mut rx, "cq");
    silence(&mut rx, &sink, 48_000.0, 0.0, 45.0);
    assert_eq!(bursts(&sink).len(), 1);
    got.push(digest(&sink));
    let (mut rx, sink) = station_stack(Mode::Wspr, 12_000.0, &[("beacon_every", Value::I64(1))]);
    call(&mut rx, "beacon");
    silence(&mut rx, &sink, 12_000.0, 0.0, 235.0);
    assert_eq!(bursts(&sink).len(), 1);
    got.push(digest(&sink));
    let rate = 44_100.0;
    let (mut rx, sink) = station_stack(Mode::Ft8, rate, &[]);
    call(&mut rx, "cq");
    let zeros = vec![Complex32::default(); 337];
    let mut out = Output::default();
    let mut fed = 0usize;
    while fed < (45.0 * rate) as usize {
        sink.fed.store(fed, Ordering::Relaxed);
        rx.feed(&zeros, &mut out);
        out.clear();
        fed += 337;
    }
    assert_eq!(bursts(&sink).len(), 1);
    got.push(digest(&sink));
    eprintln!("wsjt transmit digests {got:#x?}");
    assert_eq!(got, [0x33b226ebe0fbe5e5, 0xa865c35e4f3adc8b, 0x19dbdc482fb600af]);
}
