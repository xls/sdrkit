//! The entropy layer: Huffman tables, the bit reader and writer of entropy-coded segments, and the coding of one block of
//! quantised coefficients.
//!
//! Standard: ITU-T T.81 Annex C, clauses F.1.2 and F.2.2.

use super::JpegError;

/// A Huffman table built from the counts of codes of each length (BITS) and the symbols in code order (HUFFVAL), as T.81
/// Annex C builds it. It holds the code of each symbol for encoding, and a 9-bit look-up plus the MAXCODE procedure of
/// Figure F.16 for decoding. It allocates nothing once made.
#[derive(Clone, Debug)]
pub struct HuffmanTable {
    // (code, length) of each symbol. The length is 0 when the symbol has no code.
    codes: [(u16, u8); 256],
    // For each 9-bit prefix: symbol << 8 | length, or 0 when the code is longer than 9 bits.
    fast: [u16; 1 << FAST_BITS],
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: [u8; 256],
}

const FAST_BITS: u32 = 9;

impl HuffmanTable {
    /// Builds the table. If the counts hold more than 256 codes, `vals` is shorter than the count, or the codes do not fit their
    /// lengths (Annex C), it returns [`JpegError::BadTable`].
    pub fn new(bits: &[u8; 16], vals: &[u8]) -> Result<HuffmanTable, JpegError> {
        let total: usize = bits.iter().map(|&b| b as usize).sum();
        if total > 256 || vals.len() < total {
            return Err(JpegError::BadTable);
        }
        let mut t = HuffmanTable {
            codes: [(0, 0); 256],
            fast: [0; 1 << FAST_BITS],
            maxcode: [-1; 18],
            valptr: [0; 17],
            mincode: [0; 17],
            vals: [0; 256],
        };
        t.vals[..total].copy_from_slice(&vals[..total]);
        let mut code: u32 = 0;
        let mut k = 0usize;
        for len in 1..=16usize {
            let n = bits[len - 1] as usize;
            t.valptr[len] = k as i32;
            t.mincode[len] = code as i32;
            for _ in 0..n {
                if code >= 1 << len {
                    return Err(JpegError::BadTable);
                }
                let sym = vals[k];
                t.codes[sym as usize] = (code as u16, len as u8);
                if len as u32 <= FAST_BITS {
                    let shift = FAST_BITS - len as u32;
                    let first = (code << shift) as usize;
                    for e in &mut t.fast[first..first + (1 << shift)] {
                        *e = (sym as u16) << 8 | len as u16;
                    }
                }
                code += 1;
                k += 1;
            }
            t.maxcode[len] = if n > 0 { code as i32 - 1 } else { -1 };
            code <<= 1;
        }
        t.maxcode[17] = i32::MAX;
        Ok(t)
    }

    /// The code and its length of symbol `s`. The length is 0 when the table has no code for it.
    pub fn code(&self, s: u8) -> (u16, u8) {
        self.codes[s as usize]
    }

    /// Decodes one symbol (T.81 Figure F.16).
    #[inline]
    pub fn decode(&self, r: &mut EntropyReader) -> Result<u8, JpegError> {
        let peek = r.peek(FAST_BITS);
        let e = self.fast[peek as usize];
        if e != 0 {
            r.consume((e & 0xFF) as u32);
            return r.checked(e >> 8).map(|s| s as u8);
        }
        for len in FAST_BITS as usize + 1..=16 {
            let code = r.peek(len as u32) as i32;
            if self.maxcode[len] >= 0 && code <= self.maxcode[len] {
                r.consume(len as u32);
                let i = (self.valptr[len] + code - self.mincode[len]) as usize;
                return r.checked(self.vals[i] as u16).map(|s| s as u8);
            }
        }
        Err(if r.overrun() {
            JpegError::Truncated
        } else {
            JpegError::BadCode
        })
    }
}

/// Reads the bits of an entropy-coded segment, most significant bit first.
///
/// With `stuffed`, a 0x00 after each 0xFF is removed (T.81 F.1.2.3), and the reader stops at a marker (0xFF and a byte that is
/// not 0x00). Past the end or a marker it reads 0 bits and sets [`EntropyReader::overrun`] once such a bit is used. Without
/// `stuffed` it reads plain bytes (an SSDV or Meteor packet). It allocates nothing.
#[derive(Clone, Debug)]
pub struct EntropyReader<'a> {
    data: &'a [u8],
    pos: usize,
    // Bytes of data put in `acc`, after stuffing is removed.
    loaded: u64,
    acc: u64,
    count: u32,
    // Bits in `acc` that are padding after the end or a marker.
    padding: u32,
    stuffed: bool,
    marker: Option<u8>,
    overrun: bool,
}

