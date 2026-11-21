//! Baseline JPEG and JFIF files at three layers: the entropy-coded blocks ([`entropy`]), the 8 x 8 transforms ([`dct`]), and
//! files ([`decode_coefficients`], [`decode`], [`encode`]). A block is the standard's term for 8 x 8 samples or coefficients.
//!
//! Standard: ITU-T T.81 (ISO/IEC 10918-1), ITU-T T.871.

pub mod dct;
pub mod entropy;
pub mod tables;

pub use dct::{dequantise, fdct_8x8, idct_8x8, quantise};
pub use entropy::{decode_block, encode_block, EntropyReader, EntropyWriter, HuffmanTable};

use crate::colour::{Converter, Range, BT601};
use crate::{ImageError, PixelFormat, Raster};
use std::sync::OnceLock;
use tables::*;

/// One block of quantised coefficients, in zig-zag order.
pub type Block = [i16; 64];

/// Why JPEG data did not decode or encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JpegError {
    /// The data does not start with SOI (0xFFD8).
    #[error("not a JPEG file")]
    NotJpeg,
    /// A Huffman code that is not in the table, or a symbol out of range.
    #[error("bad Huffman code")]
    BadCode,
    /// A run of zeros past the last coefficient of a block.
    #[error("coefficient run past the end of a block")]
    Overrun,
    /// The data ended (or a marker came) inside a block or a segment.
    #[error("JPEG data cut short")]
    Truncated,
    /// A marker in the wrong place, or a marker segment of a bad length.
    #[error("bad marker")]
    BadMarker,
    /// A Huffman or quantisation table that is bad or missing.
    #[error("bad or missing table")]
    BadTable,
    /// A coefficient too large for baseline coding.
    #[error("coefficient too large for baseline coding")]
    ValueTooLarge,
    /// A JPEG process or picture this decoder does not have.
    #[error("unsupported JPEG: {0:?}")]
    Unsupported(JpegFeature),
}

/// A JPEG process or picture that the baseline decoder does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JpegFeature {
    /// Progressive coding (SOF2, SOF6, SOF10, SOF14).
    Progressive,
    /// Lossless coding (SOF3, SOF7, SOF11, SOF15).
    Lossless,
    /// Hierarchical coding (SOF5).
    Hierarchical,
    /// Arithmetic coding (SOF9, SOF13).
    Arithmetic,
    /// A height given later by a DNL marker.
    Dnl,
    /// A sample precision other than 8 bits.
    Precision {
        /// The precision of the frame header.
        bits: u8,
    },
    /// A picture larger than [`crate::raster::MAX_BYTES`].
    Size {
        /// The width of the frame header.
        width: u32,
        /// The height of the frame header.
        height: u32,
    },
    /// A scan of part of the coefficients (spectral selection or successive approximation).
    SpectralSelection,
    /// A component count other than 1 or 3.
    Components {
        /// The count.
        count: u8,
    },
}

/// The four Annex K Huffman tables, built once.
#[derive(Debug)]
pub struct StandardTables {
    /// Luminance DC (Table K.3).
    pub dc_luma: HuffmanTable,
    /// Luminance AC (Table K.5).
    pub ac_luma: HuffmanTable,
    /// Chrominance DC (Table K.4).
    pub dc_chroma: HuffmanTable,
    /// Chrominance AC (Table K.6).
    pub ac_chroma: HuffmanTable,
}

impl StandardTables {
    /// The tables (built on the first call).
    pub fn get() -> &'static StandardTables {
        static T: OnceLock<StandardTables> = OnceLock::new();
        T.get_or_init(|| {
            let make = |b: &[u8; 16], v: &[u8]| {
                HuffmanTable::new(b, v).unwrap_or_else(|_| unreachable!("Annex K tables are valid"))
            };
            StandardTables {
                dc_luma: make(&DC_LUMA_BITS, &DC_VALS),
                ac_luma: make(&AC_LUMA_BITS, &AC_LUMA_VALS),
                dc_chroma: make(&DC_CHROMA_BITS, &DC_VALS),
                ac_chroma: make(&AC_CHROMA_BITS, &AC_CHROMA_VALS),
            }
        })
    }
}

/// One component of a decoded frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameComponent {
    /// The component identifier of the frame header.
    pub id: u8,
    /// The horizontal sampling factor (1 to 4).
    pub h: u8,
    /// The vertical sampling factor (1 to 4).
    pub v: u8,
    /// The quantisation table selector (0 to 3).
    pub tq: u8,
    /// Blocks across, padding included (whole MCUs).
    pub blocks_w: usize,
    /// Blocks down, padding included (whole MCUs).
    pub blocks_h: usize,
    /// The blocks, row by row, quantised, in zig-zag order.
    pub blocks: Vec<Block>,
}

