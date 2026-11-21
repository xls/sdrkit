//! AVHRR lines: MetOp AHRPT space packets (APID 103 for channel 3A, 104 for 3B) and NOAA HRPT minor frames (NOAA KLM
//! User's Guide section 4.1), with line placement by on-board time.

use crate::items::SpacePacket;
use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// Pixels per line.
pub const WIDTH: usize = 2048;
/// Bytes of an AVHRR packet's user data.
pub const PACKET_LEN: usize = 12_960;
/// The most lines kept.
pub const MAX_LINES: u32 = 4000;
/// Milliseconds per line (six lines a second).
pub const LINE_MS: f64 = 1000.0 / 6.0;

/// Reads 10-bit word `k` (most significant bit first) from `data` starting at `byte`.
pub fn word10(data: &[u8], byte: usize, k: usize) -> u16 {
    let bit = byte * 8 + k * 10;
    let mut v = 0u16;
    for i in 0..10 {
        let b = bit + i;
        let byte = data.get(b / 8).copied().unwrap_or(0);
        v = (v << 1) | ((byte >> (7 - b % 8)) & 1) as u16;
    }
    v
}

/// Packs 10-bit words, most significant bit first.
pub fn pack10(words: &[u16]) -> Vec<u8> {
    let mut out = vec![0u8; (words.len() * 10).div_ceil(8)];
    for (k, &w) in words.iter().enumerate() {
        for i in 0..10 {
            if (w >> (9 - i)) & 1 == 1 {
                let b = k * 10 + i;
                out[b / 8] |= 0x80 >> (b % 8);
            }
        }
    }
    out
}

/// One AVHRR line: day, ms of day and the five channels' 10-bit pixels (channel-major).
#[derive(Clone, Debug, PartialEq)]
pub struct AvhrrLine {
    /// Day (since 1958 for MetOp; of the year for NOAA).
    pub day: u16,
    /// Milliseconds of the day.
    pub ms: u32,
    /// 5 x 2048 pixels, 10 bits.
    pub pixels: Vec<u16>,
}

/// Decodes the user data of an AVHRR packet. `None` for a short packet.
pub fn decode_packet(data: &[u8]) -> Option<AvhrrLine> {
    if data.len() < PACKET_LEN {
        return None;
    }
    let day = u16::from_be_bytes([data[0], data[1]]);
    let ms = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
    let mut pixels = vec![0u16; 5 * WIDTH];
    for x in 0..WIDTH {
        for c in 0..5 {
            pixels[c * WIDTH + x] = word10(data, 14, 55 + 5 * x + c);
        }
    }
    Some(AvhrrLine { day, ms, pixels })
}

/// Builds the user data of an AVHRR packet (the generator's side).
pub fn build_packet(day: u16, ms: u32, us: u16, pixels: &[u16]) -> Vec<u8> {
    let mut words = vec![0u16; 55 + 5 * WIDTH];
    for x in 0..WIDTH {
        for c in 0..5 {
            words[55 + 5 * x + c] = pixels[c * WIDTH + x] & 0x3FF;
        }
    }
    let mut d = vec![0u8; PACKET_LEN];
    d[0..2].copy_from_slice(&day.to_be_bytes());
    d[2..6].copy_from_slice(&ms.to_be_bytes());
    d[6..8].copy_from_slice(&us.to_be_bytes());
    let packed = pack10(&words);
    let n = packed.len().min(PACKET_LEN - 14);
    d[14..14 + n].copy_from_slice(&packed[..n]);
    d
}

/// Places lines by on-board time.
#[derive(Clone, Debug, Default)]
pub struct LinePlacer {
    last: Option<(u32, f64)>,
}

