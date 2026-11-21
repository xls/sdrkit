//! The LTE downlink physical layer (3GPP TS 36.211, 36.212, 36.213) as far as broadcast reception needs it: numerology, PSS, SSS,
//! CRS, the resource element maps of PBCH, PCFICH, PHICH, PDCCH and PDSCH, transmit diversity, DCI formats 1A and 1C, the
//! distributed VRB mapping, RIV and the transport block size tables.

use sdrkit_algo_fec::conv::codes::LTE_K7_R3;
use sdrkit_algo_fec::conv::{encode_tail_biting, Ends, Viterbi};
use sdrkit_algo_fec::llr::{check_finite, Quantizer};
use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
use sdrkit_algo_fec::turbo::{size_at_least, Turbo};
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::{CRC16_XMODEM, CRC24_LTE_A};
use sdrkit_util_bits::interleave::{Permutation, LTE_SUBBLOCK_CONV};
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::pack::push_u64;
use sdrkit_util_bits::seq::gold_3gpp;
use sdrkit_util_bits::seq::{ZadoffChu, LTE_SSS_C, LTE_SSS_S, LTE_SSS_Z};

/// The FFT size of a bandwidth in RBs.
pub fn fft_size(n_rb: usize) -> usize {
    match n_rb {
        0..=6 => 128,
        7..=15 => 256,
        16..=25 => 512,
        26..=50 => 1024,
        _ => 2048,
    }
}

/// The bandwidth in RBs of MIB dl-Bandwidth index 0..5.
pub const N_RB: [usize; 6] = [6, 15, 25, 50, 75, 100];

/// OFDM symbol timing of a subframe at FFT size `n`: (start within the subframe, cyclic prefix) of symbol `l` (0..2 x symbols per
/// slot).
pub fn symbol_start(n: usize, ext: bool, l: usize) -> (usize, usize) {
    let per_slot = if ext { 6 } else { 7 };
    let slot = l / per_slot;
    let i = l % per_slot;
    let slot_len = 15 * n / 2;
    if ext {
        let cp = n / 4;
        (slot * slot_len + i * (n + cp), cp)
    } else {
        let cp0 = 160 * n / 2048;
        let cp = 144 * n / 2048;
        let s = if i == 0 {
            0
        } else {
            n + cp0 + (i - 1) * (n + cp)
        };
        (slot * slot_len + s, if i == 0 { cp0 } else { cp })
    }
}

/// Symbols per slot.
pub fn symbols_per_slot(ext: bool) -> usize {
    if ext {
        6
    } else {
        7
    }
}

/// The FFT bin of subcarrier k (0..12 N_RB - 1).
pub fn bin(n_rb: usize, n: usize, k: usize) -> usize {
    if k < 6 * n_rb {
        n - 6 * n_rb + k
    } else {
        k - 6 * n_rb + 1
    }
}

/// The PSS of N_ID2 (62 values).
pub fn pss(nid2: u16) -> [Complex32; 62] {
    let u = [25, 29, 34][nid2 as usize % 3];
    let z = ZadoffChu::new(u, 63).expect("the PSS roots are prime to 63");
    let mut d = [Complex32::new(0.0, 0.0); 62];
    for (n, v) in d.iter_mut().enumerate() {
        // The value at n = 31 (the DC carrier) is left out.
        let [re, im] = z.value(if n < 31 { n } else { n + 1 } as u64);
        *v = Complex32::new(re as f32, im as f32);
    }
    d
}

/// One period of the m-sequence `reg` as +-1 values (0 becomes +1).
fn mseq(reg: Lfsr) -> [i8; 31] {
    let mut r = reg;
    let mut x = [0u8; 31];
    r.fill_bits(&mut x);
    x.map(|b| 1 - 2 * b as i8)
}

/// The SSS of (N_ID1, N_ID2) in subframe 0 (`sf5` false) or 5 (62 values, +-1).
pub fn sss(nid1: u16, nid2: u16, sf5: bool) -> [f32; 62] {
    let s = mseq(LTE_SSS_S);
    let c = mseq(LTE_SSS_C);
    let z = mseq(LTE_SSS_Z);
    let n1 = nid1 as usize;
    let qp = n1 / 30;
    let q = (n1 + qp * (qp + 1) / 2) / 30;
    let mp = n1 + q * (q + 1) / 2;
    let m0 = mp % 31;
    let m1 = (m0 + mp / 31 + 1) % 31;
    let n2 = nid2 as usize;
    let mut d = [0f32; 62];
    for n in 0..31 {
        let s0 = s[(n + m0) % 31] as f32;
        let s1 = s[(n + m1) % 31] as f32;
        let c0 = c[(n + n2) % 31] as f32;
        let c1 = c[(n + n2 + 3) % 31] as f32;
        let z0 = z[(n + m0 % 8) % 31] as f32;
        let z1 = z[(n + m1 % 8) % 31] as f32;
        if !sf5 {
            d[2 * n] = s0 * c0;
            d[2 * n + 1] = s1 * c1 * z0;
        } else {
            d[2 * n] = s1 * c0;
            d[2 * n + 1] = s0 * c1 * z1;
        }
    }
    d
}

/// The CRS of slot `ns`, symbol `l` (within the slot): 2 x 110 values (index m', 0..219).
pub fn crs_seq(pci: u16, ns: usize, l: usize, ext: bool) -> Vec<Complex32> {
    let mut v = [Complex32::new(0.0, 0.0); 220];
    crs_seq_into(pci, ns, l, ext, &mut v);
    v.to_vec()
}

