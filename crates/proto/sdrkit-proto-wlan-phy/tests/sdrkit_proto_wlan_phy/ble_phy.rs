//! The LE 2M and LE Coded PHYs and the receiver of any link (Bluetooth Core Specification v5.4, Vol 6 Part B clauses 2.1, 2.2
//! and 3.3). The coded packet is checked against the clause's figures and table 2.1; the encoder below is the register of
//! figure 3.6 written bit by bit.

use super::{cuts, impair};
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_proto_wlan_phy::ble::phy::{
    airtime_s, coded_symbols, packet_bits, pattern_map, FecEncoder, CODED_PREAMBLE,
};
use sdrkit_proto_wlan_phy::ble::{
    self, ad, BleReceiver, ChannelPlan, Coding, Link, LinkKind, Phy, Phys, PduType, Uncoded,
};
use sdrkit_proto_wlan_phy::Complex32;

/// The encoder of figure 3.6 by its polynomials: a0 = b + b(-1) + b(-2) + b(-3), a1 = b + b(-2) + b(-3), from zero.
fn encode_figure(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, &b) in bits.iter().enumerate() {
        let at = |k: usize| if i >= k { bits[i - k] } else { 0 };
        out.push(b ^ at(1) ^ at(2) ^ at(3));
        out.push(b ^ at(2) ^ at(3));
    }
    out
}

#[test]
fn fec_encoder_is_the_figure_and_the_code_the_decoder_uses() {
    // The impulse response reads the generators: a0 = 1111 (G0 = 1 + x + x^2 + x^3), a1 = 1011 (G1 = 1 + x^2 + x^3).
    let mut out = Vec::new();
    FecEncoder::default().encode(&[1, 0, 0, 0], &mut out);
    assert_eq!(out, [1, 1, 1, 0, 1, 1, 1, 1]);
    let bits: Vec<u8> = (0..200u32).map(|i| ((i * 7 + i / 3) % 5 % 2) as u8).collect();
    let mut a = Vec::new();
    let mut enc = FecEncoder::default();
    enc.encode(&bits, &mut a);
    assert_eq!(a, encode_figure(&bits));
    let mut b = Vec::new();
    Encoder::new(ble::coded_fec_code()).encode(&bits, &mut b);
    assert_eq!(a, b);
    // Three zeros bring the register back to zero (the termination sequence).
    enc.encode(&[0, 0, 0], &mut Vec::new());
    assert!(enc.is_zero());
}

#[test]
fn pattern_mapper_is_table_3_1() {
    let mut s = Vec::new();
    pattern_map(&[0, 1], Coding::S8, &mut s);
    assert_eq!(s, [0, 0, 1, 1, 1, 1, 0, 0]);
    s.clear();
    pattern_map(&[0, 1, 1], Coding::S2, &mut s);
    assert_eq!(s, [0, 1, 1]);
    assert_eq!(Coding::from_ci(0), Some(Coding::S8));
    assert_eq!(Coding::from_ci(1), Some(Coding::S2));
    assert_eq!(Coding::from_ci(2), None);
}

#[test]
fn packet_durations_are_those_of_clauses_2_1_and_2_2() {
    // "Packets take between 462 and 17040 us" on LE Coded (table 2.1): an empty PDU at S=2, the longest at S=8.
    assert!((airtime_s(0, Phy::LeCoded, Coding::S2) - 462e-6).abs() < 1e-9);
    assert!((airtime_s(255, Phy::LeCoded, Coding::S8) - 17040e-6).abs() < 1e-9);
    // 44 us is the shortest uncoded packet: LE 2M with an empty PDU.
    assert!((airtime_s(0, Phy::Le2M, Coding::S8) - 44e-6).abs() < 1e-9);
    let link = Link::ADVERTISING;
    let pdu = [0x07u8, 0x01, 0x00];
    let mut s = Vec::new();
    for coding in [Coding::S2, Coding::S8] {
        s.clear();
        coded_symbols(&pdu, &link, 12, coding, &mut s).unwrap();
        let want = airtime_s(1, Phy::LeCoded, coding) * 1e6;
        assert_eq!(s.len() as f64, want.round(), "{coding:?}");
        for r in 0..10 {
            assert_eq!(s[8 * r..8 * r + 8], CODED_PREAMBLE);
        }
    }
}

#[test]
fn le_2m_preamble_is_16_alternating_bits_from_the_access_address_lsb() {
    for aa in [0x8E89_BED6u32, 0x5065_17A3] {
        let link = Link::new(aa, 0x55_5555, LinkKind::Data);
        let mut b = Vec::new();
        packet_bits(&[1, 0], &link, 3, Uncoded::Le2M, &mut b).unwrap();
        assert_eq!(b.len(), 16 + 32 + 40);
        assert_eq!(b[0], (aa & 1) as u8);
        for i in 1..17 {
            assert_ne!(b[i], b[i - 1], "{aa:x} bit {i}");
        }
        let mut one = Vec::new();
        packet_bits(&[1, 0], &link, 3, Uncoded::Le1M, &mut one).unwrap();
        assert_eq!(one[..], b[8..]);
    }
}

