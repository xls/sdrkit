//! The VDL Mode 2 burst: the training sequence, the D8PSK phase steps, the (25, 20) header code, the scrambler, the
//! Reed-Solomon layout and interleaving, and the bits of a burst both ways. Bits are `u8`, one per bit, 0 or 1.
//! Standard: ICAO Doc 9776 clauses 6.2 (modulation), 6.3 (header, FEC, interleaving, scrambling).

use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq;

/// Symbols per second.
pub const SYMBOL_RATE: f64 = 10_500.0;

/// The training sequence, phases in units of pi/4 relative to the first.
pub const TRAINING: [i32; 16] = [0, 3, -3, 1, 1, 2, 0, 4, -3, 4, -2, 3, 1, -2, -3, 0];

/// The longest Reed-Solomon block of data octets.
pub const RS_BLOCK: usize = 249;

/// The phase step (units of pi/4) of three bits, first sent most significant (Doc 9776 table 6-1).
pub fn step_of(bits: u8) -> u8 {
    [0, 1, 3, 2, 7, 6, 4, 5][bits as usize & 7]
}

/// The three bits of a phase step.
pub fn bits_of(step: u8) -> u8 {
    [0b000, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100][step as usize & 7]
}

/// The header parity rows over the 25-bit word (Doc 9776 clause 6.3.2): bit `j` of a row is the `j`th bit sent.
pub const HEADER_ROWS: [&str; 5] = [
    "0000000011111111111110000",
    "0011111100001111111101000",
    "1100011100110000111100100",
    "1101101101010011001100010",
    "0110100111100101010100001",
];

const fn row_masks() -> [u32; 5] {
    let mut m = [0u32; 5];
    let mut i = 0;
    while i < 5 {
        let b = HEADER_ROWS[i].as_bytes();
        let mut j = 0;
        while j < 25 {
            if b[j] == b'1' {
                m[i] |= 1 << j;
            }
            j += 1;
        }
        i += 1;
    }
    m
}

/// The parity rows as masks: bit `j` is the `j`th bit sent.
const ROWS: [u32; 5] = row_masks();

fn word_of(bits: &[u8]) -> u32 {
    bits.iter()
        .take(25)
        .enumerate()
        .fold(0, |w, (j, &b)| w | (((b & 1) as u32) << j))
}

fn syndrome(w: u32) -> u8 {
    let data = w & !0b111;
    ROWS.iter().enumerate().fold(0, |s, (i, r)| {
        s | ((((data & r).count_ones() & 1) as u8) << i)
    })
}

/// Appends the 25 header bits of a transmission length (the three reserved bits zero).
pub fn header_bits(len: u32, out: &mut Vec<u8>) {
    let mut w = (len & 0x1_FFFF) << 3;
    for (i, r) in ROWS.iter().enumerate() {
        let p = (w & r & 0x000F_FFFF).count_ones() & 1;
        w |= p << (20 + i);
    }
    out.extend((0..25).map(|j| ((w >> j) & 1) as u8));
}

/// Reads a header from its first 25 bits: the length and whether it needed correcting. `None` when it is refused: a syndrome
/// that no single bit explains, a zero length, or a corrected length over 0x1FFF.
pub fn read_header(bits: &[u8]) -> Option<(u32, bool)> {
    if bits.len() < 25 {
        return None;
    }
    let mut w = word_of(bits);
    let s = syndrome(w);
    let mut corrected = false;
    if s != 0 {
        let col =
            (3..25).find(|&j| (0..5).all(|i| ((ROWS[i] >> j) & 1 != 0) == ((s >> i) & 1 != 0)))?;
        w ^= 1 << col;
        corrected = true;
    }
    let len = (w >> 3) & 0x1_FFFF;
    let max = if corrected { 0x1FFF } else { 0x3FFF };
    (len != 0 && len <= max).then_some((len, corrected))
}

/// The scrambler: x^15 + x + 1 from its seed at the first header bit (Doc 9776 clause 6.3.5).
pub fn scrambler() -> Lfsr {
    seq::VDL2
}

