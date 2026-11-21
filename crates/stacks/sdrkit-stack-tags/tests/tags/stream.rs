//! A run of a long test signal in pieces: the generator of the stack through the standard impairments, made a quarter of a
//! second at a time and fed in pieces of 10 ms, so that a long run does not hold the whole stream.

use sdrkit_components::sim::impaired;
use sdrkit_core::{
    Complex32, Event, GenConfig, Generator, InputInfo, Output, Owner, Results, StackDescriptor,
    Value,
};
use sdrkit_dev_test::Harness;

/// What a streamed run returns.
pub struct Streamed {
    /// Every event, in order.
    pub events: Vec<Event>,
    /// The results at the end.
    pub results: Results,
}

/// Runs `seconds` of the test signal of `desc` with the settings of `h` and `seed` at `rate`. The receiver is centred on
/// `centre` and tuned to `tuned` (the dial: the signal is `tuned - centre` from the centre when `gen_offset_hz` says so).
pub fn stream(
    desc: &'static StackDescriptor,
    h: &Harness,
    rate: f64,
    centre: f64,
    tuned: f64,
    seconds: f64,
    seed: u64,
) -> Streamed {
    let values = h.values().expect("the settings");
    let make = desc.make_generator.expect("a generator");
    let gen = make(&GenConfig::new(rate, values.clone()).with_seed(seed)).expect("the generator");
    run(gen, h, rate, centre, tuned, seconds, None)
}

/// As [`stream`], with setting `key` set to `value` at `at_s` of the run.
#[allow(clippy::too_many_arguments)]
pub fn stream_mid(
    desc: &'static StackDescriptor,
    h: &Harness,
    rate: f64,
    freq: f64,
    seconds: f64,
    seed: u64,
    at_s: f64,
    key: &str,
    value: Value,
) -> Streamed {
    let values = h.values().expect("the settings");
    let make = desc.make_generator.expect("a generator");
    let gen = make(&GenConfig::new(rate, values.clone()).with_seed(seed)).expect("the generator");
    run(gen, h, rate, freq, freq, seconds, Some((at_s, key, value)))
}

/// As [`stream`], with the clean signal of `gen`.
pub fn run(
    gen: Box<dyn Generator>,
    h: &Harness,
    rate: f64,
    centre: f64,
    tuned: f64,
    seconds: f64,
    mut mid: Option<(f64, &str, Value)>,
) -> Streamed {
    let values = h.values().expect("the settings");
    let mut g = impaired(gen, &values).expect("the impairments");
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
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
        if let Some((at, key, v)) = mid.take_if(|m| done as f64 / rate >= m.0) {
            let _ = at;
            let id = Owner::schema(rx.as_ref()).id(key).expect("the setting");
            Owner::apply(rx.as_mut(), id, &v).expect("the value");
        }
        let got = g.generate(&mut x[..n]);
        for p in x[..got].chunks(piece) {
            out.clear();
            rx.feed(p, &mut out);
            events.append(&mut out.events);
        }
        done += n;
        if got < n {
            break;
        }
    }
    out.clear();
    rx.finish(&mut out);
    events.append(&mut out.events);
    Streamed {
        events,
        results: Owner::results(rx.as_ref()).clone(),
    }
}
