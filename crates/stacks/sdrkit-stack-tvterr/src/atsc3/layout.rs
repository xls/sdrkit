//! The carriers of an ATSC 3.0 OFDM symbol: the number of carriers, the pilots of each symbol type with their values, the data
//! cells, the null cells of a subframe boundary symbol, and the frequency interleaver.
//!
//! Standard: ATSC A/322:2023-03 clauses 7.2.3, 7.2.6, 7.3 and 8.1, Annexes D, F and H.

use super::tables::{ADDITIONAL_CP, ADDITIONAL_CP_8K_SP32_4, CP32, CP_POWER, PREAMBLE, SP_BOOST};

/// The FFT size of a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FftSize {
    /// 8192 points.
    K8,
    /// 16384 points.
    K16,
    /// 32768 points.
    K32,
}

impl FftSize {
    /// The size of an L1 code (table 9.10), `None` for the reserved code.
    pub fn from_code(code: u8) -> Option<FftSize> {
        match code {
            0 => Some(FftSize::K8),
            1 => Some(FftSize::K16),
            2 => Some(FftSize::K32),
            _ => None,
        }
    }
    /// The L1 code.
    pub fn code(self) -> u8 {
        self as u8
    }
    /// The points of the transform.
    pub fn size(self) -> usize {
        8192 << self.code()
    }
    /// The carriers without carrier reduction (table 7.1): 6913, 13825, 27649.
    pub fn max_carriers(self) -> usize {
        (6912 << self.code()) + 1
    }
    /// The carriers that one step of carrier reduction removes: 96, 192, 384.
    pub fn reduction_unit(self) -> usize {
        96 << self.code()
    }
    /// The carriers at carrier reduction `cred` (0 to 4).
    pub fn carriers(self, cred: u8) -> usize {
        self.max_carriers() - usize::from(cred) * self.reduction_unit()
    }
}

/// The guard interval in samples of an L1 code 1 to 12 (table 9.11), `None` for a reserved code.
pub fn guard_samples(code: u8) -> Option<usize> {
    const G: [usize; 12] = [
        192, 384, 512, 768, 1024, 1536, 2048, 2432, 3072, 3648, 4096, 4864,
    ];
    G.get(usize::from(code).wrapping_sub(1)).copied()
}

/// The L1 code of a guard interval of `samples`, `None` when the standard has none of this length.
pub fn guard_code(samples: usize) -> Option<u8> {
    (1..=12).find(|&c| guard_samples(c) == Some(samples))
}

/// A scattered pilot pattern `SPa_b` (table 8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    code: u8,
}

impl Pattern {
    /// The pattern of an L1 code 0 to 15 (table 9.12), `None` for a reserved code.
    pub fn from_code(code: u8) -> Option<Pattern> {
        (code < 16).then_some(Pattern { code })
    }
    /// The pattern with these spacings, `None` when the standard has none.
    pub fn of(dx: usize, dy: usize) -> Option<Pattern> {
        (0..16)
            .map(|code| Pattern { code })
            .find(|p| p.dx() == dx && p.dy() == dy)
    }
    /// The L1 code.
    pub fn code(self) -> u8 {
        self.code
    }
    /// `D_X`: the spacing of the carriers that bear pilots.
    pub fn dx(self) -> usize {
        [3, 4, 6, 8, 12, 16, 24, 32][usize::from(self.code / 2)]
    }
    /// `D_Y`: the symbols of one pilot sequence.
    pub fn dy(self) -> usize {
        if self.code.is_multiple_of(2) {
            2
        } else {
            4
        }
    }
    /// The amplitude of the scattered pilots at boost code `boost` (0 to 4; table 9.14), `None` for a reserved code.
    pub fn amplitude(self, boost: u8) -> Option<f32> {
        SP_BOOST[usize::from(self.code)]
            .get(usize::from(boost))
            .map(|&db| amplitude(db))
    }
}

/// The amplitude of a power in hundredths of a decibel.
fn amplitude(centi_db: u16) -> f32 {
    10f64.powf(f64::from(centi_db) / 2000.0) as f32
}

