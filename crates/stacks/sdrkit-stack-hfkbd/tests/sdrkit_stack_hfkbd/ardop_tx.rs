//! The ARDOP sender: the worked examples (leader signs, ACK symbols, durations, session ids), the samples against the
//! symbol formula, FEC text, ID and Ping read back by the stack's own receiver without noise and at 12 dB, the occupied
//! bandwidth, the refusals, the link table over a lossy channel of whole frames, and two stacks that make a link, exchange
//! data and disconnect while each hears its own frames too; with answering off a request is shown and nothing is sent.

use crate::common::*;
use crate::tx_common::*;
use hf::ardop::link::{Heard, Link, LinkOut};
use hf::ardop::tx::{frame_symbols, session_id, FecType, FrameWriter, SymKind};
use hf::ardop::{frame_type, type_check, unpack6, TxFrame};
use hf::typed::ArdopLinkState;
use sdrkit_core::{Complex32, Owner, Receiver, Value};
use sdrkit_stack_hfkbd as hf;
use std::collections::VecDeque;

/// Worked examples: a 240 ms leader is 12 symbols signed + - + - + - + - + - + +; ACK quality 70 is 0xF0, with session 0x7C
/// its second byte is 0x8C and its type tones are 1575, 1575, 1425, 1425, parity, 1525, 1425, 1575, 1425, parity; the ID frame
/// lasts 1.72 s, an ACK 0.44 s and a 64-byte 4FSK.500.100 frame 3.76 s; the session ids of N0CALL to N1CALL and VK2ABC to
/// VK2RMS are 0xC4 and 0x51, so the first data frame of the first link has the second byte 0x8E.
#[test]
fn ardop_tx_worked_examples() {
    let mut s = Vec::new();
    frame_symbols(&TxFrame::bare(0xF0, 0x7C).with_leader(240), &mut s);
    let signs: Vec<f32> = s[..12].iter().map(|x| x.sign).collect();
    assert_eq!(signs, [1., -1., 1., -1., 1., -1., 1., -1., 1., -1., 1., 1.]);
    assert!(s[..12].iter().all(|x| x.kind == SymKind::Leader));
    let p = 1425.0 + 50.0 * f64::from(type_check(0xF0));
    let tones: Vec<f64> = s[12..22]
        .iter()
        .map(|x| match x.kind {
            SymKind::Tone(f) => f,
            _ => panic!("a tone"),
        })
        .collect();
    assert_eq!(
        tones,
        [1575., 1575., 1425., 1425., p, 1525., 1425., 1575., 1425., p]
    );
    assert_eq!(0xF0 ^ 0x7C, 0x8C);
    let rs = &mut hf::ardop::RsCodes::new();
    let id = TxFrame::two_fields(rs, 0x30, "N0CALL", "QF56").with_leader(240);
    let data = TxFrame::data_bytes(rs, 0x4A, 0x7C, &[0x55; 64]).with_leader(240);
    for (f, want) in [
        (&id, 1.72),
        (&TxFrame::bare(0xF0, 0x7C).with_leader(240), 0.44),
        (&data, 3.76),
    ] {
        assert!((f.air_s() - want).abs() < 1e-9, "{:#x}", f.code);
        let mut w = FrameWriter::new(RATE, 0.5, false);
        w.push_frame(f);
        assert_eq!(w.end_sample(), (want * RATE).round() as u64);
    }
    assert_eq!(session_id("N0CALL", "N1CALL"), 0xC4);
    assert_eq!(session_id("VK2ABC", "VK2RMS"), 0x51);
    assert_eq!(0x4A ^ session_id("N0CALL", "N1CALL"), 0x8E);
}

