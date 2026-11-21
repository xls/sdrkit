//! HDLC framing at the bit level, and byte stuffing.
//!
//! - [`HdlcDeframer`] takes hard bits one at a time (after NRZI decoding) and finds frames between flags (01111110), removes the
//!   stuffed zeros, aborts on seven ones, assembles octets least significant bit first and checks the frame check sequence
//!   itself: each frame it reports carries its bytes and whether the FCS was right. The FCS is any whole-byte catalogue CRC
//!   ([`Engine`]); the default ([`HdlcDeframer::x25`], [`DEFAULT_FCS`]) is the 16-bit FCS of ISO/IEC 13239 clause 4.2.5
//!   (CRC-16/IBM-SDLC, "X.25"), used by AX.25 2.2, AIS (ITU-R M.1371), VDL Mode 2 and PPP (RFC 1662); the 32-bit FCS of the same
//!   clause is [`crate::crc::catalog::CRC32_ISO_HDLC`]. The FCS follows the frame in the CRC's own
//!   order (least significant octet first for these reflected CRCs).
//!   Some links put no FCS in the frame (GOMspace AX100 Mode 6 checks its frames with Reed-Solomon instead):
//!   [`HdlcDeframer::without_fcs`] reports every whole-octet frame of an allowed length.
//! - [`hdlc_encode`] makes the bits of frames with flags, FCS and bit stuffing, and [`hdlc_encode_without_fcs`] the same with
//!   no FCS.
//! - [`ByteStuffing`] escapes and unescapes frames on byte links: PPP (RFC 1662), KISS, and the Beast feed (doubled 0x1A).
//!
//! ```
//! use sdrkit_util_bits::hdlc::{hdlc_encode, HdlcDeframer};
//! use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
//! let mut bits = Vec::new();
//! hdlc_encode(&[b"hello"], &CRC16_IBM_SDLC, 2, &mut bits);
//! let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 400);
//! let mut got = None;
//! for &b in &bits {
//!     if let Some(f) = d.push(b) {
//!         got = Some((d.frame().to_vec(), f.fcs_ok));
//!     }
//! }
//! assert_eq!(got, Some((b"hello".to_vec(), true)));
//! ```

use crate::crc::catalog::{self, Engine};
use crate::crc::Trailer;
use crate::Param;
use crate::{BitOrder, Error, Result};

/// The HDLC flag octet.
pub const FLAG: u8 = 0x7E;

/// The default frame check sequence: the 16-bit FCS of ISO/IEC 13239 clause 4.2.5 (CRC-16/IBM-SDLC, "X.25"; check value 0x906E,
/// good residue 0xF0B8 as RFC 1662 appendix C states).
pub const DEFAULT_FCS: Engine = Engine::U16(&catalog::CRC16_IBM_SDLC);

/// The longest FCS a deframer takes, in bytes (32 bits, the long FCS of ISO/IEC 13239).
const MAX_FCS: usize = 4;

/// What [`HdlcDeframer::push`] reports at the end of a frame. The frame's bytes (without the FCS) are in
/// [`HdlcDeframer::frame`] until the next frame ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameEnd {
    /// The frame's length in bytes, without the FCS.
    pub len: usize,
    /// Whether the FCS was right.
    pub fcs_ok: bool,
}

/// A streaming HDLC deframer. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct HdlcDeframer {
    fcs: Engine,
    fcs_len: usize,
    /// No FCS follows the frame: every frame of an allowed length is good.
    no_fcs: bool,
    min: usize,
    max: usize,
    deliver_bad: bool,
    ones: u32,
    in_frame: bool,
    /// The last data bits not yet committed (they may be the start of a flag), newest in bit 0.
    held: u8,
    nheld: u32,
    octet: u8,
    nbits: u32,
    buf: Vec<u8>,
    /// The last frame reported, with its received FCS after it.
    done: Vec<u8>,
    /// Bytes of `done` before the FCS.
    done_len: usize,
    /// Counters.
    pub stats: HdlcStats,
}

/// Counts kept by the deframer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HdlcStats {
    /// Frames with a good FCS.
    pub good: u64,
    /// Frames of an allowed length with a bad FCS.
    pub bad_fcs: u64,
    /// Frames ended by seven ones, cut for length, or not a whole number of octets.
    pub aborted: u64,
}

