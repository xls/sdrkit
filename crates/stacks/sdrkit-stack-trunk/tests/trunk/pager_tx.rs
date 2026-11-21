//! The POCSAG sender of the pager stack: pages sent through the stack's transmit stream come back from its own receive side
//! with the same address, function, type and text; the code words on air are those of ITU-R M.584-2; the deviation, the
//! level and the occupied bandwidth; the refusals and the end of a transmission.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{owner, InputInfo, Output, Owner, Receiver, StackConfig, Value, Values};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_proto_pager::pocsag::{self, IDLE, SYNC};
use sdrkit_proto_pager::Kind;
use sdrkit_stack_trunk::pager::gen::{pocsag_words, TestPage};
use sdrkit_stack_trunk::typed::{PagerTxFields, PAGER_PAGES};
use sdrkit_stack_trunk::PAGER;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

/// The session rate of the tests: one carrier (the setting `wide_input`).
const RATE: f64 = 192e3;
/// The tuned frequency.
const DIAL: f64 = 439.9875e6;

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

/// The settings of the stack with `set` on its defaults and `wide_input` on.
fn values(set: &[(&str, Value)]) -> Values {
    let schema = (PAGER.settings)();
    let mut s = Values::defaults(&schema);
    s.set(schema.id("wide_input").unwrap(), &Value::Bool(true))
        .unwrap();
    for (k, v) in set {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    s
}

/// The stack with `set`, transmit on, at [`RATE`] on [`DIAL`], and its transmit stream.
fn station(set: &[(&str, Value)]) -> (Box<dyn Receiver>, Arc<Sink>) {
    let mut rx = (PAGER.make_receiver)(&StackConfig::new(values(set))).unwrap();
    rx.configure(&InputInfo::new(RATE, DIAL, DIAL).recording())
        .unwrap();
    let sink = Arc::new(Sink::default());
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

/// Runs command `key` with `args`.
fn command(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), sdrkit_core::CommandError> {
    let id = Owner::commands(rx).id(key).unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    for (k, v) in args {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Sends a page of `kind` ("tone", "numeric" or "alpha").
fn send(
    rx: &mut dyn Receiver,
    address: i64,
    function: i64,
    kind: &str,
    text: &str,
) -> Result<(), sdrkit_core::CommandError> {
    let kinds = ["tone", "numeric", "alpha"];
    let k = kinds.iter().position(|x| *x == kind).unwrap() as u16;
    command(
        rx,
        "send",
        &[
            ("address", Value::I64(address)),
            ("function", Value::I64(function)),
            ("page_type", Value::Choice(k)),
            ("text", Value::Text(text.into())),
        ],
    )
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
fn fields(rx: &dyn Receiver) -> PagerTxFields {
    PagerTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
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

/// The one transmission that the stack with `set` sends for the page; it ends with its end mark.
fn one_burst(set: &[(&str, Value)], address: i64, function: i64, kind: &str, text: &str) -> Vec<Complex32> {
    let (mut rx, sink) = station(set);
    send(rx.as_mut(), address, function, kind, text).expect("the page is sent");
    until_quiet(rx.as_mut(), 120.0);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    assert!(b[0].1, "the end mark");
    b.remove(0).0
}

/// (protocol, address, function, type, message, damaged, bits repaired) of each page that the stack's receive side reads
/// from `x` with 0.3 s of silence before and after, and noise at `snr_db` in 25 kHz when given.
fn received(x: &[Complex32], snr_db: Option<f64>) -> Vec<(String, u32, String, String, String, bool, i64)> {
    let pad = vec![Complex32::default(); (0.3 * RATE) as usize];
    let mut y = pad.clone();
    y.extend_from_slice(x);
    y.extend_from_slice(&pad);
    if let Some(snr) = snr_db {
        add_noise_snr(&mut y, RATE, snr, 25e3, 7);
    }
    let mut rx = make(&PAGER, &[("wide_input", "true")]);
    configure(rx.as_mut(), RATE, DIAL, 0.0);
    feed(rx.as_mut(), &y, 16384);
    feed(rx.as_mut(), &pad, 16384);
    rows(results(rx.as_ref()), &PAGER_PAGES)
        .into_iter()
        .map(|p| {
            (
                p["protocol"].clone(),
                p["address"].parse().unwrap(),
                p["function"].clone(),
                p["type"].clone(),
                p["message"].clone(),
                p["damaged"] == "yes" || p["damaged"] == "true",
                p["bits_repaired"].parse().unwrap(),
            )
        })
        .collect()
}

/// The page comes back exactly once, whole and with no repaired bit.
fn check_loop(set: &[(&str, Value)], speed: &str, address: u32, function: i64, kind: &str, text: &str, snr: Option<f64>) {
    let x = one_burst(set, address.into(), function, kind, text);
    let got = received(&x, snr);
    let ty = match kind {
        _ if text.is_empty() => "tone",
        "numeric" => "numeric",
        "alpha" => "alpha",
        _ => "tone",
    };
    assert_eq!(
        got,
        vec![(
            format!("POCSAG {speed}"),
            address,
            function.to_string(),
            ty.to_string(),
            text.to_string(),
            false,
            0
        )],
        "{speed} {kind} {text:?}"
    );
}

/// The frequency of each step from sample n to n + 1, Hz.
fn steps(x: &[Complex32]) -> Vec<f64> {
    x.windows(2)
        .map(|p| f64::from((p[1] * p[0].conj()).arg()) * RATE / TAU)
        .collect()
}

#[test]
fn numeric_and_text_pages_come_back_at_each_rate_at_12_db() {
    for (rate, key) in [("512", 0u16), ("1200", 1), ("2400", 2)] {
        let set = [("pocsag_rate", Value::Choice(key))];
        check_loop(&set, rate, 1_234_561, 0, "numeric", "555-0123 U*", Some(12.0));
        check_loop(&set, rate, 1_234_567, 3, "alpha", "Hello from the pager stack.", Some(12.0));
    }
}

#[test]
fn an_address_in_each_frame_comes_back() {
    for k in 0..8u32 {
        let address = 1_000_000 + k;
        let words = pocsag_words(&[TestPage::new(address, 3, Kind::Alpha, "frame")]).unwrap();
        assert_eq!(words[0], SYNC);
        let at = 1 + 2 * (address & 7) as usize;
        assert_eq!(words[at], pocsag::address_word(address, 3).unwrap(), "frame {k}");
        assert!(words[1..at].iter().all(|&w| w == IDLE), "idle words before frame {k}");
        check_loop(&[], "1200", address, 3, "alpha", &format!("frame {k}"), None);
    }
}

#[test]
fn a_message_longer_than_one_batch_comes_back() {
    let text = "The quick brown fox jumps over the lazy dog; a message that runs over several batches of 16 code words. 0123456789";
    let words = pocsag_words(&[TestPage::new(1_234_562, 3, Kind::Alpha, text)]).unwrap();
    assert!(words.len() >= 3 * 17, "{} words", words.len());
    assert!(words.iter().step_by(17).all(|&w| w == SYNC));
    check_loop(&[], "1200", 1_234_562, 3, "alpha", text, Some(12.0));
    check_loop(&[], "1200", 1_234_561, 0, "numeric", "0123456789-0123456789-0123456789-0123456789-0123456789", None);
}

#[test]
fn both_polarities_come_back() {
    for key in [0u16, 1] {
        let set = [("polarity", Value::Choice(key))];
        check_loop(&set, "1200", 1_234_563, 3, "alpha", "polarity", Some(12.0));
        let set = [("polarity", Value::Choice(key)), ("pocsag_rate", Value::Choice(2))];
        check_loop(&set, "2400", 1_234_561, 0, "numeric", "2400", None);
    }
}

#[test]
fn a_tone_only_page_comes_back() {
    check_loop(&[], "1200", 1_234_564, 2, "alpha", "", None);
    check_loop(&[("pocsag_rate", Value::Choice(0))], "512", 1_234_565, 1, "tone", "", None);
}

#[test]
fn the_code_words_are_those_of_the_standard() {
    // The sync and the idle words are code words of BCH(31,21) with even parity.
    for w in [SYNC, IDLE] {
        assert_eq!(w.count_ones() % 2, 0, "{w:08X} has even parity");
        assert_eq!(bch_word(w >> 11), w, "{w:08X} is a code word");
    }
    assert_eq!(SYNC, 0x7CD2_15D8);
    assert_eq!(IDLE, 0x7A89_C197);
    // Address 1 234 567 (frame 7), function 3, "HELLO" with EOT.
    let w = pocsag_words(&[TestPage::new(1_234_567, 3, Kind::Alpha, "HELLO")]).unwrap();
    assert_eq!(&w[..15], &[[SYNC].as_slice(), &[IDLE; 14]].concat()[..]);
    assert_eq!(w[15], 0x4B5A_1A25);
    assert_eq!(w[16], 0x89A2_634D);
    assert_eq!(&w[17..20], &[SYNC, 0xCCF9_2332, 0x8000_0769]);
    assert!(w[20..].iter().all(|&x| x == IDLE));
    assert_eq!(w.len(), 34);
    // Address 1 234 561 (frame 1), function 0, numeric "555-0123".
    let w = pocsag_words(&[TestPage::new(1_234_561, 0, Kind::Numeric, "555-0123")]).unwrap();
    assert_eq!(&w[..6], &[SYNC, IDLE, IDLE, 0x4B5A_0780, 0xD555_81E5, 0xC261_9CE1]);
    assert!(w[6..].iter().all(|&x| x == IDLE));
    // The address word by hand: 1 234 567 >> 3 = 154 320 = 0x25AD0, the function 3 after it, the flag 0 first.
    assert_eq!(1_234_567u32 >> 3, 0x25AD0);
    assert_eq!(bch_word((0x25AD0 << 2) | 3), 0x4B5A_1A25);
    assert_eq!(pocsag::address_word(1_234_567, 3).unwrap(), 0x4B5A_1A25);
}

/// The code word of 21 bits `data` (the flag first): the 10 check bits of BCH(31,21), the remainder of `data` x^10 divided
/// by x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1, then the even parity bit.
fn bch_word(data: u32) -> u32 {
    let g = 0b111_0110_1001u32;
    let mut r = data << 10;
    for i in (10..31).rev() {
        if r & (1 << i) != 0 {
            r ^= g << (i - 10);
        }
    }
    let w = ((data << 10) | r) << 1;
    w | (w.count_ones() & 1)
}

#[test]
fn the_bits_on_air_are_the_preamble_and_the_words() {
    let text = "on air";
    for (key, polarity) in [(0u16, 1.0), (1, -1.0)] {
        let x = one_burst(&[("polarity", Value::Choice(key))], 1_234_566, 3, "alpha", text);
        let words = pocsag_words(&[TestPage::new(1_234_566, 3, Kind::Alpha, text)]).unwrap();
        let mut bits = Vec::new();
        pocsag::transmission_bits(&words, 576, &mut bits);
        let per_bit = RATE / 1200.0;
        assert_eq!(
            (x.len() as f64 / per_bit).round() as usize,
            bits.len(),
            "the length"
        );
        let f = steps(&x);
        let got: Vec<u8> = (0..bits.len())
            .map(|i| {
                let c = ((i as f64 + 0.5) * per_bit) as usize;
                u8::from(f[c] * polarity < 0.0)
            })
            .collect();
        assert_eq!(got, bits, "polarity {key}: a 1 is the lower frequency when normal");
        assert!(bits[..576].iter().enumerate().all(|(i, &b)| b == u8::from(i % 2 == 0)));
    }
}

#[test]
fn deviation_level_and_occupied_bandwidth() {
    let x = one_burst(&[], 1_234_567, 3, "alpha", "Deviation and bandwidth of the POCSAG sender.");
    // The level: a constant envelope at 0.5 between the 1 ms ramps.
    let ramp = (0.001 * RATE) as usize;
    let inner = &x[ramp..x.len() - ramp];
    assert!(inner.iter().all(|z| (z.norm() - 0.5).abs() < 1e-3), "level 0.5");
    assert!(x[0].norm() < 1e-3, "the rise starts at 0");
    // The deviation: the frequency at the centres of the bits.
    let f = steps(inner);
    let per_bit = RATE / 1200.0;
    let centres: Vec<f64> = (2..(inner.len() as f64 / per_bit) as usize - 2)
        .map(|i| f[((i as f64 + 0.5) * per_bit) as usize - ramp].abs())
        .collect();
    let dev = centres.iter().sum::<f64>() / centres.len() as f64;
    let max = centres.iter().copied().fold(0.0, f64::max);
    eprintln!("deviation: mean {dev:.1} Hz, largest {max:.1} Hz");
    assert!((dev - 4500.0).abs() < 25.0, "deviation {dev}");
    assert!(max < 4500.0 * 1.01, "largest {max}");
    // The 99 % power bandwidth.
    let (lo, hi) = occupied(&x);
    eprintln!("occupied bandwidth: {lo:.0} to {hi:.0} Hz, {:.0} Hz", hi - lo);
    assert!(hi - lo < 16e3, "{lo} to {hi}");
    assert!(hi - lo > 8e3, "{lo} to {hi}");
    assert!((hi + lo).abs() < 1e3, "centred: {lo} to {hi}");
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
fn refused_with_the_switch_off() {
    let (mut rx, sink) = station(&[]);
    rx.set_transmit(false);
    assert!(send(rx.as_mut(), 1_234_567, 3, "alpha", "off").is_err());
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
    // The switch going off during a transmission ends it with the end mark.
    rx.set_transmit(true);
    send(rx.as_mut(), 1_234_567, 3, "alpha", "on").unwrap();
    idle(rx.as_mut(), 0.3);
    rx.set_transmit(false);
    idle(rx.as_mut(), 0.3);
    assert!(!fields(rx.as_ref()).tx_sending);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].1, "the end mark");
}

#[test]
fn refused_for_flex() {
    let (mut rx, sink) = station(&[("protocol", Value::Choice(1))]);
    let e = send(rx.as_mut(), 1_234_567, 3, "alpha", "flex").unwrap_err();
    assert!(e.to_string().contains("FLEX"), "{e}");
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

#[test]
fn pages_that_cannot_be_sent_are_refused() {
    let (mut rx, sink) = station(&[]);
    let r = rx.as_mut();
    assert!(send(r, 2_097_152, 3, "alpha", "x").is_err(), "address above 21 bits");
    assert!(send(r, -1, 3, "alpha", "x").is_err(), "negative address");
    assert!(send(r, 2_007_664, 0, "tone", "").is_err(), "the idle word");
    assert!(send(r, 1_234_561, 0, "numeric", "12A").is_err(), "a letter in a numeric page");
    assert!(send(r, 1_234_561, 3, "alpha", "é").is_err(), "not 7-bit");
    assert!(send(r, 1_234_561, 3, "tone", "text").is_err(), "a tone-only page with text");
    assert!(
        command(r, "send", &[("text", Value::Text("no address".into()))]).is_err(),
        "no address"
    );
    idle(r, 0.3);
    assert!(sink.bursts().is_empty());
}

#[test]
fn halt_ends_the_transmission_with_the_end_mark() {
    let (mut rx, sink) = station(&[("pocsag_rate", Value::Choice(0))]);
    let long = "x".repeat(400);
    send(rx.as_mut(), 1_234_567, 3, "alpha", &long).unwrap();
    send(rx.as_mut(), 1_234_560, 3, "alpha", "waits").unwrap();
    assert_eq!(fields(rx.as_ref()).tx_waiting, 1);
    idle(rx.as_mut(), 0.5);
    let before = sink.bursts()[0].0.len();
    command(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), 0.5);
    let f = fields(rx.as_ref());
    assert!(!f.tx_sending && f.tx_waiting == 0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1, "the waiting page is dropped");
    assert!(b[0].1, "the end mark");
    // At most one bit of 512 bit/s and the 1 ms fall after the halt.
    let after = b[0].0.len() - before;
    assert!(after <= (RATE / 512.0 + 0.001 * RATE) as usize + 1, "{after} samples after the halt");
    assert!(b[0].0.last().unwrap().norm() < 0.01, "a clean fall");
}

#[test]
fn pages_sent_during_a_transmission_go_in_the_next() {
    let (mut rx, sink) = station(&[]);
    send(rx.as_mut(), 1_234_561, 0, "numeric", "1").unwrap();
    send(rx.as_mut(), 1_234_562, 3, "alpha", "two").unwrap();
    send(rx.as_mut(), 1_234_563, 3, "alpha", "three").unwrap();
    until_quiet(rx.as_mut(), 30.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    assert!(b.iter().all(|x| x.1));
    let mut all = b[0].0.clone();
    all.extend(std::iter::repeat_n(Complex32::default(), (0.3 * RATE) as usize));
    all.extend_from_slice(&b[1].0);
    let got: Vec<(u32, String)> = received(&all, None).into_iter().map(|p| (p.1, p.4)).collect();
    assert_eq!(
        got,
        [(1_234_561, "1".to_string()), (1_234_562, "two".into()), (1_234_563, "three".into())]
    );
}
