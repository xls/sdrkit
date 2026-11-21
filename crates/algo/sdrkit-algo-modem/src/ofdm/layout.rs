//! The description of an OFDM signal as data: FFT size, cyclic prefix, used carriers, nulls, reserved carriers and known pilot cells.

use crate::{Complex32, ModemError, Param};

/// One known cell of a symbol: its index among the used carriers and its transmitted value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pilot {
    /// Index into the used carriers (`0` is the carrier [`OfdmLayout::first`]).
    pub index: u32,
    /// The value sent there.
    pub value: Complex32,
}

/// The known cells of each symbol of a repeating pattern: symbol `l` uses `cells[l % period]`. Carriers are signed indices from DC
/// (carrier 0 is the centre frequency).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PilotPattern {
    /// Per symbol of the period: `(carrier, value)` of every known cell.
    pub cells: Vec<Vec<(i32, Complex32)>>,
}

impl PilotPattern {
    /// No pilots.
    pub fn none() -> Self {
        PilotPattern {
            cells: vec![Vec::new()],
        }
    }

    /// The same cells in every symbol (continual pilots).
    pub fn continual(cells: &[(i32, Complex32)]) -> Self {
        PilotPattern {
            cells: vec![cells.to_vec()],
        }
    }

    /// Number of symbols before the pattern repeats.
    pub fn period(&self) -> usize {
        self.cells.len().max(1)
    }
}

/// An OFDM layout: everything a demodulator needs to know about the grid, as data (module documentation of [`crate::ofdm`]).
///
/// The used carriers are the contiguous signed range `first..=last`; within it, `nulls` carry nothing (DC, guard holes) and
/// `reserved` carry something that is neither data nor a pilot (DVB-T TPS). Pilots come from a [`PilotPattern`]. The per-symbol
/// tables of pilot and data cells are built once in [`OfdmLayout::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct OfdmLayout {
    nfft: usize,
    cp: Vec<usize>,
    first: i32,
    last: i32,
    nulls: Vec<i32>,
    reserved: Vec<i32>,
    pilots: Vec<Vec<Pilot>>,
    data: Vec<Vec<u32>>,
}

impl OfdmLayout {
    /// A layout with `nfft` points, cyclic prefixes `cp` (one per symbol of a repeating cycle, at least one entry, each below `nfft`),
    /// used carriers `first..=last`, and the given nulls, reserved carriers and pilots. Errors when the carriers do not fit the FFT
    /// (`last - first < nfft`), a listed carrier is outside the used range, or a pilot is on a null.
    pub fn new(
        nfft: usize,
        cp: &[usize],
        first: i32,
        last: i32,
        nulls: &[i32],
        reserved: &[i32],
        pilots: &PilotPattern,
    ) -> Result<Self, ModemError> {
        if nfft < 2 {
            return Param::FftSize.at_least(2.0, nfft as f64);
        }
        if cp.is_empty() {
            return Param::Prefix.at_least(1.0, 0.0);
        }
        if let Some(&c) = cp.iter().find(|&&c| c >= nfft) {
            return Param::Prefix.range(0.0, (nfft - 1) as f64, c as f64);
        }
        if first > last || (last - first) as usize >= nfft {
            return Param::Carriers.conflict(Param::FftSize);
        }
        let n_used = (last - first + 1) as usize;
        let inside = |c: i32| c >= first && c <= last;
        if nulls.iter().chain(reserved).any(|&c| !inside(c)) {
            return Param::Nulls.conflict(Param::Carriers);
        }
        let mut class = vec![0u8; n_used]; // 0 data, 1 null, 2 reserved
        for &c in nulls {
            class[(c - first) as usize] = 1;
        }
        for &c in reserved {
            class[(c - first) as usize] = 2;
        }
        let mut pil = Vec::with_capacity(pilots.period());
        let mut data = Vec::with_capacity(pilots.period());
        let empty = Vec::new();
        for l in 0..pilots.period() {
            let cells = pilots.cells.get(l).unwrap_or(&empty);
            let mut is_pilot = vec![false; n_used];
            let mut p = Vec::with_capacity(cells.len());
            for &(c, v) in cells {
                if !inside(c) || class[(c - first) as usize] == 1 {
                    return Param::Pilots.conflict(Param::Carriers);
                }
                if !(v.re.is_finite() && v.im.is_finite()) || v.norm_sqr() == 0.0 {
                    return Param::Pilots.degenerate();
                }
                let i = (c - first) as usize;
                if !is_pilot[i] {
                    is_pilot[i] = true;
                    p.push(Pilot {
                        index: i as u32,
                        value: v,
                    });
                }
            }
            p.sort_by_key(|q| q.index);
            pil.push(p);
            data.push(
                (0..n_used as u32)
                    .filter(|&i| class[i as usize] == 0 && !is_pilot[i as usize])
                    .collect(),
            );
        }
        Ok(OfdmLayout {
            nfft,
            cp: cp.to_vec(),
            first,
            last,
            nulls: nulls.to_vec(),
            reserved: reserved.to_vec(),
            pilots: pil,
            data,
        })
    }

