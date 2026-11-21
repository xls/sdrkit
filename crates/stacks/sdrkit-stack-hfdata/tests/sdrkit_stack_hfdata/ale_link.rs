//! The ALE link: the scanning call caught by a scanning receiver at every point of its scan, two stacks connected back
//! to back that complete the three-way handshake and exchange an AMD text through their own senders and receivers, the
//! link quality measured on clean and noisy calls and carried in the handshake, and the refusals with the transmit
//! switch off.

use crate::common::{num, rows, text, Owner};
use crate::transmit::{call, idle, station, text_value, Sink, DIAL};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, StackConfig, Value, Values};
use sdrkit_stack_hfdata::ale::{self, gen};
use sdrkit_stack_hfdata::typed::{AleLinkState, AleTxFields, ALE_EVENTS, ALE_LQA, ALE_MESSAGES};
use std::collections::VecDeque;
use std::sync::Arc;

const RATE: f64 = 24_000.0;
const PIECE: usize = 2400;

/// An ALE stack with transmit on, its own address `own` and the settings `more`.
fn ale_with(own: &str, more: &[(&str, Value)]) -> (Box<dyn Receiver>, Arc<Sink>) {
    // The answer is off at the start; these stations answer unless a test turns it off again.
    let mut s = vec![("self_address", text_value(own)), ("auto_answer", Value::Bool(true))];
    s.extend(more.iter().cloned());
    station(&ale::ALE, RATE, &s, None)
}

/// A fresh ALE receiver at [`RATE`] on [`DIAL`].
fn listener() -> Box<dyn Receiver> {
    let schema = (ale::ALE.settings)();
    let mut rx = (ale::ALE.make_receiver)(&StackConfig::new(Values::defaults(&schema))).unwrap();
    rx.configure(&InputInfo::new(RATE, DIAL, DIAL).recording())
        .unwrap();
    rx
}

/// Feeds `x` with 1 s of silence after it, in pieces of 0.1 s, and returns the events.
fn hear(rx: &mut dyn Receiver, x: &[Complex32]) -> Output {
    let mut all = Output::default();
    let mut out = Output::default();
    let tail = vec![Complex32::default(); RATE as usize];
    for c in x.chunks(PIECE).chain(tail.chunks(PIECE)) {
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        out.clear();
    }
    all
}

/// The calls in the event log of `rx`: type, from, to.
fn calls(rx: &dyn Receiver) -> Vec<(String, String, String)> {
    rows(rx.results(), &ALE_EVENTS)
        .iter()
        .map(|e| (text(e, "type"), text(e, "from"), text(e, "to")))
        .collect()
}

/// The link fields of `rx`.
fn link(rx: &dyn Receiver) -> (AleLinkState, Option<String>, String) {
    let f = AleTxFields::read(Owner::results(rx).fields()).unwrap();
    (
        f.tx_link,
        f.tx_link_station.map(str::to_owned),
        f.tx_link_reason.to_owned(),
    )
}

/// The `ale.link` events of `out`: state and reason.
fn steps(out: &Output) -> Vec<(String, String)> {
    out.events
        .iter()
        .filter(|e| e.kind() == "ale.link")
        .map(|e| (e.text("state"), e.text("reason")))
        .collect()
}

/// The LQA row of `address` in `rx`: BER, SINAD, multipath and the reported ones (-1 for none).
fn lqa_of(rx: &dyn Receiver, address: &str) -> Option<[f64; 6]> {
    rows(rx.results(), &ALE_LQA)
        .iter()
        .find(|r| text(r, "address") == address)
        .map(|r| {
            let opt = |k: &str| {
                let v = crate::common::cell(r, k);
                if matches!(v, Value::None) {
                    -1.0
                } else {
                    num(r, k)
                }
            };
            [
                num(r, "lqa_ber"),
                num(r, "sinad_db"),
                num(r, "multipath"),
                opt("reported_ber"),
                opt("reported_sinad_db"),
                opt("reported_multipath"),
            ]
        })
}

/// Two stacks back to back: what one sends is what the other receives, one pass of 0.1 s later at most.
struct Pair {
    a: Box<dyn Receiver>,
    sa: Arc<Sink>,
    b: Box<dyn Receiver>,
    sb: Arc<Sink>,
    seen: [usize; 2],
    to_a: VecDeque<Complex32>,
    to_b: VecDeque<Complex32>,
    out_a: Output,
    out_b: Output,
    /// The noise added to what A receives (variance a sample), and its source.
    noise_to_a: f64,
    rng: sdrkit_algo_sim::rng::Rng,
}

