//! GSM channel coding (3GPP TS 45.003) and burst formats: the FIRE code of the BCCH, the SCH code, interleaving, and the
//! bit layouts of the bursts.

use sdrkit_algo_fec::conv::codes::GSM_K5;
use sdrkit_algo_fec::conv::{Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::{check_finite, Quantizer};
use sdrkit_util_bits::crc::catalog::{CRC10_GSM, CRC40_GSM};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{push_u64, unpack};
use sdrkit_util_bits::BitOrder;

/// Training sequence codes 0 to 7 (45.002 Table 5.2.3a, TSC set 1), bits BN61 to BN86.
pub const TSC: [[u8; 26]; 8] = [
    [
        0, 0, 1, 0, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1,
    ],
    [
        0, 0, 1, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 0, 1, 1, 1,
    ],
    [
        0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0,
    ],
    [
        0, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 1, 0,
    ],
    [
        0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1,
    ],
    [
        0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0,
    ],
    [
        1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1,
    ],
    [
        1, 1, 1, 0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 0,
    ],
];

/// The extended training sequence of the synchronisation burst (45.002 Table 5.2.5-3), bits BN42 to BN105.
pub const SCH_TRAINING: [u8; 64] = [
    1, 0, 1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
    0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1,
];

/// The fixed bits of the dummy burst (45.002 5.2.6), BN3 to BN144.
pub const DUMMY: &str = "1111101101110110000010100100111000001001000100000001111100011100010111000101110001010111010010100011001100111001111010011111000100101111101010";

/// The 40 FIRE parity bits of 184 data bits, as transmitted (inverted).
pub fn fire_parity(d: &[u8]) -> u64 {
    CRC40_GSM.bits(d)
}

/// The 10 SCH parity bits of 25 data bits, as transmitted (inverted).
pub fn sch_parity(d: &[u8]) -> u64 {
    CRC10_GSM.bits(d) as u64
}

/// The bits of `octets`, least significant bit of each octet first.
pub fn octets_lsb_first(octets: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(octets.len() * 8);
    unpack(octets, BitOrder::Lsb, &mut v);
    v
}

/// The GSM convolutional code (45.003 4.1.3) of `u`, with the encoder starting in state 0.
fn conv_encode(u: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 * u.len());
    Encoder::new(GSM_K5).encode(u, &mut out);
    out
}

/// Encodes a 23-octet BCCH message into 456 coded bits (FIRE, tail, convolutional code).
pub fn encode_bcch(msg: &[u8; 23]) -> Vec<u8> {
    let mut u = octets_lsb_first(msg);
    let p = fire_parity(&u);
    push_u64(&mut u, p, 40);
    u.extend_from_slice(&[0, 0, 0, 0]);
    conv_encode(&u)
}

/// Spreads 456 coded bits over four bursts of 114 bits (45.003 4.1.4).
pub fn interleave(c: &[u8]) -> [[u8; 114]; 4] {
    let mut flat = [0u8; 456];
    if let (Ok(p), Some(c)) = (Permutation::gsm_xcch(), c.get(..456)) {
        let _ = p.apply(c, &mut flat);
    }
    let mut out = [[0u8; 114]; 4];
    for (o, f) in out.iter_mut().zip(flat.chunks(114)) {
        o.copy_from_slice(f);
    }
    out
}

/// The channel decoder of the BCCH and the SCH (45.003 4.1 and 4.7): deinterleaving, the soft Viterbi decoder and the parity
/// checks. It is made once per searcher and allocates nothing while it decodes.
#[derive(Clone, Debug)]
pub struct GsmDecoder {
    viterbi: Option<Viterbi>,
    perm: Option<Permutation>,
    flat: [f32; 456],
    soft: [f32; 456],
    llr: [i8; 456],
    bits: [u8; 224],
}

impl Default for GsmDecoder {
    fn default() -> Self {
        GsmDecoder::new()
    }
}

impl GsmDecoder {
    /// A decoder for the 228 steps of a BCCH block.
    pub fn new() -> GsmDecoder {
        GsmDecoder {
            viterbi: Viterbi::new(GSM_K5, 228).ok(),
            perm: Permutation::gsm_xcch().ok(),
            flat: [0.0; 456],
            soft: [0.0; 456],
            llr: [0; 456],
            bits: [0; 224],
        }
    }

