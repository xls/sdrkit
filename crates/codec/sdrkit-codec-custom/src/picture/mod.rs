//! The wavelet picture codec: BT.601 colour with 4:2:0 chroma, the CDF 9/7 wavelet in lifting form, bitplane coding and a binary
//! range coder. The stream is embedded: every prefix decodes to a full-size picture.
//!
//! ```
//! use sdrkit_codec_custom::picture::{Decoder, Encoder};
//! let rgb: Vec<u8> = (0..16 * 16 * 3).map(|i| (i * 5 % 256) as u8).collect();
//! let mut stream = Vec::new();
//! let header = Encoder::new().encode(&rgb, 16, 16, 10_000, &mut stream).unwrap();
//! let mut out = Vec::new();
//! Decoder::new().decode(&header, &stream, &mut out).unwrap();
//! assert_eq!(out.len(), rgb.len());
//! ```

mod coder;
mod wavelet;

use crate::{Error, Param};
use coder::{Contexts, RangeDecoder, RangeEncoder};

/// The values that travel outside the stream, in the burst header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Header {
    /// Picture width in pixels, 1 to 8191.
    pub width: u32,
    /// Picture height in pixels, 1 to 8191.
    pub height: u32,
    /// Wavelet levels of the luma plane, 1 to 8.
    pub levels: u32,
    /// The highest coded bitplane, -1 to 14. -1 means that no decision is coded.
    pub top_plane: i32,
    /// The number of binary decisions that the encoder coded.
    pub decisions: u32,
}

/// The luma level count for a picture size: 5, lowered while the smaller side shifted by it is 0.
pub fn levels_for(width: u32, height: u32) -> u32 {
    let mut l = 5;
    while l > 1 && (width.min(height) >> l) == 0 {
        l -= 1;
    }
    l
}

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// One band of a plane.
#[derive(Clone, Copy, Debug)]
struct Band {
    plane: usize,
    orient: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    class: usize,
    parent: Option<usize>,
    map: usize,
}

/// The picture geometry: plane sizes, level counts and the bands in coding order.
struct Geometry {
    key: (usize, usize, u32),
    pw: [usize; 3],
    ph: [usize; 3],
    levels: [u32; 3],
    bands: Vec<Band>,
    map_len: usize,
}

fn dims(n: usize, l: u32) -> usize {
    let mut v = n;
    for _ in 0..l {
        v = v.div_ceil(2);
    }
    v
}

/// Keeps the geometry of the last picture and makes a new one only when the size or the levels change.
fn geometry(slot: &mut Option<Geometry>, w: usize, h: usize, l: u32) -> Geometry {
    match slot.take() {
        Some(g) if g.key == (w, h, l) => g,
        _ => Geometry::new(w, h, l),
    }
}

impl Geometry {
    fn new(w: usize, h: usize, l: u32) -> Self {
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        let pw = [w, cw, cw];
        let ph = [h, ch, ch];
        let levels = [l, l.saturating_sub(1), l.saturating_sub(1)];
        let mut bands: Vec<Band> = Vec::new();
        let mut map_len = 0;
        let mut add =
            |bands: &mut Vec<Band>, plane: usize, orient: usize, x, y, bw, bh, rank: u32| {
                bands.push(Band {
                    plane,
                    orient,
                    x,
                    y,
                    w: bw,
                    h: bh,
                    class: usize::from(rank >= 3),
                    parent: None,
                    map: map_len,
                });
                map_len += (bw + 2) * (bh + 2);
            };
        for c in 0..3 {
            let lv = levels[c];
            add(&mut bands, c, 0, 0, 0, dims(pw[c], lv), dims(ph[c], lv), l);
        }
        // Index of the detail band (plane, level, orientation) in `bands`.
        let mut index = [[[usize::MAX; 4]; 10]; 3];
        for r in (1..=l).rev() {
            for c in 0..3 {
                let lv = if c == 0 { r } else { r - 1 };
                if lv < 1 || lv > levels[c] {
                    continue;
                }
                let lw = dims(pw[c], lv);
                let lh = dims(ph[c], lv);
                let pww = dims(pw[c], lv - 1);
                let phh = dims(ph[c], lv - 1);
                let rank = if c == 0 { lv } else { lv + 1 };
                index[c][lv as usize][1] = bands.len();
                add(&mut bands, c, 1, lw, 0, pww - lw, lh, rank);
                index[c][lv as usize][2] = bands.len();
                add(&mut bands, c, 2, 0, lh, lw, phh - lh, rank);
                index[c][lv as usize][3] = bands.len();
                add(&mut bands, c, 3, lw, lh, pww - lw, phh - lh, rank);
            }
        }
        for c in 0..3 {
            for lv in 1..levels[c] {
                for o in 1..4 {
                    let i = index[c][lv as usize][o];
                    let p = index[c][lv as usize + 1][o];
                    if i != usize::MAX && p != usize::MAX {
                        bands[i].parent = Some(p);
                    }
                }
            }
        }
        Self {
            key: (w, h, l),
            pw,
            ph,
            levels,
            bands,
            map_len,
        }
    }
}

