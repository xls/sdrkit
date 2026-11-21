//! Constellation definitions: the points of a linear modulation and the bit label each point carries.
//!
//! This is the one home of every point set and bit map. Modulators (`sdrkit-algo-sim`) read the points from here; slicers and soft
//! demappers (`sdrkit-algo-modem`) build their decision tables from the same [`PointSet`], so a modulator and a demodulator cannot
//! disagree about a map.
//!
//! A [`PointSet`] holds `2^b` points indexed by label: `points()[label]` is the point that carries `label`, and the label's bits
//! are sent most significant first ([`PointSet::map_bits`]). Every set made here has unit mean energy (each point equally likely).
//!
//! | Builder | What | Standard |
//! |---|---|---|
//! | [`PointSet::psk`] | M-PSK, point `j` at `phase0 + 2 pi j / M` labelled `gray(j)` | the plain Gray map |
//! | [`PointSet::bpsk`], [`PointSet::qpsk`] | BPSK (0 is +1), QPSK of 3GPP and DVB-S2 | TS 36.211 7.1.1/7.1.2, EN 302 307 5.4.1 |
//! | [`PointSet::pam`] | M-PAM, Gray over the levels from the most negative | |
//! | [`PointSet::qam`] | square M-QAM with a [`QamMap`] | IEEE 802.11, DVB-T, 3GPP |
//! | [`PointSet::qam_hierarchical`] | DVB-T non-uniform 16/64-QAM with `alpha` 1, 2 or 4 | EN 300 744 4.3.5 |
//! | [`PointSet::dvb_c`] | DVB-C 16 to 256-QAM including the 32 and 128 cross shapes | EN 300 429 clause 8 |
//! | [`PointSet::apsk`] | APSK from rings, labels in ring order | |
//! | [`PointSet::dvbs2_8psk`], [`PointSet::dvbs2_16apsk`], [`PointSet::dvbs2_32apsk`] | DVB-S2 maps and ring ratios per code rate | EN 302 307 5.4.2 to 5.4.4 |
//! | [`PointSet::vsb8`] | 8-VSB levels | ATSC A/53 Part 2, 6.5 |
//!
//! [`by_name`] returns a set by its name (for settings and front ends); [`NAMES`] lists every name and the builder it maps to.
//!
//! ```
//! use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
//! // 3GPP 16-QAM (TS 36.211 table 7.1.3-1): 0000 is (1 + 1j)/sqrt(10), 0011 is (3 + 3j)/sqrt(10).
//! let q = PointSet::qam(16, QamMap::THREE_GPP).unwrap();
//! let s = 10f32.sqrt();
//! assert!((q.points()[0b0000].re * s - 1.0).abs() < 1e-6 && (q.points()[0b0011].im * s - 3.0).abs() < 1e-6);
//! let mut out = Vec::new();
//! q.map_bits(&[0, 0, 1, 1], &mut out).unwrap();
//! assert_eq!(out[0], q.points()[3]);
//! ```

use crate::{Complex32, DspError, Param};
use sdrkit_util_bits::gray::{gray, ungray};
use std::f64::consts::{PI, TAU};

/// The geometry of a point set, which lets a slicer choose a fast decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `m` points equally spaced on the unit circle.
    Psk {
        /// Number of points.
        m: u32,
    },
    /// A square QAM grid of `side x side` points.
    SquareQam {
        /// Points per axis.
        side: u32,
    },
    /// A cross QAM (32, 128): a square grid with the corners moved.
    CrossQam,
    /// Real levels on the I axis.
    Pam,
    /// Concentric rings.
    Apsk,
    /// Any other arrangement.
    Other,
}

/// The order of a square QAM label's bits between the two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QamLayout {
    /// The first half of the bits label I, the second half Q (IEEE 802.11).
    Split,
    /// Bits alternate I, Q, I, Q from the most significant (DVB-T `y0 y1 y2 ...`, 3GPP `b(i) b(i+1) ...`).
    Interleaved,
}

/// Which bits of each axis's Gray code are inverted, the axis code being `gray(L)` with `L` counted from the most negative level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisFlip {
    /// None: the most negative level is all zeros (IEEE 802.11: `00, 01, 11, 10` give -3, -1, +1, +3).
    None,
    /// The most significant (sign) bit: 0 is the positive half (DVB-T: `00, 01, 11, 10` give +3, +1, -1, -3).
    Msb,
    /// Every bit (3GPP: `00, 01, 10, 11` give +1, +3, -1, -3 for `b(i) b(i+2)`).
    All,
}

/// How square QAM labels map onto the grid: the bit layout between the axes and the Gray code on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QamMap {
    /// Bit order between the axes.
    pub layout: QamLayout,
    /// The axis code.
    pub flip: AxisFlip,
}

impl QamMap {
    /// IEEE 802.11-2016 clause 17.3.5.8 (also 802.11n/ac/ax): `b0 b1` on I, `b2 b3` on Q (16-QAM), 0 is the negative side.
    pub const IEEE_802_11: QamMap = QamMap {
        layout: QamLayout::Split,
        flip: AxisFlip::None,
    };
    /// DVB-T (EN 300 744 clause 4.3.5, figure 9) and ISDB-T: `y0 = I`, `y1 = Q`, `y2 = I` ...; 0 is the positive side, the outer
    /// level of each half is all zeros.
    pub const DVB: QamMap = QamMap {
        layout: QamLayout::Interleaved,
        flip: AxisFlip::Msb,
    };
    /// 3GPP TS 36.211 clause 7.1 and TS 38.211 clause 5.1: `b(i)` signs I, `b(i+1)` signs Q (0 positive), then magnitude bits.
    pub const THREE_GPP: QamMap = QamMap {
        layout: QamLayout::Interleaved,
        flip: AxisFlip::All,
    };
}