/// As [`crs_seq`], into `out` without allocating.
pub fn crs_seq_into(pci: u16, ns: usize, l: usize, ext: bool, out: &mut [Complex32; 220]) {
    let n = pci as u32;
    let ncp = if ext { 0 } else { 1 };
    let c_init = (1u32 << 10) * (7 * (ns as u32 + 1) + l as u32 + 1) * (2 * n + 1) + 2 * n + ncp;
    let mut c = [0u8; 440];
    gold_3gpp(c_init, &mut c);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    for (m, o) in out.iter_mut().enumerate() {
        *o = Complex32::new(
            s * (1.0 - 2.0 * c[2 * m] as f32),
            s * (1.0 - 2.0 * c[2 * m + 1] as f32),
        );
    }
}

/// The CRS of port `p` in slot `ns`, symbol-in-slot `l`: (subcarrier, value) pairs, or none when the port has no CRS there.
pub fn crs_res(
    pci: u16,
    n_rb: usize,
    p: usize,
    ns: usize,
    l: usize,
    ext: bool,
) -> Vec<(usize, Complex32)> {
    let mut v = Vec::new();
    crs_res_into(pci, n_rb, p, ns, l, ext, &mut v);
    v
}

/// As [`crs_res`], into `out` (cleared first) without allocating once `out` has room for `2 n_rb` pairs.
pub fn crs_res_into(
    pci: u16,
    n_rb: usize,
    p: usize,
    ns: usize,
    l: usize,
    ext: bool,
    out: &mut Vec<(usize, Complex32)>,
) {
    out.clear();
    let nsym = symbols_per_slot(ext);
    let v = match p {
        0 if l == 0 => 0,
        0 if l == nsym - 3 => 3,
        1 if l == 0 => 3,
        1 if l == nsym - 3 => 0,
        2 if l == 1 => 3 * (ns % 2),
        3 if l == 1 => 3 + 3 * (ns % 2),
        _ => return,
    };
    let mut seq = [Complex32::new(0.0, 0.0); 220];
    crs_seq_into(pci, ns, l, ext, &mut seq);
    let vs = pci as usize % 6;
    let n_rb = n_rb.min(110);
    out.extend((0..2 * n_rb).map(|m| (6 * m + (v + vs) % 6, seq[m + 110 - n_rb])));
}

/// Whether subcarrier k of symbol `l` (in the subframe) carries a CRS of any of `ports` ports.
pub fn is_crs(pci: u16, ports: usize, l: usize, k: usize, ext: bool) -> bool {
    let nsym = symbols_per_slot(ext);
    let ls = l % nsym;
    let vs = pci as usize % 6;
    if ports == 1 {
        return match ls {
            0 => k % 6 == vs,
            x if x == nsym - 3 => k % 6 == (3 + vs) % 6,
            _ => false,
        };
    }
    let on = ls == 0 || ls == nsym - 3 || (ports > 2 && ls == 1);
    on && k % 3 == vs % 3
}

/// The PBCH resource elements of subframe 0 (slot 1, symbols 0..3 of the slot) in mapping order: (k, subframe symbol).
pub fn pbch_res(pci: u16, n_rb: usize, ext: bool) -> Vec<(usize, usize)> {
    let nsym = symbols_per_slot(ext);
    let k0 = 6 * n_rb - 36;
    let mut v = Vec::with_capacity(240);
    for li in 0..4 {
        let l = nsym + li;
        for kp in 0..72 {
            let k = k0 + kp;
            // Reserved for the CRS of four ports.
            if is_crs(pci, 4, l, k, ext) {
                continue;
            }
            v.push((k, l));
        }
    }
    v
}

/// Layer mapping and precoding for transmit diversity: `ports` streams of symbols from `d`.
pub fn txd_encode(d: &[Complex32], ports: usize) -> Vec<Vec<Complex32>> {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    match ports {
        1 => vec![d.to_vec()],
        2 => {
            let mut y: Vec<Vec<Complex32>> = (0..2).map(|_| Vec::with_capacity(d.len())).collect();
            for pair in d.chunks(2) {
                let (x0, x1) = (pair[0], pair.get(1).copied().unwrap_or_default());
                y[0].push(x0 * s);
                y[1].push(-x1.conj() * s);
                y[0].push(x1 * s);
                y[1].push(x0.conj() * s);
            }
            y
        }
        _ => {
            let z = Complex32::new(0.0, 0.0);
            let mut y: Vec<Vec<Complex32>> = (0..4).map(|_| Vec::with_capacity(d.len())).collect();
            for q in d.chunks(4) {
                let g = |i: usize| q.get(i).copied().unwrap_or_default();
                let (x0, x1, x2, x3) = (g(0), g(1), g(2), g(3));
                let rows = [
                    [x0 * s, x1 * s, z, z],
                    [z, z, x2 * s, x3 * s],
                    [-x1.conj() * s, x0.conj() * s, z, z],
                    [z, z, -x3.conj() * s, x2.conj() * s],
                ];
                for (p, r) in rows.iter().enumerate() {
                    y[p].extend_from_slice(r);
                }
            }
            y
        }
    }
}

/// Undoes transmit diversity: the symbol estimates (scaled by the channel power) and their channel power, from received `r` and
/// channels `h[port][i]` per resource element.
pub fn txd_decode(
    r: &[Complex32],
    h: &[Vec<Complex32>],
    ports: usize,
    out: &mut Vec<(Complex32, f32)>,
) {
    out.clear();
    let s = std::f32::consts::FRAC_1_SQRT_2;
    match ports {
        1 => {
            for (i, &y) in r.iter().enumerate() {
                let hh = h[0][i];
                out.push((y * hh.conj(), hh.norm_sqr()));
            }
        }
        2 => {
            let mut i = 0;
            while i + 1 < r.len() {
                let h0 = (h[0][i] + h[0][i + 1]) * 0.5;
                let h1 = (h[1][i] + h[1][i + 1]) * 0.5;
                let g = (h0.norm_sqr() + h1.norm_sqr()) * s;
                let x0 = (h0.conj() * r[i] + h1 * r[i + 1].conj()) * s;
                let x1 = (h0.conj() * r[i + 1] - h1 * r[i].conj()) * s;
                out.push((x0, g * s));
                out.push((x1, g * s));
                i += 2;
            }
        }
        _ => {
            let mut i = 0;
            while i + 3 < r.len() {
                for half in 0..2 {
                    let a = i + 2 * half;
                    let (pa, pb) = if half == 0 { (0, 2) } else { (1, 3) };
                    let h0 = (h[pa][a] + h[pa][a + 1]) * 0.5;
                    let h1 = (h[pb][a] + h[pb][a + 1]) * 0.5;
                    let g = (h0.norm_sqr() + h1.norm_sqr()) * s;
                    let x0 = (h0.conj() * r[a] + h1 * r[a + 1].conj()) * s;
                    let x1 = (h0.conj() * r[a + 1] - h1 * r[a].conj()) * s;
                    out.push((x0, g * s));
                    out.push((x1, g * s));
                }
                i += 4;
            }
        }
    }
}

