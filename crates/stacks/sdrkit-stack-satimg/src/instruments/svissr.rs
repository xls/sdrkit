//! FY-2 S-VISSR frames: the PN preamble, the descrambling of the frame, and the line counter, IR1 to IR4 and the four
//! VIS lines of a frame.

use std::sync::OnceLock;

/// Preamble bits.
pub const PREAMBLE: usize = 10_000;
/// Frame bits after the preamble.
pub const FRAME_BITS: usize = 354_848;
/// Frame bytes after the preamble.
pub const FRAME_BYTES: usize = 44_356;
/// The sync window: the last 64 bits of the preamble.
pub const SYNC_WINDOW: u64 = 0x4BBB_B999_9555_7FFF;
/// IR samples per line.
pub const IR_SAMPLES: usize = 2291;
/// VIS samples per VIS line.
pub const VIS_SAMPLES: usize = 9160;
/// Bits of a block.
pub const BLOCK: usize = 20_408;
/// The first bit of IR4.
pub const IR4_BIT: usize = 329_874;
/// VIS block bits.
pub const VIS_BLOCK: usize = 57_060;
/// The most lines (rows) of a picture.
pub const MAX_LINES: u32 = 2501;

/// The PN register of S-VISSR: the Fibonacci register of x^15 + x^14 + 1 seeded 011001110011111, its output the new bit.
pub const PN: sdrkit_util_bits::lfsr::Lfsr =
    sdrkit_util_bits::lfsr::Lfsr::fibonacci((1 << 15) | (1 << 14) | 1, 0b011_0011_1001_1111);

/// The PN sequence of the preamble and the frame, one bit for each byte.
pub fn pn_bits() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = vec![0u8; PREAMBLE + FRAME_BITS];
        let mut r = PN;
        r.fill_bits(&mut out);
        out
    })
}

/// The scrambling table of the 44 356 frame bytes (PN bits from 10 000 on, odd bytes complemented).
pub fn scramble_table() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let pn = pn_bits();
        (0..FRAME_BYTES)
            .map(|i| {
                let mut v = 0u8;
                for k in 0..8 {
                    v = (v << 1) | pn[PREAMBLE + i * 8 + k];
                }
                if i % 2 == 1 {
                    !v
                } else {
                    v
                }
            })
            .collect()
    })
}

/// Scrambles or descrambles a frame in place.
pub fn scramble(frame: &mut [u8]) {
    for (b, s) in frame.iter_mut().zip(scramble_table()) {
        *b ^= s;
    }
}

fn get(data: &[u8], bit: usize, n: usize) -> u32 {
    let mut v = 0u32;
    for i in 0..n {
        let b = bit + i;
        v = (v << 1) | ((data[b / 8] >> (7 - b % 8)) & 1) as u32;
    }
    v
}

fn put(data: &mut [u8], bit: usize, n: usize, v: u32) {
    for i in 0..n {
        let b = bit + i;
        if (v >> (n - 1 - i)) & 1 == 1 {
            data[b / 8] |= 0x80 >> (b % 8);
        }
    }
}

/// One scan line of the five channels.
#[derive(Clone, Debug, PartialEq)]
pub struct SvissrLine {
    /// The line counter.
    pub counter: u16,
    /// IR1 to IR3 (8 bits, as shown: 255 - value).
    pub ir: [Vec<u8>; 3],
    /// IR4 (10 bits, as shown: 1023 - value).
    pub ir4: Vec<u16>,
    /// The four VIS lines (6 bits).
    pub vis: [Vec<u8>; 4],
    /// Set bits in the spare area (the data bit error estimate).
    pub spare_ones: u32,
}

/// Parses a clear (descrambled) frame.
pub fn parse(f: &[u8]) -> SvissrLine {
    let counter = (u16::from_be_bytes([f[67], f[68]])) & 0x0FFF;
    let ir: [Vec<u8>; 3] = std::array::from_fn(|k| {
        (0..IR_SAMPLES)
            .map(|i| 255 - get(f, (k + 1) * BLOCK + 16 + i * 8, 8) as u8)
            .collect()
    });
    let ir4 = (0..IR_SAMPLES)
        .map(|i| 1023 - get(f, IR4_BIT + i * 10, 10) as u16)
        .collect();
    let vis: [Vec<u8>; 4] = std::array::from_fn(|j| {
        (0..VIS_SAMPLES)
            .map(|i| get(f, 4 * BLOCK + j * VIS_BLOCK + 24 + i * 6, 6) as u8)
            .collect()
    });
    let spare_ones = f[2114..2293].iter().map(|b| b.count_ones()).sum();
    SvissrLine {
        counter,
        ir,
        ir4,
        vis,
        spare_ones,
    }
}

