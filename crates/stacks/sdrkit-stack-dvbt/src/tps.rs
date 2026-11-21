//! Transmission parameter signalling: the 68 bits of a frame, the sync words and the fields, protected by the shared BCH(67,53)
//! code, which corrects up to two errors.
//!
//! Standard: ETSI EN 300 744 clause 4.6.

use crate::params::{CodeRate, Constellation, Guard, Hierarchy, Mode, TpsParams};
use sdrkit_algo_fec::bch::{codes::DVB_T_TPS_67_53, Bch};
use std::cell::RefCell;

/// The sync word of frames 1 and 3 (s1 to s16). Frames 2 and 4 send its inverse.
pub const SYNC_ODD: u16 = 0b0011_0101_1110_1110;

thread_local! {
    /// The BCH(67,53) code of the TPS, one decoder for each thread (its decoding needs working memory).
    static BCH: RefCell<Bch> = RefCell::new(
        Bch::from_code(&DVB_T_TPS_67_53).unwrap_or_else(|_| unreachable!("the TPS code is a valid BCH code")),
    );
}

/// Writes the 14 BCH parity bits s54 to s67 of the 53 information bits s1 to s53 (clause 4.6.2: BCH(67,53), the BCH(127,113)
/// code shortened, s1 sent first).
pub fn protect(s: &mut [u8; 68]) {
    BCH.with(|b| {
        let mut w = [0u8; 67];
        w[..53].copy_from_slice(&s[1..54]);
        if b.borrow().encode(&mut w).is_ok() {
            s[54..68].copy_from_slice(&w[53..]);
        }
    });
}

/// Checks (and with `correct`, repairs up to two errors in) s1 to s67, given as 67 bits. Returns the number of bits corrected, or
/// `None` when the word is not a code word (within reach).
pub fn repair(bits: &mut [u8; 67], correct: bool) -> Option<usize> {
    BCH.with(|b| {
        let mut b = b.borrow_mut();
        if !correct {
            return b.is_codeword(bits).then_some(0);
        }
        b.decode(bits).ok().map(|d| d.corrected as usize)
    })
}

/// The 68 TPS bits of frame `frame` (0 to 3): s0 (the start of the differential coding, sent as 0) to s67.
pub fn encode(p: &TpsParams, frame: u8) -> [u8; 68] {
    encode_with(p, frame, None)
}

/// As [`encode`], with the LP code-rate field set to a raw value (a test of the reserved values some transmitters send).
pub fn encode_with(p: &TpsParams, frame: u8, lp_field: Option<u8>) -> [u8; 68] {
    let mut s = [0u8; 68];
    fn put(s: &mut [u8; 68], at: usize, n: usize, v: u32) {
        for k in 0..n {
            s[at + k] = ((v >> (n - 1 - k)) & 1) as u8;
        }
    }
    let sync = if frame.is_multiple_of(2) {
        SYNC_ODD
    } else {
        !SYNC_ODD
    };
    put(&mut s, 1, 16, sync as u32);
    put(&mut s, 17, 6, p.length as u32);
    put(&mut s, 23, 2, frame as u32 & 3);
    put(&mut s, 25, 2, p.constellation as u32);
    put(&mut s, 27, 3, p.hierarchy as u32);
    put(&mut s, 30, 3, p.rate_hp.code() as u32);
    put(&mut s, 33, 3, lp_field.unwrap_or(p.rate_lp.code()) as u32);
    put(&mut s, 36, 2, p.guard.code() as u32);
    put(&mut s, 38, 2, p.mode as u32);
    let cell = if frame.is_multiple_of(2) {
        p.cell_id >> 8
    } else {
        p.cell_id & 0xFF
    };
    put(&mut s, 40, 8, cell as u32);
    put(&mut s, 48, 6, p.dvbh as u32);
    protect(&mut s);
    s
}

/// Why a TPS block was not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TpsError {
    /// No sync word.
    Sync,
    /// The BCH check failed.
    Bch,
    /// A reserved value.
    Reserved(&'static str),
    /// A feature sdrkit does not decode (4K, in-depth interleaver).
    Unsupported(&'static str),
}

/// A decoded TPS block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TpsBlock {
    /// The parameters (cell id: only this frame's half, in the right byte).
    pub params: TpsParams,
    /// Frame number 0 to 3.
    pub frame: u8,
    /// The 8 cell-id bits of this frame (b15..b8 in frames 1 and 3, b7..b0 in 2 and 4).
    pub cell_half: u8,
    /// Bits corrected.
    pub corrected: usize,
    /// The frame's bits s0 to s67 after correction (bit `i` of the value is s_i).
    pub word: u128,
}

/// Decodes the 68 bits s0..s67 of a frame. `correct`: allow BCH correction of up to 2 errors (sync word included).
pub fn decode(bits: &[u8; 68], correct: bool) -> Result<TpsBlock, TpsError> {
    decode_reading(bits, correct, false)
}

