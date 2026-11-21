//! The test program of sdrkit-proto-ccsds: one module per source module, and the allocation counts.
//!
//! Known answers come from the standards: the markers and the randomiser bytes as CCSDS 131.0-B-5 prints them, header layouts
//! written out by hand from the field tables of 132.0-B-3, 732.0-B-4 and 133.0-B-2, CRC check values, and Rice code words built
//! by hand from the option definitions of 121.0-B-3.

use sdrkit_algo_sim::rng::Rng;

fn bits_of(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| (0..8).rev().map(move |k| (b >> k) & 1))
        .collect()
}

fn bytes_of_bitstring(s: &str) -> Vec<u8> {
    let bits: Vec<u8> = s
        .bytes()
        .filter(|c| *c == b'0' || *c == b'1')
        .map(|c| c - b'0')
        .collect();
    bits.chunks(8)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0u8, |a, (i, &b)| a | b << (7 - i))
        })
        .collect()
}

mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct Counting;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            COUNT.with(|c| c.set(c.get() + 1));
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            COUNT.with(|c| c.set(c.get() + 1));
            unsafe { System.realloc(p, l, n) }
        }
    }

    /// Allocations made by this thread while `f` runs (other test threads do not count).
    pub fn count(f: impl FnOnce()) -> usize {
        let before = COUNT.with(|c| c.get());
        f();
        COUNT.with(|c| c.get()) - before
    }
}

#[global_allocator]
static A: alloc_count::Counting = alloc_count::Counting;

mod marker {
    use sdrkit_proto_ccsds::marker::{self, *};
    use sdrkit_util_bits::sync::Hit;

    #[test]
    fn markers_are_as_printed_in_131_0() {
        assert_eq!(TM.bytes(), &[0x1A, 0xCF, 0xFC, 0x1D]);
        assert_eq!(TM.value(), 0x1ACF_FC1D);
        assert_eq!(TURBO_1_2.value(), 0x0347_76C7_2728_95B0);
        // Rate 1/4 is rate 1/2 and its complement, rate 1/6 is rate 1/3 and its complement (clause 9.3.4).
        let comp = |b: &[u8]| b.iter().map(|x| !x).collect::<Vec<u8>>();
        assert_eq!(&TURBO_1_4.bytes()[8..], &comp(TURBO_1_2.bytes())[..]);
        assert_eq!(&TURBO_1_6.bytes()[..12], TURBO_1_3.bytes());
        assert_eq!(&TURBO_1_6.bytes()[12..], &comp(TURBO_1_3.bytes())[..]);
        assert_eq!(TURBO_1_6.len_bits(), 192);
        assert_eq!(TURBO_1_6.matched_bits(), 128);
        assert_eq!(PROX1.len_bits(), 24);
        assert_eq!(TC_START.value(), 0xEB90);
        assert_eq!(TC_TAIL.value(), 0xC5C5_C5C5_C5C5_C579);
        assert_eq!(marker::by_name("ccsds tm"), Some(TM));
        assert_eq!(marker::by_name("nothing"), None);
    }

    #[test]
    fn the_matcher_finds_the_marker_in_both_polarities() {
        let mut s = TM.sync_word(3, true).unwrap();
        let mut bits = vec![0u8; 10];
        let mut m = vec![0u8; 32];
        TM.write_bits(true, &mut m).unwrap();
        m[3] ^= 1;
        m[17] ^= 1;
        bits.extend_from_slice(&m);
        let mut hits = Vec::new();
        s.scan(&bits, &mut hits);
        assert_eq!(hits, [(42, Hit::Inverted { errors: 2 })]);
        assert!(TM.sync_word(16, true).is_err());
        assert!(TM.write_bits(false, &mut [0u8; 31]).is_err());
    }

    #[test]
    fn the_soft_correlator_finds_the_long_turbo_marker() {
        let mut s = TURBO_1_6.soft_sync(0.9, true).unwrap();
        let mut m = vec![0u8; 192];
        TURBO_1_6.write_bits(false, &mut m).unwrap();
        let llr: Vec<i8> = m.iter().map(|&b| if b == 0 { 20 } else { -20 }).collect();
        let mut hits = Vec::new();
        s.scan(&llr, &mut hits);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 192);
    }
}

mod randomiser {
    use sdrkit_proto_ccsds::randomiser::*;

    #[test]
    fn the_tm_sequence_starts_as_131_0_prints_it() {
        // CCSDS 131.0-B-5 clause 10.4: FF 48 0E C0 9A 0D 70 BC 8E 2C 93 AD A7 B7 46 CE ...
        assert_eq!(
            &tm_table()[..16],
            &[
                0xFF, 0x48, 0x0E, 0xC0, 0x9A, 0x0D, 0x70, 0xBC, 0x8E, 0x2C, 0x93, 0xAD, 0xA7, 0xB7,
                0x46, 0xCE
            ]
        );
        // The period is 255 bits, so the table of 255 bytes repeats.
        for i in 0..300 {
            assert_eq!(tm_bit(i), tm_bit(i + 255));
        }
    }

    #[test]
    fn bytes_bits_and_llrs_agree_at_any_offset() {
        let mut bytes = [0u8; 40];
        tm_bytes_at(&mut bytes, 7);
        let mut bits = vec![0u8; 320];
        tm_hard_bits(&mut bits, 56);
        assert_eq!(bits, super::bits_of(&bytes));
        let mut llr = vec![9i8; 320];
        tm_llrs(&mut llr, 56);
        let hard: Vec<u8> = llr.iter().map(|&l| (l < 0) as u8).collect();
        assert_eq!(hard, bits);
        // The first byte of the sequence is FF: every sign changes, and -128 never comes out.
        let mut ext = [127i8, -127, -128, 5, 0, -1, 1, 127];
        tm_llrs(&mut ext, 0);
        assert_eq!(ext, [-127, 127, 127, -5, 0, 1, -1, -127]);
        let mut twice = [0x5Au8; 600];
        tm_bytes(&mut twice);
        tm_bytes(&mut twice);
        assert!(twice.iter().all(|&b| b == 0x5A));
    }

    #[test]
    fn the_tc_sequence_starts_as_231_0_prints_it() {
        // CCSDS 231.0-B-4 clause 6.3: 1111 1111 0011 1001 1001 1110 0101 1010 0110 1000 ...
        assert_eq!(&TC.bytes()[..5], &[0xFF, 0x39, 0x9E, 0x5A, 0x68]);
        let mut d = [0u8; 3];
        tc_bytes(&mut d);
        assert_eq!(d, [0xFF, 0x39, 0x9E]);
    }
}

mod codesync {
    use super::Rng;
    use sdrkit_algo_fec::conv::{codes, Encoder};
    use sdrkit_proto_ccsds::codesync::*;

    #[test]
    fn the_coded_marker_is_state_free_in_its_last_52_bits() {
        let mask = (1u64 << 52) - 1;
        for s in 0..64 {
            assert_eq!(
                encode_word(0x1ACF_FC1D, s) & mask,
                TM_MARKER_CODED & mask,
                "state {s}"
            );
        }
        assert_eq!(nrzm_word(0b1011 << 28, 0) >> 28, 0b1101);
        assert_eq!(nrzm_word(0, 1), u32::MAX);
        // Transform 4 undoes the inversion of G2.
        assert_eq!(transform(4, 10, 20), (10, -20));
        assert_eq!(transform(5, 10, 20), (20, 10));
        assert_eq!(transform(2, -128, 5), (127, -5));
    }

