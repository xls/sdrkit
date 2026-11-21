//! A FLAC writer and reader for 16-bit mono audio. The writer uses fixed predictors of order 0 to 4 with Rice-coded residuals, or a
//! verbatim frame when that is smaller, and writes a STREAMINFO and a VORBIS_COMMENT block.
//!
//! Standard: FLAC format specification (xiph.org).

use sdrkit_util_bits::crc::catalog::{CRC16_UMTS, CRC8_SMBUS};
use std::io::{Seek, SeekFrom, Write};

/// Samples per frame.
pub const BLOCK: usize = 4096;

struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn new() -> Self {
        Bits {
            out: Vec::with_capacity(BLOCK * 2 + 64),
            acc: 0,
            n: 0,
        }
    }
    fn empty() -> Self {
        Bits {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }
    #[inline]
    fn put(&mut self, v: u64, bits: u32) {
        debug_assert!(bits <= 32);
        if bits == 0 {
            return;
        }
        self.acc = (self.acc << bits) | (v & ((1u64 << bits) - 1));
        self.n += bits;
        while self.n >= 8 {
            self.n -= 8;
            self.out.push((self.acc >> self.n) as u8);
        }
    }
    fn unary(&mut self, q: u32) {
        let mut q = q;
        while q >= 32 {
            self.put(0, 32);
            q -= 32;
        }
        self.put(1, q + 1);
    }
    fn align(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put(0, pad);
        }
    }
}

fn utf8_number(mut v: u64, out: &mut Vec<u8>) {
    if v < 0x80 {
        out.push(v as u8);
        return;
    }
    let mut bytes = [0u8; 7];
    let mut len = 0;
    let mut first_bits = 6u32;
    while v >= (1u64 << first_bits) {
        bytes[len] = 0x80 | (v & 0x3f) as u8;
        len += 1;
        v >>= 6;
        first_bits -= 1;
    }
    let lead = (0xffu16 << (8 - (len + 1))) as u8 | v as u8;
    out.push(lead);
    out.extend(bytes[..len].iter().rev());
}

fn residual(x: &[i32], order: usize, r: &mut Vec<i32>) {
    r.clear();
    for i in order..x.len() {
        let p = match order {
            0 => 0,
            1 => x[i - 1],
            2 => 2 * x[i - 1] - x[i - 2],
            3 => 3 * x[i - 1] - 3 * x[i - 2] + x[i - 3],
            _ => 4 * x[i - 1] - 6 * x[i - 2] + 4 * x[i - 3] - x[i - 4],
        };
        r.push(x[i] - p);
    }
}

fn rice_cost(r: &[i32], k: u32) -> u64 {
    r.iter()
        .map(|&v| ((((v << 1) ^ (v >> 31)) as u32 >> k) as u64) + 1 + k as u64)
        .sum()
}

fn best_rice(r: &[i32]) -> (u32, u64) {
    if r.is_empty() {
        return (0, 0);
    }
    let mean = r.iter().map(|&v| v.unsigned_abs() as u64).sum::<u64>() / r.len() as u64;
    let guess = 64 - mean.max(1).leading_zeros();
    let mut best = (0u32, u64::MAX);
    for k in guess.saturating_sub(2)..=(guess + 1).min(14) {
        let c = rice_cost(r, k);
        if c < best.1 {
            best = (k, c);
        }
    }
    best
}

/// A FLAC stream being written.
pub struct FlacWriter<W: Write + Seek> {
    w: W,
    rate: u32,
    buf: Vec<i32>,
    res: Vec<i32>,
    frame: u64,
    total: u64,
    min_frame: u32,
    max_frame: u32,
    bits: Bits,
}

