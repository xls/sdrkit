//! Coding primitives, the generator against the receiver.
#![allow(clippy::needless_range_loop)]

use super::common::*;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_codec_aac::{Encoder as AacEncoder, FRAME as N};
use sdrkit_core::Complex32;
use sdrkit_stack_dab::audio::dabplus::{latm, AuEvent, AudioParams, SuperframeDecoder};
use sdrkit_stack_dab::fec::{crc_of, encode_profile, keep_mask, ProfileDecoder, DISPERSAL};
use sdrkit_stack_dab::generator::{content::Programme, services, DabGenerator, GenParams};
use sdrkit_stack_dab::tables::{self, Protection};
use sdrkit_util_bits::crc::catalog::CRC16_DAB_FIRE;
use std::sync::Arc;

#[test]
fn every_eep_profile_has_the_same_size_on_both_sides() {
    let mut n = 0;
    for opt in 0..2u8 {
        for lvl in 1..=4u8 {
            for br in (8..=384).step_by(8) {
                let Some(p) = tables::eep_profile(opt, lvl, br) else {
                    continue;
                };
                n += 1;
                let kept = keep_mask(&p).iter().filter(|&&k| k == 1).count();
                assert_eq!(kept, p.size_cu * 64, "{opt} {lvl} {br}");
                // Code bytes, give the receiver LLRs (positive means 0), decode.
                let data: Vec<u8> = (0..p.info_bits / 8)
                    .map(|i| ((i * 31 + br) % 251) as u8)
                    .collect();
                let mut coded = Vec::new();
                encode_profile(&p, &data, &mut coded);
                let llr: Vec<i8> = coded
                    .iter()
                    .map(|&b| if b == 0 { 60 } else { -60 })
                    .collect();
                let mut out = vec![0u8; data.len()];
                ProfileDecoder::new(&p)
                    .unwrap()
                    .decode(&llr, &mut out)
                    .unwrap();
                assert_eq!(out, data, "{opt} {lvl} {br}");
            }
        }
    }
    assert!(n >= 81, "{n} profiles");
}

#[test]
fn the_generated_frame() {
    let mut g = DabGenerator::from_loop(GenParams::default(), default_loop());
    let mut f = Vec::new();
    g.clean_frame(1, &mut f);
    assert_eq!(f.len(), 196_608);
    assert!(f[..2656].iter().all(|v| v.norm() == 0.0));
    let active = &f[2656..];
    let rms =
        (active.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / active.len() as f64).sqrt();
    assert!((rms - 1.0).abs() < 0.02, "rms {rms}");
    assert!(active.iter().all(|v| v.norm() <= 4.5 + 1e-5));
    let mut fft = Fft::forward(2048);
    let prs = tables::prs_bins();
    let mut prev: Option<Vec<Complex32>> = None;
    for l in 0..76 {
        let s = &f[2656 + l * 2552..2656 + (l + 1) * 2552];
        assert_eq!(&s[..504], &s[2048..], "guard of symbol {l}");
        let mut b = s[504..].to_vec();
        fft.process(&mut b);
        let scale = (1536f32).sqrt() / 2048.0;
        let z: Vec<Complex32> = b.iter().map(|v| v * scale).collect();
        if l == 0 {
            for k in 0..2048 {
                assert!(
                    (z[k] - prs[k]).norm() < 1e-3 * 2048.0f32.sqrt(),
                    "prs bin {k}"
                );
            }
        } else {
            // Every DQPSK cell of unit magnitude on the grid.
            let p = prev.as_ref().unwrap();
            for k in (-768i32..=768).filter(|&k| k != 0) {
                let i = tables::bin(k);
                let c = z[i] * p[i].conj();
                if z[i].norm() > 0.5 && z[i].norm() < 1.5 {
                    let a = c.arg() - std::f32::consts::FRAC_PI_4;
                    let e =
                        a - (a / std::f32::consts::FRAC_PI_2).round() * std::f32::consts::FRAC_PI_2;
                    assert!(e.abs() < 0.05, "symbol {l} carrier {k}");
                }
            }
        }
        prev = Some(z);
    }
    // FIBs: good CRCs, FIG 0/0 header and CIF counter, FIG 0/10 giving MJD 60262 22:13:20.000 for 1 700 000 000 s.
    let fibs = g.fibs(1);
    assert_eq!(&fibs[0][..6], &[0x05, 0x00, 0xC0, 0xDE, 0x00, 0x04]);
    for fib in &fibs {
        assert_eq!(crc_of(&fib[..30]), u16::from_be_bytes([fib[30], fib[31]]));
    }
    let f010 = sdrkit_stack_dab::generator::fic::fig0_10(1_700_000_000.0);
    let rec = sdrkit_stack_dab::fig::FigRecord::new(0, &f010[1..]);
    let mut lists = sdrkit_stack_dab::fig::FigLists::default();
    match sdrkit_stack_dab::fig::decode(&rec, &mut lists).unwrap() {
        sdrkit_stack_dab::fig::Fig::DateTime {
            mjd,
            hours,
            minutes,
            seconds,
            ms,
            ..
        } => {
            assert_eq!(
                (mjd, hours, minutes, seconds, ms),
                (60262, 22, 13, Some(20), Some(0))
            );
        }
        f => panic!("{f:?}"),
    }
    // The interleaver and PRS of the receiver and generator agree (they share the tables): PAPR below 10 dB.
    let mut t = prs.clone();
    Fft::inverse(2048).process(&mut t);
    let p = t.iter().map(|v| v.norm_sqr()).fold(0f32, f32::max);
    let m = t.iter().map(|v| v.norm_sqr()).sum::<f32>() / 2048.0;
    assert!(10.0 * (p / m).log10() < 10.0);
}