    /// Frames of `frames` x 1024 bytes (marker + 1020 bytes), plain bits and coded LLRs.
    fn coded_frames(frames: usize, nrzm: bool, g2_inverted: bool, seed: u64) -> (Vec<u8>, Vec<i8>) {
        let mut rng = Rng::new(seed);
        let mut data = Vec::new();
        for _ in 0..frames {
            data.extend_from_slice(&0x1ACF_FC1Du32.to_be_bytes());
            for _ in 0..1020 {
                data.push(rng.next_u32() as u8);
            }
        }
        let plain = super::bits_of(&data);
        let mut bits = plain.clone();
        if nrzm {
            let mut level = 0u8;
            for b in bits.iter_mut() {
                level ^= *b;
                *b = level;
            }
        }
        let code = if g2_inverted { codes::CCSDS_K7 } else { CODE };
        let mut coded = Vec::new();
        Encoder::new(code).encode(&bits, &mut coded);
        let llr = coded
            .iter()
            .map(|&c| if c == 0 { 60 } else { -60 })
            .collect();
        (plain, llr)
    }

    fn find(bits: &[u8], word: u32) -> Option<usize> {
        let mut r = 0u32;
        for (i, &b) in bits.iter().enumerate() {
            r = (r << 1) | b as u32;
            if i >= 31 && r == word {
                return Some(i - 31);
            }
        }
        None
    }

    #[test]
    fn locks_under_every_hypothesis_and_decodes() {
        for (nrzm, inv) in [(false, false), (false, true), (true, false), (true, true)] {
            let (plain, soft) = coded_frames(4, nrzm, inv, 7);
            for t in 0..8u8 {
                for shift in [0usize, 1] {
                    let mut ch: Vec<i8> = vec![0; shift];
                    for p in soft.chunks(2) {
                        let (a, b) = transform(t, p[0], p[1]);
                        ch.push(a);
                        ch.push(b);
                    }
                    let mut d = CodeSync::new(CodeConfig::LEO).unwrap();
                    let mut out = Vec::new();
                    d.push(&ch, &mut out);
                    d.flush(&mut out);
                    assert!(d.lock().locked, "nrzm {nrzm} inv {inv} t {t} shift {shift}");
                    assert_eq!(d.lock().nrzm, nrzm);
                    assert_eq!(d.lock().shifted, shift == 1);
                    let at = find(&out, 0x1ACF_FC1D).expect("a marker in the decoded bits");
                    let n = 8000.min(out.len() - at);
                    assert!(n > 4000);
                    let pos = (0..3)
                        .map(|f| f * 8192)
                        .find(|&p| out[at..at + n] == plain[p..p + n]);
                    assert!(
                        pos.is_some(),
                        "decoded bits differ (t {t} nrzm {nrzm} inv {inv})"
                    );
                    assert_eq!(d.lock().ber, 0.0);
                }
            }
        }
    }

    #[test]
    fn noisy_symbols_lock_and_the_error_estimate_follows_the_channel() {
        let (plain, soft) = coded_frames(6, false, true, 11);
        let mut rng = Rng::new(3);
        // About 4 % hard errors: Gaussian noise on +/-1.
        let noisy: Vec<i8> = soft
            .iter()
            .map(|&s| {
                let v = s.signum() as f32 + 0.57 * rng.normal_f32();
                (v * 40.0).clamp(-127.0, 127.0) as i8
            })
            .collect();
        let hard_err = soft
            .iter()
            .zip(&noisy)
            .filter(|(a, b)| (**a < 0) != (**b < 0))
            .count() as f64
            / soft.len() as f64;
        let mut d = CodeSync::new(CodeConfig::LEO).unwrap();
        let mut out = Vec::new();
        // Cut the input in odd pieces: the output must not depend on the cuts.
        for c in noisy.chunks(997) {
            d.push(c, &mut out);
        }
        d.flush(&mut out);
        assert!(d.lock().locked);
        assert!(d.lock().g2_inverted());
        let at = find(&out, 0x1ACF_FC1D).unwrap();
        let start = (0..6)
            .map(|f| f * 8192)
            .find(|&p| out[at..at + 64] == plain[p..p + 64])
            .unwrap();
        let n = (out.len() - at).min(plain.len() - start) - 300;
        let errors = out[at..at + n]
            .iter()
            .zip(&plain[start..start + n])
            .filter(|(a, b)| a != b)
            .count();
        assert!(errors * 1000 < n, "{errors} bit errors in {n}");
        assert!(
            (d.lock().ber - hard_err).abs() < 0.015,
            "ber {} channel {hard_err}",
            d.lock().ber
        );
        let mut out2 = Vec::new();
        let mut d2 = CodeSync::new(CodeConfig::LEO).unwrap();
        d2.push(&noisy, &mut out2);
        d2.flush(&mut out2);
        assert_eq!(out, out2);
    }

    #[test]
    fn the_error_estimate_does_not_depend_on_the_cuts() {
        let (_, soft) = coded_frames(8, false, true, 13);
        let mut rng = Rng::new(21);
        let noisy: Vec<i8> = soft
            .iter()
            .map(|&s| {
                let v = s.signum() as f32 + 0.6 * rng.normal_f32();
                (v * 40.0).clamp(-127.0, 127.0) as i8
            })
            .collect();
        // Every cut size divides 448, so all runs stop at the same symbols there.
        let run = |cut: usize| {
            let mut d = CodeSync::new(CodeConfig::LEO).unwrap();
            let mut out = Vec::new();
            let mut seen = Vec::new();
            for (k, c) in noisy.chunks(cut).enumerate() {
                d.push(c, &mut out);
                if ((k + 1) * cut).is_multiple_of(448) {
                    seen.push((d.lock().ber, out.len()));
                }
            }
            d.flush(&mut out);
            (seen, d.lock().ber, out)
        };
        let (_, ber_whole, out_whole) = run(noisy.len());
        assert!(ber_whole > 0.01, "ber {ber_whole}");
        let one = run(1);
        assert_eq!(one.1, ber_whole);
        assert_eq!(one.2, out_whole);
        for cut in [7, 64, 997] {
            let r = run(cut);
            assert_eq!(r.1, ber_whole, "cut {cut}");
            assert_eq!(r.2, out_whole, "cut {cut}");
            if 448usize.is_multiple_of(cut) {
                assert_eq!(r.0, one.0, "cut {cut}");
            }
        }
    }

    #[test]
    fn the_lock_is_lost_when_the_markers_stop() {
        let (_, soft) = coded_frames(3, false, false, 5);
        let mut rng = Rng::new(9);
        let mut ch = soft.clone();
        ch.extend((0..16384 * 6).map(|_| if rng.next_u32() & 1 == 0 { 60i8 } else { -60 }));
        let mut d = CodeSync::new(CodeConfig {
            max_misses: 2,
            ..CodeConfig::LEO
        })
        .unwrap();
        let mut out = Vec::new();
        d.push(&ch, &mut out);
        assert!(!d.lock().locked);
        assert_eq!(d.lock().locks, 1);
    }

    #[test]
    fn the_blind_search_finds_symmetry_and_pairing() {
        // Coded without the G2 inversion, so the transform found is exactly the one that undoes the channel (up to a half turn).
        let (_, soft) = coded_frames(1, false, false, 21);
        let mut scratch = BlindScratch::new(400).unwrap();
        for (t, shift) in [(0u8, 0usize), (3, 1), (5, 0), (6, 1)] {
            let mut ch: Vec<i8> = vec![0; shift];
            for p in soft.chunks(2) {
                let (a, b) = transform(t, p[0], p[1]);
                ch.push(a);
                ch.push(b);
            }
            let r = blind_search(&ch[..2000], &mut scratch).unwrap();
            let mut ok = false;
            for cand in [r.best.transform, r.best.transform ^ 2] {
                let mut test = ch[r.best.shifted as usize..r.best.shifted as usize + 40].to_vec();
                for p in test.chunks_mut(2) {
                    let (a, b) = transform(cand, p[0], p[1]);
                    p[0] = a;
                    p[1] = b;
                }
                ok |= test == soft[..40];
            }
            assert!(ok, "t {t} shift {shift}: found {:?}", r.best);
            assert_eq!(r.best.shifted, shift == 1);
            assert!(r.error_rate < 0.01 && r.margin > 0.08, "{r:?}");
        }
        assert!(blind_search(&soft[..100], &mut scratch).is_err());
        assert!(BlindScratch::new(10).is_err());
    }

