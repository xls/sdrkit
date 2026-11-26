//! The NR SS/PBCH block (3GPP TS 38.211 7.4, TS 38.212 7.1, TS 38.213 4.1): PSS, SSS, PBCH DMRS, the resource element maps, the
//! numerology at 256 x SCS, the PBCH payload (interleaving, scrambling, CRC, polar code) and the MIB.

use sdrkit_algo_fec::polar::{Polar, PolarParams};
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC24_NR_C;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::pack::push_u64;
use sdrkit_util_bits::seq::gold_3gpp;
use sdrkit_util_bits::seq::{NR_PSS, NR_SSS_X0, NR_SSS_X1};

/// One period of the m-sequence `reg` (127 bits).
fn mseq(reg: Lfsr) -> [u8; 127] {
    let mut r = reg;
    let mut x = [0u8; 127];
    r.fill_bits(&mut x);
    x
}

/// The PSS of N_ID2 (127 values, +-1). Standard: 3GPP TS 38.211 clause 7.4.2.2.1.
pub fn pss(nid2: u16) -> [f32; 127] {
    let x = mseq(NR_PSS);
    let mut d = [0f32; 127];
    for (n, v) in d.iter_mut().enumerate() {
        let m = (n + 43 * nid2 as usize) % 127;
        *v = 1.0 - 2.0 * x[m] as f32;
    }
    d
}

/// The SSS of (N_ID1, N_ID2) (127 values, +-1). Standard: 3GPP TS 38.211 clause 7.4.2.3.1.
pub fn sss(nid1: u16, nid2: u16) -> [f32; 127] {
    let x0 = mseq(NR_SSS_X0);
    let x1 = mseq(NR_SSS_X1);
    let m0 = 15 * (nid1 as usize / 112) + 5 * nid2 as usize;
    let m1 = nid1 as usize % 112;
    let mut d = [0f32; 127];
    for (n, v) in d.iter_mut().enumerate() {
        *v = (1.0 - 2.0 * x0[(n + m0) % 127] as f32) * (1.0 - 2.0 * x1[(n + m1) % 127] as f32);
    }
    d
}

/// The PBCH DMRS (144 values) of cell `pci` for i_SSB_bar `ibar`.
pub fn dmrs(pci: u16, ibar: u8) -> [Complex32; 144] {
    let c_init = (1u32 << 11) * (ibar as u32 + 1) * (pci as u32 / 4 + 1)
        + (1u32 << 6) * (ibar as u32 + 1)
        + (pci as u32 % 4);
    let mut c = [0u8; 288];
    gold_3gpp(c_init, &mut c);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let mut r = [Complex32::new(0.0, 0.0); 144];
    for (m, v) in r.iter_mut().enumerate() {
        *v = Complex32::new(
            s * (1.0 - 2.0 * c[2 * m] as f32),
            s * (1.0 - 2.0 * c[2 * m + 1] as f32),
        );
    }
    r
}

/// The DMRS resource elements (k, symbol 1..3) in mapping order.
pub fn dmrs_res(pci: u16) -> Vec<(usize, usize)> {
    let nu = pci as usize % 4;
    let mut v = Vec::with_capacity(144);
    for l in 1..4 {
        for k in (nu..240).step_by(4) {
            if l == 2 && (48..192).contains(&k) {
                continue;
            }
            v.push((k, l));
        }
    }
    v
}

/// The PBCH data resource elements (k, symbol) in mapping order.
pub fn pbch_res(pci: u16) -> Vec<(usize, usize)> {
    let nu = pci as usize % 4;
    let mut v = Vec::with_capacity(432);
    for l in 1..4 {
        for k in 0..240 {
            if l == 2 && (48..192).contains(&k) {
                continue;
            }
            if k % 4 == nu {
                continue;
            }
            v.push((k, l));
        }
    }
    v
}

