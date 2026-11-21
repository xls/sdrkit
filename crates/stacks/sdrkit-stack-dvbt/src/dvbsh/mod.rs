//! DVB-SH: mobile TV to handhelds from satellites and terrestrial repeaters. The stack receives the OFDM form SH-A (1K, 2K, 4K
//! and 8K, QPSK and 16-QAM, also hierarchical, widths 8, 7, 6, 5 and 1.7 MHz) on the DVB-T OFDM chain in its DVB-SH reading,
//! and the TDM form SH-B (QPSK, 8PSK and 16APSK at the symbol rates of table 5.13) on a single-carrier chain. Both forms end
//! in the common channel coding: the time de-interleaver, the turbo code with its twelve puncturing patterns, and the
//! transport stream. The form and the width are settings, found automatically by default: the OFDM form is tried on every
//! usable width, then the TDM form, in turn.
//!
//! Standard: ETSI EN 302 583 V1.2.1; ETSI TS 102 584 clause 7.2.3.3 for the time interleaver's branch delays.

pub mod coding;
pub mod gen;
pub mod ofdm;
pub mod results;
pub mod rx;
pub mod settings;
pub mod shframe;
pub mod tdm;
pub mod tdmgen;
pub mod tdmrx;

use crate::equaliser::DvbtEqualiser;
use crate::fft::DvbtFft;
use crate::finder::DvbtFinder;
use crate::items::Link;
use crate::sync::DvbtSync;
use crate::tpsdec::DvbtTps;
use crate::typed::DvbshFields;
use gen::OfdmGenerator;
use ofdm::{OfdmParams, Width};
use rx::{ShData, ShDecode, ShInput, ShLink};
use tdmgen::{TdmGenerator, TdmParams};
use tdmrx::{TdmDeint, TdmDemod, TdmFront};
use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Choice;
use sdrkit_core::{
    AutoGain, ConfigError, CostTable, GenConfig, Generator, Receiver, Results, SettingSet,
    StackConfig, StackDescriptor, StageError, Tuning, Values,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph, Stage};
use sdrkit_proto_mpegts::layer::{self, TsLayer};
use settings::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dvbsh";
/// Input samples per graph pass.
pub const PASS: usize = 8192;
/// The least input rate: the OFDM rate of the 1.7 MHz channel, 64/35 MHz.
pub const MIN_RATE: f64 = 64e6 / 35.0;

/// What the stack decodes and which of its values no public document confirms.
pub const NOTE: &str = "Decodes the OFDM form SH-A and the TDM form SH-B to the transport stream, one form at a time \
    (setting waveform). SH-B: QPSK, 8PSK and 16APSK, roll-off 0.15, 0.25 and 0.35, at the symbol rates of table 5.13 (TDM and \
    OFDM of the same bandwidth) or at a set rate (setting tdm_symbol_rate); the frame width is read from the signalling \
    field. One stream of hierarchical 16-QAM at a time (setting low_priority). Not decoded: a TDM symbol rate of unequal TDM \
    and OFDM bandwidths unless it is set, the low-latency extension (annex B; the signalling field says if it is present), \
    the SH-IP packet as such (annex A; it passes as a transport packet), the combining of TDM and OFDM copies of a code word \
    and of two receive paths. Not confirmed by a public document for SH-B: the code words of a TDM frame (the most whole code \
    words after the header), the unit of the slice offset of its time interleaver (the DATA part in interleaving units over \
    48), the scrambling code, which no field signals (setting tdm_gold, default 0), a signalling field refused when the byte \
    after its version is not 0, the order of the automatic waveform (the OFDM form on every usable width, then the TDM form \
    for 1.3 s per width) and of the symbol rates (0.1 s without slots, 0.7 s without a frame), the slot search over 8 slots in \
    blocks of 20 symbols (carrier offsets up to 1/40 of the symbol rate), and the 2 s that the slot and frame grids run on \
    without pilots. Not confirmed by a public document for both forms: the order of the copies of a repeated tail symbol (sent one after another), the search order of the \
    automatic width (5, 8, 7, 6, 1.7 MHz) and its times (0.3 s without OFDM symbols, 3 s without TPS), the soft values stored \
    in the time de-interleaver (8 bits in steps of 1/4), the most turbo iterations (setting turbo_iterations, default 8) and \
    the de-interleaver memory limit (setting max_interleaver_mb, default 768 MB), and the gap of up to 2 s in the symbols that \
    the data path fills with erasures. The turbo decoder is max-log-MAP with a \
    scaled extrinsic. The first and last units of the SH frame are counted on one clock at the input and the output of the \
    time interleaver.";