    #[test]
    fn bad_rules_are_refused() {
        assert!(CodeSync::new(CodeConfig {
            frame_bits: 10,
            ..CodeConfig::LEO
        })
        .is_err());
        assert!(CodeSync::new(CodeConfig {
            search_errors: 30,
            ..CodeConfig::LEO
        })
        .is_err());
    }
}

mod framesync {
    use sdrkit_proto_ccsds::framesync::*;
    use sdrkit_proto_ccsds::marker;

    fn stream(frames: usize, len: usize) -> (Vec<u8>, Vec<Vec<u8>>) {
        let mut bytes = vec![0x33u8; 5];
        let mut payloads = Vec::new();
        for f in 0..frames {
            bytes.extend_from_slice(marker::TM.bytes());
            let p: Vec<u8> = (0..len).map(|i| (i * 7 + f * 13) as u8).collect();
            bytes.extend_from_slice(&p);
            payloads.push(p);
        }
        (super::bits_of(&bytes), payloads)
    }

    #[test]
    fn frames_come_out_with_marker_errors_and_inversion() {
        let (mut bits, payloads) = stream(5, 40);
        // Two errors in the third marker.
        let m3 = 40 + 2 * (32 + 320);
        bits[m3 + 1] ^= 1;
        bits[m3 + 30] ^= 1;
        let mut s = FrameSync::new(SyncConfig::cadu(40)).unwrap();
        let mut got = Vec::new();
        for c in bits.chunks(13) {
            s.push_bits(c, |f| {
                got.push((f.bytes.to_vec(), f.marker_errors, f.start_bit))
            });
        }
        assert_eq!(got.len(), 5);
        for (g, p) in got.iter().zip(&payloads) {
            assert_eq!(&g.0, p);
        }
        assert_eq!(got[2].1, 2);
        assert_eq!(got[0].2, 40);
        // The inverted stream produces the same frames.
        let inv: Vec<u8> = bits.iter().map(|b| b ^ 1).collect();
        let mut s = FrameSync::new(SyncConfig::cadu(40)).unwrap();
        let mut n = 0;
        s.push_bits(&inv, |f| {
            assert!(f.inverted);
            assert_eq!(f.bytes, &payloads[n][..]);
            n += 1;
        });
        assert_eq!(n, 5);
    }

    #[test]
    fn the_flywheel_keeps_frames_and_the_lock_goes_after_too_many_misses() {
        let (mut bits, _) = stream(8, 20);
        // Destroy the markers of frames 3 and 4 (one miss each within the allowed count).
        for f in [3usize, 4] {
            let at = 40 + f * (32 + 160);
            for b in &mut bits[at..at + 32] {
                *b = 0;
            }
        }
        let mut s = FrameSync::new(SyncConfig {
            max_misses: 2,
            ..SyncConfig::cadu(20)
        })
        .unwrap();
        let mut fly = Vec::new();
        s.push_bits(&bits, |f| fly.push(f.flywheel));
        assert_eq!(fly.len(), 8);
        assert_eq!(fly.iter().filter(|&&f| f).count(), 2);
        assert_eq!(s.stats().misses, 2);
        // Noise after the frames: the lock goes after 3 misses.
        let noise: Vec<u8> = (0..2000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 31) as u8)
            .collect();
        s.push_bits(&noise, |_| {});
        assert!(!s.locked());
        assert_eq!(s.stats().losses, 1);
    }

    #[test]
    fn a_slip_changes_the_polarity() {
        let (bits, payloads) = stream(4, 16);
        let half = 40 + 2 * (32 + 128);
        let mut slipped = bits[..half].to_vec();
        slipped.extend(bits[half..].iter().map(|b| b ^ 1));
        let mut s = FrameSync::new(SyncConfig {
            frame_bytes: 16,
            ..SyncConfig::GEO
        })
        .unwrap();
        let mut got = Vec::new();
        s.push_bits(&slipped, |f| got.push((f.bytes.to_vec(), f.inverted)));
        assert_eq!(got.len(), 4);
        for (g, p) in got.iter().zip(&payloads) {
            assert_eq!(&g.0, p);
        }
        assert!(got[3].1);
        assert_eq!(s.stats().slips, 1);
    }

    #[test]
    fn llr_input_returns_frame_llrs_and_soft_acquisition() {
        let (bits, payloads) = stream(3, 10);
        // Weak, inverted symbols with four marker errors in the first marker: the hard matcher (3 errors) misses it, the soft
        // correlation finds it.
        let mut llr: Vec<i8> = bits
            .iter()
            .map(|&b| if b == 0 { -30 } else { 30 })
            .collect();
        for k in [41usize, 50, 60, 66] {
            llr[k] = llr[k].signum() * -2;
        }
        let cfg = SyncConfig {
            frame_bytes: 10,
            soft_threshold: Some(0.8),
            ..SyncConfig::cadu(10)
        };
        let mut s = FrameSync::new(cfg).unwrap();
        let mut got = Vec::new();
        s.push_llrs(&llr, |f| {
            let hard: Vec<u8> = f.llrs.unwrap().iter().map(|&l| (l < 0) as u8).collect();
            assert_eq!(super::bits_of(f.bytes), hard);
            got.push(f.bytes.to_vec());
        });
        assert_eq!(got, payloads);
    }

    #[test]
    fn bad_rules_are_refused() {
        assert!(FrameSync::new(SyncConfig::cadu(0)).is_err());
        assert!(FrameSync::new(SyncConfig {
            track_errors: 16,
            ..SyncConfig::LEO
        })
        .is_err());
        assert!(FrameSync::new(SyncConfig {
            soft_threshold: Some(2.0),
            ..SyncConfig::LEO
        })
        .is_err());
    }
}

mod channel {
    use sdrkit_algo_fec::rs::Basis;
    use sdrkit_proto_ccsds::channel::*;

    fn frame(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
            .collect()
    }

    #[test]
    fn interleaved_frames_correct_16_errors_per_code_word() {
        let cfg = ChannelConfig::rs_223(4);
        assert_eq!((cfg.coded_len(), cfg.frame_len()), (1020, 892));
        let f = frame(892, 3);
        let mut coder = ChannelCoder::new(cfg).unwrap();
        let mut out = Vec::new();
        coder.encode(&f, &mut out).unwrap();
        assert_eq!(&out[..4], &[0x1A, 0xCF, 0xFC, 0x1D]);
        assert_eq!(out.len(), 1024);
        let mut dec = ChannelDecoder::new(cfg).unwrap();
        let mut body = out[4..].to_vec();
        // 16 byte errors in each of the four code words: 64 in all.
        for e in 0..64 {
            body[(e * 15 + 1) % 1020] ^= 0xA5;
        }
        let d = dec.decode(&mut body).unwrap();
        assert!(d.ok());
        assert_eq!(d.corrected, 64);
        assert_eq!(&body[..892], &f[..]);
        // 17 errors in code word 1 only: it fails, the others decode.
        let mut body = out[4..].to_vec();
        for e in 0..17 {
            body[1 + 4 * (e * 13)] ^= 0x0F;
        }
        let d = dec.decode(&mut body).unwrap();
        assert_eq!((d.failed, d.failed_mask), (1, 0b10));
    }