/// One ring of an APSK constellation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    /// Points on the ring.
    pub points: usize,
    /// Its radius (relative; the whole set is scaled to unit mean energy).
    pub radius: f64,
    /// The angle of its first point, radians.
    pub phase: f64,
}

/// A DVB-S2 code rate, which sets the APSK ring ratios (EN 302 307 tables 9 and 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dvbs2Rate {
    /// 2/3 (16APSK only).
    R2_3,
    /// 3/4.
    R3_4,
    /// 4/5.
    R4_5,
    /// 5/6.
    R5_6,
    /// 8/9.
    R8_9,
    /// 9/10.
    R9_10,
}

impl Dvbs2Rate {
    /// The 16APSK ring ratio `gamma = R2 / R1` (EN 302 307 table 9).
    pub fn gamma_16apsk(self) -> f64 {
        match self {
            Dvbs2Rate::R2_3 => 3.15,
            Dvbs2Rate::R3_4 => 2.85,
            Dvbs2Rate::R4_5 => 2.75,
            Dvbs2Rate::R5_6 => 2.70,
            Dvbs2Rate::R8_9 => 2.60,
            Dvbs2Rate::R9_10 => 2.57,
        }
    }

    /// The 32APSK ring ratios `(gamma1 = R2 / R1, gamma2 = R3 / R1)` (EN 302 307 table 10); `None` for 2/3, which has no 32APSK.
    pub fn gammas_32apsk(self) -> Option<(f64, f64)> {
        match self {
            Dvbs2Rate::R2_3 => None,
            Dvbs2Rate::R3_4 => Some((2.84, 5.27)),
            Dvbs2Rate::R4_5 => Some((2.72, 4.87)),
            Dvbs2Rate::R5_6 => Some((2.64, 4.64)),
            Dvbs2Rate::R8_9 => Some((2.54, 4.33)),
            Dvbs2Rate::R9_10 => Some((2.53, 4.30)),
        }
    }
}

/// Points indexed by bit label (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct PointSet {
    points: Vec<Complex32>,
    bits: u32,
    shape: Shape,
}

fn bits_of(m: usize) -> Result<u32, DspError> {
    if (2..=1 << 16).contains(&m) && m.is_power_of_two() {
        Ok(m.trailing_zeros())
    } else {
        Param::Points.not_allowed(m as f64)
    }
}

fn polar(r: f64, a: f64) -> Complex32 {
    Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
}

impl PointSet {
    /// A set from its points (`points[label]`), scaled to unit mean energy. Errors unless the count is a power of two from 2 to
    /// 65536 and every point is finite with some energy in the set.
    pub fn new(points: Vec<Complex32>, shape: Shape) -> Result<Self, DspError> {
        let bits = bits_of(points.len())?;
        if points
            .iter()
            .any(|p| !p.re.is_finite() || !p.im.is_finite())
        {
            return Param::Points.not_finite();
        }
        let e: f64 = points.iter().map(|p| p.norm_sqr() as f64).sum::<f64>() / points.len() as f64;
        if e <= 0.0 {
            return Param::Power.degenerate();
        }
        let s = (1.0 / e.sqrt()) as f32;
        Ok(PointSet {
            points: points.into_iter().map(|p| p * s).collect(),
            bits,
            shape,
        })
    }