impl HdlcDeframer {
    /// A deframer checking frames with `fcs` (a catalogue engine, or a `&'static` [`Crc`](crate::crc::Crc) of any width),
    /// keeping frames of `min` to `max` bytes (without the FCS). Allocates its buffers here; nothing after, including in
    /// [`HdlcDeframer::set_fcs`].
    ///
    /// An FCS that is not whole bytes (or wider than 32 bits) can never match, so every frame then counts as a bad FCS; use
    /// [`HdlcDeframer::try_new`] to have it refused.
    pub fn new(fcs: impl Into<Engine>, min: usize, max: usize) -> Self {
        let fcs = fcs.into();
        HdlcDeframer {
            fcs,
            fcs_len: fcs_len(fcs).unwrap_or(0),
            no_fcs: false,
            min,
            max,
            deliver_bad: false,
            ones: 0,
            in_frame: false,
            held: 0,
            nheld: 0,
            octet: 0,
            nbits: 0,
            buf: Vec::with_capacity(max + MAX_FCS),
            done: Vec::with_capacity(max + MAX_FCS),
            done_len: 0,
            stats: HdlcStats::default(),
        }
    }

    /// As [`HdlcDeframer::new`], but an error when the FCS is not 8, 16, 24 or 32 bits or when `min > max`.
    pub fn try_new(fcs: impl Into<Engine>, min: usize, max: usize) -> Result<Self> {
        let fcs = fcs.into();
        fcs_len(fcs).ok_or(Error::BadParameter(Param::HdlcFcs))?;
        if min > max {
            return Err(Error::BadParameter(Param::HdlcFrameLengths));
        }
        Ok(HdlcDeframer::new(fcs, min, max))
    }

    /// A deframer with the default FCS of ISO/IEC 13239 ([`DEFAULT_FCS`], CRC-16/IBM-SDLC), as AX.25, AIS and VDL Mode 2 use.
    pub fn x25(min: usize, max: usize) -> Self {
        HdlcDeframer::new(DEFAULT_FCS, min, max)
    }

    /// A deframer for links with no FCS in the frame, keeping frames of `min` to `max` bytes. Every whole-octet frame of an
    /// allowed length is reported with `fcs_ok` set to true and counted in [`HdlcStats::good`]. The caller must make sure that
    /// the frame is right by other means (for example a Reed-Solomon code or a CRC inside the frame). An error when
    /// `min > max`. Allocates its buffers here and nothing after.
    ///
    /// ```
    /// use sdrkit_util_bits::hdlc::{hdlc_encode_without_fcs, HdlcDeframer};
    /// let mut bits = Vec::new();
    /// hdlc_encode_without_fcs(&[b"beacon"], 1, &mut bits);
    /// let mut d = HdlcDeframer::without_fcs(1, 255)?;
    /// let ends: Vec<_> = bits.iter().filter_map(|&b| d.push(b)).collect();
    /// assert_eq!(ends.len(), 1);
    /// assert_eq!(d.frame(), b"beacon");
    /// # Ok::<(), sdrkit_util_bits::Error>(())
    /// ```
    pub fn without_fcs(min: usize, max: usize) -> Result<Self> {
        let mut d = HdlcDeframer::try_new(DEFAULT_FCS, min, max)?;
        d.clear_fcs();
        Ok(d)
    }

    /// Stops checking an FCS: from now on no FCS follows a frame (as [`HdlcDeframer::without_fcs`]). A frame in progress is
    /// forgotten. [`HdlcDeframer::set_fcs`] turns the check on again. Never allocates.
    pub fn clear_fcs(&mut self) {
        self.no_fcs = true;
        self.fcs_len = 0;
        self.reset();
    }

    /// True when the deframer checks an FCS, false after [`HdlcDeframer::without_fcs`] or [`HdlcDeframer::clear_fcs`].
    pub fn has_fcs(&self) -> bool {
        !self.no_fcs
    }

    /// The FCS in use. When [`HdlcDeframer::has_fcs`] is false, the deframer does not use this value.
    pub fn fcs(&self) -> Engine {
        self.fcs
    }

    /// Changes the FCS (a frame in progress is forgotten). An error, and no change, when the FCS is not 8, 16, 24 or 32 bits.
    /// Never allocates.
    pub fn set_fcs(&mut self, fcs: impl Into<Engine>) -> Result<()> {
        let fcs = fcs.into();
        self.fcs_len = fcs_len(fcs).ok_or(Error::BadParameter(Param::HdlcFcs))?;
        self.fcs = fcs;
        self.no_fcs = false;
        self.reset();
        Ok(())
    }

    /// The shortest and longest frames kept, in bytes without the FCS.
    pub fn lengths(&self) -> (usize, usize) {
        (self.min, self.max)
    }

    /// Changes the shortest and longest frames kept (a frame in progress is forgotten). An error, and no change, when
    /// `min > max`. Allocates only when `max` grows past what the buffers hold.
    pub fn set_lengths(&mut self, min: usize, max: usize) -> Result<()> {
        if min > max {
            return Err(Error::BadParameter(Param::HdlcFrameLengths));
        }
        (self.min, self.max) = (min, max);
        let need = max + MAX_FCS;
        self.buf.reserve(need.saturating_sub(self.buf.len()));
        self.done.reserve(need.saturating_sub(self.done.len()));
        self.reset();
        Ok(())
    }