/// A baseline JPEG frame as quantised coefficients.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The components, in frame header order.
    pub components: Vec<FrameComponent>,
    /// The quantisation tables 0 to 3, in natural order.
    pub qtables: [Option<[u16; 64]>; 4],
    /// The first error in the entropy-coded data, when the data is damaged. The blocks before it hold their values, the
    /// rest are 0 (flat mid grey).
    pub damaged: Option<JpegError>,
}

fn be16(b: &[u8], i: usize) -> Result<usize, JpegError> {
    match b.get(i..i + 2) {
        Some(s) => Ok(u16::from_be_bytes([s[0], s[1]]) as usize),
        None => Err(JpegError::Truncated),
    }
}

/// Reads a baseline or extended sequential (8-bit, Huffman) JPEG file into its quantised coefficients. A progressive,
/// lossless, hierarchical or arithmetic-coded file returns [`JpegError::Unsupported`]. Damaged entropy-coded data does not
/// fail: [`Frame::damaged`] says what went wrong.
pub fn decode_coefficients(b: &[u8]) -> Result<Frame, JpegError> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return Err(JpegError::NotJpeg);
    }
    let mut dc: [Option<HuffmanTable>; 4] = Default::default();
    let mut ac: [Option<HuffmanTable>; 4] = Default::default();
    let mut qt: [Option<[u16; 64]>; 4] = [None; 4];
    let mut frame: Option<Frame> = None;
    let mut restart = 0usize;
    let mut i = 2;
    loop {
        if i >= b.len() {
            // A file without EOI: keep what was decoded.
            return frame.ok_or(JpegError::Truncated);
        }
        if b[i] != 0xFF {
            return Err(JpegError::BadMarker);
        }
        while i < b.len() && b[i] == 0xFF {
            i += 1;
        }
        let m = *b.get(i).ok_or(JpegError::Truncated)?;
        i += 1;
        match m {
            0xD9 => return frame.ok_or(JpegError::Truncated),
            0xD0..=0xD7 | 0x01 => continue,
            0xD8 => return Err(JpegError::BadMarker),
            _ => {}
        }
        let len = be16(b, i)?;
        if len < 2 || i + len > b.len() {
            return Err(JpegError::Truncated);
        }
        let seg = &b[i + 2..i + len];
        i += len;
        match m {
            0xC4 => read_dht(seg, &mut dc, &mut ac)?,
            0xDB => read_dqt(seg, &mut qt)?,
            0xC0 | 0xC1 => {
                if frame.is_some() {
                    return Err(JpegError::BadMarker);
                }
                frame = Some(read_sof(seg)?);
            }
            0xC2 | 0xC6 | 0xCA | 0xCE => {
                return Err(JpegError::Unsupported(JpegFeature::Progressive))
            }
            0xC3 | 0xC7 | 0xCB | 0xCF => return Err(JpegError::Unsupported(JpegFeature::Lossless)),
            0xC5 => return Err(JpegError::Unsupported(JpegFeature::Hierarchical)),
            0xC9 | 0xCD => return Err(JpegError::Unsupported(JpegFeature::Arithmetic)),
            0xDC => return Err(JpegError::Unsupported(JpegFeature::Dnl)),
            0xDD => restart = be16(seg, 0)?,
            0xDA => {
                let f = frame.as_mut().ok_or(JpegError::BadMarker)?;
                f.qtables = qt;
                let end = read_scan(seg, &b[i..], f, &dc, &ac, restart)?;
                i += end;
            }
            _ => {} // APPn, COM and others: skipped.
        }
    }
}

fn read_dht(
    mut seg: &[u8],
    dc: &mut [Option<HuffmanTable>; 4],
    ac: &mut [Option<HuffmanTable>; 4],
) -> Result<(), JpegError> {
    while !seg.is_empty() {
        if seg.len() < 17 {
            return Err(JpegError::BadTable);
        }
        let (tc, th) = (seg[0] >> 4, (seg[0] & 15) as usize);
        let bits: [u8; 16] = seg[1..17].try_into().map_err(|_| JpegError::BadTable)?;
        let n: usize = bits.iter().map(|&x| x as usize).sum();
        let vals = seg.get(17..17 + n).ok_or(JpegError::BadTable)?;
        if th > 3 || tc > 1 {
            return Err(JpegError::BadTable);
        }
        let t = HuffmanTable::new(&bits, vals)?;
        if tc == 0 {
            dc[th] = Some(t);
        } else {
            ac[th] = Some(t);
        }
        seg = &seg[17 + n..];
    }
    Ok(())
}

