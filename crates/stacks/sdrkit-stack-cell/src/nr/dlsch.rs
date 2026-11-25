//! The NR downlink signals that carry SIB1, as the specifications define them: the numerology of a carrier, CORESET#0 and the
//! Type0 PDCCH occasions (3GPP TS 38.213 clauses 10.1 and 13), the PDCCH (TS 38.211 clause 7.3.2, TS 38.212 clause 7.3), DCI
//! format 1_0 with the SI-RNTI, and the PDSCH with its DM-RS (TS 38.211 clauses 7.3.1 and 7.4.1.1, TS 38.214 clause 5.1).
//!
//! Frequency range 1, normal cyclic prefix, one layer, QPSK (TS 38.214 clause 5.1.3 allows no higher order for the SI-RNTI).

use super::phy::Mib;
use sdrkit_algo_fec::polar::{Polar, PolarParams};
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC24_NR_C;
use sdrkit_util_bits::pack::{field, push_u64};
use sdrkit_util_bits::seq::gold_3gpp;

/// The RNTI of system information.
pub const SI_RNTI: u16 = 0xFFFF;

/// A carrier grid: the subcarrier spacing and the FFT size it is sampled with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numerology {
    /// Subcarrier spacing, kHz (15 or 30).
    pub scs_khz: u32,
    /// FFT size (a power of two, at least 128).
    pub fft: usize,
}

impl Numerology {
    /// The grid of `n_sc` subcarriers at `scs_khz`: the smallest FFT that leaves a guard of a quarter.
    pub fn for_width(scs_khz: u32, n_sc: usize) -> Numerology {
        Numerology {
            scs_khz,
            fft: (n_sc * 4 / 3).next_power_of_two().max(128),
        }
    }
    /// μ.
    pub fn mu(&self) -> u32 {
        u32::from(self.scs_khz == 30)
    }
    /// The sample rate, Hz.
    pub fn rate(&self) -> f64 {
        self.fft as f64 * f64::from(self.scs_khz) * 1e3
    }
    /// Slots per frame.
    pub fn slots_per_frame(&self) -> usize {
        10 << self.mu()
    }
    /// The start of slot `slot`, counted from the start of a frame, samples.
    pub fn slot_start(&self, slot: usize) -> usize {
        self.symbol(14 * slot).0
    }
    /// The start (with the cyclic prefix) of symbol `l`, counted from the start of a frame, and its cyclic prefix, samples
    /// (TS 38.211 clause 5.3.1).
    pub fn symbol(&self, l: usize) -> (usize, usize) {
        let per_half_ms = 7usize << self.mu();
        let cp = self.fft * 9 / 128;
        let extra = (self.fft << self.mu()) / 128;
        let half_ms = self.fft * self.scs_khz as usize / 2;
        let (block, i) = (l / per_half_ms, l % per_half_ms);
        let start = block * half_ms
            + if i == 0 {
                0
            } else {
                extra + i * (self.fft + cp)
            };
        (start, if i == 0 { cp + extra } else { cp })
    }
    /// The phase that up-conversion to the carrier `f_hz` gives symbol `l` (TS 38.211 clause 5.4): every subcarrier has phase
    /// zero at the start of the useful part, so a grid mixed to 0 Hz from `f_hz` sees `exp(-j 2 pi f t)` with `t` the useful
    /// start within the subframe.
    pub fn carrier_phase(&self, f_hz: f64, l: usize) -> Complex32 {
        let (start, cp) = self.symbol(l);
        let per_ms = self.fft * self.scs_khz as usize;
        let t = ((start + cp) % per_ms) as f64 / self.rate();
        let turns = (f_hz * t).fract();
        let a = -std::f64::consts::TAU * turns;
        Complex32::new(a.cos() as f32, a.sin() as f32)
    }
}

