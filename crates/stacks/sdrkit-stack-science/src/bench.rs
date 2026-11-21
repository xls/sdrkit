//! Benchmarks of the crate's stacks on their own test signals, for `sdrkit bench` (see [`sdrkit_core::bench`]).
//! `items_per_second` counts input samples. `cpu_fraction_of_realtime` is for the input rate in the name.

use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, SettingId, StackConfig,
    StackDescriptor, Value, Values,
};

/// Runs the receiver of `d` with `settings` over `signal` at `rate`, tuned to `centre`, for `seconds` of processor time.
pub fn run(
    d: &StackDescriptor,
    rate: f64,
    centre: f64,
    seconds: f64,
    signal: &[Complex32],
    settings: &[(SettingId, Value)],
) -> BenchResult {
    let mut values = Values::defaults(&(d.settings)());
    for (id, v) in settings {
        if values.set(*id, v).is_err() {
            return BenchResult::default();
        }
    }
    let cfg = StackConfig::new(values);
    let Ok(mut rx) = (d.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    let sig_s = signal.len() as f64 / rate;
    measure(seconds, signal.len(), sig_s, || {
        for b in signal.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// `secs` seconds of the test signal of `d` at `rate` with the default settings and `settings`.
pub fn gen(
    d: &StackDescriptor,
    rate: f64,
    secs: f64,
    settings: &[(SettingId, Value)],
) -> Vec<Complex32> {
    let mut values = Values::defaults(&(d.settings)());
    for (id, v) in settings {
        let _ = values.set(*id, v);
    }
    let cfg = GenConfig::new(rate, values).with_seed(1);
    let Some(Ok(mut g)) = d.make_generator.map(|make| make(&cfg)) else {
        return Vec::new();
    };
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    x
}

/// SuperSID at 250 ksps: the front end to 24 ksps and the four carriers of the test signal.
fn supersid_250k(s: f64) -> BenchResult {
    use crate::supersid::{self as sid, SupersidSetting};
    use sdrkit_core::SettingSet;
    let list = (
        SupersidSetting::Transmitters.id(),
        Value::Text(sid::test_transmitters(sid::TEST_DIAL_HZ)),
    );
    let x = gen(&sid::DESCRIPTOR, 250e3, 4.0, &[]);
    run(&sid::DESCRIPTOR, 250e3, sid::TEST_DIAL_HZ, s, &x, &[list])
}

/// Sferics at 2 Msps: the impulse detector over the strokes of the test signal.
fn sferics_2000k(s: f64) -> BenchResult {
    use crate::sferics as sf;
    let x = gen(&sf::DESCRIPTOR, 2e6, 2.0, &[]);
    run(&sf::DESCRIPTOR, 2e6, sf::TEST_DIAL_HZ, s, &x, &[])
}

/// Whistlers at 48 ksps: the spectrogram, the tweek rules and the path search over the test signal (a whistler and a tweek).
fn whistler_48k(s: f64) -> BenchResult {
    use crate::whistler as w;
    let x = gen(&w::DESCRIPTOR, 48e3, 10.0, &[]);
    run(&w::DESCRIPTOR, 48e3, 0.0, s, &x, &[])
}

/// Schumann resonances at 48 ksps: the front end to 200 sps, the spectra, the baseline and the transient groups.
fn schumann_48k(s: f64) -> BenchResult {
    use crate::schumann as sch;
    let x = gen(&sch::DESCRIPTOR, 48e3, 30.0, &[]);
    run(&sch::DESCRIPTOR, 48e3, 0.0, s, &x, &[])
}

/// Lightning position at 1 Msps: the notch, the sferic timer, the MSK timer of the timing transmitter and the clocks, over
/// the strikes and the transmitter of the test signal.
fn lightning_tdoa_1000k(s: f64) -> BenchResult {
    use crate::lightning::{self as lt, TdoaSetting as L};
    use sdrkit_core::SettingSet;
    let timing = [
        (L::GenTimingTx.id(), Value::Bool(true)),
        (
            L::TimingHz.id(),
            Value::Hz(lt::TEST_DIAL_HZ + lt::GEN_TIMING_OFFSET_HZ.default),
        ),
        (L::TimingLat.id(), Value::F64(lt::gen::TIMING_TX.0)),
        (L::TimingLon.id(), Value::F64(lt::gen::TIMING_TX.1)),
    ];
    let x = gen(&lt::DESCRIPTOR, 1e6, 6.0, &timing);
    run(&lt::DESCRIPTOR, 1e6, lt::TEST_DIAL_HZ, s, &x, &timing)
}

/// Pulsar folding at 2.4 Msps: the filter bank of 64 channels and, every 15 s or more of signal after the first 20 s, the
/// search of the rows held over 9 DMs and 21 periods, over the test signal fed again and again.
fn pulsar_2400k(s: f64) -> BenchResult {
    use crate::pulsar as pl;
    let x = gen(&pl::DESCRIPTOR, 2.4e6, 4.0, &[]);
    run(&pl::DESCRIPTOR, 2.4e6, pl::gen::CENTRE_HZ, s, &x, &[])
}

/// Pulse radar detection at 10 Msps: the pulse detector on every sample, the interval histogram and the beam crossings, over 1.5 s
/// of the marine test signal at 48 rpm (a crossing every 1.25 s).
fn radar_marine_10000k(s: f64) -> BenchResult {
    use crate::radar::{self as rd, RadarSetting};
    use sdrkit_core::SettingSet;
    let fast = [(RadarSetting::GenRpm.id(), Value::F64(48.0))];
    let x = gen(&rd::DESCRIPTOR, 10e6, 1.5, &fast);
    run(&rd::DESCRIPTOR, 10e6, rd::DESCRIPTOR.tuning.default_hz, s, &x, &[])
}

/// Radio Jove at 2 Msps: the 64-point spectrogram in 0.1 s rows and the burst rules, over 8 s of the test signal.
fn jove_2000k(s: f64) -> BenchResult {
    use crate::bursts as b;
    let x = gen(&b::JOVE, 2e6, 8.0, &[]);
    run(&b::JOVE, 2e6, b::JOVE.tuning.default_hz, s, &x, &[])
}

/// The solar burst spectrogram at 2 Msps: the 32-point spectrogram in 0.25 s rows, the burst rules and the drift fits, over
/// 16 s of the test signal.
fn callisto_2000k(s: f64) -> BenchResult {
    use crate::bursts as b;
    let x = gen(&b::CALLISTO, 2e6, 16.0, &[]);
    run(&b::CALLISTO, 2e6, b::CALLISTO.tuning.default_hz, s, &x, &[])
}

/// The riometer at 2 Msps: blocks of 39 transforms of 1024 points and their median, over 4 s of the test signal.
fn riometer_2000k(s: f64) -> BenchResult {
    use crate::riometer as r;
    let x = gen(&r::DESCRIPTOR, 2e6, 4.0, &[]);
    run(&r::DESCRIPTOR, 2e6, r::DESCRIPTOR.tuning.default_hz, s, &x, &[])
}

/// The moon echo test at 250 ksps: the front end to 2 ksps, the rows and the echo searches over 16 s of the test signal.
fn eme_echo_250k(s: f64) -> BenchResult {
    use crate::eme as e;
    let x = gen(&e::DESCRIPTOR, 250e3, 16.0, &[]);
    run(&e::DESCRIPTOR, 250e3, e::DESCRIPTOR.tuning.default_hz, s, &x, &[])
}

/// Beacon openings at 250 ksps: the front end to 2 ksps, the rows and their scan over 30 s of the test signal.
fn beacon_events_250k(s: f64) -> BenchResult {
    use crate::beacon as b;
    let x = gen(&b::DESCRIPTOR, 250e3, 30.0, &[]);
    run(&b::DESCRIPTOR, 250e3, b::DESCRIPTOR.tuning.default_hz, s, &x, &[])
}

/// The crate's benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "science.supersid_250k",
            run: supersid_250k,
        },
        Benchmark {
            name: "science.sferics_2000k",
            run: sferics_2000k,
        },
        Benchmark {
            name: "science.whistler_48k",
            run: whistler_48k,
        },
        Benchmark {
            name: "science.schumann_48k",
            run: schumann_48k,
        },
        Benchmark {
            name: "science.lightning_tdoa_1000k",
            run: lightning_tdoa_1000k,
        },
        Benchmark {
            name: "science.pulsar_2400k",
            run: pulsar_2400k,
        },
        Benchmark {
            name: "science.radar_marine_10000k",
            run: radar_marine_10000k,
        },
        Benchmark {
            name: "science.jove_2000k",
            run: jove_2000k,
        },
        Benchmark {
            name: "science.callisto_2000k",
            run: callisto_2000k,
        },
        Benchmark {
            name: "science.riometer_2000k",
            run: riometer_2000k,
        },
        Benchmark {
            name: "science.eme_echo_250k",
            run: eme_echo_250k,
        },
        Benchmark {
            name: "science.beacon_events_250k",
            run: beacon_events_250k,
        },
    ]
}
