//! The tests of the road stacks, with the helpers they share.

mod v2x;

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::owner::Owner;
use sdrkit_core::publish::TableDef;
use sdrkit_core::{
    Complex32, GenConfig, InputInfo, Output, Receiver, Record, Results, StackConfig,
    StackDescriptor, Values,
};

/// The test signal of `d` with default settings at `rate`, `seconds` long.
pub fn signal(d: &StackDescriptor, rate: f64, seconds: f64, settings: Values) -> Vec<Complex32> {
    let mut g = (d.make_generator.expect("a generator"))(&GenConfig::new(rate, settings))
        .expect("the generator takes the rate");
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

/// Moves `x` by `hz` at `rate`.
pub fn shift(x: &mut [Complex32], hz: f64, rate: f64) {
    for (i, v) in x.iter_mut().enumerate() {
        let ph = std::f64::consts::TAU * hz * i as f64 / rate;
        *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
    }
}

/// Adds noise so that the mean power of the non-zero samples is `snr_db` over the noise in the sample rate.
pub fn noise(x: &mut [Complex32], snr_db: f64, rate: f64, seed: u64) {
    let on: Vec<f64> = x
        .iter()
        .map(|v| f64::from(v.norm_sqr()))
        .filter(|p| *p > 0.0)
        .collect();
    let power = if on.is_empty() {
        1.0
    } else {
        on.iter().sum::<f64>() / on.len() as f64
    };
    let level = NoiseLevel::SnrDb {
        snr_db,
        bandwidth_hz: rate,
    };
    Awgn::complex(level, power, rate, seed)
        .expect("valid noise")
        .process(x);
}

/// A receiver of `d` with default settings, configured for a recording at `rate` with the radio at `centre` and the signal at
/// `tuned`.
pub fn receiver(d: &StackDescriptor, rate: f64, centre: f64, tuned: f64) -> Box<dyn Receiver> {
    let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)())))
        .expect("the receiver builds");
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("the receiver takes the rate");
    rx
}

/// Feeds `x` and then 0.3 s of silence at `rate` (so that the last report comes), in pieces of 65 536 samples, and returns
/// everything that came out.
pub fn feed(rx: &mut dyn Receiver, x: &[Complex32], rate: f64) -> Output {
    let mut all = Output::default();
    let mut out = Output::default();
    let quiet = vec![Complex32::default(); (0.3 * rate) as usize];
    for c in x.chunks(65_536).chain(quiet.chunks(65_536)) {
        out.clear();
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
    }
    all
}

/// The results of a receiver.
pub fn results(rx: &dyn Receiver) -> &Results {
    Owner::results(rx)
}

/// The rows of table `def`, newest first.
pub fn rows(r: &Results, def: &TableDef) -> Vec<Record> {
    let Some(t) = r.decl().table_id(def.key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, rec)| rec.clone()).collect()
}

/// A plan file of the repository's `data/road/` folder, read as the host's plan import reads it.
pub fn plan_file(name: &str) -> sdrkit_data_plans::data::PlanFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/road")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::new();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    sdrkit_data_plans::data::parse(name, &text).expect("a plan file")
}