/// The coefficient state shared by the encoder and the decoder.
#[derive(Default)]
struct State {
    // Per plane, per coefficient: magnitude (true q at the encoder), the plane of significance and the lowest plane known.
    mag: [Vec<u16>; 3],
    sig: [Vec<i8>; 3],
    low: [Vec<i8>; 3],
    // Per plane, per coefficient at the encoder: true when the coefficient is negative.
    neg: [Vec<bool>; 3],
    // Significance maps of all bands with a border of one cell: 0, 1 (positive) or 2 (negative).
    map: Vec<u8>,
}

impl State {
    fn reset(&mut self, g: &Geometry) {
        for c in 0..3 {
            let n = g.pw[c] * g.ph[c];
            self.mag[c].clear();
            self.mag[c].resize(n, 0);
            self.sig[c].clear();
            self.sig[c].resize(n, -1);
            self.low[c].clear();
            self.low[c].resize(n, -1);
            self.neg[c].clear();
            self.neg[c].resize(n, false);
        }
        self.map.clear();
        self.map.resize(g.map_len, 0);
    }
}

/// The two sides of the plane loop.
trait Side {
    /// Codes one decision. `bit` is the value at the encoder. Returns `None` when the coder has stopped.
    fn code(&mut self, ctx: &mut coder::Prob, bit: bool) -> Option<bool>;
    /// True at the decoder: it writes the magnitudes.
    const DECODER: bool;
}

impl Side for RangeEncoder<'_> {
    fn code(&mut self, ctx: &mut coder::Prob, bit: bool) -> Option<bool> {
        self.encode(ctx, bit).then_some(bit)
    }
    const DECODER: bool = false;
}

impl Side for RangeDecoder<'_> {
    fn code(&mut self, ctx: &mut coder::Prob, _bit: bool) -> Option<bool> {
        self.decode(ctx)
    }
    const DECODER: bool = true;
}

#[inline]
fn neighbours(map: &[u8], base: usize, stride: usize, x: usize, y: usize) -> usize {
    // Cell (x, y) of the band is at base + (y + 1) * stride + x + 1.
    let c = base + (y + 1) * stride + x + 1;
    let cells = [
        c - stride - 1,
        c - stride,
        c - stride + 1,
        c - 1,
        c + 1,
        c + stride - 1,
        c + stride,
        c + stride + 1,
    ];
    cells.iter().filter(|&&i| map[i] != 0).count()
}

/// Runs the plane loop from `top` down to 0. Stops at the first decision the coder cannot make.
fn plane_loop<S: Side>(g: &Geometry, st: &mut State, ctx: &mut Contexts, top: i32, side: &mut S) {
    let mut p = top;
    while p >= 0 {
        if significance_pass(g, st, ctx, p as u32, side).is_none()
            || refinement_pass(g, st, ctx, p as u32, side).is_none()
        {
            return;
        }
        p -= 1;
    }
}

