//! HDLC links as packet radio sends them: NRZ, NRZI or G3RUH-scrambled NRZI around the HDLC framing of
//! [`sdrkit_util_bits::hdlc`].
//!
//! Standard: ISO/IEC 13239 clause 4, AX.25 2.2 clause 3

use crate::Param;
use crate::{Error, Result};
use sdrkit_util_bits::hdlc::{
    hdlc_encode, stuff_bits, FrameEnd, HdlcDeframer, HdlcStats, DEFAULT_FCS, FLAG,
};
use sdrkit_util_bits::lfsr::Multiplicative;
use sdrkit_util_bits::line::Nrzi;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::seq;
use sdrkit_util_bits::BitOrder;

/// The shortest AX.25 frame without its FCS: two addresses and a control field (AX.25 2.2 clause 3.1).
pub const AX25_MIN: usize = 15;
/// The longest AX.25 frame kept without its FCS. AX.25 2.2 allows 10 addresses, a 2-byte control field, the PID and 256 bytes of
/// information (329 bytes). Some stations send more, so receivers keep up to 400.
pub const AX25_MAX: usize = 400;

sdrkit_core::choice_enum! {
    /// The line code around the HDLC bits.
    pub enum Line {
        /// The HDLC bits are sent as they are.
        Nrz = ("nrz", "NRZ"),
        /// NRZI: a 0 changes the line level, a 1 keeps it.
        Nrzi = ("nrzi", "NRZI"),
        /// NRZI, then the self-synchronising G3RUH scrambler `x^17 + x^12 + 1`.
        G3ruh = ("g3ruh", "G3RUH"),
    }
}

/// The line-code state shared by the receiver and transmitter.
#[derive(Clone, Copy, Debug)]
struct Coder {
    line: Line,
    nrzi: Nrzi,
    scrambler: Multiplicative,
}

impl Coder {
    fn new(line: Line) -> Self {
        Coder {
            line,
            nrzi: Nrzi::new(0),
            scrambler: seq::g3ruh(0),
        }
    }

    #[inline]
    fn decode(&mut self, bit: u8) -> u8 {
        let mut b = [bit & 1];
        match self.line {
            Line::Nrz => {}
            Line::Nrzi => self.nrzi.decode(&mut b),
            Line::G3ruh => {
                self.scrambler.descramble(&mut b);
                self.nrzi.decode(&mut b);
            }
        }
        b[0]
    }

    fn encode(&mut self, bits: &mut [u8]) {
        match self.line {
            Line::Nrz => {}
            Line::Nrzi => self.nrzi.encode(bits),
            Line::G3ruh => {
                self.nrzi.encode(bits);
                self.scrambler.scramble(bits);
            }
        }
    }
}

/// A streaming HDLC receiver: line bits in (one `u8` per bit), frames out with a good FCS. It allocates its buffers in
/// [`Receiver::new`] and nothing after.
///
/// Seven ones in a row abort a frame. A frame that is not a whole number of octets, or whose length without the FCS is outside
/// the limits, is dropped and counted in [`HdlcStats`] (ISO/IEC 13239 clause 4.4).
///
/// ```
/// use sdrkit_proto_link::hdlc::{Line, Receiver, Transmitter};
/// let mut tx = Transmitter::new(Line::G3ruh);
/// let mut bits = Vec::new();
/// tx.encode(&[b"first frame", b"second"], 8, &mut bits);
/// let mut rx = Receiver::new(Line::G3ruh, 1, 400).unwrap();
/// let mut got = Vec::new();
/// rx.push_bits(&bits, |frame| got.push(frame.to_vec()));
/// assert_eq!(got, [b"first frame".to_vec(), b"second".to_vec()]);
/// ```
#[derive(Clone, Debug)]
pub struct Receiver {
    coder: Coder,
    deframer: HdlcDeframer,
    /// If true, [`Receiver::frame_with_fcs`] returns the received FCS.
    with_fcs: bool,
}