    /// Also report frames whose FCS is wrong (for decoders that try to repair them).
    pub fn deliver_bad(mut self, yes: bool) -> Self {
        self.deliver_bad = yes;
        self
    }

    /// Forgets any frame in progress.
    pub fn reset(&mut self) {
        self.ones = 0;
        self.in_frame = false;
        self.start_frame();
        self.in_frame = false;
    }

    fn start_frame(&mut self) {
        self.in_frame = true;
        self.held = 0;
        self.nheld = 0;
        self.octet = 0;
        self.nbits = 0;
        self.buf.clear();
    }

    fn commit(&mut self, bit: u8) {
        self.octet |= bit << self.nbits;
        self.nbits += 1;
        if self.nbits == 8 {
            if self.buf.len() >= self.max + self.fcs_len {
                self.in_frame = false;
                self.stats.aborted += 1;
                return;
            }
            self.buf.push(self.octet);
            self.octet = 0;
            self.nbits = 0;
        }
    }

    fn data_bit(&mut self, bit: u8) {
        if !self.in_frame {
            return;
        }
        // Hold 7 bits back: a flag's first seven bits look like data until its last bit arrives.
        if self.nheld == 7 {
            let oldest = (self.held >> 6) & 1;
            self.commit(oldest);
            if !self.in_frame {
                return;
            }
        } else {
            self.nheld += 1;
        }
        self.held = ((self.held << 1) | bit) & 0x7F;
    }

    fn end_frame(&mut self) -> Option<FrameEnd> {
        if !self.in_frame || (self.buf.is_empty() && self.nbits == 0) {
            return None;
        }
        let f = self.fcs_len;
        if self.nbits != 0 || self.buf.len() < self.min + f || self.buf.len() > self.max + f {
            self.stats.aborted += 1;
            return None;
        }
        let ok = self.no_fcs || (f > 0 && self.fcs.verify(&self.buf, self.fcs.natural_trailer()));
        if ok {
            self.stats.good += 1;
        } else {
            self.stats.bad_fcs += 1;
            if !self.deliver_bad {
                return None;
            }
        }
        let n = self.buf.len() - f;
        self.done.clear();
        self.done.extend_from_slice(&self.buf);
        self.done_len = n;
        Some(FrameEnd { len: n, fcs_ok: ok })
    }

    /// Takes one hard bit (its lowest bit). Returns the end of a frame when a closing flag completes one that has a good FCS (or
    /// any FCS with [`HdlcDeframer::deliver_bad`]) and an allowed length.
    pub fn push(&mut self, bit: u8) -> Option<FrameEnd> {
        if bit & 1 == 1 {
            self.ones += 1;
            if self.ones == 7 && self.in_frame {
                // Abort: seven ones.
                if !self.buf.is_empty() || self.nbits != 0 || self.nheld > 6 {
                    self.stats.aborted += 1;
                }
                self.in_frame = false;
            }
            if self.ones <= 6 {
                self.data_bit(1);
            }
            None
        } else {
            let ones = self.ones;
            self.ones = 0;
            match ones {
                5 => None, // a stuffed zero
                6 => {
                    // A flag: the held bits are the flag's own first seven bits.
                    let r = self.end_frame();
                    self.start_frame();
                    r
                }
                n if n >= 7 => None,
                _ => {
                    self.data_bit(0);
                    None
                }
            }
        }
    }

    /// The bytes of the last frame reported (without the FCS).
    pub fn frame(&self) -> &[u8] {
        &self.done[..self.done_len]
    }

    /// The bytes of the last frame reported with the FCS octets as they arrived after it. A decoder that repairs bad frames
    /// (see [`HdlcDeframer::deliver_bad`]) needs the received FCS. Without an FCS this is the same as [`HdlcDeframer::frame`].
    ///
    /// ```
    /// use sdrkit_util_bits::hdlc::{hdlc_encode, HdlcDeframer, DEFAULT_FCS};
    /// let mut bits = Vec::new();
    /// hdlc_encode(&[b"123456789"], DEFAULT_FCS, 1, &mut bits);
    /// bits[20] ^= 1; // one bit error in the data
    /// let mut d = HdlcDeframer::x25(1, 64).deliver_bad(true);
    /// let end = bits.iter().find_map(|&b| d.push(b)).unwrap();
    /// assert!(!end.fcs_ok);
    /// // The FCS of "123456789" is 0x906E, sent least significant octet first.
    /// assert_eq!(d.frame_with_fcs()[9..], [0x6E, 0x90]);
    /// assert_eq!(d.received_fcs(), &[0x6E, 0x90]);
    /// ```
    pub fn frame_with_fcs(&self) -> &[u8] {
        &self.done
    }