/// QPSK.
pub fn qpsk(b0: u8, b1: u8) -> Complex32 {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(s * (1.0 - 2.0 * b0 as f32), s * (1.0 - 2.0 * b1 as f32))
}

/// Soft bits of QPSK estimates `(x, g)` with noise variance `noise` (positive: 0).
pub fn soft_qpsk(est: &[(Complex32, f32)], noise: f32, out: &mut Vec<f32>) {
    out.clear();
    let k = 2.0 * std::f32::consts::SQRT_2 / noise.max(1e-12);
    for (x, _) in est {
        out.push(k * x.re);
        out.push(k * x.im);
    }
}

/// The CFI code words (36.212 Table 5.3.4-1).
pub fn cfi_codeword(cfi: u8) -> [u8; 32] {
    let pat: [u8; 3] = match cfi {
        1 => [0, 1, 1],
        2 => [1, 0, 1],
        3 => [1, 1, 0],
        _ => [0, 0, 0],
    };
    let mut w = [0u8; 32];
    for (i, b) in w.iter_mut().enumerate() {
        *b = pat[i % 3];
    }
    w
}

/// A resource element group: its symbol and its four (or two of six) data subcarriers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reg {
    /// The lowest subcarrier of the group (its representative).
    pub k0: usize,
    /// The symbol.
    pub l: usize,
    /// The four data subcarriers.
    pub ks: [usize; 4],
}

/// The REGs of symbol `l` (0..3 of the control region) in increasing frequency.
pub fn regs_of_symbol(pci: u16, n_rb: usize, ports: usize, l: usize, ext: bool) -> Vec<Reg> {
    // One port is treated as two for the REG structure.
    let ports_m = ports.max(2);
    let six = l == 0 || (l == 1 && ports_m == 4) || (l == 3 && ext);
    let mut v = Vec::new();
    for rb in 0..n_rb {
        let base = 12 * rb;
        if six {
            for g in 0..2 {
                let k0 = base + 6 * g;
                let mut ks = [0usize; 4];
                let mut j = 0;
                for k in k0..k0 + 6 {
                    if !is_crs(pci, ports_m, l, k, ext) {
                        if j < 4 {
                            ks[j] = k;
                        }
                        j += 1;
                    }
                }
                v.push(Reg { k0, l, ks });
            }
        } else {
            for g in 0..3 {
                let k0 = base + 4 * g;
                v.push(Reg {
                    k0,
                    l,
                    ks: [k0, k0 + 1, k0 + 2, k0 + 3],
                });
            }
        }
    }
    v
}

/// The control region of a subframe: the PCFICH REGs, the PHICH REGs and the PDCCH REGs in mapping order.
#[derive(Clone, Debug)]
pub struct Control {
    /// Symbols of the control region.
    pub symbols: usize,
    /// PCFICH REGs.
    pub pcfich: Vec<Reg>,
    /// PHICH REGs.
    pub phich: Vec<Reg>,
    /// PDCCH REGs in the order the (permuted, shifted) quadruplets are mapped.
    pub pdcch: Vec<Reg>,
}

/// The number of PHICH groups for Ng index (1/6, 1/2, 1, 2).
pub fn phich_groups(n_rb: usize, ng_index: u8, ext: bool) -> usize {
    let ng = [1.0 / 6.0, 0.5, 1.0, 2.0][ng_index as usize & 3];
    let g = (ng * n_rb as f64 / 8.0).ceil() as usize;
    if ext {
        2 * g
    } else {
        g
    }
}

/// The control region layout (normal PHICH duration).
pub fn control(pci: u16, n_rb: usize, ports: usize, cfi: u8, ng_index: u8, ext: bool) -> Control {
    let symbols = cfi as usize + usize::from(n_rb <= 10);
    let syms: Vec<Vec<Reg>> = (0..symbols)
        .map(|l| regs_of_symbol(pci, n_rb, ports, l, ext))
        .collect();
    // PCFICH: k = kbar + floor(i N_RB / 2) x 6, kbar = 6 (PCI mod 2 N_RB).
    let n_sc = 12 * n_rb;
    let kbar = 6 * (pci as usize % (2 * n_rb));
    let mut pcfich = Vec::new();
    for i in 0..4 {
        let k = (kbar + (i * n_rb / 2) * 6) % n_sc;
        if let Some(r) = syms[0].iter().find(|r| r.k0 == k) {
            pcfich.push(*r);
        }
    }
    // PHICH (normal duration, all in symbol 0): n_i = (floor(N_ID n'0 / n'0) + m' + floor(i n'0 / 3)) mod n'0.
    let free0: Vec<Reg> = syms[0]
        .iter()
        .filter(|r| !pcfich.contains(r))
        .cloned()
        .collect();
    let n0 = free0.len();
    let groups = phich_groups(n_rb, ng_index, ext);
    let mut phich = Vec::new();
    for m in 0..groups {
        for i in 0..3 {
            let n = (pci as usize * n0 / n0 + m + i * n0 / 3) % n0;
            phich.push(free0[n]);
        }
    }
    // PDCCH: for each k', for each l' in the region, a REG starting at (k', l') that is not PCFICH or PHICH.
    let mut pdcch = Vec::new();
    for k in 0..n_sc {
        for (l, regs) in syms.iter().enumerate() {
            if let Some(r) = regs.iter().find(|r| r.k0 == k && r.l == l) {
                if !pcfich.contains(r) && !phich.contains(r) {
                    pdcch.push(*r);
                }
            }
        }
    }
    Control {
        symbols,
        pcfich,
        phich,
        pdcch,
    }
}

