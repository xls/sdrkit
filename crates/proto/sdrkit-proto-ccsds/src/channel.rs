//! The channel coding of a frame: the pseudo-randomiser and interleaved Reed-Solomon. A channel frame holds `I` interleaved
//! RS(255,223) or RS(255,239) code words (byte `i` belongs to code word `i mod I`), shortened by virtual fill when the frame is
//! shorter. [`ChannelDecoder`] removes the randomiser and corrects the code words in the dual or the conventional symbol basis,
//! fixed or found with [`BasisPolicy::Auto`]. [`ChannelCoder`] makes such frames.
//!
//! Standard: CCSDS 131.0-B-5 clauses 4 and 10.

use crate::marker::Marker;
use crate::{marker, randomiser, Error, Param, Result};
use sdrkit_algo_fec::error::FecError;
use sdrkit_algo_fec::rs::{self, codes, Basis, Rs, RsCode};

/// How the decoder picks the Reed-Solomon basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BasisPolicy {
    /// Always this basis.
    Fixed(Basis),
    /// Start with this basis. If a frame fails, try the other basis and keep it if it works.
    Auto(Basis),
}

impl BasisPolicy {
    fn first(self) -> Basis {
        match self {
            BasisPolicy::Fixed(b) | BasisPolicy::Auto(b) => b,
        }
    }
}

/// The channel coding of a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelConfig {
    /// The marker the coder puts before each frame.
    pub marker: Marker,
    /// The Reed-Solomon code, or `None` (uncoded frames, or frames an inner LDPC or turbo code has corrected).
    pub rs: Option<RsCode>,
    /// Interleave depth `I`, 1 to 8.
    pub depth: usize,
    /// Virtual fill `Q`: bytes each code word is shortened by.
    pub virtual_fill: usize,
    /// The randomiser is on.
    pub randomise: bool,
    /// The basis policy (decoder only).
    pub basis: BasisPolicy,
}

impl ChannelConfig {
    /// RS(255,223) with interleave depth `depth`, randomised, dual basis: the CCSDS default (`depth` 4 is the 1020-byte
    /// frame of the weather satellites).
    pub const fn rs_223(depth: usize) -> Self {
        ChannelConfig {
            marker: marker::TM,
            rs: Some(codes::CCSDS_255_223),
            depth,
            virtual_fill: 0,
            randomise: true,
            basis: BasisPolicy::Fixed(Basis::Dual),
        }
    }

    /// RS(255,239) (E = 8) with interleave depth `depth`, randomised, dual basis.
    pub const fn rs_239(depth: usize) -> Self {
        ChannelConfig {
            rs: Some(codes::CCSDS_255_239),
            ..ChannelConfig::rs_223(depth)
        }
    }

    /// Frames of `len` bytes after the marker, randomised, without Reed-Solomon.
    pub const fn uncoded(len: usize) -> Self {
        ChannelConfig {
            marker: marker::TM,
            rs: None,
            depth: len,
            virtual_fill: 0,
            randomise: true,
            basis: BasisPolicy::Fixed(Basis::Dual),
        }
    }

    /// The same with this basis policy.
    pub const fn with_basis(mut self, basis: BasisPolicy) -> Self {
        self.basis = basis;
        self
    }

    /// The same with `q` virtual fill bytes per code word.
    pub const fn with_virtual_fill(mut self, q: usize) -> Self {
        self.virtual_fill = q;
        self
    }

    /// The same without the randomiser.
    pub const fn without_randomiser(mut self) -> Self {
        self.randomise = false;
        self
    }

    /// Bytes of the channel frame after the marker. For an uncoded link this is `depth`.
    pub const fn coded_len(&self) -> usize {
        match self.rs {
            Some(c) => self.depth * (c.n as usize - self.virtual_fill),
            None => self.depth,
        }
    }

    /// Bytes of the transfer frame the channel frame carries.
    pub const fn frame_len(&self) -> usize {
        match self.rs {
            Some(c) => self.depth * (c.k as usize - self.virtual_fill),
            None => self.depth,
        }
    }

    fn code(&self, basis: Basis) -> Option<RsCode> {
        self.rs.map(|c| {
            let c = RsCode { basis, ..c };
            if self.virtual_fill > 0 {
                c.shortened(c.n - self.virtual_fill as u16)
            } else {
                c
            }
        })
    }

    fn check(&self) -> Result<()> {
        match self.rs {
            Some(c) => {
                if !(1..=8).contains(&self.depth) {
                    return Err(Error::BadParameter(Param::InterleaveDepth));
                }
                if self.virtual_fill >= c.k as usize {
                    return Err(Error::BadParameter(Param::VirtualFill));
                }
            }
            None => {
                if self.depth == 0 {
                    return Err(Error::BadParameter(Param::UncodedFrameLength));
                }
            }
        }
        Ok(())
    }
}

/// The result of decoding one channel frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelDecoded {
    /// Symbols corrected in the code words that decoded.
    pub corrected: u32,
    /// Code words that did not decode (left as received).
    pub failed: u32,
    /// Bit `i` set: code word `i` did not decode.
    pub failed_mask: u32,
    /// The basis used.
    pub basis: Basis,
    /// True when this frame changed the basis.
    pub basis_changed: bool,
    /// Bytes of the transfer frame at the start of the buffer.
    pub frame_len: usize,
    /// Reed-Solomon was applied.
    pub checked: bool,
}

