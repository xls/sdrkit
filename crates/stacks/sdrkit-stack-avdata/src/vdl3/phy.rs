//! The VDL Mode 3 burst: the four training sequences, the burst kinds and their lengths, the Golay (24, 12) words, the
//! Reed-Solomon (72, 62) word of a data burst, the scrambler restarted after the training, and the bits of a burst both ways.
//! Bits are `u8`, one per bit, 0 or 1, in air order.
//! Standard: ICAO Annex 10 Volume III Part I, 6.4.2 (modulation), 6.4.3.1.4 (scrambler), 6.4.4 (bursts, training sequences,
//! Golay and Reed-Solomon codes).

use crate::vdl2::phy::{bits_of, scrambler, step_of};
use sdrkit_algo_fec::rs::{codes::VDL3_72_62, Rs};
use sdrkit_algo_fec::small::SmallCode;
use serde::{Deserialize, Serialize};

/// Symbols per second.
pub const SYMBOL_RATE: f64 = 10_500.0;
/// Symbols of a training sequence.
pub const TRAINING_LEN: usize = 16;
/// Symbols of the ramp before the training.
pub const RAMP_LEN: usize = 3;
/// The envelope of the ramp symbols.
pub const RAMP: [f32; RAMP_LEN] = [0.2, 0.6, 0.9];
/// Symbols after the training of a voice or data burst.
pub const VD_SYMBOLS: usize = 200;
/// Octets of the Reed-Solomon word of a data burst.
pub const RS_N: usize = 72;
/// Data octets of the Reed-Solomon word.
pub const RS_K: usize = 62;

/// The rows of P of the Golay generator `G = [P | I12]`; row 1 belongs to the most significant data bit.
pub const GOLAY_P: [u32; 12] = [
    0b1101_0111_0001,
    0b0111_1100_1001,
    0b1110_1001_0101,
    0b0110_0011_1011,
    0b1110_0110_1100,
    0b1011_0011_0110,
    0b1001_1001_1011,
    0b0101_1011_1100,
    0b0010_1101_1110,
    0b0001_0110_1111,
    0b1101_1100_0110,
    0b1010_1110_0011,
];

sdrkit_core::choice_enum! {
    /// The training sequences.
    #[derive(Serialize, Deserialize)]
    pub enum Training {
        /// S1: M downlink.
        S1 = ("s1", "S1"),
        /// S1*: net entry request.
        S1Star = ("s1*", "S1*"),
        /// S2: voice or data.
        S2 = ("s2", "S2"),
        /// S2*: M uplink, handoff check, poll response.
        S2Star = ("s2*", "S2*"),
    }
}

/// The bits of the training sequences S1 and S2 in air order, three bits a symbol.
const S1_BITS: &str = "000111001001010110000011100110011111010101100101";
const S2_BITS: &str = "000111011010000100001010100101011110001110101111";

/// The phase steps (units of pi/4) of the 16 symbols of S1 (`second == false`) or S2, the first from the ramp phase.
pub fn steps(second: bool) -> [i32; TRAINING_LEN] {
    let b = if second { S2_BITS } else { S1_BITS }.as_bytes();
    std::array::from_fn(|k| {
        let v = (0..3).fold(0u8, |v, i| (v << 1) | (b[3 * k + i] - b'0'));
        i32::from(step_of(v))
    })
}

impl Training {
    /// The phase steps of the sequence: those of S1 or S2, every change turned by 180 degrees for S1* and S2*.
    pub fn steps(self) -> [i32; TRAINING_LEN] {
        let second = matches!(self, Training::S2 | Training::S2Star);
        let mut s = steps(second);
        if matches!(self, Training::S1Star | Training::S2Star) {
            for v in s.iter_mut().skip(1) {
                *v = (*v + 4) % 8;
            }
        }
        s
    }
}

sdrkit_core::choice_enum! {
    /// The burst kinds.
    #[derive(Serialize, Deserialize)]
    pub enum BurstKind {
        /// M uplink: four Golay words of system data.
        MUplink = ("m_uplink", "M uplink"),
        /// M uplink of three slots: sixteen Golay words.
        MUplink3T = ("m_uplink_3t", "M uplink (3T)"),
        /// Handoff check: five Golay words.
        Handoff = ("handoff", "H handoff check"),
        /// Poll response: two Golay words.
        PollResponse = ("poll_response", "Poll response"),
        /// M downlink: two Golay words.
        MDownlink = ("m_downlink", "M downlink"),
        /// Net entry request: two Golay words.
        NetEntry = ("net_entry", "Net entry request"),
        /// Voice or data burst holding data: a header word and a Reed-Solomon word.
        VdData = ("vd_data", "V/D data"),
        /// Voice or data burst holding voice (or data that failed Reed-Solomon).
        VdVoice = ("vd_voice", "V/D voice"),
    }
}

