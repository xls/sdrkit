//! The receive path of a stack allocates nothing once warm: the stack graph runs on its own test signal, then on silence, and
//! the allocations of the silence are counted.

use crate::{allocations, common};
use sdrkit_core::{Complex32, Generator, Output, Receiver};
use sdrkit_stack_avdata::acars::{self, AcarsGenerator, GenParams};
use sdrkit_stack_avdata::vdl2;

/// Feeds `x` in pieces of 65 536 samples.
fn feed(rx: &mut dyn Receiver, x: &[Complex32], out: &mut Output) {
    for c in x.chunks(65_536) {
        out.clear();
        rx.feed(c, out);
    }
}

/// Feeds `x`, then silence until every decoder stopped, and returns the allocations of 2 s more of silence.
fn silence_allocations(mut rx: Box<dyn Receiver>, x: &[Complex32], rate: f64) -> u64 {
    let mut out = Output::default();
    feed(rx.as_mut(), x, &mut out);
    let zeros = vec![Complex32::default(); (2.0 * rate) as usize];
    feed(rx.as_mut(), &zeros, &mut out);
    feed(rx.as_mut(), &zeros, &mut out);
    allocations(|| feed(rx.as_mut(), &zeros, &mut out))
}

#[test]
fn acars_allocates_nothing_once_warm() {
    let rate = 2e6;
    let mut g = AcarsGenerator::new(GenParams {
        rate_hz: rate,
        centre_hz: 131.5e6,
        channels: vec![131.525e6],
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); (12.0 * rate) as usize];
    g.generate(&mut x);
    let rx = common::receiver(&acars::DESCRIPTOR, rate, 131.5e6, 131.5e6, &[]);
    assert_eq!(
        silence_allocations(rx, &x, rate),
        0,
        "allocations in 2 s of silence"
    );
}

#[test]
fn vdl2_allocates_nothing_once_warm() {
    let rate = 1e6;
    let mut g = vdl2::Vdl2Generator::new(vdl2::GenParams {
        rate_hz: rate,
        centre_hz: 136.85e6,
        channels: vec![136.975e6],
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); (6.0 * rate) as usize];
    g.generate(&mut x);
    let rx = common::receiver(&vdl2::DESCRIPTOR, rate, 136.85e6, 136.85e6, &[]);
    assert_eq!(
        silence_allocations(rx, &x, rate),
        0,
        "allocations in 2 s of silence"
    );
}