impl Pair {
    fn new(a: (Box<dyn Receiver>, Arc<Sink>), b: (Box<dyn Receiver>, Arc<Sink>)) -> Pair {
        Pair {
            a: a.0,
            sa: a.1,
            b: b.0,
            sb: b.1,
            seen: [0, 0],
            to_a: VecDeque::new(),
            to_b: VecDeque::new(),
            out_a: Output::default(),
            out_b: Output::default(),
            noise_to_a: 0.0,
            rng: sdrkit_algo_sim::rng::Rng::new(3),
        }
    }

    /// Runs both stacks for `seconds`.
    fn run(&mut self, seconds: f64) {
        let mut out = Output::default();
        for _ in 0..(seconds * RATE / PIECE as f64).round() as usize {
            let x: Vec<Complex32> = (0..PIECE)
                .map(|_| {
                    let z = self.to_a.pop_front().unwrap_or_default();
                    if self.noise_to_a > 0.0 {
                        z + self.rng.complex_normal(self.noise_to_a)
                    } else {
                        z
                    }
                })
                .collect();
            self.a.feed(&x, &mut out);
            self.out_a.events.append(&mut out.events);
            out.clear();
            self.to_b.extend(self.sa.samples_since(&mut self.seen[0]));
            let x: Vec<Complex32> = (0..PIECE)
                .map(|_| self.to_b.pop_front().unwrap_or_default())
                .collect();
            self.b.feed(&x, &mut out);
            self.out_b.events.append(&mut out.events);
            out.clear();
            self.to_a.extend(self.sb.samples_since(&mut self.seen[1]));
        }
    }
}

#[test]
fn ale_scanning_call_is_caught_at_every_dwell_of_the_scan() {
    // The called station scans 12 channels at 2 a second: it is back on this channel every 6 s and listens 0.5 s. It
    // stops on the channel when its dwell hears ALE tones, and then reads on.
    let (channels, dwell) = (12usize, 0.5);
    let scanned = |n: i64| {
        let (mut rx, sink) = ale_with(
            "XYZ",
            &[
                ("scan_channels", Value::I64(n)),
                ("scan_dwell_s", Value::F64(dwell)),
            ],
        );
        call(rx.as_mut(), "call", &[("to", text_value("ABCDEF"))]).unwrap();
        idle(rx.as_mut(), RATE, 13.0);
        sink.bursts().remove(0).samples
    };
    let x = scanned(channels as i64);
    // The address is two words: sent 8 times for the 6 s scan (6.272 s), 6 times more, then TIS XYZ: 29 words.
    assert_eq!(x.len(), 29 * 49 * (0.008 * RATE) as usize);
    let plain = scanned(0);
    assert_eq!(plain.len(), 13 * 49 * (0.008 * RATE) as usize);
    let want = vec![(
        "Individual".to_owned(),
        "XYZ".to_owned(),
        "ABCDEF".to_owned(),
    )];
    let caught = |x: &[Complex32], at_s: f64| -> bool {
        let from = (at_s * RATE) as usize;
        let to = ((at_s + dwell) * RATE) as usize;
        let tones = x.get(from..to.min(x.len())).is_some_and(|d| {
            !d.is_empty() && d.iter().map(|z| z.norm_sqr()).sum::<f32>() / d.len() as f32 > 0.1
        });
        if !tones {
            return false;
        }
        let mut rx = listener();
        hear(rx.as_mut(), &x[from..]);
        calls(rx.as_ref()) == want
    };
    let offsets: Vec<f64> = (0..channels)
        .map(|k| k as f64 * dwell)
        .chain([channels as f64 * dwell - 0.01])
        .collect();
    for &o in &offsets {
        assert!(caught(&x, o), "the scanning call is missed at {o} s");
    }
    // Without the scanning part the station misses the call when it comes back late in its scan.
    let missed: Vec<f64> = offsets
        .iter()
        .copied()
        .filter(|&o| !caught(&plain, o))
        .collect();
    assert!(missed.len() >= 4, "{missed:?}");
}