/// Each sample is the symbol formula: sign x (-j) x the tone from phase zero at the symbol start, the leader the sum of its two
/// tones at half the level, outside the 1 ms fades at the two ends of the frame; LSB is the mirror.
#[test]
fn ardop_tx_samples_follow_the_formula() {
    let f = TxFrame::data_bytes(&mut hf::ardop::RsCodes::new(), 0x4C, 0x21, b"AB").with_leader(100);
    let mut syms = Vec::new();
    frame_symbols(&f, &mut syms);
    for lower in [false, true] {
        let mut w = FrameWriter::new(RATE, 0.5, lower);
        w.push_frame(&f);
        let mut x = vec![Complex32::default(); w.end_sample() as usize];
        assert_eq!(w.fill(&mut x), x.len());
        assert!(w.finished());
        let fade = (0.001 * RATE) as usize;
        let mut t = 0.0;
        let mut worst = 0.0f64;
        for s in &syms {
            let a = (t * RATE).round() as usize;
            t += s.s;
            let b = (t * RATE).round() as usize;
            let (lo, hi) = (a.max(fade), b.min(x.len() - fade));
            for (n, y) in x.iter().enumerate().take(hi).skip(lo) {
                let tau = (n - a) as f64 / RATE;
                let tone = |hz: f64| {
                    let p = std::f64::consts::TAU * hz * tau;
                    (p.sin(), -p.cos())
                };
                let (mut re, mut im) = match s.kind {
                    SymKind::Leader => {
                        let (a, b) = (tone(1475.0), tone(1525.0));
                        ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
                    }
                    SymKind::Tone(hz) => tone(hz),
                    SymKind::Quiet | SymKind::Carriers { .. } => (0.0, 0.0),
                };
                re *= f64::from(s.sign) * 0.5;
                im *= f64::from(s.sign) * 0.5;
                if lower {
                    im = -im;
                }
                let d = (re - f64::from(y.re)).hypot(im - f64::from(y.im));
                worst = worst.max(d);
            }
        }
        assert!(worst < 1e-3, "lower {lower}: {worst}");
    }
}

/// The settings of the sending stack.
fn fec(t: FecType) -> Vec<(&'static str, Value)> {
    vec![("fec_type", Value::Choice(t.index()))]
}

/// The one burst that `set` sends after `cmd` with `args`; it ends with its end mark.
fn burst_of(set: &[(&str, Value)], cmd: &str, args: &[(&str, Value)]) -> Vec<Complex32> {
    let (mut rx, sink) = station_of(&hf::ARDOP, set, None, CALL, "QF56od");
    command(rx.as_mut(), cmd, args).expect("sent");
    for _ in 0..1200 {
        idle(rx.as_mut(), 0.1);
        if !flag_of(rx.as_ref(), "tx_sending") && field_num_of(rx.as_ref(), "tx_waiting") == 0.0 {
            break;
        }
    }
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one burst");
    assert!(b[0].ended, "the end mark");
    b.remove(0).samples
}

fn flag_of(rx: &dyn Receiver, key: &str) -> bool {
    let res = Owner::results(rx);
    let id = res.decl().field_id(key).expect("the field");
    matches!(res.fields().value(id), Some(Value::Bool(true)))
}

fn text_field_of(rx: &dyn Receiver, key: &str) -> String {
    field_text(Owner::results(rx), key)
}

fn field_num_of(rx: &dyn Receiver, key: &str) -> f64 {
    field_num(Owner::results(rx), key)
}

