//! Benchmarks of each stack's receiver on its own test signal, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]).
//! `items_per_second` counts input samples. `cpu_fraction_of_realtime` is for the input rate in the name.

use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Choice, Complex32, GenConfig, Generator, InputInfo, Output, SettingId,
    SettingSet, StackConfig, StackDescriptor, Value, Values,
};

fn run(
    d: &StackDescriptor,
    rate: f64,
    centre: f64,
    tuned: f64,
    seconds: f64,
    signal: Vec<Complex32>,
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
    let _ = rx.configure(&InputInfo::new(rate, centre, tuned).recording());
    let mut out = Output::default();
    let sig_s = signal.len() as f64 / rate;
    measure(seconds, signal.len(), sig_s, || {
        for b in signal.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

fn gen(d: &StackDescriptor, rate: f64, secs: f64) -> Vec<Complex32> {
    let cfg = GenConfig::new(rate, Values::defaults(&(d.settings)())).with_seed(1);
    let Some(Ok(mut g)) = d.make_generator.map(|make| make(&cfg)) else {
        return Vec::new();
    };
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    x
}

/// Adds white noise for a signal of mean power `power` at `snr_db` in `bandwidth_hz`.
fn noisy(x: &mut [Complex32], power: f64, snr_db: f64, bandwidth_hz: f64, rate: f64) {
    use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
    let level = NoiseLevel::SnrDb {
        snr_db,
        bandwidth_hz,
    };
    if let Ok(mut n) = Awgn::complex(level, power, rate, 1) {
        n.process(x);
    }
}

fn acars(s: f64) -> BenchResult {
    let p = crate::acars::GenParams {
        rate_factor: 3.0,
        channels: vec![25e3, 225e3, 325e3],
        ..Default::default()
    };
    let mut g = crate::acars::AcarsGenerator::new(p);
    let mut x = vec![Complex32::default(); 4_000_000];
    g.generate(&mut x);
    noisy(&mut x, g.carrier_power(), 35.0, 7e3, 2e6);
    run(&crate::acars::DESCRIPTOR, 2e6, 131.5e6, 131.5e6, s, x, &[])
}

fn vdl2(s: f64) -> BenchResult {
    let p = crate::vdl2::GenParams {
        rate_hz: 2e6,
        rate_factor: 2.5,
        aircraft: 8,
        channels: vec![-125e3, -75e3, -25e3, 25e3, 50e3, 125e3],
        ..Default::default()
    };
    let mut g = crate::vdl2::Vdl2Generator::new(p);
    let mut x = vec![Complex32::default(); 4_000_000];
    g.generate(&mut x);
    noisy(
        &mut x,
        crate::vdl2::generator::burst_power(),
        25.0,
        10.5e3,
        2e6,
    );
    // Eight channels watched, six of them carrying the signal.
    let watched: Vec<String> = [-175e3, -125e3, -75e3, -25e3, 0.0, 25e3, 50e3, 125e3]
        .iter()
        .map(|o| format!("{:.3}", (136.85e6 + o) / 1e6))
        .collect();
    run(
        &crate::vdl2::DESCRIPTOR,
        2e6,
        136.85e6,
        136.85e6,
        s,
        x,
        &[(
            crate::vdl2::Vdl2Setting::Channels.id(),
            Value::Text(watched.join(" ")),
        )],
    )
}

fn vdl4(s: f64) -> BenchResult {
    // Six channels watched and carrying the signal.
    let offsets = [-125e3, -75e3, -25e3, 25e3, 50e3, 125e3];
    let mut g = crate::vdl4::Vdl4Generator::new(crate::vdl4::GenParams {
        rate_hz: 2e6,
        channels: offsets.to_vec(),
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); 4_000_000];
    g.generate(&mut x);
    let watched: Vec<String> = offsets
        .iter()
        .map(|o| format!("{:.3}", (136.85e6 + o) / 1e6))
        .collect();
    run(
        &crate::vdl4::DESCRIPTOR,
        2e6,
        136.85e6,
        136.85e6,
        s,
        x,
        &[(
            crate::vdl4::Vdl4Setting::Channels.id(),
            Value::Text(watched.join(" ")),
        )],
    )
}

fn hfdl(s: f64) -> BenchResult {
    let p = crate::hfdl::GenParams {
        rate_hz: 48e3,
        centre_hz: 8.92e6,
        rate_factor: 2.0,
        aircraft: 4,
        duration_s: 20.0,
        ..Default::default()
    };
    let mut g = crate::hfdl::HfdlGenerator::new(p);
    let mut x = vec![Complex32::default(); 48_000 * 10];
    g.generate(&mut x);
    run(&crate::hfdl::DESCRIPTOR, 48e3, 8.92e6, 8.92e6, s, x, &[])
}

fn hfdl_2m(s: f64) -> BenchResult {
    let p = crate::hfdl::GenParams {
        rate_hz: 2e6,
        centre_hz: 8.9e6,
        rate_factor: 2.0,
        aircraft: 4,
        duration_s: 4.0,
        ..Default::default()
    };
    let mut g = crate::hfdl::HfdlGenerator::new(p);
    let mut x = vec![Complex32::default(); 2_000_000];
    g.generate(&mut x);
    run(&crate::hfdl::DESCRIPTOR, 2e6, 8.9e6, 8.9e6, s, x, &[])
}

fn hfdl_32m(s: f64) -> BenchResult {
    let p = crate::hfdl::GenParams {
        rate_hz: 32e6,
        centre_hz: 12e6,
        bursts: crate::hfdl::generator::test_pair(0.1, 11.327e6, 2),
        ..Default::default()
    };
    let mut g = crate::hfdl::HfdlGenerator::new(p);
    let mut x = vec![Complex32::default(); 16_000_000];
    g.generate(&mut x);
    let list: Vec<String> = crate::hfdl::stations::all_khz()
        .iter()
        .map(|k| k.to_string())
        .collect();
    run(
        &crate::hfdl::DESCRIPTOR,
        32e6,
        12e6,
        12e6,
        s,
        x,
        &[(
            crate::hfdl::HfdlSetting::Channels.id(),
            Value::Text(list.join(" ")),
        )],
    )
}

fn airband(s: f64) -> BenchResult {
    let x = gen(&crate::airband::DESCRIPTOR, 1e6, 2.0);
    let c = 118.7e6 + crate::airband::TUNE_OFFSET_HZ;
    let cfg = crate::airband::bank::test_layout(118.7e6);
    let list = crate::airband::names::list_to_text(&cfg);
    run(
        &crate::airband::DESCRIPTOR,
        1e6,
        c,
        118.7e6,
        s,
        x,
        &[(
            crate::airband::AirbandSetting::Channels.id(),
            Value::Text(list),
        )],
    )
}

fn selcal(s: f64) -> BenchResult {
    let x = gen(&crate::selcal::DESCRIPTOR, 2e6, 2.0);
    run(
        &crate::selcal::DESCRIPTOR,
        2e6,
        8.941e6,
        8.891e6,
        s,
        x,
        &[(
            crate::selcal::SelcalSetting::Demod.id(),
            Value::Choice(crate::selcal::DemodChoice::Fm.index()),
        )],
    )
}

fn vdl3(s: f64) -> BenchResult {
    let x = gen(&crate::vdl3::DESCRIPTOR, 1e6, 2.0);
    let f = crate::vdl3::TEST_SIGNAL_HZ;
    run(
        &crate::vdl3::DESCRIPTOR,
        1e6,
        f + crate::vdl3::TUNE_OFFSET_HZ,
        f,
        s,
        x,
        &[],
    )
}

fn gbas_vdb(s: f64) -> BenchResult {
    let x = gen(&crate::gbas_vdb::DESCRIPTOR, 250e3, 4.0);
    let f = crate::gbas_vdb::TEST_SIGNAL_HZ;
    run(&crate::gbas_vdb::DESCRIPTOR, 250e3, f, f, s, x, &[])
}

fn ldacs(s: f64) -> BenchResult {
    let x = gen(&crate::ldacs::DESCRIPTOR, 2e6, 1.0);
    let f = crate::ldacs::TEST_SIGNAL_HZ;
    run(&crate::ldacs::DESCRIPTOR, 2e6, f, f, s, x, &[])
}

fn aeromacs(s: f64) -> BenchResult {
    let x = gen(&crate::aeromacs::DESCRIPTOR, 5.6e6, 0.5);
    let f = crate::aeromacs::TEST_SIGNAL_HZ;
    run(&crate::aeromacs::DESCRIPTOR, 5.6e6, f, f, s, x, &[])
}

/// The crate's benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "avdata.acars_2000k_3ch",
            run: acars,
        },
        Benchmark {
            name: "avdata.vdl2_2000k_8ch",
            run: vdl2,
        },
        Benchmark {
            name: "avdata.vdl4_2000k_6ch",
            run: vdl4,
        },
        Benchmark {
            name: "avdata.hfdl_48k_1ch",
            run: hfdl,
        },
        Benchmark {
            name: "avdata.hfdl_2000k_table",
            run: hfdl_2m,
        },
        Benchmark {
            name: "avdata.hfdl_32000k_table",
            run: hfdl_32m,
        },
        Benchmark {
            name: "avdata.airband_1000k_5ch",
            run: airband,
        },
        Benchmark {
            name: "avdata.selcal_2000k",
            run: selcal,
        },
        Benchmark {
            name: "avdata.vdl3_1000k",
            run: vdl3,
        },
        Benchmark {
            name: "avdata.gbas_vdb_250k",
            run: gbas_vdb,
        },
        Benchmark {
            name: "avdata.ldacs_2000k",
            run: ldacs,
        },
        Benchmark {
            name: "avdata.aeromacs_5600k",
            run: aeromacs,
        },
    ]
}
