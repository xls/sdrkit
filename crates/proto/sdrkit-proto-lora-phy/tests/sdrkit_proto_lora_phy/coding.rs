//! The coding chain: round trips over every layout, error correction, and the shared pieces it uses.

use crate::common::hard;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_proto_lora_phy::coding::{hamming_rows, CODEBOOKS};
use sdrkit_proto_lora_phy::params::data_symbols;
use sdrkit_proto_lora_phy::{Codec, Header, Layout, LoraError, LoraParams, Payload};

/// The syndrome codes of `sdrkit_algo_fec::small` built from the LoRa parity rows have exactly the LoRa codebooks.
#[test]
fn small_codes_equal_the_lora_codebooks() {
    for c in 5..=8u8 {
        let code = SmallCode::from_parity_rows(c as usize, 4, &hamming_rows(c)).unwrap();
        for nib in 0..16u8 {
            // Information bits: nibble bit 0 first.
            let info =
                ((nib & 1) << 3) | (((nib >> 1) & 1) << 2) | (((nib >> 2) & 1) << 1) | (nib >> 3);
            assert_eq!(
                code.encode(info as u128) as u8,
                CODEBOOKS[c as usize - 5][nib as usize]
            );
        }
    }
}

#[test]
fn every_layout_round_trips() {
    let mut syms = Vec::new();
    let mut out = Payload::default();
    for sf in 5..=12u8 {
        let codec = Codec::new(sf).unwrap();
        for cr in 5..=8u8 {
            for ldro in [false, true] {
                for crc in [false, true] {
                    for implicit in [false, true] {
                        for len in [1usize, 2, 10, 50, 200, 255] {
                            let payload: Vec<u8> =
                                (0..len).map(|i| (i * 37 + sf as usize) as u8).collect();
                            let layout = Layout::new(sf, ldro, implicit).unwrap();
                            syms.clear();
                            let n = codec.encode(&payload, &layout, cr, crc, &mut syms).unwrap();
                            assert_eq!(n, syms.len());
                            assert_eq!(n, data_symbols(sf, cr, len, crc, implicit, ldro));
                            let s = hard(&syms);
                            let h = if implicit {
                                Header {
                                    len: len as u8,
                                    cr,
                                    crc,
                                }
                            } else {
                                let h = codec.decode_header(&s, false).expect("header");
                                assert_eq!(
                                    h,
                                    Header {
                                        len: len as u8,
                                        cr,
                                        crc
                                    }
                                );
                                h
                            };
                            for soft in [false, true] {
                                codec.decode_payload(&s, ldro, implicit, h, soft, &mut out);
                                assert_eq!(
                                    out.bytes, payload,
                                    "sf{sf} cr{cr} ldro{ldro} crc{crc} len{len}"
                                );
                                assert_eq!(out.crc_ok, crc);
                                assert!(out.clean);
                                assert_eq!(out.corrected, 0);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn one_wrong_symbol_per_block_is_corrected_at_4_7_and_4_8() {
    let mut out = Payload::default();
    for sf in 7..=12u8 {
        let codec = Codec::new(sf).unwrap();
        for cr in [7u8, 8] {
            let payload: Vec<u8> = (0..40).map(|i| i as u8 ^ 0x5A).collect();
            let mut s = Vec::new();
            codec
                .encode(
                    &payload,
                    &Layout::new(sf, false, false).unwrap(),
                    cr,
                    true,
                    &mut s,
                )
                .unwrap();
            let n = 1u32 << sf;
            // One symbol per later block moved by one bin (one bit by the Gray code).
            let mut at = 8;
            while at + 2 < s.len() {
                s[at + 2] = ((s[at + 2] as u32 + 1) % n) as u16;
                at += cr as usize;
            }
            let syms = hard(&s);
            let h = codec.decode_header(&syms, false).unwrap();
            for soft in [false, true] {
                codec.decode_payload(&syms, false, false, h, soft, &mut out);
                assert_eq!(out.bytes, payload);
                assert!(out.crc_ok);
                assert!(out.corrected > 0);
                assert!(!out.clean);
            }
        }
    }
}

/// The modulator side: a frame encodes through `LoraParams`, and bad parameters are errors, not panics.
#[test]
fn bad_parameters_are_errors() {
    assert!(Codec::new(4).is_err());
    assert!(Codec::new(13).is_err());
    assert!(Layout::new(13, false, false).is_err());
    let codec = Codec::new(7).unwrap();
    let mut v = Vec::new();
    let l7 = Layout::new(7, false, false).unwrap();
    assert!(matches!(
        codec.encode(&[0u8; 256], &l7, 5, true, &mut v),
        Err(LoraError::PayloadTooLong { len: 256 })
    ));
    assert!(codec.encode(b"x", &l7, 4, true, &mut v).is_err());
    assert!(codec.encode(b"x", &l7, 9, true, &mut v).is_err());
    assert!(codec
        .encode(
            b"x",
            &Layout::new(8, false, false).unwrap(),
            5,
            true,
            &mut v
        )
        .is_err());
    let mut p = LoraParams::new(7, 125e3, 5, 8, 0x12);
    for bad in [
        LoraParams { sf: 4, ..p.clone() },
        LoraParams { cr: 9, ..p.clone() },
        LoraParams {
            bw: f64::NAN,
            ..p.clone()
        },
        LoraParams {
            bw: -1.0,
            ..p.clone()
        },
        LoraParams {
            preamble: 2,
            ..p.clone()
        },
        LoraParams {
            implicit: true,
            implicit_len: 0,
            ..p.clone()
        },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
        assert!(sdrkit_proto_lora_phy::coding::encode(b"x", &bad).is_err());
    }
    p.sf = 6;
    assert!(!p.sx127x_can_send());
    p.implicit = true;
    assert!(p.sx127x_can_send());
    // Garbage symbols decode to something, without a panic.
    let garbage: Vec<_> = (0..300u32)
        .map(|i| sdrkit_proto_lora_phy::SymbolSoft::hard((i * 7919) as u16))
        .collect();
    let mut out = Payload::default();
    for sf in 5..=12u8 {
        let c = Codec::new(sf).unwrap();
        let _ = c.decode_header(&garbage, true);
        c.decode_payload(
            &garbage,
            true,
            false,
            Header {
                len: 255,
                cr: 8,
                crc: true,
            },
            true,
            &mut out,
        );
        c.decode_payload(
            &garbage[..3],
            false,
            true,
            Header {
                len: 9,
                cr: 5,
                crc: false,
            },
            false,
            &mut out,
        );
        assert!(c.decode_header(&garbage[..7], false).is_none());
    }
}
