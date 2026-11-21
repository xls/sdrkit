//! Multitone: the generator's contents measured by the detector.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::Generator;
use sdrkit_stack_trunk::multitone::{Content, MultitoneSignal};
use sdrkit_stack_trunk::typed::{MULTITONE_BURSTS, MULTITONE_TONES, MULTITONE_TRANSMISSIONS};
use sdrkit_stack_trunk::{multitone, MULTITONE};

fn run(content: Content) -> (Box<dyn sdrkit_core::Receiver>, Fed) {
    let sig = MultitoneSignal {
        rate: 250e3,
        content,
        ..Default::default()
    };
    let mut g = sig.generator().once();
    let mut rx = make(&MULTITONE, &[]);
    configure(rx.as_mut(), 250e3, 446.475e6, 0.0);
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    let mut fed = Fed::default();
    loop {
        let n = g.generate(&mut buf);
        add_noise(&mut buf[..n], 0.005, 1);
        fed.events
            .extend(feed(rx.as_mut(), &buf[..n], 65536).events);
        if n < buf.len() {
            break;
        }
    }
    let tail = vec![Complex32::new(0.0, 0.0); 250_000];
    fed.events.extend(feed(rx.as_mut(), &tail, 65536).events);
    (rx, fed)
}

#[test]
fn tones_and_data_are_measured() {
    let (rx, fed) = run(Content::Cycle);
    let r = results(rx.as_ref());
    let txs = rows(r, &MULTITONE_TRANSMISSIONS);
    assert!(txs.len() >= 2, "{txs:?}");
    let tones = rows(r, &MULTITONE_TONES);
    assert!(
        tones
            .iter()
            .any(|t| (t["hz"].parse::<f64>().unwrap() - 1000.0).abs() < 6.0),
        "{tones:?}"
    );
    assert!(!rows(r, &MULTITONE_BURSTS).is_empty());
    check_events(&kinds(&multitone::declaration()), &fed.events, true);
}

#[test]
fn a_pocsag_batch_is_recognised() {
    let (rx, _) = run(Content::Pocsag);
    let b = rows(results(rx.as_ref()), &MULTITONE_BURSTS);
    assert!(!b.is_empty());
    assert!(
        b.iter()
            .any(|b| b["bch_valid"].parse::<u32>().unwrap() >= 8),
        "{b:?}"
    );
}

#[test]
fn noise_alone_has_no_transmission() {
    let (rx, _) = run(Content::Noise);
    assert!(rows(results(rx.as_ref()), &MULTITONE_TRANSMISSIONS).is_empty());
}