/// CORESET#0 of a cell (TS 38.213 clause 13, tables 13-1 to 13-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coreset0 {
    /// Subcarrier spacing of the PDCCH and of SIB1, kHz.
    pub scs_khz: u32,
    /// Resource blocks.
    pub n_rb: usize,
    /// Symbols.
    pub n_symb: usize,
    /// Resource blocks from the first of CORESET#0 to the first common resource block that the SS/PBCH block overlaps.
    pub offset_rb: usize,
}

impl Coreset0 {
    /// CORESET#0 of the cell whose SS/PBCH block has the spacing `ssb_scs` (kHz) and whose MIB is `mib`. None when the MIB
    /// schedules no SIB1 or the index is reserved.
    pub fn from_mib(ssb_scs: u32, mib: &Mib) -> Option<Coreset0> {
        if mib.k_ssb > 23 {
            return None;
        }
        let scs_khz = if mib.scs_common_30 { 30 } else { 15 };
        let (rbs, symbols, offset) = crate::bands::coreset0(ssb_scs, scs_khz, mib.coreset0 as usize)?;
        Some(Coreset0 {
            scs_khz,
            n_rb: rbs as usize,
            n_symb: symbols as usize,
            offset_rb: offset as usize,
        })
    }
    /// Subcarriers.
    pub fn n_sc(&self) -> usize {
        12 * self.n_rb
    }
    /// The width, Hz.
    pub fn width_hz(&self) -> f64 {
        self.n_sc() as f64 * f64::from(self.scs_khz) * 1e3
    }
    /// The numerology the grid is sampled with.
    pub fn numerology(&self) -> Numerology {
        Numerology::for_width(self.scs_khz, self.n_sc())
    }
    /// The frequency of subcarrier `n_sc / 2` (the 0 Hz bin of the grid) relative to the centre of the SS/PBCH block, Hz: the
    /// block's first subcarrier is 120 block subcarriers below its centre, the common grid `k_ssb` steps of 15 kHz below that,
    /// and CORESET#0 `offset_rb` blocks below that.
    pub fn centre_from_ssb(&self, ssb_scs: u32, k_ssb: u8) -> f64 {
        let scs = f64::from(self.scs_khz) * 1e3;
        -120.0 * f64::from(ssb_scs) * 1e3 - f64::from(k_ssb) * 15e3 - (12 * self.offset_rb) as f64 * scs
            + (self.n_sc() / 2) as f64 * scs
    }
    /// Control channel elements.
    pub fn n_cce(&self) -> usize {
        self.n_rb * self.n_symb / 6
    }
    /// The REG bundle of CCE `j` for cell `pci`: interleaved mapping with bundles of 6, R = 2 and a shift of the cell identity
    /// (TS 38.211 clause 7.3.2.2).
    pub fn bundle(&self, pci: u16, j: usize) -> usize {
        let n = self.n_cce();
        let c = n / 2;
        let (cc, r) = (j / 2, j % 2);
        (r * c + cc + pci as usize) % n
    }
    /// The resource blocks of the PDCCH on CCEs `first .. first + level`, ascending.
    pub fn pdcch_rbs(&self, pci: u16, first: usize, level: usize) -> Vec<usize> {
        let per = 6 / self.n_symb;
        let mut rbs: Vec<usize> = (first..first + level)
            .flat_map(|j| {
                let b = self.bundle(pci, j);
                b * per..(b + 1) * per
            })
            .collect();
        rbs.sort_unstable();
        rbs
    }
    /// The PDCCH candidates of the Type0 common search space: (aggregation level, first CCE), 4 of level 4, 2 of level 8 and
    /// 1 of level 16 as far as they fit (TS 38.213 table 10.1-1 and clause 10.1 with Y = 0).
    pub fn candidates(&self) -> Vec<(usize, usize)> {
        let n = self.n_cce();
        let mut v = Vec::new();
        for (level, count) in [(4usize, 4usize), (8, 2), (16, 1)] {
            if level > n {
                continue;
            }
            for m in 0..count {
                let c = (level, level * ((m * n / (level * count)) % (n / level)));
                if !v.contains(&c) {
                    v.push(c);
                }
            }
        }
        v
    }
}

