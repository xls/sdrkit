//! BLE link layer and receiver (Bluetooth Core Specification v5.4, Vol 6 Part B).

use super::{cuts, impair};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_proto_wlan_phy::ble::{self, ad, AdvPdu, AdvReceiver, Body, PduType};
use sdrkit_proto_wlan_phy::Complex32;
use sdrkit_util_bits::crc::catalog::CRC24_BLE;

/// The CRC shift register of Vol 6 Part B figure 3.4, written bit by bit: preset in positions 0 to 23, the data bit XORed with
/// position 23 feeds positions 0, 1, 3, 4, 6, 9, 10. The CRC is sent from position 23 first.
fn crc_figure(pdu: &[u8], preset: u32) -> [u8; 3] {
    let mut s = preset & 0xFF_FFFF;
    for &b in pdu {
        for i in 0..8 {
            let fb = ((s >> 23) ^ (b as u32 >> i)) & 1;
            s = (s << 1) & 0xFF_FFFF;
            if fb == 1 {
                s ^= 0x00_065B;
            }
        }
    }
    let mut out = [0u8; 3];
    for k in 0..24 {
        out[k / 8] |= (((s >> (23 - k)) & 1) as u8) << (k % 8);
    }
    out
}

/// The whitening register of Vol 6 Part B figure 3.5, written bit by bit: position 0 set to 1, positions 1 to 6 the channel
/// index (most significant bit in position 1). The output is position 6, fed back into positions 0 and 4.
fn whiten_figure(data: &mut [u8], channel: u8) {
    let mut s = [0u8; 7];
    s[0] = 1;
    for (i, v) in s.iter_mut().enumerate().skip(1) {
        *v = (channel >> (6 - i)) & 1;
    }
    for b in data.iter_mut() {
        for bit in 0..8 {
            let out = s[6];
            for j in (1..7).rev() {
                s[j] = s[j - 1];
            }
            s[0] = out;
            s[4] ^= out;
            *b ^= out << bit;
        }
    }
}

#[test]
fn crc24_check_value_and_figure() {
    assert_eq!(CRC24_BLE.checksum(b"123456789"), 0xC25A56);
    let pdu: Vec<u8> = (0..39u8).map(|i| i.wrapping_mul(37) ^ 0x5A).collect();
    for preset in [ble::ADV_CRC_INIT, 0x12_3456, 0xA5_0F33] {
        for n in [2, 9, 39] {
            assert_eq!(
                ble::crc24_bytes(&pdu[..n], preset),
                crc_figure(&pdu[..n], preset),
                "{preset:x} {n}"
            );
        }
    }
    let mut p = pdu[..20].to_vec();
    p.extend_from_slice(&ble::crc24_bytes(&pdu[..20], ble::ADV_CRC_INIT));
    assert!(ble::crc_ok(&p));
    p[3] ^= 0x10;
    assert!(!ble::crc_ok(&p));
}

#[test]
fn whitening_matches_the_figure_on_every_channel() {
    for ch in 0..40u8 {
        let mut a: Vec<u8> = (0..40u8).collect();
        let mut b = a.clone();
        ble::whiten(&mut a, ch).unwrap();
        whiten_figure(&mut b, ch);
        assert_eq!(a, b, "channel {ch}");
    }
    // Clause 3.2: the sequence repeats every 127 bits.
    let mut z = vec![0u8; 32];
    ble::whiten(&mut z, 37).unwrap();
    let bit = |i: usize| (z[i / 8] >> (i % 8)) & 1;
    assert!((0..100).all(|i| bit(i) == bit(i + 127)));
    assert!(ble::whiten(&mut [0], 40).is_err());
}

