//! The recorded cost of each stage of the DVB-T stack (`cost.txt`) against the cost measured on the test signal, for
//! each variant: the mode, the constellation and the code rate of the multiplex; the same for the DVB-SH stack on its test
//! signal, and its transport packets cut at every mask against those of one thread. `SDRKIT_WRITE_COST=1` in a release
//! build writes the file.

use sdrkit_core::{ConfigError, GenConfig, Generator, SettingSet, Value};
use sdrkit_stack_dvbt::settings::DvbtSetting;
use std::path::PathBuf;

/// The variants: the heaviest one, which the graph declares, and those whose cost differs much from it.
const VARIANTS: &[&str] = &[
    sdrkit_stack_dvbt::receiver::COST_VARIANT,
    "8k-qam64-3/4",
    "8k-qam64-2/3",
    "8k-qam16-3/4",
    "8k-qpsk-1/2",
    "2k-qam64-3/4",
    "2k-qam16-2/3",
];

/// The choice index of each `gen_*` setting that the variant `v` fixes: mode, constellation and code rate.
fn choices(v: &str) -> [(&'static str, u16); 3] {
    let mut parts = v.split('-');
    let mut next = |options: &[&str]| {
        let part = parts.next().expect("three parts");
        options
            .iter()
            .position(|o| *o == part)
            .expect("a known part") as u16
    };
    [
        ("gen_mode", next(&["2k", "8k"])),
        ("gen_constellation", next(&["qpsk", "qam16", "qam64"])),
        ("gen_rate_hp", next(&["1/2", "2/3", "3/4", "5/6", "7/8"])),
    ]
}

/// The test signal of the stack with the multiplex of the variant that `cfg.submode` names.
fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let mut cfg = cfg.clone();
    let schema = DvbtSetting::schema();
    for (key, index) in choices(cfg.submode.as_deref().expect("a variant")) {
        let id = schema.id(key).expect("a setting of the generator");
        cfg.settings
            .set(id, &Value::Choice(index))
            .expect("a choice");
    }
    sdrkit_stack_dvbt::make_generator(&cfg)
}

#[test]
fn the_variant_names_select_the_multiplex() {
    use sdrkit_core::Choice;
    use sdrkit_stack_dvbt::params::{CodeRate, Constellation};
    use sdrkit_stack_dvbt::Mode;
    let c = choices("8k-qam64-7/8");
    assert_eq!(c[0].1, Mode::K8.index());
    assert_eq!(c[1].1, Constellation::Qam64.index());
    assert_eq!(c[2].1, CodeRate::R78.index());
    let c = choices("2k-qpsk-1/2");
    assert_eq!(c[0].1, Mode::K2.index());
    assert_eq!(c[1].1, Constellation::Qpsk.index());
    assert_eq!(c[2].1, CodeRate::R12.index());
}

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = sdrkit_core::StackDescriptor {
        make_generator: Some(generator),
        tuning: sdrkit_stack_dvbt::DVBT.tuning.clone(),
        ..sdrkit_stack_dvbt::DVBT
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
    iterations: Option<f64>,
}

/// Runs `seconds` of the test signal of `variant` in a channel of `width` (a choice of `bandwidth` and of
/// `gen_bandwidth`) through the stack cut at `mask`, one thread per cut, at `rate` or else at the rate that the
/// stack states for the width. `helpers` is the number of helper threads of the worker pool; `None` takes the
/// shared pool.
fn run(
    variant: &str,
    width: u16,
    rate: Option<f64>,
    seconds: f64,
    mask: u32,
    helpers: Option<usize>,
) -> Run {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig, Values};
    let schema = DvbtSetting::schema();
    let mut settings = Values::defaults(&schema);
    for key in ["bandwidth", "gen_bandwidth"] {
        let id = schema.id(key).expect("a setting");
        settings.set(id, &Value::Choice(width)).expect("a choice");
    }
    let rate = rate.unwrap_or_else(|| sdrkit_stack_dvbt::DVBT.rate_hz(&settings));
    let cfg = GenConfig::new(rate, settings.clone()).with_submode_opt(Some(variant.to_owned()));
    let mut gen = generator(&cfg).expect("a generator");
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    let mut rx =
        sdrkit_stack_dvbt::receiver::receiver(&StackConfig::new(settings)).expect("builds");
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
    let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).expect("a tap");
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
    assert_eq!(rx.staged().expect("stages").cuts(), mask);
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
        assert_eq!(l.cut, cut, "stage {} cut at {mask:#05b}", l.name);
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
        iterations: None,
    }
}