impl Receiver {
    /// A receiver for `line`, keeping frames of `min` to `max` bytes without the FCS. It uses the 16-bit FCS of ISO/IEC 13239
    /// (CRC-16/IBM-SDLC, as AX.25 uses). An error when `min > max`.
    pub fn new(line: Line, min: usize, max: usize) -> Result<Self> {
        let deframer = HdlcDeframer::try_new(DEFAULT_FCS, min, max)
            .map_err(|_| Error::BadParameter(Param::FrameLengths))?;
        Ok(Receiver {
            coder: Coder::new(line),
            deframer,
            with_fcs: false,
        })
    }

    /// A receiver of AX.25 frames ([`AX25_MIN`] to [`AX25_MAX`] bytes) on `line`.
    pub fn ax25(line: Line) -> Self {
        Receiver {
            coder: Coder::new(line),
            deframer: HdlcDeframer::x25(AX25_MIN, AX25_MAX),
            with_fcs: false,
        }
    }

    /// A receiver over a deframer the caller set up (another FCS, [`HdlcDeframer::deliver_bad`]). A deframer set to deliver bad
    /// frames delivers them here, but [`Receiver::frame_with_fcs`] stays empty: use [`Receiver::deliver_bad`] for that.
    pub fn with_deframer(line: Line, deframer: HdlcDeframer) -> Self {
        Receiver {
            coder: Coder::new(line),
            deframer,
            with_fcs: false,
        }
    }

    /// Also reports frames whose FCS is wrong (`fcs_ok` false in the [`FrameEnd`]), and keeps the FCS each frame carried for
    /// [`Receiver::frame_with_fcs`], so a caller can try to [`repair`] it.
    pub fn deliver_bad(self, yes: bool) -> Self {
        let Receiver {
            coder, deframer, ..
        } = self;
        Receiver {
            coder,
            deframer: deframer.deliver_bad(yes),
            with_fcs: yes,
        }
    }

    /// The line code.
    pub fn line(&self) -> Line {
        self.coder.line
    }

    /// Takes one line bit (its lowest bit). Returns the end of a frame when one completes. Its bytes are in [`Receiver::frame`].
    #[inline]
    pub fn push(&mut self, bit: u8) -> Option<FrameEnd> {
        let b = self.coder.decode(bit);
        self.deframer.push(b)
    }

    /// The bytes of the last frame reported followed by the FCS as it was received (FCS bytes in the order sent). Empty unless
    /// [`Receiver::deliver_bad`] is on.
    pub fn frame_with_fcs(&self) -> &[u8] {
        if self.with_fcs {
            self.deframer.frame_with_fcs()
        } else {
            &[]
        }
    }

    /// Takes line bits and calls `on_frame` with the bytes (without the FCS) of each frame that completes.
    pub fn push_bits(&mut self, bits: &[u8], mut on_frame: impl FnMut(&[u8])) {
        for &b in bits {
            if self.push(b).is_some() {
                on_frame(self.deframer.frame());
            }
        }
    }

    /// The bytes of the last frame reported (without the FCS).
    pub fn frame(&self) -> &[u8] {
        self.deframer.frame()
    }

    /// The counts of good, bad and aborted frames.
    pub fn stats(&self) -> HdlcStats {
        self.deframer.stats
    }

    /// Forgets any frame in progress and the line-code state.
    pub fn reset(&mut self) {
        self.deframer.reset();
        self.coder = Coder::new(self.coder.line);
    }
}

/// What [`repair`] changed in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repair {
    /// The bits flipped: 1, or 2 next to each other.
    pub bits: u8,
    /// The first bit flipped, in the order sent: byte `at / 8`, bit `at % 8` (HDLC sends the least significant bit first).
    pub at: usize,
}

