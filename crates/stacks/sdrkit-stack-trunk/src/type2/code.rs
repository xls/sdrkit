//! The outbound signalling word (OSW) of the Motorola Type II control channel: 8 sync bits and 76 payload bits that carry 38
//! information bits, each with a parity bit (a convolutional code of rate 1/2 with two states), interleaved 19 rows by 4. The information bits are a masked address, the group
//! flag, a masked command, 10 inverted check bits and a flush bit. No public document confirms any of this.

use sdrkit_algo_fec::conv::{ConvCode, Ends, Viterbi};

/// The bits of a word.
pub const WORD_BITS: usize = 84;
/// The sync bits.
pub const SYNC: u8 = 0xAC;
/// The payload bits.
pub const PAYLOAD_BITS: usize = 76;
/// The information bits.
pub const INFO_BITS: usize = 38;
/// The mask of the address.
pub const ADDRESS_MASK: u16 = 0xCC38;
/// The mask of the command.
pub const COMMAND_MASK: u16 = 0x0D5;
/// The check of 27 zero bits.
pub const CHECK_START: u16 = 0x393;

/// The check masks of the 27 information bits that the check covers.
const MASKS: [u16; 27] = check_masks();

/// The masks: a register started at 36E that, before each bit, shifts right by one and XORs 225 when its lowest bit was 1.
const fn check_masks() -> [u16; 27] {
    let mut m = [0u16; 27];
    let mut reg: u16 = 0x36E;
    let mut j = 0;
    while j < 27 {
        let lo = reg & 1;
        reg >>= 1;
        if lo == 1 {
            reg ^= 0x225;
        }
        m[j] = reg;
        j += 1;
    }
    m
}

/// One word: the address and command as the system means them, and the group flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Osw {
    /// The address (16 bits).
    pub address: u16,
    /// A group word (false: individual).
    pub group: bool,
    /// The command (10 bits).
    pub command: u16,
}

impl Osw {
    /// A word.
    pub const fn new(address: u16, group: bool, command: u16) -> Osw {
        Osw {
            address,
            group,
            command: command & 0x3FF,
        }
    }
}

/// The check of the 27 masked information bits.
pub fn check(info: &[u8; INFO_BITS]) -> u16 {
    let mut c = CHECK_START;
    for (j, m) in MASKS.iter().enumerate() {
        if info[j] == 1 {
            c ^= m;
        }
    }
    c
}

/// The 38 information bits of a word.
pub fn info_bits(w: Osw) -> [u8; INFO_BITS] {
    let mut info = [0u8; INFO_BITS];
    let a = w.address ^ ADDRESS_MASK;
    let c = (w.command & 0x3FF) ^ COMMAND_MASK;
    for (k, b) in info[..16].iter_mut().enumerate() {
        *b = ((a >> (15 - k)) & 1) as u8;
    }
    info[16] = u8::from(!w.group);
    for k in 0..10 {
        info[17 + k] = ((c >> (9 - k)) & 1) as u8;
    }
    let chk = check(&info);
    for k in 0..10 {
        info[27 + k] = 1 - ((chk >> (9 - k)) & 1) as u8;
    }
    info
}

/// The place in the sent payload of pair bit `r`: the 76 bits are 19 rows of 4, sent column by column.
const fn sent_index(r: usize) -> usize {
    (r % 4) * 19 + r / 4
}

/// The 84 bits of a word as sent: the sync, then the interleaved payload.
pub fn encode(w: Osw) -> [u8; WORD_BITS] {
    let info = info_bits(w);
    let mut out = [0u8; WORD_BITS];
    for (k, b) in out[..8].iter_mut().enumerate() {
        *b = (SYNC >> (7 - k)) & 1;
    }
    let mut prev = 0;
    for (i, &b) in info.iter().enumerate() {
        out[8 + sent_index(2 * i)] = b;
        out[8 + sent_index(2 * i + 1)] = b ^ prev;
        prev = b;
    }
    out
}

/// A decoded payload: the word and the information bits repaired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The word.
    pub osw: Osw,
    /// Information bits that differ from the hard decisions.
    pub repaired: u8,
}

/// The pair code: each information bit is sent, then its sum with the bit before it.
const PAIR_CODE: ConvCode = ConvCode::new(2, &[0b10, 0b11]);

