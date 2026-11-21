//! The AX.25 sender of the cubesat stack: frames sent through the stack's transmit stream come back from its own receive side
//! with the same bytes at 1200 bit/s AFSK and 9600 bit/s G3RUH; the address octets, the FCS and the bit stuffing against
//! values checked by hand; the level and the occupied bandwidth; the refusals and the end of a transmission.

use crate::common::{cell, cell_bytes, cell_num, command_with};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value, Values};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_proto_link::hdlc::{Line, Transmitter};
use sdrkit_stack_satcom::modes::cubesat::{self, tx};
use sdrkit_stack_satcom::typed::CubesatTxFields;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

/// The session rate.
const RATE: f64 = 1e6;
/// The tuned frequency.
const DIAL: f64 = 145.825e6;
/// The centre of the radio: the stack's tune offset above the dial.
const CENTRE: f64 = DIAL + 50e3;
/// The station callsign of the tests.
const CALL: &str = "N0CALL-7";

/// The transmit stream: each write with its end mark.
#[derive(Default)]
struct Sink {
    writes: Mutex<Vec<(Vec<Complex32>, bool)>>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (HostPort::Transmit, HostData::Iq { samples, end, .. }) = (port, data) {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
        }
    }
}

impl Sink {
    /// The transmissions: their samples and whether the end mark came.
    fn bursts(&self) -> Vec<(Vec<Complex32>, bool)> {
        let mut out: Vec<(Vec<Complex32>, bool)> = Vec::new();
        let mut open = false;
        for (s, end) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push((Vec::new(), false));
            }
            let b = out.last_mut().unwrap();
            b.0.extend_from_slice(s);
            b.1 = *end;
            open = !end;
        }
        out
    }
}

