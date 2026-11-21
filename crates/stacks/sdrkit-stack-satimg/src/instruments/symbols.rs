//! The `eodb` instruments without CADUs, which take the soft symbols of the demodulator (LLRs, positive for 0, I then Q for
//! QPSK): Oceansat-2 OCM, Kanopus-V PSS and MSS, and DMSP OLS. No public document confirms these formats.

use super::planes::{bits_at, test_pixel, ChannelSpec, Planes};
use sdrkit_proto_ccsds::quaddiff::{QuadDiffDecoder, QuadDiffEncoder};
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};

/// Counters of a symbol instrument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolStats {
    /// Frames found.
    pub frames: u64,
    /// Sync words found.
    pub sync_hits: u64,
    /// Frames (or PMSS frames) that went into a picture.
    pub used: u64,
    /// The frame synchroniser holds its lock.
    pub locked: bool,
}

/// An instrument that takes soft symbols.
pub trait SymbolInstrument: Send {
    /// Takes soft symbols.
    fn push(&mut self, llrs: &[i8]);
    /// Ends the pass.
    fn finish(&mut self) {}
    /// The channel pictures.
    fn planes(&self) -> &Planes;
    /// The channel pictures, to take strips.
    fn planes_mut(&mut self) -> &mut Planes;
    /// The counters.
    fn stats(&self) -> SymbolStats;
    /// Starts new pictures.
    fn clear(&mut self) {
        self.planes_mut().clear();
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Oceansat-2 OCM.

/// Bytes of an OCM frame.
pub const OCM_FRAME: usize = 92_220;
/// The 34-bit sync word.
pub const OCM_SYNC: u64 = 0b00_0000_0111_1110_1111_1000_0100_1100_1101;
/// The first byte of the picture data.
pub const OCM_PICTURE_AT: usize = 16_351;
/// Pixel groups of a line.
pub const OCM_WIDTH: usize = 4072;

/// The OCM randomiser bytes M(0) to M(2046): s(n) = s(n - 9) XOR s(n - 11), first bits 10100100110, packed MSB first.
pub fn ocm_pn() -> Vec<u8> {
    let mut s: Vec<u8> = [1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0].to_vec();
    while s.len() < 2047 * 8 {
        let n = s.len();
        s.push(s[n - 9] ^ s[n - 11]);
    }
    s.chunks(8).map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b)).collect()
}

/// The mask of frame byte `i`: NOT M((i - 59) mod 2047).
pub fn ocm_mask(pn: &[u8], i: usize) -> u8 {
    !pn[(i + 2047 * 64 - 59) % 2047]
}

/// The quadrant of a symbol from its hard rails: 0 (I > 0, Q > 0), 1, 2, 3 counter-clockwise.
pub fn quadrant(i_pos: bool, q_pos: bool) -> u8 {
    match (i_pos, q_pos) {
        (true, true) => 0,
        (false, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    }
}

/// Oceansat-2 OCM: 8 channels of 4072 columns, one row a frame.
pub struct Ocm {
    planes: Planes,
    syncs: [WordSync; 2],
    prev: [u8; 2],
    bits: [Vec<u8>; 2],
    chosen: Option<usize>,
    pn: Vec<u8>,
    found: Vec<Vec<u8>>,
    stats: SymbolStats,
}

impl Ocm {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        const NAMES: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];
        const UM: [f64; 8] = [0.412, 0.443, 0.49, 0.51, 0.556, 0.62, 0.74, 0.865];
        let specs: Vec<ChannelSpec> = NAMES.iter().zip(UM).map(|(&n, um)| (n, um, "OCM", OCM_WIDTH)).collect();
        let sync = || {
            WordSync::new(WordSyncConfig {
                word: OCM_SYNC,
                word_bits: 34,
                frame_bits: OCM_FRAME * 8,
                acquire_errors: 2,
                track_errors: 6,
                complement: false,
                max_misses: 3,
            })
            .unwrap_or_else(|_| unreachable!("constant rules"))
        };
        Ocm {
            planes: Planes::new(&specs, max_rows),
            syncs: [sync(), sync()],
            prev: [0; 2],
            bits: [Vec::new(), Vec::new()],
            chosen: None,
            pn: ocm_pn(),
            found: Vec::new(),
            stats: SymbolStats::default(),
        }
    }

    fn frame(&mut self, f: &[u8]) {
        self.stats.frames += 1;
        if f.len() < OCM_PICTURE_AT + OCM_WIDTH * 15 {
            return;
        }
        let row = self.planes.lines();
        let pn = &self.pn;
        for c in 0..8 {
            let Some(r) = self.planes.pixels_mut(c, row) else { return };
            for (x, o) in r.iter_mut().enumerate() {
                let at = OCM_PICTURE_AT + 15 * x;
                // Two 12-bit words in three bytes: word c of the group starts at byte at + 3 (c / 2).
                let b = at + 3 * (c / 2);
                let (b0, b1, b2) = (f[b] ^ ocm_mask(pn, b), f[b + 1] ^ ocm_mask(pn, b + 1), f[b + 2] ^ ocm_mask(pn, b + 2));
                let v = if c % 2 == 0 {
                    u16::from(b0) << 4 | u16::from(b1 >> 4)
                } else {
                    u16::from(b1 & 0x0F) << 8 | u16::from(b2)
                };
                *o = v * 16;
            }
        }
        self.stats.used += 1;
    }
}