    /// The FCS octets received after the last frame reported, in the order they arrived. Empty when no FCS follows the frame.
    pub fn received_fcs(&self) -> &[u8] {
        &self.done[self.done_len..]
    }
}

/// The FCS length in bytes, when the CRC is 8, 16, 24 or 32 bits.
fn fcs_len(fcs: Engine) -> Option<usize> {
    let w = fcs.spec().width as usize;
    (w.is_multiple_of(8) && (8..=8 * MAX_FCS).contains(&w)).then_some(w / 8)
}

/// Appends `bits` to `out` with a zero inserted after every five ones in a row (the HDLC transparency rule). `ones` carries the
/// count of ones across calls; start it at 0 after a flag.
pub fn stuff_bits(bits: &[u8], ones: &mut u32, out: &mut Vec<u8>) {
    for &b in bits {
        let b = b & 1;
        out.push(b);
        if b == 1 {
            *ones += 1;
            if *ones == 5 {
                out.push(0);
                *ones = 0;
            }
        } else {
            *ones = 0;
        }
    }
}

/// Removes stuffed zeros from bits that hold no flags; an error (index in `bits`) at six or more ones in a row.
pub fn unstuff_bits(bits: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let mut ones = 0;
    for (i, &b) in bits.iter().enumerate() {
        if b & 1 == 1 {
            ones += 1;
            if ones > 5 {
                return Err(Error::Invalid { at: i });
            }
            out.push(1);
        } else {
            if ones != 5 {
                out.push(0);
            }
            ones = 0;
        }
    }
    Ok(())
}

/// Appends the HDLC bits of `frames` to `out`: `flags` flags before the first frame and between frames (at least 1), each frame
/// as its bytes least significant bit first followed by its FCS (low byte first for a reflected CRC), bit-stuffed, and a closing
/// flag. NRZI is not applied.
/// An FCS that is not 8, 16, 24 or 32 bits is left out (nothing could check it).
pub fn hdlc_encode(frames: &[&[u8]], fcs: impl Into<Engine>, flags: usize, out: &mut Vec<u8>) {
    let fcs = fcs.into();
    encode_frames(frames, fcs, fcs_len(fcs).unwrap_or(0), flags, out);
}

/// Appends the HDLC bits of `frames` to `out` as [`hdlc_encode`] does, but with no FCS after each frame (the framing of
/// [`HdlcDeframer::without_fcs`]). NRZI is not applied.
pub fn hdlc_encode_without_fcs(frames: &[&[u8]], flags: usize, out: &mut Vec<u8>) {
    encode_frames(frames, DEFAULT_FCS, 0, flags, out);
}

/// Flags, then each frame with `n` bytes of `fcs` after it, bit-stuffed, then a flag.
fn encode_frames(frames: &[&[u8]], fcs: Engine, n: usize, flags: usize, out: &mut Vec<u8>) {
    let flag = |out: &mut Vec<u8>| crate::pack::unpack(&[FLAG], BitOrder::Lsb, out);
    let flags = flags.max(1);
    let mut bits = Vec::new();
    for _ in 0..flags {
        flag(out);
    }
    for (k, f) in frames.iter().enumerate() {
        if k > 0 {
            for _ in 1..flags {
                flag(out);
            }
        }
        bits.clear();
        crate::pack::unpack(f, BitOrder::Lsb, &mut bits);
        let c = fcs.checksum(f);
        let mut tail = [0u8; MAX_FCS];
        for (i, t) in tail[..n].iter_mut().enumerate() {
            let shift = match fcs.natural_trailer() {
                Trailer::Little => 8 * i,
                Trailer::Big => 8 * (n - 1 - i),
            };
            *t = (c >> shift) as u8;
        }
        crate::pack::unpack(&tail[..n], BitOrder::Lsb, &mut bits);
        let mut ones = 0;
        stuff_bits(&bits, &mut ones, out);
        flag(out);
    }
}

/// How a byte-stuffed link escapes its special bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escape {
    /// The escape byte, then the byte XOR this value (PPP: 0x20).
    Xor(u8),
    /// The escape byte, then a fixed byte for each special byte: (code for the flag, code for the escape) (KISS: 0xDC, 0xDD).
    Codes(u8, u8),
    /// The flag byte is doubled; there is no escape byte (Beast: 0x1A).
    Doubled,
}

