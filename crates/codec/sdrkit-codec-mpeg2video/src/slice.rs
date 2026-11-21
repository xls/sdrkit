//! The slice: macroblocks, motion vectors, prediction, coefficients and their inverse quantisation.

use crate::decoder::{Decoder, Planes};
use crate::header::{Picture, Sequence, BOTTOM_FIELD, FRAME};
use crate::mc::Source;
use crate::tables::{
    self, quantiser_scale, Tables, ADDRESS_ESCAPE, ALTERNATE, END, ESCAPE, MB_BACKWARD, MB_FORWARD,
    MB_INTRA, MB_PATTERN, MB_QUANT, ZIGZAG,
};
use crate::{idct, kernel};
use sdrkit_util_bits::BitReader;

/// A slice ends here: the data is no valid slice from this place on.
struct Damaged;

type Step<T = ()> = Result<T, Damaged>;

/// How a macroblock is predicted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prediction {
    /// One vector for the frame.
    Frame,
    /// One vector per field of a frame picture, or one vector for a field picture.
    Field,
    /// One vector for the upper and one for the lower half of a macroblock of a field picture.
    Halves,
    /// One vector and a small difference for the field of the other parity.
    DualPrime,
}

/// The prediction of one macroblock.
#[derive(Clone, Copy)]
struct Motion {
    forward: bool,
    backward: bool,
    kind: Prediction,
    /// The vectors in half samples: first or second, forward or backward, horizontal or vertical.
    vector: [[[i32; 2]; 2]; 2],
    /// The field (0 top, 1 bottom) that each vector points into.
    field: [[usize; 2]; 2],
    /// Dual prime: the vector into the field of the other parity, for the top and the bottom field of a frame
    /// picture, or the first one for a field picture.
    other: [[i32; 2]; 2],
}

struct Slice<'a> {
    seq: &'a Sequence,
    pic: Picture,
    t: &'static Tables,
    frames: &'a mut [Planes; 3],
    cur: usize,
    fwd: Option<usize>,
    bwd: Option<usize>,
    plain: bool,
    r: BitReader<'a>,
    /// The picture is a frame picture.
    frame: bool,
    /// The field of a field picture: 0 top, 1 bottom.
    parity: usize,
    scale: i32,
    dc: [i32; 3],
    /// The vector predictors: first or second, forward or backward, horizontal or vertical.
    pmv: [[[i32; 2]; 2]; 2],
    /// The directions of the macroblock before, for a skipped macroblock of a B picture.
    last: (bool, bool),
    luma: [u8; 256],
    chroma: [[u8; 64]; 2],
    block: [i16; 64],
}

impl Decoder {
    /// Decodes the slice with start code `code` into the open frame.
    pub(crate) fn slice(&mut self, code: u8, data: &[u8]) {
        let (Some(seq), Some(pic)) = (self.seq.as_ref(), self.pic) else {
            return;
        };
        let frame = pic.structure == FRAME;
        let mb_w = seq.mb_w;
        let rows = if frame { seq.mb_h } else { seq.mb_h / 2 };
        let row = code as usize - 1;
        self.stats.slices += 1;
        if row >= rows {
            self.stats.slice_errors += 1;
            self.frames[self.cur].mark_damaged();
            return;
        }
        let mut s = Slice {
            seq,
            pic,
            t: tables::tables(),
            frames: &mut self.frames,
            cur: self.cur,
            fwd: self.fwd,
            bwd: self.bwd,
            plain: self.plain,
            r: BitReader::new(data),
            frame,
            parity: (pic.structure == BOTTOM_FIELD) as usize,
            scale: 0,
            dc: [0; 3],
            pmv: [[[0; 2]; 2]; 2],
            last: (false, false),
            luma: [0; 256],
            chroma: [[0; 64]; 2],
            block: [0; 64],
        };
        let mark = if frame { 3 } else { 1 << s.parity };
        let done = &mut self.done[row * mb_w..(row + 1) * mb_w];
        if s.run(row, &mut |x| done[x] |= mark).is_err() {
            self.stats.slice_errors += 1;
            self.frames[self.cur].mark_damaged();
        }
    }
}

/// v / 2, rounded to the nearest whole number and away from zero at one half.
fn half(v: i32) -> i32 {
    (v + (v > 0) as i32) >> 1
}

