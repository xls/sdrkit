//! The transmitters of `ale` and `hfbeacons`: the words of an ALE call checked on the samples against words packed by hand
//! and coded here from the public standard, the tones and symbol timing measured, the call and the sounding read back by
//! the stack's own receiver clean and in noise; the Morse elements of the beacon measured against the ITU-R M.1677-1
//! timing and read back by the library's Morse decoder, the interval, the release between sendings; `stop`, the stated
//! start, the refusals. The transmit stream is reached as the engine reaches it: the stack's graph with the host streams
//! in its context.

use crate::common::{rows, text, Owner};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Clock, CommandError, Complex32, InputInfo, Output, Receiver, StackConfig,
    StackDescriptor, Value, Values,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_hfdata::typed::{AleTxFields, HfbTxFields, ALE_EVENTS, ALE_WORDS};
use sdrkit_stack_hfdata::{ale, beacons};
use sdrkit_util_text::morse::{Decoder, DecoderConfig, Event as MorseEvent};
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack.
pub(crate) const DIAL: f64 = 14.109e6;
/// The clock of a host that times bursts.
const NOW: f64 = 1_800_000_000.0;

/// One write to the transmit stream: its samples, its end mark and its stated start.
type Write = (Vec<Complex32>, bool, Option<f64>);

/// The transmit stream: each write, the lead that the host asks for, and the number of writes that opened a burst.
#[derive(Default)]
pub(crate) struct Sink {
    writes: Mutex<Vec<Write>>,
    lead: Duration,
    opened: Mutex<(bool, usize)>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            let mut o = self.opened.lock().unwrap();
            if !o.0 {
                o.1 += 1;
            }
            o.0 = !end;
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), end, start));
        }
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

/// One burst: its samples, whether its end mark came, and the stated starts in it (the place and the UTC).
pub(crate) struct Burst {
    pub(crate) samples: Vec<Complex32>,
    pub(crate) ended: bool,
    pub(crate) starts: Vec<(usize, f64)>,
}

impl Sink {
    pub(crate) fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, end, start) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push(Burst {
                    samples: Vec::new(),
                    ended: false,
                    starts: Vec::new(),
                });
            }
            let b = out.last_mut().unwrap();
            if let Some(utc) = start {
                b.starts.push((b.samples.len(), *utc));
            }
            b.samples.extend_from_slice(s);
            b.ended = *end;
            open = !end;
        }
        out
    }
    /// The samples of the writes after the first `*seen`, joined; `*seen` becomes the number of writes.
    pub(crate) fn samples_since(&self, seen: &mut usize) -> Vec<Complex32> {
        let w = self.writes.lock().unwrap();
        let x = w[*seen..]
            .iter()
            .flat_map(|(s, _, _)| s.iter().copied())
            .collect();
        *seen = w.len();
        x
    }
    fn opened(&self) -> usize {
        self.opened.lock().unwrap().1
    }
}