/// The Reed-Solomon layout of `data` octets: each block's data octets and parity octets (Doc 9776 clause 6.3.3). Calls `f`
/// once for each block.
pub fn layout(data: usize, mut f: impl FnMut(usize, usize)) {
    if data == 0 {
        return;
    }
    let nb = data.div_ceil(RS_BLOCK);
    for b in 0..nb {
        let len = if b + 1 < nb {
            RS_BLOCK
        } else {
            data - RS_BLOCK * (nb - 1)
        };
        let par = match len {
            RS_BLOCK => 6,
            0..=2 => 0,
            3..=30 => 2,
            31..=67 => 4,
            _ => 6,
        };
        f(len, par);
    }
}

/// The FEC octets of a burst of `data` octets.
pub fn fec_octets(data: usize) -> usize {
    let mut n = 0;
    layout(data, |_, p| n += p);
    n
}

/// The number of symbols after the training sequence for a transmission length.
pub fn symbols_for(len: u32) -> usize {
    let nd = (len as usize).div_ceil(8);
    (25 + 8 * (nd + fec_octets(nd))).div_ceil(3)
}

/// The Reed-Solomon codes of VDL Mode 2 for every block length, made once and kept.
///
/// A block of `l` data octets is a code word of the shortened (255, 249) code, field polynomial 0x187. The first octet sent is
/// the coefficient of x^0, so the code word is read backwards: its roots are then alpha^130 to alpha^135. The parity octets
/// that a short block does not send are decoded as erasures (Doc 9776 clause 6.3.3).
pub struct Codec {
    codes: Vec<Option<Rs>>,
    word: Vec<u8>,
    erasures: Vec<u16>,
}

impl Default for Codec {
    fn default() -> Self {
        Codec {
            codes: (0..=255).map(|_| None).collect(),
            word: Vec::with_capacity(255),
            erasures: Vec::with_capacity(6),
        }
    }
}

impl Codec {
    fn code(&mut self, data: usize) -> Option<&mut Rs> {
        let n = data + 6;
        if !(7..=255).contains(&n) {
            return None;
        }
        let slot = &mut self.codes[n];
        if slot.is_none() {
            *slot = Rs::new(codes::vdl2(n as u16)).ok();
        }
        slot.as_mut()
    }

    /// Writes the six parity octets of a block of `data` (1 to 249 octets) into `parity`, in the order they are sent. `false`
    /// for a block of another length.
    pub fn parity(&mut self, data: &[u8], parity: &mut [u8; 6]) -> bool {
        let n = data.len();
        let Some(rs) = self.code(n) else {
            return false;
        };
        let rev: Vec<u8> = data.iter().rev().copied().collect();
        let mut p = [0u8; 6];
        if rs.encode(&rev, &mut p).is_err() {
            return false;
        }
        for (o, v) in parity.iter_mut().zip(p.iter().rev()) {
            *o = *v;
        }
        true
    }

    /// Corrects a block in place: `data` and the `sent` parity octets (the first `sent.len()` of the six). `extra` lists more
    /// data octets known to be unreliable (indices into `data`). Returns the octets corrected, or `None` when the block cannot be
    /// corrected, the indices are out of range, or more than six octets are erased. Allocates nothing once each length was
    /// seen.
    pub fn correct(&mut self, data: &mut [u8], sent: &[u8], extra: &[usize]) -> Option<usize> {
        let l = data.len();
        let p = sent.len();
        if p > 6 || extra.iter().any(|&e| e >= l) {
            return None;
        }
        let mut word = std::mem::take(&mut self.word);
        let mut er = std::mem::take(&mut self.erasures);
        word.clear();
        word.extend(data.iter().rev());
        word.extend((0..6 - p).map(|_| 0u8));
        word.extend(sent.iter().rev());
        er.clear();
        // The parity octets not sent are the last ones of the six: in the reversed word they come right after the data.
        er.extend((0..6 - p).map(|j| (l + j) as u16));
        er.extend(extra.iter().map(|&e| (l - 1 - e) as u16));
        let r = if er.len() > 6 {
            None
        } else {
            self.code(l).and_then(|rs| rs.decode(&mut word, &er).ok())
        };
        // The octets corrected: those of the data and of the sent parity (the parity not sent is filled in, not corrected).
        let out = r.map(|_| {
            let mut n = 0;
            for (i, v) in word[..l].iter().rev().enumerate() {
                n += usize::from(data[i] != *v);
                data[i] = *v;
            }
            n + sent
                .iter()
                .zip(word[l + 6 - p..].iter().rev())
                .filter(|(a, b)| a != b)
                .count()
        });
        self.word = word;
        self.erasures = er;
        out
    }
}

