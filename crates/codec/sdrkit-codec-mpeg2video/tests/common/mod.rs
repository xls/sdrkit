//! A writer of MPEG-2 video test streams: headers, intra pictures from samples, and predicted pictures from a list of
//! macroblocks with known vectors. It also computes the samples that a decoder must give.

#![allow(
    dead_code,
    clippy::needless_range_loop,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of
)]

use sdrkit_codec_mpeg2video::tables::*;
use sdrkit_codec_mpeg2video::{Decoder, Frame, PictureKind};
use std::f64::consts::PI;

/// The three planes of a frame of whole macroblocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planes {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

impl Planes {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Planes {
            width,
            height,
            y: vec![0; n],
            cb: vec![0; n / 4],
            cr: vec![0; n / 4],
        }
    }

    /// A picture with gradients, a texture and an edge, different for each `seed`.
    pub fn pattern(width: usize, height: usize, seed: u32) -> Self {
        let mut p = Planes::new(width, height);
        let s = seed as f64;
        for y in 0..height {
            for x in 0..width {
                let (fx, fy) = (x as f64, y as f64);
                let v = 120.0
                    + 60.0 * ((fx + 3.0 * s) * 0.21).sin() * ((fy - s) * 0.17).cos()
                    + 0.8 * fx
                    + if (x / 8 + y / 8 + seed as usize) % 5 == 0 {
                        35.0
                    } else {
                        0.0
                    }
                    + (((x * 7 + y * 13 + seed as usize * 5) % 11) as f64 - 5.0);
                p.y[y * width + x] = v.clamp(16.0, 235.0) as u8;
            }
        }
        for y in 0..height / 2 {
            for x in 0..width / 2 {
                let (fx, fy) = (x as f64, y as f64);
                p.cb[y * width / 2 + x] = (128.0 + 50.0 * ((fx + s) * 0.3).sin() + fy) as u8;
                p.cr[y * width / 2 + x] = (128.0 + 50.0 * ((fy - s) * 0.25).cos() - fx) as u8;
            }
        }
        p
    }

    pub fn plane(&self, c: usize) -> &[u8] {
        [&self.y, &self.cb, &self.cr][c]
    }

    pub fn plane_mut(&mut self, c: usize) -> &mut Vec<u8> {
        [&mut self.y, &mut self.cb, &mut self.cr][c]
    }
}

/// A decoded frame, copied.
#[derive(Clone, Debug)]
pub struct Shown {
    pub planes: Planes,
    pub pts: Option<i64>,
    pub kind: PictureKind,
    pub interlaced: bool,
    pub top_field_first: bool,
    pub concealed: u32,
    pub damaged: bool,
    pub size: (u32, u32),
    pub aspect: (u32, u32),
    pub frame_rate: (u32, u32),
}

pub fn shown(f: &Frame) -> Shown {
    Shown {
        planes: Planes {
            width: f.stride,
            height: f.y.len() / f.stride,
            y: f.y.to_vec(),
            cb: f.cb.to_vec(),
            cr: f.cr.to_vec(),
        },
        pts: f.pts,
        kind: f.kind,
        interlaced: f.interlaced,
        top_field_first: f.top_field_first,
        concealed: f.concealed,
        damaged: f.damaged,
        size: (f.width, f.height),
        aspect: f.aspect,
        frame_rate: f.frame_rate,
    }
}

/// Decodes the access units `units` (bytes and time stamp) with `dec` and gives the frames in the order they came.
pub fn decode_all(dec: &mut Decoder, units: &[(Vec<u8>, Option<i64>)]) -> Vec<Shown> {
    let mut out = Vec::new();
    for (bytes, pts) in units {
        dec.decode(bytes, *pts, &mut |f| out.push(shown(f)));
    }
    dec.flush(&mut |f| out.push(shown(f)));
    out
}