impl BurstKind {
    /// The training sequence of the kind.
    pub fn training(self) -> Training {
        match self {
            BurstKind::MDownlink => Training::S1,
            BurstKind::NetEntry => Training::S1Star,
            BurstKind::VdData | BurstKind::VdVoice => Training::S2,
            _ => Training::S2Star,
        }
    }
    /// Symbols after the training.
    pub fn symbols(self) -> usize {
        match self {
            BurstKind::MUplink => 32,
            BurstKind::MUplink3T => 128,
            BurstKind::Handoff => 40,
            BurstKind::VdData | BurstKind::VdVoice => VD_SYMBOLS,
            _ => 16,
        }
    }
    /// Golay words of the burst (one header word for voice and data).
    pub fn words(self) -> usize {
        match self {
            BurstKind::VdData | BurstKind::VdVoice => 1,
            k => k.symbols() * 3 / 24,
        }
    }
    /// The kind of an S2* burst of `symbols`, `None` for another length.
    pub fn of_s2star(symbols: usize) -> Option<BurstKind> {
        Some(match symbols {
            32 => BurstKind::MUplink,
            128 => BurstKind::MUplink3T,
            40 => BurstKind::Handoff,
            16 => BurstKind::PollResponse,
            _ => return None,
        })
    }
}

/// The lengths of an S2* burst, in the order the receiver tries them.
pub const S2STAR_LENGTHS: [usize; 4] = [128, 40, 32, 16];

sdrkit_core::choice_enum! {
    /// The bit order of the Reed-Solomon octets on air.
    #[derive(Serialize, Deserialize)]
    pub enum OctetOrder {
        /// Each octet least significant bit first.
        LsbFirst = ("lsb_first", "Least significant bit first"),
        /// Each octet most significant bit first.
        MsbFirst = ("msb_first", "Most significant bit first"),
    }
}

/// The Golay (24, 12) code with its syndrome table. A word is a 24-bit value, the first bit sent most significant: the 12
/// parity bits, then the 12 data bits.
#[derive(Clone, Debug)]
pub struct Golay {
    code: SmallCode,
}

impl Default for Golay {
    fn default() -> Self {
        Golay {
            code: SmallCode::from_parity_rows(24, 12, &GOLAY_P)
                .unwrap_or_else(|_| unreachable!("the rows of P make a (24, 12) code")),
        }
    }
}

/// The two 12-bit halves of a 24-bit value swapped.
fn swap(w: u32) -> u32 {
    ((w & 0xFFF) << 12) | (w >> 12)
}

impl Golay {
    /// The word of 12 data bits.
    pub fn encode(&self, data: u16) -> u32 {
        swap(self.code.encode(u128::from(data & 0xFFF)) as u32)
    }
    /// The data bits of a received word and the bits corrected, `None` when more than 3 bits are wrong (4 are always
    /// detected).
    pub fn decode(&self, word: u32) -> Option<(u16, u8)> {
        let (cw, n) = self.code.decode(u128::from(swap(word & 0xFF_FFFF)))?;
        Some((self.code.info(cw) as u16, n))
    }
}

/// Appends the 24 bits of a word, the first sent first.
pub fn push_word(w: u32, out: &mut Vec<u8>) {
    out.extend((0..24).rev().map(|i| ((w >> i) & 1) as u8));
}

/// The 24-bit value of 24 bits, the first most significant.
pub fn word_of(bits: &[u8]) -> u32 {
    bits.iter()
        .take(24)
        .fold(0, |w, &b| (w << 1) | u32::from(b & 1))
}

/// Appends the 8 bits of an octet in `order`.
pub fn push_octet(o: u8, order: OctetOrder, out: &mut Vec<u8>) {
    match order {
        OctetOrder::LsbFirst => out.extend((0..8).map(|i| (o >> i) & 1)),
        OctetOrder::MsbFirst => out.extend((0..8).rev().map(|i| (o >> i) & 1)),
    }
}

/// The octet of 8 bits in `order`.
pub fn octet_of(bits: &[u8], order: OctetOrder) -> u8 {
    bits.iter()
        .take(8)
        .enumerate()
        .fold(0, |o, (i, &b)| match order {
            OctetOrder::LsbFirst => o | (b & 1) << i,
            OctetOrder::MsbFirst => o | (b & 1) << (7 - i),
        })
}

/// What a burst carries.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// Golay information words of 12 bits (M, handoff, poll and net entry bursts).
    Words(Vec<u16>),
    /// The header word and up to 62 data octets (padded with zeros).
    Data(u16, Vec<u8>),
    /// The header word and 576 bits of vocoder frames.
    Voice(u16, Vec<u8>),
}