/// The quadruplet permutation of the PDCCH (36.211 6.8.5): position i of the mapped sequence carries quadruplet `perm[i]`.
pub fn pdcch_permutation(m_quad: usize, pci: u16) -> Vec<usize> {
    let Ok(p) = Permutation::subblock_3gpp(m_quad, &LTE_SUBBLOCK_CONV) else {
        return Vec::new();
    };
    let w: Vec<usize> = (0..m_quad).map(|j| p.src(j).unwrap_or(j)).collect();
    // Cyclic shift: wbar(i) = w((i + N_ID) mod M_quad).
    (0..m_quad)
        .map(|i| w[(i + pci as usize) % m_quad])
        .collect()
}

/// A DCI format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DciFormat {
    /// Format 1A.
    F1a,
    /// Format 1C.
    F1c,
}

fn log2c(x: usize) -> usize {
    if x <= 1 {
        0
    } else {
        (usize::BITS - (x - 1).leading_zeros()) as usize
    }
}

/// The gap N_gap1 of a bandwidth (36.211 Table 6.2.3.2-1).
pub fn n_gap1(n_rb: usize) -> usize {
    match n_rb {
        6..=10 => n_rb.div_ceil(2),
        11 => 4,
        12..=19 => 8,
        20..=26 => 12,
        27..=44 => 18,
        45..=63 => 27,
        64..=79 => 32,
        _ => 48,
    }
}

/// N_VRB for the first gap.
pub fn n_vrb_gap1(n_rb: usize) -> usize {
    let g = n_gap1(n_rb);
    2 * g.min(n_rb - g)
}

/// The RBG size P.
pub fn rbg_size(n_rb: usize) -> usize {
    match n_rb {
        0..=10 => 1,
        11..=26 => 2,
        27..=63 => 3,
        _ => 4,
    }
}

/// N_RB^step of format 1C.
pub fn n_step(n_rb: usize) -> usize {
    if n_rb < 50 {
        2
    } else {
        4
    }
}

/// The payload size of a DCI format.
pub fn dci_size(f: DciFormat, n_rb: usize) -> usize {
    match f {
        DciFormat::F1a => {
            let rba = log2c(n_rb * (n_rb + 1) / 2);
            // Format 0 (FDD, common search space): flag, hopping, RBA, MCS, NDI, TPC, cyclic shift, CQI request, and the resource
            // allocation type bit; 1A is padded to match it, and once more when its size is one of the ambiguous sizes.
            let f0 = 1 + 1 + rba + 5 + 1 + 2 + 3 + 1 + 1;
            let mut s = (1 + 1 + rba + 5 + 3 + 1 + 2 + 2).max(f0);
            if [12, 14, 16, 20, 24, 26, 32, 40, 44, 56].contains(&s) {
                s += 1;
            }
            s
        }
        DciFormat::F1c => {
            let nv = n_vrb_gap1(n_rb) / n_step(n_rb);
            usize::from(n_rb >= 50) + log2c(nv * (nv + 1) / 2) + 5
        }
    }
}

/// A downlink assignment on SI-RNTI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dci {
    /// The format.
    pub format: DciFormat,
    /// Distributed VRBs.
    pub distributed: bool,
    /// Second gap (N_RB >= 50).
    pub gap2: bool,
    /// The first VRB.
    pub start: usize,
    /// VRBs.
    pub len: usize,
    /// MCS (1A) or TBS index (1C).
    pub mcs: u8,
    /// HARQ process (1A).
    pub harq: u8,
    /// New data (1A).
    pub ndi: u8,
    /// Redundancy version (1A).
    pub rv: u8,
    /// TPC (1A); its LSB picks the N_PRB^1A column (2 or 3).
    pub tpc: u8,
}

/// RIV of an allocation over `n` (36.213 7.1.6.3).
pub fn riv(n: usize, start: usize, len: usize) -> usize {
    if len - 1 <= n / 2 {
        n * (len - 1) + start
    } else {
        n * (n - len + 1) + (n - 1 - start)
    }
}

/// The (start, length) of a RIV over `n`.
pub fn riv_decode(n: usize, v: usize) -> Option<(usize, usize)> {
    for len in 1..=n {
        for start in 0..=n - len {
            if riv(n, start, len) == v {
                return Some((start, len));
            }
        }
    }
    None
}

fn put(bits: &mut Vec<u8>, v: usize, n: usize) {
    for i in (0..n).rev() {
        bits.push(((v >> i) & 1) as u8);
    }
}

/// Packs a DCI.
pub fn dci_pack(d: &Dci, n_rb: usize) -> Vec<u8> {
    let mut b = Vec::new();
    match d.format {
        DciFormat::F1a => {
            put(&mut b, 1, 1);
            put(&mut b, d.distributed as usize, 1);
            let rba = log2c(n_rb * (n_rb + 1) / 2);
            // The RIV of format 1A is over N_RB also for distributed VRBs (36.213 7.1.6.3).
            put(&mut b, riv(n_rb, d.start, d.len), rba);
            put(&mut b, d.mcs as usize, 5);
            put(&mut b, d.harq as usize, 3);
            // On SI-RNTI the new data indicator carries the gap of a distributed assignment (N_RB >= 50).
            let ndi = if d.distributed && n_rb >= 50 {
                d.gap2 as u8
            } else {
                d.ndi
            };
            put(&mut b, ndi as usize, 1);
            put(&mut b, d.rv as usize, 2);
            put(&mut b, d.tpc as usize, 2);
        }
        DciFormat::F1c => {
            if n_rb >= 50 {
                put(&mut b, d.gap2 as usize, 1);
            }
            let st = n_step(n_rb);
            let nv = n_vrb_gap1(n_rb) / st;
            put(
                &mut b,
                riv(nv, d.start / st, d.len / st),
                log2c(nv * (nv + 1) / 2),
            );
            put(&mut b, d.mcs as usize, 5);
        }
    }
    b.resize(dci_size(d.format, n_rb), 0);
    b
}

