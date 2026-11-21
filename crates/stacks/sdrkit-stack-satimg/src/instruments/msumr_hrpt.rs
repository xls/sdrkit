//! The MSU-MR imager on the HRPT downlink of the Meteor-M satellites: transport frames of 1024 bytes whose four payload parts
//! carry one continuous bit stream, and in that stream the MSU-MR frames of 11 850 bytes, one scan line of six channels each.

use sdrkit_codec_image::strip::Strip;
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
use std::sync::Arc;

/// Bytes of a transport frame.
pub const TRANSPORT_BYTES: usize = 1024;
/// The sync marker of a transport frame.
pub const TRANSPORT_SYNC: u32 = 0x1ACF_FC1D;
/// The payload parts of a transport frame: (first byte, bytes).
pub const PAYLOAD_PARTS: [(usize, usize); 4] = [(22, 238), (278, 238), (534, 238), (790, 234)];
/// Payload bytes of a transport frame.
pub const PAYLOAD_BYTES: usize = 948;
/// Bytes of an MSU-MR frame.
pub const FRAME_BYTES: usize = 11_850;
/// The sync word of an MSU-MR frame.
pub const FRAME_SYNC: u64 = 0x0218_A7A3_92DD_9ABF;
/// Pixels of a line.
pub const WIDTH: usize = 1572;
/// Channels of a line.
pub const CHANNELS: usize = 6;
/// Groups of four pixels a channel.
pub const GROUPS: usize = 393;
/// The first byte of the picture data.
pub const PIXELS_AT: usize = 50;
/// The most rows of the picture.
pub const MAX_ROWS: u32 = 50_000;
/// The instrument name of the strips.
pub const INSTRUMENT: &str = "MSU-MR";

/// The rules of the transport frame synchroniser: the CCSDS marker, 3 wrong bits to acquire, 10 to track, either polarity.
pub const TRANSPORT: WordSyncConfig = WordSyncConfig {
    word: TRANSPORT_SYNC as u64,
    word_bits: 32,
    frame_bits: 8 * TRANSPORT_BYTES,
    acquire_errors: 3,
    track_errors: 10,
    complement: true,
    max_misses: 3,
};

/// The rules of the MSU-MR frame synchroniser: the 64-bit word, 10 wrong bits to acquire, 14 to track, upright only (the
/// transport frames settle the polarity).
pub const MSU_FRAME: WordSyncConfig = WordSyncConfig {
    word: FRAME_SYNC,
    word_bits: 64,
    frame_bits: 8 * FRAME_BYTES,
    acquire_errors: 10,
    track_errors: 14,
    complement: false,
    max_misses: 3,
};

/// Appends the 948 payload bytes of a transport frame to `out`.
pub fn payload(frame: &[u8], out: &mut Vec<u8>) {
    for (at, n) in PAYLOAD_PARTS {
        if let Some(p) = frame.get(at..at + n) {
            out.extend_from_slice(p);
        }
    }
}

/// The four 10-bit pixels of five bytes, most significant bit first.
pub fn unpack4(b: &[u8]) -> [u16; 4] {
    let v = b.iter().take(5).fold(0u64, |v, &x| (v << 8) | u64::from(x));
    [
        (v >> 30) as u16 & 0x3FF,
        (v >> 20) as u16 & 0x3FF,
        (v >> 10) as u16 & 0x3FF,
        v as u16 & 0x3FF,
    ]
}

/// Five bytes of four 10-bit pixels, most significant bit first.
pub fn pack4(p: [u16; 4]) -> [u8; 5] {
    let v = p
        .iter()
        .fold(0u64, |v, &x| (v << 10) | u64::from(x & 0x3FF));
    let b = v.to_be_bytes();
    [b[3], b[4], b[5], b[6], b[7]]
}

/// The 10-bit pixel `x` of channel `c` (0 to 5) of an MSU-MR frame.
pub fn pixel(frame: &[u8], c: usize, x: usize) -> u16 {
    let at = PIXELS_AT + 30 * (x / 4) + 5 * c;
    frame.get(at..at + 5).map_or(0, |b| unpack4(b)[x % 4])
}