fn read_dqt(mut seg: &[u8], qt: &mut [Option<[u16; 64]>; 4]) -> Result<(), JpegError> {
    while !seg.is_empty() {
        let (pq, tq) = (seg[0] >> 4, (seg[0] & 15) as usize);
        let n = if pq == 0 { 64 } else { 128 };
        let body = seg.get(1..1 + n).ok_or(JpegError::BadTable)?;
        if tq > 3 || pq > 1 {
            return Err(JpegError::BadTable);
        }
        let mut t = [0u16; 64];
        for (k, &z) in ZIGZAG.iter().enumerate() {
            t[z as usize] = if pq == 0 {
                body[k] as u16
            } else {
                u16::from_be_bytes([body[2 * k], body[2 * k + 1]])
            };
        }
        qt[tq] = Some(t);
        seg = &seg[1 + n..];
    }
    Ok(())
}

fn read_sof(seg: &[u8]) -> Result<Frame, JpegError> {
    if seg.len() < 6 {
        return Err(JpegError::BadMarker);
    }
    if seg[0] != 8 {
        return Err(JpegError::Unsupported(JpegFeature::Precision {
            bits: seg[0],
        }));
    }
    let height = be16(seg, 1)? as u32;
    let width = be16(seg, 3)? as u32;
    let nf = seg[5] as usize;
    if height == 0 {
        return Err(JpegError::Unsupported(JpegFeature::Dnl));
    }
    if width == 0 || nf == 0 || nf > 4 || seg.len() < 6 + 3 * nf {
        return Err(JpegError::BadMarker);
    }
    if width as usize * height as usize * nf > crate::raster::MAX_BYTES {
        return Err(JpegError::Unsupported(JpegFeature::Size { width, height }));
    }
    let mut comps = Vec::with_capacity(nf);
    for k in 0..nf {
        let c = &seg[6 + 3 * k..9 + 3 * k];
        let (h, v) = (c[1] >> 4, c[1] & 15);
        if !(1..=4).contains(&h) || !(1..=4).contains(&v) || c[2] > 3 {
            return Err(JpegError::BadMarker);
        }
        comps.push(FrameComponent {
            id: c[0],
            h,
            v,
            tq: c[2],
            blocks_w: 0,
            blocks_h: 0,
            blocks: Vec::new(),
        });
    }
    let hmax = comps.iter().map(|c| c.h).max().unwrap_or(1) as usize;
    let vmax = comps.iter().map(|c| c.v).max().unwrap_or(1) as usize;
    let mx = (width as usize).div_ceil(8 * hmax);
    let my = (height as usize).div_ceil(8 * vmax);
    for c in comps.iter_mut() {
        c.blocks_w = mx * c.h as usize;
        c.blocks_h = my * c.v as usize;
        c.blocks = vec![[0; 64]; c.blocks_w * c.blocks_h];
    }
    Ok(Frame {
        width,
        height,
        components: comps,
        qtables: [None; 4],
        damaged: None,
    })
}

