//! Benchmarks of the marine receivers, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]). Each makes its test
//! signal first and times the receiver on it. `items_per_second` counts input samples. `cpu_fraction_of_realtime` is for the input
//! rate in the name.

use crate::ais::generator::{DemoOptions, PortDemo, Render};
use crate::fax::FaxDecoder;
use crate::hfgen::{self, HfGenerator, HfOptions};
use sdrkit_components::sim::{GenSetting, NoiseKind};
use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Choice, Complex32, Generator, InputInfo, Output, SettingSet,
    StackConfig, StackDescriptor, Value, Values,
};

fn signal(g: &mut dyn Generator, secs: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (secs * g.rate_hz()) as usize];
    g.generate(&mut x);
    x
}

fn run(
    d: &StackDescriptor,
    sub: Option<&str>,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    tuned: f64,
    seconds: f64,
) -> BenchResult {
    let cfg =
        StackConfig::new(Values::defaults(&(d.settings)())).with_submode_opt(sub.map(String::from));
    let mut rx = (d.make_receiver)(&cfg).expect("builds");
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("configures");
    let mut out = Output::default();
    let secs = x.len() as f64 / rate;
    measure(seconds, x.len(), secs, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// `g` with the host's impairments: noise at `snr_db` in its own bandwidth, and the other generator settings of `more`.
fn impair(g: Box<dyn Generator>, snr_db: f64, more: &[(&str, f64)]) -> Box<dyn Generator> {
    let schema = GenSetting::schema();
    let mut v = Values::defaults(&schema);
    let mut set = |key: &str, value: Value| {
        let id = schema.id(key).expect("a generator setting");
        v.set(id, &value).expect("the bench impairments are valid");
    };
    set("gen_noise", Value::Choice(NoiseKind::Snr.index()));
    set("gen_snr_db", Value::F64(snr_db));
    for (k, x) in more {
        set(k, Value::F64(*x));
    }
    sdrkit_components::sim::impaired(g, &v).expect("the bench impairments are valid")
}

/// `g` with noise at 30 dB SNR in its own bandwidth, as the host's impairments add it.
fn noisy(g: Box<dyn Generator>) -> Box<dyn Generator> {
    impair(g, 30.0, &[])
}

/// The port demo with noise at 30 dB in 48 kHz, as the host's impairments add it.
fn port_demo(render: Render) -> Box<dyn Generator> {
    let g = PortDemo::new(DemoOptions {
        render,
        ..Default::default()
    });
    impair(Box::new(g), 30.0, &[("gen_level_dbfs", -12.0)])
}

fn ais_2m(seconds: f64) -> BenchResult {
    let rate = 2e6;
    let mut g = port_demo(Render {
        rate,
        duration_s: 0.0,
        ..Default::default()
    });
    let x = signal(g.as_mut(), 1.0);
    run(&crate::AIS, None, &x, rate, 162e6, 162e6, seconds)
}

fn ais_6m(seconds: f64) -> BenchResult {
    let rate = 6e6;
    let mut g = port_demo(Render {
        rate,
        duration_s: 0.0,
        tune_offset_hz: -2.6e6,
        ..Default::default()
    });
    let x = signal(g.as_mut(), 0.5);
    run(&crate::AIS, None, &x, rate, 159.4e6, 159.4e6, seconds)
}

/// A marine HF signal moved by `offset_hz`, with noise at 30 dB in its own bandwidth, as the host's impairments add them.
fn hf_impaired(g: Box<dyn Generator>, offset_hz: f64) -> Box<dyn Generator> {
    impair(
        g,
        30.0,
        &[("gen_offset_hz", offset_hz), ("gen_level_dbfs", -12.0)],
    )
}

fn marine_navtex_2m(seconds: f64) -> BenchResult {
    let rate = 2e6;
    let Ok(g) = HfGenerator::new(
        hfgen::navtex_programme(&crate::navtex::demo_messages(), 1.0),
        HfOptions {
            rate,
            info: hfgen::navtex_info(),
            ..Default::default()
        },
    ) else {
        return BenchResult::default();
    };
    // The dial 20 kHz below the radio, as the stack tunes it, with noise at 30 dB.
    let mut g = hf_impaired(Box::new(g), -20e3);
    let x = signal(g.as_mut(), 0.5);
    run(
        &crate::MARINE,
        Some("navtex"),
        &x,
        rate,
        518e3 + 20e3,
        518e3,
        seconds,
    )
}

fn marine_auto_12k(seconds: f64) -> BenchResult {
    let rate = 12e3;
    let Ok(mut g) = HfGenerator::new(
        hfgen::navtex_programme(&crate::navtex::demo_messages(), 1.0),
        HfOptions {
            rate,
            info: hfgen::navtex_info(),
            ..Default::default()
        },
    ) else {
        return BenchResult::default();
    };
    let x = signal(&mut g, 10.0);
    run(&crate::MARINE, None, &x, rate, 0.0, 0.0, seconds)
}

fn fax_12k(seconds: f64) -> BenchResult {
    let rate = 12e3;
    let Ok(mut g) = HfGenerator::new(
        hfgen::fax_programme(hfgen::chart(1809, 40), 120.0, 576, 5.0, 1.0),
        HfOptions {
            rate,
            info: hfgen::fax_info(),
            ..Default::default()
        },
    ) else {
        return BenchResult::default();
    };
    let x = signal(&mut g, 20.0);
    let audio: Vec<f32> = x.iter().map(|c| c.re).collect();
    let secs = audio.len() as f64 / rate;
    measure(seconds, audio.len(), secs, || {
        if let Ok(mut d) = FaxDecoder::new(rate) {
            d.process(&audio);
        }
    })
}

fn vdes_1m(seconds: f64) -> BenchResult {
    let rate = 1e6;
    let mut g = crate::vdes::VdesGenerator::new(rate, 1);
    let x = signal(&mut g, 0.6);
    run(&crate::VDES, None, &x, rate, 161.9e6, 161.9e6, seconds)
}

fn vhf_2m(seconds: f64) -> BenchResult {
    let rate = 2e6;
    let o = crate::vhf::VhfGen {
        rate,
        centre_hz: 156.55e6 + 12.5e3,
        ..Default::default()
    };
    let mut g = noisy(Box::new(crate::vhf::VhfGenerator::new(o)));
    let x = signal(g.as_mut(), 4.0);
    run(
        &crate::MARINEVHF,
        None,
        &x,
        rate,
        156.5625e6,
        156.55e6,
        seconds,
    )
}

fn atis_1m(seconds: f64) -> BenchResult {
    let rate = 1e6;
    let mut g = crate::atis::AtisGenerator::new(crate::atis::AtisGen {
        rate,
        channel_hz: -150e3,
        ..Default::default()
    });
    let x = signal(&mut g, 1.0);
    run(&crate::ATIS, None, &x, rate, 156.65e6, 156.5e6, seconds)
}

/// The marine benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "marine.ais_2000k",
            run: ais_2m,
        },
        Benchmark {
            name: "marine.ais_all_6000k",
            run: ais_6m,
        },
        Benchmark {
            name: "marine.navtex_2000k",
            run: marine_navtex_2m,
        },
        Benchmark {
            name: "marine.hf_auto_12k",
            run: marine_auto_12k,
        },
        Benchmark {
            name: "marine.fax_decoder_12k",
            run: fax_12k,
        },
        Benchmark {
            name: "marine.vdes_1000k",
            run: vdes_1m,
        },
        Benchmark {
            name: "marine.marinevhf_2000k",
            run: vhf_2m,
        },
        Benchmark {
            name: "marine.atis_1000k",
            run: atis_1m,
        },
    ]
}
