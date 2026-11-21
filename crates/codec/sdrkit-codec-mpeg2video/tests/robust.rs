//! Damaged input: the decoder never panics, a damaged slice costs only its own macroblocks, and both forms of the
//! arithmetic give the same samples.

mod common;

use common::*;
use sdrkit_codec_mpeg2video::Decoder;

const W: usize = 64;
const H: usize = 64;

struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
}

/// A stream of an I, a P and two B frames with field prediction, then two field pictures: one access unit each.
fn stream() -> Vec<(Vec<u8>, Option<i64>)> {
    let mut w = Writer::new(W, H);
    let mut units = Vec::new();
    let src = Planes::pattern(W, H, 2);
    let mut want = Planes::new(W, H);
    let mut i = Params::new(1);
    i.frame_pred_frame_dct = false;
    w.sequence();
    w.picture(i, 0);
    w.intra_picture(&src, &mut want, 3, &|x, _| x % 2 == 0);
    units.push((w.take(), Some(0)));
    // A vector for macroblock `k` that stays inside the picture.
    let inside = |k: i32, rows: i32| {
        [
            if k % 4 == 3 { -1 } else { k % 3 },
            if k / 4 == rows - 1 { -1 } else { 1 },
        ]
    };
    let frame =
        |v: [i32; 2], back: bool| Mb::Inter(Inter::frame((!back).then_some(v), back.then_some(v)));
    let mut p = Params::new(2);
    p.frame_pred_frame_dct = false;
    let mbs: Vec<Mb> = (0..16)
        .map(|k| {
            if k % 4 == 1 {
                Mb::Skip
            } else {
                frame(inside(k, 4), false)
            }
        })
        .collect();
    w.picture(p, 3);
    w.inter_picture(&mbs, 4);
    units.push((w.take(), Some(10800)));
    let b = Params::new(3);
    for n in 1..3 {
        let mbs: Vec<Mb> = (0..16)
            .map(|k| {
                if k % 4 == 2 {
                    Mb::Skip
                } else {
                    frame(inside(k, 4), k % 3 == 0)
                }
            })
            .collect();
        w.picture(b, n);
        w.inter_picture(&mbs, 4);
        units.push((w.take(), None));
    }
    for (structure, kind) in [(1, 1), (2, 2)] {
        let mut f = Params::new(kind);
        f.structure = structure;
        w.picture(f, 4);
        if kind == 1 {
            w.intra_picture(&src, &mut want, 6, &|_, _| false);
        } else {
            let mbs: Vec<Mb> = (0..8)
                .map(|k| {
                    let mut m = Inter::frame(Some(inside(k, 2)), None);
                    m.kind = Kind::Field;
                    Mb::Inter(m)
                })
                .collect();
            w.inter_picture(&mbs, 6);
        }
        units.push((w.take(), None));
    }
    units
}

fn decode(units: &[(Vec<u8>, Option<i64>)], portable: bool) -> (Vec<Shown>, Decoder) {
    let mut dec = Decoder::new();
    dec.set_portable(portable);
    let out = decode_all(&mut dec, units);
    (out, dec)
}

#[test]
fn the_clean_stream_decodes_without_an_error() {
    let (out, dec) = decode(&stream(), false);
    assert_eq!(out.len(), 5);
    assert_eq!(
        (
            dec.stats().slice_errors,
            dec.stats().concealed,
            dec.stats().dropped
        ),
        (0, 0, 0)
    );
}

#[test]
fn random_bytes_do_not_panic() {
    let mut random = Random(7);
    for round in 0..300 {
        let mut data: Vec<u8> = (0..2000).map(|_| random.next() as u8).collect();
        // Start codes of every kind at random places, and headers that make the slices count.
        for _ in 0..40 {
            let at = random.next() as usize % (data.len() - 4);
            let code =
                [0x00, 0xb3, 0xb5, 0xb8, 0xb7, 1, 2, 3, 4, 0xaf][random.next() as usize % 10];
            data[at..at + 4].copy_from_slice(&[0, 0, 1, code]);
        }
        let mut units = stream();
        let cut = random.next() as usize % units[0].0.len();
        units[0].0.truncate(cut.max(40));
        units.insert(1 + round % 3, (data, None));
        for portable in [false, true] {
            decode(&units, portable);
        }
    }
}