    #[test]
    fn the_basis_is_found_and_kept() {
        let conv = ChannelConfig::rs_223(4).with_basis(BasisPolicy::Fixed(Basis::Conventional));
        let f = frame(892, 9);
        let mut out = Vec::new();
        ChannelCoder::new(conv)
            .unwrap()
            .encode(&f, &mut out)
            .unwrap();
        let auto = ChannelConfig::rs_223(4).with_basis(BasisPolicy::Auto(Basis::Dual));
        let mut dec = ChannelDecoder::new(auto).unwrap();
        let mut body = out[4..].to_vec();
        body[10] ^= 1;
        let d = dec.decode(&mut body).unwrap();
        assert!(d.ok() && d.basis_changed);
        assert_eq!(d.basis, Basis::Conventional);
        assert_eq!(&body[..892], &f[..]);
        let mut body = out[4..].to_vec();
        let d = dec.decode(&mut body).unwrap();
        assert!(d.ok() && !d.basis_changed);
        // A fixed dual decoder refuses the conventional frame (a code word does not decode).
        let mut dec = ChannelDecoder::new(ChannelConfig::rs_223(4)).unwrap();
        let mut body = out[4..].to_vec();
        assert!(!dec.decode(&mut body).unwrap().ok());
    }

    #[test]
    fn every_depth_virtual_fill_and_e8_round_trip() {
        for depth in 1..=8 {
            for cfg in [
                ChannelConfig::rs_223(depth),
                ChannelConfig::rs_239(depth),
                ChannelConfig::rs_223(depth).with_virtual_fill(95),
                ChannelConfig::rs_223(depth).without_randomiser(),
            ] {
                let f = frame(cfg.frame_len(), depth as u8);
                let mut out = Vec::new();
                ChannelCoder::new(cfg)
                    .unwrap()
                    .encode(&f, &mut out)
                    .unwrap();
                let mut body = out[4..].to_vec();
                let t = cfg.rs.unwrap().nroots() / 2;
                for e in 0..t * depth {
                    let at = (e * 13) % body.len();
                    body[at] ^= 0x81;
                }
                let mut dec = ChannelDecoder::new(cfg).unwrap();
                let d = dec.decode(&mut body).unwrap();
                assert!(d.ok(), "depth {depth} {cfg:?}");
                assert_eq!(&body[..d.frame_len], &f[..]);
            }
        }
        let u = ChannelConfig::uncoded(100);
        let mut out = Vec::new();
        ChannelCoder::new(u)
            .unwrap()
            .encode(&[0u8; 100], &mut out)
            .unwrap();
        assert_eq!(&out[4..8], &[0xFF, 0x48, 0x0E, 0xC0]);
        let mut d = ChannelDecoder::new(u).unwrap();
        let mut body = out[4..].to_vec();
        let r = d.decode(&mut body).unwrap();
        assert!(r.ok() && !r.checked);
        assert!(body.iter().all(|&b| b == 0));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(ChannelDecoder::new(ChannelConfig::rs_223(9)).is_err());
        assert!(ChannelDecoder::new(ChannelConfig::rs_223(1).with_virtual_fill(223)).is_err());
        let mut d = ChannelDecoder::new(ChannelConfig::rs_223(4)).unwrap();
        assert!(d.decode(&mut [0u8; 100]).is_err());
        let mut c = ChannelCoder::new(ChannelConfig::rs_223(4)).unwrap();
        assert!(c.encode(&[0u8; 10], &mut Vec::new()).is_err());
    }
}

mod tm {
    use sdrkit_proto_ccsds::tm::*;
    use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

    #[test]
    fn the_header_layout_is_as_132_0_draws_it() {
        let h = PrimaryHeader {
            version: 0,
            spacecraft: 0x2AB,
            vcid: 5,
            ocf: true,
            mc_count: 0x12,
            vc_count: 0x34,
            secondary_header: true,
            sync: false,
            packet_order: false,
            segment_length_id: 3,
            first_header: FirstHeader::At(0x123),
        };
        let mut b = [0u8; 6];
        h.write(&mut b).unwrap();
        // 00 1010101011 101 1 | 0x12 | 0x34 | 1 0 0 11 00100100011
        assert_eq!(b, [0x2A, 0xBB, 0x12, 0x34, 0x99, 0x23]);
        assert_eq!(PrimaryHeader::parse(&b).unwrap(), h);
        assert_eq!(h.mcid(), 0x2AB);
        assert!(PrimaryHeader { vcid: 8, ..h }.write(&mut b).is_err());
        assert_eq!(FirstHeader::from_field(0x7FF), FirstHeader::None);
        assert_eq!(FirstHeader::from_field(0x7FE), FirstHeader::IdleOnly);
        assert_eq!(FirstHeader::IdleOnly.field(), 0x7FE);
    }

    #[test]
    fn frames_with_secondary_header_ocf_and_fecf() {
        // The FECF is CRC-16/IBM-3740 (CCSDS 132.0-B-3 clause 4.1.6): check value 0x29B1.
        assert_eq!(CRC16_IBM_3740.checksum(b"123456789"), 0x29B1);
        let layout = Layout {
            frame_len: 100,
            secondary_header_len: 5,
            ocf: true,
            fecf: true,
        };
        assert_eq!(layout.data_len(), 100 - 6 - 5 - 4 - 2);
        let h = PrimaryHeader {
            secondary_header: true,
            ocf: true,
            ..PrimaryHeader::default()
        };
        let mut f = vec![0u8; 100];
        layout
            .build(&h, &[1, 2, 3, 4], &[9; 20], Some(7), &mut f)
            .unwrap();
        assert_eq!(f[6], 4); // version 00, length 5 - 1
        let v = layout.parse(&f).unwrap();
        assert_eq!(v.secondary, &[1, 2, 3, 4]);
        assert_eq!(v.ocf, Some(7));
        assert_eq!(v.fecf_ok, Some(true));
        f[50] ^= 1;
        assert_eq!(layout.parse(&f).unwrap().fecf_ok, Some(false));
        assert!(layout.parse(&f[..99]).is_err());
        assert!(layout.build(&h, &[0; 5], &[], None, &mut f).is_err());
    }
}

mod aos {
    use sdrkit_proto_ccsds::aos::*;
    use sdrkit_proto_ccsds::tm::FirstHeader;

    #[test]
    fn the_header_layout_is_as_732_0_draws_it() {
        let h = PrimaryHeader {
            version: 1,
            spacecraft: 0x2A,
            vcid: 5,
            counter: 0x12_3456,
            replay: true,
            cycle_in_use: true,
            cycle: 0xA,
        };
        let mut b = [0u8; 6];
        h.write(&mut b).unwrap();
        // 01 00101010 000101 | 0x123456 | 1 1 00 1010
        assert_eq!(b, [0x4A, 0x85, 0x12, 0x34, 0x56, 0xCA]);
        assert_eq!(PrimaryHeader::parse(&b).unwrap(), h);
        assert_eq!(h.extended_counter(), 0xA12_3456);
        assert!(PrimaryHeader {
            counter: 1 << 24,
            ..h
        }
        .write(&mut b)
        .is_err());
        assert!(PrimaryHeader { vcid: 63, ..h }.is_idle());
    }

    #[test]
    fn layouts_of_the_weather_links() {
        assert_eq!(Layout::RS_I4.data_zone_len(), 884);
        assert_eq!(Layout::METEOR.data_zone_len(), 882);
        assert_eq!(Layout::FY4.data_zone_len(), 1012);
        let l = Layout {
            frame_len: 200,
            header_error_control: true,
            insert_zone: 4,
            ocf: true,
            fecf: true,
        };
        assert_eq!(l.data_zone_len(), 200 - 6 - 2 - 4 - 2 - 4 - 2);
        let mut f = vec![0u8; 200];
        let h = PrimaryHeader {
            vcid: 9,
            ..PrimaryHeader::default()
        };
        l.build(&h, &[1, 2, 3, 4], FirstHeader::None, &[5; 10], &mut f)
            .unwrap();
        let v = l.parse(&f).unwrap();
        assert_eq!(v.insert_zone, &[1, 2, 3, 4]);
        assert_eq!(v.first_header, FirstHeader::None);
        assert_eq!(v.fecf_ok, Some(true));
        assert_eq!(v.ocf, Some(0));
        assert_eq!(&v.zone[..10], &[5; 10]);
        assert!(l
            .build(&h, &[0; 5], FirstHeader::None, &[], &mut f)
            .is_err());
    }

