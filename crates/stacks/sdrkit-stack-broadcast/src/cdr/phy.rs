//! The CDR physical layer: transmission modes, spectrum modes, half sub-bands, the beacon, the scattered pilots, the system
//! information and the frame layout. The carrier lists, the beacon sequence and the pilot pattern are sdrkit's own, because no
//! free document specifies them.
//!
//! Standard: GY/T 268.1-2013 table 4.3 (FFT sizes, guard intervals, symbols per sub-frame, carriers per half sub-band).

use super::tables::LdpcRate;
use crate::qam::Qam;
use sdrkit_algo_modem::ofdm::FrameLayout;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC6_CDR;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use sdrkit_util_bits::seq::ZadoffChu;

/// The sample rate.
pub const RATE: f64 = 816_000.0;
/// Samples per sub-frame.
pub const SUBFRAME: usize = 130_560;

/// A transmission mode 1..3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TxMode(pub u8);

impl TxMode {
    /// FFT size.
    pub fn nfft(self) -> usize {
        [2048, 1024, 2048][self.0 as usize - 1]
    }
    /// Guard of a data symbol.
    pub fn cp(self) -> usize {
        [240, 140, 56][self.0 as usize - 1]
    }
    /// Beacon guard.
    pub fn beacon_cp(self) -> usize {
        [384, 332, 168][self.0 as usize - 1]
    }
    /// Data symbols per sub-frame.
    pub fn sn(self) -> usize {
        [56, 111, 61][self.0 as usize - 1]
    }
    /// Carriers per half sub-band.
    pub fn half(self) -> i32 {
        if self.0 == 2 {
            61
        } else {
            121
        }
    }
    /// Carrier spacing.
    pub fn spacing(self) -> f64 {
        RATE / self.nfft() as f64
    }
    /// MSDS cells per sub-band and logical frame.
    pub fn msds_cells(self) -> usize {
        [46_080, 46_080, 50_688][self.0 as usize - 1]
    }
    /// SDIS cells per sub-band and logical frame.
    pub fn sdis_cells(self) -> usize {
        [1704, 1576, 1360][self.0 as usize - 1]
    }
    /// SDI payload Q per constellation.
    pub fn sdi_q(self, q: Qam) -> usize {
        let t = [[846, 1698, 2550], [782, 1570, 2358], [674, 1354, 2034]];
        t[self.0 as usize - 1][q.bits() / 2 - 1]
    }
}

/// A spectrum mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SpectrumMode(pub u8);

impl SpectrumMode {
    /// All.
    pub const ALL: [SpectrumMode; 6] = [
        SpectrumMode(1),
        SpectrumMode(2),
        SpectrumMode(9),
        SpectrumMode(10),
        SpectrumMode(22),
        SpectrumMode(23),
    ];
    /// Sub-bands (NI).
    pub fn ni(self) -> usize {
        match self.0 {
            2 | 10 | 23 => 2,
            _ => 1,
        }
    }
    /// The half sub-bands: (centre kHz, lower half (else upper)).
    pub fn halves(self) -> Vec<(f64, bool)> {
        match self.0 {
            1 => vec![(0.0, true), (0.0, false)],
            2 => vec![(-50.0, true), (-50.0, false), (50.0, true), (50.0, false)],
            9 => vec![(-150.0, true), (150.0, false)],
            10 => vec![
                (-200.0, true),
                (-200.0, false),
                (200.0, true),
                (200.0, false),
            ],
            22 => vec![(-100.0, true), (100.0, false)],
            _ => vec![
                (-150.0, true),
                (-150.0, false),
                (150.0, true),
                (150.0, false),
            ],
        }
    }
    /// Nominal frequency code (0, 50, 100, 150, 200 kHz).
    pub fn nominal_code(self) -> u8 {
        match self.0 {
            1 => 0,
            2 => 1,
            9 => 3,
            10 => 4,
            22 => 2,
            _ => 3,
        }
    }
    /// Beside an analogue FM programme.
    pub fn hybrid(self) -> bool {
        self.0 >= 9
    }
    /// The text.
    pub fn text(self) -> &'static str {
        match self.0 {
            1 => "100 kHz, all digital",
            2 => "200 kHz, all digital",
            9 => "2 × 50 kHz beside stereo FM",
            10 => "2 × 100 kHz beside stereo FM",
            22 => "2 × 50 kHz beside mono FM",
            _ => "2 × 100 kHz beside mono FM",
        }
    }
    /// From the number.
    pub fn from_code(c: u8) -> Option<SpectrumMode> {
        Self::ALL.iter().copied().find(|m| m.0 == c)
    }
}

/// The carriers of a half sub-band.
pub fn half_carriers(t: TxMode, centre_khz: f64, lower: bool) -> std::ops::RangeInclusive<i32> {
    let c = (centre_khz * 1000.0 / t.spacing()).round() as i32;
    let h = t.half();
    if lower {
        c - h..=c - 1
    } else {
        c + 1..=c + h
    }
}

