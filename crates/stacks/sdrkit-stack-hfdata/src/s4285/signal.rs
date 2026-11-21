//! The STANAG 4285 waveform: frames of 256 symbols (80 sync, then data and known symbols), the sync and scrambler generators,
//! the mapping, the profiles of rate and interleaver, the convolutional interleaver with its puncturing at 2400 bit/s.

use crate::serial::{bits_lsb, bits_msb, EOM};
use sdrkit_algo_fec::conv::{codes::HFDL_K7, Encoder};
use sdrkit_util_bits::interleave::DelayLanes;

/// Symbols of a frame.
pub const FRAME: usize = 256;
/// Sync symbols at the head of a frame.
pub const SYNC_LEN: usize = 80;
/// Data symbols of a frame.
pub const DATA: usize = 128;
/// The start-of-message pattern, first-sent bit the most significant.
pub const SOM: u32 = 0x0387_3C3C;
/// Rows of the interleaver.
pub const ROWS: usize = 32;

/// The 80 sync bits: a 5-stage register (x^5 + x^2 + 1) started at 11010 (stage 5 to stage 1), each step putting out stage 1.
pub fn sync_bits() -> [u8; SYNC_LEN] {
    let mut reg: u8 = 0b11010;
    let mut out = [0u8; SYNC_LEN];
    for o in out.iter_mut() {
        *o = reg & 1;
        let fb = (reg ^ (reg >> 2)) & 1;
        reg = (reg >> 1) | (fb << 4);
    }
    out
}

/// The start state of the scrambler at each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScramblerStart {
    /// All nine stages 1.
    AllOnes,
    /// The value 1 (stage 1 alone).
    One,
}

/// The 176 scrambler tribits of a frame: a 9-stage register (x^9 + x^4 + 1), the value its low three stages, then 3 steps.
pub fn scrambler(start: ScramblerStart) -> [u8; FRAME - SYNC_LEN] {
    let mut reg: u16 = match start {
        ScramblerStart::AllOnes => 0x1FF,
        ScramblerStart::One => 1,
    };
    let mut out = [0u8; FRAME - SYNC_LEN];
    for o in out.iter_mut() {
        *o = (reg & 7) as u8;
        for _ in 0..3 {
            let fb = (reg ^ (reg >> 4)) & 1;
            reg = (reg >> 1) | (fb << 8);
        }
    }
    out
}

/// Whether symbol `k` (0 to 175) after the sync is known: the blocks of 16 at 32 to 47, 80 to 95 and 128 to 143.
pub fn is_known(k: usize) -> bool {
    matches!(k % 48, 32..=47) && k < 144
}

/// The 3-bit mapping: label to tribit.
pub const MAP3: [u8; 8] = [1, 0, 2, 3, 6, 7, 5, 4];
/// The 2-bit mapping: label to tribit.
pub const MAP2: [u8; 4] = [0, 2, 6, 4];
/// The 1-bit mapping: label to tribit.
pub const MAP1: [u8; 2] = [0, 4];

/// A profile: rate, code and interleaver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    /// The profile number (0 to 8, 10 to 15).
    pub number: u8,
    /// Bits a second.
    pub bps: u32,
    /// Coded (with a start-of-message pattern) or not.
    pub coded: bool,
    /// Bits a data symbol.
    pub bits: usize,
    /// How often each pair of coded bits is sent.
    pub rep: usize,
    /// The interleaver increment (0 uncoded).
    pub j: usize,
    /// The long interleaver.
    pub long: bool,
}

const fn coded(number: u8, bps: u32, bits: usize, rep: usize, j: usize, long: bool) -> Profile {
    Profile {
        number,
        bps,
        coded: true,
        bits,
        rep,
        j,
        long,
    }
}

const fn plain(number: u8, bps: u32, bits: usize) -> Profile {
    Profile {
        number,
        bps,
        coded: false,
        bits,
        rep: 1,
        j: 0,
        long: false,
    }
}

/// The 15 profiles.
pub const PROFILES: [Profile; 15] = [
    coded(0, 2400, 3, 1, 4, false),
    coded(1, 1200, 2, 1, 2, false),
    coded(2, 600, 1, 1, 1, false),
    coded(3, 300, 1, 2, 1, false),
    coded(4, 150, 1, 4, 1, false),
    coded(5, 75, 1, 8, 1, false),
    plain(6, 3600, 3),
    plain(7, 2400, 2),
    plain(8, 1200, 1),
    coded(10, 2400, 3, 1, 48, true),
    coded(11, 1200, 2, 1, 24, true),
    coded(12, 600, 1, 1, 12, true),
    coded(13, 300, 1, 2, 12, true),
    coded(14, 150, 1, 4, 12, true),
    coded(15, 75, 1, 8, 12, true),
];