impl ChannelDecoded {
    /// True when every code word decoded (or the link has no Reed-Solomon).
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

/// Removes the randomiser and corrects the code words of channel frames. See the [module documentation](self).
#[derive(Debug)]
pub struct ChannelDecoder {
    cfg: ChannelConfig,
    codecs: Option<[Rs; 2]>,
    basis: Basis,
    copy: Vec<u8>,
}

impl ChannelDecoder {
    /// A decoder for `cfg`.
    pub fn new(cfg: ChannelConfig) -> Result<Self> {
        cfg.check()?;
        let codecs = match (cfg.code(Basis::Dual), cfg.code(Basis::Conventional)) {
            (Some(d), Some(c)) => Some([Rs::new(d)?, Rs::new(c)?]),
            _ => None,
        };
        Ok(ChannelDecoder {
            cfg,
            codecs,
            basis: cfg.basis.first(),
            copy: vec![0; cfg.coded_len()],
        })
    }

    /// The configuration.
    pub fn config(&self) -> &ChannelConfig {
        &self.cfg
    }

    /// The basis in use.
    pub fn basis(&self) -> Basis {
        self.basis
    }

    /// Decodes a channel frame in place: `frame` holds the [`ChannelConfig::coded_len`] bytes after the marker. On return the
    /// transfer frame is the first `frame_len` bytes. A code word that does not decode stays as received.
    pub fn decode(&mut self, frame: &mut [u8]) -> Result<ChannelDecoded> {
        if frame.len() != self.cfg.coded_len() {
            return Err(Error::BadLength);
        }
        if self.cfg.randomise {
            randomiser::tm_bytes(frame);
        }
        let mut out = ChannelDecoded {
            corrected: 0,
            failed: 0,
            failed_mask: 0,
            basis: self.basis,
            basis_changed: false,
            frame_len: self.cfg.frame_len(),
            checked: self.codecs.is_some(),
        };
        let Some(codecs) = self.codecs.as_mut() else {
            return Ok(out);
        };
        let depth = self.cfg.depth;
        let auto = matches!(self.cfg.basis, BasisPolicy::Auto(_));
        if auto {
            self.copy.copy_from_slice(frame);
        }
        let idx = |b: Basis| (b == Basis::Conventional) as usize;
        let r = codecs[idx(self.basis)].decode_interleaved(frame, depth, &[])?;
        if r.failed > 0 && auto {
            let other = other(self.basis);
            let mut try_frame = std::mem::take(&mut self.copy);
            let r2 = codecs[idx(other)].decode_interleaved(&mut try_frame, depth, &[]);
            match r2 {
                Ok(r2) if r2.failed < r.failed => {
                    frame.copy_from_slice(&try_frame);
                    self.basis = other;
                    out.basis = other;
                    out.basis_changed = true;
                    out.corrected = r2.corrected;
                    out.failed = r2.failed;
                    out.failed_mask = r2.failed_mask;
                }
                Ok(_) | Err(FecError::Uncorrectable) => {
                    out.corrected = r.corrected;
                    out.failed = r.failed;
                    out.failed_mask = r.failed_mask;
                }
                Err(e) => {
                    self.copy = try_frame;
                    return Err(e.into());
                }
            }
            self.copy = try_frame;
        } else {
            out.corrected = r.corrected;
            out.failed = r.failed;
            out.failed_mask = r.failed_mask;
        }
        // Data symbol j of code word c is at j * I + c. That is the transfer frame order, and all parity follows the data, so the
        // transfer frame is already at the front.
        Ok(out)
    }
}

fn other(b: Basis) -> Basis {
    match b {
        Basis::Dual => Basis::Conventional,
        Basis::Conventional => Basis::Dual,
    }
}

/// Makes channel frames: marker, interleaved Reed-Solomon code words, randomiser. For generators and tests.
#[derive(Debug)]
pub struct ChannelCoder {
    cfg: ChannelConfig,
    codec: Option<Rs>,
    data: Vec<u8>,
    parity: Vec<u8>,
}

impl ChannelCoder {
    /// A coder for `cfg`. The basis is the first one of the policy.
    pub fn new(cfg: ChannelConfig) -> Result<Self> {
        cfg.check()?;
        let codec = match cfg.code(cfg.basis.first()) {
            Some(c) => Some(Rs::new(c)?),
            None => None,
        };
        let (k, p) = match cfg.rs {
            Some(c) => (c.k as usize - cfg.virtual_fill, c.nroots()),
            None => (0, 0),
        };
        Ok(ChannelCoder {
            cfg,
            codec,
            data: vec![0; k],
            parity: vec![0; p],
        })
    }

    /// Appends the marker and the channel frame of `frame` (exactly [`ChannelConfig::frame_len`] bytes) to `out`.
    pub fn encode(&mut self, frame: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if frame.len() != self.cfg.frame_len() {
            return Err(Error::BadLength);
        }
        out.extend_from_slice(self.cfg.marker.bytes());
        let start = out.len();
        out.extend_from_slice(frame);
        if let Some(codec) = self.codec.as_ref() {
            let depth = self.cfg.depth;
            let k = self.data.len();
            out.resize(start + self.cfg.coded_len(), 0);
            for cw in 0..depth {
                for j in 0..k {
                    self.data[j] = frame[j * depth + cw];
                }
                codec.encode(&self.data, &mut self.parity)?;
                for (j, &p) in self.parity.iter().enumerate() {
                    out[start + (k + j) * depth + cw] = p;
                }
            }
        }
        if self.cfg.randomise {
            randomiser::tm_bytes(&mut out[start..]);
        }
        Ok(())
    }
}

/// The CCSDS dual-basis conversion tables of `sdrkit-algo-fec`, for callers that convert single symbols.
pub use rs::{FROM_DUAL, TO_DUAL};
