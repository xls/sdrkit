//! The high-rate serial-tone waveform of MIL-STD-188-110C Appendix C (the waveform of STANAG 4539): the preamble, the frames
//! and mini-probes, the reinserted 103-symbol block, the rate and interleaver codes, the scrambler, the constellations, the
//! tail-biting code with its puncturing, the interleaver.

use crate::serial::{bits_lsb, bits_msb, psk8, EOM};
use sdrkit_algo_fec::conv::{codes::HFDL_K7, encode_tail_biting};
use sdrkit_core::Complex32;

/// The 184 sync symbols (Table C-VIII), tribits.
pub const SYNC: &str = "1513613116377354366454022260757407571610522623600514222340627433\
7202644176206236743613746572011144005774735416566463430713470143\
33511146106013141776300727202611127753360533107110304073";
/// The "+" mini-probe, 31 tribits.
pub const PROBE: &str = "0000024604040642000002460404064";
/// The 13-chip Barker pattern of the D groups.
pub const BARKER: [u8; 13] = [0, 4, 0, 4, 0, 0, 4, 4, 0, 0, 0, 0, 0];
/// Data symbols of a frame.
pub const FRAME: usize = 256;
/// Symbols of a mini-probe.
pub const PROBE_LEN: usize = 31;
/// Symbols of the block that starts and repeats every superframe.
pub const BLOCK: usize = 103;
/// Symbols of the sync part of the preamble.
pub const SYNC_LEN: usize = 184;
/// Frames of a superframe.
pub const SUPERFRAME: usize = 72;

/// The tribits of a digit string.
pub fn tribits(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b - b'0').collect()
}

/// A data rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    /// 3200 bit/s, QPSK.
    B3200,
    /// 4800 bit/s, 8-PSK.
    B4800,
    /// 6400 bit/s, 16-QAM.
    B6400,
    /// 8000 bit/s, 32-QAM.
    B8000,
    /// 9600 bit/s, 64-QAM.
    B9600,
    /// 12800 bit/s, 64-QAM, uncoded, one frame.
    B12800,
}

impl Rate {
    /// Every rate.
    pub const ALL: [Rate; 6] = [
        Rate::B3200,
        Rate::B4800,
        Rate::B6400,
        Rate::B8000,
        Rate::B9600,
        Rate::B12800,
    ];
    /// The rate code of Table C-X (1 to 6).
    pub fn code(self) -> u8 {
        self as u8 + 1
    }
    /// The rate of a rate code.
    pub fn from_code(c: u8) -> Option<Rate> {
        Rate::ALL.get(usize::from(c.checked_sub(1)?)).copied()
    }
    /// Bits a second.
    pub fn bps(self) -> u32 {
        [3200, 4800, 6400, 8000, 9600, 12800][self as usize]
    }
    /// Bits a symbol (the scrambler's steps).
    pub fn bits(self) -> usize {
        [2, 3, 4, 5, 6, 6][self as usize]
    }
    /// The scrambler's bits a symbol (3 for the PSK rates).
    pub fn scrambler_bits(self) -> usize {
        self.bits().max(3)
    }
}

/// The interleaver lengths in frames, by interleaver code 1 to 6.
pub const FRAMES: [usize; 6] = [1, 3, 9, 18, 36, 72];
/// The interleaver names.
pub const INTERLEAVER_NAMES: [&str; 6] = [
    "ultra short",
    "very short",
    "short",
    "medium",
    "long",
    "very long",
];

/// The interleaver code (1 to 6) of `frames`.
pub fn interleaver_code(frames: usize) -> Option<u8> {
    FRAMES
        .iter()
        .position(|&f| f == frames)
        .map(|i| i as u8 + 1)
}

/// Whether a rate and interleaver go together (12800 bit/s only with one frame).
pub fn valid(rate: Rate, frames: usize) -> bool {
    interleaver_code(frames).is_some() && (rate != Rate::B12800 || frames == 1)
}

/// "9600 bit/s, short interleaver (9 frames)".
pub fn name(rate: Rate, frames: usize) -> String {
    let i = interleaver_code(frames).map_or(0, |c| usize::from(c) - 1);
    format!(
        "{} bit/s, {} interleaver ({} frame{})",
        rate.bps(),
        INTERLEAVER_NAMES[i],
        frames,
        if frames == 1 { "" } else { "s" }
    )
}