/// The recorded cost of the stages on the test signal, in the cost file of the family.
const COSTS: &str = include_str!("../../cost.txt");

/// The receiver graph at its defaults. The input feeds one form at a time. SH-A: the DVB-T components in their DVB-SH
/// reading share one control link; whole symbols cross into the stage `demod` (equaliser, demapper and time
/// de-interleaver). SH-B: the matched filter, the symbol timing, the slots, the carrier and the demapper are in the first
/// stage, because the carrier and the slots steer the mixer and the symbol clock; the soft values of the DATA part cross
/// into the stage `demod` (time de-interleaver). Whole code words of either form cross into the stage `decode`, whole
/// transport packets into the stage `ts`.
pub fn builder() -> GraphBuilder {
    graph("-")
}

/// The cost variant of the automatic waveform, which is not in the cost file: the higher cost of each stage.
const COST_AUTO: &str = "auto";

/// The cost variant of the receiver with the settings `v`: that of the waveform set.
fn cost_variant(v: &Values) -> &'static str {
    match v.get(&WAVEFORM) {
        WaveformChoice::Tdm => COST_TDM,
        WaveformChoice::Ofdm => "-",
        WaveformChoice::Auto => COST_AUTO,
    }
}

/// The cost file with the rows of [`COST_AUTO`] added: for each stage the higher of the OFDM and the TDM row.
fn cost_table() -> CostTable {
    let file = CostTable::parse(COSTS);
    let mut text = String::from(COSTS);
    for r in file.rows().iter().filter(|r| r.stack == ID && r.variant == "-") {
        let tdm = file.units(ID, COST_TDM, &r.stage).unwrap_or(0.0);
        text.push_str(&format!("{ID} {COST_AUTO} {} {}\n", r.stage, r.units.max(tdm)));
    }
    CostTable::parse(&text)
}

/// [`builder`] with the recorded cost of the cost variant `variant`.
fn graph(variant: &str) -> GraphBuilder {
    let l = Link::new();
    l.sh.store(true, Ordering::Relaxed);
    let sh = Arc::new(ShLink::default());
    let g = GraphBuilder::new()
        .add("front", ShInput::new(Arc::clone(&l), Arc::clone(&sh)))
        .add("finder", DvbtFinder::new(Arc::clone(&l)))
        .add("sync", DvbtSync::new(Arc::clone(&l)))
        .add("fft", DvbtFft::with_link(Arc::clone(&l)))
        .add("tps", DvbtTps::new(Arc::clone(&l)))
        .add("eq", DvbtEqualiser::with_link(Arc::clone(&l)))
        .add("data", ShData::new(Arc::clone(&l)))
        .add("tdmfront", TdmFront::new(Arc::clone(&sh)))
        .add("tdm", TdmDemod::new(Arc::clone(&sh)))
        .add("tdmdeint", TdmDeint::new())
        .add("decode", ShDecode::new(l, sh))
        .input("front")
        .connect("front.out", "finder.in")
        .connect("finder.out", "sync.in")
        .connect("sync.out", "fft.in")
        .connect("fft.out", "tps.in")
        .connect("tps.out", "eq.in")
        .connect("eq.out", "data.in")
        .connect("data.out", "decode.in")
        .connect("data.state", "decode.state")
        .connect("front.tdm", "tdmfront.in")
        .connect("tdmfront.out", "tdm.in")
        .connect("tdm.out", "tdmdeint.in")
        .connect("tdm.state", "tdmdeint.state")
        .connect("tdmdeint.out", "decode.tdm")
        .connect("tdmdeint.state", "decode.tdm_state");
    layer::attach(g, "decode.out", &TsLayer::basic())
        .status_from("decode")
        .reconfigure_on_retune()
        .first_stage("ofdm")
        .stage("tps", "eq", Stage::new("demod"))
        .stage("tdm", "tdmdeint", Stage::new("demod"))
        .stage("data", "decode", Stage::new("decode"))
        .stage("tdmdeint", "decode", Stage::new("decode"))
        .stage("decode", layer::DEMUX, Stage::new("ts").ahead_bytes(1 << 20))
        .stage("decode", layer::TS_MEDIA, Stage::new("ts").ahead_bytes(1 << 20))
        .costs(&cost_table(), ID, variant)
}