impl LinePlacer {
    /// Starts again at line 0.
    pub fn restart(&mut self) {
        self.last = None;
    }
    /// The line of an on-board time (ms of day), or `None` when it is earlier than the previous line.
    pub fn place(&mut self, ms: f64) -> Option<u32> {
        let Some((line, t0)) = self.last else {
            self.last = Some((0, ms));
            return Some(0);
        };
        let mut dt = ms - t0;
        if dt < -43_200_000.0 {
            dt += 86_400_000.0;
        }
        let mut k = (dt / LINE_MS).round();
        if k < 0.0 {
            return None;
        }
        if k > 20_000.0 {
            k = 1.0;
        }
        let l = line + k as u32;
        self.last = Some((l, ms));
        Some(l)
    }
}

/// Counters.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct AvhrrStats {
    /// Packets of APID 103.
    pub apid103: u64,
    /// Packets of APID 104.
    pub apid104: u64,
    /// NOAA minor frames.
    pub noaa_frames: u64,
    /// Other packets.
    pub other: u64,
    /// Lines placed.
    pub lines: u64,
    /// The last on-board time, ms of day.
    pub onboard_ms: Option<u32>,
    /// Lines of the picture.
    pub height: u32,
}

/// Turns AVHRR lines into strips (one per channel and line).
#[derive(Clone, Debug, Default)]
pub struct AvhrrDecoder {
    placer: LinePlacer,
    picture: u32,
    /// Counters.
    pub stats: AvhrrStats,
}

impl AvhrrDecoder {
    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.picture += 1;
        self.placer.restart();
        self.stats.height = 0;
    }
    /// Takes a MetOp packet.
    pub fn push_packet(&mut self, p: &SpacePacket, out: &mut Vec<Strip>) {
        match p.apid {
            103 | 104 => {
                if p.apid == 103 {
                    self.stats.apid103 += 1;
                } else {
                    self.stats.apid104 += 1;
                }
                if let Some(l) = decode_packet(&p.data) {
                    self.line(&l, out);
                }
            }
            _ => self.stats.other += 1,
        }
    }
    /// Takes a decoded line (MetOp or NOAA).
    pub fn line(&mut self, l: &AvhrrLine, out: &mut Vec<Strip>) {
        self.stats.onboard_ms = Some(l.ms);
        let Some(row) = self.placer.place(l.ms as f64) else {
            return;
        };
        if row >= MAX_LINES {
            return;
        }
        self.stats.lines += 1;
        self.stats.height = self.stats.height.max(row + 1);
        for c in 0..5 {
            let px: Vec<u8> = l.pixels[c * WIDTH..(c + 1) * WIDTH]
                .iter()
                .map(|&v| (v >> 2) as u8)
                .collect();
            out.push(Strip {
                picture: self.picture,
                channel: c as u16 + 1,
                first_row: row,
                x: 0,
                rows: 1,
                width: WIDTH as u32,
                samples: Arc::from(px),
                mask: 1,
                onboard_s: Some(l.ms as f64 / 1000.0),
                instrument: "AVHRR",
            });
        }
    }
}

/// The NOAA HRPT frame sync words.
pub const NOAA_SYNC: [u16; 6] = [0x284, 0x16F, 0x35C, 0x19D, 0x20F, 0x095];
/// Words in a minor frame.
pub const NOAA_WORDS: usize = 11_090;
/// The first AVHRR word.
pub const NOAA_IMAGE_WORD: usize = 750;

/// The 60-bit sync as a number (first bit most significant).
pub fn noaa_sync60() -> u64 {
    NOAA_SYNC.iter().fold(0u64, |a, &w| (a << 10) | w as u64)
}

/// Decodes a NOAA minor frame (11 090 words).
pub fn decode_noaa(words: &[u16]) -> Option<AvhrrLine> {
    if words.len() < NOAA_WORDS {
        return None;
    }
    let day = words[8] >> 1;
    let ms = (((words[9] & 0x7F) as u32) << 20) | ((words[10] as u32) << 10) | words[11] as u32;
    let mut pixels = vec![0u16; 5 * WIDTH];
    for x in 0..WIDTH {
        for c in 0..5 {
            pixels[c * WIDTH + x] = words[NOAA_IMAGE_WORD + 5 * x + c];
        }
    }
    Some(AvhrrLine { day, ms, pixels })
}

