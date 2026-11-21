//! The decoder.

use crate::{Error, BLOCK_MAGIC, END_MAGIC, MAX_CODE_LEN, MAX_SELECTORS};
use sdrkit_util_bits::crc::catalog::CRC32_BZIP2;

/// Decodes every bzip2 stream in `data`, in order, and joins their output. Bytes before, between and after the streams are
/// skipped. It returns [`Error::NotBzip2`] when no stream starts in `data`, [`Error::TooLong`] when the output would pass
/// `max_out` bytes, and another [`Error`] for any damage: one damaged stream fails the whole buffer.
pub fn decompress(data: &[u8], max_out: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut pos = 0;
    let mut streams = 0;
    while let Some(start) = find_stream(data, pos) {
        pos = stream(data, start, &mut out, max_out)?;
        streams += 1;
    }
    if streams == 0 {
        return Err(Error::NotBzip2);
    }
    Ok(out)
}

/// The first "BZh" and level digit at or after `from`.
fn find_stream(data: &[u8], from: usize) -> Option<usize> {
    data.get(from..)?
        .windows(4)
        .position(|w| &w[..3] == b"BZh" && (b'1'..=b'9').contains(&w[3]))
        .map(|p| from + p)
}

/// A reader of bits, most significant bit first.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, Error> {
        let b = *self.data.get(self.pos >> 3).ok_or(Error::Truncated)?;
        let v = (b >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Ok(u32::from(v))
    }
    fn bits(&mut self, n: u32) -> Result<u64, Error> {
        let mut v = 0u64;
        for _ in 0..n {
            v = (v << 1) | u64::from(self.bit()?);
        }
        Ok(v)
    }
}

/// Decodes the stream at byte `start` into `out`. Returns the byte after its end.
fn stream(data: &[u8], start: usize, out: &mut Vec<u8>, max_out: usize) -> Result<usize, Error> {
    let level = usize::from(data[start + 3] - b'0');
    let max_block = level * 100_000;
    let mut r = Bits {
        data,
        pos: (start + 4) * 8,
    };
    let mut combined = 0u32;
    let mut block = Block::default();
    loop {
        match r.bits(48)? {
            BLOCK_MAGIC => {
                let crc = r.bits(32)? as u32;
                if r.bit()? == 1 {
                    return Err(Error::Randomised);
                }
                let origin = r.bits(24)? as usize;
                let first = out.len();
                block.decode(&mut r, max_block, origin, out, max_out)?;
                if CRC32_BZIP2.checksum(&out[first..]) != crc {
                    return Err(Error::BlockCrc);
                }
                combined = combined.rotate_left(1) ^ crc;
            }
            END_MAGIC => {
                if r.bits(32)? as u32 != combined {
                    return Err(Error::StreamCrc);
                }
                return Ok(r.pos.div_ceil(8));
            }
            _ => return Err(Error::BadMagic),
        }
    }
}

/// A canonical Huffman code of one table: codes are given in order of length, then of symbol.
#[derive(Default)]
struct Table {
    /// Symbols in code order.
    perm: Vec<u16>,
    /// Codes of each length.
    count: [u32; MAX_CODE_LEN as usize + 1],
    /// The first code of each length.
    first: [u32; MAX_CODE_LEN as usize + 1],
    /// The index in `perm` of the first code of each length.
    index: [u32; MAX_CODE_LEN as usize + 1],
}

impl Table {
    fn build(&mut self, lens: &[u8]) {
        self.count = [0; MAX_CODE_LEN as usize + 1];
        for &l in lens {
            self.count[usize::from(l)] += 1;
        }
        self.perm.clear();
        for l in 1..=MAX_CODE_LEN as u8 {
            self.perm.extend(
                lens.iter()
                    .enumerate()
                    .filter(|(_, &x)| x == l)
                    .map(|(s, _)| s as u16),
            );
        }
        let (mut code, mut idx) = (0u32, 0u32);
        for l in 1..=MAX_CODE_LEN as usize {
            self.first[l] = code;
            self.index[l] = idx;
            code = (code + self.count[l]) << 1;
            idx += self.count[l];
        }
    }
    fn decode(&self, r: &mut Bits) -> Result<u16, Error> {
        let mut code = 0u32;
        for l in 1..=MAX_CODE_LEN as usize {
            code = (code << 1) | r.bit()?;
            let k = code.wrapping_sub(self.first[l]);
            if code >= self.first[l] && k < self.count[l] {
                return Ok(self.perm[(self.index[l] + k) as usize]);
            }
        }
        Err(Error::Symbol)
    }
}

/// The working space of one block, kept between blocks.
#[derive(Default)]
struct Block {
    tables: Vec<Table>,
    selectors: Vec<u8>,
    tt: Vec<u32>,
}

