//! Benchmarks of the bit-level hot paths of `sdrkit-util-bits`, for `sdrkit_benchmark` (see `sdrkit_core::bench`). Each makes its test
//! data first, then times the path on it. `items_per_second` counts the items the name says (bytes or bits);
//! `cpu_fraction_of_realtime` is for the rate each one's documentation names.

use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC32_ISO_HDLC};
use sdrkit_util_bits::hdlc::{hdlc_encode, HdlcDeframer};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_bits::{pack, seq, BitOrder};
use std::hint::black_box;

/// Bytes per frame of the CRC benchmarks (an Ethernet-sized frame).
const FRAME: usize = 1500;
/// Frames per timed call of the CRC benchmarks.
const FRAMES: usize = 64;
/// Bits per timed call of the per-bit benchmarks.
const BITS: usize = 1 << 16;

/// Repeatable pseudo-random bytes.
fn bytes(n: usize) -> Vec<u8> {
    let mut s = 0x1234_5678u32;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 24) as u8
        })
        .collect()
}

fn bits(n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(n);
    pack::unpack(&bytes(n.div_ceil(8)), BitOrder::Msb, &mut v);
    v.truncate(n);
    v
}

/// CRC-32/ISO-HDLC (the 802.11 FCS) over 1500-byte frames, byte table; real time is 54 Mbit/s.
fn crc32_frames(seconds: f64) -> BenchResult {
    let data = bytes(FRAME * FRAMES);
    measure(seconds, data.len(), data.len() as f64 * 8.0 / 54e6, || {
        for f in data.as_chunks::<FRAME>().0 {
            black_box(CRC32_ISO_HDLC.checksum(black_box(f)));
        }
    })
}

/// CRC-16/IBM-SDLC (the HDLC FCS) over 1500-byte frames; real time is 10 Mbit/s.
fn crc16_frames(seconds: f64) -> BenchResult {
    let data = bytes(FRAME * FRAMES);
    measure(seconds, data.len(), data.len() as f64 * 8.0 / 10e6, || {
        for f in data.as_chunks::<FRAME>().0 {
            black_box(CRC16_IBM_SDLC.checksum(black_box(f)));
        }
    })
}

/// The DVB energy-dispersal descrambler on hard bits, one bit per step; real time is 50 Mbit/s (a DVB-S2 transport stream).
fn descramble_bits(seconds: f64) -> BenchResult {
    let mut x = bits(BITS);
    let mut s = Additive::new(seq::DVB_DISPERSAL);
    measure(seconds, x.len(), x.len() as f64 / 50e6, || {
        s.reset();
        s.apply_bits(black_box(&mut x));
    })
}

/// The DVB energy-dispersal descrambler on bytes, eight sequence bits per byte; real time is 50 Mbit/s.
fn descramble_bytes(seconds: f64) -> BenchResult {
    let mut x = bytes(BITS / 8);
    let mut s = Additive::new(seq::DVB_DISPERSAL);
    measure(seconds, x.len(), x.len() as f64 * 8.0 / 50e6, || {
        s.reset();
        s.apply_bytes(black_box(&mut x), BitOrder::Msb);
    })
}

/// HDLC deframing (flags, unstuffing, FCS) of back-to-back 200-byte frames, per bit; real time is 9600 bit/s (G3RUH packet).
fn hdlc_bits(seconds: f64) -> BenchResult {
    let payload = bytes(200);
    let frames: Vec<&[u8]> = (0..32).map(|_| payload.as_slice()).collect();
    let mut line = Vec::new();
    hdlc_encode(&frames, &CRC16_IBM_SDLC, 1, &mut line);
    let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 400);
    measure(seconds, line.len(), line.len() as f64 / 9600.0, || {
        for &b in &line {
            black_box(d.push(b));
        }
    })
}

/// The 32-bit CCSDS sync word search with up to 3 errors and either polarity, per bit; real time is 10 Mbit/s.
fn sync_bits(seconds: f64) -> BenchResult {
    let x = bits(BITS);
    let mut w = match SyncWord::new(0x1ACF_FC1D, 32, 3) {
        Ok(w) => w.with_inverted(true),
        Err(_) => unreachable!("a 32-bit word with 3 errors is valid"),
    };
    measure(seconds, x.len(), x.len() as f64 / 10e6, || {
        for &b in &x {
            black_box(w.push(b));
        }
    })
}

/// Every benchmark of this module.
pub(super) fn all() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "bits.crc32_1500",
            run: crc32_frames,
        },
        Benchmark {
            name: "bits.crc16_1500",
            run: crc16_frames,
        },
        Benchmark {
            name: "bits.descramble_per_bit",
            run: descramble_bits,
        },
        Benchmark {
            name: "bits.descramble_per_byte",
            run: descramble_bytes,
        },
        Benchmark {
            name: "bits.hdlc_per_bit",
            run: hdlc_bits,
        },
        Benchmark {
            name: "bits.sync_per_bit",
            run: sync_bits,
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_benchmark_runs() {
        for b in super::all() {
            let r = (b.run)(0.0);
            assert!(r.items_per_second > 0.0, "{}", b.name);
        }
    }

    /// Prints the numbers: `cargo test --release -p sdrkit-components bits::bench -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_numbers() {
        for b in super::all() {
            let r = (b.run)(0.5);
            println!(
                "{:28} {:10.1} M items/s  {:8.5} of real time",
                b.name,
                r.items_per_second / 1e6,
                r.cpu_fraction_of_realtime
            );
        }
    }
}