/// GFSK (BT 0.5, h 0.5) of `symbols` at `sps` samples per symbol, a few symbols of carrier either side.
fn gfsk(symbols: &[u8], sps: usize) -> Vec<Complex32> {
    let mut levels = vec![0.0f32; 3];
    levels.extend(symbols.iter().map(|&b| 2.0 * b as f32 - 1.0));
    levels.extend([0.0; 3]);
    let mut m = Cpm::gfsk(0.5, 0.5, sps as f64).unwrap();
    let mut s = Vec::new();
    m.push(&levels, &mut s);
    m.flush(&mut s);
    s
}

fn ext_pdu(i: u8, len: usize) -> Vec<u8> {
    // ADV_EXT_IND with an AdvA and AdvData: extended header length 7, AdvMode 0, flag AdvA.
    let mut p = vec![0x07, 0x40 + (len + 8) as u8, 7, 0x01, i, 2, 3, 4, 5, 0xC6];
    let mut d = Vec::new();
    ad::push(&mut d, ad::COMPLETE_NAME, &vec![b'a' + i; len.saturating_sub(2)]).unwrap();
    p.extend_from_slice(&d);
    p[1] = (p.len() - 2) as u8;
    p
}

type Got = Vec<(Vec<u8>, u8, Phy, Option<Coding>, u32)>;

fn run(rx: &mut BleReceiver, pieces: &[&[Complex32]]) -> Got {
    let mut got = Vec::new();
    for p in pieces {
        rx.process(p, |pk| {
            got.push((
                pk.pdu.to_vec(),
                pk.channel,
                pk.phy,
                pk.coding,
                pk.link.access_address,
            ))
        });
    }
    got
}

#[test]
fn coded_packets_decode_at_s2_and_s8_with_offset_and_noise_at_any_cut() {
    let sps = 4;
    let rate = 4e6;
    let link = Link::ADVERTISING;
    let mut x = Vec::new();
    let mut want = Vec::new();
    for (i, coding) in [Coding::S8, Coding::S2, Coding::S2, Coding::S8]
        .into_iter()
        .enumerate()
    {
        let pdu = ext_pdu(i as u8, 10 + 30 * i);
        let mut s = Vec::new();
        coded_symbols(&pdu, &link, 37, coding, &mut s).unwrap();
        x.extend(impair(&gfsk(&s, sps), rate, 0.0, 0.0, 600 * sps, 0));
        want.push((pdu, 37u8, Phy::LeCoded, Some(coding), ble::ADV_ACCESS_ADDRESS));
    }
    // 40 kHz offset and an SNR of about 11 dB in 1 MHz.
    let y = impair(&x, rate, 40e3, 0.08 * sps as f64, 1000, 5);
    let mut a = BleReceiver::new(sps, Phys::PRIMARY).unwrap();
    let whole = run(&mut a, &[&y]);
    assert_eq!(whole, want, "{:?}", a.stats());
    let mut b = BleReceiver::new(sps, Phys::PRIMARY).unwrap();
    assert_eq!(run(&mut b, &cuts(&y, 5000, 9)), whole, "a cut changes the output");
    // An LE 1M receiver alone does not take coded packets.
    let mut c = BleReceiver::new(sps, Phys::LE_1M).unwrap();
    assert!(run(&mut c, &[&y]).is_empty());
    // At S=8 the packets still decode at about 5 dB.
    let weak = impair(&x, rate, 40e3, 0.3 * sps as f64, 1000, 5);
    let mut d = BleReceiver::new(sps, Phys::PRIMARY).unwrap();
    let s8: Vec<_> = want.iter().filter(|w| w.3 == Some(Coding::S8)).cloned().collect();
    let got: Vec<_> = run(&mut d, &[&weak])
        .into_iter()
        .filter(|w| w.3 == Some(Coding::S8))
        .collect();
    assert_eq!(got, s8);
}

