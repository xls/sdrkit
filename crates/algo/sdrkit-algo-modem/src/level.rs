//! Signal quality: SNR estimators (blind, data-aided, decision-directed, from pilots, from a spectrum) and EVM and MER from decisions.
//!
//! | Estimator | Needs | Notes |
//! |---|---|---|
//! | [`M2m4`] | symbols only | blind, from the second and fourth moments; exact for PSK (`ka = 1`), any constellation through its kurtosis `ka` (D. R. Pauluzzi and N. C. Beaulieu, IEEE Trans. Commun. 48(10), 2000) |
//! | [`DataAided`] | the symbols sent (a preamble, pilots) | least-squares gain, residual noise; unbiased at any SNR |
//! | [`DataAided::push_decided`] | a [`Constellation`] | decision-directed: as data-aided with decisions; reads high below the SNR where decisions fail |
//! | [`second_difference`] | pilots on a smooth channel | noise from `p[k-1] - 2 p[k] + p[k+1]` (variance `6 sigma^2`): the OFDM pilot method |
//! | [`band_ratio`] | a power spectrum | in-band power over the noise per bin outside |
//! | [`M2m4::with_memory`] | symbols only | M2M4 with exponential weights, for a level that changes |
//! | [`cn0_dbhz`] | the SNR of correlator outputs and their time | C/N0 in dB-Hz |
//! | [`Evm`] | decided or known symbols | EVM (RMS, % of the mean ideal power) and MER (dB), as ETSI TR 101 290 defines MER |
//!
//! SNR here is symbol energy over complex noise variance (`Es/N0` for symbols at one sample per symbol).
//!
//! ```
//! use sdrkit_algo_modem::level::M2m4;
//! use sdrkit_algo_modem::Complex32;
//! // QPSK with no noise: infinite SNR; the estimate says so.
//! let s = std::f32::consts::FRAC_1_SQRT_2;
//! let x: Vec<Complex32> = (0..100).map(|k| Complex32::new(if k % 2 == 0 { s } else { -s }, s)).collect();
//! let mut e = M2m4::psk();
//! e.push_slice(&x);
//! assert!(e.estimate().unwrap().snr_db > 60.0);
//! ```

use crate::{Complex32, Constellation, ModemError, Param};
use sdrkit_algo_dsp::stats::plain_kurtosis_iq;

/// An SNR estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snr {
    /// Signal power per symbol.
    pub signal: f64,
    /// Noise variance per symbol (complex).
    pub noise: f64,
    /// `signal / noise`, in dB (capped at 100 dB when the noise reads 0).
    pub snr_db: f64,
}

impl Snr {
    fn new(signal: f64, noise: f64) -> Option<Snr> {
        if !(signal > 0.0 && signal.is_finite() && noise.is_finite()) {
            return None;
        }
        let noise = noise.max(0.0);
        let snr_db = if noise > 0.0 {
            (10.0 * (signal / noise).log10()).min(100.0)
        } else {
            100.0
        };
        Some(Snr {
            signal,
            noise,
            snr_db,
        })
    }

    /// The carrier-to-noise-density ratio C/N0 in dB-Hz, when this SNR is of correlator outputs that each integrate `coherent_s`
    /// seconds: `snr_db - 10 log10(coherent_s)` (the noise bandwidth of one output is `1 / coherent_s`). Returns `None` unless
    /// `coherent_s` is positive and finite.
    pub fn cn0_dbhz(&self, coherent_s: f64) -> Option<f64> {
        cn0_dbhz(self.ratio(), coherent_s)
    }

    /// `signal / noise` as a ratio.
    pub fn ratio(&self) -> f64 {
        10f64.powf(self.snr_db / 10.0)
    }
}

/// The carrier-to-noise-density ratio C/N0 in dB-Hz from the SNR `snr` (a ratio, not dB) of correlator outputs that each
/// integrate `coherent_s` seconds: `10 log10(snr / coherent_s)`. Returns `None` unless `snr` and `coherent_s` are positive and
/// finite.
pub fn cn0_dbhz(snr: f64, coherent_s: f64) -> Option<f64> {
    if !(snr > 0.0 && snr.is_finite() && coherent_s > 0.0 && coherent_s.is_finite()) {
        return None;
    }
    Some(10.0 * (snr / coherent_s).log10())
}