    #[test]
    fn bitstream_pointers() {
        assert_eq!(
            BitstreamPointer::parse(&[0xFF, 0xFF]).unwrap(),
            BitstreamPointer::AllValid
        );
        assert_eq!(
            BitstreamPointer::parse(&[0x3F, 0xFE]).unwrap(),
            BitstreamPointer::IdleOnly
        );
        assert_eq!(
            BitstreamPointer::parse(&[0x01, 0x00]).unwrap(),
            BitstreamPointer::LastValid(256)
        );
        assert_eq!(BitstreamPointer::LastValid(256).bytes(), [0x01, 0x00]);
        assert!(BitstreamPointer::parse(&[1]).is_err());
    }
}

mod demux {
    use sdrkit_proto_ccsds::demux::*;

    #[test]
    fn counts_wrap_at_their_width() {
        let mut c = VcCounters::new(8).unwrap();
        assert_eq!(c.push(1, 254), Continuity::First);
        assert_eq!(c.push(1, 255), Continuity::InOrder);
        assert_eq!(c.push(1, 0), Continuity::InOrder);
        assert_eq!(c.push(1, 0), Continuity::Repeat);
        assert_eq!(c.push(1, 250), Continuity::Gap { missing: 249 });
        assert_eq!(c.push(2, 7), Continuity::First);
        let s = c.stats(1);
        assert_eq!((s.frames, s.gaps, s.lost, s.repeats), (5, 1, 249, 1));
        assert_eq!(c.seen().count(), 2);
        c.restart();
        assert_eq!(c.push(1, 9), Continuity::First);
        c.clear();
        assert_eq!(c.stats(1).frames, 0);
        assert!(VcCounters::new(0).is_err());
        let mut c = VcCounters::new(28).unwrap();
        c.push(0, 0xFFF_FFFF);
        assert_eq!(c.push(0, 0), Continuity::InOrder);
    }
}

mod packet {
    use sdrkit_proto_ccsds::packet::*;
    use sdrkit_proto_ccsds::tm::FirstHeader;

    #[test]
    fn the_header_layout_is_as_133_0_draws_it() {
        let h = PrimaryHeader {
            data_len: 5,
            ..PrimaryHeader::telemetry(0x123, SeqFlags::Unsegmented, 7)
        };
        // 000 0 0 00100100011 | 11 00000000000111 | length - 1
        assert_eq!(h.bytes().unwrap(), [0x01, 0x23, 0xC0, 0x07, 0x00, 0x04]);
        let tc = PrimaryHeader {
            telecommand: true,
            secondary_header: true,
            ..h
        };
        assert_eq!(tc.bytes().unwrap()[0], 0x19);
        assert_eq!(PrimaryHeader::parse(&tc.bytes().unwrap()).unwrap(), tc);
        assert!(PrimaryHeader { apid: 0x800, ..h }.bytes().is_err());
        assert!(PrimaryHeader { data_len: 0, ..h }.bytes().is_err());
        assert!(SeqFlags::First.starts() && !SeqFlags::First.ends());
        let mut idle_pkt = Vec::new();
        idle(10, &mut idle_pkt).unwrap();
        assert_eq!(idle_pkt, [0x07, 0xFF, 0xC0, 0x00, 0x00, 0x03, 0, 0, 0, 0]);
        assert!(idle(6, &mut idle_pkt).is_err());
    }

    fn packets(n: usize, crc: bool) -> Vec<Vec<u8>> {
        (0..n)
            .map(|i| {
                let user: Vec<u8> = (0..(100 + i * 97)).map(|k| (k + i) as u8).collect();
                let h =
                    PrimaryHeader::telemetry(64 + (i % 3) as u16, SeqFlags::Unsegmented, i as u16);
                let mut p = Vec::new();
                if crc {
                    build_with_crc(&h, &user, &mut p).unwrap();
                } else {
                    build(&h, &user, &mut p).unwrap();
                }
                p
            })
            .collect()
    }

    #[test]
    fn packets_across_zones_come_back_and_a_lost_frame_drops_only_its_packets() {
        let ps = packets(20, false);
        let refs: Vec<&[u8]> = ps.iter().map(|p| &p[..]).collect();
        let zones = pack_zones(&refs, 882).unwrap();
        let mut a = Assembler::new(AssemblerConfig::default());
        let mut got = Vec::new();
        for (i, (fh, z)) in zones.iter().enumerate() {
            a.push_mpdu(5, Some(100 + i as u32), *fh, z, |p| {
                got.push((p.header.count, p.data.len(), p.gap_before))
            });
        }
        assert_eq!(got.len(), 20);
        for (i, g) in got.iter().enumerate() {
            assert_eq!((g.0, g.1), (i as u16, 100 + i * 97));
        }
        assert_eq!(a.stats().idle, 1);
        let mut a = Assembler::new(AssemblerConfig::default());
        let mut lost = Vec::new();
        for (i, (fh, z)) in zones.iter().enumerate() {
            if i != 6 {
                a.push_mpdu(5, Some(100 + i as u32), *fh, z, |p| {
                    lost.push((p.header.count, p.gap_before))
                });
            }
        }
        assert_eq!(a.stats().gaps, 1);
        assert!(lost.len() + 3 >= got.len() && lost.len() < got.len());
        assert_eq!(lost.iter().filter(|p| p.1).count(), 1);
    }

    #[test]
    fn crc_idle_and_bad_pointers() {
        let ps = packets(3, true);
        let refs: Vec<&[u8]> = ps.iter().map(|p| &p[..]).collect();
        let zones = pack_zones(&refs, 884).unwrap();
        let mut a = Assembler::new(AssemblerConfig {
            crc: true,
            ..AssemblerConfig::default()
        });
        let mut got = Vec::new();
        for (i, (fh, z)) in zones.iter().enumerate() {
            let mut z = z.clone();
            if i == 0 {
                z[50] ^= 1; // in the first packet's data
            }
            a.push_mpdu(0, Some(i as u32), *fh, &z, |p| {
                got.push((p.data.len(), p.crc_ok))
            });
        }
        assert_eq!(
            got,
            [(100, Some(false)), (197, Some(true)), (294, Some(true))]
        );
        assert_eq!(a.stats().crc_errors, 1);
        // A pointer past the zone is refused, an idle-only zone is passed over.
        a.push_mpdu(0, None, FirstHeader::At(900), &zones[0].1, |_| panic!());
        assert_eq!(a.stats().bad_pointers, 1);
        a.push_mpdu(0, None, FirstHeader::IdleOnly, &zones[0].1, |_| panic!());
        // A damaged header (version 7) drops what is buffered.
        let mut z = vec![0xFFu8; 50];
        z[0] = 0xE0;
        a.push_mpdu(1, None, FirstHeader::At(0), &z, |_| panic!());
        assert!(a.stats().lost >= 1);
        a.reset();
        let mut keep = Assembler::new(AssemblerConfig {
            keep_idle: true,
            ..AssemblerConfig::default()
        });
        let mut n = 0;
        keep.push_mpdu(0, None, zones[0].0, &zones[0].1, |p| {
            n += p.header.is_idle() as usize
        });
        assert_eq!(n, 1);
    }

