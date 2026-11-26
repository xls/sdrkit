//! Benchmarks of `wifi` for `sdrkit bench`: the stack on its 2.4 GHz test signal at 20 Msps, and one channel chain on a busy
//! channel of HT and VHT A-MPDUs back to back at 20 Msps with every data field decoded (the keep-up gate off). Items are input
//! samples. A benchmark whose receiver decodes nothing from its signal reports no throughput.

use super::{receiver, Chain, Fields, Rx, WifiSetting, CHAIN_RATE_HZ, CHAIN_WIDTH_HZ, WIFI};
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
    busy_with(seconds, 0)
}

/// The busy channel through one chain whose data fields run on `helpers` helper threads, every field decoded.
fn busy_with(seconds: f64, helpers: usize) -> BenchResult {
    let x = busy_signal();
    let Some(mut c) = busy_chain() else {
        return BenchResult::default();
    };
    let pool = sdrkit_graph::Pool::new(helpers.max(1));
    let mut fields = Fields::new(&pool, helpers, false);
    let mut out = Vec::new();
    let mut pass = |c: &mut Chain, out: &mut Vec<_>| {
        for b in x.chunks(65_536) {
            c.run(b, 1.0, out, &mut fields);
            fields.flush(true);
            out.append(&mut fields.ready);
        }
    };
    pass(&mut c, &mut out);
    if out.is_empty() {
        return BenchResult::default();
    }
    let n = x.len();
    measure(seconds, n, n as f64 / CHAIN_RATE_HZ, || {
        out.clear();
        pass(&mut c, &mut out);
    })
}

/// One OFDM chain at 20 Msps on channel 36.
fn busy_chain() -> Option<Chain> {
    let rx = Rx::Ofdm(Box::new(ofdm::OfdmReceiver::new()), Box::new(PpduDecoder::new()));
    Chain::new(
        CHAIN_RATE_HZ,
        0.0,
        CHAIN_WIDTH_HZ,
        CHAIN_RATE_HZ,
        rx,
        0.0,
        Some(Band::G5),
        36,
    )
}

#[cfg(test)]
mod keep_up {
    use super::*;

    /// The time of the stack on its 2.4 GHz scene at 20 Msps with the centre known (chains on channels 5, 6 and 7) and not
    /// known (channel 6 only), fastest of the runs.
    #[test]
    #[ignore = "a timing measurement for a release build"]
    fn the_cost_of_the_neighbouring_channels() {
        let rate = CHAIN_RATE_HZ;
        let cfg = sdrkit_core::GenConfig::new(rate, Values::defaults(&(WIFI.settings)())).with_seed(1);
        let mut g = (WIFI.make_generator.unwrap())(&cfg).unwrap();
        let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
        g.generate(&mut x);
        for centre in [2437e6, 0.0] {
            let mut rx = receiver(&StackConfig::new(Values::defaults(&WifiSetting::schema()))).unwrap();
            let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
            let mut out = Output::default();
            let r = measure(3.0, x.len(), x.len() as f64 / rate, || {
                for b in x.chunks(65_536) {
                    out.clear();
                    rx.feed(b, &mut out);
                }
            });
            eprintln!("centre {:.0} MHz: {:.3} s per s of signal", centre / 1e6, r.cpu_fraction_of_realtime);
        }
    }

    /// The share of HT and VHT data fields skipped on two seconds of the busy channel fed at signal pace as live input, and the
    /// time with every field decoded, for 0, 1, 2 and 4 helper threads beside the chain's own thread. Three rounds, each of
    /// every count back to back, so that the counts of a round see the same load.
    #[test]
    #[ignore = "a timing measurement for a release build"]
    fn busy_channel_skip_share_at_signal_pace() {
        let x = busy_signal();
        for round in 0..3 {
            for helpers in [0usize, 1, 2, 4] {
                let mut c = busy_chain().unwrap();
                let pool = sdrkit_graph::Pool::new(helpers.max(1));
                let mut fields = Fields::new(&pool, helpers, true);
                let mut out = Vec::new();
                let start = std::time::Instant::now();
                let mut fed = 0usize;
                for _ in 0..8 {
                    for b in x.chunks(65_536) {
                        let due = fed as f64 / CHAIN_RATE_HZ;
                        let now = start.elapsed().as_secs_f64();
                        if due > now {
                            std::thread::sleep(std::time::Duration::from_secs_f64(due - now));
                        }
                        c.run(b, 1.0, &mut out, &mut fields);
                        fields.flush(false);
                        out.append(&mut fields.ready);
                        fed += b.len();
                    }
                }
                fields.flush(true);
                let busy = fields.ppdu.not_decoded_for(sdrkit_proto_wlan_phy::ppdu::Undecoded::Busy);
                let all = fields.ppdu.decoded + busy;
                let share = 100.0 * busy as f64 / all.max(1) as f64;
                let decoded_all = busy_with(1.0, helpers).cpu_fraction_of_realtime;
                eprintln!(
                    "round {round}, {helpers} helpers: skipped {share:.1} % at signal pace; every field decoded: {decoded_all:.2} s per s of signal"
                );
            }
        }
    }
}
