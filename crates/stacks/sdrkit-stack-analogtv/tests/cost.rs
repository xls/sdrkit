//! The recorded cost of each stage of the `atv` and `fpv` stacks (`cost.txt`) against the cost measured on the test
//! signal, and the output of both with a thread for the picture stage. `SDRKIT_WRITE_COST=1` in a release build writes
//! the file. Two measurements run on request: the speed of the `atv` sender against real time, and the cost of each stage.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Values};
use sdrkit_stack_analogtv::typed::{AtvFields, FpvFields};
use std::path::PathBuf;

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    for stack in [&sdrkit_stack_analogtv::ATV, &sdrkit_stack_analogtv::FPV] {
        if let Err(e) = sdrkit_graph::cost::check(stack, "-", &path, 2.0, |_| {}) {
            panic!("{e}");
        }
    }
}

/// The seconds a run took, the cost of each stage, and the sound, the bytes of the media packets, the count of the
/// events and the count of the decoded pictures.
type Run = (f64, Vec<(String, f64)>, (Vec<u32>, Vec<u8>, usize, i64));

/// Runs `seconds` of the test signal of `stack` through it cut at `mask`, one thread per cut.
fn run(stack: &StackDescriptor, seconds: f64, mask: u32) -> Run {
    let rate = stack.tuning.rate_hz;
    let settings = Values::defaults(&(stack.settings)());
    let mut gen = (stack.make_generator.unwrap())(&GenConfig::new(rate, settings.clone())).unwrap();
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    let mut rx = (stack.make_receiver)(&StackConfig::new(settings)).unwrap();
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .unwrap();
    let threads: Vec<_> = rx
        .staged()
        .unwrap()
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), mask.count_ones() as usize);
    let (mut out, mut all, mut loads) = (Output::default(), Output::default(), Vec::new());
    let t = std::time::Instant::now();
    for b in x.chunks(65_536) {
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
    let fields = sdrkit_core::owner::Owner::results(rx.as_ref()).fields();
    let pictures = if stack.id == "atv" {
        fields.at(&AtvFields::PICTURES)
    } else {
        fields.at(&FpvFields::PICTURES)
    };
    let costs = loads
        .iter()
        .map(|l| {
            (
                l.name.clone(),
                l.busy_ns as f64 / 1e9 / l.signal_s.max(1e-9),
            )
        })
        .collect();
    let sound = all.audio().iter().map(|x| x.to_bits()).collect();
    let media = all
        .media
        .iter()
        .flat_map(|m| m.packet.bytes().to_vec())
        .collect();
    (
        took,
        costs,
        (sound, media, all.events.len(), pictures.unwrap_or(0)),
    )
}

#[test]
fn the_output_is_the_same_with_a_thread_for_the_picture_stage() {
    for stack in [&sdrkit_stack_analogtv::ATV, &sdrkit_stack_analogtv::FPV] {
        let (_, costs, whole) = run(stack, 1.5, 0);
        assert_eq!(costs.len(), 2, "{}: {costs:?}", stack.id);
        let (_, _, cut) = run(stack, 1.5, 1);
        assert!(whole.3 > 10, "{}: {} pictures decoded", stack.id, whole.3);
        // The results of a cut graph are those of the last report that the picture stage had finished when the feeding
        // thread read them, so the count of the cut run is not compared: the decoded pictures are in the media bytes.
        assert!(cut.0 == whole.0, "{}: the sound differs", stack.id);
        assert!(cut.1 == whole.1, "{}: the pictures differ", stack.id);
        assert_eq!(cut.2, whole.2, "{}: the events", stack.id);
    }
}

/// The CPU time of the calling thread, seconds.
fn thread_cpu_s() -> f64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid timespec for the call to write.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9
}

/// The thread CPU time of 4 s of the `atv` sender at 10 Msps, one field at a time: the test card, or (`changing`) a new
/// picture of the host every field (the card and the card mirrored, as BGRA fitted to the picture size).
fn sender_cpu_s(changing: bool) -> f64 {
    use sdrkit_core::Generator;
    use sdrkit_stack_analogtv::transmit::{fit_frame, AtvTx, TvModulator};
    use sdrkit_stack_analogtv::typed::TvPixels;
    let rate = 10e6;
    let mut g = sdrkit_stack_analogtv::AtvGenerator::new(AtvTx::default().params(rate));
    let card = g.card().clone();
    let (w, h) = (card.w, card.h);
    let bgra = |mirror: bool| -> Vec<u8> {
        let mut b = Vec::with_capacity(w * h * 4);
        for r in 0..h {
            for c in 0..w {
                let [red, green, blue] = card.rgb(if mirror { w - 1 - c } else { c }, r);
                let q = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
                b.extend_from_slice(&[q(blue), q(green), q(red), 255]);
            }
        }
        b
    };
    let frames = [bgra(false), bgra(true)];
    let field = (rate / 50.0) as usize;
    let mut x = vec![Complex32::default(); field];
    let audio = vec![0.1f32; g.programme_due(field) + 1];
    // On a machine with fast and slow cores, the fast ones.
    #[cfg(target_os = "macos")]
    // SAFETY: the call sets the class of the calling thread only.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
    }
    let t = thread_cpu_s();
    for f in 0..200 {
        if changing {
            let p = fit_frame(&frames[f % 2], w as u32, h as u32, TvPixels::Bgra, w, h).unwrap();
            TvModulator::show(&mut g, Some(p));
        }
        let due = g.programme_due(field);
        g.push_programme(&audio[..due]);
        g.generate(&mut x);
    }
    thread_cpu_s() - t
}

#[test]
#[ignore = "a measurement: run it alone"]
fn the_speed_of_the_tv_sender_against_real_time() {
    // The first millisecond of signal of a new sender.
    {
        use sdrkit_core::Generator;
        let mut g = sdrkit_stack_analogtv::AtvGenerator::new(
            sdrkit_stack_analogtv::transmit::AtvTx::default().params(10e6),
        );
        let mut x = vec![Complex32::default(); 10_000];
        let audio = vec![0.0f32; g.programme_due(x.len()) + 1];
        let t = thread_cpu_s();
        let due = g.programme_due(x.len());
        g.push_programme(&audio[..due]);
        g.generate(&mut x);
        eprintln!(
            "atv sender: the first 1 ms of signal in {:.2} ms of CPU",
            (thread_cpu_s() - t) * 1e3
        );
    }
    for (changing, name) in [(false, "the test card"), (true, "a new picture every field")] {
        let best = (0..3)
            .map(|_| sender_cpu_s(changing))
            .fold(f64::INFINITY, f64::min);
        eprintln!(
            "atv sender, {name}: 4 s of signal in {best:.3} s of CPU, {:.2} times real time",
            4.0 / best
        );
    }
}

#[test]
#[ignore = "a measurement: run it alone in a release build"]
fn the_cost_of_each_stage_on_one_thread_and_on_two() {
    for stack in [&sdrkit_stack_analogtv::ATV, &sdrkit_stack_analogtv::FPV] {
        for mask in [0, 1] {
            let best = (0..5)
                .map(|_| run(stack, 2.0, mask))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            eprintln!(
                "{} cut {mask}: 2 s of signal in {:.2} s, cost per stage {:.3?}",
                stack.id, best.0, best.1
            );
        }
    }
}
