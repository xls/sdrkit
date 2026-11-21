//! The MIL-STD-188-110 serial-tone waveform (MIL-STD-188-110C with Change 1, section 5.3.2): rates, the code, the interleaver,
//! the mapping to tribits, the 75 bit/s sets, the preamble, the known symbols and the data scrambler.

use crate::serial::bits_msb;
use sdrkit_algo_fec::conv::{codes::HFDL_K7, Encoder};

/// The sync scrambler of the preamble (5.3.2.3.8.2), 32 tribits repeating.
pub const SYNC_SCRAMBLER: [u8; 32] = [
    7, 4, 3, 0, 5, 1, 5, 0, 2, 2, 1, 1, 5, 7, 4, 3, 5, 0, 2, 6, 2, 1, 6, 2, 0, 0, 5, 0, 5, 2, 6, 6,
];

/// The 8-tribit patterns of the channel symbols 0 to 7 (Table XIII).
pub const PATTERNS: [[u8; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 4, 0, 4, 0, 4, 0, 4],
    [0, 0, 4, 4, 0, 0, 4, 4],
    [0, 4, 4, 0, 0, 4, 4, 0],
    [0, 0, 0, 0, 4, 4, 4, 4],
    [0, 4, 0, 4, 4, 0, 4, 0],
    [0, 0, 4, 4, 4, 4, 0, 0],
    [0, 4, 4, 0, 4, 0, 0, 4],
];

/// The first nine channel symbols of every preamble segment.
pub const SEGMENT_HEAD: [u8; 9] = [0, 1, 3, 0, 1, 3, 1, 2, 0];
/// Tribits of a channel symbol.
pub const CHANNEL_SYMBOL: usize = 32;
/// Tribits of a preamble segment (15 channel symbols, 0.2 s).
pub const SEGMENT: usize = 15 * CHANNEL_SYMBOL;
/// The known tribits at the head of a segment.
pub const HEAD: usize = SEGMENT_HEAD.len() * CHANNEL_SYMBOL;

/// The 3-bit modified Gray mapping (Table VIII): label to tribit.
pub const GRAY3: [u8; 8] = [0, 1, 3, 2, 7, 6, 4, 5];
/// The 2-bit mapping (Table IX with Figure 5): label to tribit.
pub const GRAY2: [u8; 4] = [0, 2, 6, 4];
/// The 1-bit mapping: label to tribit.
pub const GRAY1: [u8; 2] = [0, 4];
/// The 75 bit/s mapping: two bits to the set (Table IX then Table X).
pub const SET_OF_BITS: [usize; 4] = [0, 1, 3, 2];
/// The 75 bit/s normal sets: 4 tribits repeated 8 times (Table X).
pub const SETS_NORMAL: [[u8; 4]; 4] = [[0, 0, 0, 0], [0, 4, 0, 4], [0, 0, 4, 4], [0, 4, 4, 0]];
/// The 75 bit/s exceptional sets: 8 tribits repeated 4 times (Table X).
pub const SETS_EXCEPTIONAL: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 4, 4, 4, 4],
    [0, 4, 0, 4, 4, 0, 4, 0],
    [0, 0, 4, 4, 4, 4, 0, 0],
    [0, 4, 4, 0, 4, 0, 0, 4],
];

/// Tribit `j` (0 to 31) of 75 bit/s set `s`.
pub fn set_tribit(s: usize, exceptional: bool, j: usize) -> u8 {
    if exceptional {
        SETS_EXCEPTIONAL[s & 3][j % 8]
    } else {
        SETS_NORMAL[s & 3][j % 4]
    }
}

/// The data rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    /// 75 bit/s.
    B75,
    /// 150 bit/s.
    B150,
    /// 300 bit/s.
    B300,
    /// 600 bit/s.
    B600,
    /// 1200 bit/s.
    B1200,
    /// 2400 bit/s.
    B2400,
    /// 4800 bit/s (no code, no interleaver).
    B4800,
}

impl Rate {
    /// Every rate.
    pub const ALL: [Rate; 7] = [
        Rate::B75,
        Rate::B150,
        Rate::B300,
        Rate::B600,
        Rate::B1200,
        Rate::B2400,
        Rate::B4800,
    ];

    /// Bits a second.
    pub fn bps(self) -> u32 {
        match self {
            Rate::B75 => 75,
            Rate::B150 => 150,
            Rate::B300 => 300,
            Rate::B600 => 600,
            Rate::B1200 => 1200,
            Rate::B2400 => 2400,
            Rate::B4800 => 4800,
        }
    }

