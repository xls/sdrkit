//! The recorded cost of each stage of the DAB stack (`cost.txt`) against the cost measured on the test signal.
//! `SDRKIT_WRITE_COST=1` in a release build writes the file.

use std::path::PathBuf;

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = &sdrkit_stack_dab::descriptors()[0];
    // The cost is that of a session that plays the DAB+ service of the test signal.
    let play = |rx: &mut dyn sdrkit_core::Receiver| {
        let id = rx.commands().id("service.select").unwrap();
        let mut args = rx.commands().def(id).unwrap().new_args();
        args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, "C103");
        sdrkit_core::owner::invoke(rx, id, &args).unwrap();
    };
    if let Err(e) = sdrkit_graph::cost::check(stack, "-", &path, 2.0, play) {
        panic!("{e}");
    }
}

/// The seconds a run took, the cost of each stage, and the sound and the bytes of the media packets.
type Run = (f64, Vec<(String, f64)>, (Vec<u32>, Vec<u8>));

/// Runs 20 s of the test signal through the stack cut at `mask`, one thread per cut, and returns the seconds it took,
/// the cost of each stage, and the sound and the bytes of the media packets.
fn run(mask: u32) -> Run {
    use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, Values};
    let stack = sdrkit_stack_dab::descriptors()[0];
    let rate = stack.tuning.rate_hz;
    let settings = Values::defaults(&(stack.settings)());
    let mut gen = (stack.make_generator.unwrap())(&GenConfig::new(rate, settings.clone())).unwrap();
    let mut x = vec![Complex32::default(); (20.0 * rate) as usize];
    gen.generate(&mut x);
    let mut rx = (stack.make_receiver)(&StackConfig::new(settings)).unwrap();
    let id = rx.commands().id("service.select").unwrap();
    let mut args = rx.commands().def(id).unwrap().new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, "C103");
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    let hz = stack.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    let threads: Vec<_> = rx
        .staged()
        .unwrap()
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    let (mut out, mut all, mut loads) = (Output::default(), Output::default(), Vec::new());
    let t = std::time::Instant::now();
    for b in x.chunks(16_384) {
        rx.feed(b, &mut out);
        all.append(&mut out);
    }
    let st = rx.staged().unwrap();
    st.drain(&mut out);
    all.append(&mut out);
    let took = t.elapsed().as_secs_f64();
    st.stage_loads(&mut loads);
    drop(st.cut(0));
    for t in threads {
        t.join().unwrap();
    }
    let costs = loads
        .iter()
        .map(|l| (l.name.clone(), l.busy_ns as f64 / 1e9 / l.signal_s.max(1e-9)))
        .collect();
    let sound = all.audio().iter().map(|x| x.to_bits()).collect();
    let media = all.media.iter().flat_map(|m| m.packet.bytes().to_vec()).collect();
    (took, costs, (sound, media))
}

#[test]
#[ignore = "a measurement: run it alone in a release build"]
fn the_stack_decodes_as_much_with_a_thread_for_each_stage() {
    let (whole_s, costs, whole) = run(0);
    eprintln!("one thread: 20 s of signal in {whole_s:.2} s, cost per stage {costs:.3?}");
    for mask in [0b01, 0b10, 0b11] {
        let (cut_s, costs, cut) = run(mask);
        eprintln!("cut at {mask:#04b}: 20 s of signal in {cut_s:.2} s, cost per stage {costs:.3?}");
        assert!(cut == whole, "the sound or the media packets differ, cut at {mask:#04b}");
        assert!(!whole.1.is_empty(), "no media packet");
    }
}
