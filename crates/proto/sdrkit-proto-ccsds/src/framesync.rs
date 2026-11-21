//! Frame synchronisation on the attached sync marker. [`FrameSync`] finds frames in a stream of hard bits or LLRs (positive
//! means 0): it searches for the marker or its complement, collects the frame, then tracks the following markers and keeps
//! emitting frames through up to `max_misses` missed markers. Each frame is passed to a callback that borrows the buffer.
//!
//! Standard: CCSDS 131.0-B-5 clause 9.

use crate::marker::{self, Marker};
use crate::{Error, Param, Result};
use sdrkit_util_bits::sync::{SoftSync, SyncWord};

/// The rules of a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncConfig {
    /// The marker.
    pub marker: Marker,
    /// Bytes of a frame after the marker (1020 for a 1024-byte CADU).
    pub frame_bytes: usize,
    /// Wrong bits accepted when the search finds a marker.
    pub acquire_errors: u32,
    /// Wrong bits accepted for the next marker of a locked stream.
    pub track_errors: u32,
    /// The lock stops after more than this many missed markers in a row.
    pub max_misses: u32,
    /// The complement of the marker within 3 bits changes the polarity while tracking (geostationary links).
    pub slips: bool,
    /// LLR input only: a normalised soft correlation of at least this value (0 to 1) also finds a marker.
    pub soft_threshold: Option<f32>,
}

impl SyncConfig {
    /// The TM marker before frames of `frame_bytes` bytes: 3 errors to acquire, 10 to track, 4 misses, no slips (the low-orbit
    /// rules of Meteor-M, MetOp and FY-3).
    pub const fn cadu(frame_bytes: usize) -> Self {
        SyncConfig {
            marker: marker::TM,
            frame_bytes,
            acquire_errors: 3,
            track_errors: 10,
            max_misses: 4,
            slips: false,
            soft_threshold: None,
        }
    }

    /// The geostationary rules for 1020-byte frames (GOES, GK-2A, Elektro-L): 3 errors to acquire, 6 to track, 3 misses, slips.
    pub const GEO: SyncConfig = SyncConfig {
        track_errors: 6,
        max_misses: 3,
        slips: true,
        ..SyncConfig::cadu(1020)
    };

    /// The low-orbit rules for 1020-byte frames.
    pub const LEO: SyncConfig = SyncConfig::cadu(1020);
}

/// A frame found by [`FrameSync`]. It borrows the synchroniser's buffers until the callback returns.
#[derive(Debug)]
pub struct Frame<'a> {
    /// The frame bytes after the marker, polarity corrected.
    pub bytes: &'a [u8],
    /// The frame LLRs after the marker, polarity corrected (LLR input only).
    pub llrs: Option<&'a [i8]>,
    /// The marker (and so the frame) arrived inverted.
    pub inverted: bool,
    /// Wrong bits in the marker.
    pub marker_errors: u32,
    /// The marker was not found where expected. The frame comes from the flywheel.
    pub flywheel: bool,
    /// The index (in input bits since construction) of the first marker bit.
    pub start_bit: u64,
}

/// Counters of [`FrameSync`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// Frames given out.
    pub frames: u64,
    /// Searches that found a marker.
    pub acquisitions: u64,
    /// Markers missed while locked.
    pub misses: u64,
    /// Polarity changes while tracking.
    pub slips: u64,
    /// Locks lost.
    pub losses: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Search,
    Collect,
    Track,
}

/// The streaming frame synchroniser. See the [module documentation](self).
///
/// ```
/// use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
/// // Some noise bits, then three frames of 2 bytes, each behind the marker 1ACFFC1D.
/// let mut bytes = vec![0x55u8; 3];
/// for _ in 0..3 {
///     bytes.extend_from_slice(&[0x1A, 0xCF, 0xFC, 0x1D, 0xA5, 0x0F]);
/// }
/// let bits: Vec<u8> = bytes.iter().flat_map(|b| (0..8).rev().map(move |k| (b >> k) & 1)).collect();
/// let mut sync = FrameSync::new(SyncConfig::cadu(2)).unwrap();
/// let mut n = 0;
/// sync.push_bits(&bits, |f| {
///     assert_eq!(f.bytes, &[0xA5, 0x0F]);
///     n += 1;
/// });
/// assert_eq!(n, 3);
/// ```
#[derive(Debug)]
pub struct FrameSync {
    cfg: SyncConfig,
    hard: SyncWord,
    soft: Option<SoftSync>,
    state: State,
    inverted: bool,
    bytes: Vec<u8>,
    llrs: Vec<i8>,
    nbits: usize,
    reg: u128,
    marker_bits: u32,
    marker_errors: u32,
    flywheel: bool,
    misses: u32,
    bit_index: u64,
    start_bit: u64,
    stats: SyncStats,
}

impl FrameSync {
    /// A synchroniser with these rules. Allocates its frame buffers here.
    pub fn new(cfg: SyncConfig) -> Result<Self> {
        if cfg.frame_bytes == 0 || cfg.frame_bytes > 1 << 20 {
            return Err(Error::BadParameter(Param::FrameBytes));
        }
        let half = cfg.marker.matched_bits() / 2;
        if cfg.track_errors >= half {
            return Err(Error::BadParameter(Param::TrackErrors));
        }
        let hard = cfg.marker.sync_word(cfg.acquire_errors, true)?;
        let soft = match cfg.soft_threshold {
            Some(t) => Some(cfg.marker.soft_sync(t, true)?),
            None => None,
        };
        Ok(FrameSync {
            cfg,
            hard,
            soft,
            state: State::Search,
            inverted: false,
            bytes: vec![0; cfg.frame_bytes],
            llrs: vec![0; cfg.frame_bytes * 8],
            nbits: 0,
            reg: 0,
            marker_bits: 0,
            marker_errors: 0,
            flywheel: false,
            misses: 0,
            bit_index: 0,
            start_bit: 0,
            stats: SyncStats::default(),
        })
    }

