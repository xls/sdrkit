//! The cost of the work counters of a graph, those of its components and of its connections: a light stack on one
//! thread with the counters on and off.

use sdrkit_core::bench::measure;
use sdrkit_core::{
    Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Staged, Values,
};
use sdrkit_graph::Graph;

/// The share of one core that `stack` takes on one second of its test signal, with the counters `on`, its components,
/// the calls of its components for one second of signal, and its connections.
fn cost(stack: &StackDescriptor, on: bool) -> (f64, usize, u64, usize) {
    let rate = stack.tuning.rate_hz;
    let settings = Values::defaults(&(stack.settings)());
    let make = stack.make_generator.unwrap();
    let mut gen = make(&GenConfig::new(rate, settings.clone())).unwrap();
    let mut x = vec![Complex32::default(); rate as usize];
    gen.generate(&mut x);
    let mut rx = (stack.make_receiver)(&StackConfig::new(settings)).unwrap();
    let hz = stack.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    let g = rx.as_any_mut().unwrap().downcast_mut::<Graph>().unwrap();
    g.set_metering(on);
    let mut rows = Vec::new();
    let mut out = Output::default();
    let r = measure(1.5, x.len(), 1.0, || {
        for b in x.chunks(16_384) {
            out.clear();
            rx.feed(b, &mut out);
        }
    });
    // The calls of one more run over the second of signal.
    let g = rx.as_any_mut().unwrap().downcast_mut::<Graph>().unwrap();
    g.component_loads(&mut rows);
    let before: u64 = rows.iter().map(|r| r.calls).sum();
    for b in x.chunks(16_384) {
        out.clear();
        rx.feed(b, &mut out);
    }
    let g = rx.as_any_mut().unwrap().downcast_mut::<Graph>().unwrap();
    g.component_loads(&mut rows);
    let calls = rows.iter().map(|r| r.calls).sum::<u64>() - before;
    let mut connections = Vec::new();
    g.connection_loads(&mut connections);
    let components = rows.iter().filter(|r| r.section.is_empty()).count();
    (r.cpu_fraction_of_realtime, components, calls, connections.len())
}

/// The nanoseconds that the counters add to one call of a component, and to one connection in one pass: a graph of
/// one empty component and a graph of two light components joined by one connection, fed one sample at a time, with
/// the counters on and off.
fn counters_ns() -> (f64, f64) {
    use sdrkit_graph::{components::{DcRemoval, NullSink}, GraphBuilder};
    const CALLS: usize = 400_000;
    let one = || GraphBuilder::new().add("sink", NullSink::default()).input("sink").build().unwrap();
    let two = || {
        GraphBuilder::new()
            .add("dc", DcRemoval::default())
            .add("sink", NullSink::default())
            .input("dc")
            .connect("dc", "sink")
            .build()
            .unwrap()
    };
    let run = |mut g: sdrkit_graph::Graph, on: bool| {
        sdrkit_core::Receiver::configure(&mut g, &InputInfo::new(48e3, 0.0, 0.0).recording()).unwrap();
        g.set_metering(on);
        let (x, mut out) = ([Complex32::default()], Output::default());
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for _ in 0..CALLS {
                g.process(&x, &mut out);
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        best * 1e9 / CALLS as f64
    };
    let per_call = (run(one(), true) - run(one(), false)).max(0.0);
    let per_pass = (run(two(), true) - run(two(), false)).max(0.0);
    // A pass of the second graph has two calls and one connection.
    let per_connection = (per_pass - 2.0 * per_call).max(0.0);
    eprintln!("the counters add {per_call:.0} ns to a call and {per_connection:.0} ns to a connection in a pass");
    (per_call, per_connection)
}

#[test]
#[ignore = "a measurement: run it alone in a release build"]
fn the_counters_cost_a_light_stack_less_than_one_hundredth() {
    let (per_call, per_connection) = counters_ns();
    for stack in [&sdrkit_stack_fm::DESCRIPTOR, &sdrkit_stack_lora::MESHTASTIC] {
        let (off, _, _, _) = cost(stack, false);
        let (on, components, calls, connections) = cost(stack, true);
        // The stack runs on its test signal of one second several times; the calls of one run are what one second
        // of signal makes. Each component is called once in a pass, and each connection counts once in a pass.
        let passes = calls as f64 / components.max(1) as f64;
        let added = (calls as f64 * per_call + passes * connections as f64 * per_connection) / 1e9;
        eprintln!(
            "{}: {:.4} of a core with the counters off, {:.4} with them on; {components} components, {connections} \
             connections, {calls} calls for a second of signal, which add an estimated {:.5} of a core ({:.2} %)",
            stack.id,
            off,
            on,
            added,
            100.0 * added / off
        );
        assert!(added < 0.01 * off, "{}: the counters add {added} to {off}", stack.id);
    }
}
