//! DVB-C2 (`dvbc2`): second-generation cable TV. 4K OFDM with guard 1/64 or 1/128, the preamble with L1 signalling part 2,
//! data slices of type 1 and 2, PLPs of 16- to 4096-QAM with 16K or 64K LDPC and BCH, on the 8 or 6 MHz raster. The receiver
//! decodes one PLP (with its common PLP, or from every slice that carries it when bundled) into BBFRAMEs; the mode adaptation
//! of `sdrkit-proto-ipcast` makes the transport stream with the common PLP put back (to the layer of `sdrkit-proto-mpegts`) or
//! the IP packets of GSE, and the receiver itself gives the user packets of a GFPS or the data of a GCS.
//!
//! Standard: ETSI EN 302 769 V1.3.1.

pub mod bicm;
pub mod comp;
pub mod decode;
pub mod gen;
pub mod header;
pub mod l1;
pub mod packets;
pub mod rx;
pub mod settings;
pub mod signal;
pub mod tables;

use crate::TS_LAYER;
use crate::typed::Dvbc2Fields;
use comp::{C2Comp, C2Fec, MIN_RATE};
use gen::{C2Generator, GenParams};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, Receiver, Results, ResultsDecl, SettingSet, StackConfig,
    StageError, Value, Values,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph, Stage};
use sdrkit_proto_ipcast::components::{BbFrameAdapt, IpComponent, SystemChoice};
use sdrkit_proto_mpegts::layer;
use sdrkit_proto_mpegts::monitor::TsMonitorComponent;
use settings::Dvbc2Setting;

/// The stack id.
pub const ID: &str = "dvbc2";

/// The period of the results, seconds of signal.
pub const REPORT_S: f64 = 0.1;

/// Input samples per graph pass.
pub const PASS: usize = 16_384;

/// The stage of the decoding: time de-interleaving, LDPC and BCH.
pub const FEC_STAGE: &str = "fec";

/// What the stack decodes and which of its values no public document confirms.
pub const NOTE: &str = "Decodes one PLP of a data slice of type 1 or 2 on the 8 or 6 MHz raster: transport stream or \
    GSE (normal or high efficiency mode) to the mode adaptation, GFPS user packets or GCS data fields to the generic output, \
    a bundled PLP from every slice in the window that carries it. The common PLP of a grouped data PLP is put back into the \
    transport stream of the data PLP: a null packet of the data PLP takes the packet of the common PLP in the same slot, the \
    streams lined up by packet count, or by the one offset that puts no two packets in a slot when they do not line up. \
    When the slice of the PLP or of its common PLP reaches out of the 7.61 MHz window, the tuning positions that the L1 \
    gives for the two slices are tried and the first whose window holds both is taken: the window moves there when the \
    samples hold it; otherwise retune_hz names the frequency and the stack asks the host once to tune to it. When no \
    position holds both, outside_slice and outside_hz name the slice out of the window and how far it reaches out, no \
    request is made, and the data PLP is decoded without its common PLP when its own slice is inside. Not done: GFPS and \
    GCS in high efficiency mode, bundled slices outside the window, the line-up of the common PLP by ISSY; slices next \
    to a broadband notch are not tested. Not confirmed by a \
    public document, and settings with these defaults: the guard correlation metric 0.22 over 8 symbol periods \
    (timing_metric), the scattered pilot energy ratio 3 that marks a preamble (preamble_ratio), the carrier index metric 0.35 \
    (index_metric) over 64 carriers either side (index_span), the header candidate measure 0.45 (header_candidate), 22 of 32 \
    header bits agreeing (header_agree), a header found by search taken only when another follows its packet, 2 failed \
    headers that lose the header lock (header_misses), 8 failed blocks that drop \
    the alignment of a type 1 slice (block_misses), 3 frames without a preamble before a new timing search \
    (preamble_misses), 50 LDPC iterations (ldpc_iterations), 16 BBFRAMEs of a bundled PLP held for the ISCR order \
    (bundle_hold), and the header scrambling word w^RM = 0x1D8D3BF3 read from one printed source (rm_scrambling). \
    Interpretations: the time interleaver has one column per carrier of the slice, N_DSLICE_WIDTH = K_DS,max - K_DS,min + 1 \
    (ti_full_width): clause 9.4.4 prints K_DS,max - K_DS,min, but clause 9.4.1 makes K_DS,max the last carrier of the slice \
    and the memory holds every carrier of the slice; PLP_START counts cells of the slice after time de-interleaving from the \
    first data cell of the frame; the 32 header cells and the L1 row repeat together in a preamble symbol; L1 time \
    interleaving with more FEC blocks than its depth is refused; a failed L1 keeps the last good one for one frame, a second \
    failure in a row starts the preamble search; a bundled PLP is ordered by the ISCR of its BBHEADERs; the common PLP is the \
    PLP of type 00 with the PLP_GROUP_ID of the chosen PLP; a window holds a slice when every carrier of the slice lies \
    within 1760 carriers of the window centre. The default frequency 346 MHz is a neutral value. The sample \
    clock is corrected in the resampler at every input rate; the carrier at 0 Hz of the samples is erased.";