/// The LLR of a soft value of the mean size of its word.
const LLR_UNIT: f32 = 64.0;
/// The largest LLR: half the mean size.
const LLR_LIMIT: f32 = 32.0;

/// The decoder of a payload: the most likely sequence of the pair code for the soft values, then the check.
pub struct WordDecoder {
    viterbi: Viterbi,
}

impl Default for WordDecoder {
    fn default() -> Self {
        WordDecoder {
            viterbi: Viterbi::new(PAIR_CODE, INFO_BITS).expect("the pair code is valid"),
        }
    }
}

impl WordDecoder {
    /// Decodes 76 soft payload values in sent order (positive is a 1; the size is the reliability): undoes the interleave,
    /// finds the most likely information bits, compares the check, unmasks. `None` when the check fails.
    pub fn decode(&mut self, payload: &[f32]) -> Option<Decoded> {
        if payload.len() < PAYLOAD_BITS {
            return None;
        }
        let mean =
            payload[..PAYLOAD_BITS].iter().map(|v| v.abs()).sum::<f32>() / PAYLOAD_BITS as f32;
        if mean <= 0.0 || !mean.is_finite() {
            return None;
        }
        // A positive LLR means 0. The values are limited: below the FM threshold a click gives a large wrong value.
        let mut llr = [0i8; PAYLOAD_BITS];
        for (i, l) in llr.iter_mut().enumerate() {
            let v = -payload[sent_index(i)] / mean;
            *l = (v * LLR_UNIT).clamp(-LLR_LIMIT, LLR_LIMIT) as i8;
        }
        let mut info = [0u8; INFO_BITS];
        self.viterbi
            .decode_only(&llr, Ends::Start(0), &mut info)
            .ok()?;
        let mut chk = 0u16;
        for k in 0..10 {
            chk = (chk << 1) | u16::from(1 - info[27 + k]);
        }
        if check(&info) != chk {
            return None;
        }
        let repaired = (0..INFO_BITS)
            .filter(|&k| info[k] != u8::from(payload[sent_index(2 * k)] > 0.0))
            .count() as u8;
        let bits =
            |from: usize, n: usize| (0..n).fold(0u16, |v, k| (v << 1) | u16::from(info[from + k]));
        Some(Decoded {
            osw: Osw {
                address: bits(0, 16) ^ ADDRESS_MASK,
                group: info[16] == 0,
                command: bits(17, 10) ^ COMMAND_MASK,
            },
            repaired,
        })
    }

    /// Decodes 76 hard payload bits in sent order, all of the same reliability.
    pub fn decode_bits(&mut self, payload: &[u8]) -> Option<Decoded> {
        if payload.len() < PAYLOAD_BITS {
            return None;
        }
        let mut soft = [0f32; PAYLOAD_BITS];
        for (v, b) in soft.iter_mut().zip(payload) {
            *v = if b & 1 == 1 { 1.0 } else { -1.0 };
        }
        self.decode(&soft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_masks_and_the_check_of_zero_bits() {
        assert_eq!(
            MASKS,
            [
                0x1B7, 0x2FE, 0x17F, 0x29A, 0x14D, 0x283, 0x364, 0x1B2, 0x0D9, 0x249, 0x301, 0x3A5,
                0x3F7, 0x3DE, 0x1EF, 0x2D2, 0x169, 0x291, 0x36D, 0x393, 0x3EC, 0x1F6, 0x0FB, 0x258,
                0x12C, 0x096, 0x04B
            ]
        );
        assert_eq!(check(&[0; INFO_BITS]), 0x393);
    }

    #[test]
    fn the_worked_example() {
        let w = Osw::new(0x1232, true, 0x014);
        let mut c = 0u16;
        let info = info_bits(w);
        for k in 0..10 {
            c = (c << 1) | u16::from(1 - info[27 + k]);
        }
        assert_eq!(c, 0xC6);
        let bits: String = encode(w).iter().map(|b| char::from(b'0' + b)).collect();
        assert_eq!(
            bits,
            "101011001011001100100110101110000110000010001111100000010001011000101001101100011001"
        );
        assert_eq!(
            WordDecoder::default().decode_bits(&encode(w)[8..]),
            Some(Decoded {
                osw: w,
                repaired: 0
            })
        );
    }
}
