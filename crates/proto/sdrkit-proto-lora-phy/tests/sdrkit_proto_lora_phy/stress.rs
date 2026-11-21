//! Random payloads on a clean channel: every frame decodes with a good CRC.

use crate::common::run;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_proto_lora_phy::LoraParams;

fn count_bad(
    p: &LoraParams,
    rate: f64,
    offset: f64,
    frames: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    let mut rng = Rng::new(seed);
    let mut bad = Vec::new();
    for k in 0..frames {
        let len = 16 + rng.below(100) as usize;
        let payload: Vec<u8> = (0..len).map(|_| rng.next_u32() as u8).collect();
        let fr = run(rate, offset, p, &payload, 0.0, 40.0, 0.0, 65_536);
        if !(fr.len() == 1 && fr[0].crc_ok && fr[0].payload == payload) {
            bad.push((k, len));
        }
    }
    bad
}

#[test]
fn random_payloads_decode_on_a_clean_channel() {
    let narrow = LoraParams::new(8, 62.5e3, 8, 8, 0x12);
    let bad = count_bad(&narrow, 2e6, -300e3, 12, 1);
    assert!(bad.is_empty(), "SF8 62.5 kHz: {bad:?}");
    let longfast = LoraParams::new(11, 250e3, 5, 16, 0x2B);
    let bad = count_bad(&longfast, 2.1e6, -300e3, 4, 2);
    assert!(bad.is_empty(), "SF11 250 kHz: {bad:?}");
}