/// The settings of the stack with `set` on its defaults.
fn values(set: &[(&str, Value)]) -> Values {
    let schema = (cubesat::DESCRIPTOR.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in set {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    s
}

/// The stack with `set`, transmit on, the station callsign `call`, and its transmit stream.
fn station_as(set: &[(&str, Value)], call: &str) -> (Box<dyn Receiver>, Arc<Sink>) {
    let mut rx = (cubesat::DESCRIPTOR.make_receiver)(&StackConfig::new(values(set))).unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, DIAL).recording())
        .unwrap();
    let sink = Arc::new(Sink::default());
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    let mut st = (*ctx.station).clone();
    st.callsign = call.into();
    ctx.station = Arc::new(st);
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

/// The stack with `set` and the station callsign [`CALL`].
fn station(set: &[(&str, Value)]) -> (Box<dyn Receiver>, Arc<Sink>) {
    station_as(set, CALL)
}

/// Sends a frame with the arguments `args` (the defaults for the rest).
fn send(rx: &mut dyn Receiver, args: &[(&str, &str)]) -> Result<(), sdrkit_core::CommandError> {
    let a: Vec<(&str, Value)> = args
        .iter()
        .map(|(k, v)| {
            let v = if *k == "pid" {
                Value::I64(v.parse().unwrap())
            } else {
                Value::Text((*v).into())
            };
            (*k, v)
        })
        .collect();
    command_with(rx, "send", &a)
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
fn idle(rx: &mut dyn Receiver, seconds: f64) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

/// The transmit fields.
fn fields(rx: &dyn Receiver) -> CubesatTxFields {
    CubesatTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// Feeds silence until nothing is sent, at most `limit` seconds.
fn until_quiet(rx: &mut dyn Receiver, limit: f64) {
    for _ in 0..(limit * 10.0) as usize {
        idle(rx, 0.1);
        let f = fields(rx);
        if !f.tx_sending && f.tx_waiting == 0 {
            idle(rx, 0.2);
            return;
        }
    }
    panic!("still sending after {limit} s");
}

/// The one transmission that the stack with `set` sends for the frames of `sends`; it ends with its end mark.
fn one_burst(set: &[(&str, Value)], sends: &[&[(&str, &str)]]) -> Vec<Complex32> {
    let (mut rx, sink) = station(set);
    for a in sends {
        send(rx.as_mut(), a).expect("the frame is sent");
    }
    until_quiet(rx.as_mut(), 30.0);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    assert!(b[0].1, "the end mark");
    b.remove(0).0
}

/// Adds complex noise at `snr_db` against `power` in `bw` Hz.
fn add_noise(x: &mut [Complex32], power: f64, snr_db: f64, bw: f64, seed: u64) {
    let n = power / 10f64.powf(snr_db / 10.0) * RATE / bw;
    let mut rng = Rng::new(seed);
    for z in x.iter_mut() {
        *z += rng.complex_normal(n);
    }
}

/// (from, to, path, pid, data) of each frame that the stack's receive side reads from `x` (centred on the dial), with 0.3 s
/// of silence before and after and noise at `snr_db` in 25 kHz against the power `power` of the burst.
fn received(x: &[Complex32], power: f64, snr_db: f64) -> Vec<(String, String, String, i64, Vec<u8>)> {
    let pad = (0.3 * RATE) as usize;
    let mut y = vec![Complex32::default(); pad];
    y.extend_from_slice(x);
    y.resize(y.len() + pad, Complex32::default());
    // The radio's centre is the tune offset above the dial.
    for (i, z) in y.iter_mut().enumerate() {
        let p = -TAU * ((CENTRE - DIAL) * i as f64 / RATE).rem_euclid(1.0);
        *z *= Complex32::new(p.cos() as f32, p.sin() as f32);
    }
    add_noise(&mut y, power, snr_db, 25e3, 7);
    let mut r = (cubesat::DESCRIPTOR.make_receiver)(&StackConfig::new(values(&[]))).unwrap();
    r.configure(&InputInfo::new(RATE, CENTRE, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in y.chunks(50_000) {
        out.clear();
        r.feed(c, &mut out);
    }
    crate::common::rows(r.results(), "frames")
        .into_iter()
        .rev()
        .map(|(_, c)| {
            (
                cell(c, "from"),
                cell(c, "to"),
                cell(c, "path"),
                cell_num(c, "pid").map_or(-1, |v| v as i64),
                cell_bytes(c, "data"),
            )
        })
        .collect()
}

/// The frame that `send` builds for `args` from [`CALL`].
fn frame_of(dest: &str, path: &str, pid: i64, info: &[u8]) -> Vec<u8> {
    tx::ui_frame(tx::source(CALL).unwrap(), dest, path, pid, info).unwrap()
}

#[test]
fn afsk_1200_comes_back_from_the_receiver() {
    let x = one_burst(
        &[],
        &[&[("destination", "CQ"), ("path", "ARISS"), ("text", "Hello from the uplink")]],
    );
    let got = received(&x, 0.25, 12.0);
    let want = frame_of("CQ", "ARISS", 0xF0, b"Hello from the uplink");
    assert_eq!(got.len(), 1, "{got:?}");
    let (from, to, path, pid, data) = &got[0];
    assert_eq!(data, &want);
    assert_eq!((from.as_str(), to.as_str(), *pid), ("N0CALL-7", "CQ", 0xF0));
    assert!(path.contains("ARISS"), "{path}");
}

#[test]
fn g3ruh_9600_comes_back_from_the_receiver() {
    let set = [("tx_rate", Value::Choice(1))];
    let x = one_burst(
        &set,
        &[&[("destination", "TEST-1"), ("hex", "00 01 7E 7D FF C0 DB"), ("pid", "204")]],
    );
    let got = received(&x, 0.25, 12.0);
    let want = frame_of("TEST-1", "", 204, &[0x00, 0x01, 0x7E, 0x7D, 0xFF, 0xC0, 0xDB]);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].4, want);
    assert_eq!((got[0].1.as_str(), got[0].3), ("TEST-1", 204));
}

#[test]
fn frames_queued_during_a_transmission_follow_it() {
    let (mut rx, sink) = station(&[]);
    send(rx.as_mut(), &[("text", "first")]).unwrap();
    send(rx.as_mut(), &[("text", "second")]).unwrap();
    send(rx.as_mut(), &[("text", "third")]).unwrap();
    assert_eq!(fields(rx.as_ref()).tx_waiting, 2);
    until_quiet(rx.as_mut(), 30.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 2, "the first frame, then the two that waited");
    let mut x = b[0].0.clone();
    x.extend(std::iter::repeat_n(Complex32::default(), (0.2 * RATE) as usize));
    x.extend_from_slice(&b[1].0);
    let got: Vec<Vec<u8>> = received(&x, 0.25, 12.0).into_iter().map(|r| r.4).collect();
    let want: Vec<Vec<u8>> = ["first", "second", "third"]
        .iter()
        .map(|t| frame_of("CQ", "", 0xF0, t.as_bytes()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn the_address_octets_are_those_worked_by_hand() {
    // AX.25 2.2 clause 3.12: each character shifted left by one; the destination with C set, the source with C clear and
    // the extension bit on the last address; control 03 (UI) and PID F0.
    let f = frame_of("CQ", "", 0xF0, b"A");
    let want = [
        0x86, 0xA2, 0x40, 0x40, 0x40, 0x40, 0xE0, // CQ, SSID 0, C = 1
        0x9C, 0x60, 0x86, 0x82, 0x98, 0x98, 0x6F, // N0CALL, SSID 7, C = 0, last
        0x03, 0xF0, 0x41,
    ];
    assert_eq!(f, want);
    // A digipeater follows the source with H clear and takes the extension bit.
    let f = frame_of("CQ", "ARISS", 0xF0, b"A");
    assert_eq!(f[13], 0x6E, "the source is no longer last");
    assert_eq!(&f[14..21], &[0x82, 0xA4, 0x92, 0xA6, 0xA6, 0x40, 0x61]);
    assert_eq!(&f[21..], &[0x03, 0xF0, 0x41]);
}

#[test]
fn the_fcs_and_the_bit_stuffing_are_those_worked_by_hand() {
    let lsb = |bytes: &[u8]| -> String {
        bytes
            .iter()
            .flat_map(|b| (0..8).map(move |k| if (b >> k) & 1 == 1 { '1' } else { '0' }))
            .collect()
    };
    let sent = |frame: &[u8]| -> String {
        let mut bits = Vec::new();
        Transmitter::new(Line::Nrz).encode(&[frame], 1, &mut bits);
        bits.iter().map(|b| if *b == 1 { '1' } else { '0' }).collect()
    };
    // The check value of CRC-16/X.25 for "123456789" is 906E, sent low octet first; no run of five ones, so no stuffing.
    let flag = "01111110";
    let want = format!("{flag}{}{}{flag}", lsb(b"123456789"), lsb(&[0x6E, 0x90]));
    assert_eq!(sent(b"123456789"), want);
    // The octet FF has FCS FF00: a zero follows every fifth one, in the data and in the FCS.
    assert_eq!(
        sent(&[0xFF]),
        "01111110\
         111110111\
         00000000\
         111110111\
         01111110"
    );
    // The frame of the example has FCS F356, sent as 56 F3; the run F0 41 holds five ones, so a zero follows them.
    let f = frame_of("CQ", "", 0xF0, b"A");
    assert_eq!(
        sent(&f),
        format!(
            "{flag}\
             0110000101000101000000100000001000000010000000100000011100111001\
             0000011001100001010000010001100100011001111100110110000000000111\
             11000000100110101011001111\
             {flag}"
        )
    );
    // The octet 7E (a flag pattern in the data) has FCS 6A81.
    assert_eq!(
        sent(&[0x7E]),
        "01111110\
         011111010\
         10000001\
         01010110\
         01111110"
    );
}

/// The 99 % power bandwidth of `x`: Hann-windowed spectra of 8192 points with half overlap, between the frequencies below
/// which 0.5 % and above which 0.5 % of the power lie. Returns (low, high), Hz.
fn occupied(x: &[Complex32]) -> (f64, f64) {
    let n = 8192;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(n);
    let mut acc = vec![0.0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let mut i = 0;
    while i + n <= x.len() {
        for k in 0..n {
            let w = 0.5 - 0.5 * (TAU * k as f64 / n as f64).cos();
            buf[k] = x[i + k] * w as f32;
        }
        fft.process(&mut buf);
        for k in 0..n {
            acc[k] += f64::from(buf[k].norm_sqr());
        }
        i += n / 2;
    }
    let mut bins: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let f = if k < n / 2 { k as f64 } else { k as f64 - n as f64 };
            (f * RATE / n as f64, acc[k])
        })
        .collect();
    bins.sort_by(|p, q| p.0.total_cmp(&q.0));
    let total: f64 = bins.iter().map(|b| b.1).sum();
    let (mut c, mut lo, mut hi) = (0.0, f64::NAN, f64::NAN);
    for (f, p) in &bins {
        c += p;
        if lo.is_nan() && c >= 0.005 * total {
            lo = *f;
        }
        if hi.is_nan() && c >= 0.995 * total {
            hi = *f;
        }
    }
    (lo, hi)
}

#[test]
fn level_and_occupied_bandwidth() {
    let text = [("text", "The occupied bandwidth of the AX.25 uplink, measured on a frame of some length.")];
    for (rate, max) in [(0u16, 16e3), (1, 25e3)] {
        let x = one_burst(&[("tx_rate", Value::Choice(rate)), ("tx_level", Value::F64(0.4))], &[&text]);
        // A constant envelope at the level between the 5 ms rise and fall (the rate change adds a short tail).
        let edge = (0.01 * RATE) as usize;
        let inner = &x[edge..x.len() - edge];
        let worst = inner.iter().map(|z| (z.norm() - 0.4).abs()).fold(0.0f32, f32::max);
        assert!(worst < 0.01, "level off by {worst}");
        assert!(x.iter().all(|z| z.norm() <= tx::PEAK + 1e-6));
        let (lo, hi) = occupied(&x);
        eprintln!("{} bit/s: occupied bandwidth {lo:.0} to {hi:.0} Hz, {:.0} Hz", [1200, 9600][rate as usize], hi - lo);
        assert!(hi - lo < max, "{lo} to {hi}");
        assert!((hi + lo).abs() < 2e3, "centred: {lo} to {hi}");
    }
}

#[test]
fn refused_with_the_switch_off() {
    let (mut rx, sink) = station(&[]);
    rx.set_transmit(false);
    assert!(send(rx.as_mut(), &[("text", "off")]).is_err());
    idle(rx.as_mut(), 0.3);
    assert!(sink.bursts().is_empty());
    // The switch going off during a transmission ends it with the end mark.
    rx.set_transmit(true);
    send(rx.as_mut(), &[("text", "on, then off")]).unwrap();
    idle(rx.as_mut(), 0.2);
    rx.set_transmit(false);
    idle(rx.as_mut(), 0.2);
    assert!(!fields(rx.as_ref()).tx_sending);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].1, "the end mark");
}

#[test]
fn refused_without_a_valid_callsign() {
    for call in ["", "  ", "N0CALLX", "N0CALL-16", "n0-call"] {
        let (mut rx, sink) = station_as(&[], call);
        let e = send(rx.as_mut(), &[("text", "no callsign")]).unwrap_err();
        let want = if call.trim().is_empty() {
            "the station callsign is not set"
        } else {
            "is not an AX.25 address"
        };
        assert!(e.to_string().contains(want) || format!("{e:?}").contains(want), "{call:?}: {e}");
        idle(rx.as_mut(), 0.2);
        assert!(sink.bursts().is_empty(), "{call:?}");
    }
    // A lower-case callsign is sent in upper case.
    let (mut rx, _) = station_as(&[], "n0call");
    send(rx.as_mut(), &[("text", "lower case")]).unwrap();
}

#[test]
fn bad_frames_are_refused() {
    let (mut rx, sink) = station(&[]);
    let nine = "A1,A2,A3,A4,A5,A6,A7,A8,A9";
    let long = "x".repeat(tx::MAX_INFO + 1);
    for args in [
        &[][..],
        &[("text", "a"), ("hex", "01")][..],
        &[("hex", "0")][..],
        &[("hex", "0G")][..],
        &[("text", long.as_str())][..],
        &[("text", "a"), ("destination", "TOOLONGX")][..],
        &[("text", "a"), ("path", nine)][..],
        &[("text", "a"), ("path", "ARISS*")][..],
    ] {
        assert!(send(rx.as_mut(), args).is_err(), "{args:?}");
    }
    assert!(tx::ui_frame(tx::source(CALL).unwrap(), "CQ", "", 255, b"a").is_err());
    idle(rx.as_mut(), 0.2);
    assert!(sink.bursts().is_empty());
}

#[test]
fn halt_ends_the_transmission_with_the_end_mark() {
    let (mut rx, sink) = station(&[]);
    send(rx.as_mut(), &[("text", "a frame that is halted while it is on air")]).unwrap();
    send(rx.as_mut(), &[("text", "a frame that waits and is dropped")]).unwrap();
    idle(rx.as_mut(), 0.2);
    command_with(rx.as_mut(), "halt", &[]).unwrap();
    until_quiet(rx.as_mut(), 5.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].1, "the end mark");
    // 0.2 s on air, one bit and the 5 ms fall, the pass in which it ends and the tail of the rate change.
    assert!(b[0].0.len() < (0.45 * RATE) as usize, "{}", b[0].0.len());
    let f = fields(rx.as_ref());
    assert_eq!((f.tx_sending, f.tx_waiting), (false, 0));
}