/// What a bootstrap's preamble structure says of the Preamble (table H.1.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreambleStructure {
    /// The FFT size of the Preamble symbols.
    pub fft: FftSize,
    /// Their guard interval, samples.
    pub guard: usize,
    /// The spacing of the Preamble pilots.
    pub dx: usize,
    /// The L1-Basic mode, 1 to 5.
    pub l1_basic_mode: u8,
    /// The amplitude of the Preamble pilots (table 8.6).
    pub pilot_amplitude: f32,
}

impl PreambleStructure {
    /// The structure of code `code` (0 to 159), `None` for a reserved code.
    pub fn of(code: u8) -> Option<PreambleStructure> {
        let &(fft, guard, dx, power) = PREAMBLE.get(usize::from(code / 5))?;
        Some(PreambleStructure {
            fft: FftSize::from_code(fft)?,
            guard: usize::from(guard),
            dx: usize::from(dx),
            l1_basic_mode: code % 5 + 1,
            pilot_amplitude: amplitude(power),
        })
    }
    /// The code of the structure with this FFT size, guard, pilot spacing and L1-Basic mode (1 to 5), `None` when table
    /// H.1.1 has none.
    pub fn code_of(fft: FftSize, guard: usize, dx: usize, l1_basic_mode: u8) -> Option<u8> {
        if !(1..=5).contains(&l1_basic_mode) {
            return None;
        }
        let row = PREAMBLE.iter().position(|&(f, g, d, _)| {
            f == fft.code() && usize::from(g) == guard && usize::from(d) == dx
        })?;
        Some(row as u8 * 5 + l1_basic_mode - 1)
    }
}

/// The first `n` values `r_k` of the sequence that gives the pilots their signs (clause 8.1.2): the register of 1 + X^9 + X^10 + X^12 + X^13
/// loaded with 0000 0000 1101 1 at each symbol.
pub fn pilot_sequence(n: usize) -> Vec<u8> {
    let mut s = Vec::with_capacity(n.max(13));
    s.extend_from_slice(&[1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
    for i in 13..n {
        let v = s[i - 13] ^ s[i - 12] ^ s[i - 10] ^ s[i - 9];
        s.push(v);
    }
    s.truncate(n);
    s
}

/// The type of an OFDM symbol after the bootstrap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolKind {
    /// A Preamble symbol with pilots every `dx` carriers of amplitude `amplitude`.
    Preamble {
        /// The pilot spacing.
        dx: usize,
        /// The pilot amplitude.
        amplitude: f32,
    },
    /// A data symbol, number `l` of its subframe (the Preamble symbols count in the first subframe).
    Data {
        /// The scattered pilot pattern.
        pattern: Pattern,
        /// The amplitude of the scattered pilots.
        amplitude: f32,
        /// The symbol number.
        l: usize,
    },
    /// A subframe boundary symbol.
    Boundary {
        /// The scattered pilot pattern of the subframe.
        pattern: Pattern,
        /// The amplitude of the scattered pilots.
        amplitude: f32,
    },
}

/// The common continual pilots as relative carrier indices at carrier reduction `cred` (clause 8.1.4.1).
pub fn common_pilots(fft: FftSize, cred: u8) -> Vec<usize> {
    let step = 4 >> fft.code();
    let low = usize::from(cred) * fft.reduction_unit() / 2;
    let high = fft.max_carriers() - low;
    CP32.iter()
        .step_by(step)
        .map(|&k| usize::from(k).div_ceil(step))
        .filter(|&k| k >= low && k < high)
        .map(|k| k - low)
        .collect()
}

/// The additional continual pilots as relative carrier indices (tables D.1.4 and D.1.5).
fn additional_pilots(fft: FftSize, cred: u8, pattern: Pattern) -> Vec<usize> {
    if fft == FftSize::K8 && pattern.code() == 15 {
        return ADDITIONAL_CP_8K_SP32_4[usize::from(cred.min(4))]
            .iter()
            .map(|&k| usize::from(k))
            .collect();
    }
    ADDITIONAL_CP[usize::from(pattern.code())][usize::from(fft.code())]
        .iter()
        .filter(|&&(_, even_only)| !(even_only && cred % 2 == 1))
        .map(|&(k, _)| usize::from(k))
        .collect()
}

/// The carriers of one symbol: `pilot[k]` is the value of carrier `k` when it is a pilot (real, with the sign of the pilot
/// sequence), 0 when it carries a data cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Carriers {
    /// The FFT size.
    pub fft: FftSize,
    /// The pilot value of each of the carriers, 0 for a data cell.
    pub pilot: Vec<f32>,
}