/// Builds a NOAA minor frame (the generator's side).
pub fn build_noaa(day: u16, ms: u32, pixels: &[u16]) -> Vec<u16> {
    let mut w = vec![0u16; NOAA_WORDS];
    w[..6].copy_from_slice(&NOAA_SYNC);
    w[8] = (day & 0x1FF) << 1;
    w[9] = ((ms >> 20) & 0x7F) as u16;
    w[10] = ((ms >> 10) & 0x3FF) as u16;
    w[11] = (ms & 0x3FF) as u16;
    for x in 0..WIDTH {
        for c in 0..5 {
            w[NOAA_IMAGE_WORD + 5 * x + c] = pixels[c * WIDTH + x] & 0x3FF;
        }
    }
    w
}

/// Finds NOAA minor frames in soft split-phase chips: pairs of chips (first minus second) in both pairings and both polarities,
/// the 60-bit sync accepted with at most 12 wrong bits.
pub struct NoaaFramer {
    chips: u64,
    prev: i8,
    regs: [u64; 2],
    lock: Option<(usize, bool)>,
    words: Vec<u16>,
    acc: u16,
    nbits: u32,
    /// Frames found.
    pub frames: u64,
    /// polarity + 2 * chip phase of the lock.
    pub rotation: u8,
    misses: u32,
    // A whole frame waits here until the sync of the next frame confirms that no slip came inside it.
    pending: Option<Vec<u16>>,
    // The mean size of the chip difference at each chip phase: large at the true phase (the two chips of a bit always differ),
    // about half that at the other (neighbouring bits often agree), so a sync seen at the wrong phase is refused.
    quality: [f32; 2],
}

impl Default for NoaaFramer {
    fn default() -> Self {
        Self::new()
    }
}