/// Appends the bits of a burst to `out` for the HDLC bit stream `hdlc` (flags and frames): the header, then the interleaved data
/// and FEC octets (each least significant bit first), padded to whole symbols and scrambled.
pub fn burst_bits(codec: &mut Codec, hdlc: &[u8], out: &mut Vec<u8>) {
    let len = hdlc.len() as u32;
    let nd = hdlc.len().div_ceil(8);
    let mut data = vec![0u8; nd];
    for (i, &b) in hdlc.iter().enumerate() {
        data[i / 8] |= (b & 1) << (i % 8);
    }
    let mut blocks: Vec<(usize, usize, [u8; 6])> = Vec::new();
    let mut at = 0;
    layout(nd, |l, p| {
        let mut par = [0u8; 6];
        codec.parity(&data[at..at + l], &mut par);
        blocks.push((at, l, par));
        let _ = p;
        at += l;
    });
    let mut pars = Vec::new();
    layout(nd, |_, p| pars.push(p));
    let start = out.len();
    header_bits(len, out);
    let octet = |o: u8, out: &mut Vec<u8>| out.extend((0..8).map(|i| (o >> i) & 1));
    for c in 0..RS_BLOCK {
        for &(a, l, _) in &blocks {
            if c < l {
                octet(data[a + c], out);
            }
        }
    }
    for j in 0..6 {
        for (b, &(_, _, par)) in blocks.iter().enumerate() {
            if j < pars[b] {
                octet(par[j], out);
            }
        }
    }
    while !(out.len() - start).is_multiple_of(3) {
        out.push(0);
    }
    let mut s = scrambler();
    for b in out[start..].iter_mut() {
        *b ^= s.bit();
    }
}

/// The result of decoding the body of a burst.
#[derive(Clone, Debug, Default)]
pub struct Body {
    /// The HDLC bits (`length` of them).
    pub hdlc: Vec<u8>,
    /// Octets corrected by Reed-Solomon.
    pub corrected: usize,
    /// A Reed-Solomon block failed.
    pub failed: bool,
}

/// The body decoder: the codes and the buffers, kept from burst to burst.
#[derive(Default)]
pub struct BodyDecoder {
    codec: Codec,
    octets: Vec<u8>,
    data: Vec<u8>,
    parity: Vec<u8>,
    blocks: Vec<(usize, usize)>,
}

