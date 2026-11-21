//! A run of a long test signal in pieces: the generator of the stack through the standard impairments, made a quarter of a
//! second at a time and fed in pieces of 10 ms, so that a long run at a high rate does not hold the whole stream.

use sdrkit_components::sim::impaired;
use sdrkit_core::{
    Complex32, Event, GenConfig, InputInfo, Output, Owner, Results, StackDescriptor,
};
use sdrkit_dev_test::Harness;

/// What a streamed run returns.
pub struct Streamed {
    /// Every event, in order.
    pub events: Vec<Event>,
    /// The results at the end.
    pub results: Results,
}

/// Runs `seconds` of the test signal of `desc` with the settings and the seed of `h` at `rate`, centred on `freq`. `tap`
/// sees each impaired piece with its start time before the receiver does.
pub fn stream(
    desc: &'static StackDescriptor,
    h: &Harness,
    rate: f64,
    freq: f64,
    seconds: f64,
    seed: u64,
    mut tap: impl FnMut(f64, &mut [Complex32]),
) -> Streamed {
    let values = h.values().expect("the settings");
    let make = desc.make_generator.expect("a generator");
    let gen = make(&GenConfig::new(rate, values.clone()).with_seed(seed)).expect("the generator");
    let mut g = impaired(gen, &values).expect("the impairments");
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(rate, freq, freq).recording())
        .expect("the input");
    let total = (seconds * rate).round() as usize;
    let block = (rate / 4.0) as usize;
    let piece = ((rate / 100.0) as usize).max(1);
    let mut x = vec![Complex32::default(); block];
    let mut out = Output::default();
    let mut events = Vec::new();
    let mut done = 0usize;
    while done < total {
        let n = block.min(total - done);
        let got = g.generate(&mut x[..n]);
        tap(done as f64 / rate, &mut x[..got]);
        for p in x[..got].chunks(piece) {
            out.clear();
            rx.feed(p, &mut out);
            events.append(&mut out.events);
        }
        done += n;
    }
    out.clear();
    rx.finish(&mut out);
    events.append(&mut out.events);
    Streamed {
        events,
        results: Owner::results(rx.as_ref()).clone(),
    }
}