/// The parameters of a picture.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// 1 I, 2 P, 3 B.
    pub kind: u8,
    /// 1 top field, 2 bottom field, 3 frame.
    pub structure: u8,
    pub f_code: [[u8; 2]; 2],
    pub dc_precision: u8,
    pub top_field_first: bool,
    pub frame_pred_frame_dct: bool,
    pub non_linear_scale: bool,
    pub intra_vlc_one: bool,
    pub alternate_scan: bool,
}

impl Params {
    pub fn new(kind: u8) -> Self {
        Params {
            kind,
            structure: 3,
            f_code: [
                [if kind >= 2 { 3 } else { 15 }; 2],
                [if kind == 3 { 3 } else { 15 }; 2],
            ],
            dc_precision: 0,
            top_field_first: true,
            frame_pred_frame_dct: true,
            non_linear_scale: false,
            intra_vlc_one: false,
            alternate_scan: false,
        }
    }
}

/// How a macroblock of a predicted picture is predicted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Frame,
    Field,
    Halves,
    DualPrime,
}

/// A macroblock of a predicted picture.
#[derive(Clone, Copy, Debug)]
pub struct Inter {
    pub kind: Kind,
    /// The forward and the backward vectors (first, second), in half samples.
    pub vectors: [Option<[[i32; 2]; 2]>; 2],
    /// The field each vector points into: by first or second vector, then by direction.
    pub field: [[usize; 2]; 2],
    /// The differences of dual prime.
    pub diff: [i32; 2],
    /// The level of one coded coefficient at place 0 of the first luminance block.
    pub residual: Option<i32>,
}