// Decodes one scan. `data` starts after the SOS segment. Returns the number of bytes of entropy-coded data used.
fn read_scan(
    seg: &[u8],
    data: &[u8],
    f: &mut Frame,
    dc: &[Option<HuffmanTable>; 4],
    ac: &[Option<HuffmanTable>; 4],
    restart: usize,
) -> Result<usize, JpegError> {
    let ns = *seg.first().ok_or(JpegError::BadMarker)? as usize;
    if ns == 0 || ns > 4 || seg.len() < 1 + 2 * ns + 3 {
        return Err(JpegError::BadMarker);
    }
    let (ss, se, a) = (seg[1 + 2 * ns], seg[2 + 2 * ns], seg[3 + 2 * ns]);
    if ss != 0 || se != 63 || a != 0 {
        return Err(JpegError::Unsupported(JpegFeature::SpectralSelection));
    }
    // (component index, DC table, AC table)
    let mut sc: [(usize, &HuffmanTable, &HuffmanTable); 4] = {
        let t = StandardTables::get();
        [(0, &t.dc_luma, &t.ac_luma); 4]
    };
    for k in 0..ns {
        let (cs, t) = (seg[1 + 2 * k], seg[2 + 2 * k]);
        let ci = f
            .components
            .iter()
            .position(|c| c.id == cs)
            .ok_or(JpegError::BadMarker)?;
        let d = dc[(t >> 4) as usize & 3]
            .as_ref()
            .ok_or(JpegError::BadTable)?;
        let a = ac[(t & 15) as usize & 3]
            .as_ref()
            .ok_or(JpegError::BadTable)?;
        sc[k] = (ci, d, a);
    }
    let hmax = f.components.iter().map(|c| c.h).max().unwrap_or(1) as usize;
    let vmax = f.components.iter().map(|c| c.v).max().unwrap_or(1) as usize;
    let mut r = EntropyReader::new(data, true);
    let mut pred = [0i32; 4];
    let mut result: Result<(), JpegError> = Ok(());
    if ns == 1 {
        // Non-interleaved: the component's own blocks in raster order (T.81 A.2.2).
        let (ci, d, a) = sc[0];
        let c = &mut f.components[ci];
        let bw = (f.width as usize * c.h as usize).div_ceil(hmax).div_ceil(8);
        let bh = (f.height as usize * c.v as usize)
            .div_ceil(vmax)
            .div_ceil(8);
        let mut n = 0usize;
        'scan: for y in 0..bh {
            for x in 0..bw {
                if restart > 0 && n > 0 && n.is_multiple_of(restart) {
                    if let Err(e) = r.restart() {
                        result = Err(e);
                        break 'scan;
                    }
                    pred = [0; 4];
                }
                n += 1;
                let blk = &mut c.blocks[y * c.blocks_w + x];
                if let Err(e) = decode_block(&mut r, d, a, &mut pred[0], blk) {
                    result = Err(e);
                    break 'scan;
                }
            }
        }
    } else {
        // Interleaved: MCU by MCU, each component's H x V blocks (T.81 A.2.3).
        let mx = (f.width as usize).div_ceil(8 * hmax);
        let my = (f.height as usize).div_ceil(8 * vmax);
        let mut n = 0usize;
        'mcus: for m_y in 0..my {
            for m_x in 0..mx {
                if restart > 0 && n > 0 && n.is_multiple_of(restart) {
                    if let Err(e) = r.restart() {
                        result = Err(e);
                        break 'mcus;
                    }
                    pred = [0; 4];
                }
                n += 1;
                for (k, &(ci, d, a)) in sc[..ns].iter().enumerate() {
                    let c = &mut f.components[ci];
                    let (h, v) = (c.h as usize, c.v as usize);
                    for by in 0..v {
                        for bx in 0..h {
                            let idx = (m_y * v + by) * c.blocks_w + m_x * h + bx;
                            if let Err(e) =
                                decode_block(&mut r, d, a, &mut pred[k], &mut c.blocks[idx])
                            {
                                result = Err(e);
                                break 'mcus;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Err(e) = result {
        f.damaged.get_or_insert(e);
    }
    // Move to the next marker after the entropy-coded data.
    let mut p = r.position().min(data.len());
    while p + 1 < data.len() {
        if data[p] == 0xFF && data[p + 1] != 0x00 && !(0xD0..=0xD7).contains(&data[p + 1]) {
            break;
        }
        p += 1;
    }
    Ok(if p + 1 < data.len() { p } else { data.len() })
}

impl Frame {
    /// The component planes after dequantising and the inverse DCT, each `blocks_w` x 8 by `blocks_h` x 8 samples. A missing
    /// quantisation table returns [`JpegError::BadTable`].
    pub fn planes(&self) -> Result<Vec<Vec<u8>>, JpegError> {
        let mut out = Vec::with_capacity(self.components.len());
        let mut deq = [0f32; 64];
        for c in &self.components {
            let q = self.qtables[c.tq as usize & 3]
                .as_ref()
                .ok_or(JpegError::BadTable)?;
            let stride = c.blocks_w * 8;
            let mut plane = vec![0u8; stride * c.blocks_h * 8];
            for by in 0..c.blocks_h {
                for bx in 0..c.blocks_w {
                    dequantise(&c.blocks[by * c.blocks_w + bx], q, &mut deq);
                    idct_8x8(&deq, &mut plane[by * 8 * stride + bx * 8..], stride);
                }
            }
            out.push(plane);
        }
        Ok(out)
    }

    /// The pixels. One component produces grey. Three components are read as JFIF Y'CbCr (T.871 clause 7) and produce BGRA. A
    /// component sampled at less than the full rate is enlarged by bilinear interpolation. Other component counts return
    /// [`JpegError::Unsupported`].
    pub fn to_raster(&self) -> Result<Raster, JpegError> {
        let planes = self.planes()?;
        let (w, h) = (self.width as usize, self.height as usize);
        let hmax = self.components.iter().map(|c| c.h).max().unwrap_or(1) as usize;
        let vmax = self.components.iter().map(|c| c.v).max().unwrap_or(1) as usize;
        // A component at less than the full rate is enlarged by bilinear interpolation between sample centres (the
        // "fancy upsampling" most decoders use), held to the samples that cover the picture.
        let sample = |k: usize, x: usize, y: usize| -> u8 {
            let c = &self.components[k];
            let stride = c.blocks_w * 8;
            if c.h as usize == hmax && c.v as usize == vmax {
                return planes[k][y * stride + x];
            }
            let cw = (w * c.h as usize).div_ceil(hmax).max(1);
            let ch = (h * c.v as usize).div_ceil(vmax).max(1);
            let fx =
                ((x as f32 + 0.5) * c.h as f32 / hmax as f32 - 0.5).clamp(0.0, (cw - 1) as f32);
            let fy =
                ((y as f32 + 0.5) * c.v as f32 / vmax as f32 - 0.5).clamp(0.0, (ch - 1) as f32);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(cw - 1), (y0 + 1).min(ch - 1));
            let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
            let p = &planes[k];
            let top = p[y0 * stride + x0] as f32 * (1.0 - ax) + p[y0 * stride + x1] as f32 * ax;
            let bottom = p[y1 * stride + x0] as f32 * (1.0 - ax) + p[y1 * stride + x1] as f32 * ax;
            (top * (1.0 - ay) + bottom * ay + 0.5) as u8
        };
        match self.components.len() {
            1 => {
                let mut r = Raster::zeroed(PixelFormat::Grey8, w as u32, h as u32);
                let stride = self.components[0].blocks_w * 8;
                for (y, d) in r.data_mut().chunks_exact_mut(w.max(1)).enumerate() {
                    d.copy_from_slice(&planes[0][y * stride..y * stride + w]);
                }
                Ok(r)
            }
            3 => {
                let conv = Converter::new(BT601, Range::Full);
                let mut r = Raster::zeroed(PixelFormat::Bgra, w as u32, h as u32);
                for (i, d) in r.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let (x, y) = (i % w, i / w);
                    let [red, g, b] = conv.rgb([sample(0, x, y), sample(1, x, y), sample(2, x, y)]);
                    *d = [b, g, red, 255];
                }
                Ok(r)
            }
            n => Err(JpegError::Unsupported(JpegFeature::Components {
                count: n as u8,
            })),
        }
    }
}

/// Decodes a baseline JPEG file to pixels (grey or BGRA). Damaged entropy-coded data produces a picture with flat grey in the
/// damaged part ([`decode_coefficients`] reports the damage).
pub fn decode(bytes: &[u8]) -> Result<Raster, ImageError> {
    Ok(decode_coefficients(bytes)?.to_raster()?)
}

/// The chroma sampling of a colour JPEG file that [`encode`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subsampling {
    /// Cb and Cr at full resolution.
    S444,
    /// Cb and Cr at half resolution in both directions (each chroma sample is the mean of 2 x 2 pixels).
    S420,
}

/// One component of a frame header that [`JfifHeader`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentSpec {
    /// The component identifier.
    pub id: u8,
    /// The horizontal sampling factor.
    pub h: u8,
    /// The vertical sampling factor.
    pub v: u8,
    /// The quantisation table (0 to 3). Component 0 uses the luminance Huffman tables, the others the chrominance tables.
    pub tq: u8,
}