/// Parses a DCI of `format` on SI-RNTI; none when it does not parse (or uses the second gap, which is not modelled).
pub fn dci_parse(bits: &[u8], format: DciFormat, n_rb: usize) -> Option<Dci> {
    let mut p = 0;
    let mut get = |n: usize| {
        let v = bits[p..p + n]
            .iter()
            .fold(0usize, |a, &x| (a << 1) | x as usize);
        p += n;
        v
    };
    match format {
        DciFormat::F1a => {
            if get(1) != 1 {
                return None;
            }
            let distributed = get(1) == 1;
            let rba = log2c(n_rb * (n_rb + 1) / 2);
            let (start, len) = riv_decode(n_rb, get(rba))?;
            if distributed && start + len > n_vrb_gap1(n_rb) {
                return None;
            }
            let mcs = get(5) as u8;
            let harq = get(3) as u8;
            let ndi = get(1) as u8;
            let gap2 = distributed && n_rb >= 50 && ndi == 1;
            if gap2 {
                return None;
            }
            Some(Dci {
                format,
                distributed,
                gap2,
                start,
                len,
                mcs,
                harq,
                ndi: if distributed && n_rb >= 50 { 0 } else { ndi },
                rv: get(2) as u8,
                tpc: get(2) as u8,
            })
        }
        DciFormat::F1c => {
            let gap2 = if n_rb >= 50 { get(1) == 1 } else { false };
            if gap2 {
                return None;
            }
            let st = n_step(n_rb);
            let nv = n_vrb_gap1(n_rb) / st;
            let r = get(log2c(nv * (nv + 1) / 2));
            let (s, l) = riv_decode(nv, r)?;
            Some(Dci {
                format,
                distributed: true,
                gap2,
                start: s * st,
                len: l * st,
                mcs: get(5) as u8,
                harq: 0,
                ndi: 0,
                rv: 0,
                tpc: 0,
            })
        }
    }
}

/// The PRB of distributed VRB `n_vrb` in slot `ns` (36.211 6.2.3.2), first gap.
pub fn dvrb_to_prb(n_rb: usize, n_vrb: usize, ns: usize) -> usize {
    let n_gap = n_gap1(n_rb);
    let nt = n_vrb_gap1(n_rb);
    let p = rbg_size(n_rb);
    let n_row = nt.div_ceil(4 * p) * p;
    let n_null = 4 * n_row - nt;
    let nv = n_vrb % nt;
    let base = nt * (n_vrb / nt);
    let p1 = 2 * n_row * (nv % 2) + nv / 2 + base;
    let p2 = n_row * (nv % 4) + nv / 4 + base;
    let even = if n_null != 0 && nv >= nt - n_null && nv % 2 == 1 {
        p1 - n_row
    } else if n_null != 0 && nv >= nt - n_null && nv.is_multiple_of(2) {
        p1 - n_row + n_null / 2
    } else if n_null != 0 && nv < nt - n_null && nv % 4 >= 2 {
        p2 - n_null / 2
    } else {
        p2
    };
    let t = if ns.is_multiple_of(2) {
        even
    } else {
        (even + nt / 2) % nt + base
    };
    if t < nt / 2 {
        t
    } else {
        t + n_gap - nt / 2
    }
}

/// The PRBs of slot 0 and 1 of an assignment.
pub fn assignment_prbs(d: &Dci, n_rb: usize) -> [Vec<usize>; 2] {
    let mut out = [Vec::new(), Vec::new()];
    for (s, o) in out.iter_mut().enumerate() {
        for v in d.start..d.start + d.len {
            o.push(if d.distributed {
                dvrb_to_prb(n_rb, v, s)
            } else {
                v
            });
        }
        o.sort_unstable();
    }
    out
}

/// TBS of 36.213 Table 7.1.7.2.1-1, column N_PRB = 2, for I_TBS 0 to 26.
pub const TBS_COL2: [u16; 27] = [
    32, 56, 72, 104, 120, 144, 176, 224, 256, 296, 328, 376, 440, 488, 552, 600, 632, 696, 776,
    840, 904, 1000, 1064, 1128, 1192, 1256, 1480,
];
/// TBS of 36.213 Table 7.1.7.2.1-1, column N_PRB = 3, for I_TBS 0 to 26.
pub const TBS_COL3: [u16; 27] = [
    56, 88, 144, 176, 208, 224, 256, 328, 392, 456, 504, 584, 680, 744, 840, 904, 968, 1064, 1160,
    1288, 1384, 1480, 1608, 1736, 1800, 1864, 2216,
];
/// TBS of 36.213 Table 7.1.7.2.3-1 (DCI format 1C), I_TBS 0 to 31.
pub const TBS_1C: [u16; 32] = [
    40, 56, 72, 120, 136, 144, 176, 208, 224, 256, 280, 296, 328, 336, 392, 488, 552, 600, 632,
    696, 776, 840, 904, 1000, 1064, 1128, 1224, 1288, 1384, 1480, 1608, 1736,
];

/// The transport block size of a DCI on SI-RNTI.
pub fn tbs(d: &Dci) -> Option<usize> {
    match d.format {
        DciFormat::F1a => {
            let col = if d.tpc & 1 == 1 { &TBS_COL3 } else { &TBS_COL2 };
            col.get(d.mcs as usize).map(|&v| v as usize)
        }
        DciFormat::F1c => TBS_1C.get(d.mcs as usize).map(|&v| v as usize),
    }
}

