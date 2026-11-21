//! Edge cases of the physical layer, the parsers and the transmitter: frames at the limits, damaged input and bad parameters.

use sdrkit_proto_lora_phy::{
    ChannelRx, Codec, DetectorConfig, Header, Layout, LoraParams, Payload, Recovery, SymbolSoft,
    TxFrame,
};
use sdrkit_stack_lora::aprs::info::parse_info;
use sdrkit_stack_lora::proto::lorawan::network;

/// A DevAddr of NetID type 0 with NwkID `id` (TS002 clause 6.1: a 0 bit, then 6 bits of NwkID).
fn type0(id: u32) -> u32 {
    id << 25 | 0x0001_2345
}

/// The NetID names follow the LoRa Alliance NetID list (0x09, 0x0F, 0x10, 0x11, 0x12, 0x24).
#[test]
fn netid_names_follow_the_lora_alliance_list() {
    let name = |id| network(type0(id)).2;
    assert_eq!(name(0x09), "Netmore");
    assert_eq!(name(0x0F), "Orange");
    assert_eq!(name(0x10), "A2A Smart City");
    assert_eq!(name(0x12), "Kerlink");
    assert_eq!(name(0x13), "The Things Network");
    // NetIDs that R119 does not allocate get their number, not a guessed operator.
    assert_eq!(name(0x11), "NetID 0x000011");
    assert_eq!(name(0x24), "NetID 0x000024");
    assert_eq!(network(type0(0x0F)).0, Some(0));
    assert_eq!(network(type0(0x0F)).1, Some(0x0F));
}

/// The payload CRC is CRC-16/XMODEM, check value 0x31C3. The LoRa CRC covers all bytes but the last two and XORs them in, so
/// "123456789" with two zero bytes produces the check value.
#[test]
fn payload_crc_is_crc16_xmodem() {
    let mut d = b"123456789".to_vec();
    d.extend_from_slice(&[0, 0]);
    assert_eq!(sdrkit_proto_lora_phy::coding::payload_crc(&d), 0x31C3);
    let e = sdrkit_util_bits::crc::catalog::entry("CRC-16/XMODEM").expect("in the catalogue");
    assert_eq!(e.spec().check, 0x31C3);
}

/// A raw GPS packet with a wrong NMEA `*hh` checksum produces no position.
#[test]
fn nmea_with_a_wrong_checksum_returns_no_position() {
    let good = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A";
    let i = parse_info(good.as_bytes(), "APRS").unwrap();
    assert!(!i.nmea_bad);
    assert!((i.lat.unwrap() - 48.1173).abs() < 1e-4);
    let bad = good.replace("*6A", "*00");
    let i = parse_info(bad.as_bytes(), "APRS").unwrap();
    assert!(i.nmea_bad);
    assert!(i.lat.is_none() && i.lon.is_none());
    assert!(i.summary.contains("checksum wrong"), "{}", i.summary);
}

/// A soft frame at SF8 4/5. Two symbols of the second block (8 and 9) sit one and 34 bins from the right bin, with the right bin
/// a close runner-up. Soft decoding alone does not repair this frame.
fn damaged() -> (Codec, Vec<SymbolSoft>, Vec<u8>, Header) {
    let sf = 8u8;
    let n = 1u32 << sf;
    let codec = Codec::new(sf).unwrap();
    let payload: Vec<u8> = (0..30).map(|i| (i * 11 + 5) as u8).collect();
    let mut s = Vec::new();
    codec
        .encode(
            &payload,
            &Layout::new(sf, false, false).unwrap(),
            5,
            true,
            &mut s,
        )
        .unwrap();
    let h = Header {
        len: 30,
        cr: 5,
        crc: true,
    };
    let soft = |b: u16, second: u16, ll: [f32; 4]| SymbolSoft {
        bins: [
            b,
            second,
            ((b as u32 + 13) % n) as u16,
            ((b as u32 + 29) % n) as u16,
        ],
        ll,
        count: 4,
        floor: 0.0,
    };
    let mut syms: Vec<SymbolSoft> = s
        .iter()
        .map(|&b| soft(b, ((b as u32 + 7) % n) as u16, [30.0, 1.0, 0.8, 0.6]))
        .collect();
    for (i, d) in [(8usize, 1u32), (9, 34)] {
        let right = s[i];
        syms[i] = soft(
            ((right as u32 + d) % n) as u16,
            right,
            [10.0, 9.9, 0.5, 0.4],
        );
    }
    let mut out = Payload::default();
    codec.decode_payload(&syms, false, false, h, true, &mut out);
    assert!(!out.crc_ok, "soft decoding alone repairs this damage");
    (codec, syms, payload, h)
}

/// The list search repairs a damaged frame with soft decoding on. It swaps both the bins and their log-likelihoods of a symbol.
#[test]
fn the_list_search_repairs_a_frame_in_soft_mode() {
    let (codec, syms, payload, h) = damaged();
    let mut out = Payload::default();
    // The swap of the old search produces the same soft decode: no repair.
    let mut old = syms.clone();
    for s in old.iter_mut().filter(|s| s.gap() < 1.0) {
        s.bins.swap(0, 1);
        s.ll.swap(0, 1);
    }
    codec.decode_payload(&old, false, false, h, true, &mut out);
    assert!(!out.crc_ok);
    let mut r = Recovery::new();
    assert!(r.list_decode(&codec, &syms, false, false, h, true, &mut out));
    assert_eq!(out.bytes, payload);
    assert!(out.crc_ok);
}

/// `ChannelRx::reset` keeps the detector counters.
#[test]
fn a_reset_keeps_the_detector_counters() {
    let rate = 500e3;
    let p = LoraParams::new(7, 125e3, 5, 8, 0x12);
    let x = TxFrame::new(&p, b"count me")
        .unwrap()
        .at(0.01)
        .render(rate, 0.02);
    let mut rx =
        ChannelRx::new(rate, 0.0, 125e3, vec![DetectorConfig::new(7, 125e3, 0x12)]).unwrap();
    let mut out = Vec::new();
    rx.process(&x, &mut out);
    assert_eq!(out.len(), 1);
    let before = *rx.detectors()[0].stats();
    assert!(before.frames >= 1 && before.preambles >= 1);
    rx.reset().unwrap();
    assert_eq!(*rx.detectors()[0].stats(), before);
}

/// A payload above 255 bytes and bad parameters give errors, not a cut length or a panic.
#[test]
fn long_payloads_and_bad_parameters_are_errors() {
    let p = LoraParams::new(7, 125e3, 5, 8, 0x12);
    assert!(TxFrame::new(&p, &[0u8; 255]).is_ok());
    assert!(TxFrame::new(&p, &[0u8; 256]).is_err());
    assert!(LoraParams::new(13, 125e3, 5, 8, 0x12).check().is_err());
    assert!(LoraParams::new(7, 125e3, 4, 8, 0x12).check().is_err());
    assert!(TxFrame::new(&LoraParams::new(13, 125e3, 5, 8, 0x12), b"x").is_err());
    assert!(ChannelRx::new(1e6, 0.0, 125e3, vec![DetectorConfig::new(13, 125e3, 0x12)]).is_err());
}