/// The union of every spectrum mode's carriers.
pub fn union(t: TxMode) -> (i32, i32) {
    let c = (200_000.0 / t.spacing()).round() as i32;
    (-(c + t.half()), c + t.half())
}

fn unit(seed: u64) -> Complex32 {
    let mut z = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z ^= z >> 31;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    let q = (z & 3) as f32;
    let ph = std::f32::consts::FRAC_PI_4 + q * std::f32::consts::FRAC_PI_2;
    Complex32::new(ph.cos(), ph.sin())
}

/// The beacon value of carrier `k` (even carriers): a Zadoff-Chu sequence over the beacon carriers.
pub fn beacon(t: TxMode, k: i32) -> Complex32 {
    let i = (t.0 as usize).saturating_sub(1).min(2);
    let (nzc, m) = ([967u32, 487, 967][i], [48u32, 12, 48][i]);
    let Ok(z) = ZadoffChu::new(m, nzc) else {
        return Complex32::new(0.0, 0.0);
    };
    let [re, im] = z.value((k / 2).rem_euclid(nzc as i32) as u64);
    Complex32::new(re as f32, im as f32) * std::f32::consts::SQRT_2
}

/// Whether (s ≥ 1, k) is a scattered pilot: every 12th carrier, moving by 4 each symbol.
pub fn is_pilot(s: usize, k: i32) -> bool {
    s >= 1 && (k + 4 * ((s as i32 - 1) % 3)).rem_euclid(12) == 0
}

/// A scattered pilot's value.
pub fn pilot(s: usize, k: i32) -> Complex32 {
    unit((s as u64) << 20 ^ (k as i64 as u64)) * std::f32::consts::SQRT_2
}

/// Whether carrier `k` lies in any half sub-band of the union.
pub fn in_some_half(t: TxMode, k: i32) -> bool {
    SpectrumMode::ALL
        .iter()
        .flat_map(|m| m.halves())
        .any(|(c, l)| half_carriers(t, c, l).contains(&k))
}

/// The modem layout of a transmission mode over the union; `inner` searches the all-digital band (|f| < 104 kHz), else the band
/// beside an analogue programme (96 to 254 kHz).
pub fn layout(t: TxMode, inner: bool) -> FrameLayout {
    let (lo, hi) = union(t);
    let k = (hi - lo + 1) as usize;
    let ns = t.sn() + 1;
    let mut known = vec![Vec::new(); ns];
    for (s, row) in known.iter_mut().enumerate() {
        for c in 0..k {
            let kk = lo + c as i32;
            if !in_some_half(t, kk) {
                continue;
            }
            if s == 0 {
                if kk % 2 == 0 {
                    row.push((c, beacon(t, kk)));
                }
            } else if is_pilot(s, kk) {
                row.push((c, pilot(s, kk)));
            }
        }
    }
    let null = (0..k)
        .filter(|&c| !in_some_half(t, lo + c as i32))
        .collect();
    let taps: Vec<Complex32> = if inner {
        sdrkit_algo_dsp::fir::lowpass(RATE, 100e3, 112e3, 50.0)
            .expect("the filter settings are valid")
            .iter()
            .map(|&v| Complex32::new(v, 0.0))
            .collect()
    } else {
        let mut t = sdrkit_algo_dsp::fir::bandpass_complex(RATE, 96e3, 254e3, 8e3, 50.0)
            .expect("the filter settings are valid");
        let l = sdrkit_algo_dsp::fir::bandpass_complex(RATE, -254e3, -96e3, 8e3, 50.0)
            .expect("the filter settings are valid");
        for (a, b) in t.iter_mut().zip(&l) {
            *a += *b;
        }
        t
    };
    FrameLayout {
        rate: RATE,
        nfft: t.nfft(),
        cp: t.cp(),
        cp0: t.beacon_cp(),
        nsym: ns,
        kmin: lo,
        kmax: hi,
        known,
        null,
        max_shift: 24,
        adv: t.cp() / 2,
        sync_threshold: 0.4,
        guard_threshold: 0.2,
        search_taps: Some(taps),
        pair_band: Some(if inner { (0.0, 104e3) } else { (96e3, 254e3) }),
    }
}

/// The system information.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SystemInfo {
    /// Multi-frequency cooperation (0 = cooperating).
    pub mf: bool,
    /// Next frequency (all ones when not).
    pub next_freq: u16,
    /// Nominal frequency code.
    pub nominal: u8,
    /// Spectrum mode.
    pub spectrum: u8,
    /// Physical frame in the super frame.
    pub frame: u8,
    /// Sub-frame in the frame.
    pub subframe: u8,
    /// Allocation mode 1..3.
    pub allocation: u8,
    /// SDI constellation (0 QPSK, 1 16-QAM, 2 64-QAM).
    pub sdi_qam: u8,
    /// Service data constellation.
    pub data_qam: u8,
    /// Hierarchy.
    pub hierarchy: u8,
    /// Uniform.
    pub uniform: bool,
    /// High LDPC rate index.
    pub rate_hi: u8,
    /// Low LDPC rate index.
    pub rate_lo: u8,
}