/// The NR numerology at 256 x SCS: (start of symbol l within the half frame, its cyclic prefix), l = 0 .. 14 x 5 x SCS/15.
pub fn symbol_start(scs_khz: u32, l: usize) -> (usize, usize) {
    let mu = if scs_khz == 30 { 1 } else { 0 };
    let per_half_ms = 7 << mu;
    let extra = 2usize << mu;
    let half_ms_len = 256 * per_half_ms + 18 * per_half_ms + extra;
    let block = l / per_half_ms;
    let i = l % per_half_ms;
    let start = block * half_ms_len
        + if i == 0 {
            0
        } else {
            256 + 18 + extra + (i - 1) * 274
        };
    (start, if i == 0 { 18 + extra } else { 18 })
}

/// The useful start of symbol `l`.
pub fn useful_start(scs_khz: u32, l: usize) -> usize {
    let (s, cp) = symbol_start(scs_khz, l);
    s + cp
}

/// Samples per half frame at 256 x SCS.
pub fn half_frame(scs_khz: u32) -> usize {
    1280 * scs_khz as usize / 15 * 15
}

/// The time pattern of the SS/PBCH blocks of a cell in frequency range 1 (TS 38.213 clause 4.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SsbPattern {
    /// Case A (15 kHz) and Case C (30 kHz): first symbols {2, 8} + 14 n.
    #[default]
    AC,
    /// Case B (30 kHz): first symbols {4, 8, 16, 20} + 28 n.
    B,
}

impl SsbPattern {
    /// The pattern of a block of spacing `scs_khz` in NR band `band`: Case B for 30 kHz in n5, n24 and n66 (TS 38.104
    /// table 5.4.3.3-1), else Case A or C. Without a band, Case A or C.
    pub fn of(scs_khz: u32, band: Option<u16>) -> SsbPattern {
        if scs_khz == 30 && matches!(band, Some(5 | 24 | 66)) {
            SsbPattern::B
        } else {
            SsbPattern::AC
        }
    }
    /// The first symbol of SSB index `i` in its half frame.
    pub fn first_symbol(self, i: usize) -> usize {
        match self {
            SsbPattern::AC => [2usize, 8][i % 2] + 14 * (i / 2),
            SsbPattern::B => [4usize, 8, 16, 20][i % 4] + 28 * (i / 4),
        }
    }
}

/// The MIB with the timing bits of the PBCH payload.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mib {
    /// The SFN (10 bits).
    pub sfn: u16,
    /// Half frame bit.
    pub hrf: u8,
    /// subCarrierSpacingCommon is scs30or120.
    pub scs_common_30: bool,
    /// k_SSB (0..31).
    pub k_ssb: u8,
    /// dmrs-TypeA-Position is pos3.
    pub dmrs_pos3: bool,
    /// controlResourceSetZero.
    pub coreset0: u8,
    /// searchSpaceZero.
    pub search_space0: u8,
    /// cellBarred is barred.
    pub barred: bool,
    /// intraFreqReselection is allowed.
    pub intra_freq_allowed: bool,
}

impl Mib {
    /// The 32 payload bits ā (24 MIB bits with the choice bit, then 8 timing bits).
    pub fn payload(&self) -> [u8; 32] {
        let mut a = [0u8; 32];
        let mut p = 0;
        let mut put = |v: u32, n: usize| {
            for i in (0..n).rev() {
                a[p] = ((v >> i) & 1) as u8;
                p += 1;
            }
        };
        put(0, 1);
        put((self.sfn >> 4) as u32, 6);
        put(self.scs_common_30 as u32, 1);
        put((self.k_ssb & 15) as u32, 4);
        put(self.dmrs_pos3 as u32, 1);
        put(self.coreset0 as u32, 4);
        put(self.search_space0 as u32, 4);
        put(!self.barred as u32, 1);
        put(!self.intra_freq_allowed as u32, 1);
        put(0, 1);
        put((self.sfn & 15) as u32, 4);
        put(self.hrf as u32, 1);
        put(((self.k_ssb >> 4) & 1) as u32, 1);
        put(0, 2);
        a
    }
    /// From the 32 payload bits.
    pub fn from_payload(a: &[u8; 32]) -> Option<Mib> {
        let get = |s: usize, n: usize| a[s..s + n].iter().fold(0u32, |v, &b| (v << 1) | b as u32);
        if a[0] != 0 {
            return None;
        }
        Some(Mib {
            sfn: ((get(1, 6) << 4) | get(24, 4)) as u16,
            scs_common_30: get(7, 1) == 1,
            k_ssb: (get(8, 4) | (get(29, 1) << 4)) as u8,
            dmrs_pos3: get(12, 1) == 1,
            coreset0: get(13, 4) as u8,
            search_space0: get(17, 4) as u8,
            barred: get(21, 1) == 0,
            intra_freq_allowed: get(22, 1) == 0,
            hrf: get(28, 1) as u8,
        })
    }
}

