//! Decoding of written test streams against samples that are known: intra pictures with every coding choice, P and B
//! pictures with every kind of prediction as frame pictures and as field pictures, and the order of output.

#![allow(clippy::needless_range_loop, clippy::type_complexity)]

mod common;

use common::*;
use sdrkit_codec_mpeg2video::{Decoder, PictureKind};

const W: usize = 64;
const H: usize = 64;
const MB_W: usize = W / 16;

/// A vector for macroblock number `i` that keeps a block of 16 by `h` at (`x`, `y`) inside `rows` rows.
fn vector(i: usize, (x, y, h): (usize, usize, usize), rows: usize) -> [i32; 2] {
    const LIST: [[i32; 2]; 12] = [
        [0, 0],
        [3, -2],
        [-5, 4],
        [8, 7],
        [1, 1],
        [-7, -9],
        [12, -3],
        [2, 5],
        [-1, 0],
        [0, -1],
        [15, 15],
        [-16, -16],
    ];
    let v = LIST[(i * 7 + 3) % LIST.len()];
    let (x, y) = (x as i32, y as i32);
    [
        v[0].clamp(-2 * x, 2 * (W as i32 - 16 - x)),
        v[1].clamp(-2 * y, 2 * (rows as i32 - h as i32 - y)),
    ]
}

/// An intra frame of `seed` as one access unit with a sequence header, and the samples it must give within 1.
fn intra_unit(w: &mut Writer, p: Params, seed: u32, scale: u8) -> (Vec<u8>, Planes) {
    let src = Planes::pattern(W, H, seed);
    let mut want = Planes::new(W, H);
    w.sequence();
    w.picture(p, 0);
    w.intra_picture(&src, &mut want, scale, &|x, y| (x + y) % 2 == 0);
    (w.take(), want)
}

#[test]
fn intra_pictures_give_the_samples_of_the_exact_transform() {
    for choice in 0..16u8 {
        for scale in [1, 4, 13, 31] {
            let mut p = Params::new(1);
            p.intra_vlc_one = choice & 1 != 0;
            p.alternate_scan = choice & 2 != 0;
            p.non_linear_scale = choice & 4 != 0;
            p.frame_pred_frame_dct = choice & 8 == 0;
            p.dc_precision = choice % 4;
            let mut w = Writer::new(W, H);
            let (unit, want) = intra_unit(&mut w, p, choice as u32, scale);
            let mut dec = Decoder::new();
            let out = decode_all(&mut dec, &[(unit, Some(1234))]);
            assert_eq!(out.len(), 1);
            let f = &out[0];
            assert_eq!(dec.stats().slice_errors, 0, "choice {choice} scale {scale}");
            assert!(
                distance(&f.planes, &want) <= 1,
                "choice {choice} scale {scale}: {}",
                distance(&f.planes, &want)
            );
            assert_eq!(
                (f.size, f.aspect, f.frame_rate),
                ((64, 64), (16, 9), (25, 1))
            );
            assert_eq!(
                (f.pts, f.kind, f.interlaced, f.top_field_first),
                (Some(1234), PictureKind::I, true, true)
            );
            assert_eq!(f.concealed, 0);
        }
    }
}

/// v / 2 to the nearest whole number, away from zero at one half.
fn half(v: i32) -> i32 {
    (v as f64 / 2.0).round() as i32
}