impl SystemInfo {
    /// The 48 bits.
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(48);
        push_u64(&mut b, self.mf as u64, 1);
        push_u64(&mut b, self.next_freq as u64, 9);
        push_u64(&mut b, self.nominal as u64, 3);
        push_u64(&mut b, self.spectrum as u64, 6);
        push_u64(&mut b, self.frame as u64, 2);
        push_u64(&mut b, self.subframe as u64, 2);
        push_u64(&mut b, self.allocation as u64, 2);
        push_u64(&mut b, self.sdi_qam as u64, 2);
        push_u64(&mut b, self.data_qam as u64, 2);
        push_u64(&mut b, self.hierarchy as u64, 2);
        push_u64(&mut b, self.uniform as u64, 1);
        push_u64(&mut b, self.rate_hi as u64, 2);
        push_u64(&mut b, self.rate_lo as u64, 2);
        push_u64(&mut b, 0, 6);
        let crc = CRC6_CDR.bits(&b);
        push_u64(&mut b, crc as u64, 6);
        b
    }
    /// Reads the 48 bits; `None` when the CRC fails or a value is reserved.
    pub fn from_bits(b: &[u8]) -> Option<SystemInfo> {
        if b.len() < 48 || CRC6_CDR.bits(&b[..42]) as u64 != to_u64(&b[42..48]) {
            return None;
        }
        let si = SystemInfo {
            mf: b[0] == 1,
            next_freq: to_u64(&b[1..10]) as u16,
            nominal: to_u64(&b[10..13]) as u8,
            spectrum: to_u64(&b[13..19]) as u8,
            frame: to_u64(&b[19..21]) as u8,
            subframe: to_u64(&b[21..23]) as u8,
            allocation: to_u64(&b[23..25]) as u8,
            sdi_qam: to_u64(&b[25..27]) as u8,
            data_qam: to_u64(&b[27..29]) as u8,
            hierarchy: to_u64(&b[29..31]) as u8,
            uniform: b[31] == 1,
            rate_hi: to_u64(&b[32..34]) as u8,
            rate_lo: to_u64(&b[34..36]) as u8,
        };
        if si.allocation == 0
            || si.sdi_qam > 2
            || si.data_qam > 2
            || SpectrumMode::from_code(si.spectrum).is_none()
        {
            return None;
        }
        Some(si)
    }
    /// The data constellation.
    pub fn data(&self) -> Qam {
        [Qam::Q4, Qam::Q16, Qam::Q64][self.data_qam as usize % 3]
    }
    /// The SDI constellation.
    pub fn sdi(&self) -> Qam {
        [Qam::Q4, Qam::Q16, Qam::Q64][self.sdi_qam as usize % 3]
    }
    /// The LDPC rate.
    pub fn ldpc(&self) -> LdpcRate {
        LdpcRate::from_index(self.rate_hi as u32).unwrap_or(LdpcRate::R1_2)
    }
}

/// Where the cells of one sub-frame are (indices into the union grid).
#[derive(Clone, Debug)]
pub struct SubframeCells {
    /// SI copies: per half sub-band, 108 cells.
    pub si: Vec<Vec<usize>>,
    /// Data cells (SDIS then MSDS take them in order across the logical frame).
    pub data: Vec<usize>,
}

/// The cells of a sub-frame for a transmission and spectrum mode.
pub fn subframe_cells(t: TxMode, m: SpectrumMode) -> SubframeCells {
    let (lo, hi) = union(t);
    let k = (hi - lo + 1) as usize;
    let mut si = Vec::new();
    let mut taken = std::collections::BTreeSet::new();
    for (c, l) in m.halves() {
        let mut v = Vec::with_capacity(108);
        'outer: for s in 1..=t.sn() {
            for kk in half_carriers(t, c, l) {
                if !is_pilot(s, kk) {
                    let i = s * k + (kk - lo) as usize;
                    v.push(i);
                    taken.insert(i);
                    if v.len() == 108 {
                        break 'outer;
                    }
                }
            }
        }
        si.push(v);
    }
    let mut data = Vec::new();
    for s in 1..=t.sn() {
        for (c, l) in m.halves() {
            for kk in half_carriers(t, c, l) {
                let i = s * k + (kk - lo) as usize;
                if !is_pilot(s, kk) && !taken.contains(&i) {
                    data.push(i);
                }
            }
        }
    }
    SubframeCells { si, data }
}
