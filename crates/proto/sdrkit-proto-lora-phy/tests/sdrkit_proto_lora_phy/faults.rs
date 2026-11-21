//! Frames with the faults of real transmitters and radios: every one must decode at a high signal level.

use crate::impair::{Radio, Tx};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig, LoraFrame, LoraParams};

/// The radio frequency on which a crystal error acts, Hz.
const RF_HZ: f64 = 920e6;
const RATE: f64 = 1e6;

/// SF11 at 250 kHz, coding rate 4/5, 16 preamble symbols.
fn long_fast() -> LoraParams {
    LoraParams::new(11, 250e3, 5, 16, 0x2b)
}

fn receiver(p: &LoraParams) -> ChannelRx {
    let mut d = DetectorConfig::new(p.sf, p.bw, p.sync);
    d.preamble = p.preamble;
    d.ldro = p.ldro;
    ChannelRx::new(RATE, 0.0, p.bw, vec![d]).unwrap()
}

fn feed(rx: &mut ChannelRx, x: &[Complex32]) -> Vec<LoraFrame> {
    let mut out = Vec::new();
    for c in x.chunks(50_000) {
        rx.process(c, &mut out);
    }
    out.into_iter().map(|(f, _)| f).collect()
}

/// A frame of `len` random bytes at a start on no sample grid, with a random phase and a carrier within one bin.
fn random_tx(p: &LoraParams, len: usize, start_s: f64, rng: &mut Rng) -> Tx {
    let mut payload = vec![0u8; len];
    rng.fill_bytes(&mut payload);
    let mut t = Tx::new(&payload, start_s + rng.uniform() * p.symbol_s());
    t.phase = rng.range(0.0, std::f64::consts::TAU);
    t.carrier_hz = rng.range(-1.0, 1.0) * p.bw / p.n() as f64;
    t
}

/// Sends `count` frames of `len` bytes, each changed by `fault`, through `radio`, each into a new receiver. Returns how
/// many were decoded with a good CRC and the bytes that were sent.
fn right(count: u64, len: usize, radio: &Radio, fault: impl Fn(&mut Tx, &mut Rng)) -> u64 {
    let p = long_fast();
    let mut good = 0;
    for seed in 0..count {
        let mut rng = Rng::new(seed + 1);
        let mut t = random_tx(&p, len, 0.02, &mut rng);
        fault(&mut t, &mut rng);
        let seconds = t.start_s + t.duration_s(&p) + 4.0 * p.symbol_s();
        let x = radio.render(&p, std::slice::from_ref(&t), seconds);
        let frames = feed(&mut receiver(&p), &x);
        good += frames.iter().any(|f| f.crc_ok && f.payload == t.payload) as u64;
    }
    good
}

fn radio() -> Radio {
    let mut r = Radio::new(RATE);
    r.rf_hz = RF_HZ;
    r
}

#[test]
fn a_crystal_error_moves_carrier_and_clock_of_a_long_frame() {
    for ppm in [-30.0, 30.0] {
        let good = right(6, 200, &radio(), |t, _| t.crystal_ppm = ppm);
        assert_eq!(good, 6, "{ppm} ppm");
    }
}

#[test]
fn a_clock_error_alone_is_followed() {
    for ppm in [-30.0, 30.0] {
        let good = right(6, 200, &radio(), |t, _| t.clock_ppm = ppm);
        assert_eq!(good, 6, "{ppm} ppm");
    }
}

#[test]
fn a_carrier_that_drifts_during_the_frame_is_followed() {
    for (len, hz_s) in [(60, -100.0), (60, 100.0), (200, -100.0), (200, 100.0)] {
        let good = right(4, len, &radio(), |t, _| t.drift_hz_s = hz_s);
        assert_eq!(good, 4, "{len} bytes, {hz_s} Hz/s");
    }
}

#[test]
fn a_converter_that_clips_keeps_the_frames() {
    let mut r = radio();
    r.amp = 2.0;
    r.adc_bits = Some(12);
    let good = right(6, 200, &r, |t, rng| t.crystal_ppm = rng.range(-30.0, 30.0));
    assert_eq!(good, 6);
}

/// The number after `"key":` in the text of a SigMF description.
fn meta_number(meta: &str, key: &str) -> Option<f64> {
    let rest = &meta[meta.find(key)? + key.len()..];
    let rest = rest.trim_start_matches(|c: char| c == '"' || c == ':' || c.is_whitespace());
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// A recording of two Meshtastic nodes near the radio on 919.875 MHz (SF11, 250 kHz), whose carriers drift by up to 50 Hz/s
/// while they send: fourteen strong frames of 35 to 221 bytes and three weak frames. The environment variable
/// `SDRKIT_MESH_REAL_A` names its SigMF description (16-bit samples). Without it the test does nothing.
#[test]
fn a_recording_of_real_nodes_has_no_crc_error() {
    let Ok(meta_path) = std::env::var("SDRKIT_MESH_REAL_A") else {
        return;
    };
    let meta = std::fs::read_to_string(&meta_path).expect("the description");
    let rate = meta_number(&meta, "core:sample_rate").expect("a rate");
    let centre = meta_number(&meta, "core:frequency").expect("a frequency");
    let bytes =
        std::fs::read(meta_path.replace(".sigmf-meta", ".sigmf-data")).expect("the samples");
    let p = long_fast();
    let mut d = DetectorConfig::new(p.sf, p.bw, p.sync);
    d.preamble = p.preamble;
    let mut rx = ChannelRx::new(rate, 919.875e6 - centre, p.bw, vec![d]).unwrap();
    let mut out = Vec::new();
    let mut x = Vec::new();
    for block in bytes.chunks(4 * 65536) {
        x.clear();
        x.extend(block.as_chunks::<4>().0.iter().map(|c| {
            Complex32::new(
                i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0,
                i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0,
            )
        }));
        rx.process(&x, &mut out);
    }
    let strong: Vec<_> = out.iter().filter(|(f, _)| f.snr_db > 10.0).collect();
    let weak: Vec<_> = out.iter().filter(|(f, _)| f.snr_db <= 10.0).collect();
    assert_eq!(strong.len(), 14);
    assert!(strong
        .iter()
        .all(|(f, _)| f.crc_ok && !f.list_fixed && f.jumps == 0));
    assert_eq!(weak.iter().filter(|(f, _)| f.crc_ok).count(), 3);
    // The long frames show the drift of the carrier.
    assert!(strong
        .iter()
        .filter(|(f, _)| f.payload.len() > 150)
        .all(|(f, _)| (f.cfo_end_hz - f.cfo_hz).abs() > 40.0));
}