/// D0, D1 and D2 of a rate and interleaver (Table C-IX).
pub fn d_values(rate: Rate, frames: usize) -> [u8; 3] {
    let r = rate.code();
    let l = interleaver_code(frames).unwrap_or(0);
    let map = [0u8, 2, 6, 4];
    let mut d = [0u8; 3];
    for (i, v) in d.iter_mut().enumerate() {
        let rb = (r >> (2 - i)) & 1;
        let lb = (l >> (2 - i)) & 1;
        *v = map[usize::from(rb * 2 + lb)];
    }
    d
}

/// The rate and interleaver of D values, when valid.
pub fn from_d(d: [u8; 3]) -> Option<(Rate, usize)> {
    for r in Rate::ALL {
        for f in FRAMES {
            if valid(r, f) && d_values(r, f) == d {
                return Some((r, f));
            }
        }
    }
    None
}

/// The 103-symbol block for a rate and interleaver.
pub fn block(rate: Rate, frames: usize) -> Vec<u8> {
    let p = tribits(PROBE);
    let mut v = p.clone();
    v.push(2);
    for d in d_values(rate, frames) {
        v.extend(BARKER.iter().map(|b| (b + d) & 7));
    }
    v.push(6);
    v.extend(p.iter().map(|x| (x + 4) & 7));
    v
}

/// Whether the mini-probe after frame `f` (1 to 71) is "-".
pub fn probe_minus(rate: Rate, frames: usize, f: usize) -> bool {
    let j = (f - 1) % 18;
    let set = ((f - 1) / 18 + 1) as u8;
    let r = rate.code();
    let l = interleaver_code(frames).unwrap_or(0);
    match j {
        0..=6 => true,
        7 | 17 => false,
        8..=10 => (r >> (10 - j)) & 1 == 1,
        11..=13 => (l >> (13 - j)) & 1 == 1,
        _ => (set >> (16 - j)) & 1 == 1,
    }
}

/// The mini-probe after frame `f` (1 to 71) as tribits.
pub fn mini_probe(rate: Rate, frames: usize, f: usize) -> Vec<u8> {
    let add = if probe_minus(rate, frames, f) { 4 } else { 0 };
    tribits(PROBE).iter().map(|x| (x + add) & 7).collect()
}

/// The data scrambler of Figure C-5: nine stages, the first symbol from the start state 000000001, then `b` steps after each
/// symbol; the value is the `b` stages at the output end (the last stage least significant).
pub fn scrambler(b: usize) -> Vec<u16> {
    let mut reg: u16 = 1;
    let mut out = Vec::with_capacity(FRAME);
    for _ in 0..FRAME {
        out.push(reg & ((1 << b) - 1));
        for _ in 0..b {
            let fb = ((reg >> 4) ^ reg) & 1;
            reg = (reg >> 1) | (fb << 8);
        }
    }
    out
}

/// The 16-QAM points (Table C-V).
pub const QAM16: [(f32, f32); 16] = [
    (0.866025, 0.5),
    (0.5, 0.866025),
    (1.0, 0.0),
    (0.258819, 0.258819),
    (-0.5, 0.866025),
    (0.0, 1.0),
    (-0.866025, 0.5),
    (-0.258819, 0.258819),
    (0.5, -0.866025),
    (0.0, -1.0),
    (0.866025, -0.5),
    (0.258819, -0.258819),
    (-0.866025, -0.5),
    (-0.5, -0.866025),
    (-1.0, 0.0),
    (-0.258819, -0.258819),
];

/// The first-quadrant points of 32-QAM, labels 0 to 7 (Table C-VI).
pub const QAM32_Q1: [(f32, f32); 8] = [
    (0.866380, 0.499386),
    (0.984849, 0.173415),
    (0.499386, 0.866380),
    (0.173415, 0.984849),
    (0.520246, 0.520246),
    (0.520246, 0.173415),
    (0.173415, 0.520246),
    (0.173415, 0.173415),
];