/// The headers of a baseline JFIF file with the Annex K Huffman tables: SOI, APP0 (JFIF 1.01, no density), DQT, SOF0, DHT and
/// SOS. A writer of a JPEG file from its own coefficients (SSDV, a test generator) writes these, then the entropy-coded data
/// with [`encode_block`] and the [`StandardTables`], then [`JfifHeader::finish`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JfifHeader {
    /// The width in pixels (1 to 65535).
    pub width: u16,
    /// The height in pixels (1 to 65535).
    pub height: u16,
    /// The components: one for grey, three for Y'CbCr.
    pub components: Vec<ComponentSpec>,
    /// The quantisation tables, by number, in natural order. Entries above 255 are written as 16-bit (an extended sequential
    /// file then, T.81 B.2.4.1).
    pub qtables: Vec<(u8, [u16; 64])>,
}

impl JfifHeader {
    /// The header of a grey (one component) file with the scaled Annex K luminance table.
    pub fn grey(width: u16, height: u16, quality: u32) -> JfifHeader {
        JfifHeader {
            width,
            height,
            components: vec![ComponentSpec {
                id: 1,
                h: 1,
                v: 1,
                tq: 0,
            }],
            qtables: vec![(0, scaled(&LUMA_Q, quality, BASELINE_MAX_Q))],
        }
    }

