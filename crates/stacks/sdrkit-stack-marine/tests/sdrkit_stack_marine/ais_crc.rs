//! The AIS port demo decodes every message sent once, on its own channel, without a bad frame: many seeds, the two
//! listening frequencies of the test signal, and input blocks of several sizes.

use crate::common::*;
use sdrkit_stack_marine::ais::generator::{DemoOptions, PortDemo, Render};
use sdrkit_stack_marine::AIS;
use sdrkit_util_geo::nmea::{self, aivdm};

/// Seconds of each run.
const SECONDS: f64 = 40.0;

/// The messages of the `!AIVDM` sentences in `ev`: channel letter and bits.
fn messages(ev: &[sdrkit_core::Event]) -> Vec<(char, Vec<u8>)> {
    let mut a = aivdm::Assembler::new();
    let mut out = Vec::new();
    for e in ev.iter().filter(|e| e.kind() == "ais.nmea") {
        let line = field_json(e, "sentence").as_str().unwrap().to_string();
        let s = nmea::parse(&line).expect("a sentence");
        let v = aivdm::Vdm::from_sentence(&s).expect("an AIVDM sentence");
        if let Some(m) = a.push(&v).expect("the parts fit") {
            out.push((m.channel.expect("a channel"), m.bits.to_vec()));
        }
    }
    out
}

/// One run: the messages sent (those whose burst ends inside the run) and the bad frames.
fn one(seed: u64, tuned: f64, chunk: usize) -> (usize, u64) {
    let mut g = PortDemo::new(DemoOptions {
        seed,
        render: Render {
            rate: 250e3,
            duration_s: 0.0,
            tune_offset_hz: tuned - 162e6,
            ..Default::default()
        },
        ..Default::default()
    });
    let x = generate(&mut g, SECONDS);
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), 250e3, tuned, tuned);
    let mut got = messages(&run(rx.as_mut(), &x, chunk));
    let sent: Vec<_> = g.sent.iter().filter(|b| b.time < SECONDS - 0.5).collect();
    for b in &sent {
        // A frame holds whole octets: the bits sent with the zeros that fill the last one.
        let mut want = b.bits.clone();
        want.resize(b.bits.len().div_ceil(8) * 8, 0);
        let k = got
            .iter()
            .position(|(c, bits)| *c == b.channel && *bits == want);
        let Some(k) = k else {
            panic!(
                "seed {seed} at {tuned} Hz: type {} of {} at {:.3} s on {} not decoded",
                b.msg_type, b.mmsi, b.time, b.channel
            );
        };
        got.swap_remove(k);
    }
    // A burst sent near the end may still be decoded.
    for b in g.sent.iter().filter(|b| b.time >= SECONDS - 0.5) {
        let mut want = b.bits.clone();
        want.resize(b.bits.len().div_ceil(8) * 8, 0);
        got.retain(|(c, bits)| !(*c == b.channel && *bits == want));
    }
    assert!(
        got.is_empty(),
        "seed {seed} at {tuned} Hz: {:?} not sent",
        got
    );
    (sent.len(), count(rx.results(), "frames_bad"))
}

#[test]
fn ais_port_demo_has_no_bad_frame() {
    let runs: Vec<(u64, f64, usize)> = (1..=24u64)
        .map(|s| {
            let tuned = if s % 2 == 0 { 162.0e6 } else { 161.975e6 };
            (s, tuned, [8192, 1000, 65536, 4097][s as usize % 4])
        })
        .collect();
    let results: Vec<(usize, u64)> = std::thread::scope(|sc| {
        let h: Vec<_> = runs
            .chunks(6)
            .map(|part| {
                sc.spawn(move || {
                    part.iter()
                        .map(|&(s, t, c)| one(s, t, c))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        h.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let sent: usize = results.iter().map(|r| r.0).sum();
    let bad: u64 = results.iter().map(|r| r.1).sum();
    assert!(sent >= 2000, "{sent} messages");
    assert_eq!(bad, 0, "{bad} bad frames of {sent} messages");
}
