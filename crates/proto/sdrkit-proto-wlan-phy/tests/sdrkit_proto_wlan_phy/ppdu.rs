//! HT, VHT, HE and EHT PPDUs through the non-HT receiver and [`PpduDecoder`]: test signals built from the standards' transmit
//! rules, decoded at several signal-to-noise ratios.
//!
//! SNR here is the signal power of the PPDU over the noise power in the 20 Msps input.

use super::impair;
use sdrkit_proto_wlan_phy::ht::{self, HtSig, Mcs, PilotPattern};
use sdrkit_proto_wlan_phy::mac::{self, Ampdu};
use sdrkit_proto_wlan_phy::ofdm::{self, OfdmReceiver, OfdmStats, Rate};
use sdrkit_proto_wlan_phy::ppdu::{build_he_preamble, Format, Ppdu, PpduDecoder, PpduInfo, SigFields, Undecoded};
use sdrkit_proto_wlan_phy::sig::{self, HeFormat, HeSigA, HeSu, USig, VhtSigA};
use sdrkit_proto_wlan_phy::vht::{self, VhtMcs};
use sdrkit_proto_wlan_phy::Complex32;

/// An MPDU of `n` body octets with its FCS.
fn mpdu(n: usize, tag: u8) -> Vec<u8> {
    let mut f = vec![0x08, 0x01, 0x2C, 0x00];
    f.extend_from_slice(&[0x02, 0, 0, 0, 0, 1, 0x06, 1, 2, 3, 4, tag, 0x02, 0, 0, 0, 0, 1, 0x10, 0x00]);
    f.extend((0..n).map(|i| tag.wrapping_mul(31).wrapping_add(i as u8)));
    mac::push_fcs(&mut f);
    f
}

/// An A-MPDU of the MPDUs.
fn ampdu(mpdus: &[Vec<u8>]) -> Vec<u8> {
    let mut a = Vec::new();
    for (i, m) in mpdus.iter().enumerate() {
        mac::push_ampdu_subframe(&mut a, m, i + 1 == mpdus.len());
    }
    a
}

/// What the receiver chain gave.
#[derive(Default)]
struct Got {
    /// MPDUs with a good FCS (FCS included), from HT and VHT PPDUs.
    mpdus: Vec<Vec<u8>>,
    /// MPDUs of those PPDUs with a bad FCS.
    bad: usize,
    /// PPDUs not decoded.
    seen: Vec<(PpduInfo, Undecoded)>,
    /// PPDUs decoded, with the VHT-SIG-B length.
    decoded: Vec<(PpduInfo, Option<usize>)>,
    /// Non-HT frames.
    non_ht: Vec<Vec<u8>>,
    stats: OfdmStats,
}

/// Runs `y` through the non-HT receiver with the PPDU decoder on its hook.
fn receive(y: &[Complex32]) -> Got {
    let mut rx = OfdmReceiver::new();
    let mut dec = PpduDecoder::new();
    let mut got = Got::default();
    let mut non_ht = Vec::new();
    for piece in y.chunks(4096) {
        rx.process_with(
            piece,
            |f| non_ht.push(f.mpdu.to_vec()),
            |v| match dec.decode(v) {
                Ppdu::NonHt => false,
                Ppdu::Seen { info, why } => {
                    got.seen.push((info, why));
                    true
                }
                Ppdu::Psdu { info, psdu, sig_b_length, .. } => {
                    got.decoded.push((info, sig_b_length));
                    if info.aggregated {
                        for s in Ampdu::new(psdu) {
                            if mac::fcs_ok(s.mpdu) {
                                got.mpdus.push(s.mpdu.to_vec());
                            } else {
                                got.bad += 1;
                            }
                        }
                    } else if mac::fcs_ok(psdu) {
                        got.mpdus.push(psdu.to_vec());
                    } else {
                        got.bad += 1;
                    }
                    true
                }
            },
        );
    }
    got.non_ht = non_ht;
    got.stats = rx.stats();
    got
}

