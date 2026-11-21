//! 802.11a/g OFDM (IEEE Std 802.11-2020 clause 17 and the worked example of annex I).

use super::{cuts, impair};
use sdrkit_proto_wlan_phy::mac;
use sdrkit_proto_wlan_phy::ofdm::{self, DataDecoder, Modulation, OfdmReceiver, Rate, Signal};
use sdrkit_proto_wlan_phy::Complex32;
use sdrkit_util_bits::seq;

/// The PSDU of annex I.1.2: a MAC header, 72 octets of text (as printed, "insired" included) and the FCS.
fn annex_i_psdu() -> Vec<u8> {
    let mut p = vec![
        0x04, 0x02, 0x00, 0x2E, 0x00, 0x60, 0x08, 0xCD, 0x37, 0xA6, 0x00, 0x20, 0xD6, 0x01, 0x3C,
        0xF1, 0x00, 0x60, 0x08, 0xAD, 0x3B, 0xAF, 0x00, 0x00,
    ];
    p.extend_from_slice(
        b"Joy, bright spark of divinity,\nDaughter of Elysium,\nFire-insired we trea",
    );
    p.extend_from_slice(&[0x67, 0x33, 0x21, 0xB6]);
    p
}

#[test]
fn annex_i_message_fcs_and_signal() {
    let p = annex_i_psdu();
    assert_eq!(p.len(), 100);
    // The FCS printed in the annex is the CRC-32 of the 96 octets before it.
    assert!(mac::fcs_ok(&p));
    // Table I-7: 36 Mbit/s, LENGTH 100.
    let s = Signal {
        rate: Rate::R36,
        length: 100,
    };
    assert_eq!(
        s.bits(),
        [1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(Rate::R36.n_sym(100), 6);
    // The annex frame control is 0x0204 (type field 01). The example is about the PHY, not a real MAC frame.
    assert_eq!(
        mac::FrameControl::from_bytes([p[0], p[1]]).ftype,
        mac::FrameType::Control
    );
}

#[test]
fn rate_table_17_4() {
    let want = [
        (6, [1, 1, 0, 1], 1, 48, 24),
        (9, [1, 1, 1, 1], 1, 48, 36),
        (12, [0, 1, 0, 1], 2, 96, 48),
        (18, [0, 1, 1, 1], 2, 96, 72),
        (24, [1, 0, 0, 1], 4, 192, 96),
        (36, [1, 0, 1, 1], 4, 192, 144),
        (48, [0, 0, 0, 1], 6, 288, 192),
        (54, [0, 0, 1, 1], 6, 288, 216),
    ];
    for (r, w) in Rate::ALL.iter().zip(want) {
        assert_eq!(
            (r.mbps(), r.rate_bits(), r.n_bpsc(), r.n_cbps(), r.n_dbps()),
            w
        );
        assert_eq!(Rate::from_rate_bits(w.1), Some(*r));
        // N_DBPS bits every 4 µs.
        assert_eq!(r.n_dbps() as u32, 4 * r.mbps());
    }
    // Every SIGNAL field of every rate reads back. A flipped bit fails the parity.
    for r in Rate::ALL {
        for len in [1u16, 100, 1500, 4095] {
            let s = Signal {
                rate: r,
                length: len,
            };
            let mut b = s.bits();
            assert_eq!(Signal::from_bits(&b).unwrap(), s);
            b[7] ^= 1;
            assert!(Signal::from_bits(&b).is_err());
        }
    }
}

#[test]
fn scrambler_sequence_of_17_3_5_5() {
    // The 127-bit sequence from the all-ones state, as printed in clause 17.3.5.5.
    let want = "0000111011110010110010010000001000100110001011101011011000001100110101001110011110110100001010101111101001010001101110001111111";
    let mut l = seq::ieee80211(0x7F);
    let got: String = (0..127).map(|_| char::from(b'0' + l.bit())).collect();
    assert_eq!(got, want);
    // The seed comes back from the first seven outputs (the scrambled SERVICE bits 0 to 6).
    for seed in 1..128u8 {
        let mut l = seq::ieee80211(seed);
        let first: Vec<u8> = (0..7).map(|_| l.bit()).collect();
        assert_eq!(ofdm::scrambler_seed(&first).unwrap(), seed);
    }
    assert!(ofdm::scrambler_seed(&[0; 7]).is_err());
}

#[test]
fn training_fields() {
    // The L-LTF time symbol has unit mean power at the PPDU scale (1/sqrt(52) per used carrier).
    let l = ofdm::ltf_time();
    let p: f32 = l.iter().map(|v| v.norm_sqr()).sum::<f32>() / 64.0;
    assert!((p - 1.0).abs() < 1e-3, "{p}");
    assert_eq!(ofdm::LTF[26], 0);
    assert_eq!(ofdm::LTF.iter().filter(|&&v| v != 0).count(), 52);
    // The STF has 12 used carriers and a period of 16 samples.
    let mut ppdu = Vec::new();
    ofdm::build_ppdu(&[1, 2, 3], Rate::R6, 0x5D, &mut ppdu).unwrap();
    for i in 16..160 {
        assert!((ppdu[i] - ppdu[i - 16]).norm() < 1e-5);
    }
    for i in 0..64 {
        assert!((ppdu[192 + i] - ppdu[256 + i]).norm() < 1e-5);
    }
    assert_eq!(ppdu.len(), 320 + 80 * (1 + Rate::R6.n_sym(3)));
    assert_eq!(ofdm::data_carriers()[0], -26);
    assert_eq!(ofdm::data_carriers()[47], 26);
}

#[test]
fn coding_chain_round_trips_at_every_rate() {
    let psdu: Vec<u8> = (0..333u32).map(|i| (i * 89 + 5) as u8).collect();
    let mut dec = DataDecoder::new();
    for r in Rate::ALL {
        for seed in [1u8, 0x5D, 0x7F] {
            let mut coded = Vec::new();
            ofdm::encode_data(&psdu, r, seed, &mut coded).unwrap();
            assert_eq!(coded.len(), r.n_sym(psdu.len()) * r.n_cbps());
            // Map and demap without noise, then deinterleave: the sign of each LLR is the coded bit.
            let perm = r.interleaver();
            let mut llr = Vec::new();
            for sym in coded.chunks_exact(r.n_cbps()) {
                let mut cells = [Complex32::default(); 48];
                ofdm::map_symbol(sym, r.modulation(), &mut cells).unwrap();
                let c = sdrkit_algo_modem::Constellation::new(r.modulation().points()).unwrap();
                let mut l = vec![0f32; r.n_cbps()];
                c.demap_block(&cells, 0.1, sdrkit_algo_modem::Demap::MaxLog, &mut l)
                    .unwrap();
                let mut d = vec![0f32; r.n_cbps()];
                perm.invert(&l, &mut d).unwrap();
                llr.extend(d);
            }
            let (got, s) = dec.decode(&llr, r, psdu.len()).unwrap();
            assert_eq!(got, &psdu[..], "{r:?}");
            assert_eq!(s, seed);
        }
    }
    assert!(ofdm::encode_data(&[], Rate::R6, 1, &mut Vec::new()).is_err());
    assert!(ofdm::encode_data(&[1], Rate::R6, 0, &mut Vec::new()).is_err());
    assert!(dec.decode(&[0.0; 47], Rate::R6, 1).is_err());
}

#[test]
fn constellations_follow_17_3_5_8() {
    // BPSK: 0 is -1. QPSK: b0 on I, b1 on Q, 0 negative, scaled 1/sqrt(2). 16-QAM: 00 -3, 01 -1, 11 1, 10 3 over sqrt(10).
    let p = Modulation::Bpsk.points();
    assert_eq!(p.points()[0], Complex32::new(-1.0, 0.0));
    let q = Modulation::Qpsk.points();
    let s = std::f32::consts::FRAC_1_SQRT_2;
    assert!((q.points()[0b10] - Complex32::new(s, -s)).norm() < 1e-6);
    let m = Modulation::Qam16.points();
    let k = 10f32.sqrt();
    assert!((m.points()[0b0010] - Complex32::new(-3.0 / k, 3.0 / k)).norm() < 1e-5);
}

fn frames(n: usize, len: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|i| {
            let mut f = Vec::new();
            mac::push_mgmt_header(
                &mut f,
                mac::MgmtSubtype::Beacon,
                mac::BROADCAST,
                [2, 0, 0, 0, 0, i as u8],
                [2, 0, 0, 0, 0, i as u8],
                i as u16,
            );
            mac::push_beacon_fields(&mut f, 0, 100, 0x0431);
            mac::push_element(&mut f, mac::element::SSID, format!("net {i}").as_bytes()).unwrap();
            while f.len() < len {
                f.push((f.len() * 7 + i) as u8);
            }
            mac::push_fcs(&mut f);
            f
        })
        .collect()
}

#[test]
fn receiver_decodes_every_rate_with_offset_noise_and_any_cut() {
    let fs = frames(8, 300);
    let mut x = Vec::new();
    for (i, (f, r)) in fs.iter().zip(Rate::ALL).enumerate() {
        let mut p = Vec::new();
        ofdm::build_ppdu(f, r, 1 + 13 * i as u8, &mut p).unwrap();
        x.extend(impair(&p, 20e6, 0.0, 0.0, 600, 0));
    }
    // 40 kHz offset, SNR 30 dB.
    let y = impair(&x, 20e6, 40e3, 1e-3, 2000, 11);
    let mut a = OfdmReceiver::new();
    let mut whole = Vec::new();
    a.process(&y, |f| {
        whole.push((f.mpdu.to_vec(), f.signal.rate, f.seed, f.sample))
    });
    assert_eq!(whole.len(), 8, "{:?}", a.stats());
    for (i, (f, (mpdu, r, seed, _))) in fs.iter().zip(&whole).enumerate() {
        assert_eq!(mpdu, &f[..f.len() - 4]);
        assert_eq!(*r, Rate::ALL[i]);
        assert_eq!(*seed, 1 + 13 * i as u8);
    }
    let mut b = OfdmReceiver::new();
    let mut cut = Vec::new();
    for p in cuts(&y, 5000, 3) {
        b.process(p, |f| {
            cut.push((f.mpdu.to_vec(), f.signal.rate, f.seed, f.sample))
        });
    }
    assert_eq!(cut, whole);
    let mut off = 0.0;
    OfdmReceiver::new().process(&y, |f| off = f.offset_hz);
    assert!((off - 40e3).abs() < 2e3, "{off}");
}

#[test]
fn receiver_decodes_6_mbits_at_low_snr() {
    let fs = frames(4, 100);
    let mut x = Vec::new();
    for f in &fs {
        let mut p = Vec::new();
        ofdm::build_ppdu(f, Rate::R6, 0x5D, &mut p).unwrap();
        x.extend(impair(&p, 20e6, 0.0, 0.0, 800, 0));
    }
    // SNR about 6 dB over the 20 MHz: BPSK rate 1/2 needs about 3 dB.
    let y = impair(&x, 20e6, -100e3, 0.25, 2000, 5);
    let mut rx = OfdmReceiver::new();
    let mut n = 0;
    rx.process(&y, |_| n += 1);
    assert_eq!(n, 4, "{:?}", rx.stats());
}

#[test]
fn frame_level_is_the_power_of_the_long_training_field() {
    let f = &frames(1, 200)[0];
    let mut p = Vec::new();
    ofdm::build_ppdu(f, Rate::R12, 0x5D, &mut p).unwrap();
    // The long training field is samples 160 to 320 of the PPDU (17.3.3).
    let ltf_db = super::power_db(&p[160..320]);
    for gain in [1.0f32, 0.1, 0.01] {
        let y = impair(&super::scaled(&p, gain), 20e6, 0.0, 1e-8, 1000, 1);
        let mut lv = Vec::new();
        OfdmReceiver::new().process(&y, |f| lv.push(f.level_dbfs));
        let want = ltf_db + 20.0 * (gain as f64).log10();
        assert_eq!(lv.len(), 1);
        assert!((lv[0] - want).abs() < 0.2, "gain {gain}: {} {want}", lv[0]);
    }
}