/// The PDSCH resource elements of `prbs` (per slot) in a subframe, in mapping order (k first, then symbol).
pub fn pdsch_res(
    pci: u16,
    n_rb: usize,
    ports: usize,
    ext: bool,
    ctrl_symbols: usize,
    subframe: usize,
    prbs: &[Vec<usize>; 2],
) -> Vec<(usize, usize)> {
    let nsym = symbols_per_slot(ext);
    let mid_lo = 6 * n_rb - 36;
    let mid_hi = 6 * n_rb + 36;
    let mut v = Vec::new();
    for l in ctrl_symbols..2 * nsym {
        let slot = l / nsym;
        let ls = l % nsym;
        // PSS and SSS (subframes 0 and 5, last two symbols of slot 0) and PBCH (subframe 0, slot 1 symbols 0..3).
        let sync = (subframe == 0 || subframe == 5) && slot == 0 && ls >= nsym - 2;
        let pbch = subframe == 0 && slot == 1 && ls < 4;
        for &prb in &prbs[slot] {
            for k in 12 * prb..12 * prb + 12 {
                if is_crs(pci, ports, l, k, ext) {
                    continue;
                }
                if (sync || pbch) && (mid_lo..mid_hi).contains(&k) {
                    continue;
                }
                v.push((k, l));
            }
        }
    }
    v
}

/// The SIB1 redundancy version of a frame: ceil(3 k / 2) mod 4 with k = (SFN / 2) mod 4.
pub fn sib1_rv(sfn: u32) -> usize {
    let k = (sfn / 2) % 4;
    ((3 * k).div_ceil(2) % 4) as usize
}

/// The scrambling sequence c(n) of 36.211 7.2 for `c_init`, `len` values.
pub fn scrambling(c_init: u32, len: usize) -> Vec<u8> {
    let mut c = vec![0u8; len];
    gold_3gpp(c_init, &mut c);
    c
}

/// The largest information length (CRC included) that [`Tbcc`] decodes.
pub const TBCC_MAX_K: usize = 2048;

/// The LTE tail-biting convolutional code with its rate matching (36.212 5.1.3.1 and 5.1.4.2), for decoding and for encoding
/// again. It keeps one rate matcher per information length and all its buffers. After the first frame of a length, a decode
/// allocates nothing.
#[derive(Clone, Debug)]
pub struct Tbcc {
    viterbi: Option<Viterbi>,
    matchers: Vec<LteRateMatch>,
    quant: Vec<i8>,
    part: Vec<i8>,
    sum: Vec<f32>,
    llr: Vec<i8>,
    coded: Vec<u8>,
    streams: Vec<u8>,
}

impl Default for Tbcc {
    fn default() -> Self {
        Tbcc::new()
    }
}

impl Tbcc {
    /// A codec for information lengths up to [`TBCC_MAX_K`].
    pub fn new() -> Tbcc {
        Tbcc {
            viterbi: Viterbi::new(LTE_K7_R3, TBCC_MAX_K).ok(),
            matchers: Vec::new(),
            quant: Vec::new(),
            part: Vec::new(),
            sum: Vec::new(),
            llr: Vec::new(),
            coded: Vec::new(),
            streams: Vec::new(),
        }
    }

    /// The rate matcher for `k` information bits, made the first time.
    fn matcher(&mut self, k: usize) -> Option<usize> {
        if let Some(i) = self.matchers.iter().position(|m| m.stream_len() == k) {
            return Some(i);
        }
        let m = LteRateMatch::new(LteCode::Convolutional, k, 0).ok()?;
        self.matchers.push(m);
        Some(self.matchers.len() - 1)
    }

    /// Makes sure that every buffer holds `n` values.
    fn size(&mut self, n: usize) {
        for v in [&mut self.quant, &mut self.part, &mut self.llr] {
            if v.len() < n {
                v.resize(n, 0);
            }
        }
        if self.sum.len() < n {
            self.sum.resize(n, 0.0);
        }
        for v in [&mut self.coded, &mut self.streams] {
            if v.len() < n {
                v.resize(n, 0);
            }
        }
    }

    /// Decodes `soft` (the E received soft values, positive means 0) into the `k` information bits in `out`. Repeated values
    /// add before the decode. Returns false when `k` is 0 or above [`TBCC_MAX_K`], `out` is short, or a soft value is not
    /// finite.
    pub fn decode(&mut self, soft: &[f32], k: usize, out: &mut [u8]) -> bool {
        if k == 0 || k > TBCC_MAX_K || out.len() < k || soft.is_empty() {
            return false;
        }
        if check_finite(soft).is_err() {
            return false;
        }
        let Some(mi) = self.matcher(k) else {
            return false;
        };
        let n = 3 * k;
        self.size(n);
        let q = Quantizer::default();
        let scale = q.scale_for(soft);
        self.sum[..n].fill(0.0);
        let rm = &self.matchers[mi];
        // Each pass through the circular buffer sends every coded bit once. The i8 sums of one pass cannot saturate, and the
        // passes add in f32.
        for chunk in soft.chunks(n) {
            let quant = &mut self.quant[..chunk.len()];
            if q.quantize(chunk, scale, quant).is_err() {
                return false;
            }
            let part = &mut self.part[..n];
            part.fill(0);
            if rm.dematch(quant, 0, rm.buffer_len(), part).is_err() {
                return false;
            }
            for (s, &p) in self.sum[..n].iter_mut().zip(part.iter()) {
                *s += p as f32;
            }
        }
        let scale = q.scale_for(&self.sum[..n]);
        if q.quantize(&self.sum[..n], scale, &mut self.part[..n])
            .is_err()
        {
            return false;
        }
        // The rate matcher keeps the three streams one after the other. The decoder takes them interleaved per step.
        for i in 0..k {
            for s in 0..3 {
                self.llr[3 * i + s] = self.part[s * k + i];
            }
        }
        let Some(v) = self.viterbi.as_mut() else {
            return false;
        };
        v.decode(&self.llr[..n], Ends::TailBiting, &mut out[..k])
            .is_ok()
    }