    /// The header of a Y'CbCr file with the scaled Annex K tables.
    pub fn colour(width: u16, height: u16, quality: u32, sub: Subsampling) -> JfifHeader {
        let f = if sub == Subsampling::S420 { 2 } else { 1 };
        JfifHeader {
            width,
            height,
            components: vec![
                ComponentSpec {
                    id: 1,
                    h: f,
                    v: f,
                    tq: 0,
                },
                ComponentSpec {
                    id: 2,
                    h: 1,
                    v: 1,
                    tq: 1,
                },
                ComponentSpec {
                    id: 3,
                    h: 1,
                    v: 1,
                    tq: 1,
                },
            ],
            qtables: vec![
                (0, scaled(&LUMA_Q, quality, BASELINE_MAX_Q)),
                (1, scaled(&CHROMA_Q, quality, BASELINE_MAX_Q)),
            ],
        }
    }

    /// Appends the headers to `out`.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[0xFF, 0xD8]);
        out.extend_from_slice(&[
            0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
        ]);
        for (id, t) in &self.qtables {
            let wide = t.iter().any(|&v| v > 255);
            let len: u16 = if wide { 2 + 1 + 128 } else { 2 + 1 + 64 };
            out.extend_from_slice(&[0xFF, 0xDB]);
            out.extend_from_slice(&len.to_be_bytes());
            out.push(((wide as u8) << 4) | (id & 3));
            for &z in &ZIGZAG {
                let v = t[z as usize].max(1);
                if wide {
                    out.extend_from_slice(&v.to_be_bytes());
                } else {
                    out.push(v as u8);
                }
            }
        }
        let wide = self.qtables.iter().any(|(_, t)| t.iter().any(|&v| v > 255));
        let nf = self.components.len();
        out.extend_from_slice(&[0xFF, if wide { 0xC1 } else { 0xC0 }]);
        out.extend_from_slice(&((8 + 3 * nf) as u16).to_be_bytes());
        out.push(8);
        out.extend_from_slice(&self.height.to_be_bytes());
        out.extend_from_slice(&self.width.to_be_bytes());
        out.push(nf as u8);
        for c in &self.components {
            out.extend_from_slice(&[c.id, (c.h << 4) | c.v, c.tq]);
        }
        let dht = |out: &mut Vec<u8>, class_id: u8, bits: &[u8; 16], vals: &[u8]| {
            out.extend_from_slice(&[0xFF, 0xC4]);
            out.extend_from_slice(&((2 + 1 + 16 + vals.len()) as u16).to_be_bytes());
            out.push(class_id);
            out.extend_from_slice(bits);
            out.extend_from_slice(vals);
        };
        dht(out, 0x00, &DC_LUMA_BITS, &DC_VALS);
        dht(out, 0x10, &AC_LUMA_BITS, &AC_LUMA_VALS);
        if nf > 1 {
            dht(out, 0x01, &DC_CHROMA_BITS, &DC_VALS);
            dht(out, 0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS);
        }
        out.extend_from_slice(&[0xFF, 0xDA]);
        out.extend_from_slice(&((6 + 2 * nf) as u16).to_be_bytes());
        out.push(nf as u8);
        for (k, c) in self.components.iter().enumerate() {
            out.extend_from_slice(&[c.id, if k == 0 { 0x00 } else { 0x11 }]);
        }
        out.extend_from_slice(&[0, 63, 0]);
    }

    /// Appends EOI to `out`, after the entropy-coded data.
    pub fn finish(out: &mut Vec<u8>) {
        out.extend_from_slice(&[0xFF, 0xD9]);
    }
}

// Copies a plane into a padded plane of `pw` x `ph`, repeating the last column and row.
fn padded(src: &[u8], w: usize, h: usize, pw: usize, ph: usize) -> Vec<u8> {
    let mut out = vec![0u8; pw * ph];
    for y in 0..ph {
        let sy = y.min(h - 1);
        for x in 0..pw {
            out[y * pw + x] = src[sy * w + x.min(w - 1)];
        }
    }
    out
}