/// The frames the stack's own receiver reads in `x`, oldest first: name, body good, from, to, locator, text.
fn read(x: &[Complex32]) -> Vec<(String, bool, String, String, String, String)> {
    let r = feed(&harness(&hf::ARDOP), x);
    let mut v: Vec<_> = rows(&r, "frames")
        .iter()
        .map(|row| {
            (
                cell_num(row, "time_s"),
                (
                    cell_text(row, "name"),
                    cell_flag(row, "body_ok").unwrap_or(false),
                    cell_text(row, "from"),
                    cell_text(row, "to"),
                    cell_text(row, "locator"),
                    cell_text(row, "text"),
                ),
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().map(|x| x.1).collect()
}

const FEC_TEXT: &str = "CQ CQ DE N0CALL: THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 73";

/// Whether the frames read are the FEC frames of the text: each of the type, even and odd in turn, body good, the text whole.
fn fec_read(f: &[(String, bool, String, String, String, String)], t: FecType, text: &str) -> bool {
    let name = frame_type(t.code()).unwrap().name;
    let base = name.trim_end_matches(".E");
    let size = frame_type(t.code()).unwrap().data;
    let n = text.len().div_ceil(size);
    f.len() == n
        && f.iter()
            .enumerate()
            .all(|(i, x)| x.1 && x.0 == format!("{base}.{}", if i % 2 == 0 { "E" } else { "O" }))
        && f.iter().map(|x| x.5.as_str()).collect::<String>() == text
}

/// Without noise every FEC frame type carries the text whole; the ID frame carries the callsign and the locator, the Ping the
/// caller and the called station.
#[test]
fn ardop_tx_loop_clean() {
    for t in [
        FecType::Fsk200_50s,
        FecType::Fsk500_100s,
        FecType::Fsk500_100,
    ] {
        let x = burst_of(&fec(t), "send", &[("text", Value::Text(FEC_TEXT.into()))]);
        let f = read(&padded(&x, 2.0));
        assert!(fec_read(&f, t, FEC_TEXT), "{t:?}: {f:?}");
    }
    let f = read(&padded(&burst_of(&[], "send_id", &[]), 2.0));
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0.as_str(), f[0].1), ("IDFrame", true));
    assert_eq!((f[0].2.as_str(), f[0].4.as_str()), ("N0CALL", "QF56OD"));
    let f = read(&padded(
        &burst_of(&[], "ping", &[("target", Value::Text("vk2rms".into()))]),
        2.0,
    ));
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].0.as_str(), f[0].1), ("Ping", true));
    assert_eq!((f[0].2.as_str(), f[0].3.as_str()), ("N0CALL", "VK2RMS"));
}

/// At 12 dB in 2.5 kHz, the SNR from which the receive specification reads bodies, over the seeds 1 to 10: every frame of
/// the text good in at least 9 of 10 for each FEC type, and the ID frame in at least 9 of 10.
#[test]
fn ardop_tx_loop_12db() {
    for (k, t) in [
        FecType::Fsk200_50s,
        FecType::Fsk500_100s,
        FecType::Fsk500_100,
    ]
    .into_iter()
    .enumerate()
    {
        let x = burst_of(&fec(t), "send", &[("text", Value::Text(FEC_TEXT.into()))]);
        let good = (1..=10u64)
            .filter(|s| {
                fec_read(
                    &read(&noisy(&x, 12.0, 2.0, 700 + 10 * k as u64 + s)),
                    t,
                    FEC_TEXT,
                )
            })
            .count();
        eprintln!("{t:?} at 12 dB: {good} of 10");
        assert!(good >= 9, "{t:?} at 12 dB: {good} of 10");
    }
    let x = burst_of(&[], "send_id", &[]);
    let good = (1..=10u64)
        .filter(|s| {
            let f = read(&noisy(&x, 12.0, 2.0, 800 + s));
            f.len() == 1 && f[0].1 && f[0].2 == "N0CALL"
        })
        .count();
    eprintln!("ID at 12 dB: {good} of 10");
    assert!(good >= 9, "ID at 12 dB: {good} of 10");
}