impl Block {
    fn decode(
        &mut self,
        r: &mut Bits,
        max_block: usize,
        origin: usize,
        out: &mut Vec<u8>,
        max_out: usize,
    ) -> Result<(), Error> {
        // The bytes in use, in 16 groups of 16.
        let groups = r.bits(16)?;
        let mut used: Vec<u8> = Vec::with_capacity(256);
        for g in 0..16u32 {
            if groups >> (15 - g) & 1 == 1 {
                let b = r.bits(16)?;
                for k in 0..16u32 {
                    if b >> (15 - k) & 1 == 1 {
                        used.push((16 * g + k) as u8);
                    }
                }
            }
        }
        if used.is_empty() {
            return Err(Error::BlockSize);
        }
        let alpha = used.len() + 2;
        let n_tables = r.bits(3)? as usize;
        if !(2..=6).contains(&n_tables) {
            return Err(Error::Tables);
        }
        let n_sel = r.bits(15)? as usize;
        if n_sel == 0 {
            return Err(Error::Tables);
        }
        // Selectors, move-to-front coded in unary. Those beyond the most a block can use are read and dropped.
        let mut order: Vec<u8> = (0..n_tables as u8).collect();
        self.selectors.clear();
        for _ in 0..n_sel {
            let mut j = 0usize;
            while r.bit()? == 1 {
                j += 1;
                if j >= n_tables {
                    return Err(Error::Tables);
                }
            }
            let t = order[j];
            order.copy_within(0..j, 1);
            order[0] = t;
            if self.selectors.len() < MAX_SELECTORS {
                self.selectors.push(t);
            }
        }
        // Code lengths, delta coded.
        self.tables.resize_with(n_tables, Table::default);
        let mut lens = vec![0u8; alpha];
        for t in 0..n_tables {
            let mut cur = r.bits(5)? as i32;
            for l in lens.iter_mut() {
                loop {
                    if !(1..=MAX_CODE_LEN as i32).contains(&cur) {
                        return Err(Error::CodeLength);
                    }
                    if r.bit()? == 0 {
                        break;
                    }
                    cur += if r.bit()? == 0 { 1 } else { -1 };
                }
                *l = cur as u8;
            }
            self.tables[t].build(&lens);
        }
        // The symbols: move-to-front indices and runs.
        let eob = (alpha - 1) as u16;
        let mut mtf: Vec<u8> = (0..used.len()).map(|v| v as u8).collect();
        let mut counts = [0u32; 256];
        self.tt.clear();
        let mut group = 0usize;
        let mut left = 0usize;
        let mut table = 0usize;
        let mut next = |r: &mut Bits, tables: &[Table]| -> Result<u16, Error> {
            if left == 0 {
                table = usize::from(*self.selectors.get(group).ok_or(Error::Tables)?);
                group += 1;
                left = 50;
            }
            left -= 1;
            tables[table].decode(r)
        };
        let mut sym = next(r, &self.tables)?;
        loop {
            if sym == eob {
                break;
            }
            if sym <= 1 {
                // A run of the front byte: RUNA adds 1, RUNB adds 2, times the place value.
                let mut run = 0usize;
                let mut place = 1usize;
                while sym <= 1 {
                    run += place << sym;
                    place <<= 1;
                    if place > 1 << 21 {
                        return Err(Error::BlockSize);
                    }
                    sym = next(r, &self.tables)?;
                }
                let b = used[usize::from(mtf[0])];
                if self.tt.len() + run > max_block {
                    return Err(Error::BlockSize);
                }
                counts[usize::from(b)] += run as u32;
                self.tt.extend(std::iter::repeat_n(u32::from(b), run));
                continue;
            }
            let k = usize::from(sym - 1);
            if k >= used.len() {
                return Err(Error::Symbol);
            }
            let m = mtf[k];
            mtf.copy_within(0..k, 1);
            mtf[0] = m;
            let b = used[usize::from(m)];
            if self.tt.len() >= max_block {
                return Err(Error::BlockSize);
            }
            counts[usize::from(b)] += 1;
            self.tt.push(u32::from(b));
            sym = next(r, &self.tables)?;
        }
        let n = self.tt.len();
        if origin >= n {
            return Err(Error::Origin);
        }
        // The inverse Burrows-Wheeler transform: entry i keeps its byte in the low 8 bits and, in the high bits, the row that
        // follows it.
        let mut start = [0u32; 256];
        let mut sum = 0u32;
        for (s, c) in start.iter_mut().zip(counts) {
            *s = sum;
            sum += c;
        }
        for i in 0..n {
            let b = (self.tt[i] & 0xFF) as usize;
            let at = start[b] as usize;
            self.tt[at] |= (i as u32) << 8;
            start[b] += 1;
        }
        // The output, with the first run-length step undone: after four equal bytes, the next byte counts more of them.
        let mut pos = (self.tt[origin] >> 8) as usize;
        let mut last: Option<u8> = None;
        let mut same = 0u32;
        for _ in 0..n {
            let e = self.tt[pos];
            pos = (e >> 8) as usize;
            let b = (e & 0xFF) as u8;
            if same == 4 {
                if out.len() + usize::from(b) > max_out {
                    return Err(Error::TooLong);
                }
                if let Some(l) = last {
                    out.extend(std::iter::repeat_n(l, usize::from(b)));
                }
                same = 0;
                last = None;
                continue;
            }
            if last == Some(b) {
                same += 1;
            } else {
                last = Some(b);
                same = 1;
            }
            if out.len() >= max_out {
                return Err(Error::TooLong);
            }
            out.push(b);
        }
        Ok(())
    }
}