/// The Type0 PDCCH occasion of SS/PBCH block `i` for searchSpaceZero `ss0` (TS 38.213 table 13-11, multiplexing pattern 1 in
/// frequency range 1): the slot `n0` of the frame, the parity of the frames that hold it, and the first symbol of CORESET#0.
/// The slot after `n0` is monitored too.
pub fn type0_occasion(ss0: u8, mu: u32, i: usize, n_symb: usize) -> (usize, usize, usize) {
    // (O in ms, twice M, the first symbol rule: 0 always 0, 1 by the parity of i, else the symbol + 1)
    const T: [(usize, usize, usize); 16] = [
        (0, 2, 0),
        (0, 1, 1),
        (2, 2, 0),
        (2, 1, 1),
        (5, 2, 0),
        (5, 1, 1),
        (7, 2, 0),
        (7, 1, 1),
        (0, 4, 0),
        (5, 4, 0),
        (0, 2, 2),
        (0, 2, 3),
        (2, 2, 2),
        (2, 2, 3),
        (5, 2, 2),
        (5, 2, 3),
    ];
    let (o, m2, rule) = T[(ss0 & 15) as usize];
    let slots = 10usize << mu;
    let total = (o << mu) + i * m2 / 2;
    let first = match rule {
        0 => 0,
        1 => {
            if i.is_multiple_of(2) {
                0
            } else {
                n_symb
            }
        }
        r => r - 1,
    };
    (total % slots, (total / slots) % 2, first)
}

/// The Gold sequence of `c_init` as QPSK symbols: `r(m) = ((1 - 2 c(2m)) + j (1 - 2 c(2m + 1))) / sqrt 2`.
fn qpsk_sequence(c_init: u32, n: usize) -> Vec<Complex32> {
    let mut c = vec![0u8; 2 * n];
    gold_3gpp(c_init, &mut c);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    (0..n)
        .map(|m| Complex32::new(s * (1.0 - 2.0 * f32::from(c[2 * m])), s * (1.0 - 2.0 * f32::from(c[2 * m + 1]))))
        .collect()
}

/// `c_init` of the PDCCH and PDSCH DM-RS in symbol `l` of slot `slot` with the scrambling identity `n_id`.
fn dmrs_init(n_id: u16, slot: usize, l: usize) -> u32 {
    let n = u64::from(n_id);
    (((1u64 << 17) * (14 * slot as u64 + l as u64 + 1) * (2 * n + 1) + 2 * n) % (1 << 31)) as u32
}

/// The PDCCH DM-RS of CORESET#0 in symbol `l` of slot `slot`: three values per resource block from the first block of the
/// CORESET, on subcarriers 1, 5 and 9 (TS 38.211 clause 7.4.1.3).
pub fn pdcch_dmrs(pci: u16, slot: usize, l: usize, n_rb: usize) -> Vec<Complex32> {
    qpsk_sequence(dmrs_init(pci, slot, l), 3 * n_rb)
}

/// The PDSCH DM-RS in symbol `l` of slot `slot`: configuration type 1, port 1000, six values per resource block from the
/// first block of CORESET#0, on the even subcarriers (TS 38.211 clause 7.4.1.1).
pub fn pdsch_dmrs(pci: u16, slot: usize, l: usize, n_rb: usize) -> Vec<Complex32> {
    qpsk_sequence(dmrs_init(pci, slot, l), 6 * n_rb)
}

/// The scrambling sequence of a PDCCH of the common search space of cell `pci` (TS 38.211 clause 7.3.2.3).
pub fn pdcch_scrambling(pci: u16, n: usize) -> Vec<u8> {
    let mut c = vec![0u8; n];
    gold_3gpp(u32::from(pci), &mut c);
    c
}

/// The scrambling sequence of a PDSCH with the SI-RNTI of cell `pci` (TS 38.211 clause 7.3.1.1).
pub fn pdsch_scrambling(pci: u16, n: usize) -> Vec<u8> {
    let mut c = vec![0u8; n];
    gold_3gpp((u32::from(SI_RNTI) << 15) + u32::from(pci), &mut c);
    c
}