#[test]
fn channels_and_access_address() {
    assert_eq!(ble::channel_hz(37), Some(2402e6));
    assert_eq!(ble::channel_hz(38), Some(2426e6));
    assert_eq!(ble::channel_hz(39), Some(2480e6));
    assert_eq!(ble::channel_hz(0), Some(2404e6));
    assert_eq!(ble::channel_hz(36), Some(2478e6));
    assert_eq!(ble::channel_hz(40), None);
    assert_eq!(ble::advertising_channel_at(2426.05e6, 0.1e6), Some(38));
    // Clause 2.1.1: the preamble alternates into the access address, whose first bit sent (bit 0 of 0x8E89BED6) is 0.
    assert_eq!(ble::preamble(ble::ADV_ACCESS_ADDRESS), 0xAA);
    let mut bits = Vec::new();
    ble::air_bits(&[0x42, 0], 37, &mut bits).unwrap();
    assert_eq!(&bits[..8], &[0, 1, 0, 1, 0, 1, 0, 1]);
    assert_eq!(bits[7], 1);
    assert_eq!(bits[8], 0); // access address bit 0
    let aa: u32 = (0..32).fold(0, |a, i| a | (bits[8 + i] as u32) << i);
    assert_eq!(aa, 0x8E89_BED6);
    // The dewhitened rest is the PDU and its CRC.
    let mut rest = Vec::new();
    sdrkit_util_bits::pack::pack(
        &bits[40..],
        sdrkit_util_bits::BitOrder::Lsb,
        sdrkit_util_bits::Pad::Zero,
        &mut rest,
    );
    ble::whiten(&mut rest, 37).unwrap();
    assert_eq!(&rest[..2], &[0x42, 0]);
    assert!(ble::crc_ok(&rest));
}

#[test]
fn pdus_parse_and_build() {
    let adv_a = ble::Addr::random([0x11, 0x22, 0x33, 0x44, 0x55, 0xC6]);
    let mut pdu = Vec::new();
    ble::build_adv(&mut pdu, PduType::AdvScanInd, &adv_a, &[2, 1, 6]).unwrap();
    assert_eq!(pdu[0], 0x46); // type 6, TxAdd
    let p = AdvPdu::parse(&pdu).unwrap();
    assert_eq!(p.adv_a(), Some(adv_a));
    assert_eq!(p.adv_a().unwrap().kind(), ble::AddrKind::RandomStatic);
    assert_eq!(p.adv_a().unwrap().text(), "C6:55:44:33:22:11");
    // Wrong lengths are refused.
    assert!(AdvPdu::parse(&[0x00, 5, 1, 2, 3, 4, 5]).is_err());
    assert!(AdvPdu::parse(&[0x01, 11]).is_err());
    // CONNECT_IND.
    let mut c = vec![0u8; 34];
    c[12..16].copy_from_slice(&0xAF9A_9357u32.to_le_bytes());
    c[16..19].copy_from_slice(&[0x11, 0x22, 0x33]);
    c[22..24].copy_from_slice(&24u16.to_le_bytes());
    c[28..33].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x1F]);
    c[33] = 0xA7;
    let mut pdu = Vec::new();
    let h = ble::Header {
        pdu_type: PduType::ConnectInd,
        ch_sel: true,
        tx_add: false,
        rx_add: true,
        length: 0,
    };
    ble::build_pdu(&mut pdu, h, &c).unwrap();
    match AdvPdu::parse(&pdu).unwrap().body {
        Body::Connect { ll, adv_a, .. } => {
            assert_eq!(ll.access_address, 0xAF9A_9357);
            assert_eq!(ll.crc_init, 0x33_2211);
            assert_eq!(ll.interval, 24);
            assert_eq!((ll.hop, ll.sca), (7, 5));
            assert!(adv_a.random);
        }
        b => panic!("{b:?}"),
    }
    // ADV_EXT_IND with AdvA, ADI, AuxPtr and TxPower.
    let mut e = vec![0u8];
    let hdr = [
        0x01 | 0x08 | 0x10 | 0x40,
        1,
        2,
        3,
        4,
        5,
        6,
        0x34,
        0x52,
        0x05,
        0,
        0x40,
        0xF6,
    ];
    e[0] = hdr.len() as u8 | 0x40;
    e.extend_from_slice(&hdr);
    e.extend_from_slice(&[2, 1, 6]);
    let mut pdu = Vec::new();
    let h = ble::Header {
        pdu_type: PduType::AdvExtInd,
        ch_sel: false,
        tx_add: true,
        rx_add: false,
        length: 0,
    };
    ble::build_pdu(&mut pdu, h, &e).unwrap();
    match AdvPdu::parse(&pdu).unwrap().body {
        Body::Extended { ext, data } => {
            assert_eq!(ext.adv_mode, 1);
            assert_eq!(ext.adv_a.unwrap().bytes, [1, 2, 3, 4, 5, 6]);
            assert_eq!(ext.adi, Some((0x234, 5)));
            assert_eq!(ext.aux_ptr, Some((5, 2)));
            assert_eq!(ext.tx_power, Some(-10));
            assert_eq!(data, &[2, 1, 6]);
        }
        b => panic!("{b:?}"),
    }
    // An extended header field past its end is refused, never read.
    let bad = [0x07, 3, 2, 0x01, 1];
    assert!(AdvPdu::parse(&bad).is_err());
}