    /// Runs the Viterbi decoder over `soft` (two values per step, positive means 0, the 4 tail steps included). The path must
    /// end in state 0. Returns the information bits, or none for a soft value that is not finite.
    fn decode_conv(&mut self, soft: &[f32]) -> Option<&[u8]> {
        check_finite(soft).ok()?;
        let n = soft.len();
        let info = (n / 2).checked_sub(4)?;
        if n > 456 || !n.is_multiple_of(2) || info == 0 {
            return None;
        }
        let q = Quantizer::default();
        q.quantize_auto(soft, &mut self.llr[..n]).ok()?;
        self.viterbi
            .as_mut()?
            .decode(&self.llr[..n], Ends::Zero, &mut self.bits[..info])
            .ok()?;
        Some(&self.bits[..info])
    }

    /// Decodes the 456 soft values of four bursts (114 data values each). Returns the 23 octets when the FIRE check passes.
    pub fn decode_bcch(&mut self, bursts: &[[f32; 114]; 4]) -> Option<[u8; 23]> {
        for (f, b) in self.flat.chunks_mut(114).zip(bursts) {
            f.copy_from_slice(b);
        }
        self.perm
            .as_ref()?
            .invert(&self.flat, &mut self.soft)
            .ok()?;
        let soft = self.soft;
        let u = self.decode_conv(&soft)?;
        if !CRC40_GSM.verify_bits(u) {
            return None;
        }
        let mut m = [0u8; 23];
        for (o, c) in m.iter_mut().zip(u[..184].chunks(8)) {
            *o = c.iter().enumerate().fold(0u8, |a, (i, &b)| a | (b << i));
        }
        Some(m)
    }

    /// Decodes the 78 soft values of an SCH. Returns the SCH when the parity checks.
    pub fn decode_sch(&mut self, soft: &[f32]) -> Option<Sch> {
        if soft.len() != 78 {
            return None;
        }
        let u = self.decode_conv(soft)?;
        CRC10_GSM.verify_bits(u).then(|| Sch::from_bits(&u[..25]))
    }
}

/// The SCH contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sch {
    /// BSIC (NCC x 8 + BCC).
    pub bsic: u8,
    /// T1.
    pub t1: u32,
    /// T2.
    pub t2: u32,
    /// T3'.
    pub t3p: u32,
}

impl Sch {
    /// From a frame number.
    pub fn from_fn(bsic: u8, fnum: u32) -> Sch {
        let t1 = fnum / (26 * 51);
        let t2 = fnum % 26;
        let t3 = fnum % 51;
        Sch {
            bsic,
            t1,
            t2,
            t3p: (t3 - 1) / 10,
        }
    }
    /// The frame number.
    pub fn fnum(&self) -> u32 {
        let t3 = 10 * self.t3p + 1;
        let d = (t3 as i64 - self.t2 as i64).rem_euclid(26) as u32;
        51 * d + t3 + 51 * 26 * self.t1
    }
    /// The 25 information bits d(0) to d(24) (44.018 9.1.30, least significant bit of each octet first).
    pub fn bits(&self) -> [u8; 25] {
        let mut d = [0u8; 25];
        let (t1, t2, t3p, b) = (self.t1, self.t2, self.t3p, self.bsic as u32);
        d[0] = ((t1 >> 9) & 1) as u8;
        d[1] = ((t1 >> 10) & 1) as u8;
        for i in 0..6 {
            d[2 + i] = ((b >> i) & 1) as u8;
        }
        for i in 0..8 {
            d[8 + i] = ((t1 >> (1 + i)) & 1) as u8;
        }
        d[16] = ((t3p >> 1) & 1) as u8;
        d[17] = ((t3p >> 2) & 1) as u8;
        for i in 0..5 {
            d[18 + i] = ((t2 >> i) & 1) as u8;
        }
        d[23] = (t1 & 1) as u8;
        d[24] = (t3p & 1) as u8;
        d
    }
    /// From the 25 information bits.
    pub fn from_bits(d: &[u8]) -> Sch {
        let g = |i: usize| d[i] as u32;
        let mut bsic = 0u32;
        for i in 0..6 {
            bsic |= g(2 + i) << i;
        }
        let mut t1 = (g(1) << 10) | (g(0) << 9) | g(23);
        for i in 0..8 {
            t1 |= g(8 + i) << (1 + i);
        }
        let mut t2 = 0;
        for i in 0..5 {
            t2 |= g(18 + i) << i;
        }
        Sch {
            bsic: bsic as u8,
            t1,
            t2,
            t3p: (g(17) << 2) | (g(16) << 1) | g(24),
        }
    }
}