/// DCI format 1_0 with the CRC scrambled by the SI-RNTI (TS 38.212 clause 7.3.1.2.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dci {
    /// Frequency domain resource assignment: the resource indication value over the blocks of CORESET#0.
    pub riv: u32,
    /// Time domain resource assignment: the row of the table, 0 to 15.
    pub tdra: u8,
    /// VRB-to-PRB mapping is interleaved.
    pub interleaved: bool,
    /// Modulation and coding scheme.
    pub mcs: u8,
    /// Redundancy version.
    pub rv: u8,
    /// System information indicator: false for SIB1, true for an SI message.
    pub si_message: bool,
}

impl Dci {
    /// Bits of the frequency domain resource assignment for `n_rb` blocks.
    pub fn riv_bits(n_rb: usize) -> u32 {
        (n_rb * (n_rb + 1) / 2).next_power_of_two().trailing_zeros()
    }
    /// Bits of the format for a CORESET#0 of `n_rb` blocks: the assignment and 28 more (15 of them reserved).
    pub fn len(n_rb: usize) -> usize {
        Self::riv_bits(n_rb) as usize + 28
    }
    /// The payload bits.
    pub fn pack(&self, n_rb: usize) -> Vec<u8> {
        let mut b = Vec::with_capacity(Self::len(n_rb));
        push_u64(&mut b, u64::from(self.riv), Self::riv_bits(n_rb));
        push_u64(&mut b, u64::from(self.tdra), 4);
        push_u64(&mut b, u64::from(self.interleaved), 1);
        push_u64(&mut b, u64::from(self.mcs), 5);
        push_u64(&mut b, u64::from(self.rv), 2);
        push_u64(&mut b, u64::from(self.si_message), 1);
        push_u64(&mut b, 0, 15);
        b
    }
    /// From the payload bits.
    pub fn unpack(bits: &[u8], n_rb: usize) -> Option<Dci> {
        let n = Self::riv_bits(n_rb);
        let at = n as usize;
        Some(Dci {
            riv: field(bits, 0, n)? as u32,
            tdra: field(bits, at, 4)? as u8,
            interleaved: field(bits, at + 4, 1)? == 1,
            mcs: field(bits, at + 5, 5)? as u8,
            rv: field(bits, at + 10, 2)? as u8,
            si_message: field(bits, at + 12, 1)? == 1,
        })
    }
}

/// The resource indication value of `len` blocks from `start` in a width of `n` (TS 38.214 clause 5.1.2.2.2).
pub fn riv_encode(n: usize, start: usize, len: usize) -> u32 {
    if len - 1 <= n / 2 {
        (n * (len - 1) + start) as u32
    } else {
        (n * (n - len + 1) + (n - 1 - start)) as u32
    }
}

/// The start and the length of a resource indication value in a width of `n`, or none when it names no allocation.
pub fn riv_decode(n: usize, riv: u32) -> Option<(usize, usize)> {
    let (a, b) = (riv as usize / n, riv as usize % n);
    let (start, len) = if a + b < n { (b, a + 1) } else { (n - 1 - b, n + 1 - a) };
    (len >= 1 && start + len <= n && riv_encode(n, start, len) == riv).then_some((start, len))
}

/// The physical block of virtual block `vrb` of a PDSCH scheduled in the Type0 common search space, counted from the first
/// block of CORESET#0 (`n_rb` blocks): itself, or with interleaving the bundle of two blocks moved by the interleaver with
/// R = 2 (TS 38.211 clause 7.3.1.6).
pub fn vrb_to_prb(n_rb: usize, interleaved: bool, vrb: usize) -> usize {
    if !interleaved {
        return vrb;
    }
    let bundles = n_rb.div_ceil(2);
    let (j, within) = (vrb / 2, vrb % 2);
    let c = bundles / 2;
    let f = if j == bundles - 1 || j >= 2 * c {
        j
    } else {
        (j % 2) * c + j / 2
    };
    2 * f + within
}

