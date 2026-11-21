//! Elektro-L and Arktika-M raw downlink: the MSU-GS infrared frame, the virtual channel rule, the test signal of both
//! submodes through the whole receiver, noise alone and a signal after noise, the recorded cost of each stage and the
//! output of every cut of the stages against one thread.

use super::common::{impaired, run};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver};
use sdrkit_dev_test::Harness;
use sdrkit_graph::{Buffer, Data, Graph, Kind, Timed};
use sdrkit_stack_satimg::components::table::FrameBytes;
use sdrkit_stack_satimg::instruments::msugs;
use sdrkit_stack_satimg::modes::rdas::{
    test_ir_frame, virtual_channel, TestCadus, ARKTIKA_M, ASKED_RATE, DESCRIPTOR, ELEKTRO_L,
};

const RATE: f64 = ASKED_RATE;

#[test]
fn an_infrared_frame_for_line_7_segment_4_decodes() {
    let f = test_ir_frame(7, 4);
    let d = msugs::decode_ir(&f).expect("a frame");
    assert_eq!((d.line, d.segment, d.valid), (7, 4, true));
    for c in 0..msugs::IR_CHANNELS {
        for i in 0..msugs::IR_SEGMENT {
            assert_eq!(d.pixels[c * msugs::IR_SEGMENT + i], msugs::test_value(c, 2 * 183 + i, 7));
        }
    }
}

#[test]
fn the_virtual_channel_follows_bytes_5_and_11() {
    let mut c = vec![0u8; 1024];
    c[5] = 4 * 2;
    assert_eq!(virtual_channel(&c), 4);
    c[5] = 7 * 2;
    c[11] = 3 * 2;
    assert_eq!(virtual_channel(&c), 3);
    c[11] = 6 * 2;
    assert_eq!(virtual_channel(&c), 7);
    let mut t = TestCadus::default();
    let mut d = c.clone();
    for k in 0..3 {
        d.copy_from_slice(&t.next_cadu());
        sdrkit_proto_ccsds::randomiser::tm_bytes(&mut d[4..]);
        assert_eq!(virtual_channel(&d), if k == 0 { 2 } else { 4 });
    }
}

fn harness(submode: &str, seconds: f64, esn0: f64, offset: f64, ppm: f64) -> Harness {
    impaired(
        Harness::new(&DESCRIPTOR)
            .submode(submode)
            .rate(RATE)
            .seconds(seconds),
        Some(esn0),
        offset,
        ppm,
    )
}

#[test]
fn elektro_l_test_signal_gives_frames_and_pictures() {
    let h = harness(ELEKTRO_L, 0.5, 18.0, 300e3, 30.0);
    let x = h.generate().expect("a signal");
    let r = run::<sdrkit_stack_satimg::items::Cadu>(&h, &x, RATE, 7500e6, 7500e6, 1 << 18, None);
    let frames = r.num("frames_ok");
    assert!(frames >= 400.0, "{frames} frames, {}", r.text("note"));
    assert!(r.num("packets") >= 50.0, "{} MSU-GS frames", r.num("packets"));
    assert!(r.num("lines_ir") >= 1.0);
    assert!(r.num("rows") >= 1.0, "picture rows {}", r.num("rows"));
}

#[test]
fn arktika_m_test_signal_gives_frames() {
    let h = harness(ARKTIKA_M, 0.4, 18.0, -200e3, -20.0);
    let x = h.generate().expect("a signal");
    let r = run::<sdrkit_stack_satimg::items::Cadu>(&h, &x, RATE, 7865e6, 7865e6, 1 << 18, None);
    assert!(r.num("frames_ok") >= 200.0, "{} frames", r.num("frames_ok"));
}

/// White noise of variance `var` (both rails together), `n` samples.
fn noise(n: usize, var: f64, seed: u64) -> Vec<Complex32> {
    let mut g = sdrkit_algo_sim::noise::Awgn::with_power(var, seed).expect("a power");
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.process(&mut x);
    x
}

/// Feeds `x` in pieces of 10 ms and returns the signal time after each piece and the good frames.
fn feed(rx: &mut dyn Receiver, x: &[Complex32]) -> Vec<(f64, f64, f64)> {
    let mut out = Output::default();
    let piece = (RATE * 0.01) as usize;
    let mut seen = Vec::new();
    for p in x.chunks(piece) {
        out.clear();
        rx.feed(p, &mut out);
        let f = |k: &str| {
            let r = sdrkit_core::Owner::results(rx);
            r.decl()
                .field_id(k)
                .and_then(|id| r.fields().value(id))
                .map_or(f64::NAN, |v| match v {
                    sdrkit_core::Value::I64(i) => *i as f64,
                    sdrkit_core::Value::F64(v) | sdrkit_core::Value::Duration(v) => *v,
                    _ => f64::NAN,
                })
        };
        seen.push((f("signal_time"), f("frames_ok"), f("gate_skipped")));
    }
    seen
}

