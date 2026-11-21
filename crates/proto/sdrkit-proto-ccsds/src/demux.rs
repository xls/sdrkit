//! Virtual channel demultiplexing and frame counter continuity.
//!
//! Each virtual channel counts its frames (CCSDS 132.0-B-3 clause 4.1.2.5, 732.0-B-4 clause 4.1.2.4). A receiver uses the count
//! to find lost frames: a packet that spans a lost frame is lost too. [`VcCounters`] keeps the last count of each virtual channel
//! (up to 64) in a fixed table and reports for each frame whether it follows in order, follows a gap, or repeats a count.
//!
//! The count widths are 8 bits for TM virtual and master channels, 24 bits for AOS, and 28 bits for AOS with the cycle field in
//! use ([`crate::aos::PrimaryHeader::extended_counter`]).
//!
//! ```
//! use sdrkit_proto_ccsds::demux::{Continuity, VcCounters};
//! let mut c = VcCounters::new(24).unwrap();
//! assert_eq!(c.push(5, 0xFF_FFFE), Continuity::First);
//! assert_eq!(c.push(5, 0xFF_FFFF), Continuity::InOrder);
//! assert_eq!(c.push(5, 0), Continuity::InOrder); // the count wraps
//! assert_eq!(c.push(5, 3), Continuity::Gap { missing: 2 });
//! assert_eq!(c.stats(5).lost, 2);
//! ```

use crate::{Error, Param, Result};

/// How a frame's count relates to the last one of its virtual channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuity {
    /// The first frame seen on this channel (or the first after a reset).
    First,
    /// The next count.
    InOrder,
    /// Counts were skipped: `missing` frames are lost.
    Gap {
        /// Frames lost.
        missing: u32,
    },
    /// The same count again (a repeated frame).
    Repeat,
}

/// Counters of one virtual channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VcStats {
    /// Frames taken.
    pub frames: u64,
    /// Gaps found.
    pub gaps: u64,
    /// Frames lost in the gaps.
    pub lost: u64,
    /// Frames that repeated the last count.
    pub repeats: u64,
    /// The last count.
    pub last: Option<u32>,
}

/// Frame count continuity for up to 64 virtual channels. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct VcCounters {
    bits: u32,
    vcs: [VcStats; 64],
}

impl VcCounters {
    /// Counters for counts of `bits` bits (1 to 32).
    pub fn new(bits: u32) -> Result<Self> {
        if !(1..=32).contains(&bits) {
            return Err(Error::BadParameter(Param::FrameCountBits));
        }
        Ok(VcCounters {
            bits,
            vcs: [VcStats::default(); 64],
        })
    }

    fn modulus_mask(&self) -> u64 {
        (1u64 << self.bits) - 1
    }

    /// Takes the count of a frame of virtual channel `vcid` (0 to 63, higher ids share channel 63).
    pub fn push(&mut self, vcid: u8, count: u32) -> Continuity {
        let mask = self.modulus_mask();
        let count = (count as u64 & mask) as u32;
        let vc = &mut self.vcs[(vcid as usize).min(63)];
        vc.frames += 1;
        let r = match vc.last {
            None => Continuity::First,
            Some(last) => {
                let step = (count as u64).wrapping_sub(last as u64) & mask;
                match step {
                    1 => Continuity::InOrder,
                    0 => {
                        vc.repeats += 1;
                        Continuity::Repeat
                    }
                    s => {
                        vc.gaps += 1;
                        vc.lost += s - 1;
                        Continuity::Gap {
                            missing: (s - 1) as u32,
                        }
                    }
                }
            }
        };
        vc.last = Some(count);
        r
    }

    /// The counters of virtual channel `vcid`.
    pub fn stats(&self, vcid: u8) -> &VcStats {
        &self.vcs[(vcid as usize).min(63)]
    }

    /// The virtual channels seen, with their counters.
    pub fn seen(&self) -> impl Iterator<Item = (u8, &VcStats)> {
        self.vcs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.frames > 0)
            .map(|(i, s)| (i as u8, s))
    }

    /// Forgets the last counts (a retune), so the next frame of each channel is [`Continuity::First`]. The totals stay.
    pub fn restart(&mut self) {
        for v in &mut self.vcs {
            v.last = None;
        }
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.vcs = [VcStats::default(); 64];
    }
}
