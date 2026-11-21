//! The recorded cost of each stage of the ATSC and DTMB stacks (`cost.txt`) against the cost measured on the test signal,
//! and the transport packets of each stack cut at every mask against those of one thread. `SDRKIT_WRITE_COST=1` in an
//! optimised build writes the file.

use sdrkit_core::{
    Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, StackDescriptor, Values,
};
use sdrkit_graph::{GraphError, StackGraph};
use sdrkit_stack_tvterr::{atsc, dtmb};
use std::path::PathBuf;

/// A stack of the crate with stages: its descriptor, its receiver and the port of its transport stream.
struct Stack {
    d: &'static StackDescriptor,
    receiver: fn(&StackConfig) -> Result<StackGraph, GraphError>,
    ts: &'static str,
}

const ATSC: Stack = Stack {
    d: &atsc::DESCRIPTOR,
    receiver: atsc::receiver,
    ts: "fec.ts",
};

const DTMB: Stack = Stack {
    d: &dtmb::DESCRIPTOR,
    receiver: dtmb::receiver,
    ts: "fec.ts",
};

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    for s in [&ATSC, &DTMB] {
        if let Err(e) = sdrkit_graph::cost::check(s.d, "-", &path, 2.0, |_| {}) {
            panic!("{}: {e}", s.d.id);
        }
    }
}

/// What one run measured: the seconds it took, the name, thread and share of one core of each stage, and the count and a
/// digest of the transport packets with their times.
struct Run {
    took_s: f64,
    stages: Vec<(String, u32, f64)>,
    packets: usize,
    digest: u64,
}

/// Runs `seconds` of the test signal of `s` through its receiver cut at `mask`, one thread per cut.
fn run(s: &Stack, seconds: f64, mask: u32) -> Run {
    let d = s.d;
    let rate = d.tuning.rate_hz;
    let settings = Values::defaults(&(d.settings)());
    let mut gen =
        (d.make_generator.expect("a test signal"))(&GenConfig::new(rate, settings.clone()))
            .expect("a generator");
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    let mut rx = (s.receiver)(&StackConfig::new(settings)).expect("builds");
    rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording())
        .expect("the rate");
    let mut tap = rx.graph_mut().tap(s.ts, 1 << 14).expect("a tap");
    let threads: Vec<_> = rx
        .staged()
        .expect("stages")
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(
        threads.len(),
        mask.count_ones() as usize,
        "a thread per cut"
    );
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let (mut packets, mut digest) = (0, 0xcbf2_9ce4_8422_2325_u64);
    let mut take = |tap: &mut sdrkit_graph::TapReader| {
        while tap.pop(&mut buf) {
            let sdrkit_graph::Data::Items(Some(v)) = &buf.data else {
                continue;
            };
            type Packets = Vec<sdrkit_graph::Timed<sdrkit_proto_mpegts::stream::TsPacket>>;
            for p in v.as_any().downcast_ref::<Packets>().into_iter().flatten() {
                packets += 1;
                for &b in p.value.bytes().iter().chain(&p.time.to_le_bytes()) {
                    digest = (digest ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    };
    let mut out = Output::default();
    let t = std::time::Instant::now();
    for b in x.chunks(16_384) {
        rx.feed(b, &mut out);
        out.clear();
        take(&mut tap);
    }
    let st = rx.staged().expect("stages");
    st.drain(&mut out);
    let took_s = t.elapsed().as_secs_f64();
    let mut loads = Vec::new();
    st.stage_loads(&mut loads);
    for (i, l) in loads.iter().enumerate().skip(1) {
        let cut = mask & (1 << (i - 1)) != 0;
        assert_eq!(l.cut, cut, "stage {} cut at {mask:#06b}", l.name);
    }
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    take(&mut tap);
    Run {
        took_s,
        stages: loads
            .iter()
            .map(|l| (l.name.clone(), l.thread, l.busy_ns as f64 / 1e9 / seconds))
            .collect(),
        packets,
        digest,
    }
}

/// The packets of `s` on one thread and at every cut are the same, and the stages are `stages`.
fn every_cut(s: &Stack, seconds: f64, least: usize, stages: &[&str]) {
    let whole = run(s, seconds, 0);
    let names: Vec<&str> = whole.stages.iter().map(|(n, _, _)| n.as_str()).collect();
    eprintln!(
        "{}: {} packets, digest {:#x}, stages {names:?}",
        s.d.id, whole.packets, whole.digest
    );
    assert!(whole.packets > least, "{} packets", whole.packets);
    assert_eq!(names, stages);
    for mask in 1..1u32 << (stages.len() - 1) {
        let cut = run(s, seconds, mask);
        assert_eq!(
            (cut.packets, cut.digest),
            (whole.packets, whole.digest),
            "cut at {mask:#06b}"
        );
    }
}

#[test]
fn every_cut_of_atsc_gives_the_packets_of_one_thread() {
    every_cut(&ATSC, 1.0, 5000, &["front", "equalise", "fec", "ts"]);
}

#[test]
fn every_cut_of_dtmb_gives_the_packets_of_one_thread() {
    every_cut(&DTMB, 1.0, 5000, &["front", "fec", "ts"]);
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_each_stage_and_the_time_of_each_cut() {
    for s in [&ATSC, &DTMB] {
        let whole = run(s, 6.0, 0);
        eprintln!(
            "{}, one thread: 6 s of signal in {:.2} s, {} packets, digest {:#x}, share per stage {:.3?}",
            s.d.id, whole.took_s, whole.packets, whole.digest, whole.stages
        );
        for mask in 1..1u32 << (whole.stages.len() - 1) {
            let cut = run(s, 6.0, mask);
            eprintln!(
                "{}, cut at {mask:#06b}: 6 s of signal in {:.2} s, thread and share per stage {:.3?}",
                s.d.id, cut.took_s, cut.stages
            );
            assert_eq!(
                (cut.packets, cut.digest),
                (whole.packets, whole.digest),
                "cut at {mask:#06b}"
            );
        }
    }
}