/// Encodes the SCH into its 78 coded bits.
pub fn encode_sch(s: &Sch) -> Vec<u8> {
    let d = s.bits();
    let mut u: Vec<u8> = d.to_vec();
    let p = sch_parity(&d);
    push_u64(&mut u, p, 10);
    u.extend_from_slice(&[0, 0, 0, 0]);
    conv_encode(&u)
}

/// The 148 bits of a normal burst carrying two halves of 57 data bits, training sequence `tsc` and stealing flags.
pub fn normal_burst(data: &[u8; 114], tsc: usize, steal: u8) -> [u8; 148] {
    let mut b = [0u8; 148];
    b[3..60].copy_from_slice(&data[..57]);
    b[60] = steal;
    b[61..87].copy_from_slice(&TSC[tsc & 7]);
    b[87] = steal;
    b[88..145].copy_from_slice(&data[57..]);
    b
}

/// The synchronisation burst.
pub fn sch_burst(coded: &[u8]) -> [u8; 148] {
    let mut b = [0u8; 148];
    b[3..42].copy_from_slice(&coded[..39]);
    b[42..106].copy_from_slice(&SCH_TRAINING);
    b[106..145].copy_from_slice(&coded[39..78]);
    b
}

/// The frequency correction burst (all zeros).
pub fn fcch_burst() -> [u8; 148] {
    [0u8; 148]
}

/// The dummy burst.
pub fn dummy_burst() -> [u8; 148] {
    let mut b = [0u8; 148];
    for (i, c) in DUMMY.bytes().enumerate() {
        b[3 + i] = c - b'0';
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(DUMMY.len(), 142);
        // FIRE: an encoded block decodes; the convolutional code can correct a damaged one, but a wrong parity fails.
        let mut msg = [0x2Bu8; 23];
        msg[0] = 0x55;
        msg[1] = 0x06;
        msg[2] = 0x19;
        let c = encode_bcch(&msg);
        assert_eq!(c.len(), 456);
        let bursts = interleave(&c);
        let soft: [[f32; 114]; 4] = bursts.map(|b| b.map(|x| if x == 0 { 1.0 } else { -1.0 }));
        let mut dec = GsmDecoder::new();
        assert_eq!(dec.decode_bcch(&soft), Some(msg));
        // 40 consecutive coded bits wrong.
        let mut bad = soft;
        let p = Permutation::gsm_xcch().unwrap();
        for k in 0..40 {
            let d = p.dest(k).unwrap();
            bad[d / 114][d % 114] = -bad[d / 114][d % 114];
        }
        assert_ne!(dec.decode_bcch(&bad), Some(msg));
        // SCH.
        for (bsic, f) in [(10u8, 100u32), (10, 2_715_647), (63, 51)] {
            let s = Sch::from_fn(bsic, f - (f % 51) + 1);
            let c = encode_sch(&s);
            assert_eq!(c.len(), 78);
            let soft: Vec<f32> = c.iter().map(|&x| if x == 0 { 1.0 } else { -1.0 }).collect();
            assert_eq!(dec.decode_sch(&soft), Some(s));
        }
        // FN from T1, T2, T3.
        for f in [0u32, 50, 51, 1325, 2_715_647] {
            let t3 = f % 51;
            let t2 = f % 26;
            let t1 = f / 1326;
            let d = (t3 as i64 - t2 as i64).rem_euclid(26) as u32;
            assert_eq!(51 * d + t3 + 1326 * t1, f);
        }
        let s = Sch::from_fn(10, 2_715_647 - (2_715_647 % 51) + 41);
        assert_eq!(s.fnum(), 2_715_647 - (2_715_647 % 51) + 41);
    }
}
