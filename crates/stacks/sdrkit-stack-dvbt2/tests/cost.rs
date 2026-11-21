//! The recorded cost of each stage of the DVB-T2 stack (`cost.txt`) against the cost measured on the test signal, for
//! each variant: the FFT size, the constellation and the code rate of the PLP. `SDRKIT_WRITE_COST=1` in a release
//! build writes the file. The packets of the stack for every placement of the cuts and with 0, 1 and 4 helpers of
//! the worker pool.

use sdrkit_core::{ConfigError, GenConfig, Generator, Value, Values};
use std::path::PathBuf;

/// The variants: the heaviest one, which the graph declares, and those whose cost differs much from it.
const VARIANTS: &[&str] = &[
    sdrkit_stack_dvbt2::COST_VARIANT,
    "32k-64qam-2/3",
    "8k-256qam-2/3",
    "8k-64qam-2/3",
    "8k-qpsk-1/2",
];

/// The FEC blocks in each T2-frame of the test signals: a frame of about 0.2 s at 8K 64-QAM.
const BLOCKS: i64 = 100;

/// The settings of the stack with the `gen_*` settings of the variant `v`.
fn settings(v: &str) -> Values {
    let schema = (sdrkit_stack_dvbt2::DVBT2.settings)();
    let mut values = Values::defaults(&schema);
    let mut parts = v.split('-');
    for key in ["gen_fft", "gen_modulation", "gen_rate"] {
        let id = schema.id(key).expect("a setting of the generator");
        let part = parts.next().expect("three parts");
        let decl = schema.setting(id).expect("a setting");
        let index = decl.field.choice_index(part).expect("a known part");
        values.set(id, &Value::Choice(index)).expect("a choice");
    }
    let schema = values.schema().clone();
    let mut choose = |key: &str, part: &str| {
        let id = schema.id(key).expect("a setting of the generator");
        let decl = schema.setting(id).expect("a setting");
        let index = decl.field.choice_index(part).expect("a known part");
        values.set(id, &Value::Choice(index)).expect("a choice");
    };
    if v.starts_with("32k") {
        // The guard interval and the pilot pattern that broadcasts use with 32K.
        choose("gen_guard", "1/128");
        choose("gen_pilots", "pp7");
    }
    let id = schema.id("gen_blocks").expect("a setting");
    values.set(id, &Value::I64(BLOCKS)).expect("a count");
    values
}

/// The test signal of the stack with the PLP of the variant that `cfg.submode` names.
fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let variant = cfg.submode.as_deref().expect("a variant");
    sdrkit_stack_dvbt2::make_generator(&GenConfig::new(cfg.rate_hz, settings(variant)))
}

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = sdrkit_core::StackDescriptor {
        make_generator: Some(generator),
        tuning: sdrkit_stack_dvbt2::DVBT2.tuning.clone(),
        ..sdrkit_stack_dvbt2::DVBT2
    };
    for variant in VARIANTS {
        if let Err(e) = sdrkit_graph::cost::check(&stack, variant, &path, 2.0, |_| {}) {
            panic!("{e}");
        }
    }
}

/// What one run measured: the seconds it took, the thread and the share of one core of each stage, the share of each
/// component and section, and a digest of the transport packets.
struct Run {
    took_s: f64,
    stages: Vec<(String, u32, f64)>,
    parts: Vec<(String, f64)>,
    packets: usize,
    digest: u64,
}

/// Runs `seconds` of the test signal of `variant` through the stack cut at `mask`, one thread per cut. `helpers` is
/// the number of helper threads of the worker pool; `None` takes the shared pool.
fn run(variant: &str, seconds: f64, mask: u32, helpers: Option<usize>) -> Run {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig};
    let settings = settings(variant);
    let rate = sdrkit_stack_dvbt2::DVBT2.tuning.rate_hz;
    let mut gen = sdrkit_stack_dvbt2::make_generator(&GenConfig::new(rate, settings.clone()))
        .expect("a generator");
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    let mut rx = sdrkit_stack_dvbt2::receiver(&StackConfig::new(settings)).expect("builds");
    if let Some(n) = helpers {
        let pool = sdrkit_graph::Pool::new(n.max(1));
        pool.set_workers(n);
        rx.set_context(sdrkit_graph::RunContext {
            pool,
            ..Default::default()
        });
    }
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .expect("the rate");
    let mut tap = rx.graph_mut().tap("ts.ts", 1 << 14).expect("a tap");
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
                for &b in p.value.bytes() {
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
    let (mut loads, mut parts) = (Vec::new(), Vec::new());
    st.stage_loads(&mut loads);
    st.component_loads(&mut parts);
    // The stage after each cut ran on a thread of its own, and the stages between two cuts on one thread.
    for (i, l) in loads.iter().enumerate().skip(1) {
        let cut = mask & (1 << (i - 1)) != 0;
        assert_eq!(l.cut, cut, "stage {} cut at {mask:#04b}", l.name);
        assert_eq!(l.thread != loads[i - 1].thread, cut, "stage {}", l.name);
    }
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    take(&mut tap);
    let share = |ns: u64| ns as f64 / 1e9 / seconds;
    Run {
        took_s,
        stages: loads
            .iter()
            .map(|l| (l.name.clone(), l.thread, share(l.busy_ns)))
            .collect(),
        parts: parts
            .iter()
            .map(|c| (format!("{}.{}", c.component, c.section), share(c.busy_ns)))
            .filter(|p| p.1 >= 0.005)
            .collect(),
        packets,
        digest,
    }
}

#[test]
fn every_placement_of_the_cuts_and_the_worker_pool_give_the_packets_of_one_thread() {
    let alone = run("8k-64qam-2/3", 1.5, 0, Some(0));
    assert!(alone.packets > 5000, "{} packets", alone.packets);
    for mask in 1..4 {
        let cut = run("8k-64qam-2/3", 1.5, mask, Some(0));
        assert_eq!(
            (cut.packets, cut.digest),
            (alone.packets, alone.digest),
            "cut at {mask:#04b}"
        );
    }
    for helpers in [1, 4] {
        for mask in [0, 3] {
            let shared = run("8k-64qam-2/3", 1.5, mask, Some(helpers));
            assert_eq!(
                (shared.packets, shared.digest),
                (alone.packets, alone.digest),
                "{helpers} helpers, cut at {mask:#04b}"
            );
        }
    }
}

#[test]
#[ignore = "a measurement: run it alone in a release build"]
fn the_share_of_each_stage_and_thread() {
    for variant in ["8k-64qam-2/3", sdrkit_stack_dvbt2::COST_VARIANT] {
        for (mask, helpers) in [(0, Some(0)), (3, Some(0)), (3, None)] {
            let r = run(variant, 4.0, mask, helpers);
            eprintln!(
                "{variant} cut at {mask:#04b}, helpers {helpers:?}: 4 s of signal in {:.2} s, {} packets, thread and \
                 share per stage {:.3?}, parts {:.3?}",
                r.took_s, r.packets, r.stages, r.parts
            );
        }
    }
}