    /// The points, indexed by label.
    pub fn points(&self) -> &[Complex32] {
        &self.points
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Never true (a set has at least two points).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Bits per symbol.
    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// The geometry.
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// The point of `label`, or `None` when the label is out of range.
    pub fn point(&self, label: usize) -> Option<Complex32> {
        self.points.get(label).copied()
    }

    /// The same points with new labels: label `l` of the result carries the point of label `map[l]` here. Errors unless `map` is a
    /// permutation of `0..len`.
    pub fn relabelled(&self, map: &[u16]) -> Result<Self, DspError> {
        let n = self.points.len();
        let mut seen = vec![false; n];
        if map.len() != n {
            return Param::Labels.length(n, map.len());
        }
        for &m in map {
            let m = m as usize;
            if m >= n || seen[m] {
                return Param::Labels.not_allowed(m as f64);
            }
            seen[m] = true;
        }
        Ok(PointSet {
            points: map.iter().map(|&m| self.points[m as usize]).collect(),
            bits: self.bits,
            shape: self.shape,
        })
    }

    /// The same set turned by `radians` (for example pi/4 for a rotated QPSK).
    pub fn rotated(&self, radians: f64) -> Self {
        let r = polar(1.0, radians);
        PointSet {
            points: self.points.iter().map(|&p| p * r).collect(),
            bits: self.bits,
            shape: self.shape,
        }
    }

    /// Maps hard bits (0 or 1 per byte, each label most significant bit first) to points, appending to `out`. Errors when the bits
    /// do not fill whole labels.
    pub fn map_bits(&self, bits: &[u8], out: &mut Vec<Complex32>) -> Result<(), DspError> {
        let b = self.bits as usize;
        if !bits.len().is_multiple_of(b) {
            return Param::Bits.not_allowed(bits.len() as f64);
        }
        for label in bits.chunks_exact(b) {
            let v = label
                .iter()
                .fold(0usize, |a, &x| (a << 1) | (x & 1) as usize);
            out.push(self.points[v]);
        }
        Ok(())
    }

    /// M-PSK: the point at `phase0 + 2 pi j / M` carries `gray(j)`, so neighbours differ in one bit.
    pub fn psk(m: usize, phase0: f64) -> Result<Self, DspError> {
        bits_of(m)?;
        if !phase0.is_finite() {
            return Param::Phase.not_finite();
        }
        let mut p = vec![Complex32::default(); m];
        for j in 0..m {
            p[gray(j as u32) as usize] = polar(1.0, phase0 + TAU * j as f64 / m as f64);
        }
        PointSet::new(p, Shape::Psk { m: m as u32 })
    }

    /// BPSK with 0 sent as +1 and 1 as -1 (TS 36.211 table 7.1.1-1 up to its pi/4 turn, and the convention that produces LLRs with no
    /// sign change).
    pub fn bpsk() -> Self {
        PointSet {
            points: vec![Complex32::new(1.0, 0.0), Complex32::new(-1.0, 0.0)],
            bits: 1,
            shape: Shape::Psk { m: 2 },
        }
    }

    /// QPSK with `b0` on I and `b1` on Q, 0 positive: TS 36.211 table 7.1.2-1 and the DVB-S2 QPSK of EN 302 307 figure 9 (00 at
    /// 45 degrees, 10 at 135, 11 at 225, 01 at 315).
    pub fn qpsk() -> Self {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let pt = |l: usize| {
            let i = if l & 2 == 0 { s } else { -s };
            let q = if l & 1 == 0 { s } else { -s };
            Complex32::new(i, q)
        };
        PointSet {
            points: (0..4).map(pt).collect(),
            bits: 2,
            shape: Shape::Psk { m: 4 },
        }
    }

    /// The axis levels `-(m-1) .. m-1` (odd integers), the level of axis code `c` under `flip`.
    fn axis_level(side: usize, code: u32, flip: AxisFlip) -> i32 {
        let k = side.trailing_zeros();
        let mask = (side - 1) as u32;
        let g = match flip {
            AxisFlip::None => code,
            AxisFlip::Msb => code ^ (1 << (k - 1)),
            AxisFlip::All => code ^ mask,
        };
        let l = ungray(g & mask) as i32;
        2 * l - (side as i32 - 1)
    }

    /// M-PAM on the I axis, Gray over the levels from the most negative (label 0 is `-(m-1)`), unit mean energy.
    pub fn pam(m: usize) -> Result<Self, DspError> {
        bits_of(m)?;
        let p = (0..m)
            .map(|l| Complex32::new(Self::axis_level(m, l as u32, AxisFlip::None) as f32, 0.0))
            .collect();
        PointSet::new(p, Shape::Pam)
    }

    /// Square M-QAM (4, 16, 64, 256, 1024, ...: an even number of bits) with the bit map `map`, unit mean energy.
    pub fn qam(m: usize, map: QamMap) -> Result<Self, DspError> {
        Self::qam_levels(m, map, 1.0)
    }

    fn split_label(label: u32, half: u32, layout: QamLayout) -> (u32, u32) {
        match layout {
            QamLayout::Split => (label >> half, label & ((1 << half) - 1)),
            QamLayout::Interleaved => {
                let (mut i, mut q) = (0u32, 0u32);
                for k in 0..half {
                    // From the most significant: I, Q, I, Q ...
                    let bi = (label >> (2 * (half - 1 - k) + 1)) & 1;
                    let bq = (label >> (2 * (half - 1 - k))) & 1;
                    i = (i << 1) | bi;
                    q = (q << 1) | bq;
                }
                (i, q)
            }
        }
    }

    fn qam_levels(m: usize, map: QamMap, alpha: f64) -> Result<Self, DspError> {
        let b = bits_of(m)?;
        if b % 2 != 0 {
            return Param::Points.not_allowed(m as f64);
        }
        let half = b / 2;
        let side = 1usize << half;
        // Non-uniform levels: the innermost pair at +-alpha, then steps of 2 (alpha = 1 is the uniform grid).
        let lv = |x: i32| -> f32 { (x.signum() as f64 * (x.abs() as f64 - 1.0 + alpha)) as f32 };
        let p = (0..m as u32)
            .map(|l| {
                let (ci, cq) = Self::split_label(l, half, map.layout);
                Complex32::new(
                    lv(Self::axis_level(side, ci, map.flip)),
                    lv(Self::axis_level(side, cq, map.flip)),
                )
            })
            .collect();
        PointSet::new(p, Shape::SquareQam { side: side as u32 })
    }

    /// DVB-T hierarchical (non-uniform) 16- or 64-QAM, EN 300 744 clause 4.3.5: the levels of each axis are `+-alpha,
    /// +-(alpha + 2) ...`, `alpha` 1 (uniform), 2 or 4, with the [`QamMap::DVB`] labels; scaled to unit mean energy (the standard's
    /// normalisation factors sqrt(10), sqrt(20), sqrt(52) for 16-QAM and sqrt(42), sqrt(60), sqrt(108) for 64-QAM).
    pub fn qam_hierarchical(m: usize, alpha: u32) -> Result<Self, DspError> {
        if !(m == 16 || m == 64) {
            return Param::Points.not_allowed(m as f64);
        }
        if !matches!(alpha, 1 | 2 | 4) {
            return Param::Alpha.not_allowed(alpha as f64);
        }
        Self::qam_levels(m, QamMap::DVB, alpha as f64)
    }

    /// The first-quadrant point (odd coordinates) of DVB-C's low `q` bits (EN 300 429 figure 7).
    fn dvb_c_quadrant(m: usize, q: u32) -> (i32, i32) {
        let g2l = |g: u32| ungray(g) as i32;
        match m {
            32 => {
                const T: [(i32, i32); 8] = [
                    (1, 1),
                    (3, 1),
                    (3, 5),
                    (5, 1),
                    (1, 3),
                    (3, 3),
                    (1, 5),
                    (5, 3),
                ];
                T[q as usize]
            }
            128 => {
                if q & 16 == 0 {
                    Self::dvb_c_quadrant(64, q & 15)
                } else {
                    let far = if q & 4 != 0 { 11 } else { 9 };
                    let level = 2 * g2l(q & 3) + 1;
                    if q & 8 == 0 {
                        (far, level)
                    } else {
                        (level, far)
                    }
                }
            }
            _ => {
                let k = (m.trailing_zeros() - 2) / 2;
                let (mut gx, mut gy) = (0u32, 0u32);
                for i in 0..k {
                    gy = (gy << 1) | ((q >> (2 * (k - 1 - i) + 1)) & 1);
                    gx = (gx << 1) | ((q >> (2 * (k - 1 - i))) & 1);
                }
                (2 * g2l(gx) + 1, 2 * g2l(gy) + 1)
            }
        }
    }

    /// DVB-C 16-, 32-, 64-, 128- or 256-QAM, EN 300 429 clause 8 and figure 7. The two most significant label bits `Ik Qk` give the
    /// quadrant **after** differential coding (00 first, 10 second, 11 third, 01 fourth: quarter turns 0, 1, 2, 3); the low bits
    /// choose the point within the first quadrant, which the quadrant turns. The differential coding of `Ik Qk` belongs to the
    /// caller (`sdrkit_algo_modem::differential`). The first-quadrant tables come from figure 7.
    pub fn dvb_c(m: usize) -> Result<Self, DspError> {
        if !matches!(m, 16 | 32 | 64 | 128 | 256) {
            return Param::Points.not_allowed(m as f64);
        }
        let b = m.trailing_zeros();
        let turns = [0u32, 3, 1, 2]; // Ik Qk = 00, 01, 10, 11
        let p = (0..m as u32)
            .map(|l| {
                let ab = l >> (b - 2);
                let (x, y) = Self::dvb_c_quadrant(m, l & ((1 << (b - 2)) - 1));
                let mut z = Complex32::new(x as f32, y as f32);
                for _ in 0..turns[ab as usize] {
                    z = Complex32::new(-z.im, z.re);
                }
                z
            })
            .collect();
        let shape = if b.is_multiple_of(2) {
            Shape::SquareQam { side: 1 << (b / 2) }
        } else {
            Shape::CrossQam
        };
        PointSet::new(p, shape)
    }

    /// APSK from rings, numbered ring by ring and round each ring from its phase (labels are plain indices; standards give their own
    /// map with [`relabelled`](Self::relabelled) or the named builders). Errors unless the total is a power of two and every ring
    /// has points, a radius of at least 0 and a finite phase.
    pub fn apsk(rings: &[Ring]) -> Result<Self, DspError> {
        if rings.is_empty() {
            return Param::Rings.length(1, 0);
        }
        for r in rings {
            if r.points == 0 {
                return Param::Points.at_least(1.0, 0.0);
            }
            if !(r.radius >= 0.0 && r.radius.is_finite()) {
                return Param::Radius.at_least(0.0, r.radius);
            }
            if !r.phase.is_finite() {
                return Param::Phase.not_finite();
            }
        }
        let mut p = Vec::new();
        for r in rings {
            for j in 0..r.points {
                p.push(polar(r.radius, r.phase + TAU * j as f64 / r.points as f64));
            }
        }
        PointSet::new(p, Shape::Apsk)
    }

    /// The DVB-S2 8PSK map (EN 302 307 clause 5.4.2, figure 10): labels `000` at 45 degrees, `001` at 0, `100` at 90, `110` at
    /// 135, `010` at 180, `011` at 225, `111` at 270, `101` at 315.
    pub fn dvbs2_8psk() -> Self {
        // Angle of each label in multiples of pi/4.
        const A: [f64; 8] = [1.0, 0.0, 4.0, 5.0, 2.0, 7.0, 3.0, 6.0];
        PointSet {
            points: A.iter().map(|&k| polar(1.0, k * PI / 4.0)).collect(),
            bits: 3,
            shape: Shape::Other,
        }
    }

    /// The DVB-S2 16APSK (4 + 12) map with ring ratio `gamma = R2 / R1` (EN 302 307 clause 5.4.3, figure 11; [`Dvbs2Rate::gamma_16apsk`]
    /// returns the ratio of each code rate). Inner ring: `1100` at 45 degrees, `1110` at 135, `1111` at 225, `1101` at 315; the outer
    /// ring is Gray round the circle. Errors unless `gamma > 1`.
    pub fn dvbs2_16apsk(gamma: f64) -> Result<Self, DspError> {
        if !(gamma > 1.0 && gamma.is_finite()) {
            return Param::Gamma.at_least(1.0, gamma);
        }
        // (ring radius, angle in multiples of pi/12) per label.
        const T: [(u8, i32); 16] = [
            (2, 3),
            (2, -3),
            (2, 9),
            (2, -9),
            (2, 1),
            (2, -1),
            (2, 11),
            (2, -11),
            (2, 5),
            (2, -5),
            (2, 7),
            (2, -7),
            (1, 3),
            (1, -3),
            (1, 9),
            (1, -9),
        ];
        let p = T
            .iter()
            .map(|&(r, k)| polar(if r == 1 { 1.0 } else { gamma }, k as f64 * PI / 12.0))
            .collect();
        PointSet::new(p, Shape::Apsk)
    }

    /// The DVB-S2 32APSK (4 + 12 + 16) map with ring ratios `gamma1 = R2 / R1`, `gamma2 = R3 / R1` (EN 302 307 clause 5.4.4,
    /// figure 12; [`Dvbs2Rate::gammas_32apsk`] returns them per code rate). The label table is the widely published one from
    /// figure 12. Errors unless `1 < gamma1 < gamma2`.
    pub fn dvbs2_32apsk(gamma1: f64, gamma2: f64) -> Result<Self, DspError> {
        if !(gamma1 > 1.0 && gamma1.is_finite()) {
            return Param::Gamma1.at_least(1.0, gamma1);
        }
        if !gamma2.is_finite() {
            return Param::Gamma2.not_finite();
        }
        if gamma2 <= gamma1 {
            return Param::Gamma2.conflict(Param::Gamma1);
        }
        // (ring, angle in multiples of pi/24) per label.
        const T: [(u8, i32); 32] = [
            (2, 6),
            (2, 10),
            (2, -6),
            (2, -10),
            (2, 18),
            (2, 14),
            (2, -18),
            (2, -14),
            (3, 3),
            (3, 9),
            (3, -6),
            (3, -12),
            (3, 18),
            (3, 12),
            (3, -21),
            (3, -15),
            (2, 2),
            (1, 6),
            (2, -2),
            (1, -6),
            (2, 22),
            (1, 18),
            (2, -22),
            (1, -18),
            (3, 0),
            (3, 6),
            (3, -3),
            (3, -9),
            (3, 21),
            (3, 15),
            (3, 24),
            (3, -18),
        ];
        let p = T
            .iter()
            .map(|&(r, k)| {
                let rad = match r {
                    1 => 1.0,
                    2 => gamma1,
                    _ => gamma2,
                };
                polar(rad, k as f64 * PI / 24.0)
            })
            .collect();
        PointSet::new(p, Shape::Apsk)
    }

    /// 8-VSB symbol levels (ATSC A/53 Part 2 clause 6.5, the 8-level symbol mapper): `Z2 Z1 Z0` = 000 .. 111 give -7, -5, -3, -1,
    /// +1, +3, +5, +7 (natural binary, not Gray), on the I axis, scaled by `1/sqrt(21)` to unit energy (without the pilot).
    pub fn vsb8() -> Self {
        let s = 1.0 / 21f32.sqrt();
        PointSet {
            points: (0..8)
                .map(|l| Complex32::new((2 * l - 7) as f32 * s, 0.0))
                .collect(),
            bits: 3,
            shape: Shape::Pam,
        }
    }
}

/// Every constellation name [`by_name`] accepts, in the order a front end lists them.
///
/// | Names | Builder |
/// |---|---|
/// | `bpsk`, `qpsk` | [`PointSet::bpsk`], [`PointSet::qpsk`] (3GPP and DVB-S2 QPSK) |
/// | `psk8` | [`PointSet::psk`]`(8, 0)`: plain Gray 8PSK starting on the I axis |
/// | `dvbs2_8psk` | [`PointSet::dvbs2_8psk`] |
/// | `pam2`, `pam4`, `pam8` | [`PointSet::pam`] |
/// | `qam<M>_ieee`, `qam<M>_dvb`, `qam<M>_3gpp` (M = 16, 64, 256, 1024) | [`PointSet::qam`] with [`QamMap::IEEE_802_11`], [`QamMap::DVB`], [`QamMap::THREE_GPP`] |
/// | `dvbt_qam<M>_a2`, `dvbt_qam<M>_a4` (M = 16, 64) | [`PointSet::qam_hierarchical`] with alpha 2 or 4 |
/// | `dvbc_qam<M>` (M = 16, 32, 64, 128, 256) | [`PointSet::dvb_c`] |
/// | `dvbs2_16apsk_<rate>` (2_3, 3_4, 4_5, 5_6, 8_9, 9_10) | [`PointSet::dvbs2_16apsk`] with that rate's ring ratio ([`Dvbs2Rate::gamma_16apsk`]) |
/// | `dvbs2_32apsk_<rate>` (3_4, 4_5, 5_6, 8_9, 9_10) | [`PointSet::dvbs2_32apsk`] with that rate's ring ratios ([`Dvbs2Rate::gammas_32apsk`]) |
/// | `vsb8` | [`PointSet::vsb8`] |
pub const NAMES: &[&str] = &[
    "bpsk",
    "qpsk",
    "psk8",
    "dvbs2_8psk",
    "pam2",
    "pam4",
    "pam8",
    "qam16_ieee",
    "qam64_ieee",
    "qam256_ieee",
    "qam1024_ieee",
    "qam16_dvb",
    "qam64_dvb",
    "qam256_dvb",
    "qam1024_dvb",
    "qam16_3gpp",
    "qam64_3gpp",
    "qam256_3gpp",
    "qam1024_3gpp",
    "dvbt_qam16_a2",
    "dvbt_qam16_a4",
    "dvbt_qam64_a2",
    "dvbt_qam64_a4",
    "dvbc_qam16",
    "dvbc_qam32",
    "dvbc_qam64",
    "dvbc_qam128",
    "dvbc_qam256",
    "dvbs2_16apsk_2_3",
    "dvbs2_16apsk_3_4",
    "dvbs2_16apsk_4_5",
    "dvbs2_16apsk_5_6",
    "dvbs2_16apsk_8_9",
    "dvbs2_16apsk_9_10",
    "dvbs2_32apsk_3_4",
    "dvbs2_32apsk_4_5",
    "dvbs2_32apsk_5_6",
    "dvbs2_32apsk_8_9",
    "dvbs2_32apsk_9_10",
    "vsb8",
];

impl Dvbs2Rate {
    /// The rate written as in [`NAMES`] (`"2_3"`, `"3_4"` ... `"9_10"`), or `None`.
    fn from_name(s: &str) -> Option<Dvbs2Rate> {
        Some(match s {
            "2_3" => Dvbs2Rate::R2_3,
            "3_4" => Dvbs2Rate::R3_4,
            "4_5" => Dvbs2Rate::R4_5,
            "5_6" => Dvbs2Rate::R5_6,
            "8_9" => Dvbs2Rate::R8_9,
            "9_10" => Dvbs2Rate::R9_10,
            _ => return None,
        })
    }
}

/// The point set called `name` (one of [`NAMES`], which lists the builder each name maps to), or `None` for any other name.
///
/// ```
/// use sdrkit_algo_dsp::constellation::{by_name, PointSet, NAMES};
/// assert_eq!(by_name("qam16_dvb").unwrap().len(), 16);
/// assert_eq!(by_name("dvbs2_8psk"), Some(PointSet::dvbs2_8psk()));
/// assert!(by_name("qam17").is_none());
/// assert!(NAMES.iter().all(|n| by_name(n).is_some()));
/// ```
pub fn by_name(name: &str) -> Option<PointSet> {
    match name {
        "bpsk" => return Some(PointSet::bpsk()),
        "qpsk" => return Some(PointSet::qpsk()),
        "psk8" => return PointSet::psk(8, 0.0).ok(),
        "dvbs2_8psk" => return Some(PointSet::dvbs2_8psk()),
        "vsb8" => return Some(PointSet::vsb8()),
        _ => {}
    }
    if !NAMES.contains(&name) {
        return None;
    }
    let order = |s: &str| s.parse::<usize>().ok();
    if let Some(m) = name.strip_prefix("pam") {
        return PointSet::pam(order(m)?).ok();
    }
    if let Some(r) = name.strip_prefix("dvbs2_16apsk_") {
        return PointSet::dvbs2_16apsk(Dvbs2Rate::from_name(r)?.gamma_16apsk()).ok();
    }
    if let Some(r) = name.strip_prefix("dvbs2_32apsk_") {
        let (g1, g2) = Dvbs2Rate::from_name(r)?.gammas_32apsk()?;
        return PointSet::dvbs2_32apsk(g1, g2).ok();
    }
    if let Some(m) = name.strip_prefix("dvbc_qam") {
        return PointSet::dvb_c(order(m)?).ok();
    }
    if let Some(rest) = name.strip_prefix("dvbt_qam") {
        let (m, a) = rest.split_once("_a")?;
        return PointSet::qam_hierarchical(order(m)?, order(a)? as u32).ok();
    }
    let (m, map) = name.strip_prefix("qam")?.split_once('_')?;
    let map = match map {
        "ieee" => QamMap::IEEE_802_11,
        "dvb" => QamMap::DVB,
        "3gpp" => QamMap::THREE_GPP,
        _ => return None,
    };
    PointSet::qam(order(m)?, map).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_builds_with_its_builder() {
        for n in NAMES {
            let s = by_name(n).unwrap_or_else(|| panic!("{n}"));
            assert!((energy(&s) - 1.0).abs() < 1e-3, "{n}");
        }
        // Each family maps to its builder with the standard's parameters (EN 302 307 tables 9 and 10 for the ring ratios).
        assert_eq!(
            by_name("qam64_3gpp"),
            PointSet::qam(64, QamMap::THREE_GPP).ok()
        );
        assert_eq!(
            by_name("dvbt_qam16_a4"),
            PointSet::qam_hierarchical(16, 4).ok()
        );
        assert_eq!(by_name("dvbc_qam128"), PointSet::dvb_c(128).ok());
        assert_eq!(
            by_name("dvbs2_16apsk_2_3"),
            PointSet::dvbs2_16apsk(3.15).ok()
        );
        assert_eq!(
            by_name("dvbs2_32apsk_9_10"),
            PointSet::dvbs2_32apsk(2.53, 4.30).ok()
        );
        assert_eq!(by_name("pam4"), PointSet::pam(4).ok());
        assert_eq!(by_name("psk8"), PointSet::psk(8, 0.0).ok());
        for bad in [
            "",
            "qam16",
            "qam16_foo",
            "qam17_dvb",
            "dvbs2_32apsk_2_3",
            "pam3",
            "dvbc_qam512",
            "QPSK",
        ] {
            assert!(by_name(bad).is_none(), "{bad}");
        }
    }

    fn energy(p: &PointSet) -> f64 {
        p.points().iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / p.len() as f64
    }

    fn at(p: &PointSet, label: usize, scale: f32) -> (i32, i32) {
        let v = p.points()[label] * scale;
        (v.re.round() as i32, v.im.round() as i32)
    }

    #[test]
    fn ieee_802_11_tables() {
        // IEEE 802.11-2016 clause 17.3.5.8, 16-QAM: b0b1 -> I, b2b3 -> Q: 00 -3, 01 -1, 11 +1, 10 +3, times 1/sqrt(10).
        let p = PointSet::qam(16, QamMap::IEEE_802_11).unwrap();
        let level = |two: usize| match two {
            0b00 => -3,
            0b01 => -1,
            0b11 => 1,
            _ => 3,
        };
        for l in 0..16 {
            assert_eq!(
                at(&p, l, 10f32.sqrt()),
                (level(l >> 2), level(l & 3)),
                "{l:04b}"
            );
        }
        // 64-QAM b0b1b2: 000, 001, 011, 010, 110, 111, 101, 100 give -7 .. +7.
        let q = PointSet::qam(64, QamMap::IEEE_802_11).unwrap();
        for (k, l) in [0b000usize, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100]
            .iter()
            .enumerate()
        {
            assert_eq!(at(&q, l << 3, 42f32.sqrt()).0, 2 * k as i32 - 7);
        }
    }

    #[test]
    fn three_gpp_tables() {
        // TS 36.211 table 7.1.2-1 (QPSK): 00 (1+j), 01 (1-j), 10 (-1+j), 11 (-1-j), over sqrt(2).
        for p in [
            PointSet::qpsk(),
            PointSet::qam(4, QamMap::THREE_GPP).unwrap(),
        ] {
            let s = 2f32.sqrt();
            assert_eq!(
                [0, 1, 2, 3].map(|l| at(&p, l, s)),
                [(1, 1), (1, -1), (-1, 1), (-1, -1)]
            );
        }
        // Table 7.1.3-1 (16-QAM), over sqrt(10).
        let p = PointSet::qam(16, QamMap::THREE_GPP).unwrap();
        let want = [
            (1, 1),
            (1, 3),
            (3, 1),
            (3, 3),
            (1, -1),
            (1, -3),
            (3, -1),
            (3, -3),
            (-1, 1),
            (-1, 3),
            (-3, 1),
            (-3, 3),
            (-1, -1),
            (-1, -3),
            (-3, -1),
            (-3, -3),
        ];
        for (l, w) in want.iter().enumerate() {
            assert_eq!(at(&p, l, 10f32.sqrt()), *w, "{l:04b}");
        }
        // Table 7.1.4-1 (64-QAM), the first 16 rows, over sqrt(42).
        let p = PointSet::qam(64, QamMap::THREE_GPP).unwrap();
        let want = [
            (3, 3),
            (3, 1),
            (1, 3),
            (1, 1),
            (3, 5),
            (3, 7),
            (1, 5),
            (1, 7),
            (5, 3),
            (5, 1),
            (7, 3),
            (7, 1),
            (5, 5),
            (5, 7),
            (7, 5),
            (7, 7),
        ];
        for (l, w) in want.iter().enumerate() {
            assert_eq!(at(&p, l, 42f32.sqrt()), *w, "{l:06b}");
        }
        // 000000 + the sign bits: 100000 is (-3, 3), 010000 is (3, -3).
        assert_eq!(at(&p, 0b100000, 42f32.sqrt()), (-3, 3));
        assert_eq!(at(&p, 0b010000, 42f32.sqrt()), (3, -3));
    }

    #[test]
    fn dvb_t_tables_and_hierarchy() {
        // EN 300 744 figure 9: 16-QAM y0 y1 y2 y3 = 0000 is (+3, +3); y2 = 1 is the inner I level; y0 = 1 the negative side.
        let p = PointSet::qam(16, QamMap::DVB).unwrap();
        let s = 10f32.sqrt();
        assert_eq!(at(&p, 0b0000, s), (3, 3));
        assert_eq!(at(&p, 0b0010, s), (1, 3));
        assert_eq!(at(&p, 0b1000, s), (-3, 3));
        assert_eq!(at(&p, 0b1010, s), (-1, 3));
        assert_eq!(at(&p, 0b0101, s), (3, -1));
        // 64-QAM, I levels for y0 y2 y4 = 000, 001, 011, 010: 7, 5, 3, 1.
        let p = PointSet::qam(64, QamMap::DVB).unwrap();
        let lv: Vec<i32> = [0b000000, 0b000010, 0b001010, 0b001000]
            .iter()
            .map(|&l| at(&p, l, 42f32.sqrt()).0)
            .collect();
        assert_eq!(lv, [7, 5, 3, 1]);
        // Hierarchical alpha: the standard's normalisation factors.
        for (m, alpha, norm) in [
            (16, 2, 20.0f32),
            (16, 4, 52.0),
            (64, 2, 60.0),
            (64, 4, 108.0),
            (64, 1, 42.0),
        ] {
            let p = PointSet::qam_hierarchical(m, alpha).unwrap();
            let corner = at(&p, 0, norm.sqrt());
            let want = alpha as i32 + if m == 16 { 2 } else { 6 };
            assert_eq!(corner, (want, want), "{m} alpha {alpha}");
            assert!((energy(&p) - 1.0).abs() < 1e-6);
        }
        assert!(PointSet::qam_hierarchical(256, 2).is_err());
    }

    #[test]
    fn dvb_c_quadrants_turn() {
        for m in [16usize, 32, 64, 128, 256] {
            let p = PointSet::dvb_c(m).unwrap();
            assert!((energy(&p) - 1.0).abs() < 1e-5);
            let b = m.trailing_zeros();
            let mut seen = std::collections::BTreeSet::new();
            for l in 0..m {
                let v = p.points()[l];
                seen.insert(((v.re * 1e4) as i32, (v.im * 1e4) as i32));
                // Ik Qk = 10 is the first quadrant turned a quarter (counter-clockwise).
                let low = l & ((1 << (b - 2)) - 1);
                if l >> (b - 2) == 0 {
                    assert!(v.re > 0.0 && v.im > 0.0);
                    let t = p.points()[(0b10 << (b - 2)) | low];
                    assert!((t - Complex32::new(-v.im, v.re)).norm() < 1e-6);
                }
            }
            assert_eq!(seen.len(), m);
        }
        assert_eq!(PointSet::dvb_c(32).unwrap().shape(), Shape::CrossQam);
    }

    #[test]
    fn dvbs2_maps() {
        // 8PSK: neighbours round the circle differ in one bit (the map is Gray).
        let p = PointSet::dvbs2_8psk();
        let by_angle = |p: &PointSet| {
            let mut v: Vec<(f32, usize)> = p
                .points()
                .iter()
                .enumerate()
                .map(|(l, z)| (z.im.atan2(z.re), l))
                .collect();
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            v.into_iter().map(|x| x.1).collect::<Vec<_>>()
        };
        let o = by_angle(&p);
        for k in 0..8 {
            assert_eq!((o[k] ^ o[(k + 1) % 8]).count_ones(), 1);
        }
        assert!((p.points()[0] - polar(1.0, PI / 4.0)).norm() < 1e-6);
        assert!((p.points()[0b100] - polar(1.0, PI / 2.0)).norm() < 1e-6);
        // 16APSK: ring ratio, inner labels 11xx, outer ring Gray round the circle.
        let g = Dvbs2Rate::R3_4.gamma_16apsk();
        let p = PointSet::dvbs2_16apsk(g).unwrap();
        assert!((energy(&p) - 1.0).abs() < 1e-6);
        let r1 = p.points()[12].norm();
        for l in 0..16 {
            let r = p.points()[l].norm() / r1;
            assert!((r - if l >= 12 { 1.0 } else { g as f32 }).abs() < 1e-5);
        }
        let outer = PointSet {
            points: p.points()[..12].to_vec(),
            bits: 4,
            shape: Shape::Apsk,
        };
        let o = by_angle(&outer);
        for k in 0..12 {
            assert_eq!((o[k] ^ o[(k + 1) % 12]).count_ones(), 1, "{:?}", o);
        }
        // 32APSK: 4 + 12 + 16 points at the right radii, each ring equally spaced.
        let (g1, g2) = Dvbs2Rate::R4_5.gammas_32apsk().unwrap();
        let p = PointSet::dvbs2_32apsk(g1, g2).unwrap();
        assert!((energy(&p) - 1.0).abs() < 1e-6);
        let r1 = p.points()[17].norm();
        let mut count = [0usize; 3];
        for z in p.points() {
            let r = z.norm() / r1;
            let ring = if (r - 1.0).abs() < 1e-4 {
                0
            } else if (r - g1 as f32).abs() < 1e-4 {
                1
            } else {
                assert!((r - g2 as f32).abs() < 1e-4);
                2
            };
            count[ring] += 1;
        }
        assert_eq!(count, [4, 12, 16]);
        let mut seen = std::collections::BTreeSet::new();
        for z in p.points() {
            seen.insert(((z.re * 1e4) as i32, (z.im * 1e4) as i32));
        }
        assert_eq!(seen.len(), 32);
        assert!(Dvbs2Rate::R2_3.gammas_32apsk().is_none());
        assert!(PointSet::dvbs2_16apsk(0.5).is_err());
    }

    #[test]
    fn vsb_pam_psk_and_errors() {
        let v = PointSet::vsb8();
        assert_eq!(at(&v, 0, 21f32.sqrt()).0, -7);
        assert_eq!(at(&v, 7, 21f32.sqrt()).0, 7);
        assert_eq!(at(&v, 4, 21f32.sqrt()).0, 1);
        assert!((energy(&v) - 1.0).abs() < 1e-6);
        let p = PointSet::pam(4).unwrap();
        assert_eq!(
            [0, 1, 3, 2].map(|l| at(&p, l, 5f32.sqrt()).0),
            [-3, -1, 1, 3]
        );
        let q = PointSet::psk(8, 0.0).unwrap();
        assert!((q.points()[gray(3) as usize] - polar(1.0, 3.0 * PI / 4.0)).norm() < 1e-6);
        assert_eq!(PointSet::bpsk().points()[0], Complex32::new(1.0, 0.0));
        assert!(PointSet::qam(32, QamMap::DVB).is_err());
        assert!(PointSet::psk(3, 0.0).is_err());
        assert!(PointSet::apsk(&[]).is_err());
        assert!(PointSet::new(vec![Complex32::default(); 4], Shape::Other).is_err());
        let r = q.relabelled(&[1, 0, 2, 3, 4, 5, 6, 7]).unwrap();
        assert_eq!(r.points()[0], q.points()[1]);
        assert!(q.relabelled(&[0, 0, 2, 3, 4, 5, 6, 7]).is_err());
        let mut out = Vec::new();
        assert!(q.map_bits(&[1, 0], &mut out).is_err());
    }
}