/// The samples of a frame picture of `mbs` predicted from `refs` (forward, backward). The tolerance is 1 in a
/// macroblock with a residual and 0 elsewhere; `soft` takes the numbers of the former.
fn frame_picture(
    mbs: &[Mb],
    p: &Params,
    refs: [Option<&Planes>; 2],
    scale: i32,
    soft: &mut Vec<usize>,
) -> Planes {
    let mut to = Planes::new(W, H);
    let mut last: Option<Inter> = None;
    for (i, mb) in mbs.iter().enumerate() {
        let (x, y) = (16 * (i % MB_W), 16 * (i / MB_W));
        let m = match mb {
            Mb::Inter(m) => *m,
            Mb::Skip if p.kind == 2 => Inter::frame(Some([0, 0]), None),
            Mb::Skip => {
                let l = last.expect("a macroblock before a skipped one");
                Inter::frame(l.vectors[0].map(|v| v[0]), l.vectors[1].map(|v| v[0]))
            }
        };
        last = Some(m);
        let mut sides = Vec::new();
        for s in 0..2 {
            let (Some(v), Some(from)) = (m.vectors[s], refs[s]) else {
                continue;
            };
            match m.kind {
                Kind::Frame => sides.push(vec![(
                    (0, 1),
                    (x, y, 16),
                    fetch_all(from, (0, 1), (x, y, 16), v[0]),
                )]),
                Kind::Field => sides.push(
                    (0..2)
                        .map(|r| {
                            (
                                (r, 2),
                                (x, y / 2, 8),
                                fetch_all(from, (m.field[r][s], 2), (x, y / 2, 8), v[r]),
                            )
                        })
                        .collect(),
                ),
                Kind::DualPrime => sides.push(
                    (0..2)
                        .map(|f| {
                            let k = if p.top_field_first == (f == 0) { 1 } else { 3 };
                            let e = 2 * f as i32 - 1;
                            let other = [
                                half(k * v[0][0]) + m.diff[0],
                                half(k * v[0][1]) + m.diff[1] + e,
                            ];
                            let same = fetch_all(from, (f, 2), (x, y / 2, 8), v[0]);
                            let opposite = fetch_all(from, (1 - f, 2), (x, y / 2, 8), other);
                            ((f, 2), (x, y / 2, 8), average(&same, &opposite))
                        })
                        .collect(),
                ),
                Kind::Halves => unreachable!(),
            }
        }
        if m.vectors[0].is_none() && p.kind == 2 {
            let from = refs[0].unwrap();
            sides.push(vec![(
                (0, 1),
                (x, y, 16),
                fetch_all(from, (0, 1), (x, y, 16), [0, 0]),
            )]);
        }
        for k in 0..sides[0].len() {
            let (view, at, mut pred) = sides[0][k].clone();
            if let Some(b) = sides.get(1) {
                pred = average(&pred, &b[k].2);
            }
            place(&mut to, view, at, &pred);
        }
        if let Some(level) = m.residual {
            soft.push(i);
            let mut q = [0; 64];
            q[0] = level;
            let r = dct(&dequantise(&q, false, &[16; 64], scale, 0), false);
            for j in 0..64 {
                let at = (y + j / 8) * W + x + j % 8;
                to.y[at] = (to.y[at] as f64 + r[j].round()).clamp(0.0, 255.0) as u8;
            }
        }
    }
    to
}

/// Compares a decoded frame with the samples wanted: equal, or within 1 in the macroblocks `soft`.
fn compare(got: &Planes, want: &Planes, soft: &[usize], what: &str) {
    for i in 0..MB_W * (H / 16) {
        let (x, y) = (16 * (i % MB_W), 16 * (i / MB_W));
        let limit = soft.contains(&i) as u8;
        for c in 0..3 {
            let (w, x, y, n) = if c == 0 {
                (W, x, y, 16)
            } else {
                (W / 2, x / 2, y / 2, 8)
            };
            for j in 0..n {
                for k in 0..n {
                    let at = (y + j) * w + x + k;
                    let (a, b) = (got.plane(c)[at], want.plane(c)[at]);
                    assert!(
                        a.abs_diff(b) <= limit,
                        "{what}: macroblock {i} plane {c} at ({k}, {j}): {a} for {b}"
                    );
                }
            }
        }
    }
}

/// The macroblocks of a P frame picture: every kind that `p` allows, skipped ones and residuals.
fn p_frame_mbs(p: &Params, dual: bool) -> Vec<Mb> {
    (0..MB_W * (H / 16))
        .map(|i| {
            let (mx, my) = (i % MB_W, i / MB_W);
            let (x, y) = (16 * mx, 16 * my);
            if (mx == 1 && my % 2 == 0) || (mx == 2 && my == 1) {
                return Mb::Skip;
            }
            let mut m = Inter::frame(Some(vector(i, (x, y, 16), H)), None);
            if !p.frame_pred_frame_dct && i % 3 != 0 {
                let v = [
                    vector(i, (x, y / 2, 8), H / 2),
                    vector(i + 5, (x, y / 2, 8), H / 2),
                ];
                m.vectors[0] = Some(v);
                if dual && i % 3 == 1 {
                    m.kind = Kind::DualPrime;
                    // Keep the derived vectors inside the picture.
                    m.vectors[0] =
                        Some([[v[0][0].clamp(-2 * x as i32 / 3, 8), 2 * (1 - my as i32)]; 2]);
                    m.diff = [[0, 1], [1, -1], [-1, 0]][i % 3];
                    if mx == 3 {
                        m.vectors[0] = Some([[-4, 2 * (1 - my as i32)]; 2]);
                    }
                } else {
                    m.kind = Kind::Field;
                    m.field = [[i % 2, 0], [(i / 2) % 2, 0]];
                }
            }
            if i % 4 == 3 {
                m.residual = Some([3, -1, 1, -7][my]);
            }
            if i == 4 && m.kind == Kind::Frame {
                m.vectors[0] = None;
                m.residual = Some(2);
            }
            Mb::Inter(m)
        })
        .collect()
}