#[test]
fn the_worker_pool_gives_the_packets_of_one_thread() {
    use sdrkit_core::Choice;
    let width = sdrkit_stack_dvbt::settings::BandwidthChoice::B8.index();
    let alone = run("8k-qam64-3/4", width, None, 1.5, 0, Some(0));
    assert!(alone.packets > 5000, "{} packets", alone.packets);
    for helpers in [1, 4] {
        let shared = run("8k-qam64-3/4", width, None, 1.5, 0, Some(helpers));
        assert_eq!(
            (shared.packets, shared.digest),
            (alone.packets, alone.digest),
            "{helpers} helpers"
        );
    }
}

#[test]
#[ignore = "a measurement: run it alone in a release build"]
fn the_share_of_each_stage_for_8k_64_qam_3_4_in_7_mhz() {
    use sdrkit_core::Choice;
    use sdrkit_stack_dvbt::settings::BandwidthChoice;
    let width = BandwidthChoice::B7.index();
    let wide = run("8k-qam64-3/4", width, Some(64e6 / 7.0), 6.0, 0, None);
    eprintln!(
        "one thread at the rate of an 8 MHz channel: share per stage {:.3?}, parts {:.3?}",
        wide.stages, wide.parts
    );
    let alone = run("8k-qam64-3/4", width, None, 6.0, 0, Some(0));
    eprintln!(
        "one thread and no helper of the pool: share per stage {:.3?}, parts {:.3?}",
        alone.stages, alone.parts
    );
    let whole = run("8k-qam64-3/4", width, None, 6.0, 0, None);
    assert_eq!((alone.packets, alone.digest), (whole.packets, whole.digest));
    eprintln!(
        "one thread: 6 s of signal in {:.2} s, {} packets, share per stage {:.3?}, sum {:.3}",
        whole.took_s,
        whole.packets,
        whole.stages,
        whole.stages.iter().map(|s| s.2).sum::<f64>()
    );
    eprintln!("parts {:.3?}", whole.parts);
    assert!(whole.packets > 1000, "{} packets", whole.packets);
    for mask in 1..8 {
        let cut = run("8k-qam64-3/4", width, None, 6.0, mask, None);
        // The share of a thread is the sum of the stages that run on it.
        let highest = (0..4)
            .map(|t| cut.stages.iter().filter(|s| s.1 == t).map(|s| s.2).sum())
            .fold(0.0, f64::max);
        eprintln!(
            "cut at {mask:#05b}: 6 s of signal in {:.2} s, thread and share per stage {:.3?}, highest thread {highest:.3}",
            cut.took_s, cut.stages
        );
        assert_eq!(
            (cut.packets, cut.digest),
            (whole.packets, whole.digest),
            "cut at {mask:#05b}"
        );
    }
}

/// The output port of the DVB-SH graph that carries the transport packets.
const DVBSH_TS: &str = "decode.out";

/// The settings of the DVB-SH stack for the test signal of the cost variant `variant`: `-` is the OFDM form at the
/// defaults, `tdm` the TDM form in 8PSK 1/2 with the waveform set to TDM.
fn dvbsh_settings(variant: &str) -> sdrkit_core::Values {
    let schema = (sdrkit_stack_dvbt::dvbsh::DVBSH.settings)();
    let mut v = sdrkit_core::Values::defaults(&schema);
    if variant == sdrkit_stack_dvbt::dvbsh::COST_TDM {
        // The choices: gen_waveform TDM 8PSK, gen_rate 1/2, waveform TDM.
        for (key, index) in sdrkit_stack_dvbt::dvbsh::cost_tdm_choices() {
            let id = schema.id(key).expect("a setting");
            v.set(id, &Value::Choice(index)).expect("a choice");
        }
    }
    v
}

/// Runs `seconds` of the DVB-SH test signal of `variant` with noise through the stack cut at `mask`, one thread per
/// cut.
fn run_dvbsh(variant: &str, seconds: f64, mask: u32) -> Run {
    run_dvbsh_with(variant, seconds, mask, 0.0, None)
}