    /// D1 and D2 (Table XI); `None` for 4800 bit/s with a long interleaver.
    pub fn d(self, long: bool) -> Option<(u8, u8)> {
        let (s, l) = match self {
            Rate::B4800 => ((7, 6), None),
            Rate::B2400 => ((6, 4), Some((4, 4))),
            Rate::B1200 => ((6, 5), Some((4, 5))),
            Rate::B600 => ((6, 6), Some((4, 6))),
            Rate::B300 => ((6, 7), Some((4, 7))),
            Rate::B150 => ((7, 4), Some((5, 4))),
            Rate::B75 => ((7, 5), Some((5, 5))),
        };
        if long {
            l
        } else {
            Some(s)
        }
    }

    /// The rate and interleaver that D1 and D2 name (`true` long).
    pub fn from_d(d1: u8, d2: u8) -> Option<(Rate, bool)> {
        for r in Rate::ALL {
            for long in [false, true] {
                if r.d(long) == Some((d1, d2)) {
                    return Some((r, long));
                }
            }
        }
        None
    }

    /// Coded bits a data symbol (75 bit/s: a set of 32 tribits carries 2).
    pub fn bits(self) -> usize {
        match self {
            Rate::B4800 | Rate::B2400 => 3,
            Rate::B1200 | Rate::B75 => 2,
            _ => 1,
        }
    }

    /// Unknown (data) and known symbols of a cycle; 75 bit/s has no known symbols.
    pub fn cycle(self) -> (usize, usize) {
        match self {
            Rate::B4800 | Rate::B2400 => (32, 16),
            Rate::B75 => (32, 0),
            _ => (20, 20),
        }
    }

    /// How often each pair of coded bits is sent.
    pub fn repetition(self) -> usize {
        match self {
            Rate::B300 => 2,
            Rate::B150 => 4,
            _ => 1,
        }
    }

    /// Whether the rate is coded.
    pub fn coded(self) -> bool {
        self != Rate::B4800
    }

    /// Interleaver rows and columns (Table VI); `None` for 4800 bit/s.
    pub fn interleaver(self, long: bool) -> Option<(usize, usize)> {
        Some(match (self, long) {
            (Rate::B4800, _) => return None,
            (Rate::B2400, false) => (40, 72),
            (Rate::B2400, true) => (40, 576),
            (Rate::B1200, false) => (40, 36),
            (Rate::B1200, true) => (40, 288),
            (Rate::B75, false) => (10, 9),
            (Rate::B75, true) => (20, 36),
            (_, false) => (40, 18),
            (_, true) => (40, 144),
        })
    }

    /// The load row step and the fetch column step of the interleaver.
    pub fn steps(self) -> (usize, usize) {
        if self == Rate::B75 {
            (7, 7)
        } else {
            (9, 17)
        }
    }

    /// The tribits a data symbol can be (before scrambling), label order: label `l` is tribit `labels()[l]`.
    pub fn labels(self) -> &'static [u8] {
        match self.bits() {
            3 => &GRAY3,
            2 if self != Rate::B75 => &GRAY2,
            _ => &GRAY1,
        }
    }

    /// The symbols of an interleaver block (1440 short, 11520 long; 4800 bit/s counts in short blocks).
    pub fn block_symbols(self, long: bool) -> usize {
        if long && self != Rate::B4800 {
            11_520
        } else {
            1440
        }
    }

    /// Coded bits of an interleaver block (with repetition), or the data bits of a 4800 bit/s block.
    pub fn block_bits(self, long: bool) -> usize {
        match self.interleaver(long) {
            Some((r, c)) => r * c,
            None => {
                let (u, k) = self.cycle();
                self.block_symbols(false) / (u + k) * u * 3
            }
        }
    }

    /// Information bits of an interleaver block.
    pub fn block_info_bits(self, long: bool) -> usize {
        if self.coded() {
            self.block_bits(long) / (2 * self.repetition())
        } else {
            self.block_bits(long)
        }
    }

    /// "1200 bit/s, long (4.8 s) interleaver".
    pub fn name(self, long: bool) -> String {
        let il = match (self, long) {
            (Rate::B4800, _) => "no interleaver",
            (_, false) => "short (0.6 s) interleaver",
            (_, true) => "long (4.8 s) interleaver",
        };
        format!("{} bit/s, {il}", self.bps())
    }
}

/// The interleaver of a rate as its fetch order: entry `i` is the load index of the `i`-th bit sent (5.3.2.3.4 and 5.3.2.3.5).
pub fn fetch_order(rate: Rate, long: bool) -> Option<Vec<u32>> {
    let (rows, cols) = rate.interleaver(long)?;
    let (row_step, col_step) = rate.steps();
    let mut at = vec![0u32; rows * cols];
    let (mut r, mut c) = (0usize, 0usize);
    for k in 0..rows * cols {
        at[r * cols + c] = k as u32;
        r = (r + row_step) % rows;
        if (k + 1) % rows == 0 {
            r = 0;
            c += 1;
        }
    }
    let mut order = Vec::with_capacity(rows * cols);
    for start in 0..cols {
        let mut c = start;
        for r in 0..rows {
            order.push(at[r * cols + c]);
            c = (c + cols - col_step % cols) % cols;
        }
    }
    Some(order)
}