/// The payload bits of a burst of `kind` (after the training, scrambled), the Golay words and the Reed-Solomon word coded.
/// The content must fit the kind: `Words` with [`BurstKind::words`] words, `Data` for V/D data, `Voice` (576 bits) for V/D
/// voice.
pub fn payload_bits(kind: BurstKind, content: &Content, order: OctetOrder, out: &mut Vec<u8>) {
    let golay = Golay::default();
    let start = out.len();
    match content {
        Content::Words(w) => {
            for i in 0..kind.words() {
                push_word(golay.encode(w.get(i).copied().unwrap_or(0)), out);
            }
        }
        Content::Data(h, d) => {
            push_word(golay.encode(*h), out);
            let mut word = [0u8; RS_N];
            for (o, b) in word.iter_mut().zip(d.iter().take(RS_K)) {
                *o = *b;
            }
            if let Ok(rs) = Rs::new(VDL3_72_62) {
                let (data, parity) = word.split_at_mut(RS_K);
                let _ = rs.encode(data, parity);
            }
            for o in word {
                push_octet(o, order, out);
            }
        }
        Content::Voice(h, bits) => {
            push_word(golay.encode(*h), out);
            out.extend(bits.iter().take(RS_N * 8).map(|b| b & 1));
            while out.len() - start < 24 + RS_N * 8 {
                out.push(0);
            }
        }
    }
    let mut s = scrambler();
    for b in out[start..].iter_mut() {
        *b ^= s.bit();
    }
}

/// The phase steps of bits, three bits a symbol, the first sent most significant.
pub fn steps_of_bits(bits: &[u8], out: &mut Vec<u8>) {
    out.extend(
        bits.chunks(3)
            .map(|c| step_of(c.iter().fold(0, |v, &b| (v << 1) | (b & 1)) << (3 - c.len()))),
    );
}

/// Appends the three bits of each phase step.
pub fn bits_of_steps(steps: &[u8], out: &mut Vec<u8>) {
    for &s in steps {
        let v = bits_of(s);
        out.extend([(v >> 2) & 1, (v >> 1) & 1, v & 1]);
    }
}

/// A decoded payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Payload {
    /// The information words (for V/D, the header).
    pub words: Vec<u16>,
    /// Every Golay word decoded.
    pub golay_ok: bool,
    /// Bits corrected by Golay.
    pub golay_corrected: u32,
    /// The Reed-Solomon word was good (V/D).
    pub rs_ok: bool,
    /// Octets corrected by Reed-Solomon.
    pub rs_corrected: u32,
    /// The 72 octets as received (V/D), the 62 data octets corrected when Reed-Solomon passed.
    pub octets: Vec<u8>,
}

/// The payload decoder: the codes and buffers, kept from burst to burst.
pub struct PayloadDecoder {
    golay: Golay,
    rs: Rs,
}

impl Default for PayloadDecoder {
    fn default() -> Self {
        PayloadDecoder {
            golay: Golay::default(),
            rs: Rs::new(VDL3_72_62).unwrap_or_else(|_| unreachable!("RS(72, 62) is a valid code")),
        }
    }
}

impl PayloadDecoder {
    /// Descrambles `bits` (the payload of a burst of `words` Golay words, and the Reed-Solomon word when `vd`) in place and
    /// decodes them into `out`.
    pub fn decode(
        &mut self,
        bits: &mut [u8],
        words: usize,
        vd: bool,
        order: OctetOrder,
        out: &mut Payload,
    ) {
        let mut s = scrambler();
        for b in bits.iter_mut() {
            *b ^= s.bit();
        }
        out.words.clear();
        out.golay_ok = true;
        out.golay_corrected = 0;
        out.rs_ok = false;
        out.rs_corrected = 0;
        out.octets.clear();
        for i in 0..words {
            let w = word_of(bits.get(24 * i..24 * i + 24).unwrap_or(&[]));
            match self.golay.decode(w) {
                Some((d, n)) => {
                    out.words.push(d);
                    out.golay_corrected += u32::from(n);
                }
                None => {
                    out.words.push((w & 0xFFF) as u16);
                    out.golay_ok = false;
                }
            }
        }
        if !vd {
            return;
        }
        let at = 24 * words;
        out.octets
            .extend((0..RS_N).map(|k| octet_of(bits.get(at + 8 * k..).unwrap_or(&[]), order)));
        let mut word = [0u8; RS_N];
        word.copy_from_slice(&out.octets);
        if let Ok(d) = self.rs.decode(&mut word, &[]) {
            out.rs_ok = true;
            out.rs_corrected = d.corrected;
            out.octets.clear();
            out.octets.extend_from_slice(&word[..RS_K]);
        }
    }
}