/// The M2M4 blind SNR estimator, accumulating moments over the symbols pushed (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M2m4 {
    ka: f64,
    /// The sums of the moments, or their means with an exponential memory.
    m2: f64,
    m4: f64,
    n: u64,
    /// The exponential memory in symbols, or 0 for plain sums.
    memory: u64,
}

impl M2m4 {
    /// For a constellation of kurtosis `ka = E|a|^4 / (E|a|^2)^2` (1 for PSK, 1.32 for 16-QAM, 1.381 for 64-QAM). Errors unless
    /// `1 <= ka < 2` (complex Gaussian noise has 2: a signal of kurtosis 2 cannot be told from noise).
    pub fn new(ka: f64) -> Result<Self, ModemError> {
        if !(1.0..2.0).contains(&ka) {
            return Param::Kurtosis.range(1.0, 2.0, ka);
        }
        Ok(M2m4 {
            ka,
            m2: 0.0,
            m4: 0.0,
            n: 0,
            memory: 0,
        })
    }

    /// For any PSK (constant modulus).
    pub fn psk() -> Self {
        M2m4 {
            ka: 1.0,
            m2: 0.0,
            m4: 0.0,
            n: 0,
            memory: 0,
        }
    }

    /// The same estimator with the moments weighted exponentially, for a level that changes (the C/N0 of a tracked signal). The
    /// first `memory` symbols give a plain mean. After that, each new symbol has the weight `1 / memory`. Errors if `memory` is
    /// 0. The sums start again.
    pub fn with_memory(mut self, memory: u32) -> Result<Self, ModemError> {
        if memory == 0 {
            return Param::Memory.at_least(1.0, 0.0);
        }
        self.memory = memory as u64;
        self.reset();
        Ok(self)
    }

    /// The number of symbols pushed since the last reset.
    pub fn count(&self) -> u64 {
        self.n
    }

    /// For the points of `c`, equally likely.
    pub fn for_constellation(c: &Constellation) -> Result<Self, ModemError> {
        Self::new(plain_kurtosis_iq(c.points()).unwrap_or(f64::NAN))
    }

    /// The constellation kurtosis in use.
    pub fn kurtosis(&self) -> f64 {
        self.ka
    }

    /// Adds one symbol (non-finite ones are skipped).
    #[inline]
    pub fn push(&mut self, z: Complex32) {
        let p = z.norm_sqr() as f64;
        if !p.is_finite() {
            return;
        }
        self.n += 1;
        if self.memory == 0 {
            self.m2 += p;
            self.m4 += p * p;
        } else {
            let a = 1.0 / self.n.min(self.memory) as f64;
            self.m2 += a * (p - self.m2);
            self.m4 += a * (p * p - self.m4);
        }
    }

    /// Adds symbols.
    pub fn push_slice(&mut self, x: &[Complex32]) {
        for &z in x {
            self.push(z);
        }
    }

    /// Forgets the symbols pushed.
    pub fn reset(&mut self) {
        self.m2 = 0.0;
        self.m4 = 0.0;
        self.n = 0;
    }

    /// The estimate from the symbols so far; `None` before 2 symbols or when the moments are inconsistent (too few symbols at very
    /// low SNR).
    pub fn estimate(&self) -> Option<Snr> {
        if self.n < 2 {
            return None;
        }
        let (m2, m4) = if self.memory == 0 {
            (self.m2 / self.n as f64, self.m4 / self.n as f64)
        } else {
            (self.m2, self.m4)
        };
        // Complex Gaussian noise: kw = 2. S = sqrt((4 - 2 ka) M2^2 + (ka - 2) M4) / (2 - ka).
        let d = (4.0 - 2.0 * self.ka) * m2 * m2 + (self.ka - 2.0) * m4;
        let s = d.max(0.0).sqrt() / (2.0 - self.ka);
        Snr::new(s, m2 - s)
    }
}

