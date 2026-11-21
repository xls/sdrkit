//! The shared signal processing the stacks rely on: the Wi-Fi channel filter, long OFDM frames, the DroneID
//! decoder, bit packing and the catalogue CRCs.

use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;
use sdrkit_proto_wlan_phy::{mac, ofdm};
use sdrkit_stack_drones::dji::droneid::{Coder, INNER_CRC};
use sdrkit_stack_drones::dji::generator::packet;
use sdrkit_stack_drones::remote_id::phy::wifi_channelizer;

fn power(x: &[Complex32]) -> f64 {
    x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len().max(1) as f64
}

/// At a rate other than 20 Msps, the Wi-Fi channel filter rejects a tone outside the channel by more than 40 dB.
#[test]
fn the_wifi_channel_rejects_what_lies_outside_it_at_other_rates() {
    let rate = 25e6;
    let n = 250_000;
    let mut tone = vec![Complex32::default(); n];
    Nco::new(rate, 11.5e6).fill(&mut tone);
    let mut c = wifi_channelizer(rate, 0.0).unwrap();
    let mut y = Vec::new();
    c.process(&tone, &mut y);
    let p = power(&y[1000..]);
    assert!(10.0 * p.log10() < -40.0, "{} dB", 10.0 * p.log10());
}

/// The longest OFDM frame (4095 bytes, about 33 000 trellis steps) decodes.
#[test]
fn the_longest_ofdm_frame_decodes() {
    let mut psdu: Vec<u8> = (0..4091u32).map(|i| (i * 7 + 3) as u8).collect();
    mac::push_fcs(&mut psdu);
    let mut x = vec![Complex32::default(); 400];
    ofdm::build_ppdu(&psdu, ofdm::Rate::R6, 0x5D, &mut x).unwrap();
    x.extend(std::iter::repeat_n(Complex32::default(), 2000));
    let mut rx = ofdm::OfdmReceiver::new();
    let mut got = None;
    rx.process(&x, |f| got = Some(f.mpdu.to_vec()));
    assert_eq!(got.as_deref(), Some(&psdu[..psdu.len() - 4]));
}

/// Once warm, the DroneID decoder allocates nothing for a frame.
#[test]
fn the_droneid_decoder_does_not_allocate_per_frame() {
    let mut c = Coder::new();
    let f = packet(2, 0.1).frame();
    let bits = c.encode(&f, 0);
    let soft: Vec<f32> = bits
        .iter()
        .map(|&b| if b == 0 { 1.0 } else { -1.0 })
        .collect();
    assert!(c.decode(&soft, 0, 8).is_some());
    let before = super::allocations();
    let (g, _) = c.decode(&soft, 0, 8).unwrap();
    assert_eq!(super::allocations() - before, 0);
    assert_eq!(g, f);
}

/// A partial byte is padded with zeros, and Manchester chips come two for each bit.
#[test]
fn a_partial_byte_is_padded_not_dropped() {
    let bits = [1u8, 0, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0];
    let mut out = Vec::new();
    sdrkit_util_bits::pack::pack(
        &bits,
        sdrkit_util_bits::BitOrder::Msb,
        sdrkit_util_bits::Pad::Zero,
        &mut out,
    );
    assert_eq!(out, [0xB1, 0xE0]);
    let c = sdrkit_stack_drones::flarm::proto::manchester(&[0xF0]);
    assert_eq!(c.len(), 16);
}

/// The stacks use the catalogue CRCs, each with its check value.
#[test]
fn the_stacks_use_catalogue_crcs_by_name() {
    use sdrkit_util_bits::crc::catalog::{CRC16_IBM_3740, CRC24_LTE_A};
    // FLARM: CRC-16/IBM-3740, check value 0x29B1.
    assert_eq!(sdrkit_stack_drones::flarm::proto::crc(b"123456789"), 0x29B1);
    assert_eq!(CRC16_IBM_3740.checksum(b"123456789"), 0x29B1);
    // DroneID: CRC-24/LTE-A (3GPP TS 36.212 clause 5.1.1), check value 0xCDE703; the CRC over the whole frame is
    // 0.
    assert_eq!(CRC24_LTE_A.checksum(b"123456789"), 0xCD_E703);
    let f = packet(0, 0.05).frame();
    assert_eq!(CRC24_LTE_A.checksum(&f), 0);
    // The DroneID inner CRC: CRC-16/MCRF4XX with its own start value, named once.
    assert_eq!(INNER_CRC.checksum(b"123456789"), 0x7109);
}