fn significance_pass<S: Side>(
    g: &Geometry,
    st: &mut State,
    ctx: &mut Contexts,
    p: u32,
    side: &mut S,
) -> Option<()> {
    let t = 1u16 << p;
    for b in &g.bands {
        if b.w == 0 || b.h == 0 {
            continue;
        }
        let stride = b.w + 2;
        let pw = g.pw[b.plane];
        let parent = b
            .parent
            .map(|i| g.bands[i])
            .filter(|pb| pb.w > 0 && pb.h > 0);
        for y in 0..b.h {
            let mut x0 = 0;
            while x0 < b.w {
                let len = 8.min(b.w - x0);
                let mut quiet = true;
                'q: for yy in 0..3 {
                    let row = b.map + (y + yy) * stride + x0;
                    for i in 0..len + 2 {
                        if st.map[row + i] != 0 {
                            quiet = false;
                            break 'q;
                        }
                    }
                }
                if quiet {
                    let mut pc = 0usize;
                    if let Some(pb) = parent {
                        let py = (y >> 1).min(pb.h - 1);
                        for px in (x0 >> 1)..=((x0 + len - 1) >> 1) {
                            let pxc = px.min(pb.w - 1);
                            if st.map[pb.map + (py + 1) * (pb.w + 2) + pxc + 1] != 0 {
                                pc += 1;
                            }
                        }
                    }
                    let mag = &st.mag[b.plane];
                    let row = (b.y + y) * pw + b.x;
                    let any = !S::DECODER && (x0..x0 + len).any(|x| mag[row + x] >= t);
                    let gb = side.code(&mut ctx.group[b.orient][b.class][pc.min(3)], any)?;
                    if !gb {
                        x0 += 8;
                        continue;
                    }
                }
                for x in x0..x0 + len {
                    let cell = b.map + (y + 1) * stride + x + 1;
                    if st.map[cell] != 0 {
                        continue;
                    }
                    let nb = neighbours(&st.map, b.map, stride, x, y);
                    let ps = parent.is_some_and(|pb| {
                        let px = (x >> 1).min(pb.w - 1);
                        let py = (y >> 1).min(pb.h - 1);
                        st.map[pb.map + (py + 1) * (pb.w + 2) + px + 1] != 0
                    });
                    let idx = (b.y + y) * pw + b.x + x;
                    let want = !S::DECODER && st.mag[b.plane][idx] >= t;
                    let sb = side.code(
                        &mut ctx.sig[b.orient][b.class][nb.min(5)][usize::from(ps)],
                        want,
                    )?;
                    if sb {
                        let sctx = 3 * st.map[cell - 1] as usize + st.map[cell - stride] as usize;
                        let neg = !S::DECODER && st.neg[b.plane][idx];
                        let s = side.code(&mut ctx.sign[sctx], neg)?;
                        st.map[cell] = 1 + u8::from(s);
                        st.sig[b.plane][idx] = p as i8;
                        st.low[b.plane][idx] = p as i8;
                        if S::DECODER {
                            st.mag[b.plane][idx] = t;
                        }
                    }
                }
                x0 += 8;
            }
        }
    }
    Some(())
}

fn refinement_pass<S: Side>(
    g: &Geometry,
    st: &mut State,
    ctx: &mut Contexts,
    p: u32,
    side: &mut S,
) -> Option<()> {
    for b in &g.bands {
        if b.w == 0 || b.h == 0 {
            continue;
        }
        let stride = b.w + 2;
        let pw = g.pw[b.plane];
        for y in 0..b.h {
            for x in 0..b.w {
                let idx = (b.y + y) * pw + b.x + x;
                let sp = st.sig[b.plane][idx];
                if sp < 0 || sp as u32 == p {
                    continue;
                }
                let c = if sp as u32 == p + 1 {
                    usize::from(neighbours(&st.map, b.map, stride, x, y) > 0)
                } else {
                    2
                };
                let want = !S::DECODER && (st.mag[b.plane][idx] >> p) & 1 == 1;
                let bit = side.code(&mut ctx.refine[c], want)?;
                if S::DECODER && bit {
                    st.mag[b.plane][idx] |= 1 << p;
                }
                st.low[b.plane][idx] = p as i8;
            }
        }
    }
    Some(())
}

fn check_size(w: u32, h: u32) -> Result<(), Error> {
    if !(1..=8191).contains(&w) || !(1..=8191).contains(&h) {
        return Err(Error::BadParameter {
            param: Param::PictureSize,
        });
    }
    Ok(())
}

