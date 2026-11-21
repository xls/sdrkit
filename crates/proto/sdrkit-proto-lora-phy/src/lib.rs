//! The LoRa physical layer: the parameters, the coding chain, the chirp modulator, frame detection and decoding, the recovery
//! searches, the channel bank, and the FSK modulation of the same radios.
//!
//! Bad parameters give a [`LoraError`]. Nothing panics on bad input.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod bank;
pub mod channel;
pub mod chips;
pub mod coding;
pub mod components;
mod error;
pub mod fsk;
pub mod modulate;
pub mod params;
pub mod receiver;
pub mod recover;
pub mod rx;

pub use bank::{Bank, BankChannel, BankPlan, DetectorState};
pub use coding::{Codec, Header, Payload, SymbolSoft};
pub use error::{LoraError, Param, Result};
pub use modulate::TxFrame;
pub use params::{Layout, Ldro, LoraParams};
pub use receiver::ChannelRx;
pub use recover::Recovery;
pub use rx::{BusyChange, Detector, DetectorConfig, DetectorStats, LoraFrame, LoraSymbols};

/// The benchmarks of the hot paths, for the benchmark hooks of the engine (`examples/bench.rs` runs them on a PC).
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};

    fn receive(seconds: f64, sf: u8) -> BenchResult {
        let p = LoraParams::new(sf, 125e3, 5, 8, 0x12);
        let rate = 250e3;
        let payload: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let x = match TxFrame::new(&p, &payload) {
            Ok(f) => f.at(0.005).render(rate, 0.01),
            Err(_) => Vec::new(),
        };
        let mut out = Vec::with_capacity(4);
        // The same frame again and again, as one stream.
        let mut rx =
            ChannelRx::new(rate, 0.0, 125e3, vec![DetectorConfig::new(sf, 125e3, 0x12)]).ok();
        measure(seconds, x.len(), x.len() as f64 / rate, || {
            if let Some(rx) = &mut rx {
                out.clear();
                rx.process(&x, &mut out);
            }
        })
    }

    fn decode(seconds: f64, soft: bool) -> BenchResult {
        let p = LoraParams::new(9, 125e3, 8, 8, 0x12);
        let payload: Vec<u8> = (0..200).map(|i| (i * 7) as u8).collect();
        let codec = Codec::new(9).ok();
        let syms: Vec<SymbolSoft> = coding::encode(&payload, &p)
            .unwrap_or_default()
            .iter()
            .map(|&b| SymbolSoft {
                bins: [b, b ^ 1, b ^ 2, b ^ 4],
                ll: [20.0, 3.0, 2.0, 1.0],
                count: 4,
                floor: 0.5,
            })
            .collect();
        let h = Header {
            len: 200,
            cr: 8,
            crc: true,
        };
        let mut out = Payload::default();
        measure(
            seconds,
            syms.len(),
            syms.len() as f64 * p.symbol_s(),
            || {
                if let Some(c) = &codec {
                    c.decode_payload(&syms, false, false, h, soft, &mut out);
                }
            },
        )
    }

    vec![
        Benchmark {
            name: "lora.receive_sf7_250k",
            run: |s| receive(s, 7),
        },
        Benchmark {
            name: "lora.receive_sf12_250k",
            run: |s| receive(s, 12),
        },
        Benchmark {
            name: "lora.decode_sf9_soft",
            run: |s| decode(s, true),
        },
        Benchmark {
            name: "lora.decode_sf9_hard",
            run: |s| decode(s, false),
        },
    ]
}
