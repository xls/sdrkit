//! Helpers of the tests: frames through noise and clock error into a channel receiver.

use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{
    ChannelRx, DetectorConfig, LoraFrame, LoraParams, SymbolSoft, TxFrame,
};

/// The amplitude of the test frames.
pub const AMP: f32 = 0.3;

/// Adds noise for an SNR of `snr_db` in the bandwidth `bw` to a signal of amplitude [`AMP`] at `rate`.
pub fn add_noise(x: &mut [Complex32], snr_db: f64, bw: f64, rate: f64, seed: u64) {
    let p = (AMP as f64).powi(2) / 10f64.powf(snr_db / 10.0) * rate / bw;
    let mut n = Awgn::with_power(p, seed).unwrap();
    n.process(x);
}

/// One frame with `payload` at `offset + cfo`, through noise and a clock error, into a receiver at `offset`, fed in pieces of
/// `piece`. Returns the frames found.
#[allow(clippy::too_many_arguments)]
pub fn run(
    rate: f64,
    offset: f64,
    p: &LoraParams,
    payload: &[u8],
    cfo: f64,
    snr: f64,
    ppm: f64,
    piece: usize,
) -> Vec<LoraFrame> {
    let f = TxFrame::new(p, payload)
        .unwrap()
        .at(0.02)
        .carrier(offset + cfo)
        .amplitude(AMP);
    let mut x = f.render(rate, 0.05);
    add_noise(&mut x, snr, p.bw, rate, 7);
    let x = if ppm != 0.0 {
        let mut c = ClockError::new(ppm).unwrap();
        let mut y = Vec::new();
        c.process(&x, &mut y);
        y
    } else {
        x
    };
    let mut cfg = DetectorConfig::new(p.sf, p.bw, p.sync);
    cfg.preamble = p.preamble;
    cfg.ldro = p.ldro;
    cfg.invert = p.invert_iq;
    if p.implicit {
        cfg.implicit = Some(sdrkit_proto_lora_phy::Header {
            len: payload.len() as u8,
            cr: p.cr,
            crc: p.crc,
        });
    }
    let mut rx = ChannelRx::new(rate, offset, p.bw, vec![cfg]).unwrap();
    let mut out = Vec::new();
    for c in x.chunks(piece) {
        rx.process(c, &mut out);
    }
    out.into_iter().map(|(f, _)| f).collect()
}

/// Hard symbols.
pub fn hard(s: &[u16]) -> Vec<SymbolSoft> {
    s.iter().map(|&b| SymbolSoft::hard(b)).collect()
}

/// Confident soft symbols: the right bin strong, three others weak.
pub fn confident(s: &[u16], n: u32) -> Vec<SymbolSoft> {
    s.iter()
        .map(|&b| SymbolSoft {
            bins: [
                b,
                ((b as u32 + 7) % n) as u16,
                ((b as u32 + 13) % n) as u16,
                ((b as u32 + 29) % n) as u16,
            ],
            ll: [30.0, 1.0, 0.8, 0.6],
            count: 4,
            floor: 0.0,
        })
        .collect()
}