#[test]
fn p_frames_follow_their_vectors() {
    for (choice, dual) in [(false, false), (true, false), (true, true)] {
        for tff in [true, false] {
            let mut w = Writer::new(W, H);
            let (first, _) = intra_unit(&mut w, Params::new(1), 3, 4);
            let mut p = Params::new(2);
            p.frame_pred_frame_dct = !choice;
            p.top_field_first = tff;
            let mbs = p_frame_mbs(&p, dual);
            w.picture(p, 1);
            w.inter_picture(&mbs, 6);
            let second = w.take();
            let mut dec = Decoder::new();
            let out = decode_all(&mut dec, &[(first, Some(0)), (second, None)]);
            assert_eq!(dec.stats().slice_errors, 0);
            assert_eq!(out.len(), 2);
            assert_eq!((out[1].kind, out[1].pts), (PictureKind::P, Some(3600)));
            let mut soft = Vec::new();
            let want = frame_picture(&mbs, &p, [Some(&out[0].planes), None], 12, &mut soft);
            compare(
                &out[1].planes,
                &want,
                &soft,
                &format!("field prediction {choice}, dual prime {dual}, tff {tff}"),
            );
        }
    }
}

/// The macroblocks of a B frame picture.
fn b_frame_mbs(p: &Params, turn: usize) -> Vec<Mb> {
    (0..MB_W * (H / 16))
        .map(|i| {
            let (mx, my) = (i % MB_W, i / MB_W);
            let (x, y) = (16 * mx, 16 * my);
            if mx == 2 && my != 2 {
                return Mb::Skip;
            }
            let field = !p.frame_pred_frame_dct && mx == 0;
            let pick = |k: usize| {
                if field {
                    [
                        vector(i + k, (x, y / 2, 8), H / 2),
                        vector(i + k + 3, (x, y / 2, 8), H / 2),
                    ]
                } else {
                    [vector(i + k, (x, y, 16), H); 2]
                }
            };
            let mut m = Inter::frame(None, None);
            match (i + turn) % 3 {
                0 => m.vectors[0] = Some(pick(1)),
                1 => m.vectors[1] = Some(pick(2)),
                _ => m.vectors = [Some(pick(4)), Some(pick(6))],
            }
            if field {
                m.kind = Kind::Field;
                m.field = [[i % 2, (i / 2) % 2], [1 - i % 2, 1]];
            }
            if i == 9 {
                m.residual = Some(-4);
            }
            Mb::Inter(m)
        })
        .collect()
}

#[test]
fn b_frames_come_between_their_anchors() {
    for field_prediction in [false, true] {
        let mut w = Writer::new(W, H);
        let (i0, _) = intra_unit(&mut w, Params::new(1), 5, 3);
        let mut p = Params::new(2);
        p.frame_pred_frame_dct = !field_prediction;
        let p_mbs = p_frame_mbs(&p, false);
        w.picture(p, 3);
        w.inter_picture(&p_mbs, 5);
        let p3 = w.take();
        let mut b = Params::new(3);
        b.frame_pred_frame_dct = !field_prediction;
        let b_mbs = [b_frame_mbs(&b, 0), b_frame_mbs(&b, 1)];
        w.picture(b, 1);
        w.inter_picture(&b_mbs[0], 5);
        let b1 = w.take();
        w.picture(b, 2);
        w.inter_picture(&b_mbs[1], 9);
        let b2 = w.take();
        let mut dec = Decoder::new();
        let out = decode_all(
            &mut dec,
            &[
                (i0, Some(0)),
                (p3, Some(10800)),
                (b1, Some(3600)),
                (b2, None),
            ],
        );
        assert_eq!(dec.stats().slice_errors, 0);
        let order: Vec<_> = out.iter().map(|f| (f.kind, f.pts)).collect();
        let want = [
            (PictureKind::I, Some(0)),
            (PictureKind::B, Some(3600)),
            (PictureKind::B, Some(7200)),
            (PictureKind::P, Some(10800)),
        ];
        assert_eq!(order, want);
        let refs = [Some(&out[0].planes), Some(&out[3].planes)];
        for (k, scale) in [(0, 10), (1, 18)] {
            let mut soft = Vec::new();
            let want = frame_picture(&b_mbs[k], &b, refs, scale, &mut soft);
            compare(
                &out[1 + k].planes,
                &want,
                &soft,
                &format!("B picture {k}, field prediction {field_prediction}"),
            );
        }
    }
}