/// A byte-stuffing scheme: a frame flag, an escape byte and the escape rule. [`ByteStuffing::PPP`], [`ByteStuffing::KISS`] and
/// [`ByteStuffing::BEAST`] are the three in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteStuffing {
    /// The frame delimiter.
    pub flag: u8,
    /// The escape byte (unused for [`Escape::Doubled`]).
    pub esc: u8,
    /// The rule.
    pub escape: Escape,
}

impl ByteStuffing {
    /// PPP in HDLC-like framing (RFC 1662 clause 4.2): flag 0x7E, escape 0x7D, escaped bytes XOR 0x20. Control characters below
    /// 0x20 are not escaped (an all-zero ACCM). HD Radio AAS uses the same rule.
    pub const PPP: ByteStuffing = ByteStuffing {
        flag: 0x7E,
        esc: 0x7D,
        escape: Escape::Xor(0x20),
    };

    /// KISS (Chepponis and Karn, "The KISS TNC", 1987): FEND 0xC0, FESC 0xDB, TFEND 0xDC, TFESC 0xDD.
    pub const KISS: ByteStuffing = ByteStuffing {
        flag: 0xC0,
        esc: 0xDB,
        escape: Escape::Codes(0xDC, 0xDD),
    };

    /// The Beast binary feed of Mode S receivers: each frame starts with 0x1A, and a 0x1A inside a frame is sent twice. The
    /// first byte of a frame is its type ('1' to '4'), never 0x1A, which keeps the start of a frame unambiguous.
    pub const BEAST: ByteStuffing = ByteStuffing {
        flag: 0x1A,
        esc: 0x1A,
        escape: Escape::Doubled,
    };

    /// Appends `data` with its special bytes escaped (no flags added).
    pub fn escape(&self, data: &[u8], out: &mut Vec<u8>) {
        for &b in data {
            match self.escape {
                Escape::Doubled => {
                    out.push(b);
                    if b == self.flag {
                        out.push(b);
                    }
                }
                Escape::Xor(x) => {
                    if b == self.flag || b == self.esc {
                        out.extend_from_slice(&[self.esc, b ^ x]);
                    } else {
                        out.push(b);
                    }
                }
                Escape::Codes(cf, ce) => {
                    if b == self.flag {
                        out.extend_from_slice(&[self.esc, cf]);
                    } else if b == self.esc {
                        out.extend_from_slice(&[self.esc, ce]);
                    } else {
                        out.push(b);
                    }
                }
            }
        }
    }

    /// Appends a whole frame: flag, escaped data, and (except for [`Escape::Doubled`], whose frames end at the next flag) a closing
    /// flag.
    pub fn frame(&self, data: &[u8], out: &mut Vec<u8>) {
        out.push(self.flag);
        self.escape(data, out);
        if self.escape != Escape::Doubled {
            out.push(self.flag);
        }
    }
}

/// A streaming unstuffer: feed bytes, get frames. Empty frames (back-to-back flags) are skipped. A frame longer than the limit is
/// dropped. No allocation after [`Unstuffer::new`].
#[derive(Clone, Debug)]
pub struct Unstuffer {
    s: ByteStuffing,
    buf: Vec<u8>,
    done: Vec<u8>,
    max: usize,
    in_frame: bool,
    escaped: bool,
    overflow: bool,
}

impl Unstuffer {
    /// An unstuffer for `s`, keeping frames up to `max` bytes.
    pub fn new(s: ByteStuffing, max: usize) -> Self {
        Unstuffer {
            s,
            buf: Vec::with_capacity(max),
            done: Vec::with_capacity(max),
            max,
            in_frame: false,
            escaped: false,
            overflow: false,
        }
    }

    fn finish(&mut self) -> Option<usize> {
        let ok = self.in_frame && !self.overflow && !self.buf.is_empty() && !self.escaped;
        let r = if ok {
            self.done.clear();
            self.done.extend_from_slice(&self.buf);
            Some(self.done.len())
        } else {
            None
        };
        self.buf.clear();
        self.escaped = false;
        self.overflow = false;
        self.in_frame = true;
        r
    }

    fn data(&mut self, b: u8) {
        if !self.in_frame {
            return;
        }
        if self.buf.len() >= self.max {
            self.overflow = true;
        } else {
            self.buf.push(b);
        }
    }

