//! Generates `seconds` of a stack's test signal and, unless `gen-only` is given, runs the receiver over it once. Run it under
//! `/usr/bin/time` twice (with and without `gen-only`) to get the receiver's CPU time on a busy machine.
use sdrkit_core::{Complex32, InputInfo, Output, StackConfig};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("atsc");
    let seconds: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let gen_only = args.iter().any(|a| a == "gen-only");
    let d = sdrkit_stack_tvterr::descriptors()
        .into_iter()
        .find(|d| d.id == mode)
        .expect("a stack of the crate");
    let rate = if mode == "dtmb" { 10e6 } else { 8e6 };
    let settings = sdrkit_core::Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.expect("a generator"))(&sdrkit_core::GenConfig::new(rate, settings.clone()).with_seed(1))
    .unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (seconds * rate) as usize];
    g.generate(&mut x);
    if gen_only {
        return;
    }
    let mut rx = (d.make_receiver)(&StackConfig::new(settings)).unwrap();
    rx.configure(&InputInfo::new(rate, 500e6, 500e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        rx.feed(c, &mut out);
        out.clear();
    }
    eprintln!(
        "{}",
        sdrkit_core::Owner::results(rx.as_ref()).status().summary
    );
}