    /// FFT size.
    pub fn nfft(&self) -> usize {
        self.nfft
    }

    /// The cyclic prefix of symbol `l` (the prefixes repeat with their own cycle).
    pub fn cp(&self, l: u64) -> usize {
        self.cp[(l % self.cp.len() as u64) as usize]
    }

    /// The samples of symbol `l`, prefix included.
    pub fn symbol_len(&self, l: u64) -> usize {
        self.nfft + self.cp(l)
    }

    /// The samples of the longest symbol, prefix included.
    pub fn longest_symbol(&self) -> usize {
        self.nfft + self.cp.iter().copied().max().unwrap_or(0)
    }

    /// The lowest used carrier (signed, from DC).
    pub fn first(&self) -> i32 {
        self.first
    }

    /// The highest used carrier.
    pub fn last(&self) -> i32 {
        self.last
    }

    /// Number of used carriers (`last - first + 1`, nulls included).
    pub fn used(&self) -> usize {
        (self.last - self.first + 1) as usize
    }

    /// The carrier (signed, from DC) of used index `i`.
    pub fn carrier(&self, i: usize) -> i32 {
        self.first + i as i32
    }

    /// The FFT bin of used index `i` (natural FFT order: negative carriers wrap to the top).
    pub fn bin(&self, i: usize) -> usize {
        (self.carrier(i)).rem_euclid(self.nfft as i32) as usize
    }

    /// The null carriers.
    pub fn nulls(&self) -> &[i32] {
        &self.nulls
    }

    /// The reserved carriers.
    pub fn reserved(&self) -> &[i32] {
        &self.reserved
    }

    /// Symbols before the pilot pattern repeats.
    pub fn period(&self) -> usize {
        self.pilots.len()
    }

    /// The pilots of symbol `l`, in carrier order.
    pub fn pilots(&self, l: u64) -> &[Pilot] {
        &self.pilots[(l % self.pilots.len() as u64) as usize]
    }

    /// The used indices of the data cells of symbol `l`, in carrier order.
    pub fn data(&self, l: u64) -> &[u32] {
        &self.data[(l % self.data.len() as u64) as usize]
    }

    /// Copies the data cells of symbol `l` out of its used cells (`cells.len() == used()`) into `out`, which must hold
    /// `data(l).len()` values; returns how many were written.
    pub fn take_data(
        &self,
        l: u64,
        cells: &[Complex32],
        out: &mut [Complex32],
    ) -> Result<usize, ModemError> {
        let d = self.data(l);
        if cells.len() < self.used() {
            return Err(ModemError::InputTooShort { need: self.used() });
        }
        if out.len() < d.len() {
            return Err(ModemError::OutputTooShort { need: d.len() });
        }
        for (o, &i) in out.iter_mut().zip(d) {
            *o = cells[i as usize];
        }
        Ok(d.len())
    }