fn noise_of(snr_db: f64) -> f64 {
    10f64.powf(-snr_db / 10.0)
}

fn ht_ppdu(psdu: &[u8], mcs: u8, short_gi: bool, aggregation: bool) -> Vec<Complex32> {
    let sig = HtSig {
        mcs,
        smoothing: true,
        not_sounding: true,
        short_gi,
        aggregation,
        ..HtSig::default()
    };
    let mut p = Vec::new();
    ht::build_ppdu_sig(psdu, sig, Mcs(mcs), 0x5D, PilotPattern::Rotating, &mut p);
    p
}

/// `n` PPDUs made by `make(i)` with gaps, a carrier offset and noise for `snr_db`.
fn burst(n: usize, snr_db: f64, seed: u64, make: impl Fn(usize) -> Vec<Complex32>) -> Vec<Complex32> {
    let mut x = Vec::new();
    for i in 0..n {
        x.extend(impair(&make(i), 20e6, 0.0, 0.0, 700, 0));
    }
    impair(&x, 20e6, 37e3, noise_of(snr_db), 1500, seed)
}

#[test]
fn ht_signal_field_and_rates() {
    // MCS 0 to 7 at 20 MHz with the long guard interval: 6.5 to 65 Mbit/s (table 19-27).
    let rates: Vec<f64> = (0..8)
        .map(|m| HtSig { mcs: m, ..HtSig::default() }.rate_mbps())
        .collect();
    assert_eq!(rates, [6.5, 13.0, 19.5, 26.0, 39.0, 52.0, 58.5, 65.0]);
    let sgi = HtSig { mcs: 7, short_gi: true, ..HtSig::default() };
    assert!((sgi.rate_mbps() - 72.2).abs() < 0.03);
    // Streams by MCS: 0 to 7 one, 8 to 15 two, 16 to 23 three, 24 to 31 four; 33 to 38 two with unequal modulation.
    let streams = |m| HtSig { mcs: m, ..HtSig::default() }.streams();
    assert_eq!((streams(7), streams(8), streams(23), streams(31), streams(33), streams(32)), (1, 2, 3, 4, 2, 0));
}

#[test]
fn ht_every_mcs_both_guard_intervals_and_ampdu() {
    for short_gi in [false, true] {
        for mcs in 0..8u8 {
            // One MPDU, then an A-MPDU of three.
            let single = mpdu(180 + 40 * usize::from(mcs), mcs);
            let parts = [mpdu(90, 1), mpdu(300, 2), mpdu(41, 3)];
            let y = burst(2, 35.0, 3 + u64::from(mcs), |i| match i {
                0 => ht_ppdu(&single, mcs, short_gi, false),
                _ => ht_ppdu(&ampdu(&parts), mcs, short_gi, true),
            });
            let got = receive(&y);
            assert_eq!(got.mpdus.len(), 4, "MCS {mcs} short GI {short_gi}: {:?}", got.seen);
            assert_eq!(got.mpdus[0], single);
            assert_eq!(&got.mpdus[1..], &parts[..]);
            assert_eq!((got.bad, got.non_ht.len()), (0, 0));
            let (info, _) = got.decoded[1];
            assert_eq!((info.format, info.mcs, info.streams, info.short_gi, info.aggregated), (Format::Ht, Some(mcs), 1, short_gi, true));
            assert_eq!(info.bandwidth_mhz, 20);
        }
    }
}

#[test]
fn ht_long_ampdu_beyond_4095_octets() {
    let parts: Vec<Vec<u8>> = (0..9).map(|i| mpdu(1400, i)).collect();
    let a = ampdu(&parts);
    assert!(a.len() > 12_000);
    let got = receive(&burst(1, 35.0, 8, |_| ht_ppdu(&a, 7, true, true)));
    assert_eq!(got.mpdus, parts);
}