    /// Takes one byte; returns the length of a frame that it completed (the frame is in [`Unstuffer::frame`]). For the doubled
    /// rule a frame is complete when the next frame starts.
    pub fn push(&mut self, b: u8) -> Option<usize> {
        match self.s.escape {
            Escape::Doubled => {
                if self.escaped {
                    self.escaped = false;
                    if b == self.s.flag {
                        self.data(b);
                        return None;
                    }
                    // A lone flag: the previous frame ended, and this byte starts the new one.
                    let r = self.finish();
                    self.data(b);
                    return r;
                }
                if b == self.s.flag {
                    self.escaped = true;
                    return None;
                }
                self.data(b);
                None
            }
            Escape::Xor(x) => {
                if b == self.s.flag {
                    return self.finish();
                }
                if self.escaped {
                    self.escaped = false;
                    self.data(b ^ x);
                } else if b == self.s.esc {
                    self.escaped = true;
                } else {
                    self.data(b);
                }
                None
            }
            Escape::Codes(cf, ce) => {
                if b == self.s.flag {
                    return self.finish();
                }
                if self.escaped {
                    self.escaped = false;
                    if b == cf {
                        self.data(self.s.flag);
                    } else if b == ce {
                        self.data(self.s.esc);
                    } else {
                        // A bad escape: the KISS spec drops the escape and keeps the byte.
                        self.data(b);
                    }
                } else if b == self.s.esc {
                    self.escaped = true;
                } else {
                    self.data(b);
                }
                None
            }
        }
    }

    /// The last frame completed.
    pub fn frame(&self) -> &[u8] {
        &self.done
    }

    /// For the doubled rule: ends the frame in progress (at the end of input) and returns its length.
    pub fn flush(&mut self) -> Option<usize> {
        if self.escaped {
            // A trailing lone flag starts nothing.
            self.escaped = false;
        }
        let r = self.finish();
        self.in_frame = false;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crc::catalog::{CRC16_IBM_SDLC, CRC16_XMODEM};

    fn deframe(bits: &[u8], d: &mut HdlcDeframer) -> Vec<(Vec<u8>, bool)> {
        let mut v = Vec::new();
        for &b in bits {
            if let Some(f) = d.push(b) {
                v.push((d.frame().to_vec(), f.fcs_ok));
            }
        }
        v
    }

    /// Flag, `octets` least significant bit first, flag; no stuffing applied.
    fn raw_frame(octets: &[u8]) -> Vec<u8> {
        let mut bits = Vec::new();
        crate::pack::unpack(&[FLAG], BitOrder::Lsb, &mut bits);
        crate::pack::unpack(octets, BitOrder::Lsb, &mut bits);
        crate::pack::unpack(&[FLAG], BitOrder::Lsb, &mut bits);
        bits
    }

    #[test]
    fn the_check_value_frame_is_accepted_with_the_published_fcs() {
        // ISO/IEC 13239 16-bit FCS = CRC-16/IBM-SDLC, check value 0x906E for "123456789", sent least significant octet first.
        // No run of five ones occurs in these octets, so the line bits are written here without the encoder.
        let line = raw_frame(b"123456789\x6E\x90");
        let mut d = HdlcDeframer::x25(1, 64);
        assert_eq!(deframe(&line, &mut d), vec![(b"123456789".to_vec(), true)]);
        let mut enc = Vec::new();
        hdlc_encode(&[b"123456789"], DEFAULT_FCS, 1, &mut enc);
        assert_eq!(enc, line);
        // The wrong FCS octet order is a bad frame.
        let mut d = HdlcDeframer::x25(1, 64).deliver_bad(true);
        assert_eq!(
            deframe(&raw_frame(b"123456789\x90\x6E"), &mut d),
            vec![(b"123456789".to_vec(), false)]
        );
        assert_eq!((d.stats.good, d.stats.bad_fcs), (0, 1));
    }

    #[test]
    fn the_32_bit_fcs_check_value_frame_needs_a_stuffed_zero() {
        // ISO/IEC 13239 32-bit FCS = CRC-32/ISO-HDLC, check value 0xCBF43926, least significant octet first: 26 39 F4 CB.
        // F4 then CB sent LSB first are 0010 1111 1101 0011: six ones in a row, so a zero is stuffed after the fifth.
        let mut line = raw_frame(b"123456789\x26\x39\xF4\xCB");
        let at = 8 + 8 * 11 + 4 + 5; // flag, 11 octets, 4 bits of F4, five ones
        assert_eq!(&line[at - 5..at], &[1, 1, 1, 1, 1]);
        line.insert(at, 0);
        let mut enc = Vec::new();
        hdlc_encode(
            &[b"123456789"],
            &crate::crc::catalog::CRC32_ISO_HDLC,
            1,
            &mut enc,
        );
        assert_eq!(enc, line);
        let mut d = HdlcDeframer::try_new(&crate::crc::catalog::CRC32_ISO_HDLC, 1, 64).unwrap();
        assert_eq!(deframe(&line, &mut d), vec![(b"123456789".to_vec(), true)]);
        // The same line under the 16-bit FCS is a bad frame of 11 octets.
        let mut d = HdlcDeframer::x25(1, 64).deliver_bad(true);
        let got = deframe(&line, &mut d);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0.len(), 11);
        assert!(!got[0].1);
        // The bad frame keeps the two octets the 16-bit deframer reads as its FCS.
        assert_eq!(d.received_fcs(), &[0xF4, 0xCB]);
        assert_eq!(d.frame_with_fcs(), b"123456789\x26\x39\xF4\xCB");
        // A good frame also returns its FCS, and a frame without an FCS returns none.
        let mut d = HdlcDeframer::try_new(&crate::crc::catalog::CRC32_ISO_HDLC, 1, 64).unwrap();
        deframe(&line, &mut d);
        assert_eq!(d.received_fcs(), &[0x26, 0x39, 0xF4, 0xCB]);
        let mut n = HdlcDeframer::without_fcs(1, 64).unwrap();
        assert_eq!(deframe(&line, &mut n).len(), 1);
        assert!(n.received_fcs().is_empty());
        assert_eq!(n.frame_with_fcs(), n.frame());
    }