    #[test]
    fn one_packet_fills_a_zone() {
        let mut p = Vec::new();
        build(
            &PrimaryHeader::telemetry(1, SeqFlags::Unsegmented, 0),
            b"abc",
            &mut p,
        )
        .unwrap();
        let mut zone = [0u8; 20];
        assert_eq!(fill_zone(&p, &mut zone).unwrap(), FirstHeader::At(0));
        assert_eq!(PrimaryHeader::parse(&zone[9..]).unwrap().apid, IDLE_APID);
        assert!(fill_zone(&[0; 30], &mut zone).is_err());
        assert!(pack_zones(&[], 0).is_err());
    }
}

mod xrit {
    use sdrkit_proto_ccsds::packet::{Packet, PrimaryHeader, SeqFlags};
    use sdrkit_proto_ccsds::xrit::headers::*;
    use sdrkit_proto_ccsds::xrit::transport::*;

    #[test]
    fn built_headers_parse_back() {
        let b = Builder::new(0)
            .image(8, 2712, 226, 1)
            .navigation(
                "GEOS(-137.0)",
                [40_932_549.0, 40_932_549.0, 1356.0, 113.0],
                false,
            )
            .annotation("OR_ABI-L2-CMIPF-M6C13_G18_test.lrit")
            .time_unix(1_791_763_200.0 + 123.456)
            .rice(&RiceRecord {
                flags: RiceRecord::NN | RiceRecord::MSB,
                pixels_per_block: 16,
                lines_per_packet: 1,
            })
            .record(129, b"NOAA\x00\x10\x00\x01\x00\x02\x01");
        let mut f = b.build(1000);
        f.extend(std::iter::repeat_n(0, 1000));
        let h = parse(&f, Dialect::Goes).unwrap();
        assert_eq!(h.data_bits, 8000);
        let img = h.image.as_ref().unwrap();
        assert_eq!(
            (img.columns, img.lines, img.compression),
            (2712, 226, Compression::Lossless)
        );
        assert_eq!(h.navigation.as_ref().unwrap().projection, "GEOS(-137.0)");
        assert!((h.unix_time().unwrap() - (1_791_763_200.0 + 123.456)).abs() < 0.01);
        assert_eq!(h.noaa.unwrap().compression, NoaaCompression::Rice);
        let p = h.rice.unwrap().params(10, 2712).unwrap();
        assert!(p.preprocess);
        assert_eq!((p.block, p.rsi, p.bits), (16, 170, 10));
        assert_eq!(records(&f).unwrap().count(), 6);
        assert!(parse(&f[..10], Dialect::Goes).is_err());
        // CDS day 25141 and 10,800,000 ms is 2026-11-01 03:00:00 UTC.
        let t = Headers {
            time: Some((25_141, 10_800_000)),
            ..Default::default()
        };
        assert_eq!(t.unix_time().unwrap(), 1_793_502_000.0);
        // A record longer than the header is an error.
        let mut bad = Builder::new(0).annotation("x").build(0);
        bad[17] = 0;
        bad[18] = 9;
        assert!(parse(&bad, Dialect::Goes).is_err());
    }

    #[test]
    fn the_gk2a_and_fy4_dialects() {
        let f = Builder::new(0)
            .image(12, 2200, 220, 1)
            .record(7, &0x0001_1F2Au32.to_be_bytes())
            .record(128, &[3, 10, 0x01, 0xB8])
            .record(131, b"20261101T030000Z")
            .build(0);
        let h = parse(&f, Dialect::Gk2a).unwrap();
        assert_eq!(h.key, Some(0x1F2A));
        let s = h.segment.unwrap();
        assert_eq!((s.sequence, s.total, s.start_line), (3, 10, 440));
        assert_eq!(h.observation_time.as_deref(), Some("20261101T030000Z"));
        assert_eq!(parse(&f, Dialect::Goes).unwrap().key, None);
        let mut rec = b"FY4A\0\0\0\0\0AGRI\0\0\0".to_vec();
        rec.push(12);
        rec.extend_from_slice(&2748u16.to_be_bytes());
        rec.extend_from_slice(&1374u16.to_be_bytes());
        rec.extend_from_slice(&[1, 9, 0, 14, 0, 2, 0, 2, 0, 100, 3]);
        let f = Builder::new(0)
            .record(1, &rec)
            .navigation("NOM", [1.5, 2.5, 3.5, 4.5], true)
            .record(7, &7u32.to_be_bytes())
            .build(0);
        let h = parse(&f, Dialect::Fy4).unwrap();
        let i = h.image.unwrap();
        let fy = i.fy4.unwrap();
        assert_eq!(
            (
                i.columns,
                i.lines,
                fy.channel,
                fy.total_segments,
                fy.segment
            ),
            (2748, 1374, 9, 14, 2)
        );
        assert_eq!(fy.satellite, "FY4A");
        assert_eq!(h.navigation.unwrap().lfac, 2.5);
        assert_eq!(h.key, Some(7));
    }

    fn pkt<'a>(apid: u16, flags: SeqFlags, count: u16, data: &'a [u8]) -> Packet<'a> {
        Packet {
            vcid: 1,
            header: PrimaryHeader::telemetry(apid, flags, count),
            data,
            crc_ok: None,
            gap_before: false,
        }
    }

    #[test]
    fn files_come_together_and_a_lost_packet_is_marked() {
        let file: Vec<u8> = (0..5000u32).map(|i| (i * 3) as u8).collect();
        let pieces = split_file(&file, 42, 1000, &[2500]);
        assert_eq!(transport_header(&pieces[0]).unwrap(), (42, 40_000));
        let n = pieces.len();
        let flags = |i: usize| {
            if i == 0 {
                SeqFlags::First
            } else if i + 1 == n {
                SeqFlags::Last
            } else {
                SeqFlags::Continuation
            }
        };
        let mut a = FileAssembler::new(AssemblerConfig::default());
        let mut got = None;
        for (i, p) in pieces.iter().enumerate() {
            a.push(&pkt(5, flags(i), i as u16, p), |f| {
                got = Some((f.bytes.to_vec(), f.errors, f.length_ok, f.counter))
            });
        }
        assert_eq!(got, Some((file.clone(), false, true, 42)));
        // Lose packet 2: the file has errors and the index steps over it.
        let mut idx = Vec::new();
        for (i, p) in pieces.iter().enumerate() {
            if i != 2 {
                a.push(&pkt(5, flags(i), i as u16, p), |f| {
                    assert!(f.errors && !f.length_ok);
                    idx = f.packet_index.to_vec();
                });
            }
        }
        assert_eq!(idx[2], 3);
        // A new first packet cuts the file in progress short. Orphans are counted.
        a.push(&pkt(5, SeqFlags::First, 0, &pieces[0]), |_| {});
        let mut cut = false;
        a.push(&pkt(5, SeqFlags::First, 0, &pieces[0]), |f| {
            cut = f.truncated
        });
        assert!(cut);
        a.reset();
        a.push(&pkt(9, SeqFlags::Continuation, 1, &pieces[1]), |_| panic!());
        assert_eq!(a.stats().orphans, 1);
        a.push(&pkt(9, SeqFlags::First, 0, &[1, 2]), |_| panic!());
        assert_eq!(a.stats().dropped, 1);
        // An unsegmented file comes out at once.
        let one = split_file(b"tiny", 1, 100, &[]);
        let mut n = 0;
        a.push(&pkt(3, SeqFlags::Unsegmented, 0, &one[0]), |f| {
            assert_eq!(f.bytes, b"tiny");
            n += 1;
        });
        assert_eq!(n, 1);
    }

    #[test]
    fn the_oldest_file_makes_room() {
        let mut a = FileAssembler::new(AssemblerConfig {
            open_files: 2,
            ..AssemblerConfig::default()
        });
        let first = split_file(&[0u8; 50], 0, 1000, &[]);
        let mut cut = Vec::new();
        for apid in 1..=3u16 {
            a.push(&pkt(apid, SeqFlags::First, 0, &first[0]), |f| {
                cut.push(f.apid)
            });
        }
        assert_eq!(cut, [1]);
    }
}

