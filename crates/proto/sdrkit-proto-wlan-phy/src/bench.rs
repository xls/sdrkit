//! Benchmarks of the receivers, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]). Each makes its test signal
//! first, then times the receiver on it. Items are input samples.

use crate::{dsss, mac, ofdm, Complex32};
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};
use std::hint::black_box;

/// Adds uniform noise of power `power` to `x`. The generator is fixed, so each run produces the same signal.
fn add_noise(x: &mut [Complex32], power: f32) {
    let a = (6.0 * power).sqrt();
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut uniform = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    for v in x {
        *v += Complex32::new(a * uniform(), a * uniform());
    }
}

/// A beacon MPDU of `len` bytes with its FCS.
fn beacon(len: usize) -> Vec<u8> {
    let mut f = Vec::new();
    mac::push_mgmt_header(
        &mut f,
        mac::MgmtSubtype::Beacon,
        mac::BROADCAST,
        [2; 6],
        [2; 6],
        1,
    );
    mac::push_beacon_fields(&mut f, 0, 100, 1);
    while f.len() < len {
        f.push(f.len() as u8);
    }
    mac::push_fcs(&mut f);
    f
}

/// The OFDM receiver on ten 1500-byte frames at `rate`, with gaps and noise 30 dB below the signal.
fn ofdm_receiver(seconds: f64, rate: ofdm::Rate) -> BenchResult {
    let mut x = Vec::new();
    for _ in 0..10 {
        x.extend(std::iter::repeat_n(Complex32::default(), 2000));
        if ofdm::build_ppdu(&beacon(1500), rate, 0x5D, &mut x).is_err() {
            return BenchResult::default();
        }
    }
    x.extend(std::iter::repeat_n(Complex32::default(), 2000));
    add_noise(&mut x, 1e-3);
    let mut rx = ofdm::OfdmReceiver::new();
    let n = x.len();
    measure(seconds, n, n as f64 / ofdm::SAMPLE_RATE, || {
        rx.process(black_box(&x), |f| {
            black_box(f.mpdu);
        })
    })
}

/// The DSSS receiver on one 200-byte frame at 1 Mbit/s, 22 samples per symbol, with noise 10 dB below the signal.
fn dsss_receiver(seconds: f64) -> BenchResult {
    let psdu = beacon(200);
    let mut bits = Vec::new();
    let mut sym = Vec::new();
    let mut chips = Vec::new();
    let built = dsss::PlcpHeader::for_psdu(dsss::Rate::Dbpsk1, psdu.len()).and_then(|h| {
        dsss::ppdu_bits(dsss::Preamble::Long, &h, &psdu, &mut bits)?;
        dsss::scramble(&mut bits, dsss::Preamble::Long);
        dsss::symbols(&bits, dsss::Preamble::Long, dsss::Rate::Dbpsk1, &mut sym)
    });
    let Ok(mut rx) = built.and_then(|_| dsss::DsssReceiver::new(22)) else {
        return BenchResult::default();
    };
    dsss::spread(&sym, &mut chips);
    let mut x: Vec<Complex32> = chips.iter().flat_map(|&c| [c, c]).collect();
    x.extend(std::iter::repeat_n(Complex32::default(), 20_000));
    add_noise(&mut x, 0.1);
    let n = x.len();
    measure(seconds, n, n as f64 / 22e6, || {
        rx.process(black_box(&x), |f| {
            black_box(f.mpdu);
        })
    })
}

/// The benchmarks: the 802.11a/g OFDM receiver at 6 and 54 Mbit/s (real time is 20 Msps) and the 802.11b receiver at 1 Mbit/s
/// (real time is 22 Msps).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "wlan_phy.ofdm_receiver_6m",
            run: |s| ofdm_receiver(s, ofdm::Rate::R6),
        },
        Benchmark {
            name: "wlan_phy.ofdm_receiver_54m",
            run: |s| ofdm_receiver(s, ofdm::Rate::R54),
        },
        Benchmark {
            name: "wlan_phy.dsss_receiver_1m",
            run: dsss_receiver,
        },
    ]
}