/// The data scrambler (5.3.2.3.8.1 and Figure 6): 160 tribits, the register loaded with `BAD` hex and stepped 8 times before each
/// value is read from its low three bits.
pub fn data_scrambler() -> [u8; 160] {
    let mut reg: u16 = 0xBAD;
    let mut out = [0u8; 160];
    for o in out.iter_mut() {
        for _ in 0..8 {
            let b = (reg >> 11) & 1;
            reg = ((reg << 1) & 0xFFF) | b;
            if b == 1 {
                reg ^= (1 << 1) | (1 << 4) | (1 << 6);
            }
        }
        *o = (reg & 7) as u8;
    }
    out
}

/// The three count channel symbols of a segment count (5.3.2.3.7.2.1): its 6 bits in three 2-bit values, most significant first,
/// each plus 4.
pub fn count_symbols(count: u8) -> [u8; 3] {
    [
        4 + ((count >> 4) & 3),
        4 + ((count >> 2) & 3),
        4 + (count & 3),
    ]
}

/// The segment count of three count channel symbols; `None` when one is below 4.
pub fn count_of(c: [u8; 3]) -> Option<u8> {
    if c.iter().any(|&v| !(4..8).contains(&v)) {
        return None;
    }
    Some(((c[0] - 4) << 4) | ((c[1] - 4) << 2) | (c[2] - 4))
}

/// The 15 channel symbols of a segment with count `count`.
pub fn segment_symbols(d1: u8, d2: u8, count: u8) -> [u8; 15] {
    let c = count_symbols(count);
    [0, 1, 3, 0, 1, 3, 1, 2, 0, d1, d2, c[0], c[1], c[2], 0]
}

/// Tribit `k` of a segment whose channel symbols are `syms`.
pub fn segment_tribit(syms: &[u8; 15], k: usize) -> u8 {
    (PATTERNS[usize::from(syms[k / CHANNEL_SYMBOL])][k % 8] + SYNC_SCRAMBLER[k % 32]) & 7
}

/// The tribits of a preamble of `segments` segments (3 short, 24 long).
pub fn preamble(d1: u8, d2: u8, segments: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(segments * SEGMENT);
    for s in (0..segments).rev() {
        let syms = segment_symbols(d1, d2, s as u8);
        v.extend((0..SEGMENT).map(|k| segment_tribit(&syms, k)));
    }
    v
}

/// Where a data-phase symbol lies: `None` for a data symbol, else the known tribit (before scrambling). `pos` counts symbols from
/// the start of the interleaver block.
pub fn known_at(rate: Rate, long: bool, pos: usize) -> Option<u8> {
    let (u, k) = rate.cycle();
    if k == 0 {
        return None;
    }
    let cyc = pos / (u + k);
    let j = pos % (u + k);
    if j < u {
        return None;
    }
    let j = j - u;
    let cycles = rate.block_symbols(long) / (u + k);
    let (d1, d2) = rate.d(long).unwrap_or((7, 6));
    if j < 16 && cyc == cycles - 2 {
        Some(PATTERNS[usize::from(d1)][j % 8])
    } else if j < 16 && cyc == cycles - 1 {
        Some(PATTERNS[usize::from(d2)][j % 8])
    } else {
        Some(0)
    }
}

/// The information bits of a message of `bytes`: the bytes least significant bit first, the end-of-message pattern, 144 zero bits,
/// zeros to the end of an interleaver block.
pub fn message_bits(rate: Rate, long: bool, bytes: &[u8]) -> Vec<u8> {
    let mut b = crate::serial::bits_lsb(bytes);
    b.extend(bits_msb(u64::from(crate::serial::EOM), 32));
    b.extend(std::iter::repeat_n(0, 144));
    let n = rate.block_info_bits(long);
    while !b.len().is_multiple_of(n) {
        b.push(0);
    }
    b
}