/// The bits of a DCI with its CRC: 24 ones are put before the payload for the CRC, and the RNTI is added to the last 16 CRC
/// bits (TS 38.212 clause 7.3.2).
pub fn dci_with_crc(payload: &[u8], rnti: u16) -> Vec<u8> {
    let mut x = vec![1u8; 24];
    x.extend_from_slice(payload);
    let crc = CRC24_NR_C.bits(&x) as u64 ^ u64::from(rnti);
    let mut out = payload.to_vec();
    push_u64(&mut out, crc, 24);
    out
}

/// Whether `bits` (a payload and 24 CRC bits) pass the CRC with `rnti`.
pub fn dci_crc_ok(bits: &[u8], rnti: u16) -> bool {
    let Some(a) = bits.len().checked_sub(24) else {
        return false;
    };
    let mut x = [1u8; 24 + 164];
    if a + 24 > 164 {
        return false;
    }
    x[24..24 + a].copy_from_slice(&bits[..a]);
    let crc = CRC24_NR_C.bits(&x[..24 + a]) as u64 ^ u64::from(rnti);
    field(bits, a, 24) == Some(crc)
}

/// The polar code of a DCI of `payload` bits on `level` CCEs (108 bits each).
pub fn dci_code(payload: usize, level: usize) -> Option<Polar> {
    Polar::new(PolarParams::downlink(payload + 24, 108 * level), 8).ok()
}

/// A PDSCH time allocation of the default table A (TS 38.214 table 5.1.2.1.1-2): mapping type B, the first symbol and the
/// number of symbols of row `row` (0 to 15) for the DM-RS position of the MIB.
pub fn tdra_default_a(row: u8, dmrs_pos3: bool) -> (bool, usize, usize) {
    // (type B, S and L for position 2, S and L for position 3)
    const T: [(bool, usize, usize, usize, usize); 16] = [
        (false, 2, 12, 3, 11),
        (false, 2, 10, 3, 9),
        (false, 2, 9, 3, 8),
        (false, 2, 7, 3, 6),
        (false, 2, 5, 3, 4),
        (true, 9, 4, 10, 4),
        (true, 4, 4, 6, 4),
        (true, 5, 7, 5, 7),
        (true, 5, 2, 5, 2),
        (true, 9, 2, 9, 2),
        (true, 12, 2, 12, 2),
        (false, 1, 13, 1, 13),
        (false, 1, 6, 1, 6),
        (false, 2, 4, 2, 4),
        (true, 4, 7, 4, 7),
        (true, 8, 4, 8, 4),
    ];
    let (b, s2, l2, s3, l3) = T[(row & 15) as usize];
    if dmrs_pos3 {
        (b, s3, l3)
    } else {
        (b, s2, l2)
    }
}

/// A PDSCH allocation in a slot with its DM-RS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pdsch {
    /// The first symbol.
    pub start: usize,
    /// Symbols.
    pub len: usize,
    /// The DM-RS symbols of the slot.
    pub dmrs_symbols: Vec<usize>,
    /// DM-RS CDM groups without data: 2, or 1 for an allocation of two symbols.
    pub cdm_groups: usize,
    /// The physical blocks in the order of the virtual blocks, counted from the first block of CORESET#0.
    pub prbs: Vec<usize>,
}