impl Inter {
    pub fn frame(forward: Option<[i32; 2]>, backward: Option<[i32; 2]>) -> Self {
        Inter {
            kind: Kind::Frame,
            vectors: [forward.map(|v| [v, v]), backward.map(|v| [v, v])],
            field: [[0; 2]; 2],
            diff: [0; 2],
            residual: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Mb {
    Skip,
    Inter(Inter),
}

fn basis() -> [[f64; 8]; 8] {
    std::array::from_fn(|k| {
        std::array::from_fn(|n| {
            let s = if k == 0 { (0.125f64).sqrt() } else { 0.5 };
            s * ((2 * n + 1) as f64 * k as f64 * PI / 16.0).cos()
        })
    })
}

/// The forward or the inverse DCT in floating point.
pub fn dct(x: &[f64; 64], forward: bool) -> [f64; 64] {
    let c = basis();
    let mut out = [0.0; 64];
    for a in 0..8 {
        for b in 0..8 {
            let mut sum = 0.0;
            for i in 0..8 {
                for j in 0..8 {
                    sum += if forward {
                        c[a][i] * c[b][j] * x[8 * i + j]
                    } else {
                        c[i][a] * c[j][b] * x[8 * i + j]
                    };
                }
            }
            out[8 * a + b] = sum;
        }
    }
    out
}

/// The coefficients that a decoder gets from the levels `q`, by the rules of the standard.
pub fn dequantise(
    q: &[i32; 64],
    intra: bool,
    matrix: &[u8; 64],
    scale: i32,
    dc_precision: u8,
) -> [f64; 64] {
    let mut f = [0i32; 64];
    for i in 0..64 {
        f[i] = if intra && i == 0 {
            q[0] * (8 >> dc_precision)
        } else if intra {
            2 * q[i] * matrix[i] as i32 * scale / 32
        } else {
            (2 * q[i] + q[i].signum()) * matrix[i] as i32 * scale / 32
        }
        .clamp(-2048, 2047);
    }
    if f.iter().sum::<i32>() % 2 == 0 {
        f[63] += if f[63] % 2 == 0 { 1 } else { -1 };
    }
    f.map(|v| v as f64)
}

/// The place of block `b` of a macroblock in its plane: plane, first sample and distance between rows, for a
/// picture whose rows start at `first` and are `pitch` rows of the frame apart.
fn block_place(
    b: usize,
    (mx, my): (usize, usize),
    width: usize,
    (first, pitch): (usize, usize),
    field_dct: bool,
) -> (usize, usize, usize) {
    if b >= 4 {
        let w = width / 2;
        return (b - 3, (first + pitch * 8 * my) * w + 8 * mx, pitch * w);
    }
    let (row, step) = if field_dct {
        (b >> 1, 2)
    } else {
        (8 * (b >> 1), 1)
    };
    (
        0,
        (first + pitch * (16 * my + row)) * width + 16 * mx + 8 * (b & 1),
        pitch * step * width,
    )
}

/// The stream writer.
pub struct Writer {
    pub out: Vec<u8>,
    fill: u32,
    pub width: usize,
    pub height: usize,
    pub p: Params,
    dc: [i32; 3],
    pmv: [[[i32; 2]; 2]; 2],
    scale_code: u8,
}

impl Writer {
    pub fn new(width: usize, height: usize) -> Self {
        Writer {
            out: Vec::new(),
            fill: 0,
            width,
            height,
            p: Params::new(1),
            dc: [0; 3],
            pmv: [[[0; 2]; 2]; 2],
            scale_code: 1,
        }
    }

    /// The bytes written since the last call.
    pub fn take(&mut self) -> Vec<u8> {
        self.fill = 0;
        std::mem::take(&mut self.out)
    }

    pub fn put(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            if self.fill == 0 {
                self.out.push(0);
            }
            let bit = ((v >> i) & 1) as u8;
            *self.out.last_mut().unwrap() |= bit << (7 - self.fill);
            self.fill = (self.fill + 1) % 8;
        }
    }

    pub fn code(&mut self, code: &str) {
        let (v, n) = code_bits(code);
        self.put(v, n);
    }

    fn value(&mut self, table: &[(&str, u8)], v: u8) {
        self.code(
            table
                .iter()
                .find(|e| e.1 == v)
                .expect("a code for the value")
                .0,
        );
    }

    pub fn start(&mut self, code: u8) {
        self.fill = 0;
        self.out.extend([0, 0, 1, code]);
    }

    /// A sequence header and its extension: 25 frames per second, 16:9, interlaced, 4:2:0.
    pub fn sequence(&mut self) {
        self.start(0xb3);
        self.put(self.width as u32, 12);
        self.put(self.height as u32, 12);
        self.put(3, 4);
        self.put(3, 4);
        self.put(10_000, 18);
        self.put(1, 1);
        self.put(112, 10);
        self.put(0, 3);
        self.start(0xb5);
        self.put(1, 4);
        self.put(0x48, 8);
        self.put(0, 1);
        self.put(1, 2);
        self.put(0, 4);
        self.put(0, 12);
        self.put(1, 1);
        self.put(0, 8);
        self.put(0, 1);
        self.put(0, 7);
    }

    /// A picture header and its coding extension.
    pub fn picture(&mut self, p: Params, temporal_reference: u32) {
        self.p = p;
        self.start(0x00);
        self.put(temporal_reference, 10);
        self.put(p.kind as u32, 3);
        self.put(0xffff, 16);
        if p.kind >= 2 {
            self.put(7, 4);
        }
        if p.kind == 3 {
            self.put(7, 4);
        }
        self.put(0, 1);
        self.start(0xb5);
        self.put(8, 4);
        for f in p.f_code.iter().flatten() {
            self.put(*f as u32, 4);
        }
        self.put(p.dc_precision as u32, 2);
        self.put(p.structure as u32, 2);
        self.put(p.top_field_first as u32, 1);
        self.put(p.frame_pred_frame_dct as u32, 1);
        self.put(0, 1);
        self.put(p.non_linear_scale as u32, 1);
        self.put(p.intra_vlc_one as u32, 1);
        self.put(p.alternate_scan as u32, 1);
        self.put(0, 1);
        self.put(1, 1);
        self.put(0, 1);
        self.put(0, 1);
    }

    pub fn slice(&mut self, row: usize, scale_code: u8) {
        self.start(row as u8 + 1);
        self.put(scale_code as u32, 5);
        self.put(0, 1);
        self.scale_code = scale_code;
        self.dc = [1 << (7 + self.p.dc_precision); 3];
        self.pmv = [[[0; 2]; 2]; 2];
    }

    fn scale(&self) -> i32 {
        quantiser_scale(self.p.non_linear_scale, self.scale_code)
    }

    pub fn increment(&mut self, mut n: usize) {
        while n > 33 {
            self.value(ADDRESS_INCREMENT, ADDRESS_ESCAPE);
            n -= 33;
        }
        self.value(ADDRESS_INCREMENT, n as u8);
    }

    pub fn mb_type(&mut self, flags: u8) {
        let t = [TYPE_I, TYPE_P, TYPE_B][self.p.kind as usize - 1];
        self.value(t, flags);
    }

    /// One block of levels `q` (along the rows).
    pub fn block(&mut self, q: &[i32; 64], intra: bool, colour: usize) {
        let scan = if self.p.alternate_scan {
            ALTERNATE
        } else {
            ZIGZAG
        };
        let one = intra && self.p.intra_vlc_one;
        let table = if one {
            coeff_one()
        } else {
            COEFF_ZERO.to_vec()
        };
        let mut first = !intra;
        if intra {
            let diff = q[0] - self.dc[colour];
            self.dc[colour] = q[0];
            let size = 32 - diff.unsigned_abs().leading_zeros();
            self.value(
                if colour == 0 {
                    DC_SIZE_LUMA
                } else {
                    DC_SIZE_CHROMA
                },
                size as u8,
            );
            if size > 0 {
                self.put(
                    if diff < 0 {
                        diff + (1 << size) - 1
                    } else {
                        diff
                    } as u32,
                    size,
                );
            }
        }
        let mut run = 0;
        for &place in &scan[intra as usize..] {
            let level = q[place as usize];
            if level == 0 {
                run += 1;
                continue;
            }
            let found = table
                .iter()
                .find(|e| e.1 == run && e.2 as i32 == level.abs());
            if first && run == 0 && level.abs() == 1 {
                self.put(2 + (level < 0) as u32, 2);
            } else if let Some(e) = found {
                self.code(e.0);
                self.put((level < 0) as u32, 1);
            } else {
                self.code(COEFF_ESCAPE);
                self.put(run as u32, 6);
                self.put(level as u32 & 0xfff, 12);
            }
            first = false;
            run = 0;
        }
        self.code(if one { COEFF_END_ONE } else { COEFF_END_ZERO });
    }

    /// One motion vector, by the predictor and the f_code. `scaled` is for a vector into a field of a frame picture.
    pub fn vector(
        &mut self,
        r: usize,
        s: usize,
        v: [i32; 2],
        scaled: bool,
        diff: Option<[i32; 2]>,
    ) {
        for t in 0..2 {
            let size = self.p.f_code[s][t] as u32 - 1;
            let f = 1 << size;
            let halved = scaled && t == 1;
            let predictor = if halved {
                self.pmv[r][s][t] >> 1
            } else {
                self.pmv[r][s][t]
            };
            let mut d = v[t] - predictor;
            if d < -16 * f {
                d += 32 * f;
            } else if d > 16 * f - 1 {
                d -= 32 * f;
            }
            if d == 0 {
                self.value(MOTION_CODE, 0);
            } else {
                let a = d.abs() - 1;
                self.value(MOTION_CODE, (a / f + 1) as u8);
                self.put((d < 0) as u32, 1);
                self.put((a % f) as u32, size);
            }
            if let Some(diff) = diff {
                match diff[t] {
                    0 => self.put(0, 1),
                    1 => self.put(2, 2),
                    _ => self.put(3, 2),
                }
            }
            self.pmv[r][s][t] = if halved { 2 * v[t] } else { v[t] };
        }
    }

    fn view(&self) -> (usize, usize, usize) {
        match self.p.structure {
            3 => (0, 1, self.height / 16),
            s => (s as usize - 1, 2, self.height / 32),
        }
    }

    /// An intra picture (or field) of the samples of `src`, one slice per row of macroblocks. `field_dct` says which
    /// macroblocks of a frame picture use the field DCT. It writes the samples that a decoder must give, within 1,
    /// to the same rows of `want`.
    pub fn intra_picture(
        &mut self,
        src: &Planes,
        want: &mut Planes,
        scale_code: u8,
        field_dct: &dyn Fn(usize, usize) -> bool,
    ) {
        let (first, pitch, rows) = self.view();
        for my in 0..rows {
            self.slice(my, scale_code);
            for mx in 0..self.width / 16 {
                self.increment(1);
                self.mb_type(MB_INTRA);
                let field =
                    self.p.structure == 3 && !self.p.frame_pred_frame_dct && field_dct(mx, my);
                if self.p.structure == 3 && !self.p.frame_pred_frame_dct {
                    self.put(field as u32, 1);
                }
                for b in 0..6 {
                    let (c, at, stride) =
                        block_place(b, (mx, my), self.width, (first, pitch), field);
                    let plane = src.plane(c);
                    let x: [f64; 64] =
                        std::array::from_fn(|i| plane[at + (i / 8) * stride + i % 8] as f64);
                    let f = dct(&x, true);
                    let scale = self.scale();
                    let limit = (1 << (8 + self.p.dc_precision)) - 1;
                    let q: [i32; 64] = std::array::from_fn(|i| {
                        if i == 0 {
                            ((f[0] / (8 >> self.p.dc_precision) as f64).round() as i32)
                                .clamp(0, limit)
                        } else {
                            (f[i] * 16.0 / (DEFAULT_INTRA_MATRIX[i] as i32 * scale) as f64).round()
                                as i32
                        }
                    });
                    self.block(&q, true, c);
                    let back = dct(
                        &dequantise(&q, true, &DEFAULT_INTRA_MATRIX, scale, self.p.dc_precision),
                        false,
                    );
                    let plane = want.plane_mut(c);
                    for i in 0..64 {
                        plane[at + (i / 8) * stride + i % 8] =
                            back[i].round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    /// A P or B picture (or field) of the macroblocks `mbs`, one slice per row.
    pub fn inter_picture(&mut self, mbs: &[Mb], scale_code: u8) {
        let (_, _, rows) = self.view();
        let mb_w = self.width / 16;
        let frame = self.p.structure == 3;
        for my in 0..rows {
            self.slice(my, scale_code);
            let mut step = 1;
            for mx in 0..mb_w {
                let Mb::Inter(m) = mbs[my * mb_w + mx] else {
                    step += 1;
                    if self.p.kind == 2 {
                        self.pmv = [[[0; 2]; 2]; 2];
                    }
                    continue;
                };
                self.increment(step);
                step = 1;
                let mut flags = if m.residual.is_some() { MB_PATTERN } else { 0 };
                if m.vectors[0].is_some() {
                    flags |= MB_FORWARD;
                }
                if m.vectors[1].is_some() {
                    flags |= MB_BACKWARD;
                }
                self.mb_type(flags);
                if flags & (MB_FORWARD | MB_BACKWARD) != 0
                    && !(frame && self.p.frame_pred_frame_dct)
                {
                    self.put(
                        match (frame, m.kind) {
                            (true, Kind::Field) | (false, Kind::Field) => 1,
                            (true, Kind::Frame) | (false, Kind::Halves) => 2,
                            _ => 3,
                        },
                        2,
                    );
                }
                if frame && !self.p.frame_pred_frame_dct && m.residual.is_some() {
                    self.put(0, 1);
                }
                let count = match (frame, m.kind) {
                    (true, Kind::Field) | (false, Kind::Halves) => 2,
                    _ => 1,
                };
                let dual = m.kind == Kind::DualPrime;
                for s in 0..2 {
                    let Some(v) = m.vectors[s] else { continue };
                    for r in 0..count {
                        if m.kind != Kind::Frame && !dual {
                            self.put(m.field[r][s] as u32, 1);
                        }
                        self.vector(
                            r,
                            s,
                            v[r],
                            frame && m.kind != Kind::Frame,
                            dual.then_some(m.diff),
                        );
                    }
                    if count == 1 {
                        self.pmv[1][s] = self.pmv[0][s];
                    }
                }
                if self.p.kind == 2 && m.vectors[0].is_none() {
                    self.pmv = [[[0; 2]; 2]; 2];
                }
                if let Some(level) = m.residual {
                    self.value(BLOCK_PATTERN, 32);
                    let mut q = [0; 64];
                    q[0] = level;
                    self.block(&q, false, 0);
                }
            }
        }
    }
}

/// One prediction block: `w` by `h` samples at (`x`, `y`) plus the vector `v` (half samples) of the rows of `plane`
/// that start at row `first` and are `pitch` rows apart.
pub fn fetch(
    plane: &[u8],
    width: usize,
    (first, pitch): (usize, usize),
    (x, y, w, h): (usize, usize, usize, usize),
    v: [i32; 2],
) -> Vec<u8> {
    let at = |px: i32, py: i32| plane[(first + pitch * py as usize) * width + px as usize] as u32;
    let mut out = Vec::new();
    for j in 0..h as i32 {
        for i in 0..w as i32 {
            let (px, py) = (
                x as i32 + i + v[0].div_euclid(2),
                y as i32 + j + v[1].div_euclid(2),
            );
            let (hx, hy) = (v[0].rem_euclid(2), v[1].rem_euclid(2));
            out.push(match (hx, hy) {
                (0, 0) => at(px, py),
                (1, 0) => (at(px, py) + at(px + 1, py) + 1) / 2,
                (0, 1) => (at(px, py) + at(px, py + 1) + 1) / 2,
                _ => (at(px, py) + at(px + 1, py) + at(px, py + 1) + at(px + 1, py + 1) + 2) / 4,
            } as u8);
        }
    }
    out
}

/// The three planes of one prediction of 16 by `h` luminance samples: see [`fetch`]. The chroma vector is half the
/// luminance vector, towards zero.
pub fn fetch_all(
    from: &Planes,
    view: (usize, usize),
    (x, y, h): (usize, usize, usize),
    v: [i32; 2],
) -> [Vec<u8>; 3] {
    let vc = [v[0] / 2, v[1] / 2];
    [
        fetch(&from.y, from.width, view, (x, y, 16, h), v),
        fetch(&from.cb, from.width / 2, view, (x / 2, y / 2, 8, h / 2), vc),
        fetch(&from.cr, from.width / 2, view, (x / 2, y / 2, 8, h / 2), vc),
    ]
}

/// The average of two predictions, rounded up.
pub fn average(a: &[Vec<u8>; 3], b: &[Vec<u8>; 3]) -> [Vec<u8>; 3] {
    std::array::from_fn(|c| {
        a[c].iter()
            .zip(&b[c])
            .map(|(a, b)| ((*a as u32 + *b as u32 + 1) / 2) as u8)
            .collect()
    })
}

/// Writes a prediction of 16 by `h` luminance samples to `to`: its first row is row `y` of the rows of the frame
/// that start at `first` and are `pitch` apart.
pub fn place(
    to: &mut Planes,
    (first, pitch): (usize, usize),
    (x, y, h): (usize, usize, usize),
    p: &[Vec<u8>; 3],
) {
    for c in 0..3 {
        let (w, x, y, h, bw) = if c == 0 {
            (to.width, x, y, h, 16)
        } else {
            (to.width / 2, x / 2, y / 2, h / 2, 8)
        };
        let plane = to.plane_mut(c);
        for j in 0..h {
            let at = (first + pitch * (y + j)) * w + x;
            plane[at..at + bw].copy_from_slice(&p[c][j * bw..(j + 1) * bw]);
        }
    }
}

/// The largest difference between the samples of two frames.
pub fn distance(a: &Planes, b: &Planes) -> u8 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    (0..3)
        .flat_map(|c| a.plane(c).iter().zip(b.plane(c)))
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0)
}