/// Builds a clear frame (the generator's side).
pub fn build(l: &SvissrLine) -> Vec<u8> {
    let mut f = vec![0u8; FRAME_BYTES];
    f[67..69].copy_from_slice(&(l.counter & 0x0FFF).to_be_bytes());
    for k in 0..3 {
        for (i, &v) in l.ir[k].iter().enumerate() {
            put(&mut f, (k + 1) * BLOCK + 16 + i * 8, 8, 255 - v as u32);
        }
    }
    for (i, &v) in l.ir4.iter().enumerate() {
        put(&mut f, IR4_BIT + i * 10, 10, 1023 - v as u32);
    }
    for j in 0..4 {
        for (i, &v) in l.vis[j].iter().enumerate() {
            put(
                &mut f,
                4 * BLOCK + j * VIS_BLOCK + 24 + i * 6,
                6,
                (v & 63) as u32,
            );
        }
    }
    f
}

/// What the frame finder reports about a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundFrame {
    /// The descrambled frame.
    pub bytes: Vec<u8>,
    /// The preamble bit error ratio.
    pub preamble_ber: f64,
}

/// The frame finder: decoded (NRZ-M undone) bits in; the sync window, the preamble check, the frame.
pub struct SvissrFramer {
    hist: Vec<u8>,
    reg: u64,
    collecting: Option<(Vec<u8>, f64)>,
    acc: u8,
    n: usize,
    /// Sync windows found.
    pub syncs: u64,
    /// Candidates refused (preamble error ratio above 0.42).
    pub refused: u64,
    /// Bits since the carrier settled (the preamble check counts only these).
    pub settled_bits: u64,
}

impl Default for SvissrFramer {
    fn default() -> Self {
        SvissrFramer {
            hist: Vec::with_capacity(PREAMBLE * 2),
            reg: 0,
            collecting: None,
            acc: 0,
            n: 0,
            syncs: 0,
            refused: 0,
            settled_bits: 0,
        }
    }
}

impl SvissrFramer {
    /// Collecting a frame now.
    pub fn collecting(&self) -> bool {
        self.collecting.is_some()
    }
    /// Runs bits, appending found frames.
    pub fn process(&mut self, bits: &[u8], out: &mut Vec<FoundFrame>) {
        let pre = &pn_bits()[..PREAMBLE];
        for &b in bits {
            self.settled_bits += 1;
            if let Some((buf, _)) = &mut self.collecting {
                self.acc = (self.acc << 1) | b;
                self.n += 1;
                if self.n.is_multiple_of(8) {
                    buf.push(self.acc);
                    self.acc = 0;
                    if buf.len() == FRAME_BYTES {
                        let (mut f, ber) = self.collecting.take().expect("collecting");
                        scramble(&mut f);
                        out.push(FoundFrame {
                            bytes: f,
                            preamble_ber: ber,
                        });
                        self.hist.clear();
                        self.reg = 0;
                    }
                }
                continue;
            }
            self.hist.push(b);
            if self.hist.len() > 2 * PREAMBLE {
                self.hist.drain(..PREAMBLE);
            }
            self.reg = (self.reg << 1) | b as u64;
            if self.hist.len() >= 64 && (self.reg ^ SYNC_WINDOW).count_ones() <= 12 {
                self.syncs += 1;
                // Compare the bits behind with the preamble (only those since the carrier settled).
                let have = self
                    .hist
                    .len()
                    .min(PREAMBLE)
                    .min(self.settled_bits as usize);
                let base = self.hist.len() - have;
                let errs = self.hist[base..]
                    .iter()
                    .zip(&pre[PREAMBLE - have..])
                    .filter(|(a, b)| a != b)
                    .count();
                let ber = errs as f64 / have.max(1) as f64;
                if ber <= 0.42 {
                    self.collecting = Some((Vec::with_capacity(FRAME_BYTES), ber));
                    self.n = 0;
                    self.acc = 0;
                } else {
                    self.refused += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preamble_ends_with_the_sync_window() {
        let pn = pn_bits();
        let w = pn[PREAMBLE - 64..PREAMBLE]
            .iter()
            .fold(0u64, |a, &b| (a << 1) | b as u64);
        assert_eq!(w, SYNC_WINDOW);
        assert_eq!(scramble_table().len(), FRAME_BYTES);
        let ones: u32 = scramble_table().iter().map(|b| b.count_ones()).sum();
        let frac = ones as f64 / (FRAME_BYTES * 8) as f64;
        assert!((0.45..=0.55).contains(&frac), "{frac}");
    }

    #[test]
    fn frames_round_trip() {
        let l = SvissrLine {
            counter: 1234,
            ir: std::array::from_fn(|k| {
                (0..IR_SAMPLES)
                    .map(|i| ((i + k * 40) % 256) as u8)
                    .collect()
            }),
            ir4: (0..IR_SAMPLES).map(|i| (i % 1024) as u16).collect(),
            vis: std::array::from_fn(|j| (0..VIS_SAMPLES).map(|i| ((i + j) % 64) as u8).collect()),
            spare_ones: 0,
        };
        let f = build(&l);
        let p = parse(&f);
        assert_eq!(p, l);
        let mut s = f.clone();
        scramble(&mut s);
        assert_ne!(s, f);
        scramble(&mut s);
        assert_eq!(s, f);
        assert_eq!(f[2114..2293].len() * 8, 1432);
    }
}