impl Carriers {
    /// The carriers of a symbol of `kind` at FFT size `fft` and carrier reduction `cred` (0 to 4).
    pub fn new(fft: FftSize, cred: u8, kind: SymbolKind) -> Carriers {
        let noc = fft.carriers(cred);
        let mut amp = vec![0f32; noc];
        let cp = amplitude(CP_POWER);
        match kind {
            SymbolKind::Preamble { dx, amplitude } => {
                amp.iter_mut().step_by(dx).for_each(|a| *a = amplitude);
            }
            SymbolKind::Data {
                pattern,
                amplitude,
                l,
            } => {
                let period = pattern.dx() * pattern.dy();
                let first = pattern.dx() * (l % pattern.dy());
                amp.iter_mut()
                    .skip(first)
                    .step_by(period)
                    .for_each(|a| *a = amplitude);
                amp[0] = amplitude;
                amp[noc - 1] = amplitude;
                for k in additional_pilots(fft, cred, pattern) {
                    if k < noc {
                        amp[k] = amplitude;
                    }
                }
            }
            SymbolKind::Boundary { pattern, amplitude } => {
                amp.iter_mut()
                    .step_by(pattern.dx())
                    .for_each(|a| *a = amplitude);
                amp[0] = amplitude;
                amp[noc - 1] = amplitude;
                for k in additional_pilots(fft, cred, pattern) {
                    if k < noc {
                        amp[k] = amplitude;
                    }
                }
            }
        }
        for k in common_pilots(fft, cred) {
            amp[k] = cp;
        }
        let r = pilot_sequence(noc);
        for (a, &r) in amp.iter_mut().zip(&r) {
            if r == 1 {
                *a = -*a;
            }
        }
        Carriers { fft, pilot: amp }
    }
    /// The number of carriers.
    pub fn len(&self) -> usize {
        self.pilot.len()
    }
    /// Whether the symbol has no carrier (never).
    pub fn is_empty(&self) -> bool {
        self.pilot.is_empty()
    }
    /// The number of data cells (null cells included).
    pub fn data_cells(&self) -> usize {
        self.pilot.iter().filter(|&&p| p == 0.0).count()
    }
    /// The carriers of the data cells, lowest first.
    pub fn data_carriers(&self) -> impl Iterator<Item = usize> + '_ {
        self.pilot
            .iter()
            .enumerate()
            .filter(|(_, &p)| p == 0.0)
            .map(|(k, _)| k)
    }
    /// The power of the pilots, the sum of their squared values.
    pub fn pilot_power(&self) -> f64 {
        self.pilot
            .iter()
            .map(|&p| f64::from(p) * f64::from(p))
            .sum()
    }
    /// The FFT bin of carrier `k`: carrier `(len − 1) / 2` is at 0 Hz (clause 8.3).
    pub fn bin(&self, k: usize) -> usize {
        let n = self.fft.size();
        (k + n - (self.len() - 1) / 2) % n
    }
}

/// The active data cells of a subframe boundary symbol (Annex F): a boundary symbol has the power of a data symbol, so its
/// denser pilots cost data cells, which stay null. `data` are the data cells of a data symbol of the subframe, `boundary`
/// the carriers of the boundary symbol, `amplitude` the scattered pilot amplitude.
pub fn boundary_active_cells(
    data: usize,
    boundary: &Carriers,
    common: usize,
    amplitude: f32,
) -> usize {
    let noc = boundary.len();
    let total = boundary.data_cells();
    let pilots = noc - total - common;
    let a2 = f64::from(amplitude) * f64::from(amplitude);
    let extra = (pilots + data + common) as f64 - noc as f64;
    let active = (data as f64 - extra * a2).ceil();
    (active.max(0.0) as usize).min(total)
}

