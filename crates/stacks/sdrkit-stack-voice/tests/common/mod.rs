//! What the tests of the multi-carrier stacks share: a receiver with a storage, a run that keeps what came out, and the
//! samples of a scene.
#![allow(dead_code)]

use sdrkit_core::{
    Complex32, Facilities, Generator, HostRequest, InputInfo, MemStorage, Output, Receiver,
    StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_stack_voice::scene::{SceneConfig, SceneGenerator};
use std::sync::Arc;

/// The starting dial of `stack` (of its submode `submode`, if given) that a host reads from the data folder through the
/// helper library.
pub fn dial(stack: &str, submode: Option<&str>) -> f64 {
    static DIALS: std::sync::OnceLock<Vec<sdrkit_presets::default_dials::DefaultDial>> =
        std::sync::OnceLock::new();
    let rows = DIALS.get_or_init(|| {
        let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data"))
            .expect("the data folder");
        sdrkit_presets::radio_dials::dials(&data).expect("the dials")
    });
    sdrkit_presets::default_dials::dial_hz(rows, stack, submode)
        .unwrap_or_else(|| panic!("no dial for {stack} {submode:?}"))
}

/// The text of the plan file `file` of the crate.
pub fn plan_text(file: &str) -> String {
    let path = format!("{}/../../../data/voice/{file}.json", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The path of the plan file `file` in the storage of a test receiver.
pub fn stored(file: &str) -> String {
    format!("plans/{file}.json")
}

/// The receiver of the stack `desc` with the settings `set`, on the storage `storage`.
pub fn receiver_of(
    desc: &StackDescriptor,
    submode: Option<&str>,
    set: &[(&str, Value)],
    storage: Arc<MemStorage>,
) -> Box<dyn Receiver> {
    let mut v = Values::defaults(&(desc.settings)());
    for (k, val) in set {
        let id = v.schema().id(k).unwrap();
        v.set(id, val).unwrap();
    }
    let mut cfg = StackConfig::new(v);
    cfg.submode = submode.map(str::to_owned);
    let mut rx = (desc.make_receiver)(&cfg).unwrap();
    rx.set_facilities(&Facilities {
        storage,
        ..Facilities::default()
    });
    rx
}

/// What a run gave.
#[derive(Default)]
pub struct Heard {
    pub audio: Vec<f32>,
    pub requests: Vec<HostRequest>,
    pub events: Vec<sdrkit_core::Event>,
}

/// Feeds `x` at `rate` around `centre` to `rx` in chunks of `chunk`.
pub fn feed(rx: &mut dyn Receiver, x: &[Complex32], rate: f64, centre: f64, chunk: usize) -> Heard {
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    feed_on(rx, x, chunk)
}

/// Feeds `x` to a configured `rx`.
pub fn feed_on(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Heard {
    let mut out = Output::default();
    let mut h = Heard::default();
    for c in x.chunks(chunk) {
        rx.feed(c, &mut out);
        let channels = out.audio_format().map_or(1, |f| usize::from(f.1)).max(1);
        h.audio.extend(out.audio().iter().step_by(channels));
        h.requests.append(&mut out.requests);
        h.events.append(&mut out.events);
        out.clear();
    }
    h
}

/// The samples of `cfg`, once through.
pub fn samples(cfg: &SceneConfig) -> Vec<Complex32> {
    let n = (cfg.length_s * cfg.rate_hz).round() as usize;
    let mut g = SceneGenerator::new(cfg.clone()).unwrap();
    let mut x = vec![Complex32::default(); n];
    assert_eq!(g.generate(&mut x), n);
    x
}

/// The samples of the test signal of `desc` at `rate` with default settings, `seconds` long.
pub fn generated_of(desc: &StackDescriptor, rate: f64, seconds: f64, seed: u64) -> Vec<Complex32> {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)()).unwrap();
    let cfg = sdrkit_core::GenConfig::new(rate, Values::defaults(&schema)).with_seed(seed);
    let mut g = (desc.make_generator.unwrap())(&cfg).unwrap();
    assert_eq!(g.rate_hz(), rate);
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    assert_eq!(g.generate(&mut x), x.len());
    x
}