/// Tries to repair a frame whose FCS failed by flipping one bit, then (with `max_bits` 2) two bits next to each other in the
/// order sent, as APRS receivers do. `frame_with_fcs` is the frame followed by the FCS it carried (see
/// [`Receiver::frame_with_fcs`]). A candidate is accepted when `fcs` passes and `accept` returns true for the frame bytes
/// without the FCS. On success the frame is changed in place. Otherwise it is unchanged and the result is `None`, also for
/// `max_bits` 0 or a frame that is not longer than its FCS.
///
/// A wrong candidate can pass a 16-bit FCS by chance (about 7 % of 300-byte frames with more errors than this repairs). Use
/// the `accept` test (for AX.25, [`crate::ax25::Frame::parse`] succeeds) and the [`Repair`] mark to tell such frames apart.
pub fn repair(
    frame_with_fcs: &mut [u8],
    fcs: impl Into<sdrkit_util_bits::crc::catalog::Engine>,
    max_bits: u8,
    mut accept: impl FnMut(&[u8]) -> bool,
) -> Option<Repair> {
    let fcs = fcs.into();
    let f = fcs.spec().width as usize / 8;
    let n = frame_with_fcs.len();
    if max_bits == 0 || n <= f {
        return None;
    }
    let trailer = fcs.natural_trailer();
    let bits = 8 * n;
    let flip = |b: &mut [u8], i: usize| b[i / 8] ^= 1 << (i % 8);
    for width in 1..=max_bits.min(2) as usize {
        for i in 0..=bits - width {
            for k in 0..width {
                flip(frame_with_fcs, i + k);
            }
            if fcs.verify(frame_with_fcs, trailer) && accept(&frame_with_fcs[..n - f]) {
                return Some(Repair {
                    bits: width as u8,
                    at: i,
                });
            }
            for k in 0..width {
                flip(frame_with_fcs, i + k);
            }
        }
    }
    None
}

/// [`repair`] for AX.25: the FCS of ISO/IEC 13239 and a frame that [`crate::ax25::Frame::parse`] reads.
pub fn repair_ax25(frame_with_fcs: &mut [u8], max_bits: u8) -> Option<Repair> {
    repair(frame_with_fcs, DEFAULT_FCS, max_bits, |b| {
        crate::ax25::Frame::parse(b).is_ok()
    })
}

/// An HDLC transmitter: frames in, line bits out. The line-code state carries from one call to the next, so a long transmission
/// can be built in pieces.
#[derive(Clone, Debug)]
pub struct Transmitter {
    coder: Coder,
    scratch: Vec<u8>,
}

impl Transmitter {
    /// A transmitter for `line` with the line at level 0 and the scrambler cleared.
    pub fn new(line: Line) -> Self {
        Transmitter {
            coder: Coder::new(line),
            scratch: Vec::new(),
        }
    }

    /// Appends to `out` the line bits of `frames`: `flags` flags first (at least 1, packet TNCs send TXDELAY as flags), each
    /// frame with its FCS (CRC-16/IBM-SDLC, low byte first) and zero-bit insertion, one flag between frames, and a closing flag.
    pub fn encode(&mut self, frames: &[&[u8]], flags: usize, out: &mut Vec<u8>) {
        self.scratch.clear();
        hdlc_encode(frames, DEFAULT_FCS, flags.max(1), &mut self.scratch);
        self.coder.encode(&mut self.scratch);
        out.extend_from_slice(&self.scratch);
    }

    /// Appends to `out` the line bits of `frames` sent with no FCS (AX100 Mode 6, where Reed-Solomon parity protects the
    /// frame): `flags` flags, each frame bit-stuffed, one flag between frames, and a closing flag.
    pub fn encode_without_fcs(&mut self, frames: &[&[u8]], flags: usize, out: &mut Vec<u8>) {
        self.scratch.clear();
        let flag_bits = |v: &mut Vec<u8>| v.extend((0..8).map(|k| (FLAG >> k) & 1));
        for _ in 0..flags.max(1) {
            flag_bits(&mut self.scratch);
        }
        let mut data = Vec::new();
        for f in frames {
            data.clear();
            unpack(f, BitOrder::Lsb, &mut data);
            let mut ones = 0;
            stuff_bits(&data, &mut ones, &mut self.scratch);
            flag_bits(&mut self.scratch);
        }
        self.coder.encode(&mut self.scratch);
        out.extend_from_slice(&self.scratch);
    }

    /// Appends `n` flags (for TXTAIL, or idle fill between transmissions).
    pub fn flags(&mut self, n: usize, out: &mut Vec<u8>) {
        self.scratch.clear();
        for _ in 0..n {
            for k in 0..8 {
                self.scratch.push((FLAG >> k) & 1);
            }
        }
        self.coder.encode(&mut self.scratch);
        out.extend_from_slice(&self.scratch);
    }
}