#[test]
fn ale_two_stacks_complete_the_handshake_and_exchange_an_amd_text() {
    let lqa = [("lqa", Value::Bool(true))];
    let mut p = Pair::new(ale_with("TSTA", &lqa), ale_with("TSTB", &[]));
    // B to A is noisy: 12 dB under the signal (power 0.25) in 3 kHz; A to B is clean.
    p.noise_to_a = 0.25 / 10f64.powf(1.2) * RATE / 3000.0;
    call(p.a.as_mut(), "call", &[("to", text_value("tstb"))]).unwrap();
    assert_eq!(link(p.a.as_ref()).0, AleLinkState::Calling);
    p.run(14.0);
    // Call, response, acknowledgement: two transmissions of A, one of B.
    assert_eq!((p.sa.bursts().len(), p.sb.bursts().len()), (2, 1));
    assert!(p.sa.bursts().iter().chain(&p.sb.bursts()).all(|b| b.ended));
    let la = link(p.a.as_ref());
    let lb = link(p.b.as_ref());
    assert_eq!(
        (la.0, la.1.as_deref()),
        (AleLinkState::Linked, Some("TSTB"))
    );
    assert_eq!(
        (lb.0, lb.1.as_deref()),
        (AleLinkState::Linked, Some("TSTA"))
    );
    let s = |v: &[(&str, &str)]| -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    };
    assert_eq!(
        steps(&p.out_a),
        s(&[
            ("Calling", "call sent"),
            ("Calling", "response heard, acknowledgement sent"),
            ("Linked", "acknowledgement sent"),
        ])
    );
    assert_eq!(
        steps(&p.out_b),
        s(&[
            ("Responding", "call heard, response sent"),
            ("Linked", "acknowledgement heard"),
        ])
    );
    // Each heard the other: A the response, B the call and the acknowledgement.
    assert_eq!(
        calls(p.a.as_ref()),
        vec![("Individual".into(), "TSTB".into(), "TSTA".into())]
    );
    assert_eq!(calls(p.b.as_ref()).len(), 2);
    // LQA: the call asked for it; the response carried B's figures of the clean call (SINAD at its top), the
    // acknowledgement A's figures of the noisy response.
    let qa = lqa_of(p.a.as_ref(), "TSTB").unwrap();
    let qb = lqa_of(p.b.as_ref(), "TSTA").unwrap();
    assert_eq!(qa[3..], [0.0, 30.0, 0.0], "{qa:?}");
    assert!((9.0..=16.0).contains(&qa[1]), "{qa:?}");
    assert_eq!(qb[3..], qa[..3]);
    assert_eq!(qb[1], 30.0);
    // An AMD text to the linked station: one transmission, no answer, the text exact in B's messages.
    call(
        p.a.as_mut(),
        "amd",
        &[
            ("to", text_value("")),
            ("text", text_value("qsy 7102 usb pse")),
        ],
    )
    .unwrap();
    p.run(8.0);
    assert_eq!((p.sa.bursts().len(), p.sb.bursts().len()), (3, 1));
    let m = rows(p.b.results(), &ALE_MESSAGES);
    assert_eq!(m.len(), 1);
    assert_eq!(
        (text(&m[0], "from"), text(&m[0], "to"), text(&m[0], "text")),
        ("TSTA".into(), "TSTB".into(), "QSY 7102 USB PSE".into())
    );
    assert_eq!(link(p.b.as_ref()).0, AleLinkState::Linked);
    // stop ends the link: an AMD text without an address then has no station.
    call(p.a.as_mut(), "stop", &[]).unwrap();
    assert_eq!(link(p.a.as_ref()).0, AleLinkState::Idle);
    assert!(call(
        p.a.as_mut(),
        "amd",
        &[("to", text_value("")), ("text", text_value("X"))]
    )
    .is_err());
    // An AMD text to a station not linked is a call with the text: B answers it, A acknowledges.
    let mut p = Pair::new(ale_with("TSTC", &[]), ale_with("TSTD", &[]));
    call(
        p.a.as_mut(),
        "amd",
        &[("to", text_value("TSTD")), ("text", text_value("HELLO_ 1"))],
    )
    .unwrap();
    p.run(14.0);
    assert_eq!((p.sa.bursts().len(), p.sb.bursts().len()), (2, 1));
    assert_eq!(link(p.a.as_ref()).0, AleLinkState::Linked);
    let m = rows(p.b.results(), &ALE_MESSAGES);
    assert_eq!(text(&m[0], "text"), "HELLO_ 1");
    // Silence after the handshake: nothing more is sent.
    p.run(5.0);
    assert_eq!((p.sa.bursts().len(), p.sb.bursts().len()), (2, 1));
}