impl Pdsch {
    /// The allocation that `dci` gives in a CORESET#0 of `n_rb` blocks for the DM-RS position of the MIB: the time allocation of
    /// the default table A, single-symbol DM-RS of type 1 with the additional positions of TS 38.214 clause 5.1.6.2 and
    /// TS 38.211 table 7.4.1.1.2-3. None when the DCI names no allocation.
    pub fn from_dci(dci: &Dci, n_rb: usize, dmrs_pos3: bool) -> Option<Pdsch> {
        let (start_rb, n) = riv_decode(n_rb, dci.riv)?;
        let (type_b, start, len) = tdra_default_a(dci.tdra, dmrs_pos3);
        let dmrs_symbols: Vec<usize> = if type_b {
            if len >= 5 {
                vec![start, start + 4]
            } else {
                vec![start]
            }
        } else {
            let l0 = if dmrs_pos3 { 3 } else { 2 };
            match start + len {
                0..=7 => vec![l0],
                8 | 9 => vec![l0, 7],
                10..=12 => vec![l0, 6, 9],
                _ => vec![l0, 7, 11],
            }
        };
        Some(Pdsch {
            start,
            len,
            dmrs_symbols,
            cdm_groups: if type_b && len == 2 { 1 } else { 2 },
            prbs: (start_rb..start_rb + n).map(|v| vrb_to_prb(n_rb, dci.interleaved, v)).collect(),
        })
    }
    /// Resource elements per block that the transport block size counts: the symbols less the DM-RS elements and the elements
    /// of the CDM groups without data.
    pub fn re_per_prb(&self) -> usize {
        12 * self.len - self.dmrs_symbols.len() * 6 * self.cdm_groups
    }
    /// The data resource elements (subcarrier from the first of CORESET#0, symbol) in mapping order: by symbol, then by
    /// virtual block, then by subcarrier.
    pub fn data_res(&self) -> Vec<(usize, usize)> {
        let mut v = Vec::with_capacity(self.re_per_prb() * self.prbs.len());
        for l in self.start..self.start + self.len {
            let dmrs = self.dmrs_symbols.contains(&l);
            if dmrs && self.cdm_groups == 2 {
                continue;
            }
            for &rb in &self.prbs {
                for k in 0..12 {
                    if !(dmrs && k % 2 == 0) {
                        v.push((12 * rb + k, l));
                    }
                }
            }
        }
        v
    }
    /// The amplitude of the DM-RS relative to the data: 3 dB more with two CDM groups without data (TS 38.214 table 4.1-1).
    pub fn dmrs_gain(&self) -> f32 {
        if self.cdm_groups == 2 {
            std::f32::consts::SQRT_2
        } else {
            1.0
        }
    }
}

/// The modulation order and the code rate x 1024 of MCS index `i` of TS 38.214 table 5.1.3.1-1, for the QPSK rows.
pub fn mcs_qpsk(i: u8) -> Option<(usize, u32)> {
    const R: [u32; 10] = [120, 157, 193, 251, 308, 379, 449, 526, 602, 679];
    R.get(i as usize).map(|&r| (2, r))
}

/// TS 38.214 table 5.1.3.2-1: the transport block sizes up to 3824 bits.
const TBS: [u16; 93] = [
    24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136, 144, 152, 160, 168, 176, 184, 192, 208, 224, 240, 256,
    272, 288, 304, 320, 336, 352, 368, 384, 408, 432, 456, 480, 504, 528, 552, 576, 608, 640, 672, 704, 736, 768, 808, 848,
    888, 928, 984, 1032, 1064, 1128, 1160, 1192, 1224, 1256, 1288, 1320, 1352, 1416, 1480, 1544, 1608, 1672, 1736, 1800,
    1864, 1928, 2024, 2088, 2152, 2216, 2280, 2408, 2472, 2536, 2600, 2664, 2728, 2792, 2856, 2976, 3104, 3240, 3368, 3496,
    3624, 3752, 3824,
];