/// Data-aided (and decision-directed) SNR: the least-squares complex gain `g = sum r a* / sum |a|^2` between received `r` and sent
/// `a`, the signal `|g|^2 E|a|^2`, and the noise `sum |r - g a|^2 / (N - 1)`. Sums are kept, so the estimate covers every symbol
/// pushed since the last reset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DataAided {
    ra: (f64, f64),
    aa: f64,
    rr: f64,
    n: u64,
}

impl DataAided {
    /// An empty estimator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one received symbol and the symbol sent.
    #[inline]
    pub fn push(&mut self, r: Complex32, sent: Complex32) {
        if !(r.re.is_finite() && r.im.is_finite() && sent.re.is_finite() && sent.im.is_finite()) {
            return;
        }
        let c = r * sent.conj();
        self.ra.0 += c.re as f64;
        self.ra.1 += c.im as f64;
        self.aa += sent.norm_sqr() as f64;
        self.rr += r.norm_sqr() as f64;
        self.n += 1;
    }

    /// Adds one received symbol with its decision on `c` (decision-directed; biased upward when decisions are often wrong).
    #[inline]
    pub fn push_decided(&mut self, r: Complex32, c: &Constellation) {
        self.push(r, c.nearest(r));
    }

    /// Adds slices of received and sent symbols (the shorter length counts).
    pub fn push_slice(&mut self, r: &[Complex32], sent: &[Complex32]) {
        for (&a, &b) in r.iter().zip(sent) {
            self.push(a, b);
        }
    }

    /// Forgets the symbols.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The least-squares gain `g` (the channel seen by the symbols: its phase is the carrier phase error).
    pub fn gain(&self) -> Option<Complex32> {
        (self.aa > 0.0)
            .then(|| Complex32::new((self.ra.0 / self.aa) as f32, (self.ra.1 / self.aa) as f32))
    }

    /// The estimate; `None` before 2 symbols or with no signal sent.
    pub fn estimate(&self) -> Option<Snr> {
        if self.n < 2 || self.aa <= 0.0 {
            return None;
        }
        let g2 = (self.ra.0 * self.ra.0 + self.ra.1 * self.ra.1) / (self.aa * self.aa);
        // sum |r - g a|^2 = sum |r|^2 - |sum r a*|^2 / sum |a|^2.
        let resid = self.rr - g2 * self.aa;
        let n = self.n as f64;
        Snr::new(g2 * self.aa / n, resid / (n - 1.0))
    }
}

/// The noise variance from the second differences of pilots on a channel that changes slowly from one pilot to the next:
/// `E|p[k-1] - 2 p[k] + p[k+1]|^2 / 6` (each difference holds `6 sigma^2` of white noise). `None` for fewer than 3 pilots.
pub fn second_difference(pilots: &[Complex32]) -> Option<f64> {
    if pilots.len() < 3 {
        return None;
    }
    let mut s = 0.0f64;
    let mut n = 0usize;
    for w in pilots.windows(3) {
        let d = w[0] - w[1] * 2.0 + w[2];
        let p = d.norm_sqr() as f64;
        if p.is_finite() {
            s += p;
            n += 1;
        }
    }
    (n > 0).then(|| s / (6.0 * n as f64))
}

/// SNR from a power spectrum: the power in `power[band]` less the noise in those bins, over the noise in the band, with the noise
/// per bin read from `noise_per_bin` (for example a `sdrkit_algo_dsp::floor::BinMedian` of the bins outside). `None` when the band is
/// empty or holds no more than noise.
pub fn band_ratio(power: &[f32], band: std::ops::Range<usize>, noise_per_bin: f32) -> Option<Snr> {
    let band = band.start.min(power.len())..band.end.min(power.len());
    let bins = band.len();
    if bins == 0 || noise_per_bin.is_nan() || noise_per_bin <= 0.0 {
        return None;
    }
    let total: f64 = power[band]
        .iter()
        .filter(|p| p.is_finite())
        .map(|&p| p as f64)
        .sum();
    let noise = noise_per_bin as f64 * bins as f64;
    Snr::new(total - noise, noise)
}

/// EVM and MER from received symbols and their ideal points (decided or known).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Evm {
    err: f64,
    ideal: f64,
    peak_err: f64,
    n: u64,
}