/// Stack `desc` at `rate` with `settings` (key, text), transmit on, and its transmit stream. With `lead`, on a live radio
/// whose host times bursts with that lead, at the clock [`NOW`].
pub(crate) fn station(
    desc: &StackDescriptor,
    rate: f64,
    settings: &[(&str, Value)],
    lead: Option<Duration>,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    let schema = (desc.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in settings {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    let mut rx = (desc.make_receiver)(&StackConfig::new(s)).unwrap();
    let input = InputInfo::new(rate, DIAL, DIAL);
    rx.configure(&if lead.is_some() {
        input
    } else {
        input.recording()
    })
    .unwrap();
    let sink = Arc::new(Sink {
        lead: lead.unwrap_or_default(),
        ..Sink::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

pub(crate) fn text_value(s: &str) -> Value {
    Value::Text(s.to_owned())
}

/// Runs command `key` with `args`.
pub(crate) fn call(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), CommandError> {
    let id = Owner::commands(rx).id(key).unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    for (k, v) in args {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
pub(crate) fn idle(rx: &mut dyn Receiver, rate: f64, seconds: f64) {
    let zeros = vec![Complex32::default(); (rate / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

// ALE.

/// The ALE stack at `rate` with the own address `own`.
fn ale_station(rate: f64, own: &str, lead: Option<Duration>) -> (Box<dyn Receiver>, Arc<Sink>) {
    station(&ale::ALE, rate, &[("self_address", text_value(own))], lead)
}

/// The extended Golay (24,12) code word of 12 data bits, written here from the standard: the data, then the 11 check bits
/// of the generator x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1, then an even parity bit.
fn golay(d: u32) -> u32 {
    let mut r = d << 11;
    for i in (11..23).rev() {
        if r & (1 << i) != 0 {
            r ^= 0xC75 << (i - 11);
        }
    }
    let check = r & 0x7FF;
    let parity = (d.count_ones() + check.count_ones()) & 1;
    (d << 12) | (check << 1) | parity
}

/// The tribits of the eight tones, 750 Hz first (the Gray code of the standard).
const TONE_BITS: [u8; 8] = [0b000, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100];

/// The 49 tones of a 24-bit word, written here from the specification: halves A and B coded, B inverted, interleaved
/// A1 B1 .. A24 B24 with a stuff bit 0, the 49 bits sent three times, three bits a tone, most significant first.
fn tones_by_hand(word: u32) -> Vec<u8> {
    let a = golay(word >> 12);
    let b = golay(word & 0xFFF) ^ 0xFF_FFFF;
    let mut bits = Vec::with_capacity(49);
    for i in (0..24).rev() {
        bits.push(((a >> i) & 1) as u8);
        bits.push(((b >> i) & 1) as u8);
    }
    bits.push(0);
    let all: Vec<u8> = bits.iter().chain(&bits).chain(&bits).copied().collect();
    all.chunks(3)
        .map(|c| {
            let t = (c[0] << 2) | (c[1] << 1) | c[2];
            TONE_BITS.iter().position(|&x| x == t).unwrap() as u8
        })
        .collect()
}

/// The frequency of each phase step of `x` at `rate`, Hz: element n is the step from sample n to n + 1.
fn steps(x: &[Complex32], rate: f64) -> Vec<f64> {
    x.windows(2)
        .map(|p| f64::from((p[1] * p[0].conj()).arg()) * rate / TAU)
        .collect()
}

#[test]
fn ale_golay_by_hand_matches_the_known_word() {
    // Data 001 is the generator itself with its parity bit.
    assert_eq!(golay(0x001), 0x0018EB);
}

#[test]
fn ale_call_words_tones_and_timing_on_the_samples() {
    let rate = 24_000.0;
    let (mut rx, sink) = ale_station(rate, "XYZ", None);
    call(rx.as_mut(), "call", &[("to", text_value("ABC"))]).unwrap();
    let f = AleTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(f.tx_sending);
    assert_eq!((f.tx_word, f.tx_words), (Some(1), Some(7)));
    idle(rx.as_mut(), rate, 4.0);
    let bursts = sink.bursts();
    assert_eq!(bursts.len(), 1);
    let x = &bursts[0].samples;
    assert!(bursts[0].ended);
    // Worked example form: the called address six times in TO words, then TIS with the own address; 7 words of 392 ms.
    // TO "ABC": type 2, then 0x41 0x42 0x43; TIS "XYZ": type 5, then 0x58 0x59 0x5A.
    let words = [
        0x50_6143, 0x50_6143, 0x50_6143, 0x50_6143, 0x50_6143, 0x50_6143, 0xB6_2CDA,
    ];
    let symbol = (0.008 * rate) as usize;
    assert_eq!(x.len(), words.len() * 49 * symbol, "2.744 s");
    let want: Vec<u8> = words.iter().flat_map(|&w| tones_by_hand(w)).collect();
    // Worked example 3: a word starts with tone 750 Hz exactly when its first three bits A1 B1 A2 are 000.
    for (k, &w) in words.iter().enumerate() {
        let a1 = golay(w >> 12) >> 23;
        let b1 = ((golay(w & 0xFFF) ^ 0xFF_FFFF) >> 23) & 1;
        let a2 = (golay(w >> 12) >> 22) & 1;
        assert_eq!(want[49 * k] == 0, (a1, b1, a2) == (0, 0, 0));
    }
    // Every phase step is the tone of its symbol: the tones are 750 + 250 k Hz, they change on the 8 ms grid, and the
    // phase runs on across every change.
    let st = steps(x, rate);
    for (n, f) in st.iter().enumerate().skip(1).take(st.len() - 2) {
        let tone = want[(n + 1) / symbol];
        let hz = 750.0 + 250.0 * f64::from(tone);
        assert!((f - hz).abs() < 0.5, "sample {n}: {f} Hz, tone {hz} Hz");
    }
    // A constant envelope at -6 dBFS between the 2 ms ramps.
    let ramp = (0.002 * rate) as usize;
    for z in &x[ramp..x.len() - ramp] {
        assert!((z.norm() - 0.5).abs() < 1e-3);
    }
    assert!(x[0].norm() < 0.01 && x[x.len() - 1].norm() < 0.05);
    let f = AleTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_sending && f.tx_words.is_none());
}

/// A call as the ALE receiver lists it: type, from, to.
type Heard = (String, String, String);

/// The ALE receiver's events and its words (votes, Golay corrections) on `x` at `rate`, with 1 s of silence before and
/// after.
fn ale_receive(x: &[Complex32], rate: f64) -> (Vec<Heard>, Vec<(f64, f64)>) {
    let schema = (ale::ALE.settings)();
    let mut rx = (ale::ALE.make_receiver)(&StackConfig::new(Values::defaults(&schema))).unwrap();
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let pad = vec![Complex32::default(); rate as usize];
    let mut out = Output::default();
    for c in pad
        .iter()
        .chain(x)
        .chain(&pad)
        .copied()
        .collect::<Vec<_>>()
        .chunks(2400)
    {
        out.clear();
        rx.feed(c, &mut out);
    }
    let events = rows(rx.results(), &ALE_EVENTS)
        .iter()
        .map(|e| (text(e, "type"), text(e, "from"), text(e, "to")))
        .collect();
    let words = rows(rx.results(), &ALE_WORDS)
        .iter()
        .map(|w| {
            (
                crate::common::num(w, "votes"),
                crate::common::num(w, "golay"),
            )
        })
        .collect();
    (events, words)
}

/// A call from XYZ to ABC and a TWS sounding of XYZ, as sent.
fn ale_call_and_sounding(rate: f64) -> Vec<Complex32> {
    let (mut rx, sink) = ale_station(rate, "XYZ", None);
    call(rx.as_mut(), "call", &[("to", text_value("abc"))]).unwrap();
    idle(rx.as_mut(), rate, 3.5);
    call(rx.as_mut(), "sound", &[]).unwrap();
    idle(rx.as_mut(), rate, 3.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    let mut x = b[0].samples.clone();
    x.extend(std::iter::repeat_n(Complex32::default(), rate as usize));
    x.extend_from_slice(&b[1].samples);
    x
}

fn want_calls() -> Vec<Heard> {
    [("Individual", "XYZ", "ABC"), ("Sounding", "XYZ", "")]
        .iter()
        .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
        .collect()
}

#[test]
fn ale_call_and_sounding_read_back_clean() {
    let rate = 24_000.0;
    let x = ale_call_and_sounding(rate);
    let (events, words) = ale_receive(&x, rate);
    assert_eq!(events, want_calls());
    assert_eq!(words.len(), 13);
    assert!(
        words.iter().all(|&(v, g)| v == 0.0 && g == 0.0),
        "{words:?}"
    );
}

#[test]
fn ale_call_and_sounding_read_back_at_10_db_in_3_khz() {
    let rate = 24_000.0;
    let x = ale_call_and_sounding(rate);
    // The signal is 0.25 in power; the noise is 10 dB under it in 3 kHz.
    let noise = 0.25 / 10.0 * rate / 3000.0;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let y: Vec<Complex32> = x.iter().map(|z| z + rng.complex_normal(noise)).collect();
    let (events, _) = ale_receive(&y, rate);
    assert_eq!(events, want_calls());
}

#[test]
fn ale_allcall_has_tws_after_the_called_address() {
    let rate = 24_000.0;
    let (mut rx, sink) = ale_station(rate, "XYZ", None);
    call(rx.as_mut(), "call", &[("to", text_value(""))]).unwrap();
    idle(rx.as_mut(), rate, 3.0);
    let b = sink.bursts();
    // TO "@?@" six times and TWS "XYZ": type 3, then 0x58 0x59 0x5A.
    let tail: Vec<u8> = tones_by_hand(0x76_2CDA);
    let x = &b[0].samples;
    let symbol = (0.008 * rate) as usize;
    assert_eq!(x.len(), 7 * 49 * symbol);
    let st = steps(x, rate);
    for (k, &t) in tail.iter().enumerate() {
        let n = (6 * 49 + k) * symbol + symbol / 2;
        assert!((st[n] - (750.0 + 250.0 * f64::from(t))).abs() < 0.5);
    }
}

#[test]
fn ale_refuses_an_empty_own_address_bad_addresses_and_transmit_off() {
    let rate = 24_000.0;
    let (mut rx, sink) = ale_station(rate, "", None);
    let e = call(rx.as_mut(), "call", &[("to", text_value("ABC"))]).unwrap_err();
    assert!(e.to_string().contains("self_address"), "{e}");
    let e = call(rx.as_mut(), "sound", &[]).unwrap_err();
    assert!(e.to_string().contains("self_address"), "{e}");
    assert!(crate::common::set(rx.as_mut(), "self_address", serde_json::json!("XY-Z")).is_err());
    assert!(crate::common::set(
        rx.as_mut(),
        "self_address",
        serde_json::json!("ABCDEFGHIJKLMNOP")
    )
    .is_err());
    crate::common::set(rx.as_mut(), "self_address", serde_json::json!("xyz")).unwrap();
    assert!(call(rx.as_mut(), "call", &[("to", text_value("A B"))]).is_err());
    rx.set_transmit(false);
    assert!(call(rx.as_mut(), "call", &[("to", text_value("ABC"))]).is_err());
    idle(rx.as_mut(), rate, 1.0);
    assert!(sink.bursts().is_empty());
}

#[test]
fn ale_stop_and_transmitter_off_end_the_burst_with_its_end_mark() {
    let rate = 24_000.0;
    let (mut rx, sink) = ale_station(rate, "XYZ", None);
    call(rx.as_mut(), "call", &[("to", text_value("ABC"))]).unwrap();
    idle(rx.as_mut(), rate, 0.5);
    call(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), rate, 0.5);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let x = &b[0].samples;
    // Stopped after 0.5 s: the burst ends with its 2 ms ramp within the next pass.
    assert!(x.len() <= (0.5 * rate) as usize + (0.002 * rate) as usize + 1);
    assert!(x[x.len() - 1].norm() < 0.05);
    let f = AleTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_sending);
    // The transmitter going off ends a call at once, with the end mark on one silent sample.
    call(rx.as_mut(), "call", &[("to", text_value("ABC"))]).unwrap();
    idle(rx.as_mut(), rate, 0.3);
    rx.set_transmit(false);
    rx.set_transmit(true);
    idle(rx.as_mut(), rate, 0.3);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].ended);
    assert!(b[1].samples.len() <= (0.3 * rate) as usize + 1);
}

#[test]
fn ale_states_the_start_of_its_burst_on_a_live_radio() {
    let rate = 250_000.0;
    let (mut rx, sink) = ale_station(rate, "XYZ", Some(Duration::from_millis(200)));
    call(rx.as_mut(), "sound", &[]).unwrap();
    idle(rx.as_mut(), rate, 3.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    // The time of the pass, the lead of 0.2 s and two passes of 0.1 s.
    assert_eq!(b[0].starts.len(), 1);
    assert_eq!(b[0].starts[0].0, 0);
    assert!((b[0].starts[0].1 - (NOW + 0.4)).abs() < 1e-6);
    assert!(b[0].ended);
}

// The own beacon.

/// The beacon stack at `rate` with call sign `call_sign`, message `message`, the long dash and the interval.
fn beacon_station(
    rate: f64,
    call_sign: &str,
    message: &str,
    long_dash_s: f64,
    interval_min: i64,
    lead: Option<Duration>,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    station(
        &beacons::HFBEACONS,
        rate,
        &[
            ("call", text_value(call_sign)),
            ("message", text_value(message)),
            ("long_dash_s", Value::F64(long_dash_s)),
            ("interval_min", Value::I64(interval_min)),
        ],
        lead,
    )
}

/// The key intervals of `x` at `rate`: runs above and below half the key-down level, seconds, from the first mark on.
fn key_runs(x: &[Complex32], rate: f64) -> Vec<(bool, f64)> {
    let mut r: Vec<(bool, usize)> = Vec::new();
    for z in x {
        let on = z.norm() >= 0.25;
        match r.last_mut() {
            Some((o, n)) if *o == on => *n += 1,
            _ => r.push((on, 1)),
        }
    }
    while r.first().is_some_and(|k| !k.0) {
        r.remove(0);
    }
    while r.last().is_some_and(|k| !k.0) {
        r.pop();
    }
    r.into_iter().map(|(o, n)| (o, n as f64 / rate)).collect()
}

/// What the library's Morse decoder reads from key intervals at `wpm`.
fn morse_read(keys: &[(bool, f64)], wpm: f64) -> String {
    let mut d = Decoder::new(DecoderConfig::following(wpm)).unwrap();
    let mut s = String::new();
    let mut put = |e: MorseEvent| match e {
        MorseEvent::Char(c) => s.push_str(c.text_or("*")),
        MorseEvent::Space => s.push(' '),
    };
    for &(on, secs) in keys {
        d.push(on, secs, &mut put);
    }
    d.push(false, 2.0, &mut put);
    d.flush(&mut put);
    s.trim().to_owned()
}

#[test]
fn beacon_elements_have_the_itu_timing_and_read_back() {
    let rate = 24_000.0;
    let (mut rx, sink) = beacon_station(rate, "N0CALL", "VVV DE {call}", 3.0, 10, None);
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 20.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let x = &b[0].samples;
    // The tone is at the pitch, 800 Hz above the dial, at -6 dBFS key down.
    let st = steps(x, rate);
    let mid = x.len() - (1.5 * rate) as usize;
    assert!((st[mid] - 800.0).abs() < 0.1 && (x[mid].norm() - 0.5).abs() < 1e-3);
    // 22 wpm: a unit is 1.2 / 22 s. Marks are 1 or 3 units, spaces 1, 3 or 7, the long dash 3 s after a word gap.
    let unit = 1.2 / 22.0;
    let tol = 1.5 / rate;
    let runs = key_runs(x, rate);
    let (dash, body) = runs.split_last().unwrap();
    assert!(dash.0 && (dash.1 - 3.0).abs() < tol, "{dash:?}");
    let gap = body.last().unwrap();
    assert!(!gap.0 && (gap.1 - 7.0 * unit).abs() < tol);
    let mut seen = [0usize; 8];
    for &(on, secs) in &body[..body.len() - 1] {
        let u = secs / unit;
        let n = u.round();
        assert!((secs - n * unit).abs() < tol, "{on} {secs}");
        assert!(if on {
            n == 1.0 || n == 3.0
        } else {
            [1.0, 3.0, 7.0].contains(&n)
        });
        seen[n as usize] += 1;
    }
    assert!(seen[1] > 0 && seen[3] > 0 && seen[7] > 0);
    assert_eq!(morse_read(&body[..body.len() - 1], 22.0), "VVV DE N0CALL");
}

#[test]
fn beacon_text_length_follows_the_units() {
    // T3ST: T (3), 3 (9 + 4 inner gaps), S (5), T (3), three gaps of 3: 33 units at 20 wpm.
    let rate = 24_000.0;
    let (mut rx, sink) = station(
        &beacons::HFBEACONS,
        rate,
        &[
            ("call", text_value("T3ST")),
            ("message", text_value("")),
            ("long_dash_s", Value::F64(0.0)),
            ("wpm", Value::F64(20.0)),
            ("pitch_hz", Value::Hz(600.0)),
        ],
        None,
    );
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 5.0);
    let b = sink.bursts();
    let runs = key_runs(&b[0].samples, rate);
    let keyed: f64 = runs.iter().map(|r| r.1).sum();
    assert!((keyed - 33.0 * 0.06).abs() < 2.0 / rate, "{keyed}");
    // The whole sending: the keying with a 5 ms rise before and a 5 ms fall after.
    assert!((b[0].samples.len() as f64 / rate - (33.0 * 0.06 + 0.01)).abs() < 2.0 / rate);
    let st = steps(&b[0].samples, rate);
    assert!(
        (st[b[0].samples.len() / 2 - 100..]
            .iter()
            .find(|f| f.abs() > 1.0)
            .unwrap()
            - 600.0)
            .abs()
            < 0.5
    );
    assert_eq!(morse_read(&runs, 20.0), "T3ST");
}

#[test]
fn beacon_sends_again_each_interval_and_frees_the_transmitter_between() {
    let rate = 24_000.0;
    let (mut rx, sink) = beacon_station(rate, "T3ST", "{call}", 0.0, 1, None);
    call(rx.as_mut(), "start", &[]).unwrap();
    let zeros = vec![Complex32::default(); 2400];
    let mut out = Output::default();
    let mut starts = Vec::new();
    for piece in 0..1300 {
        let before = sink.opened();
        out.clear();
        rx.feed(&zeros, &mut out);
        if sink.opened() > before {
            starts.push(piece);
        }
        if piece == 300 {
            let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
            assert!(f.tx_beacon_on && !f.tx_sending);
            let next = f.tx_next_s.unwrap().0;
            assert!((next - 29.9).abs() < 0.3, "{next}");
            assert_eq!(f.tx_sendings, 1);
        }
    }
    // A sending every 600 passes of 0.1 s: every minute.
    assert_eq!(starts, vec![0, 600, 1200]);
    let b = sink.bursts();
    assert_eq!(b.len(), 3);
    assert!(b.iter().all(|b| b.ended));
    let n = b[0].samples.len();
    assert!(b.iter().all(|b| b.samples.len() == n));
    call(rx.as_mut(), "stop", &[]).unwrap();
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_beacon_on && f.tx_sendings == 3);
}

#[test]
fn beacon_stop_ends_the_sending_with_its_fall() {
    let rate = 24_000.0;
    let (mut rx, sink) = beacon_station(rate, "N0CALL", "VVV DE {call}", 3.0, 10, None);
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 1.0);
    call(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), rate, 1.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let x = &b[0].samples;
    // The pass after stop writes at most 50 ms of keying and the 5 ms fall.
    assert!(x.len() <= (1.2 * rate) as usize, "{}", x.len());
    assert!(x[x.len() - 1].norm() < 0.01);
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_beacon_on && !f.tx_sending);
    idle(rx.as_mut(), rate, 1.0);
    assert_eq!(sink.bursts().len(), 1);
}

#[test]
fn beacon_refuses_an_empty_call_sign_bad_text_and_transmit_off() {
    let rate = 24_000.0;
    let (mut rx, sink) = beacon_station(rate, "", "VVV DE {call}", 3.0, 10, None);
    let e = call(rx.as_mut(), "start", &[]).unwrap_err();
    assert!(e.to_string().contains("call"), "{e}");
    assert!(crate::common::set(rx.as_mut(), "call", serde_json::json!("N0-CALL")).is_err());
    assert!(
        crate::common::set(rx.as_mut(), "message", serde_json::json!("VVV DE {call} !")).is_err()
    );
    crate::common::set(rx.as_mut(), "call", serde_json::json!("n0call")).unwrap();
    rx.set_transmit(false);
    assert!(call(rx.as_mut(), "start", &[]).is_err());
    rx.set_transmit(true);
    call(rx.as_mut(), "start", &[]).unwrap();
    assert!(call(rx.as_mut(), "start", &[]).is_err(), "busy");
    idle(rx.as_mut(), rate, 0.5);
    // The transmitter going off ends the sending and the schedule.
    rx.set_transmit(false);
    idle(rx.as_mut(), rate, 0.2);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_beacon_on);
}

#[test]
fn beacon_states_the_start_of_each_sending_on_a_live_radio() {
    let rate = 250_000.0;
    let (mut rx, sink) = beacon_station(
        rate,
        "T3ST",
        "{call}",
        0.0,
        10,
        Some(Duration::from_millis(200)),
    );
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 3.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].starts.len(), 1);
    assert_eq!(b[0].starts[0].0, 0);
    assert!((b[0].starts[0].1 - (NOW + 0.4)).abs() < 1e-6);
}