#[test]
fn advertising_data() {
    let mut d = Vec::new();
    ad::push(&mut d, ad::FLAGS, &[0x06]).unwrap();
    ad::push(&mut d, ad::COMPLETE_NAME, b"Tag").unwrap();
    ad::push_uuids16(&mut d, &[0x180F, 0xFEAA]).unwrap();
    ad::push_service_data16(&mut d, 0xFFFA, &[0x0D, 7, 1, 2]).unwrap();
    ad::push_manufacturer(&mut d, 0x0006, &[9, 9]).unwrap();
    ad::push(&mut d, ad::TX_POWER, &[0xF4]).unwrap();
    d.extend_from_slice(&[0, 0, 0]); // padding
    let v: Vec<ad::AdValue> = ad::structures(&d).map(|a| a.value()).collect();
    assert_eq!(v.len(), 6);
    assert_eq!(v[0], ad::AdValue::Flags(6));
    assert_eq!(v[1].name_text().unwrap(), "Tag");
    match v[2] {
        ad::AdValue::Uuids { complete, list } => {
            assert!(complete);
            assert_eq!(list.values().collect::<Vec<_>>(), [0x180F, 0xFEAA]);
        }
        x => panic!("{x:?}"),
    }
    assert_eq!(
        ad::service_data16(&d, 0xFFFA).next(),
        Some(&[0x0D, 7, 1, 2][..])
    );
    assert_eq!(
        v[4],
        ad::AdValue::Manufacturer {
            company: 6,
            data: &[9, 9]
        }
    );
    assert_eq!(ad::company_name(0x0006), Some("Microsoft"));
    assert_eq!(v[5], ad::AdValue::TxPower(-12));
    // A structure running past the end stops the walk.
    let t = [3, ad::FLAGS, 6];
    let mut it = ad::structures(&t);
    assert!(it.next().is_none());
    assert!(it.truncated());
}

