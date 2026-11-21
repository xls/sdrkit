//! The receiver of `attach` cut at every stage boundary, and with pools of every size, gives the BBFRAMEs of one thread.

use sdrkit_core::{Complex32, Generator, Output, Staged, Value};
use sdrkit_graph::{Buffer, Data, Format, Graph, GraphBuilder, Pool, RunContext};
use sdrkit_proto_dvbs2::gen::{DvbsGenParams, DvbsGenerator};
use sdrkit_proto_dvbs2::{attach, Dvbs2Parts};

/// The input rate of the test signal.
const RATE: f64 = 10e6;

/// The count and a digest of the BBFRAMEs, and the names of the stages.
struct Run {
    frames: usize,
    digest: u64,
    stages: Vec<String>,
}

/// The graph of `attach` with the transport stream, at the symbol rate of the test signal, with a pool of `helpers`
/// helper threads.
fn graph(helpers: usize) -> Graph {
    let mut g = attach(
        GraphBuilder::new(),
        Dvbs2Parts {
            ts: true,
            field: false,
        },
    )
    .set("s2_front.symbol_rate", Value::Hz(5e6))
    .clock(0.25)
    .build()
    .expect("the graph builds");
    let pool = Pool::new(helpers);
    pool.set_workers(helpers);
    g.set_context(RunContext {
        pool,
        ..RunContext::default()
    });
    g.configure_input(Format::iq(RATE, 0.0, 0.0))
        .expect("the rate");
    g
}

/// Runs `x` through the graph cut at `mask`, one thread per cut, with a pool of `helpers` helper threads.
fn run(x: &[Complex32], mask: u32, helpers: usize) -> Run {
    let mut g = graph(helpers);
    let mut tap = g.tap("s2_fec.out", 1 << 16).expect("the tapped port");
    let threads: Vec<_> = g
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(
        threads.len(),
        mask.count_ones() as usize,
        "a thread per cut"
    );
    let mut buf = Buffer::new(&sdrkit_graph::Kind::Bytes);
    let (mut frames, mut digest) = (0, 0xcbf2_9ce4_8422_2325_u64);
    let mut take = |tap: &mut sdrkit_graph::TapReader| {
        while tap.pop(&mut buf) {
            let Data::Bytes(v) = &buf.data else {
                continue;
            };
            for p in v {
                frames += 1;
                for &b in p.bytes().iter().chain(&p.time.to_le_bytes()) {
                    digest = (digest ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
    };
    let mut out = Output::default();
    for b in x.chunks(8192) {
        g.process(b, &mut out);
        out.clear();
        take(&mut tap);
    }
    g.drain(&mut out);
    let stages = g.stages().into_iter().map(|s| s.name).collect();
    drop(g.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    g.finish(&mut out);
    take(&mut tap);
    Run {
        frames,
        digest,
        stages,
    }
}

#[test]
fn every_cut_and_every_pool_give_the_frames_of_one_thread() {
    let mut gen = DvbsGenerator::new(DvbsGenParams::new(RATE));
    let mut x = vec![Complex32::default(); (0.4 * RATE) as usize];
    gen.generate(&mut x);
    let whole = run(&x, 0, 0);
    eprintln!(
        "{} frames, digest {:#x}, stages {:?}",
        whole.frames, whole.digest, whole.stages
    );
    assert!(whole.frames > 30, "{} frames", whole.frames);
    assert_eq!(whole.stages, ["front", "fec"]);
    for mask in 0..1u32 << (whole.stages.len() - 1) {
        for helpers in [1, 3] {
            let cut = run(&x, mask, helpers);
            assert_eq!(
                (cut.frames, cut.digest),
                (whole.frames, whole.digest),
                "cut at {mask:#04b}, {helpers} helpers"
            );
        }
    }
}