impl<W: Write + Seek> FlacWriter<W> {
    /// Starts a stream at `rate` with Vorbis comments (`TITLE=...`).
    pub fn new(mut w: W, rate: u32, tags: &[String]) -> std::io::Result<Self> {
        w.write_all(b"fLaC")?;
        // STREAMINFO (patched at the end).
        w.write_all(&[0x00, 0x00, 0x00, 34])?;
        w.write_all(&[0u8; 34])?;
        // VORBIS_COMMENT, the last metadata block.
        let vendor = b"sdrkit";
        let mut body = Vec::new();
        body.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        body.extend_from_slice(vendor);
        body.extend_from_slice(&(tags.len() as u32).to_le_bytes());
        for t in tags {
            body.extend_from_slice(&(t.len() as u32).to_le_bytes());
            body.extend_from_slice(t.as_bytes());
        }
        let len = body.len() as u32;
        w.write_all(&[0x80 | 4, (len >> 16) as u8, (len >> 8) as u8, len as u8])?;
        w.write_all(&body)?;
        let mut f = FlacWriter {
            w,
            rate,
            buf: Vec::with_capacity(BLOCK),
            res: Vec::with_capacity(BLOCK),
            frame: 0,
            total: 0,
            min_frame: u32::MAX,
            max_frame: 0,
            bits: Bits::new(),
        };
        f.write_streaminfo()?;
        Ok(f)
    }
    fn write_streaminfo(&mut self) -> std::io::Result<()> {
        let pos = self.w.stream_position()?;
        self.w.seek(SeekFrom::Start(8))?;
        let mut b = Bits::new();
        b.put(BLOCK as u64, 16);
        b.put(BLOCK as u64, 16);
        b.put(
            if self.min_frame == u32::MAX {
                0
            } else {
                self.min_frame
            } as u64,
            24,
        );
        b.put(self.max_frame as u64, 24);
        b.put(self.rate as u64, 20);
        b.put(0, 3);
        b.put(15, 5);
        b.put(self.total >> 32, 4);
        b.put(self.total & 0xffff_ffff, 32);
        for _ in 0..16 {
            b.put(0, 8);
        }
        self.w.write_all(&b.out)?;
        self.w.seek(SeekFrom::Start(pos))?;
        Ok(())
    }
    /// Appends samples given as 16-bit little-endian bytes. A last odd byte is ignored.
    pub fn push_le(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        for b in bytes.as_chunks::<2>().0 {
            self.buf.push(i32::from(i16::from_le_bytes([b[0], b[1]])));
            if self.buf.len() == BLOCK {
                self.encode_frame()?;
            }
        }
        Ok(())
    }
    fn encode_frame(&mut self) -> std::io::Result<()> {
        let n = self.buf.len();
        if n == 0 {
            return Ok(());
        }
        let mut b = std::mem::replace(&mut self.bits, Bits::empty());
        b.out.clear();
        b.acc = 0;
        b.n = 0;
        let hdr = &mut b.out;
        hdr.extend_from_slice(&[0xff, 0xf8]);
        let bs_code: u8 = if n == BLOCK { 0b1100 } else { 0b0111 };
        let sr_code: u8 = match self.rate {
            48_000 => 0b1010,
            44_100 => 0b1001,
            96_000 => 0b1011,
            _ => 0b0000,
        };
        hdr.push(bs_code << 4 | sr_code);
        hdr.push(0b100 << 1);
        utf8_number(self.frame, hdr);
        if bs_code == 0b0111 {
            hdr.extend_from_slice(&((n - 1) as u16).to_be_bytes());
        }
        let c = CRC8_SMBUS.checksum(hdr);
        hdr.push(c);
        // Pick the cheapest fixed order (or verbatim).
        let mut best: Option<(usize, u32, u64)> = None;
        for order in 0..=4usize.min(n.saturating_sub(1)) {
            residual(&self.buf, order, &mut self.res);
            let (k, cost) = best_rice(&self.res);
            let total = cost + 16 * order as u64 + 4 + 4 + 4;
            if best.is_none_or(|b| total < b.2) {
                best = Some((order, k, total));
            }
        }
        let verbatim = 16 * n as u64;
        match best {
            Some((order, k, cost)) if cost < verbatim => {
                b.put(0, 1);
                b.put(0b001000 | order as u64, 6);
                b.put(0, 1);
                for &v in &self.buf[..order] {
                    b.put(v as u16 as u64, 16);
                }
                residual(&self.buf, order, &mut self.res);
                b.put(0, 2); // Rice, 4-bit parameters
                b.put(0, 4); // partition order 0
                b.put(k as u64, 4);
                for &v in &self.res {
                    let u = ((v << 1) ^ (v >> 31)) as u32;
                    b.unary(u >> k);
                    b.put((u & ((1u32 << k) - 1)) as u64, k);
                }
            }
            _ => {
                b.put(0, 1);
                b.put(0b000001, 6);
                b.put(0, 1);
                for &v in &self.buf {
                    b.put(v as u16 as u64, 16);
                }
            }
        }
        b.align();
        let crc = CRC16_UMTS.checksum(&b.out);
        b.out.extend_from_slice(&crc.to_be_bytes());
        self.w.write_all(&b.out)?;
        let len = b.out.len() as u32;
        self.min_frame = self.min_frame.min(len);
        self.max_frame = self.max_frame.max(len);
        self.total += n as u64;
        self.frame += 1;
        self.buf.clear();
        self.bits = b;
        Ok(())
    }
    /// Writes the last frame and the final STREAMINFO.
    pub fn finish(mut self) -> std::io::Result<W> {
        self.encode_frame()?;
        self.write_streaminfo()?;
        self.w.flush()?;
        Ok(self.w)
    }
}