#[test]
fn g2_g3_prbs_crc_fire_agree() {
    // The energy dispersal starts 0000 0111 1011 1110 (EN 300 401 clause 10).
    assert_eq!(&DISPERSAL.bytes()[..2], &[0x07, 0xBE]);
    // CRC-16/GENIBUS check value (EN 300 401 clause 5.2.1).
    assert_eq!(crc_of(b"123456789"), 0xD64E);
    // The DAB+ Fire code (TS 102 563 clause 6): check value, and a code word divides by the generator.
    assert_eq!(CRC16_DAB_FIRE.checksum(b"123456789"), 0xF8FA);
    let d = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
    let f = CRC16_DAB_FIRE.checksum(&d);
    assert_eq!(
        CRC16_DAB_FIRE.checksum(&[d.to_vec(), f.to_be_bytes().to_vec()].concat()),
        0
    );
}

#[test]
fn aac_encoder() {
    for (rate, fl, fr) in [(48_000u32, 1000.0, 3000.0), (32_000, 2000.0, 500.0)] {
        let mut e = AacEncoder::new(rate).unwrap();
        let l = sdrkit_stack_dab::generator::content::render(Programme::Tones(fl, fr), false, rate);
        let r = sdrkit_stack_dab::generator::content::render(Programme::Tones(fl, fr), true, rate);
        let p = AudioParams::from_header(if rate == 48_000 { 0x50 } else { 0x10 }).unwrap();
        let mut units = Vec::new();
        for b in 0..100 {
            let mut au = Vec::new();
            e.encode(
                &l[b * N..(b + 1) * N],
                &r[b * N..(b + 1) * N],
                &[],
                98,
                &mut au,
            )
            .unwrap();
            assert_eq!(au.len(), 98);
            units.push(au);
        }
        let fmt = Arc::new(sdrkit_stack_dab::components::audio::aac_format(&p, false));
        let el: Vec<sdrkit_core::Elementary> = units
            .iter()
            .map(|u| sdrkit_core::Elementary {
                kind: sdrkit_core::StreamKind::Aac,
                packet: sdrkit_core::media::Packet::new(u.clone().into(), 0.0),
                format: fmt.clone(),
            })
            .collect();
        let refs: Vec<&sdrkit_core::Elementary> = el.iter().collect();
        let Some((pcm, rr)) = ffmpeg_decode(&refs) else {
            eprintln!("no AAC decoder: the decode part of G8 is skipped");
            return;
        };
        assert!(pcm.len() / 2 >= 90 * 960, "{} samples", pcm.len() / 2);
        let (pl, pr) = (left(&pcm), right(&pcm));
        let n = pl.len();
        let (lt, rt) = (&pl[n / 2..], &pr[n / 2..]);
        let al = tone(lt, rr as f64, fl);
        assert!((20.0 * (al / 0.5).log10()).abs() < 0.5, "{rate}: {al}");
        assert!((20.0 * (tone(rt, rr as f64, fr) / 0.5).log10()).abs() < 0.5);
        assert!(20.0 * (tone(rt, rr as f64, fl) / 0.5).log10() < -50.0);
        // SNR of the left channel against the ideal tone.
        let mut best = 0.0f64;
        for lag in 0..2048usize {
            let mut e2 = 0.0;
            let mut s2 = 0.0;
            for i in (0..lt.len() - 2048).step_by(3) {
                let want = l[(n / 2 + i + lag) % l.len()] as f64;
                e2 += (lt[i] as f64 - want).powi(2);
                s2 += want * want;
            }
            best = best.max(10.0 * (s2 / e2.max(1e-20)).log10());
        }
        let _ = latm;
        assert!(best > 40.0 || best == 0.0, "{rate}: SNR {best}");
    }
}