impl BodyDecoder {
    /// Decodes the descrambled bits after the header (data then FEC octets, interleaved) of a transmission of `len` bits into
    /// `out`. Allocates nothing once its buffers have grown to the burst length.
    pub fn decode(&mut self, bits: &[u8], len: u32, out: &mut Body) {
        out.hdlc.clear();
        out.corrected = 0;
        out.failed = false;
        let nd = (len as usize).div_ceil(8);
        self.blocks.clear();
        layout(nd, |l, p| self.blocks.push((l, p)));
        let nf: usize = self.blocks.iter().map(|b| b.1).sum();
        self.octets.clear();
        self.octets.extend((0..nd + nf).map(|k| {
            (0..8).fold(0u8, |o, i| {
                o | (bits.get(8 * k + i).copied().unwrap_or(0) & 1) << i
            })
        }));
        // De-interleave: data octet c of each block in turn, then parity octet j of each block in turn.
        self.data.clear();
        self.data.resize(nd, 0);
        self.parity.clear();
        self.parity.resize(6 * self.blocks.len(), 0);
        let mut k = 0;
        for c in 0..RS_BLOCK {
            let mut at = 0;
            for &(l, _) in &self.blocks {
                if c < l {
                    self.data[at + c] = self.octets[k];
                    k += 1;
                }
                at += l;
            }
        }
        for j in 0..6 {
            for (b, &(_, p)) in self.blocks.iter().enumerate() {
                if j < p {
                    self.parity[6 * b + j] = self.octets[k];
                    k += 1;
                }
            }
        }
        let mut at = 0;
        for (b, &(l, p)) in self.blocks.iter().enumerate() {
            if p > 0 {
                let sent = &self.parity[6 * b..6 * b + p];
                match self.codec.correct(&mut self.data[at..at + l], sent, &[]) {
                    Some(n) => out.corrected += n,
                    None => out.failed = true,
                }
            }
            at += l;
        }
        out.hdlc
            .extend((0..len as usize).map(|i| (self.data[i / 8] >> (i % 8)) & 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_single_header_error_is_corrected() {
        for len in [1u32, 100, 0x1FFF, 5000] {
            let mut w = Vec::new();
            header_bits(len, &mut w);
            assert_eq!(read_header(&w), Some((len, false)));
            for j in 3..25 {
                let mut x = w.clone();
                x[j] ^= 1;
                assert_eq!(read_header(&x), Some((len, true)), "len {len} bit {j}");
            }
            for j in 0..3 {
                let mut x = w.clone();
                x[j] ^= 1;
                assert_eq!(read_header(&x), Some((len, false)), "reserved {j}");
            }
        }
        let mut z = Vec::new();
        header_bits(0, &mut z);
        assert!(read_header(&z).is_none());
        assert!(read_header(&z[..10]).is_none());
    }

    #[test]
    fn layout_examples() {
        assert_eq!(fec_octets(30), 2);
        assert_eq!(fec_octets(31), 4);
        assert_eq!(fec_octets(68), 6);
        let mut v = Vec::new();
        layout(300, |l, p| v.push((l, p)));
        assert_eq!(v, vec![(249, 6), (51, 4)]);
        assert_eq!(fec_octets(300), 10);
        assert_eq!(fec_octets(2), 0);
    }

    #[test]
    fn gray_table_round_trips() {
        for b in 0..8u8 {
            assert_eq!(bits_of(step_of(b)), b);
        }
        assert_eq!(bits_of(2), 0b011);
        assert_eq!(bits_of(4), 0b110);
    }

    #[test]
    fn a_body_round_trips_with_errors_spread_over_the_blocks() {
        let mut codec = Codec::default();
        let hdlc: Vec<u8> = (0..300 * 8).map(|i| ((i * 7 % 11) < 5) as u8).collect();
        let mut bits = Vec::new();
        burst_bits(&mut codec, &hdlc, &mut bits);
        let mut s = scrambler();
        let mut d: Vec<u8> = bits.iter().map(|&b| b ^ s.bit()).collect();
        let (len, _) = read_header(&d).unwrap();
        assert_eq!(len as usize, hdlc.len());
        // Two octet errors in each block.
        for k in [3usize, 4, 100, 101] {
            d[25 + 8 * k] ^= 1;
        }
        let mut dec = BodyDecoder::default();
        let mut body = Body::default();
        dec.decode(&d[25..], len, &mut body);
        assert!(!body.failed);
        assert_eq!(body.corrected, 4);
        assert_eq!(body.hdlc, hdlc);
    }

    /// The parity of a block from a recording: the sent octets 0x1B 0xC6 are the first two of the six.
    #[test]
    fn the_sent_parity_of_a_recorded_block() {
        let d = [
            0x7Eu8, 0x04, 0x42, 0x02, 0x00, 0x50, 0x4C, 0xA2, 0x8D, 0x04, 0x10, 0x01, 0x0B, 0x54,
            0x12, 0x34, 0x56, 0x78, 0x90, 0x00, 0xC1, 0x00, 0xED, 0x2A, 0x7E,
        ];
        let mut c = Codec::default();
        let mut full = [0u8; 6];
        assert!(c.parity(&d, &mut full));
        assert_eq!(&full[..2], &[0x1B, 0xC6]);
        let mut x = d;
        x[3] ^= 0x40;
        assert_eq!(c.correct(&mut x, &[0x1B, 0xC6], &[]), Some(1));
        assert_eq!(x, d);
        let mut y = d;
        y[0] ^= 1;
        y[10] ^= 2;
        y[20] ^= 3;
        assert_eq!(c.correct(&mut y, &full, &[]), Some(3));
        assert_eq!(y, d);
        let mut z = d;
        z[5] ^= 0x11;
        z[9] ^= 0x22;
        assert_eq!(c.correct(&mut z, &full, &[5, 9]), Some(2));
        assert_eq!(z, d);
    }
}