impl NoaaFramer {
    /// A framer searching.
    pub fn new() -> Self {
        NoaaFramer {
            chips: 0,
            prev: 0,
            regs: [0; 2],
            lock: None,
            words: Vec::with_capacity(NOAA_WORDS),
            acc: 0,
            nbits: 0,
            frames: 0,
            rotation: 0,
            misses: 0,
            pending: None,
            quality: [0.0; 2],
        }
    }
    /// Drops the lock.
    pub fn reset(&mut self) {
        self.lock = None;
        self.pending = None;
        self.words.clear();
        self.nbits = 0;
    }
    /// The lock is held.
    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }
    /// Runs soft chips, appending whole minor frames.
    pub fn process(&mut self, soft: &[i8], out: &mut Vec<Vec<u16>>) {
        let sync = noaa_sync60();
        let mask = (1u64 << 60) - 1;
        for &c in soft {
            let i = self.chips;
            self.chips += 1;
            let prev = self.prev;
            self.prev = c;
            if i == 0 {
                continue;
            }
            // Chip i completes a bit in phase (i - 1) % 2.
            let phase = ((i - 1) & 1) as usize;
            let v = prev as i32 - c as i32;
            let bit = (v > 0) as u64;
            self.quality[phase] += 0.01 * (v.unsigned_abs() as f32 - self.quality[phase]);
            self.regs[phase] = (self.regs[phase] << 1) | bit;
            match self.lock {
                None => {
                    let r = self.regs[phase] & mask;
                    for inv in [false, true] {
                        let want = if inv { !sync & mask } else { sync };
                        if (r ^ want).count_ones() <= 12
                            && i >= 120
                            && self.quality[phase] >= self.quality[1 - phase]
                        {
                            self.lock = Some((phase, inv));
                            self.rotation = inv as u8 + 2 * phase as u8;
                            self.words.clear();
                            self.words.extend_from_slice(&NOAA_SYNC);
                            self.nbits = 0;
                            self.acc = 0;
                            self.misses = 0;
                            break;
                        }
                    }
                }
                Some((p, inv)) if p == phase => {
                    self.acc = (self.acc << 1) | (bit as u16 ^ inv as u16);
                    self.nbits += 1;
                    if self.nbits == 10 {
                        self.words.push(self.acc & 0x3FF);
                        self.acc = 0;
                        self.nbits = 0;
                        if self.words.len() == NOAA_WORDS {
                            self.frames += 1;
                            self.pending = Some(std::mem::replace(
                                &mut self.words,
                                Vec::with_capacity(NOAA_WORDS),
                            ));
                        } else if self.words.len() == 6 {
                            let got = self.words.iter().fold(0u64, |a, &w| (a << 10) | w as u64);
                            if (got ^ sync).count_ones() <= 12 {
                                // The sync came where expected: the frame before it is whole.
                                self.misses = 0;
                                out.extend(self.pending.take());
                            } else if (got ^ !sync & mask).count_ones() <= 12 {
                                // The carrier turned by half a turn: the polarity changes from here, and the frame before
                                // holds the turn.
                                self.lock = Some((p, !inv));
                                self.rotation = !inv as u8 + 2 * p as u8;
                                for w in self.words.iter_mut() {
                                    *w ^= 0x3FF;
                                }
                                self.pending = None;
                                self.misses = 0;
                            } else {
                                self.pending = None;
                                self.misses += 1;
                                if self.misses > 3 {
                                    self.reset();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noaa_numbers() {
        assert_eq!(NOAA_SYNC, [0x284, 0x16F, 0x35C, 0x19D, 0x20F, 0x095]);
        assert_eq!(noaa_sync60() >> 40, (0x284 << 10) | 0x16F);
        assert_eq!(pack10(&[0x284, 0x16F]), vec![0xA1, 0x16, 0xF0]);
        let px: Vec<u16> = (0..5 * WIDTH).map(|i| (i % 1024) as u16).collect();
        let f = build_noaa(123, 45_678_901, &px);
        assert_eq!(f.len(), NOAA_WORDS);
        let l = decode_noaa(&f).unwrap();
        assert_eq!((l.day, l.ms), (123, 45_678_901));
        assert_eq!(l.pixels, px);
    }

    #[test]
    fn avhrr_packets_round_trip() {
        let px: Vec<u16> = (0..5 * WIDTH).map(|i| ((i * 7) % 1024) as u16).collect();
        let d = build_packet(24_000, 3_600_000, 0, &px);
        assert_eq!(d.len(), PACKET_LEN);
        let l = decode_packet(&d).unwrap();
        assert_eq!((l.day, l.ms), (24_000, 3_600_000));
        assert_eq!(l.pixels[2 * WIDTH + 3], px[2 * WIDTH + 3]);
        assert_eq!(word10(&d, 14, 55 + 5 * 3 + 2), px[2 * WIDTH + 3]);
        assert!(decode_packet(&d[..1000]).is_none());
    }

    fn chips(words: &[u16], invert: bool) -> Vec<i8> {
        let mut out = Vec::new();
        for &w in words {
            for i in (0..10).rev() {
                let b = ((w >> i) & 1 == 1) ^ invert;
                let (a, c) = if b { (60, -60) } else { (-60, 60) };
                out.push(a);
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn frames_found_with_errors_in_either_polarity() {
        let px = vec![512u16; 5 * WIDTH];
        let f = build_noaa(1, 1000, &px);
        for inv in [false, true] {
            let mut stream = vec![10i8; 7];
            let mut f2 = f.clone();
            // 7 wrong sync bits are accepted.
            f2[0] ^= 0x7F;
            stream.extend(chips(&f, inv));
            stream.extend(chips(&f2, inv));
            stream.extend(chips(&f, inv));
            let mut nf = NoaaFramer::new();
            let mut out = Vec::new();
            nf.process(&stream, &mut out);
            assert!(out.len() >= 2, "{inv}");
            assert_eq!(decode_noaa(&out[0]).unwrap().pixels, px);
        }
        // Many errors are refused.
        let mut bad = f.clone();
        for w in bad.iter_mut().take(6) {
            *w ^= 0x155;
        }
        let mut nf = NoaaFramer::new();
        let mut out = Vec::new();
        nf.process(&chips(&bad[..100], false), &mut out);
        assert!(!nf.locked());
    }
}