    /// The rules.
    pub fn config(&self) -> &SyncConfig {
        &self.cfg
    }

    /// The counters.
    pub fn stats(&self) -> &SyncStats {
        &self.stats
    }

    /// True when the synchroniser follows a stream of frames.
    pub fn locked(&self) -> bool {
        self.state != State::Search
    }

    /// The stream polarity is inverted.
    pub fn inverted(&self) -> bool {
        self.inverted
    }

    /// Stops the lock and starts a new search. An upper layer calls this, for example after several frames fail Reed-Solomon.
    pub fn drop_lock(&mut self) {
        if self.state != State::Search {
            self.stats.losses += 1;
        }
        self.state = State::Search;
        self.hard.reset();
        if let Some(s) = self.soft.as_mut() {
            s.reset();
        }
        self.misses = 0;
    }

    /// Takes hard bits (0 or 1). Calls `frame` for each frame found.
    pub fn push_bits(&mut self, bits: &[u8], mut frame: impl FnMut(Frame<'_>)) {
        for &b in bits {
            self.step(b & 1, 0, false, &mut frame);
        }
    }

    /// Takes LLRs (positive means 0). Calls `frame` for each frame found, with the frame's LLRs.
    pub fn push_llrs(&mut self, llrs: &[i8], mut frame: impl FnMut(Frame<'_>)) {
        for &l in llrs {
            self.step((l < 0) as u8, l, true, &mut frame);
        }
    }

    #[inline]
    fn step(&mut self, bit: u8, llr: i8, soft: bool, frame: &mut impl FnMut(Frame<'_>)) {
        self.bit_index += 1;
        match self.state {
            State::Search => {
                let hit = self.hard.push(bit).map(|h| (h.inverted(), h.errors()));
                let soft_hit = match (soft, self.soft.as_mut()) {
                    (true, Some(s)) => s.push(llr).map(|h| h.score < 0.0),
                    _ => None,
                };
                let found = match (hit, soft_hit) {
                    (Some(h), _) => Some(h),
                    (None, Some(inv)) => Some((
                        inv,
                        self.hard
                            .distance()
                            .min(self.cfg.marker.matched_bits() - self.hard.distance()),
                    )),
                    _ => None,
                };
                if let Some((inv, errors)) = found {
                    self.inverted = inv;
                    self.marker_errors = errors;
                    self.flywheel = false;
                    self.stats.acquisitions += 1;
                    self.start_collect();
                }
            }
            State::Collect => {
                let v = bit ^ self.inverted as u8;
                let i = self.nbits;
                let byte = &mut self.bytes[i / 8];
                *byte = (*byte << 1) | v;
                if soft {
                    self.llrs[i] = if self.inverted {
                        llr.saturating_neg().max(-127)
                    } else {
                        llr
                    };
                }
                self.nbits += 1;
                if self.nbits == self.cfg.frame_bytes * 8 {
                    self.emit(soft, frame);
                }
            }
            State::Track => {
                self.reg = (self.reg << 1) | bit as u128;
                self.marker_bits += 1;
                if self.marker_bits < self.cfg.marker.len_bits() {
                    return;
                }
                let m = self.cfg.marker.matched_bits();
                let mask = if m == 128 {
                    u128::MAX
                } else {
                    (1u128 << m) - 1
                };
                let want = if self.inverted {
                    !self.cfg.marker.value()
                } else {
                    self.cfg.marker.value()
                } & mask;
                let e = ((self.reg ^ want) & mask).count_ones();
                let ec = m - e;
                self.flywheel = false;
                if e <= self.cfg.track_errors {
                    self.misses = 0;
                    self.marker_errors = e;
                } else if self.cfg.slips && ec <= 3 {
                    self.inverted = !self.inverted;
                    self.misses = 0;
                    self.marker_errors = ec;
                    self.stats.slips += 1;
                } else {
                    self.misses += 1;
                    self.stats.misses += 1;
                    self.marker_errors = e;
                    self.flywheel = true;
                    if self.misses > self.cfg.max_misses {
                        self.drop_lock();
                        return;
                    }
                }
                self.start_collect();
            }
        }
    }

    fn start_collect(&mut self) {
        self.state = State::Collect;
        self.nbits = 0;
        self.start_bit = self.bit_index - self.cfg.marker.len_bits() as u64;
    }

    fn emit(&mut self, soft: bool, frame: &mut impl FnMut(Frame<'_>)) {
        self.stats.frames += 1;
        frame(Frame {
            bytes: &self.bytes,
            llrs: soft.then_some(&self.llrs[..]),
            inverted: self.inverted,
            marker_errors: self.marker_errors,
            flywheel: self.flywheel,
            start_bit: self.start_bit,
        });
        // The callback can not stop the lock, so a frame always leads to the next marker.
        self.state = State::Track;
        self.reg = 0;
        self.marker_bits = 0;
    }
}