/// The graph of the receiver at its default settings. The decoding (`fec`) begins the second stage; the publisher of the
/// results is in it.
pub fn builder() -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("rx", C2Comp::default())
        .add("fec", C2Fec::default())
        .add("bbframe", BbFrameAdapt::default())
        .add("ip", IpComponent::default())
        .add("monitor", TsMonitorComponent::new(1))
        .input("rx.in")
        .connect("rx.cells", "fec.in")
        .connect("rx.report", "fec.report")
        .stage("rx", "fec", Stage::new(FEC_STAGE))
        .connect("fec.bbframes", "bbframe.in")
        .connect("fec.plps", "bbframe.plps")
        .output("rx.requests")
        .connect("bbframe.ip", "ip.in")
        .connect("bbframe.ts", "monitor.in")
        .set("bbframe.system", Value::Choice(SystemChoice::DvbT2.index()));
    layer::attach(b, "monitor.out", &TS_LAYER)
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("fec")
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Where each stack setting goes.
const ROUTES: &[(&str, &[&str])] = &[
    layer::DECODE_VIDEO_ROUTE,
    ("plp", &["rx.plp"]),
    ("slice", &["rx.slice"]),
    ("centre_known", &["rx.centre_known"]),
    ("raster", &["rx.raster"]),
    ("ldpc_iterations", &["rx.ldpc_iterations"]),
    ("timing_metric", &["rx.timing_metric"]),
    ("preamble_ratio", &["rx.preamble_ratio"]),
    ("index_metric", &["rx.index_metric"]),
    ("index_span", &["rx.index_span"]),
    ("header_candidate", &["rx.header_candidate"]),
    ("header_agree", &["rx.header_agree"]),
    ("header_misses", &["rx.header_misses"]),
    ("block_misses", &["rx.block_misses"]),
    ("preamble_misses", &["rx.preamble_misses"]),
    ("ti_full_width", &["rx.ti_full_width"]),
    ("rm_scrambling", &["rx.rm_scrambling"]),
    ("bundle_hold", &["rx.bundle_hold"]),
];

/// The DVB-C2 receiver with the settings of `cfg`.
///
/// # Errors
///
/// [`GraphError::NamedSetting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "too low for DVB-C2 (8 Msps or more on the 8 MHz raster, 6 Msps on the 6 MHz raster)")
        .fixed_passes(PASS)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for (key, targets) in ROUTES {
        b = b.route(key, targets)?;
    }
    b.default_centre(DVBC2.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = GenParams::from_values(&cfg.settings);
    C2Generator::new(p, cfg.rate_hz)
        .map(|g| Box::new(g) as Box<dyn Generator>)
        .ok_or_else(|| ConfigError::Stage(StageError::new(gen::GenError)))
}