impl SymbolInstrument for Ocm {
    fn push(&mut self, llrs: &[i8]) {
        for b in &mut self.bits {
            b.clear();
        }
        for p in llrs.as_chunks::<2>().0 {
            let v = quadrant(p[0] < 0, p[1] < 0);
            let w = ((v & 1) << 1) | (v >> 1);
            for (k, q) in [v, w].into_iter().enumerate() {
                let d = q.wrapping_sub(self.prev[k]) & 3;
                self.prev[k] = q;
                self.bits[k].push(d >> 1);
            }
        }
        for k in 0..2 {
            if self.chosen.is_some_and(|c| c != k) {
                continue;
            }
            let found = &mut self.found;
            self.syncs[k].push_bits(&self.bits[k], |f| found.push(f.bytes.to_vec()));
            if self.syncs[k].locked() {
                self.chosen = Some(k);
            } else if self.chosen == Some(k) {
                self.chosen = None;
            }
            if self.chosen == Some(k) || self.chosen.is_none() {
                let found = std::mem::take(&mut self.found);
                for f in &found {
                    self.frame(f);
                }
            }
            self.found.clear();
        }
        self.stats.sync_hits = self.syncs.iter().map(|s| s.stats().sync_hits).sum();
        self.stats.locked = self.chosen.is_some();
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn stats(&self) -> SymbolStats {
        self.stats
    }
}

/// The OCM frame of test line `y`: the 8 channels of the test picture, randomised, behind the sync word.
pub fn ocm_test_frame(y: usize, pn: &[u8]) -> Vec<u8> {
    let mut f = vec![0u8; OCM_FRAME];
    for x in 0..OCM_WIDTH {
        let at = OCM_PICTURE_AT + 15 * x;
        for c in (0..8).step_by(2) {
            let (a, b) = (test_pixel(c, x, y), test_pixel(c + 1, x, y));
            let p = at + 3 * (c / 2);
            f[p] = (a >> 4) as u8;
            f[p + 1] = ((a & 0x0F) << 4) as u8 | (b >> 8) as u8;
            f[p + 2] = b as u8;
        }
    }
    for (i, b) in f.iter_mut().enumerate() {
        *b ^= ocm_mask(pn, i);
    }
    // The sync word over the first 34 bits.
    let s = OCM_SYNC << 30;
    for k in 0..34 {
        let bit = ((s >> (63 - k)) & 1) as u8;
        f[k / 8] = (f[k / 8] & !(0x80 >> (k % 8))) | (bit << (7 - k % 8));
    }
    f
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Kanopus-V.

/// Bits of an outer frame.
pub const KANOPUS_FRAME_BITS: usize = 24_576;
/// Bytes of a set of an outer frame.
pub const KANOPUS_SET: usize = 384;
/// The sync bytes at set bytes 0, 48, 96 and 144 of sets 0 and 1.
pub const KANOPUS_SYNC: [u8; 4] = [0x04, 0xB3, 0xE3, 0x75];
/// The derandomiser of each set, repeated 8 times.
pub const KANOPUS_PN: [u8; 48] = [
    0x55, 0xE2, 0xA4, 0x6A, 0x30, 0xA7, 0xDA, 0x16, 0x24, 0x86, 0x39, 0x07, 0x0D, 0x1D, 0xEC, 0x64, 0xF0, 0x3D, 0xD7, 0x66,
    0x98, 0x08, 0x15, 0x94, 0xA0, 0xBA, 0x5B, 0x22, 0x3E, 0x67, 0x40, 0x1B, 0x54, 0x3A, 0xB7, 0x2B, 0x9E, 0xB0, 0x4B, 0xD3,
    0xB6, 0xF3, 0x8D, 0xF1, 0xE5, 0xC4, 0x27, 0x36,
];
/// The order in which the sets give their bytes.
pub const KANOPUS_ORDER: [usize; 8] = [1, 0, 3, 2, 5, 4, 7, 6];
/// Bytes of a PMSS frame.
pub const PMSS_FRAME: usize = 1960;
/// The 16-byte start of a PMSS frame.
pub const PMSS_SYNC: [u8; 16] = [0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A, 0xF5, 0x0A];
/// Lines of a sensor scene.
pub const PMSS_LINES: usize = 495;
/// Columns kept of a PMSS line.
pub const KANOPUS_WIDTH: usize = 1920;

/// The place in the outer frame bits of set `s`, set byte `b`, bit `k` (0 the most significant).
#[inline]
fn kanopus_bit(s: usize, b: usize, k: usize) -> usize {
    64 * b + 8 * k + s
}

/// Wrong bits of the sync bytes of the outer frame starting at `bits[p..]`.
fn kanopus_errors(bits: &[u8], p: usize, limit: u32) -> u32 {
    let mut e = 0;
    for s in 0..2 {
        for (j, &w) in KANOPUS_SYNC.iter().enumerate() {
            for k in 0..8 {
                let want = (w >> (7 - k)) & 1;
                e += u32::from(bits[p + kanopus_bit(s, 48 * j, k)] != want);
                if e > limit {
                    return e;
                }
            }
        }
    }
    e
}

/// The 3056 bytes of the stream of an outer frame (its bits from `bits[p..]`).
fn kanopus_stream(bits: &[u8], p: usize, out: &mut Vec<u8>) {
    let mut sets = [[0u8; KANOPUS_SET]; 8];
    for (s, set) in sets.iter_mut().enumerate() {
        for (b, byte) in set.iter_mut().enumerate() {
            let mut v = 0u8;
            for k in 0..8 {
                v = (v << 1) | bits[p + kanopus_bit(s, b, k)];
            }
            *byte = v ^ KANOPUS_PN[b % 48];
        }
    }
    for g in 0..8 {
        for k in 0..48 {
            for &s in &KANOPUS_ORDER {
                if k == 0 && s < 2 {
                    continue;
                }
                out.push(sets[s][48 * g + k]);
            }
        }
    }
}

/// Kanopus-V PSS (6 swaths) and MSS (4 bands), 1920 columns, in scenes of 463 lines.
pub struct Kanopus {
    planes: Planes,
    dec: QuadDiffDecoder,
    bits: Vec<u8>,
    outer: Option<usize>,
    misses: u32,
    stream: Vec<u8>,
    pmss_locked: bool,
    scene: Vec<u8>,
    sensors: [u32; 16],
    last: Option<usize>,
    in_step: bool,
    mismatches: u32,
    stats: SymbolStats,
}

impl Kanopus {
    /// A decoder of at most `max_rows` lines a channel.
    pub fn new(max_rows: usize) -> Self {
        let mut specs: Vec<ChannelSpec> = Vec::new();
        for n in ["PSS 1", "PSS 2", "PSS 3", "PSS 4", "PSS 5", "PSS 6"] {
            specs.push((n, 0.0, "Kanopus PSS", KANOPUS_WIDTH));
        }
        for n in ["MSS 1", "MSS 2", "MSS 3", "MSS 4"] {
            specs.push((n, 0.0, "Kanopus MSS", KANOPUS_WIDTH));
        }
        Kanopus {
            planes: Planes::new(&specs, max_rows),
            dec: QuadDiffDecoder::second_inverted(),
            bits: Vec::new(),
            outer: None,
            misses: 0,
            stream: Vec::new(),
            pmss_locked: false,
            scene: vec![0; PMSS_LINES * PMSS_FRAME],
            sensors: [0; 16],
            last: None,
            in_step: false,
            mismatches: 0,
            stats: SymbolStats::default(),
        }
    }

    /// Finds and takes the outer frames of the bit buffer.
    fn outer_frames(&mut self) {
        let span = kanopus_bit(1, 144, 7) + 1;
        let mut p = 0;
        let mut out = Vec::new();
        loop {
            match self.outer {
                None => {
                    // The exact search.
                    let mut hit = None;
                    while p + span <= self.bits.len() {
                        if kanopus_errors(&self.bits, p, 0) == 0 {
                            hit = Some(p);
                            break;
                        }
                        p += 1;
                    }
                    let Some(h) = hit else { break };
                    self.stats.sync_hits += 1;
                    self.outer = Some(h);
                    self.misses = 0;
                    p = h;
                }
                Some(at) => {
                    if at + KANOPUS_FRAME_BITS > self.bits.len() || at + span > self.bits.len() {
                        p = at;
                        break;
                    }
                    if kanopus_errors(&self.bits, at, 9) < 10 {
                        self.stats.sync_hits += 1;
                        self.stats.frames += 1;
                        self.misses = 0;
                        out.clear();
                        kanopus_stream(&self.bits, at, &mut out);
                        self.stream.extend_from_slice(&out);
                        self.outer = Some(at + KANOPUS_FRAME_BITS);
                    } else {
                        self.misses += 1;
                        self.outer = (self.misses <= 10).then_some(at + 1);
                        if self.outer.is_none() {
                            p = at + 1;
                        }
                    }
                }
            }
        }
        let keep_from = self.outer.unwrap_or(p).min(self.bits.len());
        self.bits.drain(..keep_from);
        if let Some(a) = &mut self.outer {
            *a -= keep_from;
        }
        self.stats.locked = self.outer.is_some();
    }

    /// Finds and takes the PMSS frames of the byte stream.
    fn pmss_frames(&mut self) {
        let mut p = 0;
        while p + PMSS_FRAME <= self.stream.len() {
            let wrong: u32 = self.stream[p..p + 16]
                .iter()
                .zip(&PMSS_SYNC)
                .map(|(a, b)| (a ^ b).count_ones())
                .sum();
            let ok = if self.pmss_locked { wrong < 8 } else { wrong == 0 };
            if !ok {
                self.pmss_locked = false;
                p += 1;
                continue;
            }
            self.pmss_locked = true;
            let f: Vec<u8> = self.stream[p..p + PMSS_FRAME].to_vec();
            self.pmss(&f);
            p += PMSS_FRAME;
        }
        self.stream.drain(..p);
    }

    fn pmss(&mut self, f: &[u8]) {
        self.stats.used += 1;
        let sensor = usize::from(f[1944] & 0x0F);
        let counter = ((usize::from(f[1946] & 0x0F) << 8) | usize::from(f[1945])) / 2;
        if counter >= PMSS_LINES {
            return;
        }
        let idx = match (self.in_step, self.last) {
            (false, Some(l)) if counter == (l + 1) % PMSS_LINES => {
                self.in_step = true;
                self.mismatches = 0;
                counter
            }
            (true, Some(l)) => {
                let want = (l + 1) % PMSS_LINES;
                if counter == want {
                    self.mismatches = 0;
                } else {
                    self.mismatches += 1;
                    if self.mismatches > 2 {
                        self.in_step = false;
                    }
                }
                want
            }
            _ => counter,
        };
        self.last = Some(idx);
        if !self.in_step {
            return;
        }
        self.scene[idx * PMSS_FRAME..(idx + 1) * PMSS_FRAME].copy_from_slice(f);
        self.sensors[sensor] += 1;
        if idx == PMSS_LINES - 1 {
            self.end_scene();
        }
    }

    fn end_scene(&mut self) {
        let (best, n) = self
            .sensors
            .iter()
            .enumerate()
            .max_by_key(|(_, n)| **n)
            .map_or((0, 0), |(i, n)| (i, *n));
        self.sensors = [0; 16];
        if n == 0 || !(1..=10).contains(&best) {
            return;
        }
        let ch = best - 1;
        let first = self.planes.planes[ch].rows;
        for (k, line) in (30..=492).enumerate() {
            let src = &self.scene[line * PMSS_FRAME..];
            if let Some(r) = self.planes.pixels_mut(ch, first + k) {
                for (x, o) in r.iter_mut().enumerate() {
                    *o = u16::from(src[16 + 4 + x]) * 256;
                }
            }
        }
    }
}

impl SymbolInstrument for Kanopus {
    fn push(&mut self, llrs: &[i8]) {
        for p in llrs.as_chunks::<2>().0 {
            let (x, y) = (u8::from(p[0] < 0), u8::from(p[1] < 0));
            let (a, b) = self.dec.decode(y, x);
            self.bits.push(a);
            self.bits.push(b);
        }
        self.outer_frames();
        self.pmss_frames();
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn stats(&self) -> SymbolStats {
        self.stats
    }
}

/// The 8-bit test value of Kanopus channel `ch` (0 to 9) at column `x` and picture line `y`.
pub fn kanopus_test_value(ch: usize, x: usize, y: usize) -> u8 {
    (test_pixel(ch, x, y) >> 4) as u8
}

/// The test bit stream of Kanopus-V: PMSS frames of the sensors 1 to 10 in turn, a scene of 495 lines each, in outer frames.
#[derive(Default)]
pub struct KanopusSource {
    stream: Vec<u8>,
    sensor: usize,
    counter: usize,
    scenes: [usize; 10],
    enc: Option<QuadDiffEncoder>,
}

impl KanopusSource {
    fn pmss(&mut self) {
        let ch = self.sensor;
        let mut f = vec![0u8; PMSS_FRAME];
        f[..16].copy_from_slice(&PMSS_SYNC);
        let line = self.counter;
        if (30..=492).contains(&line) {
            let y = self.scenes[ch] * 463 + line - 30;
            for x in 0..KANOPUS_WIDTH {
                f[20 + x] = kanopus_test_value(ch, x, y);
            }
        }
        f[1944] = (ch + 1) as u8;
        let c2 = (2 * line) as u16;
        f[1945] = c2 as u8;
        f[1946] = (c2 >> 8) as u8;
        self.stream.extend_from_slice(&f);
        self.counter += 1;
        if self.counter == PMSS_LINES {
            self.counter = 0;
            self.scenes[ch] += 1;
            self.sensor = (self.sensor + 1) % 10;
        }
    }

    /// Appends the channel bits of the next outer frame: I then Q of each symbol.
    pub fn next_frame(&mut self, out: &mut Vec<u8>) {
        while self.stream.len() < 3056 {
            self.pmss();
        }
        let mut sets = [[0u8; KANOPUS_SET]; 8];
        let mut it = self.stream.drain(..3056);
        for g in 0..8 {
            for k in 0..48 {
                for &s in &KANOPUS_ORDER {
                    if k == 0 && s < 2 {
                        continue;
                    }
                    sets[s][48 * g + k] = it.next().unwrap_or(0);
                }
            }
        }
        drop(it);
        for set in sets.iter_mut() {
            for (b, v) in set.iter_mut().enumerate() {
                *v ^= KANOPUS_PN[b % 48];
            }
        }
        for set in sets.iter_mut().take(2) {
            for (j, &w) in KANOPUS_SYNC.iter().enumerate() {
                set[48 * j] = w;
            }
        }
        let mut bits = vec![0u8; KANOPUS_FRAME_BITS];
        for (s, set) in sets.iter().enumerate() {
            for (b, &v) in set.iter().enumerate() {
                for k in 0..8 {
                    bits[kanopus_bit(s, b, k)] = (v >> (7 - k)) & 1;
                }
            }
        }
        let enc = self.enc.get_or_insert_with(QuadDiffEncoder::second_inverted);
        for p in bits.as_chunks::<2>().0 {
            let (x1, y1) = enc.encode(p[0], p[1]);
            // The receiver reads the pair as (Q, I).
            out.push(y1);
            out.push(x1);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// DMSP OLS.

/// The 13-bit sync word of an OLS frame.
pub const OLS_SYNC: u64 = 0b1_0101_1001_1111;
/// Bits of an OLS frame.
pub const OLS_FRAME_BITS: usize = 150;
/// Columns of an OLS line.
pub const OLS_WIDTH: usize = 7350;
/// Data frames of a line at the most.
pub const OLS_FRAMES: usize = 490;

/// The 8-bit word `i` of an OLS frame: the 8 bits from frame bit 8 i + 6.
pub fn ols_word(f: &[u8], i: usize) -> u8 {
    bits_at(f, 8 * i + 6, 8) as u8
}

/// DMSP OLS: visible and infrared lines of 7350 columns.
pub struct Ols {
    planes: Planes,
    sync: WordSync,
    bits: Vec<u8>,
    found: Vec<Vec<u8>>,
    line: Option<(usize, bool, usize)>,
    frames_since: usize,
    vis: Vec<u16>,
    ir: Vec<u16>,
    stats: SymbolStats,
}

impl Ols {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        let specs: Vec<ChannelSpec> = vec![("VIS", 0.7, "OLS", OLS_WIDTH), ("IR", 11.0, "OLS", OLS_WIDTH)];
        Ols {
            planes: Planes::new(&specs, max_rows),
            sync: WordSync::new(WordSyncConfig {
                word: OLS_SYNC,
                word_bits: 13,
                frame_bits: OLS_FRAME_BITS,
                acquire_errors: 0,
                track_errors: 5,
                complement: true,
                max_misses: 3,
            })
            .unwrap_or_else(|_| unreachable!("constant rules")),
            bits: Vec::new(),
            found: Vec::new(),
            line: None,
            frames_since: usize::MAX,
            vis: vec![0; OLS_WIDTH],
            ir: vec![0; OLS_WIDTH],
            stats: SymbolStats::default(),
        }
    }

    fn end_line(&mut self) {
        if self.line.take().is_none() {
            return;
        }
        let row = self.planes.lines();
        for (ch, src) in [(0, &self.vis), (1, &self.ir)] {
            if let Some(r) = self.planes.pixels_mut(ch, row) {
                for (x, o) in r.iter_mut().enumerate() {
                    *o = src[OLS_WIDTH - 1 - x];
                }
            }
        }
        self.stats.used += 1;
        self.vis.fill(0);
        self.ir.fill(0);
    }

    fn frame(&mut self, f: &[u8]) {
        self.stats.frames += 1;
        let w = |i: usize| ols_word(f, i);
        let matching: u32 = (1..=12)
            .map(|i| 8 - (w(i) ^ if i % 2 == 1 { 0xFB } else { 0x07 }).count_ones())
            .sum();
        if matching > 70 {
            if self.line.is_none() || self.frames_since > 400 {
                self.end_line();
                let reverse = (w(15) >> 5) & 1 == 1;
                let s = usize::from(w(13) & 0xFC) + usize::from(w(14) / 64);
                self.line = Some((s, reverse, 0));
                self.frames_since = 0;
            }
            return;
        }
        self.frames_since = self.frames_since.saturating_add(1);
        let Some((s, reverse, p)) = self.line else { return };
        if (w(1) == 0 && w(2) == 0) || p >= OLS_FRAMES {
            return;
        }
        self.line = Some((s, reverse, p + 1));
        let mut ir = [0u16; 3];
        for (k, v) in ir.iter_mut().enumerate() {
            let a = 1 + 5 * k;
            *v = (0..4).fold(0u16, |acc, j| (acc << 2) | u16::from(w(a + j) & 3));
        }
        for i in 0..15 {
            let base = 15 * p as isize - (s / 10) as isize;
            let col = if reverse {
                OLS_WIDTH as isize - 1 - (base - i as isize)
            } else {
                base + i as isize
            };
            let px = if reverse { 14 - i } else { i };
            if !(1..OLS_WIDTH as isize).contains(&col) {
                continue;
            }
            let c = col as usize;
            self.vis[c] = u16::from(w(1 + px) & 0xFC) * 256;
            self.ir[c] = ir[px / 5] * 256;
        }
    }
}

impl SymbolInstrument for Ols {
    fn push(&mut self, llrs: &[i8]) {
        self.bits.clear();
        self.bits.extend(llrs.iter().map(|&l| u8::from(l < 0)));
        let found = &mut self.found;
        self.sync.push_bits(&self.bits, |f| found.push(f.bytes.to_vec()));
        let found = std::mem::take(&mut self.found);
        for f in &found {
            self.frame(f);
        }
        self.found = found;
        self.found.clear();
        self.stats.sync_hits = self.sync.stats().sync_hits;
        self.stats.locked = self.sync.locked();
    }
    fn finish(&mut self) {
        self.end_line();
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn stats(&self) -> SymbolStats {
        self.stats
    }
}

/// The visible test value (8 bits, the low 2 bits 0) of the turned column `x` of OLS line `y`.
pub fn ols_vis(x: usize, y: usize) -> u8 {
    ((test_pixel(0, x, y) >> 4) as u8 & 0xFC).max(4)
}

/// The infrared test value of data frame `p`, value `k` of OLS line `y`.
pub fn ols_ir(p: usize, k: usize, y: usize) -> u8 {
    (test_pixel(1, 3 * p + k, y) >> 4) as u8
}

/// The test bits of OLS: line after line, a line-start frame and then 490 forward data frames.
#[derive(Default)]
pub struct OlsSource {
    y: usize,
    p: Option<usize>,
}

impl OlsSource {
    /// Appends the bits of the next frame.
    pub fn next_frame(&mut self, out: &mut Vec<u8>) {
        let mut w = [0u8; 18];
        match self.p {
            None => {
                for (i, v) in w.iter_mut().enumerate().take(13).skip(1) {
                    *v = if i % 2 == 1 { 0xFB } else { 0x07 };
                }
                self.p = Some(0);
            }
            Some(p) => {
                for i in 0..15 {
                    let x = (OLS_WIDTH - 1).saturating_sub(15 * p + i);
                    w[1 + i] = ols_vis(x, self.y);
                }
                for k in 0..3 {
                    let v = ols_ir(p, k, self.y);
                    for j in 0..4 {
                        w[1 + 5 * k + j] |= (v >> (6 - 2 * j)) & 3;
                    }
                }
                self.p = (p + 1 < OLS_FRAMES).then_some(p + 1);
                if self.p.is_none() {
                    self.y += 1;
                }
            }
        }
        let start = out.len();
        for k in (0..13).rev() {
            out.push(((OLS_SYNC >> k) & 1) as u8);
        }
        out.push(0);
        for &v in &w[1..18] {
            for k in (0..8).rev() {
                out.push((v >> k) & 1);
            }
        }
        out.truncate(start + OLS_FRAME_BITS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ocm_randomiser_matches_the_computed_bytes() {
        let pn = ocm_pn();
        assert_eq!(pn.len(), 2047);
        assert_eq!(pn[..8], [0xA4, 0xC6, 0xFB, 0xA2, 0xA5, 0x06, 0x23, 0xD5]);
        assert_eq!((59..63).map(|i| ocm_mask(&pn, i)).collect::<Vec<_>>(), [0x5B, 0x39, 0x04, 0x5D]);
        assert_eq!((0..4).map(|i| ocm_mask(&pn, i)).collect::<Vec<_>>(), [0xB0, 0x31, 0xE1, 0x33]);
    }

    #[test]
    fn kanopus_frames_round_trip_through_the_bits() {
        let mut src = KanopusSource::default();
        let mut k = Kanopus::new(2000);
        let mut bits = Vec::new();
        // About 1.2 scenes of PMSS frames.
        for _ in 0..400 {
            bits.clear();
            src.next_frame(&mut bits);
            let llrs: Vec<i8> = bits.iter().map(|&b| if b == 1 { -50 } else { 50 }).collect();
            k.push(&llrs);
        }
        let p = &k.planes().planes[0];
        assert_eq!(p.rows, 463);
        for (x, y) in [(0, 0), (1919, 462), (700, 100)] {
            assert_eq!(p.get(x, y), u16::from(kanopus_test_value(0, x, y)) * 256, "at {x}, {y}");
        }
    }

    #[test]
    fn ols_lines_round_trip() {
        let mut src = OlsSource::default();
        let mut o = Ols::new(100);
        let mut bits = Vec::new();
        for _ in 0..(3 * 491 + 2) {
            src.next_frame(&mut bits);
        }
        let llrs: Vec<i8> = bits.iter().map(|&b| if b == 1 { -50 } else { 50 }).collect();
        o.push(&llrs);
        o.finish();
        let p = &o.planes().planes[0];
        assert!(p.rows >= 2, "{} lines", p.rows);
        for x in [7349 - 15, 3000, 7349 - 15 * 489 - 14] {
            assert_eq!(p.get(x, 1), u16::from(ols_vis(x, 1)) * 256, "visible at {x}");
        }
    }
}