#[test]
fn beacon_power_steps_follow_the_long_dash_10_db_apart() {
    // Worked example 2: a 3 s long dash, then 1 s at 0, -10, -20 and -30 dB: 7 s of carrier with no gap.
    let rate = 24_000.0;
    let (mut rx, sink) = station(
        &beacons::HFBEACONS,
        rate,
        &[
            ("call", text_value("T3ST")),
            ("message", text_value("{call}")),
            ("long_dash_s", Value::F64(3.0)),
            ("power_steps", Value::Bool(true)),
        ],
        None,
    );
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    // T3ST is 33 units at 22 wpm, a word gap of 7, 7 s of dashes, the rise and the fall.
    let unit = 1.2 / 22.0;
    let want = 40.0 * unit + 7.0 + 0.01;
    assert!(
        (f.tx_sending_s.0 - want).abs() < 1e-6,
        "{}",
        f.tx_sending_s.0
    );
    assert!((f.tx_duty_pct - 100.0 * want / 600.0).abs() < 1e-9);
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 12.0);
    let b = sink.bursts();
    let x = &b[0].samples;
    assert!((x.len() as f64 / rate - want).abs() < 2.0 / rate);
    // The last 7 s: the mean amplitude in the middle of each second.
    let end = x.len() - (0.005 * rate) as usize;
    let level = |s: f64| {
        let at = end - ((7.0 - s) * rate) as usize;
        let w = &x[at - 2400..at + 2400];
        w.iter().map(|z| f64::from(z.norm())).sum::<f64>() / w.len() as f64
    };
    for s in [0.5, 1.5, 2.5, 3.5] {
        assert!((level(s) - 0.5).abs() < 1e-3, "{s}: {}", level(s));
    }
    for (k, s) in [4.5, 5.5, 6.5].iter().enumerate() {
        let db = 20.0 * (level(*s) / 0.5).log10();
        assert!((db + 10.0 * (k + 1) as f64).abs() < 0.1, "{s}: {db} dB");
    }
    // The steps are one key down: no gap below half the level of the -30 dB step.
    let tail = &x[end - (7.0 * rate) as usize..end];
    assert!(tail.iter().all(|z| z.norm() > 0.5 * 0.0158 * 0.5));
}