/// The DVB-C2 stack.
pub static DVBC2: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
    id: ID,
    name: "DVB-C2",
    group: "TV",
    blurb: "Second-generation cable TV (DVB-C2, ETSI EN 302 769): one PLP with its common PLP put back into its transport stream, or a PLP bundled over several slices, 16- to 4096-QAM, transport stream, GSE, GFPS or GCS.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 47e6,
        max_hz: 1002e6,
        radio_filter_hz: Some(8e6),
        no_offset_tuning: true,
        ..sdrkit_core::Tuning::anywhere(346e6, 10e6, MIN_RATE, 8e6)
    },
    submodes: &[],
    settings: Dvbc2Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[sdrkit_core::SignalType::DvbC2],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: Some(benchmarks),
};

static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The test signal in 64K 64-QAM 2/3 (it decodes at the default SNR of a verification), type 1, guard 1/64, a few kHz off:
/// the L1 decodes, FEC blocks decode and the test multiplex names its service.
fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 64-QAM 2/3 type 1", o, 3e3)
        .with(&settings::GEN_MODCOD, settings::GenModCod::Q64R23);
    c.seconds = c.seconds.max(1.5);
    c.expect_audio = Some(false);
    c.check = Some(check_case);
    vec![c]
}

fn check_case(r: &Results) -> Vec<CheckLine> {
    let ok = r.fields().at(&Dvbc2Fields::BLOCKS_OK).unwrap_or(0);
    let bad = r.fields().at(&Dvbc2Fields::BLOCKS_BAD).unwrap_or(0);
    let l1 = r.fields().at(&Dvbc2Fields::NETWORK_ID);
    let mut names: Vec<&str> = Vec::new();
    if let Some(t) = r
        .decl()
        .table_id(sdrkit_proto_mpegts::publish::SERVICES.key)
        .and_then(|id| r.table(id))
    {
        for (_, row) in t.rows_newest_first() {
            if let Some(name) = row.get(&sdrkit_proto_mpegts::publish::NAME) {
                names.push(name);
            }
        }
    }
    let named = names.join(", ");
    vec![
        CheckLine::new("l1", l1.is_some(), format!("network id {l1:?}")),
        CheckLine::new("blocks_ok", ok >= 100, format!("{ok} good, {bad} bad")),
        CheckLine::new("services", named.contains("sdrkit test"), named),
    ]
}

/// The benchmarks of the stack: the receiver on its test signal, 64K 256-QAM 5/6 at 10 Msps and 30 dB (items: input samples;
/// the CPU share is of one core by the clock).
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "dvbc2.receiver_256qam_56",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::BenchResult {
    let schema = Dvbc2Setting::schema();
    let v = Values::defaults(&schema);
    let p = GenParams::from_values(&v);
    let rate = 10e6;
    let Some(mut g) = C2Generator::new(p, rate) else {
        return sdrkit_core::BenchResult::default();
    };
    let n = (1.0 * rate) as usize;
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    if let Ok(mut a) = sdrkit_algo_sim::noise::Awgn::with_power(1e-3, 0x4332) {
        a.process(&mut x);
    }
    let cfg = StackConfig::new(v);
    let x = std::sync::Arc::new(x);
    sdrkit_core::bench::measure(seconds, n, n as f64 / rate, || {
        let Ok(mut rx) = receiver(&cfg) else {
            return;
        };
        let input = sdrkit_core::InputInfo::new(rate, p.freq_hz, p.freq_hz).recording();
        if rx.configure(&input).is_err() {
            return;
        }
        let mut out = sdrkit_core::Output::default();
        for c in x.chunks(16384) {
            rx.feed(c, &mut out);
            out.clear();
        }
        rx.finish(&mut out);
    })
}
