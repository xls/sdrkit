//! Where the I and Q of each receiver lie in a buffer, the native ring samples made from it, and the packing of transmit
//! samples. The layout follows the mask that the server provides with the data, which can hold more channels than were requested.

use super::context::{DataFormat, Device};
use sdrkit_core::Complex32;
use sdrkit_graph::stream::SampleType;
use std::mem::MaybeUninit;

/// Whether bit `i` is set in a mask (word 0 holds bits 0 to 31).
pub fn bit(mask: &[u32], i: usize) -> bool {
    mask.get(i / 32).is_some_and(|w| w >> (i % 32) & 1 == 1)
}

/// A mask with the given bits set, in as many words as `channels` needs.
pub fn mask_of(bits: &[usize], channels: usize) -> Vec<u32> {
    let mut m = vec![0u32; channels.div_ceil(32).max(1)];
    for &b in bits {
        m[b / 32] |= 1 << (b % 32);
    }
    m
}

/// How the samples are coded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coding {
    /// Signed integers of `bytes` bytes (2 for the AD936x), little or big endian, shifted right by `shift`, with `bits`
    /// significant bits.
    Int {
        /// Bytes per value.
        bytes: usize,
        /// Big endian.
        big: bool,
        /// Right shift of the value in its storage.
        shift: u32,
        /// Significant bits (12 for the AD936x).
        bits: u32,
    },
    /// Tezuka CS8: one 16-bit channel that carries an 8-bit I, then an 8-bit Q.
    Cs8,
}

/// Where the I and Q of each receiver are in one sample instant.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Bytes per sample instant.
    pub stride: usize,
    /// Byte offsets of (I, Q) for each receiver.
    pub pairs: Vec<(usize, usize)>,
    /// The coding.
    pub coding: Coding,
}

/// The byte offset of each enabled channel (by position in the list of the device) in a sample instant, and the size of the
/// instant.
fn offsets(dev: &Device, mask: &[u32]) -> (Vec<(usize, usize, DataFormat)>, usize) {
    let mut chans: Vec<(u32, usize, DataFormat)> = dev
        .channels
        .iter()
        .enumerate()
        .filter(|(i, _)| bit(mask, *i))
        .filter_map(|(i, c)| Some((c.scan.as_ref()?.0, i, c.format()?)))
        .collect();
    chans.sort_by_key(|c| c.0);
    let mut at: usize = 0;
    let mut out = Vec::new();
    for (_, pos, f) in chans {
        let len = f.bytes();
        let align = (f.storage / 8) as usize;
        at = at.div_ceil(align) * align;
        out.push((pos, at, f));
        at += len;
    }
    let max_align = out
        .iter()
        .map(|o| (o.2.storage / 8) as usize)
        .max()
        .unwrap_or(1);
    (out, at.div_ceil(max_align) * max_align)
}

/// Why a buffer layout cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// The buffer does not carry the channel at this position.
    NoChannel(usize),
    /// The samples have a storage size that the driver does not read, in bits.
    Storage(u32),
    /// No receiver was requested.
    NoReceiver,
}

/// The layout of a buffer with channels `mask` enabled, for the receivers whose (I, Q) channels are at positions `want`. With
/// `cs8`, the pair of each receiver is one channel (I and Q both name it).
///
/// # Errors
///
/// [`LayoutError`] if the buffer does not carry a channel, or its storage size is not 8, 16 or 32 bits.
pub fn layout(
    dev: &Device,
    mask: &[u32],
    want: &[(usize, usize)],
    cs8: bool,
) -> Result<Layout, LayoutError> {
    let (offs, stride) = offsets(dev, mask);
    let find = |pos: usize| {
        offs.iter()
            .find(|o| o.0 == pos)
            .map(|o| (o.1, o.2))
            .ok_or(LayoutError::NoChannel(pos))
    };
    let mut pairs = Vec::new();
    let mut coding = None;
    for &(i, q) in want {
        let (oi, fi) = find(i)?;
        if cs8 {
            pairs.push((oi, oi + 1));
            coding = Some(Coding::Cs8);
        } else {
            let (oq, _) = find(q)?;
            pairs.push((oi, oq));
            let bytes = (fi.storage / 8) as usize;
            if bytes != 1 && bytes != 2 && bytes != 4 {
                return Err(LayoutError::Storage(fi.storage));
            }
            coding = Some(Coding::Int {
                bytes,
                big: fi.big_endian,
                shift: fi.shift,
                bits: fi.bits.clamp(1, 32),
            });
        }
    }
    Ok(Layout {
        stride,
        pairs,
        coding: coding.ok_or(LayoutError::NoReceiver)?,
    })
}