/// The tribits of a whole transmission (preamble and data phase) of the information bits `info` (whole interleaver blocks).
pub fn transmission(rate: Rate, long: bool, info: &[u8]) -> Vec<u8> {
    let (d1, d2) = rate.d(long).expect("the rate has this interleaver");
    let mut out = preamble(d1, d2, if long { 24 } else { 3 });
    let mut coded = Vec::with_capacity(info.len() * 2);
    if rate.coded() {
        let mut e = Encoder::new(HFDL_K7);
        let rep = rate.repetition();
        for &bit in info {
            let l = e.push_bit(bit);
            for _ in 0..rep {
                coded.push(l & 1);
                coded.push((l >> 1) & 1);
            }
        }
    } else {
        coded.extend_from_slice(info);
    }
    let order = fetch_order(rate, long);
    let scr = data_scrambler();
    let bs = rate.block_bits(long);
    let nsym = rate.block_symbols(long);
    let mut d = 0usize;
    for block in coded.chunks(bs) {
        let sent: Vec<u8> = match &order {
            Some(o) => o.iter().map(|&i| block[i as usize]).collect(),
            None => block.to_vec(),
        };
        let mut bits = sent.iter();
        for pos in 0..nsym {
            if rate == Rate::B75 {
                if pos % 32 == 0 {
                    let a = bits.next().copied().unwrap_or(0);
                    let b = bits.next().copied().unwrap_or(0);
                    let s = SET_OF_BITS[usize::from(a * 2 + b)];
                    let exc = pos / 32 == nsym / 32 - 1;
                    for j in 0..32 {
                        out.push((set_tribit(s, exc, j) + scr[(d + j) % 160]) & 7);
                    }
                }
                d += 1;
                continue;
            }
            let t = match known_at(rate, long, pos) {
                Some(k) => k,
                None => {
                    let mut l = 0usize;
                    for _ in 0..rate.bits() {
                        l = (l << 1) | usize::from(bits.next().copied().unwrap_or(0));
                    }
                    rate.labels()[l]
                }
            };
            out.push((t + scr[d % 160]) & 7);
            d += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digits(v: &[u8]) -> String {
        v.iter().map(|d| char::from(b'0' + d)).collect()
    }

    #[test]
    fn the_code_of_a_single_one() {
        let mut e = Encoder::new(HFDL_K7);
        let mut o = Vec::new();
        e.encode(&[1, 0, 0, 0, 0, 0, 0], &mut o);
        assert_eq!(digits(&o), "11011111001011");
    }

    #[test]
    fn counts_and_preamble_tribits() {
        assert_eq!(count_symbols(23), [5, 5, 7]);
        assert_eq!(count_symbols(2), [4, 4, 6]);
        assert_eq!(count_symbols(0), [4, 4, 4]);
        assert_eq!(count_of([5, 5, 7]), Some(23));
        let syms = segment_symbols(6, 4, 2);
        let first: Vec<u8> = (0..32).map(|k| segment_tribit(&syms, k)).collect();
        assert_eq!(digits(&first), "74305150221157435026216200505266");
        let d1: Vec<u8> = (288..320).map(|k| segment_tribit(&syms, k)).collect();
        assert_eq!(digits(&d1), "74741550225513435062656200141666");
    }

    #[test]
    fn data_scrambler_first_values() {
        assert_eq!(
            digits(&data_scrambler()[..40]),
            "0243364576705543543707626246724755707333"
        );
    }

    #[test]
    fn interleavers() {
        let o = fetch_order(Rate::B2400, false).unwrap();
        assert_eq!(&o[..6], &[0, 2209, 1538, 867, 196, 2365]);
        let long = fetch_order(Rate::B2400, true).unwrap();
        // The second bit of the long block is row 1, column 559; the third row 2, column 542.
        assert_eq!(long[1], 559 * 40 + 9);
        assert_eq!(long[2], 542 * 40 + 18);
        for r in Rate::ALL {
            for l in [false, true] {
                let Some(o) = fetch_order(r, l) else {
                    assert_eq!(r, Rate::B4800);
                    continue;
                };
                let mut s = o.clone();
                s.sort_unstable();
                assert!(s.iter().enumerate().all(|(i, &v)| v as usize == i), "{r:?}");
                assert_eq!(o.len(), r.block_bits(l));
                assert_eq!(
                    r.block_bits(l) / if r == Rate::B75 { 2 } else { r.bits() },
                    r.block_symbols(l) / (r.cycle().0 + r.cycle().1) * r.cycle().0
                        / if r == Rate::B75 { 32 } else { 1 },
                    "{r:?} {l}"
                );
            }
        }
    }

    #[test]
    fn rate_names_and_lengths() {
        assert_eq!(Rate::from_d(4, 5), Some((Rate::B1200, true)));
        assert_eq!(Rate::from_d(7, 7), None);
        assert_eq!(Rate::B600.block_info_bits(false), 360);
        let t = transmission(Rate::B600, false, &vec![0; 360]);
        assert_eq!(t.len(), 3 * SEGMENT + 1440);
        let t = transmission(Rate::B75, true, &vec![0; 360]);
        assert_eq!(t.len(), 24 * SEGMENT + 11_520);
    }
}