    #[test]
    fn frames_without_fcs_follow_the_stuffing_rule() {
        // The ISO/IEC 13239 transparency rule: the data octets 0x7E 0x01, sent least significant bit first, are
        // 0111 1110 1000 0000. A zero is stuffed after the first five ones. No FCS follows (GOMspace AX100 Mode 6 framing).
        let flag = [0, 1, 1, 1, 1, 1, 1, 0];
        let mut line = flag.to_vec();
        line.extend_from_slice(&[0, 1, 1, 1, 1, 1, 0, 1, 0]);
        line.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        line.extend_from_slice(&flag);
        let mut enc = Vec::new();
        hdlc_encode_without_fcs(&[&[0x7E, 0x01]], 1, &mut enc);
        assert_eq!(enc, line);
        let mut d = HdlcDeframer::without_fcs(1, 16).unwrap();
        assert!(!d.has_fcs());
        assert_eq!(deframe(&line, &mut d), vec![(vec![0x7E, 0x01], true)]);
        assert_eq!((d.stats.good, d.stats.bad_fcs), (1, 0));
        // The FCS deframer finds no FCS in two octets and reports nothing.
        let mut x = HdlcDeframer::x25(1, 16);
        assert!(deframe(&line, &mut x).is_empty());
        // Lengths still apply, and an FCS can be turned on again.
        let mut long = Vec::new();
        hdlc_encode_without_fcs(&[&[0x55; 17], &[1, 2, 3]], 2, &mut long);
        assert_eq!(deframe(&long, &mut d), vec![(vec![1, 2, 3], true)]);
        assert_eq!(d.stats.aborted, 1);
        d.set_fcs(DEFAULT_FCS).unwrap();
        assert!(d.has_fcs());
        let mut with = Vec::new();
        hdlc_encode(&[b"123456789"], DEFAULT_FCS, 1, &mut with);
        assert_eq!(deframe(&with, &mut d), vec![(b"123456789".to_vec(), true)]);
        assert!(deframe(&line, &mut d).is_empty());
        assert!(HdlcDeframer::without_fcs(5, 4).is_err());
    }

    #[test]
    fn fcs_and_lengths_can_change_and_bad_values_are_refused() {
        assert!(HdlcDeframer::try_new(&crate::crc::catalog::CRC12_DECT, 1, 10).is_err());
        assert!(HdlcDeframer::try_new(DEFAULT_FCS, 11, 10).is_err());
        let mut d = HdlcDeframer::x25(1, 400);
        assert!(d.set_fcs(&crate::crc::catalog::CRC40_GSM).is_err());
        assert_eq!(d.fcs().spec().name, "CRC-16/IBM-SDLC");
        assert!(d.set_lengths(5, 4).is_err());
        assert_eq!(d.lengths(), (1, 400));
        d.set_fcs(&crate::crc::catalog::CRC32_ISO_HDLC).unwrap();
        d.set_lengths(9, 9).unwrap();
        let line = {
            let mut v = Vec::new();
            hdlc_encode(
                &[b"12345678", b"123456789", b"1234567890"],
                d.fcs(),
                1,
                &mut v,
            );
            v
        };
        assert_eq!(deframe(&line, &mut d), vec![(b"123456789".to_vec(), true)]);
        assert_eq!(d.stats.aborted, 2);
    }

    #[test]
    fn stuffing_inserts_a_zero_after_five_ones() {
        let mut out = Vec::new();
        let mut ones = 0;
        stuff_bits(&[1, 1, 1, 1, 1, 1, 1, 1], &mut ones, &mut out);
        assert_eq!(out, [1, 1, 1, 1, 1, 0, 1, 1, 1]);
        let mut back = Vec::new();
        unstuff_bits(&out, &mut back).unwrap();
        assert_eq!(back, [1; 8]);
        assert!(unstuff_bits(&[1, 1, 1, 1, 1, 1], &mut back).is_err());
    }

