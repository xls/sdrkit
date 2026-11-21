//! The standard components across families, end to end: data bits are convolutionally encoded (the DVB K = 7, rate 1/2 code of
//! EN 300 421 clause 4.4.3), sent as differential QPSK with RRC pulses by `sdrkit-algo-sim`, with a carrier offset, a clock error and
//! noise, and come back through `modem.matched_filter -> modem.symbol_sync -> modem.carrier_sync -> modem.slicer -> fec.viterbi`
//! exactly. (Differential coding removes the four-fold phase ambiguity the carrier loop leaves; a symbol carries one code-bit
//! pair, so the Viterbi decoder sees the pairs aligned.)

mod set_util;

use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_algo_modem::differential::Differential;
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::registry;
use sdrkit_components::testing::run_iq;
use sdrkit_core::Complex32;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Description, Format};

const RATE: f64 = 48_000.0;
const SPS: f64 = 4.0;
const DATA_BITS: usize = 15_000;

/// The data bits and the impaired signal carrying their code bits.
fn signal(seed: u64) -> (Vec<u8>, Vec<Complex32>) {
    let mut rng = Rng::new(seed);
    let mut data = vec![0u8; DATA_BITS];
    rng.fill_bits(&mut data);
    let mut coded = Vec::new();
    let mut enc = Encoder::new(codes::DVB_K7);
    enc.encode(&data, &mut coded);
    let steps = Differential::dqpsk();
    let pts = steps.steps().points().to_vec();
    let mut z = Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_4);
    let mut syms = vec![z];
    for b in coded.chunks(2) {
        z *= pts[((b[0] << 1) | b[1]) as usize];
        syms.push(z);
    }
    let mut tx = Linear::rrc(0.35, 16.0, SPS).unwrap();
    let mut x = Vec::new();
    tx.push(&syms, &mut x);
    tx.flush(&mut x);
    let mut y = Vec::new();
    ClockError::new(250.0).unwrap().process(&x, &mut y);
    FrequencyOffset::new(RATE, -120.0, 0.0)
        .unwrap()
        .process(&mut y);
    // Es/N0 = 8 dB: the coded stream has errors, the decoded one none.
    Awgn::with_power(SPS / 6.3, seed + 1)
        .unwrap()
        .process(&mut y);
    (data, y)
}

/// The chain as a JSON description, built from the registry by name.
const CHAIN: &str = r#"{
  "components": [
    { "id": "mf",  "type": "modem.matched_filter", "settings": { "pulse": "rrc", "beta": 0.35, "sps": 4, "decimate": 2 } },
    { "id": "ss",  "type": "modem.symbol_sync",    "settings": { "sps": 2, "bandwidth": 0.002 } },
    { "id": "cs",  "type": "modem.carrier_sync",   "settings": { "detector": "costas", "order": 4, "bandwidth": 0.02, "fll": 0.005 } },
    { "id": "sl",  "type": "modem.slicer",         "settings": { "constellation": "qpsk", "differential": "dqpsk" } },
    { "id": "vit", "type": "fec.viterbi",          "settings": { "code": "dvb_k7", "frame": 0 } }
  ],
  "edges": [["input", "mf.in"], ["mf.out", "ss.in"], ["ss.out", "cs.in"], ["cs.out", "sl.in"], ["sl.out", "vit.in"]],
  "status": "vit"
}"#;

#[test]
fn coded_dqpsk_through_five_standard_components_decodes_exactly() {
    let (data, x) = signal(5);
    let mut g = Description::from_json(CHAIN)
        .unwrap()
        .build(&registry())
        .unwrap();
    let Data::HardBits(rx) = run_iq(&mut g, Format::iq(RATE, 0.0, 0.0), &x, 3000, "vit.out") else {
        panic!("hard bits expected")
    };
    // Find where the data lines up, after the loops have locked (within the first 2000 symbols).
    let from = 4000;
    let shift = (-200isize..=200)
        .find(|&s| (from..from + 300).all(|i| rx.get(i) == data.get((i as isize - s) as usize)))
        .expect("the decoded bits line up with the data");
    let end = (data.len() as isize + shift).min(rx.len() as isize) as usize - 100;
    let errors = (2000..end)
        .filter(|&i| rx[i] != data[(i as isize - shift) as usize])
        .count();
    assert!(end - 2000 > 10_000, "checked {} bits", end - 2000);
    assert_eq!(errors, 0, "decoded bit errors after lock (shift {shift})");
    // The decoder corrected channel errors on the way, and says so.
    let corrected = set_util::result_f64(&g, "vit", "bits_corrected");
    assert!(corrected > 0.0, "bits_corrected {corrected}");
}