/// The transport block size of `n_prb` blocks with `re_per_prb` elements each at `qm` bits per symbol and the code rate
/// `r1024 / 1024`, one layer (TS 38.214 clause 5.1.3.2). None above 3824 bits, which system information never is.
pub fn tbs(re_per_prb: usize, n_prb: usize, qm: usize, r1024: u32) -> Option<usize> {
    let n_re = re_per_prb.min(156) * n_prb;
    let n_info = (n_re * qm) as f64 * f64::from(r1024) / 1024.0;
    if n_info > 3824.0 || n_info < 1.0 {
        return None;
    }
    let n = ((n_info.log2().floor() as i32) - 6).max(3) as u32;
    let step = (1u64 << n) as f64;
    let quantised = (step * (n_info / step).floor()).max(24.0);
    TBS.iter().map(|&t| t as usize).find(|&t| t as f64 >= quantised)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerology_and_occasions() {
        // 30 kHz at 512: 15.36 Msps, 7680 samples per half millisecond, 14 symbols in it.
        let n = Numerology::for_width(30, 288);
        assert_eq!((n.fft, n.rate()), (512, 15.36e6));
        assert_eq!(n.symbol(0), (0, 44));
        assert_eq!(n.symbol(1), (556, 36));
        assert_eq!(n.symbol(14).0, 7680);
        assert_eq!(n.symbol(280).0, 153_600);
        // 15 kHz at 512: 7.68 Msps, 7 symbols per half millisecond.
        let n = Numerology::for_width(15, 288);
        assert_eq!(n.symbol(7), (3840, 40));
        assert_eq!(n.slot_start(1), 7680);
        assert_eq!(Numerology::for_width(15, 1152).fft, 2048);
        // A carrier on a multiple of the symbol rate sees no phase; half a turn at half of it.
        let u = n.symbol(1).0 + n.symbol(1).1;
        let f = 7.68e6 / u as f64;
        assert!((n.carrier_phase(f * 3.0, 1) - Complex32::new(1.0, 0.0)).norm() < 1e-3);
        assert!((n.carrier_phase(f * 2.5, 1) - Complex32::new(-1.0, 0.0)).norm() < 1e-3);
        // Table 13-11. Index 0: O = 0, M = 1: block i in slot i of even frames, symbol 0.
        assert_eq!(type0_occasion(0, 1, 5, 2), (5, 0, 0));
        // Index 4 at 30 kHz: O = 5 ms is 10 slots. Index 9 at 15 kHz with M = 2: block 3 in slot 5 + 6 = 11, the next frame.
        assert_eq!(type0_occasion(4, 1, 3, 2), (13, 0, 0));
        assert_eq!(type0_occasion(9, 0, 3, 2), (1, 1, 0));
        // Index 1: M = 1/2, two per slot: blocks 2 and 3 in slot 1, the odd block after the CORESET of the even one.
        assert_eq!(type0_occasion(1, 0, 2, 2), (1, 0, 0));
        assert_eq!(type0_occasion(1, 0, 3, 2), (1, 0, 2));
        assert_eq!(type0_occasion(13, 0, 0, 1), (2, 0, 2));
    }

    #[test]
    fn coreset_and_dci() {
        let c = Coreset0 {
            scs_khz: 30,
            n_rb: 24,
            n_symb: 2,
            offset_rb: 2,
        };
        // 24 blocks of 2 symbols: 8 CCEs of 3 blocks; four candidates of level 4 fold to two, and one of level 8.
        assert_eq!(c.n_cce(), 8);
        assert_eq!(c.candidates(), vec![(4, 0), (4, 4), (8, 0)]);
        // The interleaver with R = 2 and C = 4 visits every bundle once; without a shift CCE 0, 1, 2 go to bundles 0, 4, 1.
        assert_eq!([c.bundle(0, 0), c.bundle(0, 1), c.bundle(0, 2)], [0, 4, 1]);
        let mut all: Vec<usize> = (0..8).map(|j| c.bundle(501, j)).collect();
        all.sort_unstable();
        assert_eq!(all, (0..8).collect::<Vec<_>>());
        assert_eq!(c.pdcch_rbs(0, 0, 4), vec![0, 1, 2, 3, 4, 5, 12, 13, 14, 15, 16, 17]);
        // The SS/PBCH block of 20 blocks sits 2 blocks into the 24 with k_SSB 0: the grid centre is the block centre.
        assert_eq!(c.centre_from_ssb(30, 0), 0.0);
        assert_eq!(c.centre_from_ssb(30, 2), -30e3);
        // DCI format 1_0 with the SI-RNTI: 37, 39 and 41 bits for 24, 48 and 96 blocks.
        assert_eq!([Dci::len(24), Dci::len(48), Dci::len(96)], [37, 39, 41]);
        let d = Dci {
            riv: riv_encode(24, 3, 20),
            tdra: 1,
            interleaved: true,
            mcs: 4,
            rv: 3,
            si_message: false,
        };
        let bits = d.pack(24);
        assert_eq!(Dci::unpack(&bits, 24), Some(d));
        let with = dci_with_crc(&bits, SI_RNTI);
        assert!(dci_crc_ok(&with, SI_RNTI));
        assert!(!dci_crc_ok(&with, 0x1234));
        // The resource indication value: 24 blocks all is 24 x (24 - 24 + 1) + 23 = 47; one block at 5 is 5.
        assert_eq!(riv_encode(24, 0, 24), 47);
        assert_eq!(riv_decode(24, 5), Some((5, 1)));
        for (s, l) in [(0, 24), (3, 20), (11, 13), (23, 1), (0, 12), (5, 14)] {
            assert_eq!(riv_decode(24, riv_encode(24, s, l)), Some((s, l)));
        }
        assert_eq!(riv_decode(24, 24 * 13 + 23), None);
        // Interleaved blocks: a permutation; bundle 1 goes to bundle C = 6 of 12.
        let mut seen: Vec<usize> = (0..24).map(|v| vrb_to_prb(24, true, v)).collect();
        assert_eq!(seen[2], 12);
        assert_eq!(seen[22], 22);
        seen.sort_unstable();
        assert_eq!(seen, (0..24).collect::<Vec<_>>());
    }

    #[test]
    fn pdsch_sizes() {
        // Row 1 of table A at position 2: type A, symbols 2 to 13, DM-RS at 2, 7 and 11: 144 - 36 elements per block.
        let d = Dci {
            riv: riv_encode(24, 0, 24),
            ..Default::default()
        };
        let p = Pdsch::from_dci(&d, 24, false).unwrap();
        assert_eq!((p.start, p.len, p.dmrs_symbols.clone(), p.cdm_groups), (2, 12, vec![2, 7, 11], 2));
        assert_eq!(p.re_per_prb(), 108);
        assert_eq!(p.data_res().len(), 108 * 24);
        // Position 3 with row 3 (index 2): symbols 3 to 10, 11 in all from the slot start: DM-RS at 3, 6 and 9.
        let p = Pdsch::from_dci(&Dci { tdra: 2, ..d }, 24, true).unwrap();
        assert_eq!((p.start, p.len, p.dmrs_symbols.clone()), (3, 8, vec![3, 6, 9]));
        // Type B of two symbols: one CDM group, data on the odd subcarriers of the DM-RS symbol.
        let p = Pdsch::from_dci(&Dci { tdra: 8, ..d }, 24, false).unwrap();
        assert_eq!((p.start, p.len, p.cdm_groups, p.re_per_prb()), (5, 2, 1, 18));
        assert_eq!(p.data_res().len(), 18 * 24);
        // Type B of seven symbols: DM-RS on its first and fifth symbols.
        let p = Pdsch::from_dci(&Dci { tdra: 7, ..d }, 24, false).unwrap();
        assert_eq!(p.dmrs_symbols, vec![5, 9]);
        // Transport block sizes by the steps of TS 38.214 5.1.3.2.
        // 108 x 24 elements, QPSK, rate 120/1024: N_info = 607.5, n = 3, 600, the next table value is 608.
        assert_eq!(tbs(108, 24, 2, 120), Some(608));
        // 108 x 24, rate 308/1024: N_info = 1559.25, n = 4, 1552, table 1608.
        assert_eq!(tbs(108, 24, 2, 308), Some(1608));
        // 18 x 4 elements at rate 120/1024: N_info = 16.9, n = 3, 16, at least 24.
        assert_eq!(tbs(18, 4, 2, 120), Some(24));
        // More than 3824 bits is not system information.
        assert_eq!(tbs(156, 96, 2, 679), None);
        assert_eq!(mcs_qpsk(9), Some((2, 679)));
        assert_eq!(mcs_qpsk(10), None);
    }
}