/// The 64-QAM points of labels 0 to 31 (Table C-VII).
pub const QAM64_LOW: [(f32, f32); 32] = [
    (1.0, 0.0),
    (0.822878, 0.568218),
    (0.821137, 0.152996),
    (0.932897, 0.360142),
    (0.0, -1.0),
    (0.822878, -0.568218),
    (0.821137, -0.152996),
    (0.932897, -0.360142),
    (0.568218, 0.822878),
    (0.588429, 0.588429),
    (0.588429, 0.117686),
    (0.588429, 0.353057),
    (0.568218, -0.822878),
    (0.588429, -0.588429),
    (0.588429, -0.117686),
    (0.588429, -0.353057),
    (0.152996, 0.821137),
    (0.117686, 0.588429),
    (0.117686, 0.117686),
    (0.117686, 0.353057),
    (0.152996, -0.821137),
    (0.117686, -0.588429),
    (0.117686, -0.117686),
    (0.117686, -0.353057),
    (0.360142, 0.932897),
    (0.353057, 0.588429),
    (0.353057, 0.117686),
    (0.353057, 0.353057),
    (0.360142, -0.932897),
    (0.353057, -0.588429),
    (0.353057, -0.117686),
    (0.353057, -0.353057),
];

/// The QPSK mapping of 3200 bit/s: dibit to tribit (Table C-III).
pub const MAP2: [u8; 4] = [0, 2, 6, 4];
/// The 8-PSK mapping of 4800 bit/s: label to tribit (Table C-IV).
pub const MAP3: [u8; 8] = [1, 0, 2, 3, 6, 7, 5, 4];

/// The points of a rate's labels before scrambling (for the PSK rates the tribits' points; scrambling turns them).
pub fn points(rate: Rate) -> Vec<Complex32> {
    let c = |p: &(f32, f32)| Complex32::new(p.0, p.1);
    match rate {
        Rate::B3200 => MAP2.iter().map(|&t| psk8(t)).collect(),
        Rate::B4800 => MAP3.iter().map(|&t| psk8(t)).collect(),
        Rate::B6400 => QAM16.iter().map(c).collect(),
        Rate::B8000 => {
            let mut v = vec![Complex32::default(); 32];
            for (k, p) in QAM32_Q1.iter().enumerate() {
                v[k] = c(p);
                v[8 + k] = Complex32::new(-p.0, p.1);
                v[16 + k] = Complex32::new(p.0, -p.1);
                v[24 + k] = Complex32::new(-p.0, -p.1);
            }
            v
        }
        Rate::B9600 | Rate::B12800 => {
            let mut v = vec![Complex32::default(); 64];
            for (k, p) in QAM64_LOW.iter().enumerate() {
                v[k] = c(p);
                v[32 + k] = Complex32::new(-p.0, p.1);
            }
            v[32] = Complex32::new(0.0, 1.0);
            v[36] = Complex32::new(-1.0, 0.0);
            v
        }
    }
}

/// The point sent for label `l` with scrambler value `s`.
pub fn symbol(rate: Rate, pts: &[Complex32], l: usize, s: u16) -> Complex32 {
    match rate {
        Rate::B3200 => psk8((MAP2[l] + s as u8) & 7),
        Rate::B4800 => psk8((MAP3[l] + s as u8) & 7),
        _ => pts[l ^ usize::from(s)],
    }
}

/// The interleaver increments (Table C-XVI), by rate (3200 to 9600) and interleaver code.
pub const INCREMENTS: [[usize; 6]; 5] = [
    [97, 229, 805, 1393, 3281, 6985],
    [145, 361, 1045, 2089, 5137, 10273],
    [189, 481, 1393, 3281, 6985, 11141],
    [201, 601, 1741, 3481, 8561, 14441],
    [229, 805, 2089, 5137, 10273, 17329],
];

/// The channel bits of an interleaver block, `S`.
pub fn block_bits(rate: Rate, frames: usize) -> usize {
    FRAME * rate.bits() * frames
}

/// The input bits of an interleaver block, `K` (3/4 of `S`; all of it at 12800 bit/s).
pub fn block_input(rate: Rate, frames: usize) -> usize {
    if rate == Rate::B12800 {
        block_bits(rate, frames)
    } else {
        block_bits(rate, frames) * 3 / 4
    }
}

/// The place of punctured bit `n` in the block: `n x increment mod S`. `None` at 12800 bit/s.
pub fn places(rate: Rate, frames: usize) -> Option<Vec<u32>> {
    if rate == Rate::B12800 {
        return None;
    }
    let s = block_bits(rate, frames);
    let inc = INCREMENTS[rate as usize][usize::from(interleaver_code(frames)? - 1)];
    Some((0..s).map(|n| ((n * inc) % s) as u32).collect())
}

/// The puncturing pattern to 3/4 on T1 T2 of three steps.
pub const PUNCTURE: [u8; 6] = [1, 1, 1, 0, 0, 1];

