//! Benchmarks of the hot paths, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]). Each makes its input first, then
//! times the work on it; `items_per_second` counts output samples (Gaussian values for `sim.gauss`), and
//! `cpu_fraction_of_realtime` is for the rate in the name.

use crate::chain::{Chain, ChainConfig, SignalInfo};
use crate::clock::ClockError;
use crate::fading::profiles::Gsm;
use crate::fading::{Profile, Tdl};
use crate::modulate::{Cpm, Linear, Ramp};
use crate::noise::{Awgn, NoiseLevel};
use crate::rng::Rng;
use sdrkit_algo_dsp::constellation::PointSet;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark, Complex32};

const PIECE: usize = 65_536;

fn tone(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            Complex32::from_polar(
                0.5,
                (std::f64::consts::TAU * ((0.013 * i as f64) % 1.0)) as f32,
            )
        })
        .collect()
}

fn gauss(seconds: f64) -> BenchResult {
    let mut r = Rng::new(1);
    let mut x = vec![0f32; PIECE];
    measure(seconds, PIECE, PIECE as f64 / 2.4e6, || {
        r.fill_normal(&mut x)
    })
}

fn awgn(seconds: f64) -> BenchResult {
    let mut n = Awgn::with_power(0.1, 1).expect("valid");
    let mut x = tone(PIECE);
    measure(seconds, PIECE, PIECE as f64 / 2.4e6, || n.process(&mut x))
}

fn fading(seconds: f64, profile: Profile, rate: f64) -> BenchResult {
    let mut t = Tdl::from_profile(&profile, rate, 1).expect("valid");
    let mut x = tone(PIECE);
    measure(seconds, PIECE, PIECE as f64 / rate, || t.process(&mut x))
}

fn clock(seconds: f64) -> BenchResult {
    let mut c = ClockError::new(37.0).expect("valid");
    let x = tone(PIECE);
    let mut out = Vec::with_capacity(2 * PIECE);
    measure(seconds, PIECE, PIECE as f64 / 2.4e6, || {
        out.clear();
        c.process(&x, &mut out);
    })
}

fn chain(seconds: f64) -> BenchResult {
    let cfg = ChainConfig {
        offset_hz: 1234.0,
        drift_hz_s: 10.0,
        clock_ppm: 20.0,
        noise: NoiseLevel::Cn0DbHz(60.0),
        fading: Profile::Rayleigh {
            max_doppler_hz: 50.0,
        },
        ..ChainConfig::default()
    };
    let mut c = Chain::new(&cfg, &SignalInfo::new(1e6)).expect("valid");
    let x = tone(PIECE);
    let mut out = Vec::with_capacity(2 * PIECE);
    measure(seconds, PIECE, PIECE as f64 / 1e6, || {
        out.clear();
        c.process(&x, &mut out);
    })
}

fn linear(seconds: f64) -> BenchResult {
    // QPSK, RRC 0.35 over 16 symbols at 8 samples per symbol (1 MBd at 8 Msps for the real-time figure).
    let mut m = Linear::rrc(0.35, 16.0, 8.0).expect("valid");
    let mut r = Rng::new(2);
    let a = std::f32::consts::FRAC_1_SQRT_2;
    let syms: Vec<Complex32> = (0..PIECE / 8)
        .map(|_| {
            Complex32::new(
                if r.bit() == 1 { a } else { -a },
                if r.bit() == 1 { a } else { -a },
            )
        })
        .collect();
    let mut out = Vec::with_capacity(2 * PIECE);
    measure(seconds, PIECE, PIECE as f64 / 8e6, || {
        out.clear();
        m.push(&syms, &mut out);
    })
}

/// QAM symbols of `order` points through an RRC 0.15 pulse over `span` symbols at `sps` samples per symbol; the real-time figure
/// is for an output rate of `rate`.
fn linear_qam(seconds: f64, order: usize, sps: f64, span: f64, rate: f64) -> BenchResult {
    let mut m = Linear::rrc(0.15, span, sps).expect("valid");
    let set = PointSet::dvb_c(order).expect("valid");
    let mut r = Rng::new(4);
    let n = (PIECE as f64 / sps) as usize;
    let syms: Vec<Complex32> = (0..n)
        .map(|_| {
            let l = (0..set.bits()).fold(0usize, |a, _| (a << 1) | r.bit() as usize);
            set.points()[l]
        })
        .collect();
    let mut out = Vec::with_capacity(2 * PIECE);
    measure(seconds, PIECE, PIECE as f64 / rate, || {
        out.clear();
        m.push(&syms, &mut out);
    })
}