/// Writes a baseline JFIF file of a raster at a quality from 1 to 100 (Annex K tables scaled as [`tables::scaled`] does).
/// Grey produces a one-component file. RGB and BGRA produce a Y'CbCr file (T.871 clause 7) with the chroma subsampling `sub`. A picture
/// wider or taller than 65535 pixels, or empty, returns an error.
///
/// ```
/// use sdrkit_codec_image::jpeg::{self, Subsampling};
/// use sdrkit_codec_image::{PixelFormat, Raster};
///
/// let px: Vec<u8> = (0..32 * 16).map(|i| (i % 32 * 8) as u8).collect();
/// let r = Raster::from_vec(PixelFormat::Grey8, 32, 16, px).unwrap();
/// let file = jpeg::encode(&r, 90, Subsampling::S444).unwrap();
/// let back = jpeg::decode(&file).unwrap();
/// assert_eq!((back.width(), back.height()), (32, 16));
/// ```
pub fn encode(r: &Raster, quality: u32, sub: Subsampling) -> Result<Vec<u8>, ImageError> {
    let (w, h) = (r.width() as usize, r.height() as usize);
    if w == 0 || h == 0 || w > 65535 || h > 65535 {
        return Err(ImageError::TooLarge {
            width: r.width(),
            height: r.height(),
        });
    }
    let t = StandardTables::get();
    let mut out = Vec::with_capacity(w * h / 2 + 1024);
    if r.format() == PixelFormat::Grey8 {
        let hd = JfifHeader::grey(w as u16, h as u16, quality);
        hd.write(&mut out);
        let (pw, ph) = (w.div_ceil(8) * 8, h.div_ceil(8) * 8);
        let p = padded(r.data(), w, h, pw, ph);
        let q = &hd.qtables[0].1;
        let mut wr = EntropyWriter::new(&mut out, true);
        let (mut pred, mut f, mut zz) = (0i32, [0f32; 64], [0i16; 64]);
        for by in 0..ph / 8 {
            for bx in 0..pw / 8 {
                fdct_8x8(&p[by * 8 * pw + bx * 8..], pw, &mut f);
                quantise(&f, q, &mut zz);
                encode_block(&mut wr, &t.dc_luma, &t.ac_luma, &mut pred, &zz)?;
            }
        }
        wr.align();
        JfifHeader::finish(&mut out);
        return Ok(out);
    }
    // Colour: Y'CbCr planes.
    let conv = Converter::new(BT601, Range::Full);
    let n = w * h;
    let (mut yp, mut cbp, mut crp) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
    for (i, px) in r.rgb().enumerate() {
        let [y, cb, cr] = conv.ycbcr(px);
        (yp[i], cbp[i], crp[i]) = (y, cb, cr);
    }
    let f = if sub == Subsampling::S420 { 2 } else { 1 };
    let (mw, mh) = (8 * f, 8 * f);
    let (pw, ph) = (w.div_ceil(mw) * mw, h.div_ceil(mh) * mh);
    let yp = padded(&yp, w, h, pw, ph);
    let (cw, ch) = (pw / f, ph / f);
    let chroma = |src: Vec<u8>| -> Vec<u8> {
        let p = padded(&src, w, h, pw, ph);
        if f == 1 {
            return p;
        }
        let mut out = vec![0u8; cw * ch];
        for y in 0..ch {
            for x in 0..cw {
                let s = p[2 * y * pw + 2 * x] as u32
                    + p[2 * y * pw + 2 * x + 1] as u32
                    + p[(2 * y + 1) * pw + 2 * x] as u32
                    + p[(2 * y + 1) * pw + 2 * x + 1] as u32;
                out[y * cw + x] = ((s + 2) / 4) as u8;
            }
        }
        out
    };
    let (cbp, crp) = (chroma(cbp), chroma(crp));
    let hd = JfifHeader::colour(w as u16, h as u16, quality, sub);
    hd.write(&mut out);
    let (lq, cq) = (hd.qtables[0].1, hd.qtables[1].1);
    let mut wr = EntropyWriter::new(&mut out, true);
    let mut pred = [0i32; 3];
    let (mut fb, mut zz) = ([0f32; 64], [0i16; 64]);
    for my in 0..ph / mh {
        for mx in 0..pw / mw {
            for by in 0..f {
                for bx in 0..f {
                    let o = (my * mh + by * 8) * pw + mx * mw + bx * 8;
                    fdct_8x8(&yp[o..], pw, &mut fb);
                    quantise(&fb, &lq, &mut zz);
                    encode_block(&mut wr, &t.dc_luma, &t.ac_luma, &mut pred[0], &zz)?;
                }
            }
            for (k, plane) in [&cbp, &crp].into_iter().enumerate() {
                fdct_8x8(&plane[my * 8 * cw + mx * 8..], cw, &mut fb);
                quantise(&fb, &cq, &mut zz);
                encode_block(&mut wr, &t.dc_chroma, &t.ac_chroma, &mut pred[k + 1], &zz)?;
            }
        }
    }
    wr.align();
    JfifHeader::finish(&mut out);
    Ok(out)
}