#[test]
fn ht_ppdus_that_one_antenna_cannot_decode_are_counted_with_the_reason() {
    let body = mpdu(200, 5);
    let cases: [(HtSig, Undecoded, u8); 5] = [
        (HtSig { mcs: 15, ..HtSig::default() }, Undecoded::Streams, 2),
        (HtSig { mcs: 23, ..HtSig::default() }, Undecoded::Streams, 3),
        (HtSig { mcs: 3, cbw40: true, ..HtSig::default() }, Undecoded::Width, 1),
        (HtSig { mcs: 3, ldpc: true, ..HtSig::default() }, Undecoded::Ldpc, 1),
        (HtSig { mcs: 3, stbc: 1, ..HtSig::default() }, Undecoded::Stbc, 1),
    ];
    let y = burst(cases.len(), 25.0, 4, |i| {
        let mut p = Vec::new();
        ht::build_ppdu_sig(&body, cases[i].0, Mcs(3), 0x11, PilotPattern::Fixed, &mut p);
        p
    });
    let got = receive(&y);
    assert_eq!(got.seen.len(), cases.len());
    for ((info, why), (sig, want, streams)) in got.seen.iter().zip(cases) {
        assert_eq!((*why, info.streams, info.format), (want, streams, Format::Ht));
        assert_eq!(info.mcs, Some(sig.mcs));
        assert_eq!(info.bandwidth_mhz, if sig.cbw40 { 40 } else { 20 });
    }
    // Nothing was decoded as non-HT and nothing was counted as a bad frame.
    assert_eq!((got.mpdus.len(), got.non_ht.len(), got.stats.bad_fcs), (0, 0, 0));
}