/// GFSK samples of a packet at `sps` samples per bit: bit 1 the higher frequency, BT 0.5, index 0.5.
fn modulate(pdu: &[u8], channel: u8, sps: usize) -> Vec<Complex32> {
    let mut bits = Vec::new();
    ble::air_bits(pdu, channel, &mut bits).unwrap();
    let levels: Vec<f32> = bits.iter().map(|&b| 2.0 * b as f32 - 1.0).collect();
    let mut m = Cpm::gfsk(ble::BT, 0.5, sps as f64).unwrap();
    let mut out = Vec::new();
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

fn packets(sps: usize) -> Vec<(Vec<u8>, u8)> {
    let mut v = Vec::new();
    for (i, ch) in [37u8, 38, 39, 37, 38].into_iter().enumerate() {
        let mut data = Vec::new();
        ad::push(
            &mut data,
            ad::COMPLETE_NAME,
            format!("node {i} {sps}").as_bytes(),
        )
        .unwrap();
        ad::push_manufacturer(&mut data, 0x004C, &[i as u8; 6]).unwrap();
        let mut pdu = Vec::new();
        ble::build_adv(
            &mut pdu,
            PduType::AdvInd,
            &ble::Addr::public([i as u8, 2, 3, 4, 5, 6]),
            &data,
        )
        .unwrap();
        v.push((pdu, ch));
    }
    v
}

fn run(rx: &mut AdvReceiver, pieces: &[&[Complex32]]) -> Vec<(Vec<u8>, u8, u64)> {
    let mut got = Vec::new();
    for p in pieces {
        rx.process(p, |pk| got.push((pk.pdu.to_vec(), pk.channel, pk.sample)));
    }
    got
}

#[test]
fn receiver_decodes_packets_with_offset_and_noise_at_any_cut() {
    for sps in [2usize, 4, 8] {
        let rate = sps as f64 * 1e6;
        let pk = packets(sps);
        let mut x = Vec::new();
        for (pdu, ch) in &pk {
            x.extend(impair(
                &modulate(pdu, *ch, sps),
                rate,
                0.0,
                0.0,
                300 * sps,
                0,
            ));
        }
        // 60 kHz offset and an SNR of about 15 dB in the bit bandwidth.
        let noise = 0.03 * sps as f64;
        let y = impair(&x, rate, 60e3, noise, 1000, 7);
        let mut a = AdvReceiver::new(sps).unwrap();
        let whole = run(&mut a, &[&y]);
        assert_eq!(whole.len(), pk.len(), "sps {sps}: {:?}", a.stats());
        for ((pdu, ch), (got, gch, _)) in pk.iter().zip(&whole) {
            assert_eq!(got, pdu);
            assert_eq!(gch, ch);
        }
        let mut b = AdvReceiver::new(sps).unwrap();
        assert_eq!(
            run(&mut b, &cuts(&y, 3000, sps as u64)),
            whole,
            "sps {sps}: cut changes the output"
        );
        let mut c = AdvReceiver::new(sps).unwrap();
        c.set_channel_hint(Some(38)).unwrap();
        let mut off = 0.0;
        c.process(&y, |p| off = p.offset_hz);
        assert!((off - 60e3).abs() < 15e3, "{off}");
    }
    assert!(AdvReceiver::new(1).is_err());
    assert!(AdvReceiver::new(4)
        .unwrap()
        .set_channel_hint(Some(12))
        .is_err());
}

#[test]
fn receiver_ignores_noise_and_nan() {
    let mut rx = AdvReceiver::new(4).unwrap();
    let mut x = impair(&[], 4e6, 0.0, 1.0, 200_000, 3);
    x[1000] = Complex32::new(f32::NAN, 0.0);
    let mut n = 0;
    rx.process(&x, |_| n += 1);
    assert_eq!(n, 0);
    assert_eq!(rx.stats().packets, 0);
}

#[test]
fn packet_level_is_the_power_of_the_preamble_and_access_address() {
    let (pdu, ch) = &packets(4)[0];
    // GFSK has a constant envelope of 1, so 0 dBFS.
    let x = modulate(pdu, *ch, 4);
    for gain in [1.0f32, 0.1, 0.01] {
        let y = impair(&super::scaled(&x, gain), 4e6, 0.0, 1e-8, 2000, 3);
        let mut lv = Vec::new();
        AdvReceiver::new(4)
            .unwrap()
            .process(&y, |p| lv.push(p.level_dbfs));
        let want = 20.0 * (gain as f64).log10();
        assert_eq!(lv.len(), 1);
        assert!((lv[0] - want).abs() < 0.2, "gain {gain}: {} {want}", lv[0]);
    }
}