/// The MSU-MR frame of test line `y`: the sync, the time of day 10 h + y s in bytes 8 to 10, and pixel x of channel c equal
/// to `value(c, x, y)`.
pub fn test_line(y: u32, value: impl Fn(u16, u32, u32) -> u16) -> Vec<u8> {
    let mut f = vec![0u8; FRAME_BYTES];
    f[..8].copy_from_slice(&FRAME_SYNC.to_be_bytes());
    f[8] = (10 + y / 3600) as u8;
    f[9] = ((y / 60) % 60) as u8;
    f[10] = (y % 60) as u8;
    for g in 0..GROUPS {
        for c in 0..CHANNELS {
            let x = 4 * g as u32;
            let p = [0, 1, 2, 3].map(|i| value(c as u16, x + i, y));
            let at = PIXELS_AT + 30 * g + 5 * c;
            f[at..at + 5].copy_from_slice(&pack4(p));
        }
    }
    f
}

/// The transport frame with frame counter `counter` and the 948 payload bytes `piece`: the marker, the counter in bytes 4 to
/// 6, the payload parts, and 0x55 in every other byte.
pub fn transport_frame(counter: u32, piece: &[u8]) -> [u8; TRANSPORT_BYTES] {
    let mut f = [0x55u8; TRANSPORT_BYTES];
    f[..4].copy_from_slice(&TRANSPORT_SYNC.to_be_bytes());
    f[4..7].copy_from_slice(&counter.to_be_bytes()[1..]);
    let mut at = 0;
    for (start, n) in PAYLOAD_PARTS {
        let from = at.min(piece.len());
        let take = n.min(piece.len() - from);
        f[start..start + take].copy_from_slice(&piece[from..from + take]);
        at += n;
    }
    f
}

/// Counters of [`MsuHrptDecoder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MsuHrptStats {
    /// MSU-MR frames.
    pub frames: u64,
    /// Sync words of MSU-MR frames found.
    pub sync_hits: u64,
    /// Rows of the picture.
    pub rows: u32,
}

/// The payload bit stream to MSU-MR lines: a synchroniser on the 64-bit word, then six strips of 1572 pixels a line, the
/// top 8 bits of each 10-bit pixel. Rows count in the order the lines arrive.
#[derive(Clone, Debug)]
pub struct MsuHrptDecoder {
    sync: Option<WordSync>,
    bits: Vec<u8>,
    hits_before: u64,
    /// Counters.
    pub stats: MsuHrptStats,
}

impl Default for MsuHrptDecoder {
    fn default() -> Self {
        MsuHrptDecoder {
            sync: WordSync::new(MSU_FRAME).ok(),
            bits: Vec::new(),
            hits_before: 0,
            stats: MsuHrptStats::default(),
        }
    }
}

impl MsuHrptDecoder {
    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.stats.rows = 0;
        self.stats.frames = 0;
    }

    /// Ends the lock of the MSU-MR synchroniser: the payload stream has a gap.
    pub fn restart(&mut self) {
        if let Some(s) = &mut self.sync {
            s.drop_lock();
        }
    }

    /// A confirmed lock is held.
    pub fn locked(&self) -> bool {
        self.sync.as_ref().is_some_and(WordSync::locked)
    }

    /// Takes payload bytes in the order they arrived and appends the strips of each complete line to `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<Strip>) {
        let Some(sync) = &mut self.sync else { return };
        self.bits.clear();
        for &b in bytes {
            self.bits.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
        let stats = &mut self.stats;
        sync.push_bits(&self.bits, |f| {
            if stats.rows < MAX_ROWS {
                out.extend(line_strips(f.bytes, stats.rows));
                stats.rows += 1;
            }
            stats.frames += 1;
        });
        let hits = sync.stats().sync_hits;
        self.stats.sync_hits += hits - self.hits_before.min(hits);
        self.hits_before = hits;
    }
}

/// The six strips of the line in MSU-MR frame `f` at row `row`.
fn line_strips(f: &[u8], row: u32) -> Vec<Strip> {
    (0..CHANNELS)
        .map(|c| {
            let px: Vec<u8> = (0..WIDTH).map(|x| (pixel(f, c, x) >> 2) as u8).collect();
            let mut s = Strip::new(c as u16 + 1, row, 0, 1, WIDTH as u32, Arc::from(px));
            s.instrument = INSTRUMENT;
            s
        })
        .collect()
}
