//! The inverse DCT of 8 by 8 blocks in integer arithmetic. It meets the accuracy limits of IEEE Std 1180-1990.
//!
//! The columns are transformed first and keep 4 fraction bits, then the rows. All sums wrap, so that the form for
//! 64-bit ARM gives the same bits for every input.

/// cos(k * pi / 16) * 2^13 for k = 1 to 7.
pub(crate) const C1: i32 = 8035;
pub(crate) const C2: i32 = 7568;
pub(crate) const C3: i32 = 6811;
pub(crate) const C4: i32 = 5793;
pub(crate) const C5: i32 = 4551;
pub(crate) const C6: i32 = 3135;
pub(crate) const C7: i32 = 1598;

/// The rounding term and the shift after the columns.
pub(crate) const ROUND_1: i32 = 1 << 9;
pub(crate) const SHIFT_1: u32 = 10;
/// The rounding term and the shift after the rows.
pub(crate) const ROUND_2: i32 = 1 << 17;
pub(crate) const SHIFT_2: u32 = 18;

#[inline(always)]
fn m(a: i32, c: i32) -> i32 {
    a.wrapping_mul(c)
}

/// One transform of 8 values: the outputs before the shift.
#[inline(always)]
fn pass(x: [i32; 8]) -> [i32; 8] {
    let e0 = m(x[0].wrapping_add(x[4]), C4);
    let e1 = m(x[0].wrapping_sub(x[4]), C4);
    let e2 = m(x[2], C2).wrapping_add(m(x[6], C6));
    let e3 = m(x[2], C6).wrapping_sub(m(x[6], C2));
    let a0 = e0.wrapping_add(e2);
    let a3 = e0.wrapping_sub(e2);
    let a1 = e1.wrapping_add(e3);
    let a2 = e1.wrapping_sub(e3);
    let b0 = m(x[1], C1)
        .wrapping_add(m(x[3], C3))
        .wrapping_add(m(x[5], C5))
        .wrapping_add(m(x[7], C7));
    let b1 = m(x[1], C3)
        .wrapping_sub(m(x[3], C7))
        .wrapping_sub(m(x[5], C1))
        .wrapping_sub(m(x[7], C5));
    let b2 = m(x[1], C5)
        .wrapping_sub(m(x[3], C1))
        .wrapping_add(m(x[5], C7))
        .wrapping_add(m(x[7], C3));
    let b3 = m(x[1], C7)
        .wrapping_sub(m(x[3], C5))
        .wrapping_add(m(x[5], C3))
        .wrapping_sub(m(x[7], C1));
    [
        a0.wrapping_add(b0),
        a1.wrapping_add(b1),
        a2.wrapping_add(b2),
        a3.wrapping_add(b3),
        a3.wrapping_sub(b3),
        a2.wrapping_sub(b2),
        a1.wrapping_sub(b1),
        a0.wrapping_sub(b0),
    ]
}

/// The value of every sample of a block that has only the coefficient `dc` at place 0, before the limits.
#[inline]
pub(crate) fn flat(dc: i16) -> i32 {
    let a = m(dc as i32, C4).wrapping_add(ROUND_1) >> SHIFT_1;
    m(a, C4).wrapping_add(ROUND_2) >> SHIFT_2
}

/// The inverse transform of `block` (coefficients along the rows), limited to -256..=255.
pub fn inverse(block: &[i16; 64]) -> [i16; 64] {
    let mut t = [0i32; 64];
    for u in 0..8 {
        let col = pass(std::array::from_fn(|v| block[8 * v + u] as i32));
        for v in 0..8 {
            t[8 * v + u] = col[v].wrapping_add(ROUND_1) >> SHIFT_1;
        }
    }
    let mut out = [0i16; 64];
    for (o, r) in out
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .zip(t.as_chunks::<8>().0)
    {
        let row = pass(*r);
        for x in 0..8 {
            o[x] = (row[x].wrapping_add(ROUND_2) >> SHIFT_2).clamp(-256, 255) as i16;
        }
    }
    out
}

/// Writes the inverse transform of `block` to the 8 by 8 samples of `dst` whose rows are `stride` apart.
/// `dst` holds at least `7 * stride + 8` samples.
pub(crate) fn put(block: &[i16; 64], dst: &mut [u8], stride: usize) {
    let r = inverse(block);
    for (y, row) in r.as_chunks::<8>().0.iter().enumerate() {
        for (d, v) in dst[y * stride..y * stride + 8].iter_mut().zip(row) {
            *d = (*v).clamp(0, 255) as u8;
        }
    }
}

/// Adds the inverse transform of `block` to the 8 by 8 samples of `dst` whose rows are `stride` apart.
pub(crate) fn add(block: &[i16; 64], dst: &mut [u8], stride: usize) {
    let r = inverse(block);
    for (y, row) in r.as_chunks::<8>().0.iter().enumerate() {
        for (d, v) in dst[y * stride..y * stride + 8].iter_mut().zip(row) {
            *d = (*d as i16 + *v).clamp(0, 255) as u8;
        }
    }
}

/// [`put`] for a block that has only the coefficient at place 0.
pub(crate) fn put_flat(dc: i16, dst: &mut [u8], stride: usize) {
    let v = flat(dc).clamp(0, 255) as u8;
    for y in 0..8 {
        dst[y * stride..y * stride + 8].fill(v);
    }
}

/// [`add`] for a block that has only the coefficient at place 0.
pub(crate) fn add_flat(dc: i16, dst: &mut [u8], stride: usize) {
    let v = flat(dc).clamp(-256, 255) as i16;
    for y in 0..8 {
        for d in &mut dst[y * stride..y * stride + 8] {
            *d = (*d as i16 + v).clamp(0, 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_block_equals_the_full_transform() {
        for dc in (-2048..=2047).step_by(7) {
            let mut b = [0i16; 64];
            b[0] = dc;
            let full = inverse(&b);
            let f = flat(dc).clamp(-256, 255) as i16;
            assert!(full.iter().all(|v| *v == f), "dc {dc}");
        }
    }
}