#[test]
fn le_2m_packets_decode_on_a_data_link_with_its_crc_preset() {
    for sps in [4usize, 8] {
        let rate = sps as f64 * 1e6;
        let link = Link::new(0x5065_17A3, 0x1A_2B3C, LinkKind::Data);
        let mut x = Vec::new();
        let mut want = Vec::new();
        for i in 0..4u8 {
            // LL data start (LLID 0b10), then the L2CAP bytes.
            let mut pdu = vec![0x02 | (i & 1) << 2, 0];
            pdu.extend((0..(5 + 20 * i as usize)).map(|k| (k as u8).wrapping_mul(29) ^ i));
            pdu[1] = (pdu.len() - 2) as u8;
            let mut b = Vec::new();
            packet_bits(&pdu, &link, 9, Uncoded::Le2M, &mut b).unwrap();
            x.extend(impair(&gfsk(&b, sps / 2), rate, 0.0, 0.0, 400 * sps, 0));
            want.push((pdu, 9u8, Phy::Le2M, None, link.access_address));
        }
        let y = impair(&x, rate, -80e3, 0.03 * sps as f64, 1000, 3);
        let mut rx = BleReceiver::new(sps, Phys::ALL).unwrap();
        rx.set_channels(ChannelPlan::Known(9)).unwrap();
        rx.add_link(link).unwrap();
        let got = run(&mut rx, &cuts(&y, 4000, sps as u64));
        assert_eq!(got, want, "sps {sps}: {:?}", rx.stats());
        // Without the link the packets are not taken.
        let mut other = BleReceiver::new(sps, Phys::ALL).unwrap();
        other.set_channels(ChannelPlan::Known(9)).unwrap();
        assert!(run(&mut other, &[&y]).is_empty());
    }
}

#[test]
fn le_1m_advertising_on_a_secondary_channel_is_taken_on_its_channel() {
    let sps = 4;
    let pdu = ext_pdu(3, 40);
    let mut b = Vec::new();
    packet_bits(&pdu, &Link::ADVERTISING, 21, Uncoded::Le1M, &mut b).unwrap();
    let y = impair(&gfsk(&b, sps), 4e6, 10e3, 0.05, 2000, 1);
    let mut rx = BleReceiver::new(sps, Phys::ALL).unwrap();
    rx.set_channels(ChannelPlan::Known(21)).unwrap();
    let got = run(&mut rx, &[&y]);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, pdu);
    assert_eq!(ble::AdvPdu::parse(&got[0].0).unwrap().header.pdu_type, PduType::AdvExtInd);
    assert!(rx.set_channels(ChannelPlan::Known(40)).is_err());
    assert!(BleReceiver::new(2, Phys::ALL).is_err());
    assert!(BleReceiver::new(3, Phys::ALL).is_err());
    rx.remove_link(ble::ADV_ACCESS_ADDRESS);
    assert_eq!(rx.links().len(), 1);
}

#[test]
fn the_crc_preset_is_recovered_from_a_packet() {
    // The reference packet of Vol 6 Part C clause 2.1: its CRC bytes give back the advertising preset.
    let crc = u32::from_le_bytes([0x29, 0x0A, 0xCE, 0]);
    assert_eq!(ble::crc24_init_of(&[0x00, 0x03, 0x42, 0x4C, 0x45], crc), ble::ADV_CRC_INIT);
    let pdu: Vec<u8> = (0..60u8).map(|i| i.wrapping_mul(29) ^ 0xC3).collect();
    for preset in [0x00_0001, 0x12_3456, 0xA5_0F33, 0xFF_FFFF, 0x80_0000] {
        for n in [2, 3, 17, 60] {
            let crc = ble::crc24(&pdu[..n], preset);
            assert_eq!(ble::crc24_init_of(&pdu[..n], crc), preset, "{preset:x} {n}");
        }
    }
}

#[test]
fn access_addresses_obey_clause_2_1_2() {
    use ble::access_address_ok as ok;
    assert!(ok(0x5065_17A3) && ok(0x5A3C_9617) && ok(0xAF9A_B4D1));
    // The advertising access address, and one bit from it.
    assert!(!ok(ble::ADV_ACCESS_ADDRESS) && !ok(ble::ADV_ACCESS_ADDRESS ^ 0x100));
    assert!(ok(ble::ADV_ACCESS_ADDRESS ^ 0x1100));
    // Seven equal bits in a row (six are allowed).
    assert!(!ok(0x5065_7F23) && ok(0x5065_3F23));
    assert!(!ok(0x5065_8023) && ok(0x5065_8123));
    // Four equal octets.
    assert!(!ok(0x5A5A_5A5A));
    // More than 24 transitions.
    assert!(!ok(0x5555_5553) && !ok(0xA955_5555));
    // Fewer than two transitions in the six most significant bits.
    assert!(!ok(0xF065_17A3) && !ok(0x0165_17A3) && !ok(0xE065_17A3) && ok(0xD065_17A3));
}