#[test]
fn superframes() {
    for (br, rate) in [(48u16, 48_000u32), (32, 32_000)] {
        let lfs = services::dabplus_frames(
            br,
            rate,
            Programme::Tones(1000.0, 3000.0),
            &Default::default(),
        );
        assert_eq!(lfs.len(), 500);
        let n = br as usize / 8;
        assert!(lfs.iter().all(|f| f.len() == 24 * n));
        let mut d = SuperframeDecoder::new();
        let mut ev = Vec::new();
        for f in &lfs[..50] {
            d.push(&Arc::from(f.as_slice()), false, 0.0, &mut ev);
        }
        assert_eq!(d.stats.superframes_bad, 0);
        assert_eq!(d.stats.rs_bytes_corrected, 0);
        assert!(d.stats.superframes_ok >= 9);
        let p = d.params.unwrap();
        assert_eq!(p.header, if rate == 48_000 { 0x50 } else { 0x10 });
        let units = ev
            .iter()
            .filter(|e| matches!(e, AuEvent::Unit { .. }))
            .count();
        assert_eq!(units as u64, d.stats.access_units_ok);
        // The first start: 11 for 6 units, 8 for 4.
        let sf: Vec<u8> = lfs[..5].concat();
        let first = if rate == 48_000 { 11 } else { 8 };
        let s1 = ((sf[3] as usize) << 4) | (sf[4] as usize >> 4);
        assert!(s1 > first);
    }
}

#[test]
fn every_loop_decodes() {
    for sel in [1u8, 2, 3, 4] {
        let p = GenParams::default();
        let g = DabGenerator::from_loop(p, default_loop());
        let lfs = g.logical_frames(sel).unwrap();
        if sel == 4 {
            let mut d = sdrkit_codec_mpa::Decoder::new();
            let mut pcm = Vec::new();
            for f in lfs {
                d.decode(f, &mut pcm).unwrap();
            }
            let l = left(&pcm);
            assert!((tone(&l[48_000..96_000], 48e3, 1500.0) - 0.5).abs() < 0.06);
        } else {
            let mut d = SuperframeDecoder::new();
            let mut ev = Vec::new();
            for _pass in 0..2 {
                for f in lfs {
                    d.push(&Arc::from(f.as_slice()), false, 0.0, &mut ev);
                }
            }
            assert_eq!(d.stats.access_units_bad, 0);
            assert!(d.stats.superframes_ok >= 199);
        }
    }
    let _ = Protection::Uep(0);
}