/// The samples of one field picture of `mbs` in `to`, which holds the decoded frame. `parity` is 0 for the top field.
fn field_picture(
    mbs: &[Mb],
    kind: u8,
    parity: usize,
    second: bool,
    refs: [Option<&Planes>; 2],
    to: &mut Planes,
) {
    let decoded = to.clone();
    let mut last: Option<Inter> = None;
    for (i, mb) in mbs.iter().enumerate() {
        let (x, y) = (16 * (i % MB_W), 16 * (i / MB_W));
        let field_of = |m: &mut Inter| {
            m.kind = Kind::Field;
            m.field = [[parity; 2]; 2];
        };
        let m = match mb {
            Mb::Inter(m) => *m,
            Mb::Skip if kind == 2 => {
                let mut m = Inter::frame(Some([0, 0]), None);
                field_of(&mut m);
                m
            }
            Mb::Skip => {
                let l = last.expect("a macroblock before a skipped one");
                let mut m = Inter::frame(l.vectors[0].map(|v| v[0]), l.vectors[1].map(|v| v[0]));
                field_of(&mut m);
                m
            }
        };
        last = Some(m);
        let source = |s: usize, field: usize| -> &Planes {
            if kind == 2 && second && field != parity {
                &decoded
            } else {
                refs[s].expect("a frame to predict from")
            }
        };
        let mut sides: Vec<Vec<((usize, usize, usize), [Vec<u8>; 3])>> = Vec::new();
        for s in 0..2 {
            let Some(v) = m.vectors[s] else { continue };
            sides.push(match m.kind {
                Kind::Field => {
                    vec![(
                        (x, y, 16),
                        fetch_all(
                            source(s, m.field[0][s]),
                            (m.field[0][s], 2),
                            (x, y, 16),
                            v[0],
                        ),
                    )]
                }
                Kind::Halves => (0..2)
                    .map(|r| {
                        let at = (x, y + 8 * r, 8);
                        (
                            at,
                            fetch_all(source(s, m.field[r][s]), (m.field[r][s], 2), at, v[r]),
                        )
                    })
                    .collect(),
                Kind::DualPrime => {
                    let e = 2 * parity as i32 - 1;
                    let other = [half(v[0][0]) + m.diff[0], half(v[0][1]) + m.diff[1] + e];
                    let same = fetch_all(source(s, parity), (parity, 2), (x, y, 16), v[0]);
                    let opposite =
                        fetch_all(source(s, 1 - parity), (1 - parity, 2), (x, y, 16), other);
                    vec![((x, y, 16), average(&same, &opposite))]
                }
                Kind::Frame => unreachable!(),
            });
        }
        for k in 0..sides[0].len() {
            let (at, mut pred) = sides[0][k].clone();
            if let Some(b) = sides.get(1) {
                pred = average(&pred, &b[k].1);
            }
            place(to, (parity, 2), at, &pred);
        }
    }
}

/// The macroblocks of a P or B field picture. `fields` are the fields that a vector may point into.
fn field_mbs(kind: u8, turn: usize, fields: &[usize], dual: bool) -> Vec<Mb> {
    (0..MB_W * (H / 32))
        .map(|i| {
            let (mx, my) = (i % MB_W, i / MB_W);
            let (x, y) = (16 * mx, 16 * my);
            // A skipped macroblock of a P field needs the field of the same parity of the frame before.
            if mx == 1 + my && fields.len() > 1 {
                return Mb::Skip;
            }
            let mut m = Inter::frame(None, None);
            m.kind = Kind::Field;
            let f = |k: usize| fields[(i + k) % fields.len()];
            m.field = [[f(0), f(1)], [f(2), f(3)]];
            let whole = |k: usize| [vector(i + k, (x, y, 16), H / 2); 2];
            let halves = |k: usize| {
                [
                    vector(i + k, (x, y, 8), H / 2),
                    vector(i + k + 1, (x, y + 8, 8), H / 2),
                ]
            };
            let split = (i + turn) % 4 == 1 && mx != my;
            if split {
                m.kind = Kind::Halves;
            }
            let pick = |k: usize| if split { halves(k) } else { whole(k) };
            match (kind, (i + turn) % 3) {
                (2, _) | (3, 0) => m.vectors[0] = Some(pick(2)),
                (_, 1) => m.vectors[1] = Some(pick(5)),
                _ => m.vectors = [Some(pick(3)), Some(pick(8))],
            }
            if dual && kind == 2 && !split && i % 2 == 0 {
                m.kind = Kind::DualPrime;
                m.vectors[0] = Some(
                    [[[2, 2], [-3, 1], [4, -2], [0, 3]][i % 4]
                        .map(|v: i32| v.clamp(-(x as i32), 4)); 2],
                );
                m.vectors[0] = m.vectors[0].map(|v| {
                    [[
                        v[0][0],
                        if my == 0 {
                            v[0][1].abs() + 2
                        } else {
                            -v[0][1].abs() - 2
                        },
                    ]; 2]
                });
                m.diff = [[1, 0], [-1, 1], [0, -1], [1, 1]][i % 4];
            }
            Mb::Inter(m)
        })
        .collect()
}