/// Whether payload bit i (0..31) is an SFN bit, the half frame bit, or one of the last three.
fn kind(i: usize) -> u8 {
    match i {
        1..=6 | 24..=27 => 0,
        28 => 1,
        29..=31 => 2,
        _ => 3,
    }
}

/// The interleaver of 38.212 7.1.1: positions of ā_i in a'.
pub fn interleave_positions() -> [usize; 32] {
    match Permutation::nr_pbch(kind) {
        Ok(p) => std::array::from_fn(|i| p.dest(i).unwrap_or(i)),
        Err(_) => std::array::from_fn(|i| i),
    }
}

/// The payload scrambling (38.212 7.1.2) for Lmax 4 or 8: s_i for i = 0..31 of a'.
fn payload_scrambling(pci: u16, sfn: u16, pos: &[usize; 32]) -> [u8; 32] {
    let m = 29;
    let v = ((sfn >> 1) & 3) as usize;
    let mut c = [0u8; 4 * 29 + 32];
    gold_3gpp(pci as u32, &mut c);
    // Unscrambled: the 2nd and 3rd LSB of the SFN (ā_26, ā_25) and the half frame bit (ā_28).
    let mut skip = [false; 32];
    for i in [25usize, 26, 28] {
        skip[pos[i]] = true;
    }
    let mut s = [0u8; 32];
    let mut j = 0;
    for (i, si) in s.iter_mut().enumerate() {
        if !skip[i] {
            *si = c[j + v * m];
            j += 1;
        }
    }
    s
}

/// The 864 coded and scrambled PBCH bits of `mib` for cell `pci`, SSB index `issb` and Lmax.
pub fn encode_pbch(mib: &Mib, pci: u16, issb: u8, lmax: u8) -> Vec<u8> {
    let a = mib.payload();
    let pos = interleave_positions();
    let mut ap = [0u8; 32];
    for i in 0..32 {
        ap[pos[i]] = a[i];
    }
    let s = payload_scrambling(pci, mib.sfn, &pos);
    let mut bits: Vec<u8> = ap.iter().zip(&s).map(|(x, y)| x ^ y).collect();
    let crc = CRC24_NR_C.bits(&bits);
    push_u64(&mut bits, crc as u64, 24);
    let mut coded = vec![0u8; 864];
    if let Ok(mut code) = Polar::new(PolarParams::PBCH, 8) {
        // The PBCH parameters and a 56-bit input always fit the code.
        let _ = code.encode(&bits, &mut coded);
    }
    let v = if lmax == 4 { issb & 3 } else { issb & 7 } as usize;
    let mut c = vec![0u8; 864 * (v + 1)];
    gold_3gpp(pci as u32, &mut c);
    coded
        .iter()
        .zip(&c[864 * v..])
        .map(|(b, s)| b ^ s)
        .collect()
}

/// The PBCH decoder of one searcher: the polar list decoder and every buffer, made once so that a decode allocates nothing.
#[derive(Clone, Debug)]
pub struct PbchDecoder {
    polar: Option<Polar>,
    pos: [usize; 32],
    scr: Vec<u8>,
    soft: Vec<f32>,
    bits: [u8; 56],
}