mod rice {
    use super::Rng;
    use sdrkit_proto_ccsds::rice::*;

    #[test]
    fn the_mapping_is_as_121_0_clause_4_defines_it() {
        // Prediction 10 in 0..255: theta 10. +2 maps to 4, -5 maps to 9, +20 maps to theta + 20 = 30.
        assert_eq!(map(12, 10, 0, 255), 4);
        assert_eq!(map(5, 10, 0, 255), 9);
        assert_eq!(map(30, 10, 0, 255), 30);
        // Prediction 250: theta 5, a step of -150 maps to 155.
        assert_eq!(map(100, 250, 0, 255), 155);
        for pred in -8..8i64 {
            for x in -8..8i64 {
                let m = map(x, pred, -8, 7);
                assert!(m <= 15);
                assert_eq!(unmap(m, pred, -8, 7), x);
            }
        }
    }

    fn params(bits: u32, block: usize, rsi: usize, pre: bool) -> Params {
        Params {
            preprocess: pre,
            ..Params::new(bits, block, rsi).unwrap()
        }
    }

    #[test]
    fn code_words_are_as_121_0_clause_5_defines_them() {
        // Split sample, k = 1: ID 010, eight FS code words of 1 ("01"), then eight low bits 1.
        let p = params(8, 8, 1, false);
        let mut c = Vec::new();
        encode(&[3; 8], &p, &mut c).unwrap();
        assert_eq!(
            c,
            super::bytes_of_bitstring(&format!("010{}{}00000", "01".repeat(8), "1".repeat(8)))
        );
        // Three zero blocks: ID 000, selector 0, FS of 2 (three blocks): 000 0 001.
        let p3 = params(8, 8, 3, false);
        c.clear();
        encode(&[0; 24], &p3, &mut c).unwrap();
        assert_eq!(c, [0b0000_0010]);
        // Ten zero blocks to the end of the interval: ID 000, selector 0, FS of 4 (ROS): 000 0 00001.
        let p10 = params(8, 8, 10, false);
        c.clear();
        encode(&[0; 80], &p10, &mut c).unwrap();
        assert_eq!(c, [0x00, 0x80]);
        // Seven zero blocks, not to the end: FS of 7.
        let p8 = params(8, 8, 8, false);
        c.clear();
        let mut s = vec![0u32; 64];
        s[60] = 200;
        encode(&s, &p8, &mut c).unwrap();
        let b = super::bits_of(&c);
        assert_eq!(&b[..12], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        // Second extension: (0,1) (0,0) (1,0) (0,0) give 2, 0, 1, 0: ID 000, selector 1, 001 1 01 1.
        c.clear();
        encode(&[0, 1, 0, 0, 1, 0, 0, 0], &p, &mut c).unwrap();
        assert_eq!(c, super::bytes_of_bitstring("000 1 001 1 01 1 00000"));
        // No compression: ID 111 then the eight samples.
        c.clear();
        let wide = [255u32, 0, 255, 0, 255, 0, 255, 0];
        encode(&wide, &p, &mut c).unwrap();
        assert_eq!(c[0] >> 5, 0b111);
        // The decoder reads each back.
        for samples in [vec![3u32; 8], vec![0, 1, 0, 0, 1, 0, 0, 0], wide.to_vec()] {
            let mut c = Vec::new();
            encode(&samples, &p, &mut c).unwrap();
            let mut out = vec![0u32; 8];
            assert_eq!(decode(&c, &p, &mut out).unwrap().samples, 8);
            assert_eq!(out, samples);
        }
    }

    #[test]
    fn identifier_lengths_follow_table_5_1() {
        let id = |bits, restricted| {
            Params {
                restricted,
                ..Params::new(bits, 16, 1).unwrap()
            }
            .id_len()
        };
        assert_eq!(
            (
                id(2, false),
                id(2, true),
                id(4, true),
                id(8, false),
                id(16, false),
                id(17, false),
                id(32, false)
            ),
            (3, 1, 2, 3, 4, 5, 5)
        );
        assert_eq!(
            Params {
                restricted: true,
                ..Params::new(2, 16, 1).unwrap()
            }
            .k_max(),
            None
        );
        assert_eq!(Params::new(8, 16, 1).unwrap().k_max(), Some(5));
        assert_eq!(Params::new(16, 16, 1).unwrap().k_max(), Some(13));
        assert!(Params::new(0, 16, 1).is_err());
        assert!(Params::new(8, 12, 1).is_err());
        assert!(Params::new(8, 16, 5000).is_err());
    }

    #[test]
    fn random_lines_round_trip_in_every_setting() {
        let mut rng = Rng::new(17);
        for &(bits, block, rsi, pre, signed, restricted) in &[
            (8u32, 16usize, 170usize, true, false, false),
            (10, 16, 7, true, false, false),
            (12, 32, 3, true, true, false),
            (16, 64, 1, true, false, false),
            (32, 8, 4, true, false, false),
            (4, 8, 2, true, false, true),
            (2, 8, 2, false, false, true),
            (1, 16, 2, true, false, false),
            (8, 8, 70, true, false, false),
        ] {
            let p = Params {
                preprocess: pre,
                signed,
                restricted,
                ..Params::new(bits, block, rsi).unwrap()
            };
            let mask = if bits == 32 {
                u32::MAX
            } else {
                (1u32 << bits) - 1
            };
            let n = 2711;
            let mut v = 0u32;
            let line: Vec<u32> = (0..n)
                .map(|i| {
                    // Smooth parts, runs of zeros, and noisy parts, so every option is used.
                    if (i / 300) % 3 == 0 {
                        if (i / 50) % 4 == 0 {
                            0
                        } else {
                            v
                        }
                    } else if (i / 300) % 3 == 1 {
                        v = v.wrapping_add(rng.next_u32() % 3) & mask;
                        v
                    } else {
                        rng.next_u32() & mask
                    }
                })
                .collect();
            let mut c = Vec::new();
            encode(&line, &p, &mut c).unwrap();
            // Two lines back to back: the bits used tell where the second starts.
            let first = c.len();
            encode(&line, &p, &mut c).unwrap();
            let mut out = vec![0u32; n];
            let d = decode(&c, &p, &mut out).unwrap();
            assert_eq!(out, line, "{bits} bits, J {block}, rsi {rsi}");
            assert_eq!(d.bytes(), first);
            let d2 = decode(&c[d.bytes()..], &p, &mut out).unwrap();
            assert_eq!(d2.samples, n);
            assert_eq!(out, line);
            // A cut line decodes short and does not fail.
            let d3 = decode(&c[..first / 2], &p, &mut out).unwrap();
            assert!(d3.samples < n);
        }
    }

    #[test]
    fn a_run_stops_at_the_segment_inside_the_interval() {
        // 70 blocks per interval: the second interval starts at block 70, and its first segment of 64 blocks ends at block 134,
        // not at block 128 (a segment counted from the start of the data).
        let p = params(8, 8, 70, false);
        let mut s = vec![9u32; 140 * 8];
        for v in &mut s[120 * 8..140 * 8] {
            *v = 0;
        }
        let mut c = Vec::new();
        encode(&s, &p, &mut c).unwrap();
        let mut out = vec![1u32; s.len()];
        decode(&c, &p, &mut out).unwrap();
        assert_eq!(out, s);
        // Each of the first 120 blocks is split sample with k = 2: ID 011, eight FS of 2 (001), eight times 2 low bits: 43 bits.
        // Then blocks 120..134 reach the segment end (ROS), and blocks 134..140 reach the interval end (ROS): 000 0 00001 twice.
        let bits = super::bits_of(&c);
        let pos = 120 * 43;
        assert_eq!(&bits[..3], &[0, 1, 1]);
        let ros = [0u8, 0, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(&bits[pos..pos + 9], &ros);
        assert_eq!(&bits[pos + 9..pos + 18], &ros);
        assert_eq!(c.len(), (pos + 18).div_ceil(8));
    }

    #[test]
    fn viirs_parameters_round_trip_a_detector_line() {
        let p = Params::VIIRS;
        p.check().unwrap();
        assert_eq!(p.id_len(), 4);
        // A smooth 15-bit line with flat runs (zero blocks) and steps, as long as the widest VIIRS zone (1280 x 3 samples).
        let line: Vec<u32> = (0..3840u32)
            .map(|i| match i / 500 {
                1 => 9000,
                3 => 20_000 + (i % 13) * 700,
                _ => 2000 + (i * 37) % 4000,
            })
            .collect();
        let mut coded = Vec::new();
        encode(&line, &p, &mut coded).unwrap();
        let mut out = vec![0u32; line.len()];
        let d = decode(&coded, &p, &mut out).unwrap();
        assert_eq!(d.samples, line.len());
        assert_eq!(out, line);
        assert!(d.bytes() <= coded.len());
    }
}

mod alloc {
    use super::alloc_count::count;
    use sdrkit_algo_fec::conv::{codes, Encoder};
    use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
    use sdrkit_proto_ccsds::codesync::{CodeConfig, CodeSync};
    use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
    use sdrkit_proto_ccsds::packet::{self, Assembler, AssemblerConfig, PrimaryHeader, SeqFlags};
    use sdrkit_proto_ccsds::{aos, rice, xrit};

    #[test]
    fn steady_state_paths_do_not_allocate() {
        // A coded stream of CADUs carrying packets.
        let layout = aos::Layout::RS_I4;
        let mut stream = Vec::new();
        let mut coder = ChannelCoder::new(ChannelConfig::rs_223(4)).unwrap();
        let mut frame = vec![0u8; layout.frame_len];
        let mut pkts = Vec::new();
        for i in 0..40u16 {
            packet::build(
                &PrimaryHeader::telemetry(77, SeqFlags::Unsegmented, i),
                &[i as u8; 500],
                &mut pkts,
            )
            .unwrap();
        }
        let zones = packet::pack_zones(&[&pkts], layout.data_zone_len()).unwrap();
        for (i, (fh, z)) in zones.iter().enumerate() {
            let h = aos::PrimaryHeader {
                vcid: 2,
                counter: i as u32,
                ..aos::PrimaryHeader::default()
            };
            layout.build(&h, &[], *fh, z, &mut frame).unwrap();
            coder.encode(&frame, &mut stream).unwrap();
        }
        let bits = super::bits_of(&stream);
        let mut coded = Vec::new();
        Encoder::new(codes::CCSDS_K7).encode(&bits, &mut coded);
        let llr: Vec<i8> = coded
            .iter()
            .map(|&c| if c == 0 { 40 } else { -40 })
            .collect();

        let mut cs = CodeSync::new(CodeConfig::GEO).unwrap();
        let mut decoded = Vec::with_capacity(bits.len() + 4096);
        let mut fs = FrameSync::new(SyncConfig::GEO).unwrap();
        let mut dec = ChannelDecoder::new(ChannelConfig::rs_223(4)).unwrap();
        let mut asm = Assembler::new(AssemblerConfig::default());
        let mut files = xrit::transport::FileAssembler::new(Default::default());
        let mut body = vec![0u8; 1020];
        let mut n_packets = 0;
        // Warm up: the first frames grow the buffers of the virtual channel and the file slots.
        let warm = 16384 * 4;
        cs.push(&llr[..warm], &mut decoded);
        let mut run = |bits: &[u8], n_packets: &mut usize| {
            fs.push_bits(bits, |f| {
                body.copy_from_slice(f.bytes);
                let d = dec.decode(&mut body).unwrap();
                let v = layout.parse(&body[..d.frame_len]).unwrap();
                asm.push_mpdu(
                    v.header.vcid,
                    Some(v.header.counter),
                    v.first_header,
                    v.zone,
                    |p| {
                        *n_packets += 1;
                        files.push(&p, |_| {});
                    },
                );
            });
        };
        run(&decoded, &mut n_packets);
        let mark = decoded.len();
        let a = count(|| cs.push(&llr[warm..], &mut decoded));
        assert_eq!(a, 0, "code sync");
        let a = count(|| run(&decoded[mark..], &mut n_packets));
        assert_eq!(a, 0, "frame sync, channel, packets");
        assert!(n_packets >= 30, "{n_packets}");

        let p = rice::Params::new(10, 16, 170).unwrap();
        let line: Vec<u32> = (0..2712).map(|i| (i * 7 % 1024) as u32).collect();
        let mut c = Vec::new();
        rice::encode(&line, &p, &mut c).unwrap();
        let mut out = vec![0u32; 2712];
        let a = count(|| {
            for _ in 0..100 {
                rice::decode(&c, &p, &mut out).unwrap();
            }
        });
        assert_eq!(a, 0, "rice");
    }
}

mod cadufind {
    use sdrkit_proto_ccsds::aos::{Layout, PrimaryHeader};
    use sdrkit_proto_ccsds::cadufind::CaduFinder;
    use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
    use sdrkit_proto_ccsds::tm::FirstHeader;

    fn cadu(n: u8) -> Vec<u8> {
        let mut c = vec![0x1A, 0xCF, 0xFC, 0x1D];
        c.extend((0..2044u32).map(|i| (i as u8) ^ n));
        c
    }

    #[test]
    fn cadus_behind_junk_and_split_across_pieces_are_found() {
        let mut stream = vec![0x11, 0x22, 0x33];
        stream.extend(cadu(1));
        stream.extend(cadu(2));
        stream.extend_from_slice(&[0x1A, 0xCF, 0xFC]);
        let mut f = CaduFinder::new(2048);
        let mut got = Vec::new();
        let (a, b) = stream.split_at(1500);
        f.push(a, |c| got.push(c.to_vec()));
        f.push(b, |c| got.push(c.to_vec()));
        assert_eq!(got, [cadu(1), cadu(2)]);
        assert_eq!(f.stats.skipped, 3);
        // A loss empties the stream.
        f.reset();
        f.push(&cadu(3), |_| panic!("no marker after it"));
    }

    #[test]
    fn a_secondary_header_outside_the_length_field_is_counted_in() {
        // APID 0x20, unsegmented, secondary header flag set, length field 35: 6 + 36 + 8 = 50 bytes.
        let mut pkt = vec![0x08, 0x20, 0xC0, 0x00, 0x00, 0x23];
        pkt.extend((0..44u8).map(|i| i + 1));
        let layout = Layout::CADU_2048;
        assert_eq!(layout.data_zone_len(), 2034);
        let mut zone = pkt.clone();
        sdrkit_proto_ccsds::packet::idle(layout.data_zone_len() - pkt.len(), &mut zone).unwrap();
        let mut frame = vec![0u8; layout.frame_len];
        let h = PrimaryHeader {
            spacecraft: 1,
            vcid: 7,
            ..PrimaryHeader::default()
        };
        layout.build(&h, &[], FirstHeader::At(0), &zone, &mut frame).unwrap();
        let v = layout.parse(&frame).unwrap();
        assert_eq!(v.fecf_ok, Some(true));
        let mut asm = Assembler::new(AssemblerConfig {
            uncounted_secondary: 8,
            ..AssemblerConfig::default()
        });
        let mut got = Vec::new();
        asm.push_mpdu(7, Some(0), v.first_header, v.zone, |p| got.push(p.data.len()));
        assert_eq!(got, [44]);
    }
}