#[test]
fn beacon_starts_at_its_second_of_the_interval_and_ends_after_its_count() {
    let rate = 24_000.0;
    let (mut rx, sink) = station(
        &beacons::HFBEACONS,
        rate,
        &[
            ("call", text_value("T3ST")),
            ("message", text_value("{call}")),
            ("long_dash_s", Value::F64(0.0)),
            ("interval_min", Value::I64(1)),
            ("offset_s", Value::F64(30.0)),
            ("count", Value::I64(2)),
        ],
        None,
    );
    // The clock is on a minute: the first sending is 30 s later, the second a minute after it, then none.
    call(rx.as_mut(), "start", &[]).unwrap();
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(f.tx_beacon_on && !f.tx_sending && f.tx_sendings == 0);
    assert!((f.tx_next_s.unwrap().0 - 30.0).abs() < 0.01);
    let zeros = vec![Complex32::default(); 2400];
    let mut out = Output::default();
    let mut starts = Vec::new();
    for piece in 0..1600 {
        let before = sink.opened();
        out.clear();
        rx.feed(&zeros, &mut out);
        if sink.opened() > before {
            starts.push(piece);
        }
    }
    assert_eq!(starts, vec![300, 900]);
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_beacon_on && f.tx_sendings == 2);
}

#[test]
fn beacon_send_sends_once_outside_the_schedule() {
    let rate = 24_000.0;
    let (mut rx, sink) = beacon_station(rate, "T3ST", "{call}", 0.0, 1, None);
    call(rx.as_mut(), "send", &[]).unwrap();
    assert!(call(rx.as_mut(), "send", &[]).is_err(), "busy");
    assert!(call(rx.as_mut(), "start", &[]).is_err(), "busy");
    let f = HfbTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert!(!f.tx_beacon_on && f.tx_sending);
    idle(rx.as_mut(), rate, 70.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    assert_eq!(morse_read(&key_runs(&b[0].samples, rate), 22.0), "T3ST");
    rx.set_transmit(false);
    let e = call(rx.as_mut(), "send", &[]).unwrap_err();
    assert!(e.to_string().contains("off"), "{e}");
}

/// FNV-1a over the bursts of `sink` joined across passes: per burst its length, the bits of every sample, its end mark,
/// and its stated starts (place, UTC to 1 µs).
fn digest(sink: &Sink) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for b in sink.bursts() {
        put(b.samples.len() as u64);
        for z in &b.samples {
            put(u64::from(z.re.to_bits()) << 32 | u64::from(z.im.to_bits()));
        }
        put(u64::from(b.ended));
        for (at, utc) in &b.starts {
            put(*at as u64);
            put((utc * 1e6).round() as u64);
        }
    }
    h
}