/// Where each stack setting goes. A setting with no target is read by the stack itself.
const ROUTES: &[(&str, &[&str])] = &[
    ("width", &["front.width", "tdmfront.width"]),
    ("waveform", &["front.waveform"]),
    ("tdm_symbol_rate", &["tdmfront.tdm_symbol_rate"]),
    ("tdm_gold", &["tdm.tdm_gold"]),
    ("low_priority", &["data.low_priority"]),
    ("turbo_iterations", &["decode.turbo_iterations"]),
    (
        "max_interleaver_mb",
        &["data.max_interleaver_mb", "tdm.max_interleaver_mb"],
    ),
    layer::DECODE_VIDEO_ROUTE,
];

/// The DVB-SH receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = graph(cost_variant(&cfg.settings))
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    g.set_max_items(1 << 16);
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .fixed_passes(PASS)
        .min_rate(MIN_RATE, "DVB-SH needs a sample rate of at least 1.83 Msps");
    for (key, targets) in ROUTES {
        b = b.route(key, targets)?;
    }
    b.default_centre(DVBSH.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The parameters and width of the test signal from the `gen_*` settings.
pub fn test_params(v: &Values) -> (OfdmParams, Width) {
    let width = v.get(&GEN_WIDTH).width().unwrap_or(Width::W5);
    let qam = v.get(&GEN_CONSTELLATION) == GenConstellation::Qam16;
    let p = OfdmParams {
        mode: v.get(&GEN_MODE).mode(),
        guard: v.get(&GEN_GUARD),
        constellation: if qam {
            crate::params::Constellation::Qam16
        } else {
            crate::params::Constellation::Qpsk
        },
        hierarchy: if qam {
            v.get(&GEN_HIERARCHY)
        } else {
            crate::params::Hierarchy::None
        },
        hp: v.get(&GEN_RATE).index() as u8,
        lp: v.get(&GEN_RATE_LP).index() as u8,
        ti: ShTimeInterleaver {
            common_multiplier: v.get(&GEN_TI_CM) as u8,
            late_taps: v.get(&GEN_TI_LT) as u8,
            slices: v.get(&GEN_TI_NS) as u8,
            slice_distance: v.get(&GEN_TI_SD) as u8,
            non_late_increment: v.get(&GEN_TI_NLI) as u8,
        },
        cell_id: 0,
    };
    (p, width)
}

/// The parameters of the TDM test signal from the `gen_*` settings; `None` when the test signal is the OFDM form. The
/// frame has the length of the OFDM SH frame of the width, the guard and the constellation set.
pub fn test_params_tdm(v: &Values) -> Option<TdmParams> {
    let modulation = v.get(&GEN_WAVEFORM).modulation()?;
    let (p, width) = test_params(v);
    Some(TdmParams {
        modulation,
        rolloff: v.get(&GEN_ROLLOFF),
        width,
        guard: p.guard,
        ofdm_qam16: p.constellation == crate::params::Constellation::Qam16,
        id: p.hp,
        ti: p.ti,
        gold: v.get(&TDM_GOLD) as u32,
    })
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    if let Some(p) = test_params_tdm(&cfg.settings) {
        let g = TdmGenerator::new(p, Some(cfg.rate_hz))
            .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
        return Ok(Box::new(g));
    }
    let (p, width) = test_params(&cfg.settings);
    let g = OfdmGenerator::new(p, width, Some(cfg.rate_hz))
        .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

/// The DVB-SH stack.
pub static DVBSH: StackDescriptor = StackDescriptor {
    id: ID,
    name: "DVB-SH",
    group: "TV",
    blurb: "DVB-SH mobile TV to handhelds (ETSI EN 302 583): the OFDM form (1K to 8K, QPSK and 16-QAM, 1.7 to 8 MHz) and the TDM form (QPSK, 8PSK, 16APSK), turbo coded with a long time interleaver.",
    tuning: Tuning {
        min_hz: 400e6,
        max_hz: 3000e6,
        ..Tuning::anywhere(2177.5e6, 8e6, MIN_RATE, 5e6)
    },
    submodes: &[],
    settings: DvbshSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::LOCK_PROFILE,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(benchmarks),
};

static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The OFDM test signal 3 kHz off, long enough for TPS, the SH frame and the time interleaver: code words decode and the
/// transport stream flows.
fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, OFDM 2K QPSK 1/3", o, 3e3);
    c.seconds = c.seconds.max(3.0);
    c.check = Some(check_decoding);
    // The TDM form in 8PSK 1/2, found by the automatic waveform after the OFDM widths.
    let mut t = VerifyCase::new("test signal, TDM 8PSK 1/2", o, 3e3);
    t.seconds = t.seconds.max(4.0);
    t.settings = vec![
        CaseSetting::new(&GEN_WAVEFORM, GenWaveform::Tdm8psk),
        CaseSetting::new(&GEN_RATE, Pattern::P8),
    ];
    t.check = Some(check_decoding);
    vec![c, t]
}

/// Code words decode and the transport stream flows.
fn check_decoding(r: &Results) -> Vec<CheckLine> {
    let mut v = at_least(r, &DvbshFields::CODEWORDS_OK, 20);
    v.extend(at_least(r, &DvbshFields::LOCK, 4));
    v
}

/// A count of the results at least `min`.
fn at_least(r: &Results, f: &Column<field::I64>, min: i64) -> Vec<CheckLine> {
    let key = f.key;
    match r.fields().at(f) {
        Some(v) => vec![CheckLine::new(
            key,
            v >= min,
            format!("{key} {v}, at least {min}"),
        )],
        None => vec![CheckLine::new(key, false, format!("{key} has no value"))],
    }
}

/// The benchmarks of the stack: the receiver on its OFDM test signal (5 MHz, 2K QPSK 1/3, guard 1/8) and on its TDM test
/// signal (5 MHz, 8PSK 1/2, roll-off 0.25, 35/9 Msym/s, the waveform set to TDM), each at 8 Msps and 20 dB SNR (items: input
/// samples; the CPU share is of one core by the clock).
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "dvbsh.receiver_5m_2k_qpsk_13",
            run: bench_receiver,
        },
        sdrkit_core::Benchmark {
            name: "dvbsh.receiver_tdm_5m_8psk_12",
            run: bench_receiver_tdm,
        },
    ]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::BenchResult {
    bench(seconds, Values::defaults(&DvbshSetting::schema()))
}