#[test]
fn field_pictures_follow_their_vectors() {
    let mut w = Writer::new(W, H);
    let mut units = Vec::new();
    // Frame 0: an I field (top) and a P field that is predicted from it.
    let src = Planes::pattern(W, H, 9);
    let mut unused = Planes::new(W, H);
    let mut i = Params::new(1);
    i.structure = 1;
    w.sequence();
    w.picture(i, 0);
    w.intra_picture(&src, &mut unused, 5, &|_, _| false);
    units.push((w.take(), Some(0)));
    let mut p = Params::new(2);
    p.structure = 2;
    let f0_bottom = field_mbs(2, 0, &[0], false);
    w.picture(p, 0);
    w.inter_picture(&f0_bottom, 5);
    units.push((w.take(), None));
    // Frame 3: two P fields, bottom field first, with dual prime.
    let mut p3 = [Params::new(2); 2];
    p3[0].structure = 2;
    p3[1].structure = 1;
    let f3 = [
        field_mbs(2, 1, &[0, 1], true),
        field_mbs(2, 2, &[1, 0], true),
    ];
    for k in 0..2 {
        w.picture(p3[k], 3);
        w.inter_picture(&f3[k], 7);
        units.push((w.take(), (k == 0).then_some(10800)));
    }
    // Frames 1 and 2: B fields, both in one access unit.
    let mut b = [Params::new(3); 2];
    b[0].structure = 1;
    b[1].structure = 2;
    let mut f12 = Vec::new();
    for n in 0..2 {
        let mbs = [
            field_mbs(3, n, &[0, 1], false),
            field_mbs(3, n + 1, &[1, 0], false),
        ];
        for k in 0..2 {
            w.picture(b[k], 1 + n as u32);
            w.inter_picture(&mbs[k], 7);
        }
        units.push((w.take(), (n == 0).then_some(3600)));
        f12.push(mbs);
    }
    let mut dec = Decoder::new();
    for (n, u) in units.iter().enumerate() {
        dec.decode(&u.0, u.1, &mut |_| ());
        assert_eq!(dec.stats().slice_errors, 0, "access unit {n}");
    }
    let mut dec = Decoder::new();
    let out = decode_all(&mut dec, &units);
    let order: Vec<_> = out
        .iter()
        .map(|f| (f.kind, f.pts, f.top_field_first))
        .collect();
    let want = [
        (PictureKind::I, Some(0), true),
        (PictureKind::B, Some(3600), true),
        (PictureKind::B, Some(7200), true),
        (PictureKind::P, Some(10800), false),
    ];
    assert_eq!(order, want);
    assert!(out.iter().all(|f| f.concealed == 0));

    let mut frame0 = out[0].planes.clone();
    field_picture(&f0_bottom, 2, 1, true, [None, None], &mut frame0);
    compare(&out[0].planes, &frame0, &[], "frame 0, bottom field");

    let mut frame3 = out[3].planes.clone();
    field_picture(
        &f3[0],
        2,
        1,
        false,
        [Some(&out[0].planes), None],
        &mut frame3,
    );
    compare(&out[3].planes, &frame3, &[], "frame 3, bottom field");
    field_picture(
        &f3[1],
        2,
        0,
        true,
        [Some(&out[0].planes), None],
        &mut frame3,
    );
    compare(&out[3].planes, &frame3, &[], "frame 3, top field");

    for n in 0..2 {
        let mut frame = out[1 + n].planes.clone();
        for k in 0..2 {
            field_picture(
                &f12[n][k],
                3,
                k,
                k == 1,
                [Some(&out[0].planes), Some(&out[3].planes)],
                &mut frame,
            );
        }
        compare(&out[1 + n].planes, &frame, &[], &format!("frame {}", 1 + n));
    }
}