    /// The IEEE 802.11a/g non-HT layout (IEEE Std 802.11-2016 clause 17.3.5.10): 64 points, prefix 16, carriers -26..=26 with DC
    /// null, pilots on -21, -7, 7, 21 with values (1, 1, 1, -1) times the polarity `p_l` of [`ieee80211_pilot_polarity`] (symbol 0
    /// is SIGNAL and takes `p_0`). The pattern has a period of 127 symbols.
    pub fn ieee80211a() -> Self {
        let pol = ieee80211_pilot_polarity();
        let base = [(-21, 1.0f32), (-7, 1.0), (7, 1.0), (21, -1.0)];
        let cells = pol
            .iter()
            .map(|&p| {
                base.iter()
                    .map(|&(c, v)| (c, Complex32::new(v * p as f32, 0.0)))
                    .collect()
            })
            .collect();
        OfdmLayout::new(64, &[16], -26, 26, &[0], &[], &PilotPattern { cells })
            .unwrap_or_else(|_| unreachable!("the 802.11a table is valid"))
    }

    /// The names [`by_name`](Self::by_name) accepts: `ieee80211a` ([`OfdmLayout::ieee80211a`]), `dvbt_2k`
    /// ([`OfdmLayout::dvbt_2k`]) and `dvbt_8k` ([`OfdmLayout::dvbt_8k`]).
    pub const NAMES: [&'static str; 3] = ["ieee80211a", "dvbt_2k", "dvbt_8k"];

    /// The layout called `name` (one of [`NAMES`](Self::NAMES)); `guard` is the DVB-T guard interval divisor (4, 8, 16 or 32,
    /// the prefix being `nfft / guard`) and is not used by layouts with a fixed prefix. Errors for an unknown name or a bad guard.
    ///
    /// ```
    /// use sdrkit_algo_modem::ofdm::OfdmLayout;
    /// assert_eq!(OfdmLayout::by_name("dvbt_2k", 8).unwrap().symbol_len(0), 2048 + 256);
    /// assert_eq!(OfdmLayout::by_name("ieee80211a", 4).unwrap(), OfdmLayout::ieee80211a());
    /// assert_eq!(OfdmLayout::by_name("dvbt_8k", 4).unwrap().symbol_len(0), 8192 + 2048);
    /// assert!(OfdmLayout::by_name("dvbt_4k", 4).is_err());
    /// ```
    pub fn by_name(name: &str, guard: usize) -> Result<Self, ModemError> {
        match name {
            "ieee80211a" => Ok(OfdmLayout::ieee80211a()),
            "dvbt_2k" => OfdmLayout::dvbt_2k(guard),
            "dvbt_8k" => OfdmLayout::dvbt_8k(guard),
            _ => Param::Layout.unknown_name(),
        }
    }

    /// The DVB-T 2K layout (ETSI EN 300 744 clause 4.4 and 4.5): 2048 points, carriers `k = 0 ..= 1704` placed at `k - 852` from DC,
    /// prefix `2048 / guard` (`guard` = 4, 8, 16 or 32); scattered pilots at `k = 3 (l mod 4) + 12 p` (clause 4.5.3), the 45
    /// continual pilots of table 7, all at `4/3 * 2 (1/2 - w_k)` with `w_k` the pilot PRBS (clause 4.5.2,
    /// `sdrkit_util_bits::seq::DVBT_PILOTS`); the 17 TPS carriers of table 8 are reserved. Errors for another guard.
    pub fn dvbt_2k(guard: usize) -> Result<Self, ModemError> {
        Self::dvbt(2048, 1704, &DVBT_2K_CONTINUAL, &DVBT_2K_TPS, guard)
    }

    /// The DVB-T 8K layout (ETSI EN 300 744 clause 4.4 and 4.5): 8192 points, carriers `k = 0 ..= 6816` placed at `k - 3408` from
    /// DC, prefix `8192 / guard` (`guard` = 4, 8, 16 or 32). Scattered pilots at `k = 3 (l mod 4) + 12 p` (clause 4.5.3) and the 177
    /// continual pilots of table 7 are all at `4/3 * 2 (1/2 - w_k)`, with `w_k` the pilot PRBS (clause 4.5.2). The 68 TPS carriers of
    /// table 8 are reserved. Every symbol then has 6048 data cells (clause 4.5.1). Errors for another guard.
    ///
    /// ```
    /// use sdrkit_algo_modem::ofdm::OfdmLayout;
    /// let l = OfdmLayout::dvbt_8k(32).unwrap();
    /// assert_eq!((l.nfft(), l.cp(0), l.used()), (8192, 256, 6817));
    /// assert_eq!(l.data(0).len(), 6048);
    /// ```
    pub fn dvbt_8k(guard: usize) -> Result<Self, ModemError> {
        Self::dvbt(8192, 6816, &DVBT_8K_CONTINUAL, &DVBT_8K_TPS, guard)
    }

    /// A DVB-T layout of `nfft` points and carriers `0 ..= kmax` (EN 300 744 clause 4.5): scattered pilots, the continual pilots
    /// `continual`, all at the boosted value from the pilot PRBS, and the TPS carriers `tps` reserved.
    fn dvbt(
        nfft: usize,
        kmax: i32,
        continual: &[u16],
        tps: &[u16],
        guard: usize,
    ) -> Result<Self, ModemError> {
        if !matches!(guard, 4 | 8 | 16 | 32) {
            return Param::Guard.not_allowed(guard as f64);
        }
        let centre = kmax / 2;
        let mut prbs = sdrkit_util_bits::seq::DVBT_PILOTS;
        let w: Vec<f32> = (0..=kmax)
            .map(|_| 4.0 / 3.0 * 2.0 * (0.5 - prbs.bit() as f32))
            .collect();
        let cells = (0..4)
            .map(|l| {
                let mut v: Vec<(i32, Complex32)> = (0..=kmax)
                    .filter(|k| k % 12 == 3 * l)
                    .chain(continual.iter().map(|&k| k as i32))
                    .map(|k| (k - centre, Complex32::new(w[k as usize], 0.0)))
                    .collect();
                v.sort_by_key(|c| c.0);
                v.dedup_by_key(|c| c.0);
                v
            })
            .collect();
        let tps: Vec<i32> = tps.iter().map(|&k| k as i32 - centre).collect();
        OfdmLayout::new(
            nfft,
            &[nfft / guard],
            -centre,
            centre,
            &[],
            &tps,
            &PilotPattern { cells },
        )
    }
}

/// The continual pilot carriers of DVB-T 2K mode (ETSI EN 300 744 table 7).
pub const DVBT_2K_CONTINUAL: [u16; 45] = [
    0, 48, 54, 87, 141, 156, 192, 201, 255, 279, 282, 333, 432, 450, 483, 525, 531, 618, 636, 714,
    759, 765, 780, 804, 873, 888, 918, 939, 942, 969, 984, 1050, 1101, 1107, 1110, 1137, 1140,
    1146, 1206, 1269, 1323, 1377, 1491, 1683, 1704,
];

/// The TPS carriers of DVB-T 2K mode (ETSI EN 300 744 table 8).
pub const DVBT_2K_TPS: [u16; 17] = [
    34, 50, 209, 346, 413, 569, 595, 688, 790, 901, 1073, 1219, 1262, 1286, 1469, 1594, 1687,
];

/// The continual pilot carriers of DVB-T 8K mode (ETSI EN 300 744 table 7, 8K column).
pub const DVBT_8K_CONTINUAL: [u16; 177] = [
    0, 48, 54, 87, 141, 156, 192, 201, 255, 279, 282, 333, 432, 450, 483, 525, 531, 618, 636, 714,
    759, 765, 780, 804, 873, 888, 918, 939, 942, 969, 984, 1050, 1101, 1107, 1110, 1137, 1140,
    1146, 1206, 1269, 1323, 1377, 1491, 1683, 1704, 1752, 1758, 1791, 1845, 1860, 1896, 1905, 1959,
    1983, 1986, 2037, 2136, 2154, 2187, 2229, 2235, 2322, 2340, 2418, 2463, 2469, 2484, 2508, 2577,
    2592, 2622, 2643, 2646, 2673, 2688, 2754, 2805, 2811, 2814, 2841, 2844, 2850, 2910, 2973, 3027,
    3081, 3195, 3387, 3408, 3456, 3462, 3495, 3549, 3564, 3600, 3609, 3663, 3687, 3690, 3741, 3840,
    3858, 3891, 3933, 3939, 4026, 4044, 4122, 4167, 4173, 4188, 4212, 4281, 4296, 4326, 4347, 4350,
    4377, 4392, 4458, 4509, 4515, 4518, 4545, 4548, 4554, 4614, 4677, 4731, 4785, 4899, 5091, 5112,
    5160, 5166, 5199, 5253, 5268, 5304, 5313, 5367, 5391, 5394, 5445, 5544, 5562, 5595, 5637, 5643,
    5730, 5748, 5826, 5871, 5877, 5892, 5916, 5985, 6000, 6030, 6051, 6054, 6081, 6096, 6162, 6213,
    6219, 6222, 6249, 6252, 6258, 6318, 6381, 6435, 6489, 6603, 6795, 6816,
];

/// The TPS carriers of DVB-T 8K mode (ETSI EN 300 744 table 8, 8K column).
pub const DVBT_8K_TPS: [u16; 68] = [
    34, 50, 209, 346, 413, 569, 595, 688, 790, 901, 1073, 1219, 1262, 1286, 1469, 1594, 1687, 1738,
    1754, 1913, 2050, 2117, 2273, 2299, 2392, 2494, 2605, 2777, 2923, 2966, 2990, 3173, 3298, 3391,
    3442, 3458, 3617, 3754, 3821, 3977, 4003, 4096, 4198, 4309, 4481, 4627, 4670, 4694, 4877, 5002,
    5095, 5146, 5162, 5321, 5458, 5525, 5681, 5707, 5800, 5902, 6013, 6185, 6331, 6374, 6398, 6581,
    6706, 6799,
];

/// The 802.11a long training symbol `L(-26..=26)` (IEEE Std 802.11-2016 equation 17-8), DC included as 0.
pub const IEEE80211A_LTF: [i8; 53] = [
    1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 0, 1, -1,
    -1, 1, 1, -1, 1, -1, 1, -1, -1, -1, -1, -1, 1, 1, -1, -1, 1, -1, 1, -1, 1, 1, 1, 1,
];

/// The 802.11a short training symbol `S(-26..=26)` (IEEE Std 802.11-2016 equation 17-6) as multiples of `sqrt(13/6) (1 + j)`.
pub const IEEE80211A_STF: [i8; 53] = [
    0, 0, 1, 0, 0, 0, -1, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
    -1, 0, 0, 0, -1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0,
];

/// The 127 pilot polarities `p_0 .. p_126` of IEEE Std 802.11-2016 equation 17-25: the 802.11 scrambler from all ones, a 1 giving
/// -1 and a 0 giving +1.
pub fn ieee80211_pilot_polarity() -> [i8; 127] {
    let mut s = sdrkit_util_bits::seq::ieee80211(0x7F);
    let mut p = [0i8; 127];
    for v in p.iter_mut() {
        *v = 1 - 2 * s.bit() as i8;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ieee80211_tables_and_polarity() {
        // Equation 17-25: p_0 .. p_15.
        let p = ieee80211_pilot_polarity();
        assert_eq!(
            p[..16],
            [1, 1, 1, 1, -1, -1, -1, 1, -1, -1, -1, -1, 1, 1, -1, 1]
        );
        assert_eq!(p[120..], [-1; 7]);
        let l = OfdmLayout::ieee80211a();
        assert_eq!(l.used(), 53);
        assert_eq!(l.period(), 127);
        assert_eq!(l.data(0).len(), 48);
        assert_eq!(
            l.pilots(4)
                .iter()
                .map(|p| l.carrier(p.index as usize))
                .collect::<Vec<_>>(),
            [-21, -7, 7, 21]
        );
        // Symbol 4 has p_4 = -1: pilot values (-1, -1, -1, 1).
        assert_eq!(
            l.pilots(4).iter().map(|p| p.value.re).collect::<Vec<_>>(),
            [-1.0, -1.0, -1.0, 1.0]
        );
        assert_eq!(l.bin(0), 64 - 26);
    }

    #[test]
    fn ieee80211_training_symbols_match_the_annex_example() {
        // IEEE Std 802.11-2016 annex I: the first time samples of the training symbols (before windowing):
        // STF 0.046+0.046j, -0.132+0.002j, -0.013-0.079j, 0.143-0.013j; LTF 0.156, -0.005-0.120j, 0.040-0.111j, 0.097+0.083j.
        let idft = |cells: &dyn Fn(usize) -> (f64, f64), t: usize| {
            let (mut re, mut im) = (0.0, 0.0);
            for i in 0..53 {
                let k = i as f64 - 26.0;
                let (a, b) = cells(i);
                let ph = std::f64::consts::TAU * k * t as f64 / 64.0;
                re += a * ph.cos() - b * ph.sin();
                im += a * ph.sin() + b * ph.cos();
            }
            (
                (re / 64.0 * 1000.0).round() / 1000.0,
                (im / 64.0 * 1000.0).round() / 1000.0,
            )
        };
        let g = (13.0f64 / 6.0).sqrt();
        let stf = |i: usize| (IEEE80211A_STF[i] as f64 * g, IEEE80211A_STF[i] as f64 * g);
        let ltf = |i: usize| (IEEE80211A_LTF[i] as f64, 0.0);
        let s: Vec<_> = (0..4).map(|t| idft(&stf, t)).collect();
        assert_eq!(
            s,
            [
                (0.046, 0.046),
                (-0.132, 0.002),
                (-0.013, -0.079),
                (0.143, -0.013)
            ]
        );
        let l: Vec<_> = (0..4).map(|t| idft(&ltf, t)).collect();
        assert_eq!(
            l,
            [
                (0.156, 0.0),
                (-0.005, -0.12),
                (0.04, -0.111),
                (0.097, 0.083)
            ]
        );
    }

    #[test]
    fn dvbt_2k_pilot_positions() {
        let l = OfdmLayout::dvbt_2k(4).unwrap();
        assert_eq!(l.used(), 1705);
        assert_eq!(l.cp(0), 512);
        assert_eq!(l.period(), 4);
        // EN 300 744: 2K has 1512 data carriers in every symbol (clause 4.4, table 1: 1705 = 1512 + 142/131 pilots + 17 TPS +
        // continual pilots that fall on scattered positions count once).
        for s in 0..4 {
            assert_eq!(l.data(s).len(), 1512, "symbol {s}");
        }
        // Symbol 1: scattered pilots at k = 3 + 12 p.
        let k: Vec<i32> = l
            .pilots(1)
            .iter()
            .map(|p| l.carrier(p.index as usize) + 852)
            .collect();
        assert!(k.contains(&3) && k.contains(&15) && !k.contains(&6));
        // w_0 = 1 (the PRBS starts with ones): the pilot at k = 0 is -4/3.
        assert!((l.pilots(0)[0].value.re + 4.0 / 3.0).abs() < 1e-6);
        assert!(OfdmLayout::dvbt_2k(5).is_err());
        assert!(OfdmLayout::new(8, &[2], -5, 5, &[], &[], &PilotPattern::none()).is_err());
        assert!(OfdmLayout::new(16, &[2], -5, 5, &[9], &[], &PilotPattern::none()).is_err());
        let p = PilotPattern::continual(&[(0, Complex32::new(1.0, 0.0))]);
        assert!(OfdmLayout::new(16, &[2], -5, 5, &[0], &[], &p).is_err());
    }

    #[test]
    fn dvbt_8k_tables_and_layout() {
        // EN 300 744 table 7 and 8 (8K columns), compared value by value with the standard's text at a few places: the first
        // continual pilot beyond the 2K range, the middle ones and the last, and the TPS carriers likewise.
        assert_eq!(DVBT_8K_CONTINUAL[45..48], [1752, 1758, 1791]);
        assert_eq!(DVBT_8K_CONTINUAL[88..91], [3408, 3456, 3462]);
        assert_eq!(DVBT_8K_CONTINUAL[132..135], [5112, 5160, 5166]);
        assert_eq!(DVBT_8K_CONTINUAL[176], 6816);
        assert_eq!(DVBT_8K_TPS[17..20], [1738, 1754, 1913]);
        assert_eq!(DVBT_8K_TPS[67], 6799);
        // Both tables are strictly increasing, and their first parts are the 2K tables.
        assert!(DVBT_8K_CONTINUAL.windows(2).all(|w| w[0] < w[1]));
        assert!(DVBT_8K_TPS.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(DVBT_8K_CONTINUAL[..45], DVBT_2K_CONTINUAL);
        assert_eq!(DVBT_8K_TPS[..17], DVBT_2K_TPS);
        // The 8K tables are the 2K tables repeated every 1704 carriers, a shared end point counted once: 4 x 45 - 3 = 177 and
        // 4 x 17 = 68.
        let mut rep: Vec<u16> = (0..4u16)
            .flat_map(|b| DVBT_2K_CONTINUAL.iter().map(move |&k| k + 1704 * b))
            .collect();
        rep.dedup();
        assert_eq!(rep, DVBT_8K_CONTINUAL);
        let rep: Vec<u16> = (0..4u16)
            .flat_map(|b| DVBT_2K_TPS.iter().map(move |&k| k + 1704 * b))
            .collect();
        assert_eq!(rep, DVBT_8K_TPS);

        let l = OfdmLayout::dvbt_8k(4).unwrap();
        assert_eq!(
            (l.nfft(), l.cp(0), l.used(), l.period()),
            (8192, 2048, 6817, 4)
        );
        assert_eq!((l.first(), l.last()), (-3408, 3408));
        assert_eq!(l.reserved().len(), 68);
        // Clause 4.4 (table 1): 6817 carriers in 8K mode. Clause 4.5.1: 6048 useful data carriers in every symbol.
        for s in 0..4 {
            assert_eq!(l.data(s).len(), 6048, "symbol {s}");
        }
        // Every continual pilot is a pilot in every symbol, at the boosted value 4/3 * 2 (1/2 - w_k).
        let mut prbs = sdrkit_util_bits::seq::DVBT_PILOTS;
        let w: Vec<u8> = (0..6817).map(|_| prbs.bit()).collect();
        for s in 0..4 {
            let p = l.pilots(s);
            for &k in &DVBT_8K_CONTINUAL {
                let i = (k as i32 - 3408 - l.first()) as u32;
                let v = p
                    .iter()
                    .find(|q| q.index == i)
                    .expect("continual pilot")
                    .value
                    .re;
                let want = 4.0 / 3.0 * 2.0 * (0.5 - w[k as usize] as f32);
                assert!((v - want).abs() < 1e-6);
            }
        }
        // Scattered pilots of symbol 2 at k = 6 + 12 p.
        let k: Vec<i32> = l
            .pilots(2)
            .iter()
            .map(|p| l.carrier(p.index as usize) + 3408)
            .collect();
        assert!(k.contains(&6) && k.contains(&6810) && !k.contains(&9));
        assert_eq!(OfdmLayout::by_name("dvbt_8k", 8).unwrap().cp(0), 1024);
        assert!(OfdmLayout::dvbt_8k(5).is_err());
    }
}
