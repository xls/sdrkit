//! FIC decoding (EN 300 401 clauses 5.2, 10, 11): de-puncturing, Viterbi decoding, energy dispersal, and
//! the FIB check and walk.

use crate::fec::{crc_ok, ProfileDecoder};
use crate::tables::{fic_profile, FIC_BITS, FIC_BLOCK_BITS};

/// One fast information block.
#[derive(Clone, Debug, PartialEq)]
pub struct Fib {
    /// The frame it came in.
    pub frame: u64,
    /// Its position in the frame (0 … 11); FIBs 3c … 3c + 2 belong to CIF c.
    pub pos: u8,
    /// The 32 bytes (30 data bytes and the CRC).
    pub data: [u8; 32],
    /// The CRC checked.
    pub ok: bool,
}

/// The FIC decoder.
pub struct FicDecoder {
    decoder: ProfileDecoder,
    bytes: [u8; 96],
    /// Good FIBs since the reset.
    pub ok: u64,
    /// Bad FIBs since the reset.
    pub bad: u64,
    /// Good FIBs in the last frame.
    pub ok_last_frame: u8,
}

impl Default for FicDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FicDecoder {
    /// A decoder.
    pub fn new() -> Self {
        FicDecoder {
            decoder: ProfileDecoder::new(&fic_profile())
                .unwrap_or_else(|_| unreachable!("the FIC profile is a table constant")),
            bytes: [0; 96],
            ok: 0,
            bad: 0,
            ok_last_frame: 0,
        }
    }

    /// Decodes the 9216 FIC LLRs of a frame into its 12 FIBs. A frame of another length produces no FIBs.
    pub fn decode(&mut self, frame: u64, soft: &[i8], out: &mut Vec<Fib>) {
        self.ok_last_frame = 0;
        if soft.len() != FIC_BITS {
            return;
        }
        for c in 0..4 {
            let block = &soft[c * FIC_BLOCK_BITS..(c + 1) * FIC_BLOCK_BITS];
            if self.decoder.decode(block, &mut self.bytes).is_err() {
                continue;
            }
            for f in 0..3 {
                let mut data = [0u8; 32];
                data.copy_from_slice(&self.bytes[f * 32..f * 32 + 32]);
                let ok = crc_ok(&data);
                if ok {
                    self.ok += 1;
                    self.ok_last_frame += 1;
                } else {
                    self.bad += 1;
                }
                out.push(Fib {
                    frame,
                    pos: (3 * c + f) as u8,
                    data,
                    ok,
                });
            }
        }
    }
}

/// The FIGs of a good FIB: (type, the data field after the header byte). Walks the 30 data bytes: 0xFF ends the FIB, a FIG that
/// goes past byte 30 ends the walk, a FIG of length 0 is skipped. It also returns the bytes used (headers included).
pub fn walk(fib: &[u8; 32], mut f: impl FnMut(u8, &[u8])) -> usize {
    let mut i = 0usize;
    while i < 30 {
        let h = fib[i];
        if h == 0xFF {
            break;
        }
        let ty = h >> 5;
        let len = (h & 31) as usize;
        if len == 0 {
            i += 1;
            continue;
        }
        if i + 1 + len > 30 {
            break;
        }
        f(ty, &fib[i + 1..i + 1 + len]);
        i += 1 + len;
    }
    i.min(30)
}