#[test]
fn noise_alone_gives_no_frame_and_is_mostly_let_go_by() {
    let mut rx = Harness::new(&DESCRIPTOR).submode(ELEKTRO_L).receiver().expect("a receiver");
    rx.configure(&InputInfo::new(RATE, 7500e6, 7500e6).recording()).expect("the rate");
    let x = noise((RATE * 0.6) as usize, 0.05, 5);
    let seen = feed(&mut *rx, &x);
    let (t, frames, skipped) = *seen.last().unwrap();
    assert_eq!(frames, 0.0);
    assert!(skipped > 0.7 * x.len() as f64, "{skipped} of {}", x.len());
    let mut last = 0.0;
    for (k, (t, _, _)) in seen.iter().enumerate() {
        let fed = (k + 1) as f64 * 0.01;
        let t = if t.is_nan() { 0.0 } else { *t };
        assert!(fed - t <= 0.2 + 1e-9, "a report gap at {fed} s (signal time {t})");
        last = t;
    }
    assert!(t >= last);
}

#[test]
fn a_signal_after_noise_is_found_at_once() {
    let h = harness(ELEKTRO_L, 0.3, 10.0, 150e3, 0.0);
    let signal = h.generate().expect("a signal");
    // The noise of the impairments: Es/N0 10 dB at -13 dBFS.
    let p = 10f64.powf(-1.3);
    let var = p * RATE / 30.72e6 / 10.0;
    let before = noise((RATE * 0.2) as usize, var, 11);
    let mut rx = h.receiver().expect("a receiver");
    rx.configure(&InputInfo::new(RATE, 7500e6, 7500e6).recording()).expect("the rate");
    let a = feed(&mut *rx, &before);
    let skipped_before = a.last().unwrap().2;
    // The first good frame leaves the frame layer within 0.06 s of the start of the signal.
    let mut reader = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .tap("frames.frames", 1 << 16)
        .expect("the tapped port");
    let mut buf = Buffer::new(&Kind::typed::<FrameBytes>());
    let mut out = Output::default();
    let piece = (RATE * 0.002) as usize;
    let mut first = None;
    for (k, p) in signal[..(RATE * 0.1) as usize].chunks(piece).enumerate() {
        out.clear();
        rx.feed(p, &mut out);
        while reader.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                if v.as_any().downcast_ref::<Vec<Timed<FrameBytes>>>().is_some_and(|v| !v.is_empty()) {
                    first.get_or_insert(k);
                }
            }
        }
    }
    let first = first.expect("frames") as f64 * 0.002;
    assert!(first <= 0.06, "the first frame {first} s after the start");
    let b = feed(&mut *rx, &signal[(RATE * 0.1) as usize..]);
    let (_, frames, skipped) = *b.last().unwrap();
    assert!(frames >= 300.0, "{frames} frames");
    let _ = first;
    assert!(skipped - skipped_before < 0.05 * RATE, "{} samples of signal let go by", skipped - skipped_before);
}

/// The good frames that leave the frame layer for `x`, fed in pieces of 2 ms and then finished, as live samples or as a
/// recording.
fn frames_of(h: &Harness, x: &[Complex32], live: bool) -> usize {
    let mut rx = h.receiver().expect("a receiver");
    let info = InputInfo::new(RATE, 7500e6, 7500e6);
    rx.configure(&if live { info } else { info.recording() }).expect("the rate");
    let mut reader = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .tap("frames.frames", 1 << 16)
        .expect("the tapped port");
    let mut buf = Buffer::new(&Kind::typed::<FrameBytes>());
    let mut out = Output::default();
    let mut count = 0;
    let mut drain = |count: &mut usize| {
        while reader.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                *count += v.as_any().downcast_ref::<Vec<Timed<FrameBytes>>>().map_or(0, Vec::len);
            }
        }
    };
    for p in x.chunks((RATE * 0.002) as usize) {
        out.clear();
        rx.feed(p, &mut out);
        drain(&mut count);
    }
    out.clear();
    rx.finish(&mut out);
    drain(&mut count);
    count
}

#[test]
fn live_samples_give_the_frames_of_a_recording() {
    let h = harness(ELEKTRO_L, 0.25, 18.0, 100e3, 0.0);
    let x = h.generate().expect("a signal");
    let (live, recorded) = (frames_of(&h, &x, true), frames_of(&h, &x, false));
    assert!(recorded >= 150, "{recorded} frames");
    assert_eq!(live, recorded);
}