    /// Encodes `bits` and rate-matches the code word to `out.len()` bits. Returns false when `bits` is empty or too long.
    pub fn encode(&mut self, bits: &[u8], out: &mut [u8]) -> bool {
        let k = bits.len();
        if k == 0 || k > TBCC_MAX_K || out.is_empty() {
            return false;
        }
        let Some(mi) = self.matcher(k) else {
            return false;
        };
        let n = 3 * k;
        self.size(n);
        if encode_tail_biting(&LTE_K7_R3, bits, &mut self.coded[..n]).is_err() {
            return false;
        }
        for i in 0..k {
            for s in 0..3 {
                self.streams[s * k + i] = self.coded[3 * i + s];
            }
        }
        let rm = &self.matchers[mi];
        rm.rate_match(&self.streams[..n], 0, rm.buffer_len(), out)
            .is_ok()
    }
}

/// Encodes `bits` with the LTE tail-biting convolutional code and rate-matches the code word to `e` bits (36.212 5.1.3.1 and
/// 5.1.4.2). Returns an empty vector for empty input.
pub fn tbcc_encode(bits: &[u8], e: usize) -> Vec<u8> {
    let mut out = vec![0u8; e];
    if !Tbcc::new().encode(bits, &mut out) {
        out.clear();
    }
    out
}

/// How well soft values agree with a code word: the sum of soft x sign over the sum of |soft| (1 is perfect, 0 is random).
pub fn agreement(soft: &[f32], bits: &[u8]) -> f32 {
    let (mut a, mut b) = (0f32, 0f32);
    for (s, &x) in soft.iter().zip(bits) {
        a += if x == 0 { *s } else { -*s };
        b += s.abs();
    }
    if b > 0.0 {
        a / b
    } else {
        0.0
    }
}

/// Appends the 16-bit CRC of `bits` (36.212 5.1.1, CRC16), XOR-ed with `mask`, most significant bit first.
pub fn append_crc16(bits: &mut Vec<u8>, mask: u16) {
    let c = CRC16_XMODEM.bits(bits) ^ mask;
    push_u64(bits, c as u64, 16);
}

/// Appends the CRC24A of `bits` (36.212 5.1.1), most significant bit first.
pub fn append_crc24a(bits: &mut Vec<u8>) {
    let c = CRC24_LTE_A.bits(bits);
    push_u64(bits, c as u64, 24);
}

/// The turbo code block size K and its filler bits F for a transport block of `tbs` bits with its CRC24A (36.212 5.1.2, one
/// code block). None when the block needs segmentation.
pub fn turbo_block(tbs: usize) -> Option<(usize, usize)> {
    let b = tbs + 24;
    let k = size_at_least(b)?;
    Some((k, k - b))
}

/// Turbo-encodes `block` (K bits, the first `f` filler bits 0) and rate-matches it to `e` bits for redundancy version `rv`
/// (36.212 5.1.3.2 and 5.1.4.1, the whole circular buffer). Returns an empty vector when K is not a turbo size.
pub fn turbo_encode(block: &[u8], f: usize, rv: u8, e: usize) -> Vec<u8> {
    let k = block.len();
    let (Ok(t), Ok(rm)) = (Turbo::new(k), LteRateMatch::new(LteCode::Turbo, k + 4, f)) else {
        return Vec::new();
    };
    let mut streams = vec![0u8; 3 * (k + 4)];
    let mut out = vec![0u8; e];
    if t.encode(block, &mut streams).is_err()
        || rm
            .rate_match(&streams, rv, rm.buffer_len(), &mut out)
            .is_err()
    {
        return Vec::new();
    }
    out
}

/// The turbo decoder of the system information path (36.212 5.1.3.2 and 5.1.4.1): soft combining of redundancy versions, then
/// max-log-MAP decoding with the CRC24A check. It keeps one rate matcher per (K, F) and one decoder per K. After the first
/// block of a size, nothing is allocated.
#[derive(Clone, Debug, Default)]
pub struct TurboRx {
    matchers: Vec<(usize, LteRateMatch)>,
    decoders: Vec<Turbo>,
    at: Option<(usize, usize)>,
    k: usize,
    f: usize,
    quant: Vec<i8>,
    part: Vec<i8>,
    sum: Vec<f32>,
    llr: Vec<i8>,
    hard: Vec<u8>,
}

impl TurboRx {
    /// Starts a block of size `k` with `f` filler bits: the soft sums go to zero. Returns false when `k` is not a turbo size.
    pub fn begin(&mut self, k: usize, f: usize) -> bool {
        self.at = None;
        if f >= k {
            return false;
        }
        let mi = match self
            .matchers
            .iter()
            .position(|(ff, m)| *ff == f && m.stream_len() == k + 4)
        {
            Some(i) => i,
            None => match LteRateMatch::new(LteCode::Turbo, k + 4, f) {
                Ok(m) => {
                    self.matchers.push((f, m));
                    self.matchers.len() - 1
                }
                Err(_) => return false,
            },
        };
        let di = match self.decoders.iter().position(|t| t.k() == k) {
            Some(i) => i,
            None => match Turbo::new(k) {
                Ok(t) => {
                    self.decoders.push(t);
                    self.decoders.len() - 1
                }
                Err(_) => return false,
            },
        };
        let n = 3 * (k + 4);
        for v in [&mut self.quant, &mut self.part, &mut self.llr] {
            if v.len() < n {
                v.resize(n, 0);
            }
        }
        if self.sum.len() < n {
            self.sum.resize(n, 0.0);
        }
        if self.hard.len() < k {
            self.hard.resize(k, 0);
        }
        self.sum[..n].fill(0.0);
        self.at = Some((mi, di));
        self.k = k;
        self.f = f;
        true
    }

