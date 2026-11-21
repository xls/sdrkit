//! Helpers of the STL tests: the STL graph, the test link with noise, runs that collect the sound and the results.
#![allow(dead_code)]

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Receiver, SettingSet, StackConfig, Value, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_fm::stl::generator::{StlGenerator, StlParams};
use sdrkit_stack_fm::stl::settings::{GenLink, StlSetting};
use sdrkit_stack_fm::stl::typed::{LinkKind, StlFields, LINKS};
use sdrkit_stack_fm::Reading;

/// The rate of most tests.
pub const RATE: f64 = 1e6;
/// The tuned frequency.
pub const TUNED_HZ: f64 = 947.5e6;

/// The STL graph at `rate` with these settings changed from the defaults.
pub fn receiver(rate: f64, settings: &[(&str, Value)]) -> StackGraph {
    let schema = StlSetting::schema();
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        let id = schema.id(k).unwrap_or_else(|e| panic!("{k}: {e}"));
        v.set(id, x).unwrap_or_else(|e| panic!("{k}: {e}"));
    }
    let mut rx = sdrkit_stack_fm::stl::receiver(&StackConfig::new(v)).expect("builds");
    rx.configure(&InputInfo::new(rate, TUNED_HZ, TUNED_HZ).recording())
        .expect("configures");
    rx
}

/// A test link of kind `link` at `rate`.
pub fn link(link: GenLink, rate: f64) -> StlParams {
    StlParams::new(link, rate)
}

/// The test link `p` with noise at `cnr_db` in 200 kHz (none from 100 dB), seed `seed`.
pub struct Source {
    g: StlGenerator,
    noise: Option<Awgn>,
}

impl Source {
    /// The source.
    pub fn new(p: StlParams, cnr_db: f64, seed: u64) -> Self {
        let rate = p.fm.rate_hz;
        let noise = (cnr_db < 100.0).then(|| noise(rate, cnr_db, seed));
        Source {
            g: StlGenerator::new(p),
            noise,
        }
    }
    /// Fills `x`.
    pub fn fill(&mut self, x: &mut [Complex32]) {
        self.g.generate(x);
        if let Some(n) = &mut self.noise {
            n.process(x);
        }
    }
}

/// Noise at `cnr_db` in 200 kHz against a carrier of unit power.
pub fn noise(rate: f64, cnr_db: f64, seed: u64) -> Awgn {
    let level = NoiseLevel::SnrDb {
        snr_db: cnr_db,
        bandwidth_hz: 200e3,
    };
    Awgn::complex(level, 1.0, rate, seed).expect("valid noise")
}

/// The sound of a run as interleaved stereo at 48 kHz.
pub struct Run {
    /// Interleaved stereo.
    pub audio: Vec<f32>,
}

impl Run {
    /// Left and right of the last `seconds`.
    pub fn tail(&self, seconds: f64) -> (Vec<f32>, Vec<f32>) {
        let frames = (seconds * 48_000.0) as usize;
        let from = (self.audio.len() / 2).saturating_sub(frames);
        sdrkit_dev_test::measure::deinterleave(&self.audio[2 * from..])
    }
}

/// Feeds `seconds` of `fill` through `rx` in pieces of `piece` samples.
pub fn run_cut(
    rx: &mut StackGraph,
    rate: f64,
    seconds: f64,
    piece: usize,
    mut fill: impl FnMut(&mut [Complex32]),
) -> Run {
    let total = (rate * seconds) as usize;
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut buf = vec![Complex32::default(); piece];
    let mut done = 0;
    while done < total {
        let n = piece.min(total - done);
        fill(&mut buf[..n]);
        out.clear();
        rx.feed(&buf[..n], &mut out);
        for p in out.audio_runs() {
            if p.channels == 1 {
                audio.extend(p.samples.iter().flat_map(|&s| [s, s]));
            } else {
                audio.extend_from_slice(p.samples);
            }
        }
        done += n;
    }
    Run { audio }
}

/// Feeds `seconds` of the source through `rx` in pieces of 0.1 s.
pub fn run(rx: &mut StackGraph, src: &mut Source, rate: f64, seconds: f64) -> Run {
    run_cut(rx, rate, seconds, (rate / 10.0) as usize, |x| src.fill(x))
}

/// The FM fields of `rx`.
pub fn reading(rx: &StackGraph) -> Reading {
    Reading::from_results(Owner::results(rx))
}

/// The kind of the link.
pub fn kind(rx: &StackGraph) -> LinkKind {
    Owner::results(rx)
        .fields()
        .at(&StlFields::LINK)
        .unwrap_or_default()
}

/// The result fields of `rx`.
pub fn fields(rx: &StackGraph) -> &sdrkit_core::Record {
    Owner::results(rx).fields()
}

/// The rows of the table of link changes.
pub fn link_rows(rx: &StackGraph) -> usize {
    let r = Owner::results(rx);
    let id = r.decl().table_id(LINKS.def().key).expect("declared");
    r.table(id).map_or(0, |t| t.len())
}

/// The SINAD of the tone at `hz` in `x` at 48 kHz, dB: the tone against everything else.
pub fn sinad_db(x: &[f32], hz: f64) -> f64 {
    let a = sdrkit_dev_test::measure::tone_amplitude(x, 48_000.0, hz);
    let total = sdrkit_dev_test::measure::rms(x).powi(2);
    let tone = a * a / 2.0;
    10.0 * (tone / (total - tone).max(1e-30)).log10()
}

/// The amplitude of the tone at `hz` in `x` at 48 kHz.
pub fn amp(x: &[f32], hz: f64) -> f64 {
    sdrkit_dev_test::measure::tone_amplitude(x, 48_000.0, hz)
}
