//! Benchmarks of `wifi` for `sdrkit bench`: the stack on its 2.4 GHz test signal at 20 Msps, and one channel chain on a busy
//! channel of HT and VHT A-MPDUs back to back at 20 Msps with every data field decoded (the keep-up gate off). Items are input
//! samples. A benchmark whose receiver decodes nothing from its signal reports no throughput.

use super::{receiver, Chain, Rx, WifiSetting, CHAIN_RATE_HZ, CHAIN_WIDTH_HZ, WIFI};
use crate::wifi::plan::Band;
use sdrkit_core::bench::{measure, BenchResult, Benchmark};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, SettingSet, StackConfig, Values};
use sdrkit_proto_wlan_phy::ht::{self, HtSig, Mcs, PilotPattern};
use sdrkit_proto_wlan_phy::ppdu::PpduDecoder;
use sdrkit_proto_wlan_phy::vht::{self, VhtMcs};
use sdrkit_proto_wlan_phy::{mac, ofdm};

/// The benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "wifi.receiver_20m",
            run: stack,
        },
        Benchmark {
            name: "wifi.ht_vht_busy_20m",
            run: busy,
        },
    ]
}

fn stack(seconds: f64) -> BenchResult {
    let rate = CHAIN_RATE_HZ;
    let cfg = sdrkit_core::GenConfig::new(rate, Values::defaults(&(WIFI.settings)())).with_seed(1);
    let Some(Ok(mut g)) = WIFI.make_generator.map(|make| make(&cfg)) else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&WifiSetting::schema()))) else {
        return BenchResult::default();
    };
    let dial = WIFI.tuning.default_hz;
    let _ = rx.configure(&InputInfo::new(rate, dial, dial).recording());
    let mut out = Output::default();
    for b in x.chunks(65_536) {
        out.clear();
        rx.feed(b, &mut out);
    }
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// A data MPDU of `n` payload bytes with its FCS.
fn data_mpdu(n: usize, tag: u8) -> Vec<u8> {
    let mut f = vec![
        0x88, 0x01, 0, 0, 2, 0, 0, 0, 0, 1, 6, 1, 2, 3, 4, tag, 2, 0, 0, 0, 0, 1, 0x10, 0, 0, 0,
    ];
    f.extend((0..n).map(|i| tag.wrapping_add(i as u8)));
    mac::push_fcs(&mut f);
    f
}

/// 0.25 s of a busy channel at 20 Msps: HT MCS 7 and VHT MCS 8 A-MPDUs of four 1500-byte MPDUs, 16 µs apart.
pub fn busy_signal() -> Vec<Complex32> {
    let mut ampdu = Vec::new();
    for i in 0..4u8 {
        mac::push_ampdu_subframe(&mut ampdu, &data_mpdu(1500, i), i == 3);
    }
    let sig = HtSig {
        mcs: 7,
        smoothing: true,
        not_sounding: true,
        aggregation: true,
        ..HtSig::default()
    };
    let (mut h, mut v) = (Vec::new(), Vec::new());
    ht::build_ppdu_sig(&ampdu, sig, Mcs(7), 0x21, PilotPattern::Rotating, &mut h);
    vht::build_ppdu(&ampdu, VhtMcs(8), true, 0x44, &mut v);
    let total = (0.25 * ofdm::SAMPLE_RATE) as usize;
    let mut x = Vec::with_capacity(total + h.len());
    while x.len() < total {
        for p in [&h, &v] {
            x.extend_from_slice(p);
            x.extend(std::iter::repeat_n(Complex32::default(), 320));
        }
    }
    // A little noise keeps the receiver honest about its detection work.
    let mut seed = 1u32;
    for s in &mut x {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let a = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.02;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let b = ((seed >> 8) as f32 / 16_777_216.0 - 0.5) * 0.02;
        *s += Complex32::new(a, b);
    }
    x
}

fn busy(seconds: f64) -> BenchResult {
    let x = busy_signal();
    let rx = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()), Box::new(PpduDecoder::new()));
    let Some(mut c) = Chain::new(
        CHAIN_RATE_HZ,
        0.0,
        CHAIN_WIDTH_HZ,
        CHAIN_RATE_HZ,
        rx,
        0.0,
        Some(Band::G5),
        36,
    ) else {
        return BenchResult::default();
    };
    c.keep_up = false;
    let mut out = Vec::new();
    for b in x.chunks(65_536) {
        c.run(b, 1.0, &mut out);
    }
    if out.is_empty() {
        return BenchResult::default();
    }
    let n = x.len();
    measure(seconds, n, n as f64 / CHAIN_RATE_HZ, || {
        out.clear();
        for b in x.chunks(65_536) {
            c.run(b, 1.0, &mut out);
        }
    })
}