/// One 4:2:0 MCU (minimum coded unit) of quantised coefficients in zig-zag order: four luminance blocks (left to right, top to
/// bottom), then Cb, then Cr. SSDV codes its pictures in these.
pub type Mcu420 = [Block; 6];

/// Writes a baseline JFIF 4:2:0 file of `mcus` (row by row) with the Annex K tables scaled to `quality`. A missing MCU
/// (`None`) is flat mid grey. `width` and `height` are rounded down to whole MCUs (16 x 16 pixels).
pub fn write_mcus_420(
    width: u16,
    height: u16,
    quality: u32,
    mcus: &[Option<Mcu420>],
) -> Result<Vec<u8>, JpegError> {
    let (w, h) = (width / 16 * 16, height / 16 * 16);
    let hd = JfifHeader::colour(w, h, quality, Subsampling::S420);
    let mut out = Vec::with_capacity(mcus.len() * 256 + 1024);
    hd.write(&mut out);
    let t = StandardTables::get();
    let mut wr = EntropyWriter::new(&mut out, true);
    let mut pred = [0i32; 3];
    let grey: Mcu420 = [[0; 64]; 6];
    for k in 0..(w as usize / 16) * (h as usize / 16) {
        let m = mcus.get(k).copied().flatten().unwrap_or(grey);
        for (b, blk) in m.iter().enumerate() {
            let c = b.saturating_sub(3);
            let (d, a) = if c == 0 {
                (&t.dc_luma, &t.ac_luma)
            } else {
                (&t.dc_chroma, &t.ac_chroma)
            };
            encode_block(&mut wr, d, a, &mut pred[c], blk)?;
        }
    }
    wr.align();
    JfifHeader::finish(&mut out);
    Ok(out)
}

/// Cuts an RGB picture into quantised 4:2:0 MCUs at a quality, row by row (a test picture for an SSDV generator). The width
/// and height are rounded down to whole MCUs. If `rgb` is shorter than `w` x `h` x 3 bytes, it returns no MCUs.
pub fn rgb_to_mcus_420(rgb: &[u8], w: usize, h: usize, quality: u32) -> Vec<Mcu420> {
    if rgb.len() < w * h * 3 {
        return Vec::new();
    }
    let lq = scaled(&LUMA_Q, quality, BASELINE_MAX_Q);
    let cq = scaled(&CHROMA_Q, quality, BASELINE_MAX_Q);
    let conv = Converter::new(BT601, Range::Full);
    let mut out = Vec::with_capacity((w / 16) * (h / 16));
    let (mut yb, mut cb, mut cr) = ([0u8; 256], [0u8; 64], [0u8; 64]);
    let mut f = [0f32; 64];
    for my in 0..h / 16 {
        for mx in 0..w / 16 {
            let mut sums = [[0f32; 2]; 64];
            for y in 0..16 {
                for x in 0..16 {
                    let i = 3 * ((my * 16 + y) * w + mx * 16 + x);
                    let p = conv.ycbcr_f32(
                        rgb[i] as f32 / 255.0,
                        rgb[i + 1] as f32 / 255.0,
                        rgb[i + 2] as f32 / 255.0,
                    );
                    yb[y * 16 + x] = (p[0] + 0.5).clamp(0.0, 255.0) as u8;
                    sums[(y / 2) * 8 + x / 2][0] += p[1];
                    sums[(y / 2) * 8 + x / 2][1] += p[2];
                }
            }
            for k in 0..64 {
                cb[k] = (sums[k][0] / 4.0 + 0.5).clamp(0.0, 255.0) as u8;
                cr[k] = (sums[k][1] / 4.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
            let mut m: Mcu420 = [[0; 64]; 6];
            for (b, blk) in m.iter_mut().enumerate().take(4) {
                fdct_8x8(&yb[(b / 2) * 128 + (b % 2) * 8..], 16, &mut f);
                quantise(&f, &lq, blk);
            }
            fdct_8x8(&cb, 8, &mut f);
            quantise(&f, &cq, &mut m[4]);
            fdct_8x8(&cr, 8, &mut f);
            quantise(&f, &cq, &mut m[5]);
            out.push(m);
        }
    }
    out
}