#[test]
fn damaged_and_truncated_streams_do_not_panic_and_both_forms_agree() {
    let clean = stream();
    let mut random = Random(99);
    for round in 0..1500 {
        let mut units = clean.clone();
        let n = random.next() as usize % units.len();
        let unit = &mut units[n].0;
        match round % 4 {
            0 => unit.truncate(random.next() as usize % unit.len()),
            1 => {
                for _ in 0..1 + round % 5 {
                    let at = random.next() as usize % unit.len();
                    unit[at] ^= 1 << (random.next() % 8);
                }
            }
            2 => {
                let at = random.next() as usize % unit.len();
                let end = (at + 1 + random.next() as usize % 16).min(unit.len());
                unit[at..end].fill(if round % 8 == 2 { 0 } else { 0xff });
            }
            _ => {
                let at = random.next() as usize % unit.len();
                unit.insert(at, random.next() as u8);
            }
        }
        let (a, _) = decode(&units, false);
        let (b, _) = decode(&units, true);
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert!(a.planes == b.planes, "round {round}: the two forms differ");
        }
    }
}

#[test]
fn a_damaged_slice_costs_only_its_own_row() {
    let clean = stream();
    let (good, _) = decode(&clean, false);
    // Damage the second slice of the P frame: its quantiser scale code becomes 0.
    let mut units = clean.clone();
    let unit = &mut units[1].0;
    let at = unit
        .windows(4)
        .position(|w| w == [0, 0, 1, 2])
        .expect("the slice of the second row");
    unit[at + 4] &= 0x07;
    let (bad, dec) = decode(&units, false);
    assert_eq!(dec.stats().slice_errors, 1);
    assert_eq!(dec.stats().concealed, 4);
    let (p_good, p_bad) = (&good[3], &bad[3]);
    assert_eq!(p_bad.concealed, 4);
    // The P frame and the B frames that are predicted from it are damaged; the I pictures are not.
    let marks: Vec<bool> = bad.iter().map(|f| f.damaged).collect();
    assert_eq!(marks, [false, true, true, true, false]);
    assert!(good.iter().all(|f| !f.damaged));
    for row in 0..H {
        let range = row * W..(row + 1) * W;
        if (16..32).contains(&row) {
            // The row of macroblocks is a copy of the frame before.
            assert_eq!(p_bad.planes.y[range.clone()], bad[0].planes.y[range]);
        } else {
            assert_eq!(p_bad.planes.y[range.clone()], p_good.planes.y[range]);
        }
    }
}

#[test]
fn every_truncation_of_the_first_unit_is_safe() {
    let clean = stream();
    for cut in 0..clean[0].0.len() {
        let mut units = clean.clone();
        units[0].0.truncate(cut);
        decode(&units, cut % 2 == 0);
    }
}

#[test]
fn lost_data_damages_only_when_a_picture_is_missing() {
    let units = stream();
    let decode = |skip: Option<usize>| {
        let mut dec = Decoder::new();
        let mut out = Vec::new();
        for (n, (bytes, pts)) in units.iter().enumerate() {
            if Some(n) == skip {
                dec.discontinuity();
                continue;
            }
            // Data that is no picture of this stream was lost before the P frame.
            if skip.is_none() && n == 1 {
                dec.discontinuity();
            }
            dec.decode(
                bytes,
                pts.or(Some([0, 10800, 3600, 7200, 14400, 16200][n])),
                &mut |f| out.push(shown(f)),
            );
        }
        dec.flush(&mut |f| out.push(shown(f)));
        out.iter()
            .map(|f| (f.pts.unwrap(), f.damaged))
            .collect::<Vec<_>>()
    };
    let clean = [
        (0, false),
        (3600, false),
        (7200, false),
        (10800, false),
        (14400, false),
    ];
    assert_eq!(decode(None), clean);
    // The first B frame is lost: the frames around it are as coded.
    let without_b = [(0, false), (7200, false), (10800, false), (14400, false)];
    assert_eq!(decode(Some(2)), without_b);
    // The P frame is lost: the B frames have nothing to be predicted from and do not come.
    assert_eq!(decode(Some(1)), [(0, false), (14400, false)]);
}
