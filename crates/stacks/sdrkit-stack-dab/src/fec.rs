//! The channel coding that DAB puts on top of the shared codes: the puncturing pattern of a protection profile, the decoder of a
//! profile's code words, the energy dispersal and the CRC of FIBs and data. LLRs follow the shared rule: positive means 0.
//!
//! Standard: EN 300 401 clauses 5.2.1, 10, 11 and 12.

use crate::tables::Profile;
use sdrkit_algo_fec::conv::codes::{DAB_K7_R4, DAB_PI, DAB_TAIL};
use sdrkit_algo_fec::conv::{depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::Llr8;
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::lfsr::Period;
use sdrkit_util_bits::seq::PRBS9_DISPERSAL;
use sdrkit_util_bits::{BitOrder, Pad};

/// The energy dispersal sequence as bytes, most significant bit first: 511 bytes hold eight whole periods of 511 bits, so a
/// byte string of any length wraps round correctly. EN 300 401 clause 10.
pub static DISPERSAL: Period<511> = Period::of(PRBS9_DISPERSAL, BitOrder::Msb);

/// Whether `data` ends with the CRC-16 of the bytes before it, high byte first (EN 300 401 clause 5.2.1, CRC-16/GENIBUS).
pub fn crc_ok(data: &[u8]) -> bool {
    data.len() >= 2 && CRC16_GENIBUS.verify(data, Trailer::Big)
}

/// The CRC-16 of `data` (EN 300 401 clause 5.2.1, CRC-16/GENIBUS).
pub fn crc_of(data: &[u8]) -> u16 {
    CRC16_GENIBUS.checksum(data)
}

/// The mother-code positions that `p` keeps, one flag (0 or 1) per position: the puncturing vectors of its runs, four times
/// each per 128 positions, then the tail vector. The length is `4 × (information bits + 6)`. EN 300 401 clause 11.1.2.
pub fn keep_mask(p: &Profile) -> Vec<u8> {
    let mut m = Vec::with_capacity(4 * (p.info_bits + 6));
    for &(blocks, pi) in &p.runs {
        let v = DAB_PI[pi - 1].keep();
        for _ in 0..4 * blocks {
            m.extend_from_slice(v);
        }
    }
    m.extend_from_slice(DAB_TAIL.keep());
    m
}

/// The decoder of one protection profile: depuncturing, the K = 7 Viterbi decoder and the energy dispersal. Every buffer is
/// sized when it is made.
#[derive(Clone, Debug)]
pub struct ProfileDecoder {
    mask: Vec<u8>,
    mother: Vec<Llr8>,
    bits: Vec<u8>,
    viterbi: Viterbi,
}

impl ProfileDecoder {
    /// A decoder for `p`. An error when the profile has no positions to keep (it cannot come from the tables).
    pub fn new(p: &Profile) -> Result<Self, sdrkit_algo_fec::FecError> {
        let mask = keep_mask(p);
        Puncture::try_new(&mask)?;
        let steps = mask.len() / 4;
        Ok(ProfileDecoder {
            mother: vec![0; mask.len()],
            bits: vec![0; steps - 6],
            viterbi: Viterbi::new(DAB_K7_R4, steps)?,
            mask,
        })
    }

    /// The number of coded bits (the LLRs [`ProfileDecoder::decode`] takes).
    pub fn coded_bits(&self) -> usize {
        self.mask.iter().filter(|&&k| k == 1).count()
    }

    /// Decodes the coded LLRs `llr` into `out` (bytes, most significant bit first, energy dispersal removed). `out` must hold
    /// the information bits divided by 8. An error when the lengths do not fit the profile.
    pub fn decode(
        &mut self,
        llr: &[Llr8],
        out: &mut [u8],
    ) -> Result<(), sdrkit_algo_fec::FecError> {
        let p = Puncture::try_new(&self.mask)?;
        depuncture_frame(&p, 0, llr, &mut self.mother)?;
        self.viterbi
            .decode(&self.mother, Ends::Zero, &mut self.bits)?;
        if out.len() * 8 != self.bits.len() {
            return Err(sdrkit_algo_fec::FecError::InputLength {
                expected: self.bits.len() / 8,
                got: out.len(),
            });
        }
        // The lengths were checked just above, so the bits fill `out` exactly.
        let _ = sdrkit_util_bits::pack::pack_into(&self.bits, BitOrder::Msb, Pad::Zero, out);
        DISPERSAL.apply(out, 0);
        Ok(())
    }
}

/// Encodes `data` (bytes) for profile `p`: energy dispersal, the K = 7 code with its tail, and puncturing. Appends the coded
/// bits (one per byte) to `out`. For the test signal.
pub fn encode_profile(p: &Profile, data: &[u8], out: &mut Vec<u8>) {
    let mut d = data.to_vec();
    DISPERSAL.apply(&mut d, 0);
    let mut bits = Vec::with_capacity(8 * d.len());
    sdrkit_util_bits::pack::unpack(&d, BitOrder::Msb, &mut bits);
    let mut mother = Vec::with_capacity(4 * (bits.len() + 6));
    let mut enc = Encoder::new(DAB_K7_R4);
    enc.encode(&bits, &mut mother);
    enc.terminate(&mut mother);
    let mask = keep_mask(p);
    if let Ok(pat) = Puncture::try_new(&mask) {
        puncture(&pat, 0, &mother, out);
    }
}