fn gmsk(seconds: f64) -> BenchResult {
    // GMSK BT 0.3 at 4 samples per symbol (GSM: 270.833 kBd at 1.083 Msps).
    let mut m = Cpm::gmsk(0.3, 4.0).expect("valid");
    let mut r = Rng::new(3);
    let lv: Vec<f32> = (0..PIECE / 4)
        .map(|_| if r.bit() == 1 { 1.0 } else { -1.0 })
        .collect();
    let mut out = Vec::with_capacity(2 * PIECE);
    measure(seconds, PIECE, PIECE as f64 / 1.083e6, || {
        out.clear();
        m.push(&lv, &mut out);
    })
}

fn gfsk_burst(seconds: f64, ramp: bool) -> BenchResult {
    // FT8-like bursts: 8-GFSK BT 2 at 32 samples per symbol, 79 symbols, held edges, with or without the raised-cosine ramps
    // over 1/8 of a symbol (the real-time figure is for 6.25 Bd at 200 samples per second).
    let mut m = Cpm::gfsk(2.0, 1.0, 32.0).expect("valid").with_held_edges();
    if ramp {
        m = m.with_ramp(Ramp::RaisedCosine(4.0)).expect("valid");
    }
    let lv: Vec<f32> = (0..79).map(|i| ((i * 5) % 8) as f32 * 2.0 - 7.0).collect();
    let mut out = Vec::with_capacity(4 * 79 * 32);
    measure(seconds, 79 * 32, 79.0 * 32.0 / 200.0, || {
        out.clear();
        m.push(&lv, &mut out);
        m.flush(&mut out);
    })
}

/// The simulation benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "sim.gauss_2400k",
            run: gauss,
        },
        Benchmark {
            name: "sim.awgn_2400k",
            run: awgn,
        },
        Benchmark {
            name: "sim.fading_drm3_48k",
            run: |s| fading(s, Profile::Drm(3), 48e3),
        },
        Benchmark {
            name: "sim.fading_tu12_10m",
            run: |s| {
                fading(
                    s,
                    Profile::Gsm {
                        model: Gsm::Tu12,
                        max_doppler_hz: 100.0,
                    },
                    10e6,
                )
            },
        },
        Benchmark {
            name: "sim.fading_tu6_1m_fractional",
            run: |s| {
                fading(
                    s,
                    Profile::Gsm {
                        model: Gsm::Tu6,
                        max_doppler_hz: 100.0,
                    },
                    1e6,
                )
            },
        },
        Benchmark {
            name: "sim.clock_2400k",
            run: clock,
        },
        Benchmark {
            name: "sim.chain_1m",
            run: chain,
        },
        Benchmark {
            name: "sim.linear_rrc_8sps_8m",
            run: linear,
        },
        Benchmark {
            name: "sim.linear_qam64_4sps_8m",
            run: |s| linear_qam(s, 64, 4.0, 12.0, 8e6),
        },
        Benchmark {
            name: "sim.linear_qam64_8sps_8m",
            run: |s| linear_qam(s, 64, 8.0, 12.0, 8e6),
        },
        Benchmark {
            name: "sim.linear_qam256_4sps_8m",
            run: |s| linear_qam(s, 256, 4.0, 12.0, 8e6),
        },
        Benchmark {
            name: "sim.linear_qam256_8sps_8m",
            run: |s| linear_qam(s, 256, 8.0, 12.0, 8e6),
        },
        Benchmark {
            name: "sim.linear_qam64_dvbc_6m952_8m",
            run: |s| linear_qam(s, 64, 8e6 / 6.952e6, 24.0, 8e6),
        },
        Benchmark {
            name: "sim.gmsk_4sps_1083k",
            run: gmsk,
        },
        Benchmark {
            name: "sim.gfsk_burst_held_200",
            run: |s| gfsk_burst(s, false),
        },
        Benchmark {
            name: "sim.gfsk_burst_held_ramped_200",
            run: |s| gfsk_burst(s, true),
        },
    ]
}
