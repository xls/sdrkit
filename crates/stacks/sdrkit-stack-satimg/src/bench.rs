//! The benchmarks of this crate: the receivers of the 137 MHz, L-band, X-band and DVB-S2 stacks on their test signals.

use crate::modes;
use sdrkit_core::bench::{BenchResult, Benchmark};
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Values};

/// Runs the receiver of `desc` on `signal_s` seconds of its test signal at `rate`, for `seconds` of processor time.
fn receiver(seconds: f64, desc: &'static StackDescriptor, rate: f64, signal_s: f64) -> BenchResult {
    receiver_of(seconds, desc, None, rate, signal_s)
}

/// As [`receiver`], for the submode `submode`.
fn receiver_of(
    seconds: f64,
    desc: &'static StackDescriptor,
    submode: Option<&str>,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    receiver_noise(seconds, desc, submode, rate, signal_s, 0.0)
}

/// As [`receiver_of`], with white noise of power `noise` added to the test signal (a receiver whose carrier survey needs a
/// noise floor finds no carrier in a clean signal).
fn receiver_noise(
    seconds: f64,
    desc: &'static StackDescriptor,
    submode: Option<&str>,
    rate: f64,
    signal_s: f64,
    noise: f32,
) -> BenchResult {
    let Ok(schema) = sdrkit_components::sim::with_generator_settings(&(desc.settings)()) else {
        return BenchResult::default();
    };
    let settings = Values::defaults(&schema);
    let Some(make) = desc.make_generator else {
        return BenchResult::default();
    };
    let Ok(mut g) = make(&GenConfig::new(rate, settings.clone()).with_submode_opt(submode.map(str::to_owned)).with_seed(1)) else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    if noise > 0.0 {
        let mut rng = sdrkit_util_bits::xorshift::Xorshift64::new(7);
        let sd = (noise / 2.0).sqrt();
        let mut uniform = || ((rng.step() >> 11) as f64 / (1u64 << 53) as f64).max(1e-300);
        for z in &mut x {
            let (u, v) = (uniform(), uniform());
            let r = (-2.0 * u.ln()).sqrt() as f32 * sd;
            let a = (2.0 * std::f64::consts::PI * v) as f32;
            *z += Complex32::new(r * a.cos(), r * a.sin());
        }
    }
    let cfg = StackConfig::new(settings).with_submode_opt(submode.map(str::to_owned));
    let Ok(mut rx) = (desc.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    let _ = rx.configure(
        &InputInfo::new(
            rate,
            desc.tuning.default_hz + desc.tuning.tune_offset_hz,
            desc.tuning.default_hz,
        )
        .recording(),
    );
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks: each receiver at the rate of its stack.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "noaa_apt.receiver_1024k",
            run: |s| receiver(s, &modes::apt::DESCRIPTOR, 1.024e6, 4.0),
        },
        Benchmark {
            name: "noaa_dsb.receiver_1024k",
            run: |s| receiver(s, &modes::dsb::DESCRIPTOR, 1.024e6, 4.0),
        },
        Benchmark {
            name: "goes_gvar.receiver_6m",
            run: |s| receiver(s, &modes::gvar::DESCRIPTOR, 6e6, 1.0),
        },
        Benchmark {
            name: "aws.receiver_6m",
            run: |s| receiver(s, &modes::aws::DESCRIPTOR, 6e6, 1.0),
        },
        Benchmark {
            name: "hrpt_meteor.receiver_6m",
            run: |s| receiver_of(s, &modes::hrpt::DESCRIPTOR, Some("meteor"), 6e6, 1.0),
        },
        Benchmark {
            name: "noaa_gac.receiver_8m",
            run: |s| receiver(s, &modes::gac::DESCRIPTOR, 8e6, 1.0),
        },
        Benchmark {
            name: "msg_raw.receiver_6m",
            run: |s| receiver(s, &modes::msg::DESCRIPTOR, 6e6, 1.0),
        },
        Benchmark {
            name: "goes_cda.receiver_r40_1m",
            run: |s| receiver_of(s, &modes::goes_cda::DESCRIPTOR, Some("r40"), 1e6, 2.0),
        },
        Benchmark {
            name: "goes_cda.receiver_n_1m",
            run: |s| receiver_of(s, &modes::goes_cda::DESCRIPTOR, Some("n"), 1e6, 4.0),
        },
        Benchmark {
            name: "goes_mdl.receiver_3m",
            run: |s| receiver(s, &modes::goes_mdl::DESCRIPTOR, 3e6, 1.0),
        },
        Benchmark {
            name: "gk2a_cdas.receiver_8m",
            run: |s| receiver(s, &modes::gk2a_cdas::DESCRIPTOR, 8e6, 1.0),
        },
        Benchmark {
            name: "sband_ccsds.receiver_4m",
            run: |s| receiver(s, &modes::sband::DESCRIPTOR, 4e6, 1.0),
        },
        Benchmark {
            name: "sat_filecast.himawaricast_6m",
            run: |s| {
                receiver_noise(
                    s,
                    &modes::filecast::DESCRIPTOR,
                    Some(modes::filecast::HIMAWARICAST),
                    6e6,
                    4.0,
                    0.02,
                )
            },
        },
        Benchmark {
            name: "goes_grb.receiver_20m",
            run: |s| receiver_noise(s, &modes::grb::DESCRIPTOR, None, 20e6, 1.5, 0.02),
        },
        Benchmark {
            name: "gk2a_uhrit.receiver_30m",
            run: |s| receiver_noise(s, &modes::uhrit::DESCRIPTOR, None, 30e6, 1.0, 0.02),
        },
        Benchmark {
            name: "fy4a_hrit.receiver_2m",
            run: |s| receiver_noise(s, &modes::fy4::HRIT, None, 2e6, 2.0, 0.02),
        },
        Benchmark {
            name: "elektro_l.arktika_ggak_1024k",
            run: |s| {
                receiver_of(
                    s,
                    &modes::xrit::ELEKTRO,
                    Some(modes::xrit::ARKTIKA_SUBMODE),
                    1.024e6,
                    4.0,
                )
            },
        },
        Benchmark {
            name: "rdas.elektro_l_45m",
            run: |s| receiver_noise(s, &modes::rdas::DESCRIPTOR, Some(modes::rdas::ELEKTRO_L), 45e6, 0.2, 0.002),
        },
        Benchmark {
            name: "eodb.saral_l_coded_qpsk_3m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("saral_l"), 3e6, 1.0, 0.002),
        },
        Benchmark {
            name: "eodb.jpss_hrd_coded_qpsk_25m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("jpss_hrd"), 25e6, 0.2, 0.002),
        },
        Benchmark {
            name: "eodb.eos_terra_coded_bpsk_45m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("eos_terra"), 45e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.eos_aqua_aura_uncoded_oqpsk_15m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("eos_aqua_aura"), 15e6, 0.2, 0.002),
        },
        Benchmark {
            name: "eodb.metop_dump_uncoded_qpsk_45m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("metop_dump"), 45e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.kanopus_v_own_frames_90m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("kanopus_v"), 90e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.jpss_hrd_n21_coded_oqpsk_40m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("jpss_hrd_n21"), 40e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.metopsg_ddb_coded_qpsk_140m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("metopsg_ddb"), 140e6, 0.05, 0.002),
        },
        Benchmark {
            name: "eodb.landsat_x_ldpc_oqpsk_300m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("landsat_x"), 300e6, 0.03, 0.002),
        },
        Benchmark {
            name: "eodb.proba_v_x_coded_oqpsk_45m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("proba_v_x"), 45e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.gcom_w1_coded_oqpsk_20m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("gcom_w1"), 20e6, 0.2, 0.002),
        },
        Benchmark {
            name: "eodb.gcom_c1_uncoded_oqpsk_90m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("gcom_c1"), 90e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.oceansat2_own_frames_qpsk_35m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("oceansat2"), 35e6, 0.1, 0.002),
        },
        Benchmark {
            name: "eodb.oceansat3_x_uncoded_qpsk_120m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("oceansat3_x"), 120e6, 0.05, 0.002),
        },
        Benchmark {
            name: "eodb.saral_x_coded_qpsk_25m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("saral_x"), 25e6, 0.2, 0.002),
        },
        Benchmark {
            name: "eodb.dmsp_rtd_own_frames_2m",
            run: |s| receiver_noise(s, &modes::eodb::DESCRIPTOR, Some("dmsp_rtd"), 2e6, 1.0, 0.002),
        },
    ]
}