/// The test signal with white noise at an Es/N0 of 20 dB.
fn noisy_generator(cfg: &sdrkit_core::GenConfig) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
    struct Noisy(Box<dyn sdrkit_core::Generator>, sdrkit_algo_sim::noise::Awgn);
    impl sdrkit_core::Generator for Noisy {
        fn rate_hz(&self) -> f64 {
            self.0.rate_hz()
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            let n = self.0.generate(out);
            self.1.process(&mut out[..n]);
            n
        }
        fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
            self.0.signal_info()
        }
    }
    let make = DESCRIPTOR.make_generator.expect("a test signal");
    let noise = sdrkit_algo_sim::noise::Awgn::with_power(cfg.rate_hz / 30.72e6 / 100.0, 5).expect("a power");
    Ok(Box::new(Noisy(make(cfg)?, noise)))
}

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = sdrkit_core::StackDescriptor {
        make_generator: Some(noisy_generator),
        tuning: DESCRIPTOR.tuning.clone(),
        ..DESCRIPTOR
    };
    if let Err(e) = sdrkit_graph::cost::check(&stack, "-", &path, 2.0, |_| {}) {
        panic!("{e}");
    }
}

/// The good frames and the picture strips of `x` through the receiver cut at `mask`, one thread for each cut: the count
/// and a digest of each, and the names of the stages.
fn cut_run(h: &Harness, x: &[Complex32], mask: u32) -> ((usize, u64), (usize, u64), Vec<String>) {
    use sdrkit_codec_image::strip::Strip;
    let mut rx = h.receiver().expect("a receiver");
    rx.configure(&InputInfo::new(RATE, 7500e6, 7500e6).recording()).expect("the rate");
    let graph = rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>()).expect("a graph");
    let mut frame_tap = graph.tap("frames.frames", 1 << 16).expect("the tapped port");
    let mut strip_tap = graph.tap("msugs.out", 1 << 16).expect("the tapped port");
    let threads: Vec<_> = rx
        .staged()
        .expect("stages")
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), mask.count_ones() as usize, "a thread for each cut");
    let mix = |d: &mut u64, bytes: &[u8]| {
        for &b in bytes {
            *d = (*d ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    let mut frame_buf = Buffer::new(&Kind::typed::<FrameBytes>());
    let mut strip_buf = Buffer::new(&Kind::typed::<Strip>());
    let (mut frames, mut strips) = ((0, 0xcbf2_9ce4_8422_2325_u64), (0, 0xcbf2_9ce4_8422_2325_u64));
    let mut take = |frames: &mut (usize, u64), strips: &mut (usize, u64)| {
        while frame_tap.pop(&mut frame_buf) {
            let Data::Items(Some(v)) = &frame_buf.data else { continue };
            for f in v.as_any().downcast_ref::<Vec<Timed<FrameBytes>>>().into_iter().flatten() {
                frames.0 += 1;
                mix(&mut frames.1, &f.value.bytes);
            }
        }
        while strip_tap.pop(&mut strip_buf) {
            let Data::Items(Some(v)) = &strip_buf.data else { continue };
            for t in v.as_any().downcast_ref::<Vec<Timed<Strip>>>().into_iter().flatten() {
                let s = &t.value;
                strips.0 += 1;
                mix(&mut strips.1, &s.channel.to_le_bytes());
                mix(&mut strips.1, &s.first_row.to_le_bytes());
                mix(&mut strips.1, &s.x.to_le_bytes());
                mix(&mut strips.1, &s.samples);
            }
        }
    };
    let mut out = Output::default();
    for p in x.chunks(16_384) {
        out.clear();
        rx.feed(p, &mut out);
        take(&mut frames, &mut strips);
    }
    let st = rx.staged().expect("stages");
    st.drain(&mut out);
    let mut loads = Vec::new();
    st.stage_loads(&mut loads);
    for (i, l) in loads.iter().enumerate().skip(1) {
        assert_eq!(l.cut, mask & (1 << (i - 1)) != 0, "stage {} at {mask:#04b}", l.name);
    }
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    take(&mut frames, &mut strips);
    (frames, strips, loads.into_iter().map(|l| l.name).collect())
}

#[test]
fn every_cut_of_the_stages_gives_the_frames_and_strips_of_one_thread() {
    let h = harness(ELEKTRO_L, 0.25, 18.0, 100e3, 0.0);
    let x = h.generate().expect("a signal");
    let (frames, strips, stages) = cut_run(&h, &x, 0);
    assert_eq!(stages, ["front", "demod", "picture"]);
    assert!(frames.0 >= 150 && strips.0 >= 100, "{} frames, {} strips", frames.0, strips.0);
    for mask in 1..4 {
        let (f, s, _) = cut_run(&h, &x, mask);
        assert_eq!((f, s), (frames, strips), "cut at {mask:#04b}");
    }
}