/// The interleaving sequence `H_l(p)` of symbol `l` of a subframe for `n_data` data cells (clause 7.3): `n_data` addresses
/// below `n_data`, each once.
pub fn frequency_interleaver(fft: FftSize, l: usize, n_data: usize) -> Vec<u32> {
    // Table 7.12 to 7.14: bit `i` of R' goes to bit `wire[i]` of R; index 0 is the row of even symbols.
    const W8: [[u8; 12]; 2] = [
        [7, 1, 4, 2, 9, 6, 8, 10, 0, 3, 11, 5],
        [11, 4, 9, 3, 1, 2, 5, 0, 6, 7, 10, 8],
    ];
    const W16: [[u8; 13]; 2] = [
        [9, 7, 6, 10, 12, 5, 1, 11, 0, 2, 3, 4, 8],
        [6, 8, 10, 12, 2, 0, 4, 1, 11, 3, 5, 9, 7],
    ];
    const W32: [u8; 14] = [7, 13, 3, 4, 9, 2, 12, 11, 1, 8, 10, 0, 5, 6];
    let nr = 13 + u32::from(fft.code());
    let (wire, r_taps, g_taps): (&[u8], &[u8], &[u8]) = match fft {
        FftSize::K8 => (&W8[l % 2], &[0, 1, 4, 6], &[0, 1, 4, 5, 9, 11]),
        FftSize::K16 => (&W16[l % 2], &[0, 1, 4, 5, 9, 11], &[0, 1, 2, 12]),
        FftSize::K32 => (&W32, &[0, 1, 2, 12], &[0, 1]),
    };
    let taps = |x: u32, t: &[u8]| t.iter().fold(0, |a, &b| a ^ (x >> b & 1));
    let mut g = (1u32 << nr) - 1;
    for _ in 0..l / 2 {
        g = g >> 1 | taps(g, g_taps) << (nr - 1);
    }
    let mut h = Vec::with_capacity(n_data);
    let mut rp = 0u32;
    for i in 0..1u32 << nr {
        rp = match i {
            0 | 1 => 0,
            2 => 1,
            _ => rp >> 1 | taps(rp, r_taps) << (nr - 2),
        };
        let r = wire
            .iter()
            .enumerate()
            .fold(0, |r, (b, &to)| r | (rp >> b & 1) << to);
        let a = ((i & 1) << (nr - 1) | r) ^ g;
        if (a as usize) < n_data {
            h.push(a);
        }
    }
    h
}

/// Frequency interleaves the data cells `x` of symbol `l` into `a` (clause 7.3). Both hold the data cells of the symbol.
pub fn interleave<T: Copy>(fft: FftSize, l: usize, x: &[T], a: &mut [T]) {
    let h = frequency_interleaver(fft, l, x.len());
    if fft == FftSize::K32 && l.is_multiple_of(2) {
        for (p, &hp) in h.iter().enumerate() {
            a[hp as usize] = x[p];
        }
    } else {
        for (p, &hp) in h.iter().enumerate() {
            a[p] = x[hp as usize];
        }
    }
}

