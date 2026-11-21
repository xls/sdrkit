//! MSC sub-channel decoding (EN 300 401 clauses 10 to 12): extraction from the CIFs, time
//! de-interleaving with early start and erased CIFs, de-puncturing by the EEP or UEP profile, Viterbi decoding and energy
//! dispersal, giving one logical frame every 24 ms.

use crate::fec::ProfileDecoder;
use crate::tables::{self, Profile, Protection, CIF_BITS, CU_BITS, CU_PER_CIF};
use sdrkit_graph::SlicePool;
use sdrkit_util_bits::interleave::DelayLanes;
use std::sync::Arc;

/// The time interleaver's delays d(i mod 16), in CIFs (EN 300 401 clause 12).
pub const DELAYS: [usize; 16] = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];

/// What a sub-channel carries, as far as decoding goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// DAB+ audio (ASCTy 63).
    DabPlus,
    /// MPEG Layer II audio (ASCTy 0).
    Mp2,
    /// Stream data with this DSCTy (24 = DMB video).
    Stream(u8),
    /// Packet mode data.
    Packet,
}

/// One logical frame of a sub-channel.
#[derive(Clone, Debug)]
pub struct LogicalFrame {
    /// Sub-channel id.
    pub subch: u8,
    /// What it carries.
    pub content: Content,
    /// Bit rate in kbit/s.
    pub bitrate: u16,
    /// The bytes (bit rate × 3).
    pub data: Arc<[u8]>,
    /// More than 3/4 of its soft bits were erased: the bytes are not to be trusted.
    pub erased: bool,
    /// Decoded during the early start (some interleaver positions missing).
    pub early: bool,
    /// Logical frames of this sub-channel since it was started.
    pub index: u64,
    /// Signal time in seconds.
    pub time: f64,
}

/// The geometry a decoder is built for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Start in CU.
    pub start_cu: u16,
    /// Size in CU.
    pub size_cu: u16,
    /// Protection.
    pub protection: Protection,
    /// Bit rate.
    pub bitrate: u16,
}

/// One sub-channel's de-interleaver and decoder.
pub struct SubchannelDecoder {
    /// Sub-channel id.
    pub subch: u8,
    /// Content.
    pub content: Content,
    /// Geometry.
    pub geometry: Geometry,
    profile: Profile,
    bits: usize,
    lanes: DelayLanes<i8>,
    held: usize,
    early_start: bool,
    frame: Vec<i8>,
    decoder: ProfileDecoder,
    pool: SlicePool<u8>,
    /// Logical frames out.
    pub frames: u64,
    /// Erased CIFs put in.
    pub erased_cifs: u64,
}

impl SubchannelDecoder {
    /// A decoder for a sub-channel, or `None` when the geometry cannot be decoded (does not fit the CIF, unknown profile).
    pub fn new(subch: u8, content: Content, g: Geometry, early_start: bool) -> Option<Self> {
        if g.size_cu == 0 || g.start_cu as usize + g.size_cu as usize > CU_PER_CIF {
            return None;
        }
        let profile = tables::profile(g.protection, g.bitrate as usize)?;
        if profile.coded_bits() > g.size_cu as usize * CU_BITS {
            return None;
        }
        let bits = g.size_cu as usize * CU_BITS;
        Some(SubchannelDecoder {
            subch,
            content,
            geometry: g,
            decoder: ProfileDecoder::new(&profile).ok()?,
            profile,
            bits,
            lanes: DelayLanes::dab_time(bits as u32, true, 0).ok()?,
            held: 0,
            early_start: early_start && content == Content::DabPlus,
            frame: vec![0; bits],
            pool: SlicePool::default(),
            frames: 0,
            erased_cifs: 0,
        })
    }

    /// Empties the de-interleaver (after too many lost CIFs).
    pub fn restart(&mut self) {
        self.held = 0;
        self.lanes.reset(0);
    }