/// [`run_dvbsh`] with the signal `cfo_hz` off the centre, and with noise alone between the two seconds of `off`.
fn run_dvbsh_with(
    variant: &str,
    seconds: f64,
    mask: u32,
    cfo_hz: f64,
    off: Option<(f64, f64)>,
) -> Run {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig};
    use sdrkit_stack_dvbt::dvbsh;
    let d = &dvbsh::DVBSH;
    let rate = d.tuning.rate_hz;
    let settings = dvbsh_settings(variant);
    let mut gen = (d.make_generator.expect("a test signal"))(&GenConfig::new(rate, settings.clone()))
        .expect("a generator");
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    gen.generate(&mut x);
    if cfo_hz != 0.0 {
        let step = std::f64::consts::TAU * cfo_hz / rate;
        for (i, v) in x.iter_mut().enumerate() {
            let (s, c) = (step * i as f64).sin_cos();
            *v *= Complex32::new(c as f32, s as f32);
        }
    }
    if let Some((a, b)) = off {
        let n = x.len();
        let at = |s: f64| ((s * rate) as usize).min(n);
        x[at(a)..at(b)].fill(Complex32::default());
    }
    sdrkit_algo_sim::noise::Awgn::with_power(1e-2, 0x5348)
        .expect("noise")
        .process(&mut x);
    let mut rx = dvbsh::receiver(&StackConfig::new(settings)).expect("builds");
    rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording())
        .expect("the rate");
    let mut tap = rx.graph_mut().tap(DVBSH_TS, 1 << 14).expect("a tap");
    let threads: Vec<_> = rx
        .staged()
        .expect("stages")
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), mask.count_ones() as usize, "a thread per cut");
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
                digest = (digest ^ p.time.to_bits()).wrapping_mul(0x0100_0000_01b3);
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
    for (i, l) in loads.iter().enumerate().skip(1) {
        let cut = mask & (1 << (i - 1)) != 0;
        assert_eq!(l.cut, cut, "stage {} cut at {mask:#05b}", l.name);
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
        iterations: {
            use sdrkit_core::owner::Owner;
            let r = Owner::results(&rx);
            let key = sdrkit_stack_dvbt::typed::DvbshFields::TURBO_ITERATIONS.key;
            match r.decl().field_id(key).and_then(|id| r.fields().value(id)) {
                Some(Value::F64(v)) => Some(*v),
                _ => None,
            }
        },
    }
}

#[test]
fn the_recorded_cost_of_each_dvbsh_stage_is_the_measured_one() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let stack = &sdrkit_stack_dvbt::dvbsh::DVBSH;
    if let Err(e) = sdrkit_graph::cost::check(stack, "-", &path, 2.0, |_| {}) {
        panic!("{e}");
    }
}

/// The test signal of the DVB-SH stack for the cost variant that `cfg.submode` names.
fn dvbsh_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let settings = dvbsh_settings(cfg.submode.as_deref().unwrap_or("-"));
    let make = sdrkit_stack_dvbt::dvbsh::DVBSH.make_generator.expect("a test signal");
    make(&GenConfig::new(cfg.rate_hz, settings))
}

/// The DVB-SH stack with the settings of the cost variant that `cfg.submode` names.
fn dvbsh_receiver(
    cfg: &sdrkit_core::StackConfig,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    let settings = dvbsh_settings(cfg.submode.as_deref().unwrap_or("-"));
    (sdrkit_stack_dvbt::dvbsh::DVBSH.make_receiver)(&sdrkit_core::StackConfig::new(settings))
}

#[test]
fn the_recorded_cost_of_each_dvbsh_stage_in_the_tdm_form_is_the_measured_one() {
    use sdrkit_stack_dvbt::dvbsh::{COST_TDM, DVBSH};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    // The cost variant is a setting of the stack, not a submode: the stand-in turns the name into the settings. Its
    // rate is 489 blocks of 16384 samples per second: the measurement drops the rest of the block in work when its
    // warm-up ends, and the TDM form takes 2 s to find the slots again after samples are lost.
    let stack = sdrkit_core::StackDescriptor {
        make_generator: Some(dvbsh_generator),
        make_receiver: dvbsh_receiver,
        tuning: sdrkit_core::Tuning {
            rate_hz: 489.0 * 16_384.0,
            ..DVBSH.tuning.clone()
        },
        ..DVBSH
    };
    if let Err(e) = sdrkit_graph::cost::check(&stack, COST_TDM, &path, 2.0, |_| {}) {
        panic!("{e}");
    }
}