/// The channel bits of one block of `k` input bits: tail-biting code (the register loaded with bits 0 to 5, then bits 6 on and
/// bits 0 to 5 encoded), punctured, interleaved.
pub fn encode_block(rate: Rate, frames: usize, input: &[u8]) -> Vec<u8> {
    let Some(pl) = places(rate, frames) else {
        return input.to_vec();
    };
    let k = input.len();
    let mut rot: Vec<u8> = input[6.min(k)..].to_vec();
    rot.extend_from_slice(&input[..6.min(k)]);
    let mut coded = vec![0u8; 2 * k];
    encode_tail_biting(&HFDL_K7, &rot, &mut coded).expect("the output fits");
    let punct: Vec<u8> = coded
        .iter()
        .enumerate()
        .filter(|(i, _)| PUNCTURE[i % 6] == 1)
        .map(|(_, &b)| b)
        .collect();
    let mut out = vec![0u8; punct.len()];
    for (n, &b) in punct.iter().enumerate() {
        out[pl[n] as usize] = b;
    }
    out
}

/// Decodes a block from its channel soft values in the order sent (positive favours 0): deinterleaved by `places`,
/// depunctured, decoded by `viterbi` round the circle, the input order restored. Without `places` (12800 bit/s) the hard bits.
/// `None` when the decoder refuses the block.
pub fn decode_block(
    viterbi: &mut sdrkit_algo_fec::conv::Viterbi,
    places: Option<&[u32]>,
    soft: &[f32],
) -> Option<Vec<u8>> {
    let Some(pl) = places else {
        return Some(soft.iter().map(|&v| u8::from(v < 0.0)).collect());
    };
    let k = soft.len() * 3 / 4;
    let mut llr = vec![0i8; 2 * k];
    let mut n = 0usize;
    for (i, l) in llr.iter_mut().enumerate() {
        if PUNCTURE[i % 6] == 1 {
            let v = soft.get(pl[n] as usize).copied().unwrap_or(0.0);
            *l = crate::serial::soft::llr8(v, 2.0);
            n += 1;
        }
    }
    let mut u = vec![0u8; k];
    viterbi
        .decode_only(&llr, sdrkit_algo_fec::conv::Ends::TailBiting, &mut u)
        .ok()?;
    let mut v = u[k - 6..].to_vec();
    v.extend_from_slice(&u[..k - 6]);
    Some(v)
}

/// The input bits of a message: `bytes` least significant bit first, the end-of-message pattern when `eom`, zeros to the end of
/// an interleaver block.
pub fn message_bits(rate: Rate, frames: usize, bytes: &[u8], eom: bool) -> Vec<u8> {
    let mut b = bits_lsb(bytes);
    if eom {
        b.extend(bits_msb(u64::from(EOM), 32));
    }
    let k = block_input(rate, frames);
    while !b.len().is_multiple_of(k) || b.is_empty() {
        b.push(0);
    }
    b
}

