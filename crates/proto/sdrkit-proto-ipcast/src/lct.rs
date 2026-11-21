//! The LCT header with the header extensions that ROUTE uses: EXT_TOL, EXT_TIME and the presentation time.
//!
//! Standard: RFC 5651 clause 5.1, RFC 9223 clause 4.4, ATSC A/331 annex A.

use crate::error::{Error, Result};
use crate::error::{Limit, Malformed};

/// HET of EXT_NOP (RFC 5651 clause 5.2).
pub const EXT_NOP: u8 = 0;
/// HET of EXT_AUTH.
pub const EXT_AUTH: u8 = 1;
/// HET of EXT_TIME (RFC 5651 clause 5.2.2).
pub const EXT_TIME: u8 = 2;
/// HET of EXT_FTI (RFC 5775).
pub const EXT_FTI: u8 = 64;
/// HET of the ROUTE presentation time (A/331 annex A, 64-bit NTP time).
pub const EXT_ROUTE_PRESENTATION_TIME: u8 = 66;
/// HET of EXT_TOL with a 48-bit transfer length (A/331 annex A).
pub const EXT_TOL_48: u8 = 67;
/// HET of EXT_FDT (RFC 6726).
pub const EXT_FDT: u8 = 192;
/// HET of EXT_CENC (RFC 6726).
pub const EXT_CENC: u8 = 193;
/// HET of EXT_TOL with a 24-bit transfer length (A/331 annex A).
pub const EXT_TOL_24: u8 = 194;

/// A parsed LCT header.
///
/// ```
/// use sdrkit_proto_ipcast::lct::{Lct, LctBuilder};
/// let mut p = Vec::new();
/// LctBuilder { tsi: 5, toi: 9, codepoint: 8, transfer_len: Some(5000), close_object: false }.write(&mut p).unwrap();
/// p.extend_from_slice(&0u32.to_be_bytes()); // start_offset
/// let h = Lct::parse(&p).unwrap();
/// assert_eq!((h.tsi, h.toi, h.transfer_len, h.source), (5, 9, Some(5000), true));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lct {
    /// PSI, 2 bits.
    pub psi: u8,
    /// True for a ROUTE source packet (the most significant PSI bit is 1).
    pub source: bool,
    /// A: the sender closes the session.
    pub close_session: bool,
    /// B: this packet is the last one of the object.
    pub close_object: bool,
    /// The congestion control information (up to 128 bits, the first 64 here).
    pub cci: u64,
    /// The transport session identifier (up to 48 bits).
    pub tsi: u64,
    /// The transport object identifier (the low 64 bits).
    pub toi: u64,
    /// The codepoint.
    pub codepoint: u8,
    /// EXT_TOL: the transfer length of the object in bytes.
    pub transfer_len: Option<u64>,
    /// EXT_TIME: the sender current time as a 64-bit NTP time (SCT-High and, when sent, SCT-Low).
    pub sender_time: Option<u64>,
    /// EXT_TIME: the expected residual time, in seconds.
    pub residual_time: Option<u32>,
    /// The ROUTE presentation time, 64-bit NTP time.
    pub presentation_time: Option<u64>,
    /// The header length in bytes (HDR_LEN x 4).
    pub len: usize,
}

