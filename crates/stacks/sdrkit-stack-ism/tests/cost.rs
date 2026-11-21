//! The recorded cost of each stage (`cost.txt`) against the cost measured on the test signal, for the stacks of the
//! family that mark stages. `SDRKIT_WRITE_COST=1` in an optimised build writes the file. The events of Wi-SUN for
//! every placement of the cuts and with 0, 1 and 4 helpers of the worker pool.

use std::path::PathBuf;

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = &sdrkit_stack_ism::wisun::WISUN;
    if let Err(e) = sdrkit_graph::cost::check(stack, "-", &path, 2.0, |_| {}) {
        panic!("{e}");
    }
}

/// Runs `seconds` of the test signal through the Wi-SUN stack cut at `mask`, one thread per cut, with `helpers`
/// helper threads of the worker pool, and returns the events as text.
fn run(seconds: f64, mask: u32, helpers: usize) -> Vec<String> {
    use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, Values};
    let stack = &sdrkit_stack_ism::wisun::WISUN;
    let rate = stack.tuning.rate_hz;
    let settings = Values::defaults(&(stack.settings)());
    let mut gen = (stack.make_generator.expect("a test signal"))(&GenConfig::new(rate, settings.clone()))
        .expect("a generator");
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    let mut rx = sdrkit_stack_ism::wisun::receiver(&StackConfig::new(settings)).expect("builds");
    let pool = sdrkit_graph::Pool::new(helpers.max(1));
    pool.set_workers(helpers);
    rx.set_context(sdrkit_graph::RunContext {
        pool,
        ..Default::default()
    });
    let hz = stack.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, hz, hz).recording())
        .expect("the rate");
    let threads: Vec<_> = rx
        .staged()
        .expect("stages")
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), mask.count_ones() as usize, "a thread per cut");
    let (mut out, mut events) = (Output::default(), Vec::new());
    for b in x.chunks(16_384) {
        rx.feed(b, &mut out);
        events.extend(out.events.drain(..).map(|e| format!("{e:?}")));
        out.clear();
    }
    let st = rx.staged().expect("stages");
    st.drain(&mut out);
    events.extend(out.events.drain(..).map(|e| format!("{e:?}")));
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    events
}

#[test]
fn every_placement_of_the_cuts_and_the_worker_pool_give_the_events_of_one_thread() {
    let alone = run(1.0, 0, 0);
    assert!(!alone.is_empty(), "no event");
    for mask in 1..4 {
        assert!(run(1.0, mask, 0) == alone, "cut at {mask:#04b}");
    }
    for helpers in [1, 4] {
        for mask in [0, 3] {
            assert!(run(1.0, mask, helpers) == alone, "{helpers} helpers, cut at {mask:#04b}");
        }
    }
}