    /// Puts in one CIF (55 296 LLRs; `None` for an erased CIF) and decodes the logical frame it completes, if any.
    pub fn push_cif(&mut self, cif: Option<&[i8]>, time: f64) -> Option<LogicalFrame> {
        match cif {
            Some(c) if c.len() == CIF_BITS => {
                let a = self.geometry.start_cu as usize * CU_BITS;
                self.frame.copy_from_slice(&c[a..a + self.bits]);
            }
            _ => {
                self.frame.fill(0);
                self.erased_cifs += 1;
            }
        }
        self.lanes.process(&mut self.frame);
        self.held = (self.held + 1).min(16);
        let need = if self.early_start { 10 } else { 16 };
        if self.held < need {
            return None;
        }
        let early = self.held < 16;
        // A bit whose receive delay (15 - d) reaches back past the CIFs held is still the fill value (an erasure).
        let missing_lanes = DELAYS.iter().filter(|&&d| 15 - d >= self.held).count();
        let missing = missing_lanes * self.bits / 16;
        if early && missing * 8 > self.bits * 3 {
            return None;
        }
        let coded = self.profile.coded_bits();
        let zeros = self.frame[..coded].iter().filter(|&&v| v == 0).count();
        let erased = zeros * 4 > coded * 3;
        let n = self.profile.info_bits / 8;
        let (decoder, frame) = (&mut self.decoder, &self.frame[..coded]);
        let mut ok = true;
        let data = self
            .pool
            .exact(n, |b| ok = decoder.decode(frame, b).is_ok());
        if !ok {
            return None;
        }
        let lf = LogicalFrame {
            subch: self.subch,
            content: self.content,
            bitrate: self.geometry.bitrate,
            data,
            erased,
            early,
            index: self.frames,
            time,
        };
        self.frames += 1;
        Some(lf)
    }
}

/// Counts the CIFs lost between two frames.
#[derive(Clone, Debug, Default)]
pub struct CifCounter {
    last_count: Option<u16>,
    last_start: Option<i64>,
}

/// What the counter decided for a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    /// Consecutive (or the first).
    None,
    /// This many CIFs were lost before this frame.
    Lost(usize),
    /// The frame was already decoded.
    Repeat,
}

impl CifCounter {
    /// Forgets the history.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The gap before a frame starting at `start` (samples at 2.048 Msps) whose first CIF has `count` (when its FIC carried
    /// FIG 0/0).
    pub fn frame(&mut self, start: i64, count: Option<u16>) -> Gap {
        let gap = match (count, self.last_count) {
            (Some(c), Some(l)) => {
                let d = (c as i64 - l as i64 - 4).rem_euclid(5000);
                if (4936..5000).contains(&d) {
                    Gap::Repeat
                } else if d > 2500 {
                    self.by_time(start)
                } else if d == 0 {
                    Gap::None
                } else {
                    Gap::Lost(d as usize)
                }
            }
            _ => self.by_time(start),
        };
        if gap != Gap::Repeat {
            if let Some(c) = count {
                self.last_count = Some(c);
            } else if let Some(l) = self.last_count {
                // Advance the expected count by what this frame covers.
                let lost = if let Gap::Lost(n) = gap { n as u16 } else { 0 };
                self.last_count = Some((l + 4 + lost) % 5000);
            }
            self.last_start = Some(start);
        }
        gap
    }

    fn by_time(&self, start: i64) -> Gap {
        match self.last_start {
            None => Gap::None,
            Some(l) => {
                let d = (start - l) as f64 / tables::FRAME as f64;
                if d < 0.5 {
                    Gap::Repeat
                } else {
                    let n = d.round() as i64 - 1;
                    if n <= 0 {
                        Gap::None
                    } else {
                        Gap::Lost(4 * n as usize)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_finds_gaps_and_repeats() {
        let mut c = CifCounter::default();
        assert_eq!(c.frame(0, Some(100)), Gap::None);
        assert_eq!(c.frame(196_608, Some(104)), Gap::None);
        assert_eq!(c.frame(3 * 196_608, Some(112)), Gap::Lost(4));
        assert_eq!(c.frame(3 * 196_608, Some(112)), Gap::Repeat);
        assert_eq!(c.frame(5 * 196_608, None), Gap::Lost(4));
        assert_eq!(c.frame(5 * 196_608 + 10, None), Gap::Repeat);
    }
}