/// The 99 % power bandwidth of an FEC burst: the 500 Hz types within the 500 Hz from 1250 to 1750 Hz above the dial; the
/// 200 Hz type within 1375 to 1625 Hz and at most 10 % over 200 Hz (its tones span 150 Hz at 50 baud with a continuous
/// phase). The peak is the level, 0.5. On LSB the band is the mirror image.
#[test]
fn ardop_tx_bandwidth() {
    let text = FEC_TEXT.repeat(4);
    for (t, lo, hi, width) in [
        (FecType::Fsk200_50s, 1375.0, 1625.0, 220.0),
        (FecType::Fsk500_100s, 1250.0, 1750.0, 500.0),
        (FecType::Fsk500_100, 1250.0, 1750.0, 500.0),
    ] {
        let x = burst_of(&fec(t), "send", &[("text", Value::Text(text.clone()))]);
        let (a, b) = occupied(&x);
        eprintln!("{t:?}: {a:.1} to {b:.1} Hz, {:.1} Hz", b - a);
        assert!(a >= lo && b <= hi && b - a <= width, "{t:?}: {a} to {b}");
        let peak = x.iter().map(|z| z.norm()).fold(0.0, f32::max);
        assert!((peak - 0.5).abs() < 0.01, "{peak}");
    }
    let mut set = fec(FecType::Fsk500_100);
    set.push(("demod", Value::Choice(hf::front::Demod::Lsb.index())));
    let x = burst_of(&set, "send", &[("text", Value::Text(text))]);
    let (a, b) = occupied(&x);
    assert!(a >= -1750.0 && b <= -1250.0, "{a} to {b}");
}

