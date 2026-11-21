//! The recovery searches with soft and hard decisions.

use crate::common::confident;
use sdrkit_proto_lora_phy::{Codec, Header, Layout, Payload, Recovery, SymbolSoft};

/// A runner-up with the bins and the log-likelihoods both swapped.
fn old_runner_up(s: &SymbolSoft) -> SymbolSoft {
    let mut s = *s;
    s.bins.swap(0, 1);
    s.ll.swap(0, 1);
    s
}

/// Makes a soft frame at SF8 4/5 with two weak wrong symbols in one block that soft decoding cannot repair.
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
    let good = confident(&s, n);
    let mut out = Payload::default();
    // Two symbols of the second block, each moved to a wrong bin with the right bin as a close runner-up.
    for a in 0..5usize {
        for b in (a + 1)..5 {
            for da in 1..n {
                for db in [1u32, 2, 3, 5, 8, 13, 21, 34] {
                    let mut syms = good.clone();
                    for (i, d) in [(8 + a, da), (8 + b, db)] {
                        let right = s[i];
                        syms[i] = SymbolSoft {
                            bins: [((right as u32 + d) % n) as u16, right, 0, 1],
                            ll: [10.0, 9.9, 0.5, 0.4],
                            count: 4,
                            floor: 0.0,
                        };
                    }
                    codec.decode_payload(&syms, false, false, h, true, &mut out);
                    if !out.crc_ok {
                        return (codec, syms, payload, h);
                    }
                }
            }
        }
    }
    panic!("no damage that soft decoding misses");
}

#[test]
fn old_list_search_changes_nothing_in_soft_mode() {
    let (codec, syms, _, h) = damaged();
    let mut base = Payload::default();
    codec.decode_payload(&syms, false, false, h, true, &mut base);
    // Every combination of this runner-up produces the same soft decode, so the search never repairs a frame.
    let weak: Vec<usize> = (0..syms.len()).filter(|&i| syms[i].gap() < 1.0).collect();
    assert_eq!(weak.len(), 2);
    let mut work = syms.clone();
    let mut out = Payload::default();
    for mask in 1u32..4 {
        for (k, &i) in weak.iter().enumerate() {
            work[i] = if mask >> k & 1 == 1 {
                old_runner_up(&syms[i])
            } else {
                syms[i]
            };
        }
        codec.decode_payload(&work, false, false, h, true, &mut out);
        assert_eq!(out.bytes, base.bytes);
        assert!(!out.crc_ok);
    }
}

#[test]
fn list_search_repairs_the_frame_in_soft_mode() {
    let (codec, syms, payload, h) = damaged();
    let mut r = Recovery::new();
    let mut out = Payload::default();
    assert!(r.list_decode(&codec, &syms, false, false, h, true, &mut out));
    assert_eq!(out.bytes, payload);
    assert!(out.crc_ok);
    // Hard decisions repair it too.
    assert!(r.list_decode(&codec, &syms, false, false, h, false, &mut out));
    assert_eq!(out.bytes, payload);
    // A frame with no soft values has nothing to try.
    let hard: Vec<_> = syms.iter().map(|s| SymbolSoft::hard(s.bin())).collect();
    assert!(!r.list_decode(&codec, &hard, false, false, h, true, &mut out));
}

#[test]
fn one_and_two_jumps_are_undone_and_a_clean_frame_is_left_alone() {
    let sf = 11u8;
    let n = 1u32 << sf;
    let codec = Codec::new(sf).unwrap();
    let payload: Vec<u8> = (0..120).map(|i| (i * 7 + 3) as u8).collect();
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
    let soft_of = |bins: &[u16]| -> Vec<SymbolSoft> {
        bins.iter()
            .map(|&b| {
                let mut s = SymbolSoft::hard(b);
                s.count = 2;
                s.bins[1] = b ^ 1;
                s.ll = [10.0, 1.0, 0.0, 0.0];
                s
            })
            .collect()
    };
    let h = codec.decode_header(&soft_of(&s), true).unwrap();
    let mut r = Recovery::new();
    let mut out = Payload::default();
    // A clean frame: no jump to undo.
    assert!(r
        .jump_search(&codec, &soft_of(&s), false, h, 11.7, &mut out)
        .is_none());
    // One jump of 12 bins at symbol 40.
    let mut a = s.clone();
    for v in a.iter_mut().skip(40) {
        *v = ((*v as u32 + 12) % n) as u16;
    }
    let syms = soft_of(&a);
    codec.decode_payload(&syms, false, false, h, true, &mut out);
    assert!(!out.crc_ok);
    assert_eq!(
        r.jump_search(&codec, &syms, false, h, 11.7, &mut out),
        Some(1)
    );
    assert_eq!(out.bytes, payload);
    // Two jumps: -23 at 30 and +35 at 70.
    let mut b = s.clone();
    for (k, v) in b.iter_mut().enumerate() {
        let mut sh: i64 = 0;
        if k >= 30 {
            sh -= 23;
        }
        if k >= 70 {
            sh += 35;
        }
        *v = ((*v as i64 + sh).rem_euclid(n as i64)) as u16;
    }
    assert_eq!(
        r.jump_search(&codec, &soft_of(&b), false, h, 11.7, &mut out),
        Some(2)
    );
    assert_eq!(out.bytes, payload);
    assert!(r
        .jump_search(&codec, &soft_of(&b), false, h, f64::NAN, &mut out)
        .is_none());
}