/// The symbols of a whole transmission: `agc` AGC blocks, the sync, the block, then the frames of every interleaver block with
/// their mini-probes and the block again after each 72 frames; a mini-probe after the last frame.
pub fn transmission(rate: Rate, frames: usize, agc: usize, input: &[u8]) -> Vec<Complex32> {
    let sync = tribits(SYNC);
    let mut out = Vec::new();
    for _ in 0..agc {
        out.extend(sync.iter().map(|&t| psk8(t).conj()));
    }
    out.extend(sync.iter().map(|&t| psk8(t)));
    let blk = block(rate, frames);
    out.extend(blk.iter().map(|&t| psk8(t)));
    let pts = points(rate);
    let scr = scrambler(rate.scrambler_bits());
    let b = rate.bits();
    let mut f = 0usize;
    let k = block_input(rate, frames);
    for chunk in input.chunks(k) {
        let ch = encode_block(rate, frames, chunk);
        for frame in ch.chunks(FRAME * b) {
            for (i, sym) in frame.chunks(b).enumerate() {
                let l = sym.iter().fold(0usize, |a, &x| (a << 1) | usize::from(x));
                out.push(symbol(rate, &pts, l, scr[i]));
            }
            f += 1;
            if f.is_multiple_of(SUPERFRAME) {
                out.extend(blk.iter().map(|&t| psk8(t)));
            } else {
                out.extend(
                    mini_probe(rate, frames, f % SUPERFRAME)
                        .iter()
                        .map(|&t| psk8(t)),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d3(d: [u8; 3]) -> String {
        d.iter().map(|x| char::from(b'0' + x)).collect()
    }

    #[test]
    fn d_values_and_codes() {
        let want = [
            ["004", "026", "024", "206", "204", "226"],
            ["062", "040", "042", "260", "262", "240"],
            ["064", "046", "044", "266", "264", "246"],
            ["602", "620", "622", "400", "402", "420"],
            ["604", "626", "624", "406", "404", "426"],
        ];
        for (ri, row) in want.iter().enumerate() {
            for (fi, w) in row.iter().enumerate() {
                assert_eq!(d3(d_values(Rate::ALL[ri], FRAMES[fi])), *w);
            }
        }
        assert_eq!(d3(d_values(Rate::B12800, 1)), "662");
        let mut n = 0;
        for r in Rate::ALL {
            for f in FRAMES {
                if valid(r, f) {
                    assert_eq!(from_d(d_values(r, f)), Some((r, f)));
                    n += 1;
                }
            }
        }
        assert_eq!(n, 31);
        assert_eq!(tribits(SYNC).len(), SYNC_LEN);
        assert_eq!(block(Rate::B4800, 3).len(), BLOCK);
    }

    #[test]
    fn scrambler_first_values() {
        let s3: Vec<u16> = scrambler(3)[..16].to_vec();
        assert_eq!(s3, [1, 0, 0, 1, 4, 0, 3, 0, 1, 7, 4, 2, 5, 1, 4, 5]);
        let s6: Vec<u16> = scrambler(6)[..16].to_vec();
        assert_eq!(
            s6,
            [1, 8, 4, 3, 57, 20, 13, 44, 23, 54, 9, 10, 20, 11, 63, 38]
        );
    }

    #[test]
    fn interleaver_and_sizes() {
        let p = places(Rate::B3200, 1).unwrap();
        assert_eq!(&p[..8], &[0, 97, 194, 291, 388, 485, 70, 167]);
        for r in &Rate::ALL[..5] {
            for f in FRAMES {
                let p = places(*r, f).unwrap();
                let mut s = p.clone();
                s.sort_unstable();
                assert!(s.iter().enumerate().all(|(i, &v)| v as usize == i));
                assert_eq!(block_input(*r, f) * 4, block_bits(*r, f) * 3);
            }
        }
    }

    #[test]
    fn every_combination_round_trips() {
        let mut x = 0x1234_5678u32;
        for r in &Rate::ALL[..5] {
            for f in FRAMES {
                let k = block_input(*r, f);
                let input: Vec<u8> = (0..k)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 17;
                        x ^= x << 5;
                        (x & 1) as u8
                    })
                    .collect();
                let ch = encode_block(*r, f, &input);
                assert_eq!(ch.len(), block_bits(*r, f));
                let soft: Vec<f32> = ch
                    .iter()
                    .map(|&b| if b == 0 { 4.0 } else { -4.0 })
                    .collect();
                let mut v = sdrkit_algo_fec::conv::Viterbi::new(HFDL_K7, k + 8).unwrap();
                let got = decode_block(&mut v, places(*r, f).as_deref(), &soft).unwrap();
                assert_eq!(got, input, "{r:?} {f}");
            }
        }
        let k = block_input(Rate::B12800, 1);
        let input = vec![1u8; k];
        let soft: Vec<f32> = encode_block(Rate::B12800, 1, &input)
            .iter()
            .map(|&b| if b == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut v = sdrkit_algo_fec::conv::Viterbi::new(HFDL_K7, 8).unwrap();
        assert_eq!(decode_block(&mut v, None, &soft).unwrap(), input);
    }

    #[test]
    fn probes_and_points() {
        assert!(probe_minus(Rate::B4800, 3, 1));
        assert!(!probe_minus(Rate::B4800, 3, 8));
        // Rate code 2 = 010, interleaver code 2 = 010, set 1 = 001.
        let s: Vec<bool> = (9..=17).map(|f| probe_minus(Rate::B4800, 3, f)).collect();
        assert_eq!(
            s,
            [false, true, false, false, true, false, false, false, true]
        );
        for r in Rate::ALL {
            assert_eq!(points(r).len(), 1 << r.bits());
        }
        assert_eq!(points(Rate::B9600)[35], Complex32::new(-0.932897, 0.360142));
    }
}