impl Profile {
    /// The tribits of the labels.
    pub fn labels(&self) -> &'static [u8] {
        match self.bits {
            3 => &MAP3,
            2 => &MAP2,
            _ => &MAP1,
        }
    }
    /// Whether the rows `r mod 4 = 3` are left out (2400 bit/s coded).
    pub fn punctured(&self) -> bool {
        self.coded && self.bps == 2400
    }
    /// Whether row `r` is sent.
    pub fn sends_row(&self, r: usize) -> bool {
        !(self.punctured() && r % 4 == 3)
    }
    /// "1200 bit/s coded (QPSK), short".
    pub fn name(&self) -> String {
        let m = match self.bits {
            3 => "8-PSK",
            2 => "QPSK",
            _ => "BPSK",
        };
        if self.coded {
            let il = if self.long { "long" } else { "short" };
            format!("{} bit/s coded ({m}), {il}", self.bps)
        } else {
            format!("{} bit/s uncoded ({m})", self.bps)
        }
    }
    /// The zeros the test signal sends after the end-of-message pattern: the information bits of the interleaver's delay
    /// (31 j cycles) and 200 more.
    pub fn flush_bits(&self) -> usize {
        (ROWS - 1) * self.j * ROWS / (2 * self.rep) + 200
    }
}

/// The interleaver of a coded profile as delay lanes, one a row (transmit: row `r` delayed `j r` cycles; receive: `j (31 - r)`).
pub fn lanes<T: Copy>(j: usize, receive: bool, fill: T) -> DelayLanes<T> {
    let d: Vec<u32> = (0..ROWS)
        .map(|r| (j * if receive { ROWS - 1 - r } else { r }) as u32)
        .collect();
    DelayLanes::new(&d, fill)
}

/// The row of coded bit `i` of a cycle.
pub fn row_of(i: usize) -> usize {
    (9 * i) % ROWS
}

/// The bits a coded profile sends for the information bits `info`: encoded, repeated, interleaved, the left-out rows dropped.
pub fn coded_bits(p: &Profile, info: &[u8]) -> Vec<u8> {
    let mut e = Encoder::new(HFDL_K7);
    let mut c = Vec::with_capacity(info.len() * 2 * p.rep);
    for &b in info {
        let l = e.push_bit(b);
        for _ in 0..p.rep {
            c.push(l & 1);
            c.push((l >> 1) & 1);
        }
    }
    while c.len() % ROWS != 0 {
        c.push(0);
    }
    let mut il = lanes(p.j, false, 0u8);
    let mut out = Vec::with_capacity(c.len());
    let mut by_row = [0u8; ROWS];
    for cyc in c.chunks(ROWS) {
        for (i, &b) in cyc.iter().enumerate() {
            by_row[row_of(i)] = b;
        }
        for (r, &b) in by_row.iter().enumerate() {
            let o = il.push(b);
            if p.sends_row(r) {
                out.push(o);
            }
        }
    }
    out
}

/// The information bits of a coded message: the start pattern, `bytes` least significant bit first, the end pattern, the flush.
pub fn message_bits(p: &Profile, bytes: &[u8]) -> Vec<u8> {
    let mut b = bits_msb(u64::from(SOM), 32);
    b.extend(bits_lsb(bytes));
    b.extend(bits_msb(u64::from(EOM), 32));
    b.extend(std::iter::repeat_n(0, p.flush_bits()));
    b
}

/// The tribits of frames carrying `bits` (whole frames, padded with zeros).
pub fn frames(p: &Profile, bits: &[u8], start: ScramblerStart) -> Vec<u8> {
    let sync = sync_bits();
    let scr = scrambler(start);
    let per = DATA * p.bits;
    let nf = bits.len().div_ceil(per).max(1);
    let mut out = Vec::with_capacity(nf * FRAME);
    let mut it = bits.iter().copied();
    for _ in 0..nf {
        out.extend(sync.iter().map(|&b| b * 4));
        for (k, &s) in scr.iter().enumerate() {
            let t = if is_known(k) {
                0
            } else {
                let mut l = 0usize;
                for _ in 0..p.bits {
                    l = (l << 1) | usize::from(it.next().unwrap_or(0));
                }
                p.labels()[l]
            };
            out.push((t + s) & 7);
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
    fn sync_and_scrambler() {
        assert_eq!(
            digits(&sync_bits()),
            "01011001111100011011101010000100101100111110001101110101000010010110011111000110"
        );
        assert_eq!(
            digits(&scrambler(ScramblerStart::AllOnes)),
            "7770476705364044265507173660522434552701340100140301742514572661\
1214231776444667113541300341503157724607232363475310355533023575\
250054271537007431175652440643057664506570677107"
        );
        assert_eq!(scrambler(ScramblerStart::One)[0], 1);
        assert_eq!((0..176).filter(|&k| is_known(k)).count(), 48);
    }

    #[test]
    fn the_interleaver_delays_31_j_cycles() {
        let mut tx = lanes(1, false, 0u8);
        let mut rx = lanes(1, true, 0u8);
        let mut seen = None;
        for cyc in 0..40 {
            let mut by_row = [0u8; ROWS];
            if cyc == 0 {
                by_row[row_of(0)] = 1;
            }
            let mut out = [0u8; ROWS];
            for r in 0..ROWS {
                out[r] = rx.push(tx.push(by_row[r]));
            }
            for i in 0..ROWS {
                if out[row_of(i)] == 1 {
                    seen = Some((cyc, i));
                }
            }
        }
        assert_eq!(seen, Some((31, 0)));
    }

    #[test]
    fn profile_sizes() {
        for p in PROFILES {
            let per_frame = DATA * p.bits;
            if p.coded {
                let sent = if p.punctured() { 24 } else { 32 };
                let info = per_frame * 32 / sent / 2 / p.rep;
                assert_eq!(info * 2400, p.bps as usize * FRAME, "{p:?}");
            } else {
                assert_eq!(per_frame * 2400, p.bps as usize * FRAME);
            }
        }
    }
}