#[inline(always)]
fn int_at(raw: &[u8], at: usize, bytes: usize, big: bool, shift: u32) -> i32 {
    let v = match (bytes, big) {
        (2, false) => i32::from(i16::from_le_bytes([raw[at], raw[at + 1]])),
        (2, true) => i32::from(i16::from_be_bytes([raw[at], raw[at + 1]])),
        (1, _) => i32::from(raw[at] as i8),
        (_, false) => i32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]),
        (_, true) => i32::from_be_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]),
    };
    v >> shift
}

/// A value of `bits` significant bits at the full scale of 16 bits.
#[inline(always)]
fn full16(v: i32, bits: u32) -> [u8; 2] {
    let v = if bits <= 16 {
        v << (16 - bits)
    } else {
        v >> (bits - 16)
    };
    (v.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16).to_le_bytes()
}

#[inline(always)]
fn put(d: &mut [MaybeUninit<u8>], b: &[u8]) {
    for (slot, v) in d.iter_mut().zip(b) {
        slot.write(*v);
    }
}

impl Layout {
    /// Sample instants in `bytes` bytes.
    pub fn instants(&self, bytes: usize) -> usize {
        bytes / self.stride.max(1)
    }

    /// The type of the samples that [`Layout::fill`] writes: [`SampleType::Cs16`] at full 16-bit scale, or [`SampleType::Cs8`].
    pub fn sample_type(&self) -> SampleType {
        match self.coding {
            Coding::Cs8 => SampleType::Cs8,
            Coding::Int { .. } => SampleType::Cs16,
        }
    }

    /// Bytes of one sample that [`Layout::fill`] writes.
    pub fn sample_bytes(&self) -> usize {
        crate::sample::size(self.sample_type())
    }

    /// The I and Q of receiver `rx` in instant `p`, as ring bytes. Only the first [`Layout::sample_bytes`] bytes count.
    #[inline(always)]
    fn one(&self, p: &[u8], rx: usize) -> [u8; 4] {
        let (oi, oq) = self.pairs[rx];
        match self.coding {
            Coding::Int {
                bytes,
                big,
                shift,
                bits,
            } => {
                let i = full16(int_at(p, oi, bytes, big, shift), bits);
                let q = full16(int_at(p, oq, bytes, big, shift), bits);
                [i[0], i[1], q[0], q[1]]
            }
            Coding::Cs8 => [p[oi], p[oq], 0, 0],
        }
    }

    /// Writes the samples `from..` of receiver `rx` of `raw` into `dst` (ring bytes, a whole number of samples). It writes every
    /// byte of `dst`.
    pub fn fill(&self, raw: &[u8], rx: usize, dst: &mut [MaybeUninit<u8>], from: usize) {
        let (oi, oq) = self.pairs[rx];
        let s = self.stride;
        let size = self.sample_bytes();
        let n = dst.len() / size;
        let src = &raw[from * s..(from + n) * s];
        match self.coding {
            Coding::Int {
                bytes: 2,
                big: false,
                shift: 0,
                bits,
            } if s == 4 && oi == 0 && oq == 2 => {
                // One receiver, 16-bit storage.
                for (d, p) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<4>().0)
                {
                    let i = full16(i32::from(i16::from_le_bytes([p[0], p[1]])), bits);
                    let q = full16(i32::from(i16::from_le_bytes([p[2], p[3]])), bits);
                    put(d, &[i[0], i[1], q[0], q[1]]);
                }
            }
            _ => {
                for (d, p) in dst.chunks_exact_mut(size).zip(src.chunks_exact(s)) {
                    put(d, &self.one(p, rx)[..size]);
                }
            }
        }
    }

    /// The full scale of one component as [`Layout::components`] gives it: 2048 for 12 bits, 128 for CS8.
    pub fn full_scale(&self) -> i32 {
        match self.coding {
            Coding::Int { bits, .. } => 1 << (bits.clamp(2, 31) - 1),
            Coding::Cs8 => 128,
        }
    }

    /// Calls `f` with the I and Q of receiver `rx` for each instant of `raw`, as the converter gives them.
    pub fn components(&self, raw: &[u8], rx: usize, mut f: impl FnMut(i32, i32)) {
        let (oi, oq) = self.pairs[rx];
        let s = self.stride.max(1);
        match self.coding {
            Coding::Int {
                bytes,
                big,
                shift,
                ..
            } => {
                for p in raw.chunks_exact(s) {
                    f(
                        int_at(p, oi, bytes, big, shift),
                        int_at(p, oq, bytes, big, shift),
                    );
                }
            }
            Coding::Cs8 => {
                for p in raw.chunks_exact(s) {
                    f(i32::from(p[oi] as i8), i32::from(p[oq] as i8));
                }
            }
        }
    }

    /// Writes every receiver interleaved per instant (RX1, RX2, RX1...): ring sample `j` is receiver `j % n` of instant
    /// `j / n`. It writes the ring samples `from..` into `dst`, every byte of it.
    pub fn fill_interleaved(&self, raw: &[u8], dst: &mut [MaybeUninit<u8>], from: usize) {
        let n = self.pairs.len();
        let s = self.stride;
        let size = self.sample_bytes();
        for (k, d) in dst.chunks_exact_mut(size).enumerate() {
            let j = from + k;
            put(d, &self.one(&raw[j / n * s..], j % n)[..size]);
        }
    }
}