#[test]
fn every_cut_of_the_tdm_form_with_a_carrier_offset_gives_the_packets_of_one_thread() {
    let whole = run_dvbsh_with(sdrkit_stack_dvbt::dvbsh::COST_TDM, 3.0, 0, 40e3, None);
    assert!(whole.packets > 1000, "{} packets", whole.packets);
    for mask in 1..8 {
        let cut = run_dvbsh_with(sdrkit_stack_dvbt::dvbsh::COST_TDM, 3.0, mask, 40e3, None);
        assert_eq!(
            (cut.packets, cut.digest),
            (whole.packets, whole.digest),
            "cut at {mask:#05b}"
        );
    }
}

#[test]
fn every_dvbsh_cut_gives_the_packets_of_one_thread() {
    let whole = run_dvbsh("-", 3.0, 0);
    assert!(whole.packets > 1000, "{} packets", whole.packets);
    assert_eq!(whole.stages.len(), 4, "stages {:?}", whole.stages);
    for mask in 1..8 {
        let cut = run_dvbsh("-", 3.0, mask);
        assert_eq!(
            (cut.packets, cut.digest),
            (whole.packets, whole.digest),
            "cut at {mask:#05b}"
        );
    }
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_each_dvbsh_stage_and_the_time_of_each_cut() {
    for variant in ["-", sdrkit_stack_dvbt::dvbsh::COST_TDM] {
        share_of_each_dvbsh_stage(variant);
    }
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_each_stage_of_the_tdm_form_without_a_signal_and_over_a_gap() {
    let tdm = sdrkit_stack_dvbt::dvbsh::COST_TDM;
    for (name, off) in [("noise alone", (0.0, 3.0)), ("no signal from 1 s to 2 s", (1.0, 2.0))] {
        let r = run_dvbsh_with(tdm, 3.0, 0, 0.0, Some(off));
        eprintln!(
            "{name}: 3 s in {:.2} s, {} packets, share per stage {:.3?}, parts {:.3?}",
            r.took_s, r.packets, r.stages, r.parts
        );
    }
}

#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_each_stage_of_the_automatic_waveform_without_a_signal() {
    let r = run_dvbsh_with("-", 6.0, 0, 0.0, Some((0.0, 6.0)));
    eprintln!(
        "noise alone, automatic waveform: 6 s in {:.2} s, {} packets, share per stage {:.3?}, parts {:.3?}",
        r.took_s, r.packets, r.stages, r.parts
    );
}

#[test]
fn the_dvbsh_graph_declares_the_cost_rows_of_its_waveform() {
    use sdrkit_core::{CostTable, Receiver, StackConfig};
    use sdrkit_stack_dvbt::dvbsh::{self, COST_TDM};
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    let table = CostTable::parse(&std::fs::read_to_string(path).expect("the cost file"));
    let schema = (dvbsh::DVBSH.settings)();
    let waveform = schema.id("waveform").expect("a setting");
    // The waveform choices: automatic, OFDM, TDM. The automatic waveform declares the higher row of each stage.
    for (choice, rows) in [(0, &["-", COST_TDM][..]), (1, &["-"][..]), (2, &[COST_TDM][..])] {
        let mut v = sdrkit_core::Values::defaults(&schema);
        v.set(waveform, &Value::Choice(choice)).expect("a choice");
        let mut rx = dvbsh::receiver(&StackConfig::new(v)).expect("builds");
        for s in rx.staged().expect("stages").stages() {
            let want = rows
                .iter()
                .filter_map(|r| table.units("dvbsh", r, &s.name))
                .fold(f64::MIN, f64::max);
            assert_eq!(s.cost, Some(want), "waveform {choice}, stage {}", s.name);
        }
    }
}

fn share_of_each_dvbsh_stage(variant: &str) {
    let whole = run_dvbsh(variant, 6.0, 0);
    eprintln!(
        "{variant} one thread: 6 s of signal in {:.2} s, {} packets, digest {:#x}, {:.2?} turbo iterations, share per stage {:.3?}, parts {:.3?}",
        whole.took_s, whole.packets, whole.digest, whole.iterations, whole.stages, whole.parts
    );
    for mask in 1..1u32 << (whole.stages.len() - 1) {
        let cut = run_dvbsh(variant, 6.0, mask);
        eprintln!(
            "cut at {mask:#05b}: 6 s of signal in {:.2} s, thread and share per stage {:.3?}",
            cut.took_s, cut.stages
        );
        assert_eq!((cut.packets, cut.digest), (whole.packets, whole.digest), "cut at {mask:#05b}");
    }
}

/// A stand-in for the DVB-T2 stack with stages of its own: the DVB-SH graph, which never locks on a DVB-T signal.
fn stand_in_t2(
    _: &sdrkit_core::StackConfig,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    use sdrkit_core::{StackConfig, Values};
    let d = &sdrkit_stack_dvbt::dvbsh::DVBSH;
    let rx = sdrkit_stack_dvbt::dvbsh::receiver(&StackConfig::new(Values::defaults(&(d.settings)())))?;
    Ok(Box::new(rx))
}

/// The output of `dvb_terrestrial` on 4 s of the DVB-T test signal, its stages cut at `mask` before and after the
/// hand-over from the stand-in DVB-T2 stack to DVB-T, as the host places the stages of a stack again: the count and
/// a digest of the media packets, the stage names before and after the hand-over, and the standard at the end.
fn run_terrestrial(mask: u32) -> (usize, u64, Vec<String>, Vec<String>, sdrkit_stack_dvbt::results::Decoding) {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig, Values};
    use sdrkit_stack_dvbt::{TerrestrialReceiver, TerrestrialSetting};
    let _ = sdrkit_stack_dvbt::terrestrial::set_t2_factory(stand_in_t2);
    let rate = sdrkit_stack_dvbt::Bandwidth::B8.natural_rate();
    let settings = Values::defaults(&DvbtSetting::schema());
    let cfg = GenConfig::new(rate, settings).with_submode_opt(Some("8k-qam64-3/4".to_owned()));
    let mut gen = generator(&cfg).expect("a generator");
    let mut x = vec![Complex32::default(); (4.0 * rate) as usize];
    gen.generate(&mut x);
    let mut rx = TerrestrialReceiver::new(&StackConfig::new(Values::defaults(
        &TerrestrialSetting::schema(),
    )))
    .expect("builds");
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .expect("the rate");
    let names = |rx: &mut TerrestrialReceiver| -> Vec<String> {
        let st = rx.staged().expect("stages");
        st.stages().into_iter().map(|s| s.name).collect()
    };
    let before = names(&mut rx);
    let mut threads = Vec::new();
    let (mut out, mut all) = (Output::default(), Output::default());
    for b in x.chunks(16_384) {
        let st = rx.staged().expect("stages");
        let want = mask & ((1u32 << (st.stages().len() - 1)) - 1);
        if st.cuts() != want {
            threads.extend(
                st.cut(want)
                    .into_iter()
                    .map(|w| std::thread::spawn(move || w.run())),
            );
        }
        rx.feed(b, &mut out);
        all.append(&mut out);
    }
    let after = names(&mut rx);
    let st = rx.staged().expect("stages");
    st.drain(&mut out);
    all.append(&mut out);
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for m in &all.media {
        for &b in m.packet.bytes() {
            digest = (digest ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    (all.media.len(), digest, before, after, rx.decoding())
}

#[test]
fn dvb_terrestrial_passes_on_the_stages_of_the_standard_that_decodes_and_gives_the_output_of_one_thread() {
    use sdrkit_stack_dvbt::results::Decoding;
    let (packets, digest, before, after, decoding) = run_terrestrial(0);
    assert_eq!(decoding, Decoding::Dvbt);
    assert!(packets > 100, "{packets} media packets");
    assert_eq!(before, ["ofdm", "demod", "decode", "ts"]);
    assert_eq!(after, ["ofdm", "equalise", "fec", "ts"]);
    for mask in 1..8 {
        let cut = run_terrestrial(mask);
        assert_eq!((cut.0, cut.1, cut.4), (packets, digest, decoding), "cut at {mask:#05b}");
    }
}