#[test]
fn ale_lqa_of_a_clean_and_a_noisy_call_in_the_table() {
    let call_of = |from: &str, ber: f64, seed: u64| {
        let mut g = gen::AleGenerator::new(gen::AleGenOptions {
            rate: RATE,
            ber,
            seed,
            repeat: false,
            calls: gen::individual(from, "TSTB", false, None, false, 6),
            tail_s: 0.0,
        });
        let mut x = vec![Complex32::default(); (g.length_s() * RATE) as usize];
        let n = g.generate(&mut x);
        x.truncate(n);
        x
    };
    let clean = call_of("CLN", 0.0, 1);
    // 2 % of the channel bits turned over and noise 10 dB under the signal in 3 kHz.
    let mut noisy = call_of("NSY", 0.02, 2);
    let noise = 1.0 / 10.0 * RATE / 3000.0;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    for z in &mut noisy {
        *z += rng.complex_normal(noise);
    }
    let mut rx = listener();
    hear(rx.as_mut(), &clean);
    hear(rx.as_mut(), &noisy);
    let c = lqa_of(rx.as_ref(), "CLN").unwrap();
    let n = lqa_of(rx.as_ref(), "NSY").unwrap();
    assert_eq!(c, [0.0, 30.0, 0.0, -1.0, -1.0, -1.0]);
    // About 147 x 0.02 = 2.9 vote errors a word; SINAD 10 log10(1 + 10) = 10.4 dB.
    assert!((2.0..=4.0).contains(&n[0]), "{n:?}");
    assert!((8.0..=13.0).contains(&n[1]), "{n:?}");
    let dial = rows(rx.results(), &ALE_LQA)
        .iter()
        .map(|r| num(r, "dial"))
        .collect::<Vec<_>>();
    assert_eq!(dial, vec![DIAL, DIAL]);
}

#[test]
fn ale_does_not_answer_with_the_transmitter_off_or_the_answer_off() {
    let mut p = Pair::new(
        ale_with("TSTA", &[("answer_wait_s", Value::F64(1.0))]),
        ale_with("TSTB", &[]),
    );
    p.b.set_transmit(false);
    call(p.a.as_mut(), "call", &[("to", text_value("TSTB"))]).unwrap();
    p.run(10.0);
    assert_eq!((p.sa.bursts().len(), p.sb.bursts().len()), (1, 0));
    let lb = link(p.b.as_ref());
    assert_eq!(lb.0, AleLinkState::NotAnswered);
    assert!(lb.2.contains("the transmitter is off"), "{}", lb.2);
    let la = link(p.a.as_ref());
    assert_eq!(
        (la.0, la.2.as_str()),
        (AleLinkState::TimedOut, "no response")
    );
    // The call was heard all the same.
    assert_eq!(calls(p.b.as_ref()).len(), 1);
    // With the answer off, the call is heard and not answered.
    let mut p = Pair::new(
        ale_with("TSTA", &[]),
        ale_with("TSTB", &[("auto_answer", Value::Bool(false))]),
    );
    call(p.a.as_mut(), "call", &[("to", text_value("TSTB"))]).unwrap();
    p.run(8.0);
    assert_eq!(p.sb.bursts().len(), 0);
    let lb = link(p.b.as_ref());
    assert_eq!(
        (lb.0, lb.2.as_str()),
        (
            AleLinkState::NotAnswered,
            "call heard; automatic answer is off"
        )
    );
}

#[test]
fn ale_amd_refuses_bad_texts_no_station_and_transmit_off() {
    let (mut rx, sink) = ale_with("TSTA", &[]);
    let amd = |rx: &mut dyn Receiver, to: &str, t: &str| {
        call(
            rx,
            "amd",
            &[("to", text_value(to)), ("text", text_value(t))],
        )
    };
    let e = amd(rx.as_mut(), "", "HELLO").unwrap_err();
    assert!(e.to_string().contains("no station is linked"), "{e}");
    assert!(amd(rx.as_mut(), "TSTB", "").is_err());
    assert!(amd(rx.as_mut(), "TSTB", "PRICE {5}").is_err());
    assert!(amd(rx.as_mut(), "TSTB", &"A".repeat(91)).is_err());
    assert!(amd(rx.as_mut(), "TS-B", "HELLO").is_err());
    rx.set_transmit(false);
    assert!(amd(rx.as_mut(), "TSTB", "HELLO").is_err());
    rx.set_transmit(true);
    amd(rx.as_mut(), "TSTB", &"A".repeat(90)).unwrap();
    // The called address (two words) 6 times, 30 words of text, the own address (two words): 44 words.
    let f = AleTxFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert_eq!(f.tx_words, Some(44));
    idle(rx.as_mut(), RATE, 0.2);
    assert_eq!(sink.bursts().len(), 1);
}