/// The samples of receiver `rx` of `raw` as complex samples, through the ring form (tests and simple callers).
pub fn convert_vec(raw: &[u8], l: &Layout, rx: usize) -> Vec<Complex32> {
    let n = l.instants(raw.len());
    let mut bytes: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); n * l.sample_bytes()];
    l.fill(raw, rx, &mut bytes, 0);
    // SAFETY: `fill` wrote every byte.
    let bytes: Vec<u8> = bytes
        .into_iter()
        .map(|b| unsafe { b.assume_init() })
        .collect();
    let mut out = vec![Complex32::default(); n];
    crate::sample::to_iq(l.sample_type(), &bytes, &mut out);
    out
}

/// How transmit samples are packed: the stride, the (I, Q) offsets, and the format of the DAC channels.
#[derive(Clone, Debug, PartialEq)]
pub struct TxLayout {
    /// Bytes per sample instant.
    pub stride: usize,
    /// Byte offsets of I and Q.
    pub pair: (usize, usize),
    /// The largest value (full scale), before the shift.
    pub full: f32,
    /// Left shift into the storage.
    pub shift: u32,
    /// Big endian.
    pub big: bool,
}

/// The transmit layout for the (I, Q) channels at positions `pair`, with the channels of `mask` enabled.
///
/// # Errors
///
/// [`LayoutError`] if the buffer does not carry a channel, or its storage size is not 16 bits.
pub fn tx_layout(
    dev: &Device,
    mask: &[u32],
    pair: (usize, usize),
) -> Result<TxLayout, LayoutError> {
    let (offs, stride) = offsets(dev, mask);
    let find = |pos: usize| {
        offs.iter()
            .find(|o| o.0 == pos)
            .map(|o| (o.1, o.2))
            .ok_or(LayoutError::NoChannel(pos))
    };
    let (oi, f) = find(pair.0)?;
    let (oq, _) = find(pair.1)?;
    if f.storage != 16 {
        return Err(LayoutError::Storage(f.storage));
    }
    Ok(TxLayout {
        stride,
        pair: (oi, oq),
        full: ((1u32 << (f.bits.clamp(2, 16) - 1)) - 1) as f32,
        shift: f.shift,
        big: f.big_endian,
    })
}

impl TxLayout {
    /// Packs `iq` (clipped to full scale) into `out`, which is resized to whole instants; other channels are zero.
    pub fn pack(&self, iq: &[Complex32], out: &mut Vec<u8>) {
        out.clear();
        out.resize(iq.len() * self.stride, 0);
        self.write(iq, out);
    }