impl<'a> EntropyReader<'a> {
    /// A reader of `data` from its first byte.
    pub fn new(data: &'a [u8], stuffed: bool) -> EntropyReader<'a> {
        EntropyReader {
            data,
            pos: 0,
            loaded: 0,
            acc: 0,
            count: 0,
            padding: 0,
            stuffed,
            marker: None,
            overrun: false,
        }
    }

    fn fill(&mut self) {
        while self.count <= 56 {
            let mut byte = 0u8;
            let mut real = false;
            if self.marker.is_none() && self.pos < self.data.len() {
                let b = self.data[self.pos];
                if self.stuffed && b == 0xFF {
                    match self.data.get(self.pos + 1) {
                        Some(0x00) => {
                            byte = 0xFF;
                            real = true;
                            self.pos += 2;
                        }
                        Some(&m) => self.marker = Some(m),
                        None => self.pos += 1,
                    }
                } else {
                    byte = b;
                    real = true;
                    self.pos += 1;
                }
            }
            self.acc |= (byte as u64) << (56 - self.count);
            self.count += 8;
            if real {
                self.loaded += 1;
            } else {
                self.padding += 8;
            }
        }
    }

    /// The next `n` bits (1 to 16) without using them.
    #[inline]
    pub fn peek(&mut self, n: u32) -> u32 {
        if self.count < n {
            self.fill();
        }
        (self.acc >> (64 - n)) as u32
    }

    /// Uses `n` bits (at most 16, after a peek of as many).
    #[inline]
    pub fn consume(&mut self, n: u32) {
        let n = n.min(self.count);
        self.acc <<= n;
        self.count -= n;
        if self.count < self.padding {
            self.overrun = true;
            self.padding = self.count;
        }
    }

    fn checked(&self, v: u16) -> Result<u16, JpegError> {
        if self.overrun {
            Err(JpegError::Truncated)
        } else {
            Ok(v)
        }
    }

    /// The next `n` bits (0 to 16) as a number.
    #[inline]
    pub fn bits(&mut self, n: u32) -> Result<u32, JpegError> {
        if n == 0 {
            return Ok(0);
        }
        let v = self.peek(n);
        self.consume(n);
        if self.overrun {
            Err(JpegError::Truncated)
        } else {
            Ok(v)
        }
    }

    /// True once a bit past the end of the data or past a marker has been used.
    pub fn overrun(&self) -> bool {
        self.overrun
    }

    /// The marker the reader stopped at, if any (the byte after 0xFF).
    pub fn marker(&self) -> Option<u8> {
        self.marker
    }

    /// The byte position of the next unread byte, or of the marker the reader stopped at.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// The position of the next bit to use, in bits from the start of the data. With `stuffed`, the count leaves out the 0x00
    /// after each 0xFF. Bits used past the end or past a marker do not count. Without `stuffed` and with no restart,
    /// `bit_position() / 8` is the index in the data of the byte that holds the next bit.
    pub fn bit_position(&self) -> u64 {
        8 * self.loaded - u64::from(self.count - self.padding)
    }

    /// Drops the bits left in the current byte, so the next bit is the first bit of the next byte. If the position is already
    /// at the start of a byte, it does nothing. A packet format whose units start on a byte (SSDV) uses it.
    pub fn skip_to_byte(&mut self) {
        let n = (self.count - self.padding) % 8;
        self.acc <<= n;
        self.count -= n;
    }

    /// Drops the bits left in the byte and moves past a restart marker RSTm (T.81 F.1.2.3, E.1.4). It returns the marker's
    /// number m (0 to 7). If the next marker is not a restart marker, it returns [`JpegError::BadMarker`].
    pub fn restart(&mut self) -> Result<u8, JpegError> {
        self.acc = 0;
        self.count = 0;
        self.padding = 0;
        if self.marker.is_none() {
            // Skip to the next marker (the encoder pads with 1 bits, so the rest of the byte is dropped).
            while self.pos + 1 < self.data.len()
                && !(self.data[self.pos] == 0xFF && self.data[self.pos + 1] != 0x00)
            {
                self.pos += 1;
            }
            self.marker = self.data.get(self.pos + 1).copied();
        }
        match self.marker {
            Some(m @ 0xD0..=0xD7) => {
                self.pos += 2;
                self.marker = None;
                self.overrun = false;
                Ok(m - 0xD0)
            }
            _ => Err(JpegError::BadMarker),
        }
    }
}

/// Writes the bits of an entropy-coded segment, most significant bit first. With `stuffed`, a 0x00 follows each 0xFF (T.81
/// F.1.2.3). It appends to a vector the caller owns, so it allocates nothing once the vector has room.
#[derive(Debug)]
pub struct EntropyWriter<'a> {
    out: &'a mut Vec<u8>,
    acc: u32,
    count: u32,
    stuffed: bool,
}