#[test]
fn vht_signal_fields() {
    // VHT-SIG-A of a single-user PPDU: the reserved bits are 1, the CRC covers 34 bits, the tail is zero.
    let a = VhtSigA::su(20, 1, 7, true, false, 19);
    assert_eq!((a.bandwidth_mhz(), a.nsts(), a.mcs(), a.short_gi(), a.ldpc()), (20, 1, 7, true, false));
    assert!(a.is_su() && a.short_gi_disambiguation() && !a.stbc());
    assert_eq!(VhtSigA::from_bits(&a.bits()), Ok(a));
    let mut b = a.bits();
    b[5] ^= 1;
    assert_eq!(VhtSigA::from_bits(&b), Err(sig::SigError::Crc));
    let wide = VhtSigA::su(80, 3, 9, false, true, 4);
    assert_eq!((wide.bandwidth_mhz(), wide.nsts(), wide.mcs(), wide.ldpc()), (80, 3, 9, true));
    // Data bits per symbol at 20 MHz (table 21-30): 26 to 312; MCS 9 does not exist for one stream.
    let dbps: Vec<usize> = (0..9).map(|m| VhtMcs(m).n_dbps()).collect();
    assert_eq!(dbps, [26, 52, 78, 104, 156, 208, 234, 260, 312]);
    assert!(!VhtMcs(9).valid_20());
    assert!((VhtMcs(8).rate_mbps(false) - 78.0).abs() < 1e-9);
    assert!((VhtMcs(8).rate_mbps(true) - 86.7).abs() < 0.04);
    // The symbol count comes back from the L-SIG LENGTH, with the disambiguation bit when N_SYM mod 10 is 9.
    for n in 1..200 {
        assert_eq!(vht::n_sym(vht::l_length(n, false), false, false), n);
        assert_eq!(vht::n_sym(vht::l_length(n, true), true, n % 10 == 9), n, "{n}");
        assert_eq!(vht::l_length(n, true) % 3, 0);
    }
    // VHT-SIG-B: the length in units of four octets, three reserved ones.
    let b = sig::vht_sig_b_20(321);
    assert_eq!(sig::vht_sig_b_20_length(&b), 1284);
    assert_eq!(&b[17..], &[1, 1, 1, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn vht_every_mcs_both_guard_intervals() {
    for short_gi in [false, true] {
        for mcs in 0..9u8 {
            let parts = [mpdu(120 + 30 * usize::from(mcs), mcs), mpdu(611, 200)];
            let a = ampdu(&parts);
            let y = burst(1, 38.0, 20 + u64::from(mcs), |_| {
                let mut p = Vec::new();
                vht::build_ppdu(&a, VhtMcs(mcs), short_gi, 0x3A, &mut p);
                p
            });
            let got = receive(&y);
            assert_eq!(got.mpdus, parts, "MCS {mcs} short GI {short_gi}: {:?}", got.seen);
            let (info, sig_b) = got.decoded[0];
            assert_eq!((info.format, info.mcs, info.streams, info.short_gi), (Format::Vht, Some(mcs), 1, short_gi));
            // The CRC in the SERVICE field confirms VHT-SIG-B, which gives the A-MPDU length.
            assert_eq!(sig_b, Some(a.len().div_ceil(4) * 4));
            assert_eq!((got.bad, got.non_ht.len(), got.stats.bad_fcs), (0, 0, 0));
        }
    }
}

/// A VHT PPDU whose VHT-SIG-A is replaced by `a`: the builder's samples with the two VHT-SIG-A symbols of another PPDU that
/// has the same L-SIG.
fn vht_with_sig_a(a: VhtSigA) -> Vec<Complex32> {
    use sdrkit_algo_fec::conv::{codes, Encoder};
    let body = ampdu(&[mpdu(100, 1)]);
    let mut p = Vec::new();
    vht::build_ppdu(&body, VhtMcs(2), false, 0x3A, &mut p);
    // Re-make the two symbols from the bits: rate 1/2, the 6 Mbit/s interleaver, BPSK then BPSK on the Q axis.
    let mut coded = Vec::new();
    Encoder::new(codes::WIFI_K7).encode(&a.bits(), &mut coded);
    let perm = Rate::R6.interleaver();
    let pol = sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity();
    let mut fft = sdrkit_algo_dsp::fft::Fft::inverse(64);
    for s in 0..2 {
        let mut half = [0u8; 48];
        perm.apply(&coded[48 * s..48 * (s + 1)], &mut half).unwrap();
        let mut cells = [Complex32::default(); 48];
        ofdm::map_symbol(&half, ofdm::Modulation::Bpsk, &mut cells).unwrap();
        let mut x = [Complex32::default(); 64];
        for (&k, &c) in ofdm::data_carriers().iter().zip(&cells) {
            x[ofdm::bin(k)] = if s == 1 { Complex32::new(0.0, c.re) } else { c };
        }
        for (k, v) in ofdm::PILOTS {
            x[ofdm::bin(k)] = Complex32::new(v * f32::from(pol[1 + s]), 0.0);
        }
        fft.process(&mut x);
        let at = 320 + 80 + 80 * s;
        let scale = 1.0 / 52f32.sqrt();
        for i in 0..80 {
            p[at + i] = x[(i + 48) % 64] * scale;
        }
    }
    p
}

#[test]
fn vht_ppdus_that_are_not_decoded_are_counted_with_the_reason() {
    let mu = {
        // Group ID 5: a multi-user PPDU with 2 + 1 streams.
        let mut a = VhtSigA::su(20, 1, 0, false, false, 1);
        a.a1 = (a.a1 & !(0x3F << 4) & !(0xFFF << 10)) | 5 << 4 | 2 << 10 | 1 << 13;
        a.a2 &= !(0xFF << 10);
        a.a2 |= u32::from(a.crc().reverse_bits()) << 10;
        a
    };
    let cases = [
        (VhtSigA::su(20, 2, 4, false, false, 1), Undecoded::Streams, 20, 2),
        (VhtSigA::su(80, 1, 4, false, false, 1), Undecoded::Width, 80, 1),
        (VhtSigA::su(20, 1, 4, false, true, 1), Undecoded::Ldpc, 20, 1),
        (VhtSigA::su(20, 1, 9, false, false, 1), Undecoded::Mcs, 20, 1),
        (mu, Undecoded::MultiUser, 20, 3),
    ];
    let y = burst(cases.len(), 25.0, 6, |i| vht_with_sig_a(cases[i].0));
    let got = receive(&y);
    assert_eq!(got.seen.len(), cases.len(), "{:?}", got.stats);
    for ((info, why), (a, want, mhz, streams)) in got.seen.iter().zip(cases) {
        assert_eq!((*why, info.bandwidth_mhz, info.streams, info.format), (want, mhz, streams, Format::Vht));
        assert_eq!(info.sig, SigFields::Vht(a));
    }
    assert_eq!((got.mpdus.len(), got.non_ht.len(), got.stats.bad_fcs), (0, 0, 0));
}

#[test]
fn he_and_eht_signal_fields_read_back() {
    let f = HeSu { mcs: 9, dcm: true, bss_color: 41, bandwidth: 2, gi_ltf: 3, nsts: 2, ldpc: true, uplink: true };
    let a = HeSigA::su(&f);
    assert_eq!(HeSigA::from_bits(&a.bits()), Ok(a));
    let s = HeFormat::Su;
    assert!(a.format_su());
    assert_eq!((a.mcs(s), a.dcm(s), a.bss_color(s), a.bandwidth_mhz(s)), (Some(9), true, 41, 80));
    assert_eq!((a.gi_ltf(s), a.nsts(s), a.ldpc(s), a.uplink(s)), (Some(3), Some(2), Some(true), true));
    let mut b = a.bits();
    b[30] ^= 1;
    assert_eq!(HeSigA::from_bits(&b), Err(sig::SigError::Crc));
    let u = USig::eht(4, false, 17, 1, 0, 3);
    assert_eq!(USig::from_bits(&u.bits()), Ok(u));
    assert_eq!((u.version(), u.bandwidth_mhz(), u.bss_color(), u.ppdu_type(), u.sig_symbols()), (0, 320, 17, 1, 4));
    assert!(!u.uplink() && !u.is_tb());
}

#[test]
fn he_and_eht_ppdus_are_found_and_their_signal_fields_read() {
    let su = HeSigA::su(&HeSu { mcs: 7, bss_color: 23, gi_ltf: 1, nsts: 1, ldpc: true, ..HeSu::default() });
    let su2 = HeSigA::su(&HeSu { mcs: 11, bss_color: 5, bandwidth: 2, nsts: 2, ..HeSu::default() });
    let er = HeSigA::su(&HeSu { mcs: 0, dcm: true, bss_color: 60, nsts: 1, ..HeSu::default() });
    // HE MU: downlink, HE-SIG-B MCS 2, BSS colour 9, 40 MHz, 3 HE-SIG-B symbols.
    let mu = HeSigA { a1: 2 << 1 | 9 << 5 | 1 << 15 | 2 << 18, a2: 0 }.sealed();
    // HE TB: format bit 0, BSS colour 33, 20 MHz.
    let tb = HeSigA { a1: 33 << 1, a2: 0 }.sealed();
    let eht = USig::eht(2, true, 12, 1, 0, 1);
    // L-SIG LENGTH: remainder 1 for HE SU and HE TB, 2 for HE MU and HE ER SU, 0 for EHT.
    let cases: [(u16, [u8; 52], bool); 6] = [
        (301, su.bits(), false),
        (604, su2.bits(), false),
        (302, er.bits(), true),
        (155, mu.bits(), false),
        (100, tb.bits(), false),
        (300, eht.bits(), false),
    ];
    for snr in [25.0, 8.0] {
        let y = burst(cases.len(), snr, 9, |i| {
            let mut p = Vec::new();
            build_he_preamble(cases[i].0, &cases[i].1, cases[i].2, &mut p);
            p
        });
        let got = receive(&y);
        assert_eq!(got.seen.len(), 6, "SNR {snr}: {:?}", got.stats);
        let fmt: Vec<_> = got.seen.iter().map(|(i, w)| (i.format, *w)).collect();
        assert_eq!(
            fmt,
            [
                (Format::HeSu, Undecoded::HeData),
                (Format::HeSu, Undecoded::Streams),
                (Format::HeErSu, Undecoded::HeData),
                (Format::HeMu, Undecoded::MultiUser),
                (Format::HeTb, Undecoded::TriggerBased),
                (Format::Eht, Undecoded::Width),
            ],
            "SNR {snr}"
        );
        let i = &got.seen[0].0;
        assert_eq!((i.mcs, i.bss_color, i.streams, i.ldpc, i.bandwidth_mhz, i.uplink), (Some(7), Some(23), 1, true, 20, Some(false)));
        assert_eq!(got.seen[0].0.sig, SigFields::He(su, HeFormat::Su));
        assert_eq!((got.seen[1].0.bandwidth_mhz, got.seen[1].0.streams), (80, 2));
        assert_eq!(got.seen[2].0.sig, SigFields::He(er, HeFormat::ErSu));
        assert_eq!((got.seen[3].0.bandwidth_mhz, got.seen[3].0.bss_color), (40, Some(9)));
        assert_eq!((got.seen[4].0.bss_color, got.seen[4].0.uplink), (Some(33), Some(true)));
        assert_eq!((got.seen[5].0.bandwidth_mhz, got.seen[5].0.bss_color, got.seen[5].0.uplink), (80, Some(12), Some(true)));
        assert_eq!((got.non_ht.len(), got.stats.bad_fcs), (0, 0));
    }
}

#[test]
fn non_ht_frames_at_6_mbits_are_never_taken_by_the_hook() {
    // 60 non-HT frames at 6 Mbit/s at 7 dB: the receiver with the hook gives the same frames as the receiver without it.
    let y = burst(60, 7.0, 77, |i| {
        let mut p = Vec::new();
        ofdm::build_ppdu(&mpdu(60 + i, i as u8), Rate::R6, 0x21 + i as u8, &mut p).unwrap();
        p
    });
    let with = receive(&y);
    let mut rx = OfdmReceiver::new();
    let mut plain = Vec::new();
    rx.process(&y, |f| plain.push(f.mpdu.to_vec()));
    assert!(plain.len() >= 50, "{}", plain.len());
    assert_eq!(with.non_ht, plain);
    assert_eq!((with.seen.len(), with.decoded.len()), (0, 0));
    assert_eq!(with.stats, rx.stats());
}

/// The share of `n` PPDUs whose every MPDU came through at `snr_db`.
fn success(n: usize, snr_db: f64, want: usize, make: impl Fn(usize) -> Vec<Complex32>) -> f64 {
    let got = receive(&burst(n, snr_db, 1000 + snr_db as u64, make));
    got.mpdus.len() as f64 / (n * want) as f64
}

/// The sensitivity of each mode, stated: the SNR at which at least 90 % of 1000-octet PSDUs come through. The values are 1 dB
/// above the lowest SNR that the measurement `sensitivity_table` found.
const SENSITIVITY_DB: [f64; 8] = [9.0, 11.0, 13.0, 15.0, 18.0, 22.0, 24.0, 25.0];
const VHT8_SENSITIVITY_DB: f64 = 29.0;

#[test]
fn ht_and_vht_decode_at_their_stated_sensitivity() {
    let body = mpdu(972, 7);
    let a = ampdu(std::slice::from_ref(&body));
    for mcs in 0..8u8 {
        let snr = SENSITIVITY_DB[usize::from(mcs)];
        let ht = success(20, snr, 1, |_| ht_ppdu(&body, mcs, false, false));
        assert!(ht >= 0.9, "HT MCS {mcs} at {snr} dB: {ht}");
        let v = success(20, snr, 1, |_| {
            let mut p = Vec::new();
            vht::build_ppdu(&a, VhtMcs(mcs), false, 0x3A, &mut p);
            p
        });
        assert!(v >= 0.9, "VHT MCS {mcs} at {snr} dB: {v}");
    }
    let v = success(20, VHT8_SENSITIVITY_DB, 1, |_| {
        let mut p = Vec::new();
        vht::build_ppdu(&a, VhtMcs(8), false, 0x3A, &mut p);
        p
    });
    assert!(v >= 0.9, "VHT MCS 8: {v}");
    // Far below the sensitivity nothing is decoded and nothing false is given.
    let low = receive(&burst(10, -3.0, 5, |_| ht_ppdu(&body, 7, false, false)));
    assert_eq!(low.mpdus.len(), 0);
}

/// Prints the lowest SNR (1 dB steps) at which 90 % of 40 PPDUs with a 1000-octet PSDU come through, per mode.
/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "a measurement"]
fn sensitivity_table() {
    let body = mpdu(972, 7);
    let a = ampdu(std::slice::from_ref(&body));
    let find = |make: &dyn Fn(usize) -> Vec<Complex32>| -> f64 {
        let mut snr = 0.0;
        while snr < 40.0 && success(40, snr, 1, make) < 0.9 {
            snr += 1.0;
        }
        snr
    };
    for mcs in 0..8u8 {
        let ht = find(&|_| ht_ppdu(&body, mcs, false, false));
        let hts = find(&|_| ht_ppdu(&body, mcs, true, false));
        println!("HT MCS {mcs}: {ht} dB, short GI {hts} dB");
    }
    for mcs in 0..9u8 {
        let v = find(&|_| {
            let mut p = Vec::new();
            vht::build_ppdu(&a, VhtMcs(mcs), false, 0x3A, &mut p);
            p
        });
        println!("VHT MCS {mcs}: {v} dB");
    }
    let su = HeSigA::su(&HeSu { mcs: 7, bss_color: 23, nsts: 1, ..HeSu::default() });
    let mut snr = 0.0;
    loop {
        let y = burst(40, snr, 500 + snr as u64, |_| {
            let mut p = Vec::new();
            build_he_preamble(301, &su.bits(), false, &mut p);
            p
        });
        let got = receive(&y);
        let ok = got.seen.iter().filter(|(i, _)| i.format == Format::HeSu).count();
        if ok >= 36 || snr >= 30.0 {
            println!("HE-SIG-A (RL-SIG found and CRC good): {snr} dB");
            break;
        }
        snr += 1.0;
    }
}

#[test]
fn with_data_decoding_off_ppdus_are_counted_as_busy() {
    let a = ampdu(&[mpdu(300, 1)]);
    let y = burst(2, 30.0, 2, |i| {
        if i == 0 {
            ht_ppdu(&a, 5, false, true)
        } else {
            let mut p = Vec::new();
            vht::build_ppdu(&a, VhtMcs(5), false, 0x3A, &mut p);
            p
        }
    });
    let mut rx = OfdmReceiver::new();
    let mut dec = PpduDecoder::new();
    dec.set_data(false);
    let mut seen = Vec::new();
    rx.process_with(
        &y,
        |_| panic!("no non-HT frame"),
        |v| match dec.decode(v) {
            Ppdu::Seen { info, why } => {
                seen.push((info.format, why, info.mcs));
                true
            }
            _ => false,
        },
    );
    assert_eq!(
        seen,
        [(Format::Ht, Undecoded::Busy, Some(5)), (Format::Vht, Undecoded::Busy, Some(5))]
    );
}

/// Prints the time the receiver and decoder take for back-to-back A-MPDUs of 12 kB: HT at MCS 7 and VHT at MCS 8, both with
/// the short guard interval (the most data bits per sample). Run in a release build with `--ignored --nocapture`.
#[test]
#[ignore = "a measurement"]
fn time_for_dense_traffic_at_20_msps() {
    let parts: Vec<Vec<u8>> = (0..8).map(|i| mpdu(1470, i)).collect();
    let a = ampdu(&parts);
    let y = burst(24, 35.0, 2, |i| {
        if i % 2 == 0 {
            ht_ppdu(&a, 7, true, true)
        } else {
            let mut p = Vec::new();
            vht::build_ppdu(&a, VhtMcs(8), true, 0x3A, &mut p);
            p
        }
    });
    let signal = y.len() as f64 / 20e6;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        let got = receive(&y);
        let spent = t.elapsed().as_secs_f64();
        assert_eq!(got.mpdus.len(), 24 * 8);
        println!("decoded {:.1} ms of signal in {:.1} ms", signal * 1e3, spent * 1e3);
    }
}