/// As [`decode`], in the DVB-T reading or (`sh`) the DVB-SH reading of ETSI EN 302 583 clause 5.7.4.3: s53 must be 1, the
/// constellation is QPSK or 16-QAM, the mode codes 10 and 11 are 4K and 1K, and the fields that change from frame to frame
/// (the puncturing patterns, the interleaver bits, the super-frame number) are left in [`TpsBlock::word`]: the code rates of
/// the parameters are then 1/2 and `dvbh` is 0.
pub fn decode_reading(bits: &[u8; 68], correct: bool, sh: bool) -> Result<TpsBlock, TpsError> {
    let mut b = [0u8; 67];
    b.copy_from_slice(&bits[1..68]);
    if !correct {
        let sync = word(&b, 0, 16) as u16;
        if sync != SYNC_ODD && sync != !SYNC_ODD {
            return Err(TpsError::Sync);
        }
    }
    let corrected = repair(&mut b, correct).ok_or(TpsError::Bch)?;
    let sync = word(&b, 0, 16) as u16;
    if sync != SYNC_ODD && sync != !SYNC_ODD {
        return Err(TpsError::Sync);
    }
    let get = |at: usize, n: usize| word(&b, at - 1, n);
    let frame = get(23, 2) as u8;
    if frame.is_multiple_of(2) != (sync == SYNC_ODD) {
        return Err(TpsError::Sync);
    }
    if sh && get(53, 1) != 1 {
        return Err(TpsError::Unsupported("a DVB-T frame in the DVB-SH reading"));
    }
    let constellation = match get(25, 2) {
        0 => Constellation::Qpsk,
        1 => Constellation::Qam16,
        2 if !sh => Constellation::Qam64,
        _ => return Err(TpsError::Reserved("constellation")),
    };
    let h = get(27, 3);
    if h & 4 != 0 {
        return Err(TpsError::Unsupported("in-depth interleaver"));
    }
    let hierarchy = Hierarchy::ALL[h as usize & 3];
    let word = std::iter::once(bits[0])
        .chain(b.iter().copied())
        .enumerate()
        .fold(0u128, |a, (i, x)| a | (x as u128 & 1) << i);
    if sh {
        let mode = match get(38, 2) {
            0 => Mode::K2,
            1 => Mode::K8,
            2 => Mode::K4,
            _ => Mode::K1,
        };
        let cell_half = get(40, 8) as u8;
        return Ok(TpsBlock {
            params: TpsParams {
                mode,
                guard: Guard::from_code(get(36, 2) as u8),
                constellation,
                hierarchy,
                rate_hp: CodeRate::R12,
                rate_lp: CodeRate::R12,
                cell_id: if frame.is_multiple_of(2) {
                    (cell_half as u16) << 8
                } else {
                    cell_half as u16
                },
                length: get(17, 6) as u8,
                dvbh: 0,
            },
            frame,
            cell_half,
            corrected,
            word,
        });
    }
    let rate_hp =
        CodeRate::from_code(get(30, 3) as u8).ok_or(TpsError::Reserved("HP code rate"))?;
    // A non-hierarchical multiplex can leave a reserved LP rate: it is read as the HP rate.
    let rate_lp = match CodeRate::from_code(get(33, 3) as u8) {
        Some(r) => r,
        None if !hierarchy.hierarchical() => rate_hp,
        None => return Err(TpsError::Reserved("LP code rate")),
    };
    let guard = Guard::from_code(get(36, 2) as u8);
    let mode = match get(38, 2) {
        0 => Mode::K2,
        1 => Mode::K8,
        2 => return Err(TpsError::Unsupported("4K mode (DVB-H)")),
        _ => return Err(TpsError::Reserved("transmission mode")),
    };
    let cell_half = get(40, 8) as u8;
    let params = TpsParams {
        mode,
        guard,
        constellation,
        hierarchy,
        rate_hp,
        rate_lp,
        cell_id: if frame.is_multiple_of(2) {
            (cell_half as u16) << 8
        } else {
            cell_half as u16
        },
        length: get(17, 6) as u8,
        dvbh: get(48, 6) as u8,
    };
    Ok(TpsBlock {
        params,
        frame,
        cell_half,
        corrected,
        word,
    })
}

fn word(b: &[u8], at: usize, n: usize) -> u32 {
    b[at..at + n].iter().fold(0u32, |a, &x| (a << 1) | x as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f4_tps_fields_and_correction() {
        let p = TpsParams {
            mode: Mode::K8,
            guard: Guard::G8,
            constellation: Constellation::Qam16,
            rate_hp: CodeRate::R34,
            rate_lp: CodeRate::R34,
            cell_id: 0x1234,
            length: 31,
            ..Default::default()
        };
        for f in 0..4u8 {
            let bits = encode(&p, f);
            let d = decode(&bits, false).unwrap();
            assert_eq!(d.frame, f);
            assert_eq!(
                (
                    d.params.mode,
                    d.params.guard,
                    d.params.constellation,
                    d.params.rate_hp,
                    d.params.length
                ),
                (Mode::K8, Guard::G8, Constellation::Qam16, CodeRate::R34, 31)
            );
            assert_eq!(d.cell_half, if f % 2 == 0 { 0x12 } else { 0x34 });
            // One flipped protected bit (s40): rejected without correction, repaired with it.
            let mut b = bits;
            b[40] ^= 1;
            assert!(decode(&b, false).is_err());
            assert_eq!(decode(&b, true).unwrap().corrected, 1);
            for other in [3usize, 66] {
                let mut b = bits;
                b[40] ^= 1;
                b[other] ^= 1;
                let d = decode(&b, true).unwrap();
                assert_eq!(d.corrected, 2);
                assert_eq!(d.cell_half, if f % 2 == 0 { 0x12 } else { 0x34 });
            }
        }
        // A reserved LP rate in a non-hierarchical multiplex reads as the HP rate.
        let mut bits = encode(&p, 0);
        for (k, v) in [(33, 1), (34, 1), (35, 1)] {
            bits[k] = v;
        }
        protect(&mut bits);
        assert_eq!(decode(&bits, false).unwrap().params.rate_lp, CodeRate::R34);
    }
}