/// Undoes [`interleave`]: the received data cells `a` of symbol `l` into `x`.
pub fn deinterleave<T: Copy>(fft: FftSize, l: usize, a: &[T], x: &mut [T]) {
    let h = frequency_interleaver(fft, l, a.len());
    if fft == FftSize::K32 && l.is_multiple_of(2) {
        for (p, &hp) in h.iter().enumerate() {
            x[p] = a[hp as usize];
        }
    } else {
        for (p, &hp) in h.iter().enumerate() {
            x[hp as usize] = a[p];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tables::printed::*;
    use super::*;

    fn fft_of(k: u8) -> FftSize {
        FftSize::from_code(match k {
            8 => 0,
            16 => 1,
            _ => 2,
        })
        .unwrap()
    }

    /// Clause 8.1.2: the first 24 values printed.
    #[test]
    fn the_pilot_sequence_starts_as_printed() {
        let want = [
            1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(pilot_sequence(24), want);
        assert_eq!(pilot_sequence(5), [1, 1, 0, 1, 1]);
    }

    /// Clause 8.1.4.1 and tables D.1.2, D.1.3, 8.4: the pilot sets of 16K and 8K derive from the set of 32K.
    #[test]
    fn the_common_pilots_of_16k_and_8k_derive_from_32k() {
        let all = |f| -> Vec<u16> { common_pilots(f, 0).iter().map(|&k| k as u16).collect() };
        assert_eq!(all(FftSize::K16), CP16);
        assert_eq!(all(FftSize::K8), CP8);
        assert_eq!(all(FftSize::K32), CP32);
        for (cred, counts) in [
            [48, 96, 192],
            [48, 96, 192],
            [47, 93, 186],
            [46, 92, 184],
            [45, 90, 180],
        ]
        .iter()
        .enumerate()
        {
            for (f, want) in [FftSize::K8, FftSize::K16, FftSize::K32].iter().zip(counts) {
                assert_eq!(common_pilots(*f, cred as u8).len(), *want, "{f:?} {cred}");
            }
        }
    }

    /// Table 7.1 and table H.1.1.
    #[test]
    fn carriers_and_preamble_structures_follow_the_tables() {
        assert_eq!(FftSize::K8.carriers(0), 6913);
        assert_eq!(FftSize::K8.carriers(4), 6529);
        assert_eq!(FftSize::K16.carriers(1), 13633);
        assert_eq!(FftSize::K32.carriers(4), 26113);
        let s = |c| PreambleStructure::of(c).unwrap();
        let facts = |c| {
            let p = s(c);
            (p.fft, p.guard, p.dx, p.l1_basic_mode)
        };
        assert_eq!(facts(0), (FftSize::K8, 192, 16, 1));
        assert_eq!(facts(3), (FftSize::K8, 192, 16, 4));
        assert_eq!(facts(9), (FftSize::K8, 384, 8, 5));
        assert_eq!(facts(34), (FftSize::K8, 2048, 3, 5));
        assert_eq!(facts(35), (FftSize::K16, 192, 32, 1));
        assert_eq!(facts(89), (FftSize::K16, 4096, 3, 5));
        assert_eq!(facts(90), (FftSize::K32, 192, 32, 1));
        assert_eq!(
            (s(135).fft, s(135).guard, s(135).dx),
            (FftSize::K32, 3072, 3)
        );
        assert_eq!(facts(159), (FftSize::K32, 4864, 3, 5));
        assert!(PreambleStructure::of(160).is_none());
        assert!((s(3).pilot_amplitude - 1.841).abs() < 1e-3);
        for c in 0..160 {
            let p = s(c);
            // The same FFT size, guard and spacing can stand twice in the table (32K, 3072 samples); the first is found.
            let back = PreambleStructure::code_of(p.fft, p.guard, p.dx, p.l1_basic_mode).unwrap();
            assert_eq!(facts(back), facts(c));
        }
        assert_eq!(guard_samples(5), Some(1024));
        assert_eq!(guard_samples(0), None);
        assert_eq!(guard_samples(13), None);
        assert_eq!(guard_code(4864), Some(12));
        assert_eq!(Pattern::of(6, 2).unwrap().code(), 4);
        assert!((Pattern::of(6, 2).unwrap().amplitude(3).unwrap() - 1.585).abs() < 1e-3);
        assert!(Pattern::from_code(16).is_none());
        assert!(Pattern::from_code(0).unwrap().amplitude(5).is_none());
    }

    /// Table 7.2: the data cells of a Preamble symbol for each row and carrier reduction.
    #[test]
    fn preamble_cell_counts_follow_table_7_2() {
        for &(k, guard, dx, cells) in PREAMBLE_CELLS.iter() {
            for (cred, &want) in cells.iter().enumerate() {
                let c = Carriers::new(
                    fft_of(k),
                    cred as u8,
                    SymbolKind::Preamble {
                        dx: usize::from(dx),
                        amplitude: 1.5,
                    },
                );
                assert_eq!(
                    c.data_cells(),
                    usize::from(want),
                    "{k}K {guard} {dx} {cred}"
                );
            }
        }
    }

    /// Tables 7.3 to 7.6: the data cells of every symbol of a data pattern and of a boundary symbol. Annex F: the active cells.
    #[test]
    fn data_and_boundary_cell_counts_follow_tables_7_3_to_7_6_and_annex_f() {
        for (&(k, cred, noc, cells, allowed), &(_, _, _, boundary)) in
            DATA_CELLS.iter().zip(BOUNDARY_CELLS.iter())
        {
            let fft = fft_of(k);
            assert_eq!(fft.carriers(cred), usize::from(noc));
            let common = common_pilots(fft, cred).len();
            for code in 0..16u8 {
                let want = usize::from(cells[usize::from(code)]);
                if want == 0 {
                    continue;
                }
                let pattern = Pattern::from_code(code).unwrap();
                // The standard prints counts for combinations that it does not allow; some of those have no additional
                // pilots in table D.1.4 and vary from symbol to symbol.
                if allowed[usize::from(code)] {
                    for l in 0..pattern.dy() {
                        let c = Carriers::new(
                            fft,
                            cred,
                            SymbolKind::Data {
                                pattern,
                                amplitude: 1.2,
                                l,
                            },
                        );
                        assert_eq!(c.data_cells(), want, "{k}K {cred} code {code} l {l}");
                    }
                }
                let b = Carriers::new(
                    fft,
                    cred,
                    SymbolKind::Boundary {
                        pattern,
                        amplitude: 1.2,
                    },
                );
                let total = usize::from(boundary[usize::from(code)]);
                assert_eq!(b.data_cells(), total, "{k}K {cred} code {code} boundary");
                for &(fk, fc, boost, active) in ACTIVE_CELLS.iter() {
                    if fk == k && fc == cred {
                        let a = pattern.amplitude(boost).unwrap();
                        assert_eq!(
                            boundary_active_cells(want, &b, common, a),
                            usize::from(active[usize::from(code)]),
                            "{k}K {cred} code {code} boost {boost}"
                        );
                    }
                }
            }
        }
    }

    /// Clause 7.3: the sequence holds each address once, differs from symbol to symbol, and the interleaver goes there and back.
    #[test]
    fn the_frequency_interleaver_is_a_permutation_per_symbol() {
        for (fft, n) in [
            (FftSize::K8, 6075),
            (FftSize::K8, 6647),
            (FftSize::K16, 13295),
            (FftSize::K32, 27023),
        ] {
            let mut last = Vec::new();
            for l in 0..5 {
                let h = frequency_interleaver(fft, l, n);
                assert_eq!(h.len(), n, "{fft:?} {l}");
                let mut seen = vec![false; n];
                h.iter().for_each(|&a| seen[a as usize] = true);
                assert!(seen.iter().all(|&s| s));
                if fft != FftSize::K32 || l % 2 == 0 {
                    assert_ne!(h, last, "{fft:?} {l}");
                } else {
                    assert_eq!(h, last, "{fft:?} {l}");
                }
                last = h;
                let x: Vec<u32> = (0..n as u32).collect();
                let mut a = vec![0u32; n];
                interleave(fft, l, &x, &mut a);
                assert_ne!(a, x);
                let mut back = vec![0u32; n];
                deinterleave(fft, l, &a, &mut back);
                assert_eq!(back, x);
            }
        }
        // 8K, symbol 0: i = 0 and 1 give R = 0, so the first addresses are G and G with its top bit turned: 0x1FFF is above
        // the data cells and 0x0FFF is the first address. i = 2 gives R' = 1, which table 7.12 takes to bit 7 of R: 0x1F7F
        // is above the data cells. i = 3 gives R' = 0x800, which goes to bit 5 of R.
        let h = frequency_interleaver(FftSize::K8, 0, 6075);
        assert_eq!(h[0], 0x0FFF);
        assert_eq!(h[1], 0x0FFF ^ 0x20);
    }

    /// Clause 8.3: the centre carrier is at 0 Hz.
    #[test]
    fn the_centre_carrier_is_at_bin_zero() {
        let c = Carriers::new(
            FftSize::K8,
            0,
            SymbolKind::Preamble {
                dx: 16,
                amplitude: 1.0,
            },
        );
        assert_eq!(c.bin(3456), 0);
        assert_eq!(c.bin(0), 8192 - 3456);
        assert_eq!(c.bin(6912), 3456);
    }
}
