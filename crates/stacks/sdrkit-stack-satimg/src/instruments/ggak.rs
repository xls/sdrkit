//! Elektro-L GGAK telemetry frames: 224 bytes on the CCSDS marker 1ACFFC1D, with no code, no Reed-Solomon and no
//! randomiser, each frame sent twice.

use crate::items::GgakFrame;
use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
use sdrkit_proto_ccsds::marker;
use sdrkit_util_bits::crc::catalog::CRC16_CMS;

/// Bytes in a frame, the marker included.
pub const FRAME_LEN: usize = 224;

/// The check of a frame: CRC-16/CMS of bytes 4 to 221.
pub fn crc(frame: &[u8]) -> u16 {
    CRC16_CMS.checksum(&frame[4..222])
}

/// Takes a frame apart (the marker first). If the frame is not 224 bytes long, it returns `None`.
pub fn parse(bytes: &[u8], inverted: bool, time: f64) -> Option<GgakFrame> {
    if bytes.len() != FRAME_LEN {
        return None;
    }
    let crc_ok = crc(bytes) == u16::from_be_bytes([bytes[222], bytes[223]]);
    let mut ts = [0u8; 8];
    ts.copy_from_slice(&bytes[5..13]);
    let mut field = Vec::with_capacity(13);
    for k in 0..13 {
        let o = 13 + k * 6;
        let v = |i: usize| i16::from_be_bytes([bytes[o + i], bytes[o + i + 1]]);
        field.push([v(0), v(2), v(4)]);
    }
    Some(GgakFrame {
        bytes: bytes.to_vec(),
        marker: bytes[4] >> 4,
        subframe: bytes[4] & 15,
        timestamp: u64::from_be_bytes(ts),
        field,
        crc_ok,
        inverted,
        time,
    })
}

/// Builds a frame (the generator's side).
pub fn build(marker_nibble: u8, subframe: u8, timestamp: u64, field: &[[i16; 3]; 13]) -> Vec<u8> {
    let mut f = vec![0u8; FRAME_LEN];
    f[..4].copy_from_slice(marker::TM.bytes());
    f[4] = (marker_nibble << 4) | (subframe & 15);
    f[5..13].copy_from_slice(&timestamp.to_be_bytes());
    for (k, t) in field.iter().enumerate() {
        for (i, v) in t.iter().enumerate() {
            f[13 + k * 6 + i * 2..13 + k * 6 + i * 2 + 2].copy_from_slice(&v.to_be_bytes());
        }
    }
    let c = crc(&f);
    f[222..224].copy_from_slice(&c.to_be_bytes());
    f
}

/// |B| of a triple, and whether it is plausible (10 to 65 000).
pub fn magnitude(t: &[i16; 3]) -> (f64, bool) {
    let m = ((t[0] as f64).powi(2) + (t[1] as f64).powi(2) + (t[2] as f64).powi(2)).sqrt();
    (m, (10.0..=65_000.0).contains(&m))
}

/// The frame rules: the marker with at most 2 wrong bits in either polarity, searched again after each missed marker.
pub const SYNC: SyncConfig = SyncConfig {
    acquire_errors: 2,
    track_errors: 2,
    max_misses: 0,
    ..SyncConfig::cadu(FRAME_LEN - 4)
};

/// The frame finder: hard bits in, frames out. The second copy of a frame is counted, not given out.
#[derive(Debug)]
pub struct GgakFramer {
    sync: FrameSync,
    frame: Vec<u8>,
    last: Option<Vec<u8>>,
    /// Frames found (not counting duplicates).
    pub frames: u64,
    /// Good CRCs.
    pub good: u64,
    /// Bad CRCs.
    pub bad: u64,
    /// Duplicates.
    pub duplicates: u64,
}

impl Default for GgakFramer {
    fn default() -> Self {
        Self::new()
    }
}

impl GgakFramer {
    /// A framer that searches.
    pub fn new() -> Self {
        GgakFramer {
            // The rules are constants that FrameSync takes.
            sync: FrameSync::new(SYNC).unwrap_or_else(|_| unreachable!("valid GGAK rules")),
            frame: Vec::with_capacity(FRAME_LEN),
            last: None,
            frames: 0,
            good: 0,
            bad: 0,
            duplicates: 0,
        }
    }

    /// Runs hard bits and appends the new frames.
    pub fn process(&mut self, bits: &[u8], time: f64, out: &mut Vec<GgakFrame>) {
        let frame = &mut self.frame;
        let last = &mut self.last;
        let (frames, good, bad, duplicates) = (
            &mut self.frames,
            &mut self.good,
            &mut self.bad,
            &mut self.duplicates,
        );
        self.sync.push_bits(bits, |f| {
            if f.flywheel {
                return;
            }
            frame.clear();
            frame.extend_from_slice(marker::TM.bytes());
            frame.extend_from_slice(f.bytes);
            if last.as_deref() == Some(&frame[..]) {
                *duplicates += 1;
                *last = None;
                return;
            }
            if let Some(p) = parse(frame, f.inverted, time) {
                if p.crc_ok {
                    *good += 1;
                } else {
                    *bad += 1;
                }
                *frames += 1;
                *last = Some(frame.clone());
                out.push(p);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_duplicates_count_once() {
        let field = [[100i16, -200, 300]; 13];
        let f = build(5, 3, 0x0102_0304_0506_0708, &field);
        let p = parse(&f, false, 0.0).unwrap();
        assert!(p.crc_ok);
        assert_eq!(
            (p.marker, p.subframe, p.timestamp),
            (5, 3, 0x0102_0304_0506_0708)
        );
        assert!(magnitude(&p.field[0]).1);
        let mut bits = vec![0u8; 13];
        for _ in 0..2 {
            for &b in &f {
                for k in (0..8).rev() {
                    bits.push(((b >> k) & 1) ^ 1); // inverted polarity
                }
            }
        }
        let mut g = GgakFramer::default();
        let mut out = Vec::new();
        g.process(&bits, 0.0, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(g.duplicates, 1);
        assert!(out[0].inverted && out[0].crc_ok);
    }
}