impl Default for PbchDecoder {
    fn default() -> Self {
        PbchDecoder::new()
    }
}

impl PbchDecoder {
    /// A decoder with a list of 8 paths.
    pub fn new() -> PbchDecoder {
        PbchDecoder {
            polar: Polar::new(PolarParams::PBCH, 8).ok(),
            pos: interleave_positions(),
            scr: vec![0; 864 * 8],
            soft: vec![0.0; 864],
            bits: [0; 56],
        }
    }

    /// Decodes 864 soft PBCH values (positive means 0) for v (the SSB index bits of the scrambling, 0 to 7). Returns the MIB
    /// when the CRC checks. Returns `None` for a wrong length, a v above 7 or a soft value that is not finite.
    pub fn decode(&mut self, soft: &[f32], pci: u16, v: usize) -> Option<Mib> {
        if soft.len() != 864 || v > 7 {
            return None;
        }
        let polar = self.polar.as_mut()?;
        let scr = &mut self.scr[..864 * (v + 1)];
        gold_3gpp(pci as u32, scr);
        for ((d, x), &s) in self.soft.iter_mut().zip(soft).zip(&scr[864 * v..]) {
            *d = if s == 1 { -x } else { *x };
        }
        polar
            .decode(&self.soft, |b| CRC24_NR_C.verify_bits(b), &mut self.bits)
            .ok()?;
        let pos = &self.pos;
        let bits = &self.bits;
        // The unscrambled SFN bits give the payload scrambling.
        let sfn_23 = ((bits[pos[25]] as u16) << 2) | ((bits[pos[26]] as u16) << 1);
        let s = payload_scrambling(pci, sfn_23, pos);
        let mut a = [0u8; 32];
        for i in 0..32 {
            a[i] = bits[pos[i]] ^ s[pos[i]];
        }
        Mib::from_payload(&a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nr_sequences() {
        let p0 = pss(0);
        assert_eq!(&p0[..7], &[1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0]);
        // All 336 SSS of N_ID2 0 are distinguishable (low cross-correlation).
        let a = sss(74, 0);
        let mut worst = 0f32;
        for n in 0..336 {
            if n == 74 {
                continue;
            }
            let b = sss(n, 0);
            let c: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>().abs();
            worst = worst.max(c);
        }
        assert!(worst < 50.0, "{worst}");
        assert_eq!(dmrs_res(222).len(), 144);
        assert_eq!(pbch_res(222).len(), 432);
        assert_ne!(dmrs(222, 0)[..4], dmrs(222, 1)[..4]);
        // 15 kHz: 0.5 ms is 1920 samples at 3.84 Msps.
        assert_eq!(symbol_start(15, 7).0, 1920);
        assert_eq!(symbol_start(30, 14).0, 3840);
        assert_eq!(half_frame(15), 19200);
        assert_eq!(half_frame(30), 38400);
    }

    #[test]
    fn pbch_chain() {
        let m = Mib {
            sfn: 517,
            hrf: 1,
            scs_common_30: true,
            k_ssb: 19,
            dmrs_pos3: false,
            coreset0: 12,
            search_space0: 2,
            barred: false,
            intra_freq_allowed: true,
        };
        assert_eq!(Mib::from_payload(&m.payload()), Some(m.clone()));
        let pos = interleave_positions();
        let mut sorted = pos.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..32).collect::<Vec<_>>());
        let mut code = PbchDecoder::new();
        for (issb, lmax) in [(0u8, 8u8), (5, 8), (6, 4)] {
            let bits = encode_pbch(&m, 501, issb, lmax);
            let soft: Vec<f32> = bits
                .iter()
                .map(|&b| if b == 0 { 2.0 } else { -2.0 })
                .collect();
            let v = if lmax == 4 { issb & 3 } else { issb } as usize;
            assert_eq!(code.decode(&soft, 501, v), Some(m.clone()));
        }
    }
}