impl<'a> EntropyWriter<'a> {
    /// A writer that appends to `out`.
    pub fn new(out: &'a mut Vec<u8>, stuffed: bool) -> EntropyWriter<'a> {
        EntropyWriter {
            out,
            acc: 0,
            count: 0,
            stuffed,
        }
    }

    /// Writes the low `len` bits (0 to 16) of `v`.
    #[inline]
    pub fn put(&mut self, v: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (v & ((1u32 << len) - 1));
        self.count += len;
        while self.count >= 8 {
            self.count -= 8;
            let b = (self.acc >> self.count) as u8;
            self.out.push(b);
            if self.stuffed && b == 0xFF {
                self.out.push(0);
            }
        }
        self.acc &= (1u32 << self.count) - 1;
    }

    /// Pads the last byte with 1 bits (T.81 F.1.2.3).
    pub fn align(&mut self) {
        if self.count > 0 {
            let n = 8 - self.count;
            self.put((1 << n) - 1, n);
        }
    }

    /// The bits written so far.
    pub fn bit_len(&self) -> usize {
        self.out.len() * 8 + self.count as usize
    }
}

/// The size category of a value: the number of bits of its magnitude (T.81 Tables F.1 and F.2).
#[inline]
pub fn category(v: i32) -> u32 {
    32 - v.unsigned_abs().leading_zeros()
}

/// The additional bits of `v` in category `size`: the value, or for a negative value the value minus 1, in `size` bits.
#[inline]
fn magnitude_bits(v: i32, size: u32) -> u32 {
    if v < 0 {
        (v - 1) as u32 & ((1u32 << size) - 1)
    } else {
        v as u32
    }
}

/// The value of `size` additional bits `v` (T.81 Figure F.12, EXTEND).
#[inline]
pub fn extend(v: u32, size: u32) -> i32 {
    if size == 0 {
        0
    } else if v < 1 << (size - 1) {
        v as i32 - (1i32 << size) + 1
    } else {
        v as i32
    }
}

/// Decodes one block of quantised coefficients into `coefs`, in zig-zag order (T.81 F.2.2.1 and F.2.2.2). `pred` is the DC
/// predictor of the component: the decoded DC difference is added to it. It allocates nothing.
pub fn decode_block(
    r: &mut EntropyReader,
    dc: &HuffmanTable,
    ac: &HuffmanTable,
    pred: &mut i32,
    coefs: &mut [i16; 64],
) -> Result<(), JpegError> {
    *coefs = [0; 64];
    let s = dc.decode(r)? as u32;
    if s > 11 {
        return Err(JpegError::BadCode);
    }
    let diff = extend(r.bits(s)?, s);
    *pred = pred.wrapping_add(diff);
    coefs[0] = (*pred).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let mut k = 1usize;
    while k < 64 {
        let rs = ac.decode(r)?;
        let (run, size) = ((rs >> 4) as usize, (rs & 15) as u32);
        if size == 0 {
            if run == 15 {
                // ZRL: sixteen zeros.
                k += 16;
                if k > 64 {
                    return Err(JpegError::Overrun);
                }
                continue;
            }
            break; // EOB
        }
        k += run;
        if k > 63 || size > 10 {
            return Err(JpegError::Overrun);
        }
        coefs[k] = extend(r.bits(size)?, size) as i16;
        k += 1;
    }
    Ok(())
}

/// Encodes one block of quantised coefficients, given in zig-zag order (T.81 F.1.2.1 and F.1.2.2). `pred` is the DC
/// predictor of the component: it becomes this block's DC. If a value is too large for its category (DC above 11 bits, AC
/// above 10 bits) or a table has no code for a symbol, it returns an error and writes a part of the block.
pub fn encode_block(
    w: &mut EntropyWriter,
    dc: &HuffmanTable,
    ac: &HuffmanTable,
    pred: &mut i32,
    coefs: &[i16; 64],
) -> Result<(), JpegError> {
    let diff = coefs[0] as i32 - *pred;
    *pred = coefs[0] as i32;
    let size = category(diff);
    if size > 11 {
        return Err(JpegError::ValueTooLarge);
    }
    put_code(w, dc, size as u8)?;
    w.put(magnitude_bits(diff, size), size);
    let mut run = 0u32;
    for &v in &coefs[1..] {
        if v == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            put_code(w, ac, 0xF0)?;
            run -= 16;
        }
        let size = category(v as i32);
        if size > 10 {
            return Err(JpegError::ValueTooLarge);
        }
        put_code(w, ac, (run << 4) as u8 | size as u8)?;
        w.put(magnitude_bits(v as i32, size), size);
        run = 0;
    }
    if run > 0 {
        put_code(w, ac, 0x00)?;
    }
    Ok(())
}

#[inline]
fn put_code(w: &mut EntropyWriter, t: &HuffmanTable, sym: u8) -> Result<(), JpegError> {
    let (code, len) = t.code(sym);
    if len == 0 {
        return Err(JpegError::BadCode);
    }
    w.put(code as u32, len as u32);
    Ok(())
}