/// Data channel packets of two links the receiver does not hold, LE 1M and LE 2M, on channel 12.
type LinkWants = Vec<(u32, u32, [u8; 2], Phy)>;
fn unknown_links(sps: usize) -> (Vec<Complex32>, LinkWants) {
    let links = [
        Link::new(0x5065_17A3, 0x12_3456, LinkKind::Data),
        Link::new(0xAF9A_B4D1, 0xC0_FFEE, LinkKind::Data),
    ];
    let mut x = Vec::new();
    let mut want = Vec::new();
    for i in 0..8usize {
        let (link, phy) = (links[i % 2], if i % 2 == 0 { Uncoded::Le1M } else { Uncoded::Le2M });
        // An empty PDU, an L2CAP start, a control PDU.
        let mut pdu = match i % 3 {
            0 => vec![0x01 | (i as u8 & 1) << 2, 0],
            1 => vec![0x02, 9, 5, 0, 4, 0, 0x1B, 3, 0, 7, 8],
            _ => vec![0x03, 6, 0x0C, 9, 1, 2, 3, 4],
        };
        pdu[0] |= (i as u8 & 2) << 2;
        let mut bits = Vec::new();
        packet_bits(&pdu, &link, 12, phy, &mut bits).unwrap();
        let s = gfsk(&bits, if phy == Uncoded::Le2M { sps / 2 } else { sps });
        x.extend(impair(&s, 4e6, 0.0, 0.0, 700 * sps, 0));
        want.push((link.access_address, link.crc_init, [pdu[0], pdu[1]], phy.phy()));
    }
    (x, want)
}

#[test]
fn packets_of_links_not_held_give_their_access_address_and_crc_preset() {
    let sps = 4;
    let (x, mut want) = unknown_links(sps);
    // 30 kHz offset and an SNR of about 17 dB in 1 MHz.
    let y = impair(&x, 4e6, 30e3, 0.02 * sps as f64, 3000, 11);
    let run = |pieces: &[&[Complex32]], blind: bool| {
        let mut rx = BleReceiver::new(sps, Phys::ALL).unwrap();
        rx.set_channels(ChannelPlan::Known(12)).unwrap();
        rx.set_blind(blind);
        let mut out = Vec::new();
        for p in pieces {
            rx.process(p, |_| panic!("no link is held"));
            rx.take_blind(&mut out);
        }
        // A stray match of the rules inside a packet is no packet of the two links: a tracker drops it, as it comes once.
        let mut got: Vec<_> = out
            .iter()
            .filter(|b| [0x5065_17A3, 0xAF9A_B4D1].contains(&b.access_address))
            .map(|b| (b.access_address, b.crc_init, b.header, b.phy))
            .collect();
        assert!(out.iter().all(|b| b.channel == 12 && (b.header[0] & 3 == 3) == b.opcode.is_some()));
        got.sort_by_key(|g| (g.3 == Phy::Le2M, g.0));
        got
    };
    want.sort_by_key(|g| (g.3 == Phy::Le2M, g.0));
    let whole = run(&[&y], true);
    let mut sorted = whole.clone();
    sorted.sort_by_key(|g| (g.3 == Phy::Le2M, g.0, g.2));
    want.sort_by_key(|g| (g.3 == Phy::Le2M, g.0, g.2));
    assert_eq!(sorted, want);
    let mut cut = run(&cuts(&y, 5000, 3), true);
    cut.sort_by_key(|g| (g.3 == Phy::Le2M, g.0, g.2));
    assert_eq!(cut, want, "a cut changes the output");
    assert!(run(&[&y], false).is_empty());
    // A held link takes its packets itself: the search leaves them alone.
    let mut rx = BleReceiver::new(sps, Phys::ALL).unwrap();
    rx.set_channels(ChannelPlan::Known(12)).unwrap();
    rx.add_link(Link::new(0x5065_17A3, 0x12_3456, LinkKind::Data)).unwrap();
    rx.set_blind(true);
    let (mut good, mut out) = (0, Vec::new());
    rx.process(&y, |_| good += 1);
    rx.take_blind(&mut out);
    assert!(out.iter().all(|b| b.access_address != 0x5065_17A3));
    let other = out.iter().filter(|b| b.access_address == 0xAF9A_B4D1).count();
    assert_eq!((good, other), (4, 4));
}

#[test]
fn noise_gives_no_access_address_twice() {
    let sps = 4;
    let silence = vec![Complex32::default(); 4_000_000];
    let y = impair(&silence, 4e6, 0.0, 0.5, 0, 21);
    let mut rx = BleReceiver::new(sps, Phys::ALL).unwrap();
    rx.set_channels(ChannelPlan::Known(5)).unwrap();
    rx.set_blind(true);
    let mut out = Vec::new();
    rx.process(&y, |_| panic!("noise"));
    rx.take_blind(&mut out);
    let mut aas: Vec<u32> = out.iter().map(|b| b.access_address).collect();
    let n = aas.len();
    aas.sort_unstable();
    aas.dedup();
    assert_eq!(aas.len(), n);
    assert!(n < 40, "{n} candidates in a second of noise");
}