impl Slice<'_> {
    fn run(&mut self, row: usize, done: &mut dyn FnMut(usize)) -> Step {
        let mb_w = self.seq.mb_w;
        self.quantiser()?;
        if self.r.bit() {
            self.r.skip(8);
            while self.r.bit() {
                self.r.skip(8);
                if self.r.overrun() {
                    return Err(Damaged);
                }
            }
        }
        self.reset_dc();
        let mut x = self.increment()? - 1;
        loop {
            if x >= mb_w {
                return Err(Damaged);
            }
            self.macroblock(x, row)?;
            if self.r.overrun() {
                return Err(Damaged);
            }
            done(x);
            if self.r.peek(23) == 0 {
                return Ok(());
            }
            let step = self.increment()?;
            for k in 1..step {
                if x + k >= mb_w {
                    return Err(Damaged);
                }
                self.skipped(x + k, row)?;
                done(x + k);
            }
            x += step;
        }
    }

    fn reset_dc(&mut self) {
        self.dc = [1 << (7 + self.pic.dc_precision); 3];
    }

    fn quantiser(&mut self) -> Step {
        let code = self.r.u(5) as u8;
        self.scale = quantiser_scale(self.pic.non_linear_scale, code);
        if code == 0 {
            return Err(Damaged);
        }
        Ok(())
    }

    /// The next code of `t`: its value.
    #[inline]
    fn code(&mut self, t: &tables::Vlc) -> Step<u8> {
        let e = t.get(self.r.peek(16) as u32);
        if e.len == 0 {
            return Err(Damaged);
        }
        self.r.skip(e.len as usize);
        Ok(e.a)
    }

    fn increment(&mut self) -> Step<usize> {
        let t = self.t;
        let mut n = 0;
        loop {
            let v = self.code(&t.increment)?;
            if v != ADDRESS_ESCAPE {
                return Ok(n + v as usize);
            }
            n += 33;
            if n > self.seq.mb_w + 33 {
                return Err(Damaged);
            }
        }
    }

    /// A macroblock that the slice passes over: a copy in a P picture, the prediction of the macroblock before in a
    /// B picture.
    fn skipped(&mut self, x: usize, row: usize) -> Step {
        self.reset_dc();
        let mut m = Motion {
            forward: true,
            backward: false,
            kind: if self.frame {
                Prediction::Frame
            } else {
                Prediction::Field
            },
            vector: [[[0; 2]; 2]; 2],
            field: [[self.parity; 2]; 2],
            other: [[0; 2]; 2],
        };
        match self.pic.kind {
            2 => self.pmv = [[[0; 2]; 2]; 2],
            3 => {
                (m.forward, m.backward) = self.last;
                m.vector[0] = self.pmv[0];
                if !m.forward && !m.backward {
                    return Err(Damaged);
                }
            }
            _ => return Err(Damaged),
        }
        self.predict(&m, x, row)?;
        self.store(x, row);
        Ok(())
    }

    fn macroblock(&mut self, x: usize, row: usize) -> Step {
        let t = self.t;
        let flags = self.code(&t.mb_type[self.pic.kind as usize - 1])?;
        let pic = self.pic;
        let mut motion_type = if self.frame { 2 } else { 1 };
        if flags & (MB_FORWARD | MB_BACKWARD) != 0 && !(self.frame && pic.frame_pred_frame_dct) {
            motion_type = self.r.u(2);
        }
        let field_dct = self.frame
            && !pic.frame_pred_frame_dct
            && flags & (MB_INTRA | MB_PATTERN) != 0
            && self.r.bit();
        if flags & MB_QUANT != 0 {
            self.quantiser()?;
        }
        if flags & MB_INTRA != 0 {
            if pic.concealment_vectors {
                if !self.frame {
                    self.r.skip(1);
                }
                self.vector(0, 0, false, None)?;
                self.pmv[1][0] = self.pmv[0][0];
                self.r.skip(1);
            } else {
                self.pmv = [[[0; 2]; 2]; 2];
            }
            self.last = (false, false);
            for b in 0..6 {
                let flat = self.coefficients(true, usize::saturating_sub(b, 3))?;
                self.inverse(b, field_dct, flat, true);
            }
            self.store(x, row);
            return Ok(());
        }
        self.reset_dc();
        let (kind, count, dual) = match (self.frame, motion_type) {
            (true, 1) => (Prediction::Field, 2, false),
            (true, 2) => (Prediction::Frame, 1, false),
            (false, 1) => (Prediction::Field, 1, false),
            (false, 2) => (Prediction::Halves, 2, false),
            (_, 3) if pic.kind == 2 => (Prediction::DualPrime, 1, true),
            _ => return Err(Damaged),
        };
        let mut m = Motion {
            forward: flags & MB_FORWARD != 0,
            backward: flags & MB_BACKWARD != 0,
            kind,
            vector: [[[0; 2]; 2]; 2],
            field: [[self.parity; 2]; 2],
            other: [[0; 2]; 2],
        };
        // A vector for a field of a frame picture counts in rows of the field; its predictor counts in rows of the
        // frame.
        let scaled = self.frame && kind != Prediction::Frame;
        for s in 0..2 {
            if (s == 0 && !m.forward) || (s == 1 && !m.backward) {
                continue;
            }
            let mut diff = [0i32; 2];
            for r in 0..count {
                if kind != Prediction::Frame && !dual {
                    m.field[r][s] = self.r.bit() as usize;
                }
                m.vector[r][s] = self.vector(r, s, scaled, dual.then_some(&mut diff))?;
            }
            if count == 1 {
                self.pmv[1][s] = self.pmv[0][s];
            }
            if dual {
                let [vx, vy] = m.vector[0][0];
                if self.frame {
                    let near = if pic.top_field_first { [1, 3] } else { [3, 1] };
                    for (p, k) in near.into_iter().enumerate() {
                        m.other[p] = [
                            half(k * vx) + diff[0],
                            half(k * vy) + diff[1] + 2 * p as i32 - 1,
                        ];
                    }
                } else {
                    m.other[0] = [
                        half(vx) + diff[0],
                        half(vy) + diff[1] + 2 * self.parity as i32 - 1,
                    ];
                }
            }
        }
        if pic.kind == 2 && !m.forward {
            m.forward = true;
            self.pmv = [[[0; 2]; 2]; 2];
        }
        self.last = (m.forward, m.backward);
        self.predict(&m, x, row)?;
        if flags & MB_PATTERN != 0 {
            let pattern = self.code(&t.pattern)?;
            for b in 0..6 {
                if pattern & (32 >> b) != 0 {
                    let flat = self.coefficients(false, 0)?;
                    self.inverse(b, field_dct, flat, false);
                }
            }
        }
        self.store(x, row);
        Ok(())
    }

    /// Reads one motion vector and updates its predictor. `scaled` is for a vector that counts in rows of a field of
    /// a frame picture. `diff` takes the two differences of dual prime.
    fn vector(
        &mut self,
        r: usize,
        s: usize,
        scaled: bool,
        mut diff: Option<&mut [i32; 2]>,
    ) -> Step<[i32; 2]> {
        let mut out = [0i32; 2];
        for t in 0..2 {
            let f_code = self.pic.f_code[s][t] as u32;
            if !(1..=9).contains(&f_code) {
                return Err(Damaged);
            }
            let size = f_code - 1;
            let magnitude = self.code(&tables::tables().motion)? as i32;
            let mut delta = 0;
            if magnitude != 0 {
                let negative = self.r.bit();
                delta = ((magnitude - 1) << size) + 1;
                if size > 0 {
                    delta += self.r.u(size) as i32;
                }
                if negative {
                    delta = -delta;
                }
            }
            if let Some(d) = diff.as_deref_mut() {
                d[t] = if self.r.bit() {
                    1 - 2 * self.r.bit() as i32
                } else {
                    0
                };
            }
            let halved = scaled && t == 1;
            let predictor = if halved {
                self.pmv[r][s][t] >> 1
            } else {
                self.pmv[r][s][t]
            };
            let (low, range) = (-(16 << size), 32 << size);
            let mut v = predictor + delta;
            if v < low {
                v += range;
            } else if v >= -low {
                v -= range;
            }
            self.pmv[r][s][t] = if halved { 2 * v } else { v };
            out[t] = v;
        }
        Ok(out)
    }

    /// The frame store that a prediction in direction `s` from field `field` reads.
    fn store_of(&self, s: usize, field: Option<usize>) -> Step<usize> {
        // The second field of a P frame is predicted from the first field of the same frame.
        if !self.frame && self.pic.kind == 2 && self.pic.second && field == Some(1 - self.parity) {
            return Ok(self.cur);
        }
        if s == 0 { self.fwd } else { self.bwd }.ok_or(Damaged)
    }

    /// Forms one prediction block of the macroblock in `luma` and `chroma`: 16 samples wide and `h` rows high, whose
    /// top left sample is at (`x`, `y`) of the frame, or of the field `field`, plus the vector `v`. It goes to the
    /// rows of the macroblock from `to` on, `step` apart; `to_c` is the first row in the chroma blocks.
    #[allow(clippy::too_many_arguments)]
    fn part(
        &mut self,
        s: usize,
        field: Option<usize>,
        v: [i32; 2],
        (x, y, h): (usize, usize, usize),
        (to, to_c, step): (usize, usize, usize),
        average: bool,
    ) -> Step {
        let f = &self.frames[self.store_of(s, field)?];
        let width = self.seq.mb_w * 16;
        let (first, rows, pitch) = match field {
            None => (0, self.seq.mb_h * 16, 1),
            Some(p) => (p, self.seq.mb_h * 8, 2),
        };
        let place =
            |v: [i32; 2], x: usize, y: usize, w: usize, h: usize, width: usize, rows: usize| {
                let (px, py) = (x as i32 + (v[0] >> 1), y as i32 + (v[1] >> 1));
                let (hx, hy) = (v[0] & 1, v[1] & 1);
                let inside = px >= 0
                    && py >= 0
                    && px + w as i32 + hx <= width as i32
                    && py + h as i32 + hy <= rows as i32;
                inside.then(|| {
                    (
                        (first + pitch * py as usize) * width + px as usize,
                        hx != 0,
                        hy != 0,
                    )
                })
            };
        let (at, half_x, half_y) = place(v, x, y, 16, h, width, rows).ok_or(Damaged)?;
        let src = Source {
            src: &f.y[at..],
            stride: pitch * width,
            half_x,
            half_y,
        };
        kernel::predict(
            self.plain,
            &src,
            &mut self.luma[16 * to..],
            16 * step,
            16,
            h,
            average,
        );
        let vc = [v[0] / 2, v[1] / 2];
        let (at, half_x, half_y) =
            place(vc, x / 2, y / 2, 8, h / 2, width / 2, rows / 2).ok_or(Damaged)?;
        for (plane, out) in [&f.cb, &f.cr].into_iter().zip(&mut self.chroma) {
            let src = Source {
                src: &plane[at..],
                stride: pitch * width / 2,
                half_x,
                half_y,
            };
            kernel::predict(
                self.plain,
                &src,
                &mut out[8 * to_c..],
                8 * step,
                8,
                h / 2,
                average,
            );
        }
        Ok(())
    }

    /// Forms the prediction of the macroblock at (`x`, `row`) in `luma` and `chroma`.
    fn predict(&mut self, m: &Motion, x: usize, row: usize) -> Step {
        let mut average = false;
        let x = 16 * x;
        for s in 0..2 {
            if (s == 0 && !m.forward) || (s == 1 && !m.backward) {
                continue;
            }
            match (self.frame, m.kind) {
                (true, Prediction::Frame) => {
                    self.part(
                        s,
                        None,
                        m.vector[0][s],
                        (x, 16 * row, 16),
                        (0, 0, 1),
                        average,
                    )?;
                }
                (true, Prediction::Field) => {
                    for r in 0..2 {
                        self.part(
                            s,
                            Some(m.field[r][s]),
                            m.vector[r][s],
                            (x, 8 * row, 8),
                            (r, r, 2),
                            average,
                        )?;
                    }
                }
                (true, Prediction::DualPrime) => {
                    for p in 0..2 {
                        self.part(
                            s,
                            Some(p),
                            m.vector[0][s],
                            (x, 8 * row, 8),
                            (p, p, 2),
                            false,
                        )?;
                        self.part(s, Some(1 - p), m.other[p], (x, 8 * row, 8), (p, p, 2), true)?;
                    }
                }
                (false, Prediction::Field) => {
                    self.part(
                        s,
                        Some(m.field[0][s]),
                        m.vector[0][s],
                        (x, 16 * row, 16),
                        (0, 0, 1),
                        average,
                    )?;
                }
                (false, Prediction::Halves) => {
                    for r in 0..2 {
                        let from = (x, 16 * row + 8 * r, 8);
                        self.part(
                            s,
                            Some(m.field[r][s]),
                            m.vector[r][s],
                            from,
                            (8 * r, 4 * r, 1),
                            average,
                        )?;
                    }
                }
                (false, Prediction::DualPrime) => {
                    self.part(
                        s,
                        Some(self.parity),
                        m.vector[0][s],
                        (x, 16 * row, 16),
                        (0, 0, 1),
                        false,
                    )?;
                    self.part(
                        s,
                        Some(1 - self.parity),
                        m.other[0],
                        (x, 16 * row, 16),
                        (0, 0, 1),
                        true,
                    )?;
                }
                _ => return Err(Damaged),
            }
            average = true;
        }
        Ok(())
    }

    /// Reads the coefficients of one block into `block`, with the inverse quantisation. `colour` is 0 for luminance,
    /// 1 for Cb, 2 for Cr. True when the block has only the coefficient at place 0.
    fn coefficients(&mut self, intra: bool, colour: usize) -> Step<bool> {
        let t = self.t;
        let scan = if self.pic.alternate_scan {
            &ALTERNATE
        } else {
            &ZIGZAG
        };
        let (matrix, table) = if intra {
            (&self.seq.intra, &t.coeff[self.pic.intra_vlc_one as usize])
        } else {
            (&self.seq.non_intra, &t.coeff[0])
        };
        let scale = self.scale;
        let mut i = 0usize;
        let mut sum = 0i32;
        let mut flat = true;
        if intra {
            let size = self.code(if colour == 0 {
                &t.dc_luma
            } else {
                &t.dc_chroma
            })? as u32;
            if size != 0 {
                let v = self.r.u(size) as i32;
                self.dc[colour] += if v < 1 << (size - 1) {
                    v + 1 - (1 << size)
                } else {
                    v
                };
            }
            sum = (self.dc[colour] << (3 - self.pic.dc_precision)).clamp(-2048, 2047);
            self.block[0] = sum as i16;
            i = 1;
        } else if self.r.peek(1) == 1 {
            let negative = self.r.u(2) & 1 != 0;
            let v = (3 * matrix[0] as i32 * scale) >> 5;
            sum = if negative { -v.min(2048) } else { v.min(2047) };
            self.block[0] = sum as i16;
            i = 1;
        }
        loop {
            let w = self.r.peek(24) as u32;
            let e = table.get(w >> 8);
            if e.len == 0 {
                return Err(Damaged);
            }
            let (run, level);
            match e.b {
                END => {
                    self.r.skip(e.len as usize);
                    break;
                }
                ESCAPE => {
                    self.r.skip(e.len as usize);
                    let v = self.r.u(18) as u32;
                    run = (v >> 12) as usize;
                    level = ((v & 0xfff) as i32) << 20 >> 20;
                    if level & 2047 == 0 {
                        return Err(Damaged);
                    }
                }
                l => {
                    run = e.a as usize;
                    level = if (w >> (23 - e.len)) & 1 != 0 {
                        -(l as i32)
                    } else {
                        l as i32
                    };
                    self.r.skip(e.len as usize + 1);
                }
            }
            i += run;
            if i > 63 {
                return Err(Damaged);
            }
            let j = scan[i] as usize;
            let weight = matrix[j] as i32 * scale;
            let a = level.abs();
            let v = if intra {
                (a * weight) >> 4
            } else {
                ((2 * a + 1) * weight) >> 5
            };
            let v = if level < 0 { -v.min(2048) } else { v.min(2047) };
            self.block[j] = v as i16;
            sum += v;
            flat &= j == 0;
            i += 1;
        }
        // Mismatch control: the sum of the coefficients is odd.
        if sum & 1 == 0 {
            self.block[63] ^= 1;
            flat = false;
        }
        Ok(flat)
    }

    /// Transforms `block` into block `b` of the macroblock (0 to 3 luminance, 4 Cb, 5 Cr) and clears it. A block
    /// that is not intra is added to the prediction.
    fn inverse(&mut self, b: usize, field_dct: bool, flat: bool, intra: bool) {
        let (dst, stride): (&mut [u8], usize) = match (b, field_dct) {
            (4.., _) => (&mut self.chroma[b - 4][..], 8),
            (_, true) => (&mut self.luma[16 * (b >> 1) + 8 * (b & 1)..], 32),
            (_, false) => (&mut self.luma[128 * (b >> 1) + 8 * (b & 1)..], 16),
        };
        match (flat, intra) {
            (true, true) => idct::put_flat(self.block[0], dst, stride),
            (true, false) => idct::add_flat(self.block[0], dst, stride),
            (false, true) => kernel::put(self.plain, &self.block, dst, stride),
            (false, false) => kernel::add(self.plain, &self.block, dst, stride),
        }
        self.block = [0; 64];
    }

    /// Copies the macroblock in `luma` and `chroma` to its place in the open frame.
    fn store(&mut self, x: usize, row: usize) {
        let width = self.seq.mb_w * 16;
        let (first, pitch) = if self.frame { (0, 1) } else { (self.parity, 2) };
        let f = &mut self.frames[self.cur];
        for (k, line) in self.luma.as_chunks::<16>().0.iter().enumerate() {
            let at = (first + pitch * (16 * row + k)) * width + 16 * x;
            f.y[at..at + 16].copy_from_slice(line);
        }
        for (plane, block) in [&mut f.cb, &mut f.cr].into_iter().zip(&self.chroma) {
            for (k, line) in block.as_chunks::<8>().0.iter().enumerate() {
                let at = (first + pitch * (8 * row + k)) * (width / 2) + 8 * x;
                plane[at..at + 8].copy_from_slice(line);
            }
        }
    }
}