/// The picture encoder. It keeps its buffers between pictures, so pictures of one size do not allocate after the first.
#[derive(Default)]
pub struct Encoder {
    geo: Option<Geometry>,
    planes: [Vec<f32>; 3],
    full_cb: Vec<f32>,
    full_cr: Vec<f32>,
    line: Vec<f32>,
    tmp: Vec<f32>,
    st: State,
}

impl Encoder {
    /// Makes an encoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Encodes an 8-bit RGB picture (row by row, R G B per pixel) with a budget in bytes. The stream goes to `out` (cleared
    /// first). Returns the header values.
    ///
    /// Errors: [`Error::BadParameter`] when the size is outside 1 to 8191, [`Error::BadInputLength`] when `rgb` is not
    /// width x height x 3 bytes.
    pub fn encode(
        &mut self,
        rgb: &[u8],
        width: u32,
        height: u32,
        budget: usize,
        out: &mut Vec<u8>,
    ) -> Result<Header, Error> {
        check_size(width, height)?;
        let (w, h) = (width as usize, height as usize);
        if rgb.len() != w * h * 3 {
            return Err(Error::BadInputLength { need: w * h * 3 });
        }
        let l = levels_for(width, height);
        let g = geometry(&mut self.geo, w, h, l);
        self.colour(rgb, w, h);
        for c in 0..3 {
            wavelet::forward_2d(
                &mut self.planes[c],
                g.pw[c],
                g.ph[c],
                g.levels[c],
                &mut self.line,
                &mut self.tmp,
            );
        }
        self.st.reset(&g);
        let mut maxq = 0u16;
        for c in 0..3 {
            for (i, &v) in self.planes[c].iter().enumerate() {
                let q = v.abs().min(30000.0) as u16;
                self.st.mag[c][i] = q;
                self.st.neg[c][i] = v < 0.0;
                maxq = maxq.max(q);
            }
        }
        let top = if maxq == 0 {
            -1
        } else {
            (15 - maxq.leading_zeros() as i32).min(14)
        };
        let mut ctx = Contexts::new();
        let mut enc = RangeEncoder::new(out, budget);
        plane_loop(&g, &mut self.st, &mut ctx, top, &mut enc);
        let decisions = enc.finish();
        self.geo = Some(g);
        Ok(Header {
            width,
            height,
            levels: l,
            top_plane: top,
            decisions,
        })
    }

    fn colour(&mut self, rgb: &[u8], w: usize, h: usize) {
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let n = w * h;
        self.planes[0].clear();
        self.planes[0].resize(n, 0.0);
        self.full_cb.clear();
        self.full_cb.resize(n, 0.0);
        self.full_cr.clear();
        self.full_cr.resize(n, 0.0);
        let (yr, yg, yb) = (f(0x3E99_1687), f(0x3F16_45A2), f(0x3DE9_78D5));
        let (br, bg, bb) = (f(0xBE2C_C921), f(0xBEA9_9B6F), f(0x3F00_0000));
        let (rr, rg, rb) = (f(0x3F00_0000), f(0xBED6_5E46), f(0xBDA6_86E8));
        for i in 0..n {
            let r = rgb[3 * i] as f32;
            let gg = rgb[3 * i + 1] as f32;
            let b = rgb[3 * i + 2] as f32;
            self.planes[0][i] = ((yr * r + yg * gg) + yb * b) - 128.0;
            self.full_cb[i] = (br * r + bg * gg) + bb * b;
            self.full_cr[i] = (rr * r + rg * gg) + rb * b;
        }
        for (c, full) in [(1usize, &self.full_cb), (2, &self.full_cr)] {
            let p = &mut self.planes[c];
            p.clear();
            p.resize(cw * ch, 0.0);
            for y in 0..ch {
                for x in 0..cw {
                    let mut s = 0f32;
                    for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        let yy = (2 * y + dy).min(h - 1);
                        let xx = (2 * x + dx).min(w - 1);
                        s += full[yy * w + xx];
                    }
                    p[y * cw + x] = s * 0.25;
                }
            }
        }
    }
}

/// The picture decoder. It keeps its buffers between pictures.
#[derive(Default)]
pub struct Decoder {
    geo: Option<Geometry>,
    planes: [Vec<f32>; 3],
    line: Vec<f32>,
    tmp: Vec<f32>,
    st: State,
}

