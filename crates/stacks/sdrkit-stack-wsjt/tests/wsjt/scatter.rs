//! Helpers of the tests of the burst, ping and beacon stacks: audio of known SNR, a stack's receiver fed with audio, and
//! the decode events it wrote.

use crate::common::{iq_of, noise};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Event, InputInfo, Output, Receiver, StackConfig, StackDescriptor, Value, Values,
};

/// Noise sigma of the tests, per real sample at 8 kHz.
pub const SIGMA: f64 = 0.05;

/// The amplitude of a tone of `snr` dB in 2500 Hz against noise of sigma [`SIGMA`] at `rate`.
pub fn amp_at(snr: f64, rate: f64) -> f32 {
    (2.0 * 10f64.powf(snr / 10.0) * SIGMA * SIGMA * 2500.0 / (rate / 2.0)).sqrt() as f32
}

/// `x` added into `into` from sample `at` on.
pub fn put(into: &mut [f32], x: &[f32], at: usize) {
    for (i, &v) in x.iter().enumerate() {
        if let Some(o) = into.get_mut(at + i) {
            *o += v;
        }
    }
}

/// `seconds` of noise of sigma [`SIGMA`] at 8 kHz with `x` added at `at_s`.
pub fn in_noise(x: &[f32], seconds: f64, at_s: f64, seed: u64) -> Vec<f32> {
    let mut a = noise((seconds * 8000.0) as usize, SIGMA, seed);
    put(&mut a, x, (at_s * 8000.0) as usize);
    a
}

/// The receiver of stack `d` with these settings, for 8 kHz audio as a recording.
pub fn receiver(d: &StackDescriptor, extra: &[(&str, Value)]) -> Box<dyn Receiver> {
    let schema = (d.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in extra {
        let id = schema.id(k).unwrap_or_else(|e| panic!("{k}: {e}"));
        s.set(id, v).unwrap_or_else(|e| panic!("{k}: {e}"));
    }
    let mut rx = (d.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(8000.0, 50e6, 50e6).recording())
        .unwrap();
    rx
}

/// Feeds 8 kHz audio to `rx` in pieces of `piece` samples, finishes, and returns the events.
pub fn feed_audio(rx: &mut dyn Receiver, a: &[f32], piece: usize) -> Vec<Event> {
    let iq = iq_of(a);
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in iq.chunks(piece) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    rx.finish(&mut out);
    ev.append(&mut out.events);
    ev
}

/// The receiver of stack `d` fed with `seconds` of its own test signal at 48 kHz, and the events it wrote.
pub fn on_test_signal(d: &StackDescriptor, seconds: f64) -> (Box<dyn Receiver>, Vec<Event>) {
    let rate = 48_000.0;
    let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(
        rate,
        Values::defaults(&(d.settings)()),
    ))
    .unwrap();
    let mut iq = vec![sdrkit_core::Complex32::default(); (seconds * rate) as usize];
    g.generate(&mut iq);
    let mut rx = receiver(d, &[]);
    rx.configure(&InputInfo::new(rate, 50e6 + 20e3, 50e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in iq.chunks(9600) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    (rx, ev)
}

/// The decode events among `ev`.
pub fn decodes(ev: &[Event]) -> Vec<&Event> {
    ev.iter().filter(|e| e.kind() == "wsjt.decode").collect()
}

/// The text of a decode event.
pub fn text(e: &Event) -> &str {
    e.get(&keys::TEXT).unwrap_or("")
}

/// The rows of a data file at `path` from the crate folder (comment lines and the header left out).
pub fn data_rows(path: &str) -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let text = std::fs::read_to_string(&path).expect("the data file");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

/// The data files hold the dials of the specification and the PI4 variants by channel spacing; the stacks hold neither.
#[test]
fn the_data_files_have_their_rows() {
    let dials = data_rows("../../../data/wsjt/scatter_dials.csv");
    assert_eq!(dials.len(), 4);
    for (row, d) in dials.iter().zip([
        &sdrkit_stack_wsjt::FSK441,
        &sdrkit_stack_wsjt::ISCAT,
        &sdrkit_stack_wsjt::JT6M,
        &sdrkit_stack_wsjt::PI4,
    ]) {
        assert_eq!(row[0], d.id);
        let hz: f64 = row[2].parse().unwrap();
        assert!(hz >= d.tuning.min_hz && hz <= d.tuning.max_hz, "{}", d.id);
        assert_ne!(
            hz, d.tuning.default_hz,
            "{}: the descriptor holds the test-signal dial",
            d.id
        );
    }
    let variants = data_rows("data/pi4_variants.csv");
    assert_eq!(variants.len(), 6);
    for v in &variants {
        let k: u32 = v[1].parse().unwrap();
        assert!([40, 80, 96, 120].contains(&k));
    }
}