/// The samples of both senders on a live radio with stated starts: an ALE call and a sounding, a stopped call, and two
/// sendings of the beacon, the second stopped.
#[test]
fn the_samples_of_the_senders_keep_their_digest() {
    let rate = 250_000.0;
    let lead = Some(Duration::from_millis(200));
    let mut got = Vec::new();
    let (mut rx, sink) = ale_station(rate, "XYZ", lead);
    call(rx.as_mut(), "call", &[("to", text_value("abc"))]).unwrap();
    idle(rx.as_mut(), rate, 3.5);
    call(rx.as_mut(), "sound", &[]).unwrap();
    idle(rx.as_mut(), rate, 3.0);
    call(rx.as_mut(), "call", &[("to", text_value("abc"))]).unwrap();
    idle(rx.as_mut(), rate, 0.5);
    call(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), rate, 0.5);
    assert!(sink.bursts().iter().all(|b| b.ended && b.starts.len() == 1));
    assert_eq!(sink.bursts().len(), 3);
    got.push(digest(&sink));
    let (mut rx, sink) = beacon_station(rate, "T3ST", "{call} TEST", 0.0, 1, lead);
    call(rx.as_mut(), "start", &[]).unwrap();
    idle(rx.as_mut(), rate, 61.0);
    call(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), rate, 0.5);
    assert!(sink.bursts().iter().all(|b| b.ended && b.starts.len() == 1));
    assert_eq!(sink.bursts().len(), 2);
    got.push(digest(&sink));
    eprintln!("hfdata transmit digests {got:#x?}");
    assert_eq!(got, [0x57b5ff20db080664, 0xc27b96ed5fab4282]);
}