/// With transmit off, without a callsign, with a callsign a frame cannot carry, with a bad called station and on NFM, every
/// request is refused and nothing is sent. `stop` ends a burst at once with the end mark.
#[test]
fn ardop_tx_refusals_and_stop() {
    let text = [("text", Value::Text("HELLO".into()))];
    let target = [("target", Value::Text("N1CALL".into()))];
    let (mut rx, sink) = station(&hf::ARDOP, &[], None);
    rx.set_transmit(false);
    for (cmd, args) in [
        ("send", &text[..]),
        ("send_id", &[][..]),
        ("ping", &target[..]),
        ("connect", &target[..]),
    ] {
        assert!(command(rx.as_mut(), cmd, args).is_err(), "{cmd}");
    }
    idle(rx.as_mut(), 1.0);
    assert!(sink.bursts().is_empty());
    for call in ["", "N0CALL-TOOLONG", "N0 CALL"] {
        let (mut rx, sink) = station_of(&hf::ARDOP, &[], None, call, "");
        assert!(command(rx.as_mut(), "send", &text).is_err(), "{call:?}");
        assert!(
            command(rx.as_mut(), "connect", &target).is_err(),
            "{call:?}"
        );
        idle(rx.as_mut(), 1.0);
        assert!(sink.bursts().is_empty());
    }
    let (mut rx, _) = station(&hf::ARDOP, &[], None);
    let bad = [("target", Value::Text("N1 CALL".into()))];
    assert!(command(rx.as_mut(), "connect", &bad).is_err());
    let nfm = [("demod", Value::Choice(hf::front::Demod::Nfm.index()))];
    let (mut rx, _) = station(&hf::ARDOP, &nfm, None);
    assert!(command(rx.as_mut(), "send", &text).is_err());
    // Stop.
    let (mut rx, sink) = station(&hf::ARDOP, &[], None);
    command(
        rx.as_mut(),
        "send",
        &[("text", Value::Text(FEC_TEXT.into()))],
    )
    .unwrap();
    idle(rx.as_mut(), 1.0);
    command(rx.as_mut(), "stop", &[]).unwrap();
    idle(rx.as_mut(), 1.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    assert!(b[0].samples.len() <= (1.1 * RATE) as usize + 1);
}

/// A small fixed random source.
struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The frame as the receiver reads it.
fn heard_of(f: &TxFrame) -> Heard {
    let mut h = Heard {
        code: f.code,
        session: f.session,
        header_ok: true,
        body_ok: true,
        ..Heard::default()
    };
    match f.code {
        0x31..=0x38 | 0x3E => {
            h.from = Some(unpack6(&f.body[0..6]));
            h.to = Some(unpack6(&f.body[6..12]));
        }
        0x4A | 0x4B => {
            let n = f.body[0] as usize;
            h.data = f.body[1..1 + n].to_vec();
        }
        _ => {}
    }
    h
}

/// One side of a link at the frame level: the link, its frames in flight (the time each arrives and the frame), and the end
/// of its last burst.
struct Side {
    link: Link,
    flight: VecDeque<(f64, TxFrame)>,
    busy_until: f64,
    got: Vec<u8>,
}

impl Side {
    fn new(call: &str, answer: bool) -> Self {
        let mut link = Link::new();
        link.set(call, 240, answer);
        Side {
            link,
            flight: VecDeque::new(),
            busy_until: 0.0,
            got: Vec::new(),
        }
    }
    /// Sends the frames of `out` (each after the one before it, 0.1 s of silence first); a frame is lost when `drop`.
    fn take(&mut self, out: &mut Vec<LinkOut>, now: f64, rng: &mut Rng, loss: f64) {
        for o in out.drain(..) {
            match o {
                LinkOut::Send(f) => {
                    let start = self.busy_until.max(now);
                    let end = start + 0.1 + f.air_s();
                    self.busy_until = end;
                    if rng.unit() >= loss {
                        // The other side reads it 0.2 s after its end.
                        self.flight.push_back((end + 0.2, f));
                    }
                }
                LinkOut::Deliver(d) => self.got.extend(d),
                _ => {}
            }
        }
    }
}

/// Runs a link at the frame level: the caller sends `a_data`, the called station `b_data` once connected, the caller ends the
/// link once it has the called station's data; frames are lost with the chance `loss`, all of them after `cut_s`. Returns the
/// two sides and the time.
fn frame_link(a_data: &[u8], b_data: &[u8], loss: f64, cut_s: f64, seed: u64) -> (Side, Side, f64) {
    let mut a = Side::new("N0CALL", false);
    let mut b = Side::new("N1CALL", true);
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    a.link.connect("N1CALL", 0.0, &mut out).unwrap();
    a.link.queue(a_data, 0.0, &mut out).unwrap();
    a.take(&mut out, 0.0, &mut rng, loss);
    let mut b_queued = false;
    let mut disc = false;
    let mut now = 0.0;
    while now < 3000.0 {
        now += 0.05;
        let l = if now > cut_s { 1.0 } else { loss };
        while a.flight.front().is_some_and(|f| f.0 <= now) {
            let (_, f) = a.flight.pop_front().unwrap();
            b.link.heard(&heard_of(&f), now, &mut out);
            b.take(&mut out, now, &mut rng, l);
        }
        while b.flight.front().is_some_and(|f| f.0 <= now) {
            let (_, f) = b.flight.pop_front().unwrap();
            a.link.heard(&heard_of(&f), now, &mut out);
            a.take(&mut out, now, &mut rng, l);
        }
        a.link.tick(now, &mut out);
        a.take(&mut out, now, &mut rng, l);
        b.link.tick(now, &mut out);
        b.take(&mut out, now, &mut rng, l);
        if !b_queued && b.link.state() == ArdopLinkState::Receiving {
            b.link.queue(b_data, now, &mut out).unwrap();
            b.take(&mut out, now, &mut rng, l);
            b_queued = true;
        }
        if !disc && a.got.len() == b_data.len() {
            a.link.disconnect(now, &mut out);
            a.take(&mut out, now, &mut rng, l);
            disc = true;
        }
        if !a.link.active() && !b.link.active() && a.link.state() != ArdopLinkState::Connecting {
            break;
        }
    }
    (a, b, now)
}

/// The link table at the frame level: 2000 bytes from the caller and 500 back (the BREAK turn) over a channel that loses 20 %
/// of the frames, over the seeds 1 to 5: the bytes are identical, the link ends with DISC and END, no side fails. With every
/// frame lost after the connection the caller fails with "no answer from the other station".
#[test]
fn ardop_tx_link_frames() {
    let a_data: Vec<u8> = (0..2000u32).map(|i| (i * 7 % 251) as u8).collect();
    let b_data: Vec<u8> = (0..500u32).map(|i| (i * 13 % 241) as u8).collect();
    for seed in 1..=5u64 {
        let (a, b, t) = frame_link(&a_data, &b_data, 0.2, f64::INFINITY, seed * 0x9E37_79B9);
        eprintln!("seed {seed}: {t:.0} s");
        assert_eq!(b.got, a_data, "seed {seed}");
        assert_eq!(a.got, b_data, "seed {seed}");
        assert_eq!(
            a.link.state(),
            ArdopLinkState::Closed,
            "seed {seed}: {}",
            a.link.reason()
        );
        assert_eq!(a.link.reason(), "disconnected");
        assert_eq!(b.link.state(), ArdopLinkState::Closed);
        assert_eq!(b.link.reason(), "the other station disconnected");
    }
    let (a, _, _) = frame_link(&a_data, &b_data, 0.0, 20.0, 1);
    assert_eq!(a.link.state(), ArdopLinkState::Failed);
    assert_eq!(a.link.reason(), "no answer from the other station");
}

/// After the stack answered the other station's DISC, a DISC repeated within 99 s gets END again and later nothing; after
/// `abort` or a link that the stack ended, a DISC gets nothing.
#[test]
fn ardop_tx_link_sends_nothing_after_it_closed() {
    let sends = |out: &[LinkOut]| out.iter().filter(|o| matches!(o, LinkOut::Send(_))).count();
    let disc = |s: u8| heard_of(&TxFrame::bare(0x29, s));
    let mut out = Vec::new();
    let mut a = Link::new();
    a.set("N0CALL", 240, false);
    a.connect("N1CALL", 0.0, &mut out).unwrap();
    let s = session_id("N0CALL", "N1CALL");
    a.heard(&heard_of(&TxFrame::con_ack(0x3A, 240, s)), 2.0, &mut out);
    assert_eq!(a.state(), ArdopLinkState::Sending);
    out.clear();
    a.heard(&disc(s), 3.0, &mut out);
    assert_eq!((a.state(), sends(&out)), (ArdopLinkState::Closed, 1));
    out.clear();
    a.heard(&disc(s), 50.0, &mut out);
    assert_eq!(sends(&out), 1, "END again within 99 s");
    out.clear();
    a.heard(&disc(s), 120.0, &mut out);
    assert_eq!(sends(&out), 0, "nothing after 99 s");
    // Stopped.
    a.connect("N1CALL", 200.0, &mut out).unwrap();
    a.heard(&heard_of(&TxFrame::con_ack(0x3A, 240, s)), 202.0, &mut out);
    a.abort("stopped", &mut out);
    out.clear();
    a.heard(&disc(s), 203.0, &mut out);
    assert_eq!((a.state(), sends(&out)), (ArdopLinkState::Closed, 0));
}

/// Two stacks, each hearing its own samples and the other's with noise at 20 dB in 2.5 kHz; each burst is lost whole with
/// the chance `loss`. Runs `step` after each pass of 20 ms until it returns true or `limit` seconds pass. Returns the stacks,
/// the time and the writes of each to its transmit stream.
fn two_stacks(
    a_set: &[(&str, Value)],
    b_set: &[(&str, Value)],
    limit: f64,
    loss: f64,
    mut step: impl FnMut(&mut dyn Receiver, &mut dyn Receiver, f64) -> bool,
) -> (Box<dyn Receiver>, Box<dyn Receiver>, f64, usize, usize) {
    let (mut a, sa) = station_of(&hf::ARDOP, a_set, None, "N0CALL", "QF56");
    let (mut b, sb) = station_of(&hf::ARDOP, b_set, None, "N1CALL", "QF57");
    let n = (RATE / 50.0) as usize;
    let (mut ia, mut ib) = (0, 0);
    let (mut qa, mut qb): (VecDeque<Complex32>, VecDeque<Complex32>) = Default::default();
    let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(RATE / 2500.0, 99).unwrap();
    let mut noise = vec![Complex32::default(); n];
    let gain = (0.25f64 / 10f64.powf(2.0)).sqrt() as f32;
    let mut out = sdrkit_core::Output::default();
    let mut t = 0.0;
    let (mut na, mut nb) = (0, 0);
    let mut rng = Rng(0x5DEE_CE66);
    // Per side: a burst is open, and it is lost.
    let mut st = [(false, false); 2];
    while t < limit {
        for (k, (sink, i, q, count)) in [
            (&sa, &mut ia, &mut qa, &mut na),
            (&sb, &mut ib, &mut qb, &mut nb),
        ]
        .into_iter()
        .enumerate()
        {
            for (w, end) in sink.take_since(i) {
                *count += 1;
                if !st[k].0 {
                    st[k] = (true, rng.unit() < loss);
                }
                if !st[k].1 {
                    q.extend(w);
                }
                if end {
                    st[k].0 = false;
                }
            }
        }
        noise.fill(Complex32::default());
        awgn.process(&mut noise);
        let x: Vec<Complex32> = noise
            .iter()
            .map(|w| {
                qa.pop_front().unwrap_or_default() + qb.pop_front().unwrap_or_default() + w * gain
            })
            .collect();
        out.clear();
        a.feed(&x, &mut out);
        out.clear();
        b.feed(&x, &mut out);
        t += 0.02;
        if step(a.as_mut(), b.as_mut(), t) {
            break;
        }
    }
    (a, b, t, na, nb)
}

/// Two stacks on one channel, each hearing its own frames, with a leader of 100 ms, passes of 20 ms and 20 % of the bursts
/// lost: the caller connects and sends 2000 bytes, the called station (answering on) sends 500 back, the caller disconnects.
/// Both receive the bytes exactly and close.
#[test]
fn ardop_tx_link_two_stacks() {
    let a_text: String = (0..2000)
        .map(|i| char::from(b'A' + (i * 7 % 26) as u8))
        .collect();
    let b_text: String = (0..500)
        .map(|i| char::from(b'0' + (i * 3 % 10) as u8))
        .collect();
    let short = ("leader_ms", Value::I64(100));
    let answer = [("auto_answer", Value::Bool(true)), short.clone()];
    let mut phase = 0;
    let (a, b, t, _, _) = two_stacks(&[short], &answer, 1500.0, 0.2, |a, b, _| {
        let st = |r: &dyn Receiver| text_field_of(r, "link_state");
        match phase {
            0 => {
                command(a, "connect", &[("target", Value::Text("N1CALL".into()))]).unwrap();
                command(a, "send", &[("text", Value::Text(a_text.clone()))]).unwrap();
                phase = 1;
            }
            1 if st(b).starts_with("Connected") => {
                command(b, "send", &[("text", Value::Text(b_text.clone()))]).unwrap();
                phase = 2;
            }
            2 if text_field_of(a, "link_text") == b_text => {
                command(a, "disconnect", &[]).unwrap();
                phase = 3;
            }
            3 => return st(a) == "Closed" && st(b) == "Closed",
            _ => {}
        }
        st(a) == "Failed" || st(b) == "Failed"
    });
    eprintln!("link of two stacks: {t:.1} s");
    assert_eq!(text_field_of(b.as_ref(), "link_text"), a_text);
    assert_eq!(text_field_of(a.as_ref(), "link_text"), b_text);
    assert_eq!(text_field_of(a.as_ref(), "link_state"), "Closed");
    assert_eq!(text_field_of(a.as_ref(), "link_reason"), "disconnected");
    assert_eq!(text_field_of(b.as_ref(), "link_state"), "Closed");
    assert_eq!(field_num_of(a.as_ref(), "link_sent"), a_text.len() as f64);
}

/// With answering off (the default) a connect request to the station callsign is shown and the called stack sends nothing.
#[test]
fn ardop_tx_no_answer_by_default() {
    let (_, b, _, _, nb) = two_stacks(&[], &[], 12.0, 0.0, |a, _, t| {
        if t < 0.03 {
            command(a, "connect", &[("target", Value::Text("N1CALL".into()))]).unwrap();
        }
        false
    });
    assert_eq!(nb, 0, "the called stack sent");
    assert_eq!(text_field_of(b.as_ref(), "link_request"), "N0CALL");
    assert_eq!(text_field_of(b.as_ref(), "link_state"), "Idle");
}