/// The rate, the channel count and the total sample count in the STREAMINFO of a FLAC file that starts with `bytes`.
pub fn streaminfo(bytes: &[u8]) -> Option<(u32, u16, u64)> {
    if bytes.len() < 42 || &bytes[..4] != b"fLaC" || bytes[4] & 0x7f != 0 {
        return None;
    }
    let si = &bytes[8..42];
    let rate = (u32::from(si[10]) << 12) | (u32::from(si[11]) << 4) | (u32::from(si[12]) >> 4);
    let channels = u16::from((si[12] >> 1) & 0x07) + 1;
    let hi = u64::from(si[13] & 0x0f);
    let lo = u64::from(u32::from_be_bytes([si[14], si[15], si[16], si[17]]));
    Some((rate, channels, hi << 32 | lo))
}

/// Decodes a FLAC file written by [`FlacWriter`] back to samples (fixed and verbatim subframes, mono 16-bit), for tests and tools.
/// Returns `None` for a stream it cannot read, or when the CRC-8 of a frame header or the CRC-16 of a frame is wrong (FLAC format
/// specification, "FRAME_HEADER" and "FRAME_FOOTER").
#[cfg(test)]
fn decode(bytes: &[u8]) -> Option<Vec<i16>> {
    struct R<'a> {
        b: &'a [u8],
        pos: usize,
    }
    impl R<'_> {
        fn get(&mut self, n: u32) -> Option<u64> {
            let mut v = 0u64;
            for _ in 0..n {
                let byte = *self.b.get(self.pos / 8)?;
                let bit = (byte >> (7 - self.pos % 8)) & 1;
                v = v << 1 | bit as u64;
                self.pos += 1;
            }
            Some(v)
        }
        fn unary(&mut self) -> Option<u32> {
            let mut q = 0;
            while self.get(1)? == 0 {
                q += 1;
            }
            Some(q)
        }
        fn align(&mut self) {
            self.pos = self.pos.div_ceil(8) * 8;
        }
    }
    let mut p = 4usize;
    loop {
        let h = *bytes.get(p)?;
        let len = u32::from_be_bytes([0, bytes[p + 1], bytes[p + 2], bytes[p + 3]]) as usize;
        p += 4 + len;
        if h & 0x80 != 0 {
            break;
        }
    }
    let mut out = Vec::new();
    let mut r = R {
        b: bytes,
        pos: p * 8,
    };
    while r.pos / 8 + 2 < bytes.len() {
        let start = r.pos / 8;
        if r.get(16)? != 0xfff8 {
            return None;
        }
        let bs = r.get(4)?;
        let _sr = r.get(4)?;
        r.get(8)?;
        // UTF-8 frame number.
        let lead = r.get(8)? as u8;
        let extra = lead.leading_ones().saturating_sub(1);
        r.get(8 * extra)?;
        let n = match bs {
            0b1100 => BLOCK,
            0b0111 => r.get(16)? as usize + 1,
            _ => return None,
        };
        let header_end = r.pos / 8;
        if r.get(8)? as u8 != CRC8_SMBUS.checksum(&bytes[start..header_end]) {
            return None;
        }
        r.get(1)?;
        let ty = r.get(6)?;
        r.get(1)?;
        let mut x: Vec<i32> = Vec::with_capacity(n);
        if ty == 1 {
            for _ in 0..n {
                x.push(r.get(16)? as u16 as i16 as i32);
            }
        } else if ty & 0b111000 == 0b001000 {
            let order = (ty & 7) as usize;
            for _ in 0..order {
                x.push(r.get(16)? as u16 as i16 as i32);
            }
            r.get(2)?;
            r.get(4)?;
            let k = r.get(4)? as u32;
            for i in order..n {
                let q = r.unary()?;
                let u = (q << k) | r.get(k)? as u32;
                let res = ((u >> 1) as i32) ^ -((u & 1) as i32);
                let pr = match order {
                    0 => 0,
                    1 => x[i - 1],
                    2 => 2 * x[i - 1] - x[i - 2],
                    3 => 3 * x[i - 1] - 3 * x[i - 2] + x[i - 3],
                    _ => 4 * x[i - 1] - 6 * x[i - 2] + 4 * x[i - 3] - x[i - 4],
                };
                x.push(pr + res);
            }
        } else {
            return None;
        }
        r.align();
        let end = r.pos / 8;
        let crc = r.get(16)? as u16;
        if CRC16_UMTS.checksum(&bytes[start..end]) != crc {
            return None;
        }
        out.extend(x.iter().map(|&v| v as i16));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_trip_is_lossless() {
        let x: Vec<i16> = (0..10_000)
            .map(|i| ((i as f32 * 0.05).sin() * 8000.0 + ((i * 7919) % 101) as f32) as i16)
            .collect();
        let mut f = FlacWriter::new(
            std::io::Cursor::new(Vec::new()),
            48_000,
            &["TITLE=test".into()],
        )
        .unwrap();
        let le: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
        f.push_le(&le).unwrap();
        let bytes = f.finish().unwrap().into_inner();
        assert_eq!(&bytes[..4], b"fLaC");
        assert_eq!(streaminfo(&bytes), Some((48_000, 1, 10_000)));
        assert_eq!(decode(&bytes).unwrap(), x);
        assert!(bytes.len() < x.len() * 2);
    }

    /// The end of the metadata blocks of a FLAC stream: where the first frame starts.
    fn first_frame(bytes: &[u8]) -> usize {
        let mut p = 4;
        loop {
            let h = bytes[p];
            let len = u32::from_be_bytes([0, bytes[p + 1], bytes[p + 2], bytes[p + 3]]) as usize;
            p += 4 + len;
            if h & 0x80 != 0 {
                return p;
            }
        }
    }

    #[test]
    fn a_frame_whose_header_crc8_is_wrong_is_refused() {
        let x: Vec<u8> = (0..1000)
            .flat_map(|i| (((i as f32 * 0.05).sin() * 8000.0) as i16).to_le_bytes())
            .collect();
        let mut f = FlacWriter::new(std::io::Cursor::new(Vec::new()), 48_000, &[]).unwrap();
        f.push_le(&x).unwrap();
        let mut bytes = f.finish().unwrap().into_inner();
        assert_eq!(decode(&bytes).unwrap().len(), 1000);
        // One frame of 1000 samples: sync (2 bytes), block size and rate (1), channels and bits (1), frame number (1),
        // block size - 1 (2), then the CRC-8 of those 7 bytes.
        let start = first_frame(&bytes);
        assert_eq!((bytes[start], bytes[start + 1]), (0xFF, 0xF8));
        assert_eq!(
            bytes[start + 7],
            CRC8_SMBUS.checksum(&bytes[start..start + 7])
        );
        bytes[start + 7] ^= 0x01;
        let end = bytes.len() - 2;
        let crc = CRC16_UMTS.checksum(&bytes[start..end]);
        bytes[end..].copy_from_slice(&crc.to_be_bytes());
        assert!(decode(&bytes).is_none());
    }

    #[test]
    fn the_crcs_have_their_check_values() {
        assert_eq!(CRC8_SMBUS.checksum(b"123456789"), 0xF4);
        assert_eq!(CRC16_UMTS.checksum(b"123456789"), 0xFEE8);
    }
}