    /// Writes `iq` (clipped to full scale) into this pair's channels of `out`, one sample per instant from the first instant. The
    /// other channels of `out` are not touched, so several outputs can share one buffer. Samples beyond the end of `out` are
    /// ignored.
    pub fn write(&self, iq: &[Complex32], out: &mut [u8]) {
        let q = |x: f32| -> [u8; 2] {
            let v = ((x.clamp(-1.0, 1.0) * self.full).round() as i32) << self.shift;
            let v = v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            if self.big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let (oi, oq) = self.pair;
        for (c, p) in iq.iter().zip(out.chunks_exact_mut(self.stride)) {
            p[oi..oi + 2].copy_from_slice(&q(c.re));
            p[oq..oq + 2].copy_from_slice(&q(c.im));
        }
    }

    /// Unpacks what [`TxLayout::pack`] made (tests, and a fake server that checks what it was sent).
    pub fn unpack(&self, raw: &[u8]) -> Vec<Complex32> {
        let (oi, oq) = self.pair;
        let v = |p: &[u8], at: usize| {
            let b = [p[at], p[at + 1]];
            let x = if self.big {
                i16::from_be_bytes(b)
            } else {
                i16::from_le_bytes(b)
            };
            ((x as i32) >> self.shift) as f32 / self.full
        };
        raw.chunks_exact(self.stride)
            .map(|p| Complex32::new(v(p, oi), v(p, oq)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::Context;
    use super::*;

    fn rx_dev(n: usize) -> Device {
        let ch: String = (0..n)
            .map(|i| format!("<channel id=\"voltage{i}\" type=\"input\"><scan-element index=\"{i}\" format=\"le:S12/16&gt;&gt;0\" /></channel>"))
            .collect();
        let x = format!("<context name=\"n\"><device id=\"iio:device4\" name=\"cf-ad9361-lpc\">{ch}</device></context>");
        Context::parse(&x).unwrap().devices.remove(0)
    }

    fn le(v: &[i16]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn one_receiver_and_a_superset_mask() {
        let d = rx_dev(4);
        let l = layout(&d, &[0b11], &[(0, 1)], false).unwrap();
        assert_eq!((l.stride, l.pairs.clone()), (4, vec![(0, 2)]));
        assert_eq!(
            convert_vec(&le(&[1024, -2048]), &l, 0),
            [Complex32::new(0.5, -1.0)]
        );
        // Another client enabled all four: RX2 is picked out of 8-byte instants.
        let l = layout(&d, &[0b1111], &[(2, 3)], false).unwrap();
        assert_eq!((l.stride, l.pairs.clone()), (8, vec![(4, 6)]));
        assert_eq!(
            convert_vec(&le(&[1, 2, 1024, 512]), &l, 0),
            [Complex32::new(0.5, 0.25)]
        );
        assert!(layout(&d, &[0b11], &[(2, 3)], false).is_err());
    }

    #[test]
    fn both_receivers_separate_and_interleaved() {
        let d = rx_dev(4);
        let l = layout(&d, &[0b1111], &[(0, 1), (2, 3)], false).unwrap();
        // A 12-bit value reaches -2048 but only +2047: +1024 is half scale.
        let raw = le(&[1024, 0, 0, 1024, -2048, 0, 0, -2048]);
        assert_eq!(
            convert_vec(&raw, &l, 0),
            [Complex32::new(0.5, 0.0), Complex32::new(-1.0, 0.0)]
        );
        assert_eq!(
            convert_vec(&raw, &l, 1),
            [Complex32::new(0.0, 0.5), Complex32::new(0.0, -1.0)]
        );
        let v = iq(&l, &raw, 3, 1);
        assert_eq!(
            v,
            [
                Complex32::new(0.0, 0.5),
                Complex32::new(-1.0, 0.0),
                Complex32::new(0.0, -1.0)
            ]
        );
        // Every split of a ring wrap produces the same samples.
        let raw: Vec<u8> = (0..400i16).flat_map(|x| x.to_le_bytes()).collect();
        let all = iq(&l, &raw, 200, 0);
        for cut in [0, 1, 2, 7, 199, 200] {
            let mut got = iq(&l, &raw, cut, 0);
            got.extend(iq(&l, &raw, 200 - cut, cut));
            assert_eq!(got, all, "cut at {cut}");
        }
        assert_eq!(all[3], Complex32::new(6.0 / 2048.0, 7.0 / 2048.0));
    }

    /// `n` interleaved ring samples from sample `from`, as complex samples.
    fn iq(l: &Layout, raw: &[u8], n: usize, from: usize) -> Vec<Complex32> {
        let mut b = vec![MaybeUninit::uninit(); n * l.sample_bytes()];
        l.fill_interleaved(raw, &mut b, from);
        // SAFETY: `fill_interleaved` wrote every byte.
        let b: Vec<u8> = b.into_iter().map(|x| unsafe { x.assume_init() }).collect();
        let mut out = vec![Complex32::default(); n];
        crate::sample::to_iq(l.sample_type(), &b, &mut out);
        out
    }

    #[test]
    fn cs8_and_transmit_packing() {
        let d = rx_dev(2);
        let l = layout(&d, &[0b1], &[(0, 0)], true).unwrap();
        assert_eq!(convert_vec(&[64, 0xC0], &l, 0), [Complex32::new(0.5, -0.5)]);
        let x = "<context name=\"n\"><device id=\"iio:device3\" name=\"cf-ad9361-dds-core-lpc\">\
                 <channel id=\"voltage0\" type=\"output\"><scan-element index=\"0\" format=\"le:S16/16&gt;&gt;0\" /></channel>\
                 <channel id=\"voltage1\" type=\"output\"><scan-element index=\"1\" format=\"le:S16/16&gt;&gt;0\" /></channel>\
                 <channel id=\"altvoltage0\" type=\"output\"></channel></device></context>";
        let dds = Context::parse(x).unwrap().devices.remove(0);
        let t = tx_layout(&dds, &[0b11], (0, 1)).unwrap();
        let mut out = Vec::new();
        t.pack(
            &[Complex32::new(1.0, -2.0), Complex32::new(0.5, 0.0)],
            &mut out,
        );
        assert_eq!(out, le(&[32767, -32767, 16384, 0]));
        let back = t.unpack(&out);
        assert!((back[1].re - 0.5).abs() < 1e-4 && back[0].im == -1.0);
    }
}