impl Evm {
    /// An empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a received symbol and its ideal point.
    #[inline]
    pub fn push(&mut self, r: Complex32, ideal: Complex32) {
        let e = (r - ideal).norm_sqr() as f64;
        let p = ideal.norm_sqr() as f64;
        if e.is_finite() && p.is_finite() {
            self.err += e;
            self.ideal += p;
            self.peak_err = self.peak_err.max(e);
            self.n += 1;
        }
    }

    /// Adds a received symbol decided on `c`.
    #[inline]
    pub fn push_decided(&mut self, r: Complex32, c: &Constellation) {
        self.push(r, c.nearest(r));
    }

    /// Forgets the symbols.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Symbols counted.
    pub fn count(&self) -> u64 {
        self.n
    }

    /// RMS EVM in percent of the RMS ideal amplitude; `None` with no symbols.
    pub fn evm_percent(&self) -> Option<f64> {
        (self.n > 0 && self.ideal > 0.0).then(|| 100.0 * (self.err / self.ideal).sqrt())
    }

    /// The largest single error vector, percent of the RMS ideal amplitude.
    pub fn peak_evm_percent(&self) -> Option<f64> {
        (self.n > 0 && self.ideal > 0.0)
            .then(|| 100.0 * (self.peak_err * self.n as f64 / self.ideal).sqrt())
    }