/// The cost variant of the TDM form: the test signal in 8PSK 1/2 with the waveform set to TDM.
pub const COST_TDM: &str = "tdm";

/// The key and the choice index of each setting that the cost variant [`COST_TDM`] and the TDM benchmark set.
pub fn cost_tdm_choices() -> [(&'static str, u16); 3] {
    [
        (GEN_WAVEFORM.field.key, GenWaveform::Tdm8psk.index()),
        (GEN_RATE.field.key, Pattern::P8.index()),
        (WAVEFORM.field.key, WaveformChoice::Tdm.index()),
    ]
}

fn bench_receiver_tdm(seconds: f64) -> sdrkit_core::BenchResult {
    let schema = DvbshSetting::schema();
    let mut v = Values::defaults(&schema);
    for (key, index) in cost_tdm_choices() {
        if let Ok(id) = schema.id(key) {
            // The options are those of the settings.
            let _ = v.set(id, &sdrkit_core::Value::Choice(index));
        }
    }
    bench(seconds, v)
}

/// Runs the receiver of the settings `v` on 2 s of the test signal of `v`.
fn bench(seconds: f64, v: Values) -> sdrkit_core::BenchResult {
    let rate = 8e6;
    let Ok(mut g) = make_generator(&GenConfig::new(rate, v.clone())) else {
        return sdrkit_core::BenchResult::default();
    };
    let n = (2.0 * rate) as usize;
    let mut x = vec![sdrkit_core::Complex32::default(); n];
    g.generate(&mut x);
    if let Ok(mut a) = sdrkit_algo_sim::noise::Awgn::with_power(1e-2, 0x5348) {
        a.process(&mut x);
    }
    let cfg = StackConfig::new(v);
    let x = Arc::new(x);
    sdrkit_core::bench::measure(seconds, n, n as f64 / rate, || {
        let Ok(mut rx) = receiver(&cfg) else {
            return;
        };
        let input = sdrkit_core::InputInfo::new(rate, 2177.5e6, 2177.5e6).recording();
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