    #[test]
    fn frames_back_to_back_with_shared_and_single_flags() {
        let frames: [&[u8]; 4] = [
            b"\x7E\x7E\xFF\xFF",
            b"A",
            b"\x00\x00\x00",
            b"APRS>test:payload with 0x7E ~~~",
        ];
        for flags in [1, 2, 5] {
            let mut bits = Vec::new();
            hdlc_encode(&frames, &CRC16_IBM_SDLC, flags, &mut bits);
            let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 330);
            let got = deframe(&bits, &mut d);
            assert_eq!(got.len(), 4, "flags {flags}");
            for (g, f) in got.iter().zip(frames) {
                assert_eq!(g.0, f);
                assert!(g.1);
            }
            assert_eq!(d.stats.good, 4);
        }
        // A non-reflected FCS is sent high byte first.
        let mut bits = Vec::new();
        hdlc_encode(&[b"xmodem"], &CRC16_XMODEM, 1, &mut bits);
        let mut d = HdlcDeframer::new(&CRC16_XMODEM, 1, 100);
        assert_eq!(deframe(&bits, &mut d), vec![(b"xmodem".to_vec(), true)]);
    }

    #[test]
    fn errors_aborts_and_lengths() {
        let mut bits = Vec::new();
        hdlc_encode(&[b"0123456789"], &CRC16_IBM_SDLC, 1, &mut bits);
        // A bit error: FCS fails; delivered only on request.
        let mut e = bits.clone();
        e[30] ^= 1;
        let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 100);
        assert!(deframe(&e, &mut d).is_empty());
        let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 100).deliver_bad(true);
        let got = deframe(&e, &mut d);
        assert!(got.len() <= 1 && got.iter().all(|g| !g.1));
        // Seven ones in the middle abort the frame; the next one is still found.
        let mut a = bits[..40].to_vec();
        a.extend_from_slice(&[1; 8]);
        a.extend_from_slice(&bits);
        let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 100);
        assert_eq!(deframe(&a, &mut d), vec![(b"0123456789".to_vec(), true)]);
        assert_eq!(d.stats.aborted, 1);
        // Length limits.
        let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 11, 100);
        assert!(deframe(&bits, &mut d).is_empty());
        let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 5);
        assert!(deframe(&bits, &mut d).is_empty());
        d.reset();
    }

    #[test]
    fn byte_stuffing_follows_the_specs() {
        // RFC 1662 clause 4.2: 0x7E is sent 0x7D 0x5E, 0x7D is sent 0x7D 0x5D.
        let mut o = Vec::new();
        ByteStuffing::PPP.escape(&[0x7E, 0x01, 0x7D], &mut o);
        assert_eq!(o, [0x7D, 0x5E, 0x01, 0x7D, 0x5D]);
        // KISS: FEND is sent FESC TFEND, FESC is sent FESC TFESC.
        o.clear();
        ByteStuffing::KISS.escape(&[0xC0, 0x00, 0xDB], &mut o);
        assert_eq!(o, [0xDB, 0xDC, 0x00, 0xDB, 0xDD]);
        o.clear();
        ByteStuffing::BEAST.escape(&[0x31, 0x1A, 0x02], &mut o);
        assert_eq!(o, [0x31, 0x1A, 0x1A, 0x02]);
    }

    #[test]
    fn unstuffing_streams() {
        for s in [ByteStuffing::PPP, ByteStuffing::KISS, ByteStuffing::BEAST] {
            let frames: [&[u8]; 3] = [
                &[0x00, 0x7E, 0xC0, 0x1A, 0x7D, 0xDB],
                &[0x33, 0x1A, 0x1A],
                &[5; 40],
            ];
            let mut line = Vec::new();
            for f in frames {
                s.frame(f, &mut line);
            }
            let mut u = Unstuffer::new(s, 100);
            let mut got = Vec::new();
            for &b in &line {
                if u.push(b).is_some() {
                    got.push(u.frame().to_vec());
                }
            }
            if u.flush().is_some() {
                got.push(u.frame().to_vec());
            }
            assert_eq!(got, frames.map(|f| f.to_vec()), "{s:?}");
            let mut small = Unstuffer::new(s, 10);
            let n = line.iter().filter(|&&b| small.push(b).is_some()).count()
                + small.flush().is_some() as usize;
            assert_eq!(n, 2, "{s:?}: the 40-byte frame is dropped");
        }
    }
}