    /// MER in dB: `10 log10(sum |ideal|^2 / sum |error|^2)` (ETSI TR 101 290 clause 9.15), capped at 100 dB.
    pub fn mer_db(&self) -> Option<f64> {
        if self.n == 0 || self.ideal <= 0.0 {
            return None;
        }
        Some(if self.err > 0.0 {
            (10.0 * (self.ideal / self.err).log10()).min(100.0)
        } else {
            100.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
    use sdrkit_algo_sim::rng::Rng;

    fn symbols(set: &PointSet, n: usize, n0: f64, seed: u64) -> (Vec<Complex32>, Vec<Complex32>) {
        let mut r = Rng::new(seed);
        let sent: Vec<Complex32> = (0..n)
            .map(|_| set.points()[r.below(set.len() as u64) as usize])
            .collect();
        let rx = sent
            .iter()
            .map(|&s| s * Complex32::from_polar(0.8, 0.4) + r.complex_normal(n0))
            .collect();
        (sent, rx)
    }

    #[test]
    fn estimators_read_the_snr_set() {
        // 0.8^2 signal power over n0.
        for (name, set) in [
            ("qpsk", PointSet::qpsk()),
            ("16qam", PointSet::qam(16, QamMap::DVB).unwrap()),
        ] {
            let c = Constellation::new(set.clone()).unwrap();
            for snr_db in [3.0f64, 10.0, 20.0] {
                let n0 = 0.64 / 10f64.powf(snr_db / 10.0);
                let (sent, rx) = symbols(&set, 40_000, n0, 1);
                let mut m = M2m4::for_constellation(&c).unwrap();
                m.push_slice(&rx);
                let est = m.estimate().unwrap().snr_db;
                assert!((est - snr_db).abs() < 0.5, "{name} M2M4 at {snr_db}: {est}");
                let mut da = DataAided::new();
                da.push_slice(&rx, &sent);
                let est = da.estimate().unwrap().snr_db;
                assert!(
                    (est - snr_db).abs() < 0.2,
                    "{name} data-aided at {snr_db}: {est}"
                );
                let g = da.gain().unwrap();
                assert!((g - Complex32::from_polar(0.8, 0.4)).norm() < 0.02);
                if snr_db >= 20.0 {
                    // Decision-directed agrees once decisions are right (after the phase is removed).
                    let mut dd = DataAided::new();
                    for &r in &rx {
                        dd.push_decided(r * Complex32::from_polar(1.25, -0.4), &c);
                    }
                    let est = dd.estimate().unwrap().snr_db;
                    assert!((est - snr_db).abs() < 0.5, "{name} DD at {snr_db}: {est}");
                }
            }
        }
        assert!(M2m4::new(2.0).is_err());
        assert!(M2m4::psk().estimate().is_none());
    }

    #[test]
    fn evm_mer_second_difference_and_band_ratio() {
        let set = PointSet::qam(64, QamMap::DVB).unwrap();
        let c = Constellation::new(set.clone()).unwrap();
        let mut r = Rng::new(3);
        let n0 = 1e-3; // MER 30 dB
        let mut e = Evm::new();
        for _ in 0..20_000 {
            let s = set.points()[r.below(64) as usize];
            e.push_decided(s + r.complex_normal(n0), &c);
        }
        assert!((e.mer_db().unwrap() - 30.0).abs() < 0.2);
        assert!((e.evm_percent().unwrap() - 100.0 * n0.sqrt()).abs() < 0.1);
        assert!(e.peak_evm_percent().unwrap() > e.evm_percent().unwrap());
        // Pilots on a slowly turning channel.
        let pilots: Vec<Complex32> = (0..5000)
            .map(|k| Complex32::from_polar(1.0, 0.001 * k as f32) + r.complex_normal(0.01))
            .collect();
        let v = second_difference(&pilots).unwrap();
        assert!((v / 0.01 - 1.0).abs() < 0.06, "{v}");
        assert!(second_difference(&pilots[..2]).is_none());
        // A band holding 10 units over a floor of 1 per bin in 10 bins: SNR 0 dB.
        let mut p = vec![1.0f32; 64];
        p[20..30].iter_mut().for_each(|v| *v = 2.0);
        let s = band_ratio(&p, 20..30, 1.0).unwrap();
        assert!(s.snr_db.abs() < 1e-9);
        assert!(band_ratio(&p, 70..80, 1.0).is_none());
        assert!(Evm::new().mer_db().is_none());
    }

    /// C/N0 by M2M4 (D. R. Pauluzzi and N. C. Beaulieu, IEEE Trans. Commun. 48(10), 2000, eq. for constant-modulus signals:
    /// `S = sqrt(2 M2^2 - M4)`, `N = M2 - S`) on synthetic 1 ms BPSK correlator outputs of known C/N0: amplitude `A`, complex noise
    /// variance `N`, so `C/N0 = A^2 / (N T)`.
    #[test]
    fn exponential_m2m4_returns_cn0_of_a_synthetic_signal() {
        let t = 1e-3;
        for (cn0, seed) in [(35.0, 1), (45.0, 2)] {
            let n0 = 1.0;
            let a = (10f64.powf(cn0 / 10.0) * t * n0).sqrt() as f32;
            let mut r = Rng::new(seed);
            let mut e = M2m4::psk().with_memory(2000).unwrap();
            for k in 0..20_000 {
                let d = if (k / 20) % 3 == 0 { -a } else { a };
                e.push(
                    Complex32::new(d, 0.0) * Complex32::from_polar(1.0, 0.3) + r.complex_normal(n0),
                );
            }
            assert_eq!(e.count(), 20_000);
            let got = e.estimate().unwrap().cn0_dbhz(t).unwrap();
            assert!((got - cn0).abs() < 0.5, "{cn0} dB-Hz: {got}");
        }
        // Before the memory fills, the weights are a plain mean: the same estimate as plain sums.
        let mut r = Rng::new(3);
        let x: Vec<Complex32> = (0..300)
            .map(|_| Complex32::new(2.0, 0.0) + r.complex_normal(1.0))
            .collect();
        let mut plain = M2m4::psk();
        let mut ew = M2m4::psk().with_memory(500).unwrap();
        plain.push_slice(&x);
        ew.push_slice(&x);
        let (p, w) = (plain.estimate().unwrap(), ew.estimate().unwrap());
        assert!((p.snr_db - w.snr_db).abs() < 1e-9);
        // A constant modulus of 2 has M2 = 4 and M4 = 16, so S = 4 and N = 0.
        let mut c = M2m4::psk().with_memory(10).unwrap();
        for k in 0..30 {
            c.push(Complex32::from_polar(2.0, k as f32));
        }
        let s = c.estimate().unwrap();
        assert!((s.signal - 4.0).abs() < 1e-4 && s.snr_db > 40.0, "{s:?}");
        assert!(M2m4::psk().with_memory(0).is_err());
        assert_eq!(s.cn0_dbhz(0.0), None);
    }
}