    /// Adds the soft values `soft` (positive means 0) received for redundancy version `rv`. Returns false before
    /// [`TurboRx::begin`], for a bad `rv` or for a soft value that is not finite.
    pub fn add(&mut self, soft: &[f32], rv: u8) -> bool {
        let Some((mi, _)) = self.at else {
            return false;
        };
        if rv > 3 || check_finite(soft).is_err() {
            return false;
        }
        let n = 3 * (self.k + 4);
        let rm = &self.matchers[mi].1;
        // The bits one pass through the circular buffer sends: every stream bit except the filler bits of d0 and d1.
        let pass = n - 2 * self.f;
        let q = Quantizer::default();
        let scale = q.scale_for(soft);
        for chunk in soft.chunks(pass) {
            let quant = &mut self.quant[..chunk.len()];
            if q.quantize(chunk, scale, quant).is_err() {
                return false;
            }
            let part = &mut self.part[..n];
            part.fill(0);
            if rm.dematch(quant, rv, rm.buffer_len(), part).is_err() {
                return false;
            }
            for (s, &p) in self.sum[..n].iter_mut().zip(part.iter()) {
                *s += p as f32;
            }
        }
        true
    }

    /// Decodes the sums with at most `iters` iterations. Returns the K decided bits (filler bits first) when the CRC24A of the
    /// bits after the filler checks.
    pub fn decode(&mut self, iters: u32) -> Option<&[u8]> {
        let (_, di) = self.at?;
        let (k, f) = (self.k, self.f);
        let n = 3 * (k + 4);
        let q = Quantizer::default();
        let scale = q.scale_for(&self.sum[..n]);
        q.quantize(&self.sum[..n], scale, &mut self.llr[..n]).ok()?;
        // The filler bits are known zeros.
        self.llr[..f].fill(127);
        let hard = &mut self.hard[..k];
        self.decoders[di]
            .decode(
                &self.llr[..n],
                iters,
                |h| CRC24_LTE_A.verify_bits(&h[f..]),
                hard,
            )
            .ok()?;
        Some(&self.hard[..k])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lte_sequences() {
        let p = pss(0);
        assert!((p[0].re - 1.0).abs() < 1e-6);
        // SSS of every identity distinct: low correlation with a neighbour.
        let a = sss(100, 1, false);
        let b = sss(101, 1, false);
        let c: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>().abs();
        assert!(c < 30.0);
        let a5 = sss(100, 1, true);
        assert_ne!(a, a5);
        assert_eq!(pbch_res(301, 25, false).len(), 240);
        assert_eq!(pbch_res(301, 25, true).len(), 216);
        assert_eq!(crs_res(301, 25, 0, 0, 0, false).len(), 50);
    }

    #[test]
    fn numerology() {
        assert_eq!(symbol_start(128, false, 7).0, 960);
        assert_eq!(symbol_start(128, false, 1), (138, 9));
        assert_eq!(bin(6, 128, 0), 92);
        assert_eq!(bin(6, 128, 36), 1);
    }

    #[test]
    fn dci_sizes_and_round_trips() {
        for (n, s1a, s1c) in [
            (6, 21, 8),
            (15, 22, 10),
            (25, 25, 12),
            (50, 27, 13),
            (75, 27, 14),
            (100, 28, 15),
        ] {
            assert_eq!(dci_size(DciFormat::F1a, n), s1a, "1A {n}");
            assert_eq!(dci_size(DciFormat::F1c, n), s1c, "1C {n}");
        }
        for n in [6usize, 15, 25, 50, 100] {
            let d = Dci {
                format: DciFormat::F1a,
                distributed: false,
                gap2: false,
                start: 2,
                len: 3,
                mcs: 7,
                harq: 0,
                ndi: 0,
                rv: 2,
                tpc: 1,
            };
            assert_eq!(dci_parse(&dci_pack(&d, n), DciFormat::F1a, n), Some(d));
            let d = Dci {
                distributed: true,
                start: 0,
                len: 4,
                ..d
            };
            assert_eq!(dci_parse(&dci_pack(&d, n), DciFormat::F1a, n), Some(d));
            let st = n_step(n);
            let c = Dci {
                format: DciFormat::F1c,
                distributed: true,
                gap2: false,
                start: 0,
                len: 2 * st,
                mcs: 9,
                harq: 0,
                ndi: 0,
                rv: 0,
                tpc: 0,
            };
            assert_eq!(dci_parse(&dci_pack(&c, n), DciFormat::F1c, n), Some(c));
        }
    }

    #[test]
    fn vrb_mapping_is_a_permutation() {
        for n in [6usize, 15, 25, 50, 100] {
            for s in 0..2 {
                let nt = n_vrb_gap1(n);
                let mut v: Vec<usize> = (0..nt).map(|x| dvrb_to_prb(n, x, s)).collect();
                v.sort_unstable();
                v.dedup();
                assert_eq!(v.len(), nt, "{n} {s}");
                assert!(v.iter().all(|&p| p < n));
            }
        }
        assert_eq!(sib1_rv(0), 0);
        assert_eq!(sib1_rv(2), 2);
        assert_eq!(sib1_rv(4), 3);
        assert_eq!(sib1_rv(6), 1);
    }

    #[test]
    fn control_region() {
        let c = control(301, 25, 4, 2, 1, false);
        assert_eq!(c.pcfich.len(), 4);
        assert_eq!(c.phich.len(), 6);
        // 25 RBs, CFI 2, 4 ports: symbol 0 has 50 REGs, symbol 1 (four ports) 50: 100 - 4 - 6 = 90 REGs, 10 CCEs.
        assert_eq!(c.pdcch.len(), 90);
        let p = pdcch_permutation(90, 301);
        let mut s = p.clone();
        s.sort_unstable();
        assert_eq!(s, (0..90).collect::<Vec<_>>());
    }
}
