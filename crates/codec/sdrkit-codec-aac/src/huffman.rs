//! The Huffman decoder of every code book of this crate: a book of code words and lengths by symbol becomes a lookup table of
//! two levels, read with one or two peeks of the bit reader.

use sdrkit_util_bits::BitReader;

/// Bits of the first lookup level.
const ROOT: u32 = 9;
/// Marks an entry that points to a second-level table.
const SUB: u32 = 1 << 31;

/// A decoding table. An entry holds the symbol in bits 0..16 and the code length in bits 16..21, or, with [`SUB`] set, the
/// offset of a second-level table in bits 0..24 and its width in bits 24..29.
#[derive(Debug)]
pub(crate) struct Book {
    lut: Vec<u32>,
}

impl Book {
    /// Builds the table of `n` symbols whose code word and length `code(i)` returns. The codes must be prefix-free and at
    /// most 24 bits long.
    pub(crate) fn new(n: usize, code: impl Fn(usize) -> (u32, u8)) -> Book {
        let root = 1usize << ROOT;
        // The widest code under each first-level prefix fixes the width of its second-level table.
        let mut extra = vec![0u32; root];
        for i in 0..n {
            let (c, l) = code(i);
            let l = u32::from(l);
            if l > ROOT {
                let p = (c >> (l - ROOT)) as usize;
                extra[p] = extra[p].max(l - ROOT);
            }
        }
        let mut lut = vec![0u32; root];
        for (p, &e) in extra.iter().enumerate() {
            if e > 0 {
                lut[p] = SUB | (e << 24) | lut.len() as u32;
                lut.resize(lut.len() + (1 << e), 0);
            }
        }
        for i in 0..n {
            let (c, l) = code(i);
            let l32 = u32::from(l);
            let leaf = i as u32 | (l32 << 16);
            if l32 <= ROOT {
                let first = (c << (ROOT - l32)) as usize;
                lut[first..first + (1 << (ROOT - l32))].fill(leaf);
            } else {
                let e = lut[(c >> (l32 - ROOT)) as usize];
                let width = (e >> 24) & 31;
                let base = (e & 0xFF_FFFF) as usize;
                let low = (c & ((1 << (l32 - ROOT)) - 1)) << (width - (l32 - ROOT));
                let first = base + low as usize;
                lut[first..first + (1 << (width - (l32 - ROOT)))].fill(leaf);
            }
        }
        Book { lut }
    }

    /// Reads one code word and returns its symbol, or `None` if the bits form no code word of the book.
    #[inline]
    pub(crate) fn read(&self, r: &mut BitReader<'_>) -> Option<usize> {
        let mut e = self.lut[r.peek(ROOT) as usize];
        if e & SUB != 0 {
            let width = (e >> 24) & 31;
            let low = r.peek(ROOT + width) as usize & ((1 << width) - 1);
            e = self.lut[(e & 0xFF_FFFF) as usize + low];
        }
        let len = (e >> 16) & 31;
        if len == 0 {
            return None;
        }
        r.skip(len as usize);
        Some((e & 0xFFFF) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{SCF_CODE, SCF_LEN};
    use sdrkit_util_bits::BitWriter;

    #[test]
    fn reads_back_every_code_word() {
        let book = Book::new(121, |i| (SCF_CODE[i], SCF_LEN[i]));
        let mut bytes = Vec::new();
        let mut w = BitWriter::new(&mut bytes);
        for i in 0..121 {
            w.put(u64::from(SCF_CODE[i]), u32::from(SCF_LEN[i]));
        }
        w.finish();
        let mut r = BitReader::new(&bytes);
        for i in 0..121 {
            assert_eq!(book.read(&mut r), Some(i));
        }
    }
}