impl Decoder {
    /// Makes a decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes `stream` with the header values into RGB bytes in `rgb` (resized to width x height x 3). Returns the number of
    /// decisions decoded. Zero bytes after the stream have no effect, because the decoder stops after `decisions` decisions.
    ///
    /// Errors: [`Error::BadParameter`] when the size is outside 1 to 8191, the levels are outside 1 to 8 or the top plane is
    /// outside -1 to 14.
    pub fn decode(&mut self, hd: &Header, stream: &[u8], rgb: &mut Vec<u8>) -> Result<u32, Error> {
        check_size(hd.width, hd.height)?;
        if !(1..=8).contains(&hd.levels) {
            return Err(Error::BadParameter {
                param: Param::Levels,
            });
        }
        if !(-1..=14).contains(&hd.top_plane) {
            return Err(Error::BadParameter {
                param: Param::TopPlane,
            });
        }
        let (w, h) = (hd.width as usize, hd.height as usize);
        let g = geometry(&mut self.geo, w, h, hd.levels);
        self.st.reset(&g);
        let mut ctx = Contexts::new();
        let mut dec = RangeDecoder::new(stream, hd.decisions);
        plane_loop(&g, &mut self.st, &mut ctx, hd.top_plane, &mut dec);
        let decided = dec.decisions();
        let off = f(0x3ED7_0A3D);
        for c in 0..3 {
            let p = &mut self.planes[c];
            p.clear();
            p.resize(g.pw[c] * g.ph[c], 0.0);
        }
        for b in &g.bands {
            let pw = g.pw[b.plane];
            let stride = b.w + 2;
            for y in 0..b.h {
                for x in 0..b.w {
                    let idx = (b.y + y) * pw + b.x + x;
                    let lp = self.st.low[b.plane][idx];
                    if lp < 0 {
                        continue;
                    }
                    let mut v = self.st.mag[b.plane][idx] as f32 + off * (1u32 << lp) as f32;
                    if self.st.map[b.map + (y + 1) * stride + x + 1] == 2 {
                        v = -v;
                    }
                    self.planes[b.plane][idx] = v;
                }
            }
        }
        for c in 0..3 {
            wavelet::inverse_2d(
                &mut self.planes[c],
                g.pw[c],
                g.ph[c],
                g.levels[c],
                &mut self.line,
                &mut self.tmp,
            );
        }
        let (cw, ch) = (g.pw[1], g.ph[1]);
        rgb.clear();
        rgb.resize(w * h * 3, 0);
        let (k_r, k_gb, k_gr, k_b) = (
            f(0x3FB3_74BC),
            f(0x3EB0_3298),
            f(0x3F36_D19E),
            f(0x3FE2_D0E5),
        );
        let cb = &self.planes[1];
        let cr = &self.planes[2];
        let sample = |pl: &[f32], fx: f32, fy: f32| {
            let fx = fx.clamp(0.0, (cw - 1) as f32);
            let fy = fy.clamp(0.0, (ch - 1) as f32);
            let x0 = fx as usize;
            let y0 = fy as usize;
            let x1 = (x0 + 1).min(cw - 1);
            let y1 = (y0 + 1).min(ch - 1);
            let ax = fx - x0 as f32;
            let ay = fy - y0 as f32;
            let top = pl[y0 * cw + x0] * (1.0 - ax) + pl[y0 * cw + x1] * ax;
            let bottom = pl[y1 * cw + x0] * (1.0 - ax) + pl[y1 * cw + x1] * ax;
            top * (1.0 - ay) + bottom * ay
        };
        let byte = |v: f32| (v.clamp(0.0, 255.0) + 0.5) as u8;
        for y in 0..h {
            let fy = (y as f32 - 0.5) * 0.5;
            for x in 0..w {
                let yv = self.planes[0][y * w + x] + 128.0;
                let fx = (x as f32 - 0.5) * 0.5;
                let cbv = sample(cb, fx, fy);
                let crv = sample(cr, fx, fy);
                let i = 3 * (y * w + x);
                rgb[i] = byte(yv + k_r * crv);
                rgb[i + 1] = byte((yv - k_gb * cbv) - k_gr * crv);
                rgb[i + 2] = byte(yv + k_b * cbv);
            }
        }
        self.geo = Some(g);
        Ok(decided)
    }
}