impl Lct {
    /// Reads an LCT header. If the version is not 1, a length is outside the packet, or HEL is 0, it returns an error.
    pub fn parse(b: &[u8]) -> Result<Lct> {
        if b.len() < 4 {
            return Err(Error::Short);
        }
        if b[0] >> 4 != 1 {
            return Err(Error::Invalid(Malformed::LctVersion));
        }
        let c = ((b[0] >> 2) & 3) as usize;
        let psi = b[0] & 3;
        let s = (b[1] >> 7) as usize;
        let o = ((b[1] >> 5) & 3) as usize;
        let hw = ((b[1] >> 4) & 1) as usize;
        let len = b[2] as usize * 4;
        if b.len() < len {
            return Err(Error::Short);
        }
        let cci_len = 4 * (c + 1);
        let tsi_len = 4 * s + 2 * hw;
        let toi_len = 4 * o + 2 * hw;
        let mut at = 4;
        if at + cci_len + tsi_len + toi_len > len {
            return Err(Error::Invalid(Malformed::HdrLen));
        }
        let mut h = Lct {
            psi,
            source: psi & 2 != 0,
            close_session: b[1] & 2 != 0,
            close_object: b[1] & 1 != 0,
            codepoint: b[3],
            len,
            ..Lct::default()
        };
        h.cci = be(&b[at..at + cci_len.min(8)]);
        at += cci_len;
        h.tsi = be(&b[at..at + tsi_len]);
        at += tsi_len;
        let toi = &b[at..at + toi_len];
        h.toi = be(&toi[toi.len().saturating_sub(8)..]);
        at += toi_len;
        while at < len {
            let het = b[at];
            let n = if het >= 128 {
                4
            } else {
                let hel = *b.get(at + 1).ok_or(Error::Short)? as usize * 4;
                if hel == 0 {
                    return Err(Error::Invalid(Malformed::Hel));
                }
                hel
            };
            if at + n > len {
                return Err(Error::Invalid(Malformed::HeaderExtensionLength));
            }
            let e = &b[at..at + n];
            match het {
                EXT_TOL_24 => h.transfer_len = Some(be(&e[1..4])),
                EXT_TOL_48 if n >= 8 => h.transfer_len = Some(be(&e[2..8])),
                EXT_ROUTE_PRESENTATION_TIME if n >= 12 => h.presentation_time = Some(be(&e[4..12])),
                EXT_TIME => {
                    let usef = u16::from_be_bytes([e[2], e[3]]);
                    let mut v = 4;
                    let mut word = || {
                        let w = e.get(v..v + 4).map(be);
                        v += 4;
                        w
                    };
                    let hi = if usef & 0x8000 != 0 { word() } else { None };
                    let lo = if usef & 0x4000 != 0 { word() } else { None };
                    let ert = if usef & 0x2000 != 0 { word() } else { None };
                    if let Some(hi) = hi {
                        h.sender_time = Some(hi << 32 | lo.unwrap_or(0));
                    }
                    h.residual_time = ert.map(|x| x as u32);
                }
                _ => {}
            }
            at += n;
        }
        Ok(h)
    }
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, &x| (a << 8) | x as u64)
}

/// Writes the LCT header of a ROUTE source packet as RFC 9223 clause 4.4 profiles it: V = 1, C = 0 (a 32-bit CCI of zero),
/// PSI = 10, S = 1, O = 01, H = 0, and EXT_TOL when the transfer length is given (24 bits when it fits, else 48 bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LctBuilder {
    /// The TSI.
    pub tsi: u32,
    /// The TOI.
    pub toi: u32,
    /// The codepoint.
    pub codepoint: u8,
    /// The transfer length to send in EXT_TOL.
    pub transfer_len: Option<u64>,
    /// Sets B: the last packet of the object.
    pub close_object: bool,
}

impl LctBuilder {
    /// Appends the header to `out` (the caller appends the start_offset and the data). A transfer length of 2^48 or more
    /// returns [`Error::TooLarge`].
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        let ext: &[u8] = &[];
        let mut tol = [0u8; 8];
        let ext = match self.transfer_len {
            None => ext,
            Some(t) if t < 1 << 24 => {
                tol[..4].copy_from_slice(&[EXT_TOL_24, (t >> 16) as u8, (t >> 8) as u8, t as u8]);
                &tol[..4]
            }
            Some(t) if t < 1 << 48 => {
                tol[0] = EXT_TOL_48;
                tol[1] = 2;
                tol[2..].copy_from_slice(&t.to_be_bytes()[2..]);
                &tol[..]
            }
            Some(_) => return Err(Error::TooLarge(Limit::TransferLength)),
        };
        let words = (4 + 4 + 4 + 4 + ext.len()) / 4;
        out.push(0x10 | 0x02);
        out.push(0x80 | 0x20 | self.close_object as u8);
        out.push(words as u8);
        out.push(self.codepoint);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&self.tsi.to_be_bytes());
        out.extend_from_slice(&self.toi.to_be_bytes());
        out.extend_from_slice(ext);
        Ok(())
    }
}
