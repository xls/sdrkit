//! Motion compensation: one prediction block from samples at whole or half sample places.

/// What a prediction block is formed from. `src` starts at the sample of the whole part of the vector; its rows are
/// `stride` apart, and it holds `(h - 1 + half_y) * stride + w + half_x` samples at least.
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    pub src: &'a [u8],
    pub stride: usize,
    /// The vector has a half sample part to the right.
    pub half_x: bool,
    /// The vector has a half sample part downwards.
    pub half_y: bool,
}

#[inline(always)]
fn rows<const W: usize>(
    s: &Source,
    dst: &mut [u8],
    dstride: usize,
    h: usize,
    average: bool,
    f: impl Fn(&[u8; W], &[u8; W], &[u8; W], &[u8; W], &mut [u8; W]),
) {
    let (dx, dy) = (s.half_x as usize, s.half_y as usize * s.stride);
    for y in 0..h {
        let at = y * s.stride;
        let a: &[u8; W] = s.src[at..at + W].try_into().expect("W samples");
        let b: &[u8; W] = s.src[at + dx..at + dx + W].try_into().expect("W samples");
        let c: &[u8; W] = s.src[at + dy..at + dy + W].try_into().expect("W samples");
        let d: &[u8; W] = s.src[at + dy + dx..at + dy + dx + W]
            .try_into()
            .expect("W samples");
        let o: &mut [u8; W] = (&mut dst[y * dstride..y * dstride + W])
            .try_into()
            .expect("W samples");
        if average {
            let mut p = [0u8; W];
            f(a, b, c, d, &mut p);
            for (o, p) in o.iter_mut().zip(p) {
                *o = ((*o as u16 + p as u16 + 1) >> 1) as u8;
            }
        } else {
            f(a, b, c, d, o);
        }
    }
}

#[inline(always)]
fn block<const W: usize>(s: &Source, dst: &mut [u8], dstride: usize, h: usize, average: bool) {
    match (s.half_x, s.half_y) {
        (false, false) => rows::<W>(s, dst, dstride, h, average, |a, _, _, _, o| *o = *a),
        (true, false) => rows::<W>(s, dst, dstride, h, average, |a, b, _, _, o| {
            for i in 0..W {
                o[i] = ((a[i] as u16 + b[i] as u16 + 1) >> 1) as u8;
            }
        }),
        (false, true) => rows::<W>(s, dst, dstride, h, average, |a, _, c, _, o| {
            for i in 0..W {
                o[i] = ((a[i] as u16 + c[i] as u16 + 1) >> 1) as u8;
            }
        }),
        (true, true) => rows::<W>(s, dst, dstride, h, average, |a, b, c, d, o| {
            for i in 0..W {
                o[i] = ((a[i] as u16 + b[i] as u16 + c[i] as u16 + d[i] as u16 + 2) >> 2) as u8;
            }
        }),
    }
}

/// Forms a prediction block of `w` (16 or 8) by `h` samples in `dst`, whose rows are `dstride` apart. With `average`
/// the block is averaged with what `dst` holds, rounding up.
pub(crate) fn predict(
    s: &Source,
    dst: &mut [u8],
    dstride: usize,
    w: usize,
    h: usize,
    average: bool,
) {
    if w == 16 {
        block::<16>(s, dst, dstride, h, average);
    } else {
        block::<8>(s, dst, dstride, h, average);
    }
}
