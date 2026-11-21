//! GFSK and GMSK: the discriminator receiver, least-squares channel estimation, and maximum-likelihood sequence estimation (MLSE)
//! with hard (Viterbi) or soft (max-log BCJR) output.
//!
//! **Discriminator receiver** ([`GfskDemod`]): [`DiscriminatorLevels`] (FM discriminator, no library `atan2`; an
//! integrate-and-dump over one symbol; the shared symbol synchroniser [`crate::timing::SymbolSync`] with the zero-crossing
//! detector on the real levels), and a [`LevelSlicer`] that removes the frequency offset as a DC level and returns
//! LLRs. Cheap, non-coherent, any BT and index: BLE, ANT, DECT, FLARM, Z-Wave, Wi-SUN, POCSAG on IQ, cubesat GMSK.
//!
//! **MLSE** ([`Mlse`]): the received samples (one per symbol) are modelled as `y_k = sum_m g_m a_(k-m) + noise` for symbols
//! `a = +1` (bit 0) or `-1` (bit 1) and a short complex channel `g` of `L` taps; the Viterbi algorithm over the `2^(L-1)`-state
//! trellis (`sdrkit_algo_fec::conv::TrellisDecoder`) finds the most likely bits, and the max-log BCJR outputs true LLRs of each bit
//! (positive means 0) for a following decoder. The channel comes from a training sequence by least squares ([`ls_channel`]) or,
//! for GMSK, from the pulse ([`gmsk_taps`]). GSM uses 5 taps (16 states), AIS and BLE 3 (4 states); the same type serves any
//! ISI channel with binary symbols (the ADS-B 3-tap model, an equaliser's MLSE back end).
//!
//! **Streaming MLSE** ([`MlseStream`]): the same soft output over an endless stream, by the sliding-window BCJR (the forward
//! recursion runs on; the backward one starts a few symbols past each block). [`GfskMlseDemod`] puts it behind the discriminator
//! front end ([`crate::fsk::DiscriminatorLevels`]) with the ISI the integrate-and-dump leaves ([`discriminator_taps`]): a
//! non-coherent GFSK/GMSK receiver better than slicing at low BT.
//!
//! **GMSK as a linear signal.** By Laurent's decomposition a GMSK signal is close to a linear modulation with the main pulse
//! `C0`: `s(t) ~ sum_k b_k C0(t - k T)` with `b_k = j^(sum a)` (P. A. Laurent, "Exact and approximate construction of digital phase
//! modulations by superposition of amplitude modulated pulses", IEEE Trans. Commun. 34(2), 1986). Turning symbol `k` back by
//! `j^-k` ([`derotate`]) leaves real symbols `c_k = c_(k-1) a_k`, so the MLSE decides `c` and the data are `a_k = c_k c_(k-1)` (GSM
//! precodes differentially, so its `c` is the data).
//!
//! ```
//! use sdrkit_algo_modem::gmsk::Mlse;
//! use sdrkit_algo_modem::Complex32;
//! // A 3-tap channel and a few noiseless symbols.
//! let g = [Complex32::new(0.3, 0.0), Complex32::new(1.0, 0.0), Complex32::new(0.3, 0.0)];
//! let bits = [0u8, 1, 1, 0, 1, 0, 0, 0, 1, 1];
//! let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
//! let y: Vec<Complex32> = (0..bits.len())
//!     .map(|k| (0..3).filter(|&m| k >= m).map(|m| g[m] * a[k - m]).sum())
//!     .collect();
//! let mut mlse = Mlse::new(&g, 64).unwrap();
//! let mut out = [0u8; 10];
//! mlse.decode(&y, 0.1, Some(0), &mut out).unwrap();
//! assert_eq!(out, bits);
//! ```

use crate::fsk::{DiscriminatorConfig, DiscriminatorLevels, LevelSlicer};
use crate::timing::{SymbolSync, ZeroCrossing};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::pulse;
use sdrkit_algo_fec::conv::{Ends, LabelMetrics, Trellis, TrellisDecoder};

/// The most taps [`ls_channel`] and [`Mlse`] take.
pub const MAX_TAPS: usize = 8;

/// The result of a least-squares channel fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LsFit {
    /// The mean squared residual: the noise variance `E|n|^2`.
    pub noise_var: f32,
    /// The constant term, when one was fitted (a frequency offset left as DC after a discriminator), else 0.
    pub dc: Complex32,
}

/// Least-squares estimate of a channel of `taps.len()` taps (1 to [`MAX_TAPS`]) from received samples `y` and the known symbols
/// `s` that caused them: minimises `sum_k |y_k - sum_m g_m s_(k-m) - dc|^2` over `k` from `taps - 1` to the end, writing `g` to
/// `taps`. With `fit_dc` a constant is fitted too (the level model of a GFSK receiver, whose offset is a DC). Errors when the lengths
/// differ, there are too few equations, or the symbols do not excite every tap (a singular system).
#[allow(clippy::needless_range_loop)] // matrix code reads best with indices
pub fn ls_channel(
    y: &[Complex32],
    s: &[Complex32],
    taps: &mut [Complex32],
    fit_dc: bool,
) -> Result<LsFit, ModemError> {
    let l = taps.len();
    if l == 0 || l > MAX_TAPS {
        return Param::Taps.range(1.0, MAX_TAPS as f64, l as f64);
    }
    if y.len() != s.len() {
        return Param::Training.length(y.len(), s.len());
    }
    let p = l + fit_dc as usize;
    if y.len() < l - 1 + p + 1 {
        return Err(ModemError::InputTooShort { need: l + p });
    }
    // Normal equations R g = r, built in f64.
    const P: usize = MAX_TAPS + 1;
    let mut r = [[num_complex::Complex64::default(); P + 1]; P];
    let regressor = |k: usize, i: usize| -> num_complex::Complex64 {
        if i < l {
            let v = s[k - i];
            num_complex::Complex64::new(v.re as f64, v.im as f64)
        } else {
            num_complex::Complex64::new(1.0, 0.0)
        }
    };
    for k in l - 1..y.len() {
        let yk = num_complex::Complex64::new(y[k].re as f64, y[k].im as f64);
        for i in 0..p {
            let xi = regressor(k, i).conj();
            for j in 0..p {
                r[i][j] += xi * regressor(k, j);
            }
            r[i][p] += xi * yk;
        }
    }
    if r.iter()
        .flatten()
        .any(|v| !(v.re.is_finite() && v.im.is_finite()))
    {
        return Err(ModemError::NotFinite {
            param: Param::Input,
        });
    }
    // Gaussian elimination with partial pivoting.
    let scale = (0..p)
        .map(|i| r[i][i].norm())
        .fold(0.0f64, f64::max)
        .max(1e-300);
    for c in 0..p {
        let piv = (c..p)
            .max_by(|&a, &b| r[a][c].norm().total_cmp(&r[b][c].norm()))
            .unwrap_or(c);
        if r[piv][c].norm() < 1e-9 * scale {
            return Param::Training.degenerate();
        }
        r.swap(c, piv);
        let d = r[c][c];
        for j in c..=p {
            r[c][j] /= d;
        }
        for i in 0..p {
            if i != c {
                let f = r[i][c];
                if f != num_complex::Complex64::default() {
                    for j in c..=p {
                        let v = r[c][j];
                        r[i][j] -= f * v;
                    }
                }
            }
        }
    }
    for (t, row) in taps.iter_mut().zip(&r) {
        *t = Complex32::new(row[p].re as f32, row[p].im as f32);
    }
    let dc = if fit_dc {
        Complex32::new(r[l][p].re as f32, r[l][p].im as f32)
    } else {
        Complex32::default()
    };
    let mut e = 0f64;
    for k in l - 1..y.len() {
        let mut m = dc;
        for (i, t) in taps.iter().enumerate() {
            m += t * s[k - i];
        }
        e += (y[k] - m).norm_sqr() as f64;
    }
    let dof = (y.len() - (l - 1)).saturating_sub(p).max(1);
    Ok(LsFit {
        noise_var: (e / dof as f64) as f32,
        dc,
    })
}

/// The main Laurent pulse `C0` of GMSK (h = 1/2) with bandwidth-time product `bt`: `C0(t) = prod_{i=0}^{L-1} S(t + i T)`, `S` the
/// sine of the phase pulse (`sin(pi q(t))` rising over the `L` symbols of the frequency pulse, mirrored after), tabulated once.
struct LaurentC0 {
    q: Vec<f64>,
    res: usize,
    lsym: usize,
}

impl LaurentC0 {
    fn new(bt: f64) -> Self {
        // Frequency pulse length in symbols (covering the Gaussian to well below 1e-6).
        let sigma = std::f64::consts::LN_2.sqrt() / (std::f64::consts::TAU * bt);
        let lsym = (1.0 + 10.0 * sigma).ceil() as usize | 1;
        let res = 512usize;
        // Phase pulse rising from 0 to 1/2 (half the integral of the unit-area frequency pulse).
        let mut q = vec![0f64; lsym * res + 1];
        let mut acc = 0.0;
        for (i, qi) in q.iter_mut().enumerate().skip(1) {
            let t = (i as f64 - 0.5) / res as f64 - lsym as f64 / 2.0;
            acc += pulse::gaussian_freq(t, bt) / res as f64;
            *qi = acc;
        }
        let end = q[lsym * res];
        q.iter_mut().for_each(|v| *v *= 0.5 / end);
        LaurentC0 { q, res, lsym }
    }

    fn q_at(&self, t: f64) -> f64 {
        let lt = self.lsym as f64;
        if t <= 0.0 {
            0.0
        } else if t >= lt {
            0.5
        } else {
            let x = t * self.res as f64;
            let i = x.floor() as usize;
            let f = x - i as f64;
            self.q[i] + f * (self.q[(i + 1).min(self.q.len() - 1)] - self.q[i])
        }
    }

    fn s(&self, t: f64) -> f64 {
        let lt = self.lsym as f64;
        let psi = if t <= 0.0 || t >= 2.0 * lt {
            0.0
        } else if t <= lt {
            std::f64::consts::PI * self.q_at(t)
        } else {
            std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * self.q_at(t - lt)
        };
        psi.sin()
    }

    /// `C0` at `t` symbols from its peak.
    fn at(&self, t: f64) -> f64 {
        let t = t + (self.lsym as f64 + 1.0) / 2.0;
        (0..self.lsym).map(|i| self.s(t + i as f64)).product()
    }
}

/// The symbol-spaced taps of the main Laurent pulse `C0` of GMSK with bandwidth-time product `bt` (h = 1/2), `taps` of them (odd,
/// 1 to [`MAX_TAPS`]) centred on the pulse's peak, scaled so the centre tap is 1: the shape of the channel an [`Mlse`] sees for a
/// GMSK signal sampled at the peaks and turned back by [`derotate`] (before any receive filter).
pub fn gmsk_taps(bt: f64, taps: usize) -> Result<Vec<f32>, ModemError> {
    Param::Bt.check_positive(bt)?;
    if taps == 0 || taps > MAX_TAPS {
        return Param::Taps.range(1.0, MAX_TAPS as f64, taps as f64);
    }
    if taps.is_multiple_of(2) {
        return Param::Taps.not_allowed(taps as f64);
    }
    let c = LaurentC0::new(bt);
    let mid = c.at(0.0);
    let half = (taps / 2) as f64;
    Ok((0..taps)
        .map(|m| (c.at(m as f64 - half) / mid) as f32)
        .collect())
}

/// A receive filter matched to `C0` for GMSK of `bt` at `sps` samples per symbol: symmetric, odd length, unit energy, centred on
/// the pulse's peak (delay `(len - 1) / 2`), over the pulse's support of `+-span` symbols (2 is plenty for BT 0.3 and above).
pub fn gmsk_matched(bt: f64, sps: f64, span: f64) -> Result<Vec<f32>, ModemError> {
    Param::Bt.check_positive(bt)?;
    Param::Sps.check_in(sps, 1.0, 256.0)?;
    Param::Span.check_above(span, 0.0, 8.0)?;
    let c = LaurentC0::new(bt);
    let half = (span * sps).floor() as i64;
    let mut h: Vec<f64> = (-half..=half).map(|k| c.at(k as f64 / sps)).collect();
    let e = h.iter().map(|v| v * v).sum::<f64>().sqrt();
    h.iter_mut().for_each(|v| *v /= e);
    Ok(h.into_iter().map(|v| v as f32).collect())
}

/// Turns symbol samples back by `j^-(k0 + k)` (no trigonometry: quarter turns), so a GMSK or MSK signal sampled once per symbol
/// becomes real symbols (module documentation). `out` must be as long as `y`.
pub fn derotate(y: &[Complex32], k0: u64, out: &mut [Complex32]) -> Result<(), ModemError> {
    if out.len() < y.len() {
        return Err(ModemError::OutputTooShort { need: y.len() });
    }
    for (k, (o, &v)) in out.iter_mut().zip(y).enumerate() {
        *o = match (k0 + k as u64) & 3 {
            0 => v,
            1 => Complex32::new(v.im, -v.re),
            2 => -v,
            _ => Complex32::new(-v.im, v.re),
        };
    }
    Ok(())
}

/// Maximum-likelihood sequence estimation for binary symbols over a short ISI channel (module documentation). Sized at `new` for
/// frames up to `max_steps` symbols; decoding allocates nothing.
#[derive(Clone, Debug)]
pub struct Mlse {
    taps: usize,
    trellis: Trellis,
    dec: TrellisDecoder,
    expected: Vec<Complex32>,
    metrics: Vec<i32>,
    alpha: Vec<f32>,
    beta: Vec<f32>,
    max_steps: usize,
}

impl Mlse {
    /// An estimator for channel `taps` (`g_0` multiplies the newest symbol; 1 to [`MAX_TAPS`] taps) and frames up to `max_steps`.
    pub fn new(taps: &[Complex32], max_steps: usize) -> Result<Self, ModemError> {
        let l = taps.len();
        Param::Taps.check_count(l, 1, MAX_TAPS)?;
        Param::Steps.check_count(max_steps, 1, 1 << 20)?;
        let states = 1usize << (l - 1);
        let mut next = Vec::with_capacity(2 * states);
        let mut label = Vec::with_capacity(2 * states);
        for st in 0..states {
            for b in 0..2usize {
                let reg = (b << (l - 1)) | st;
                next.push((reg >> 1) as u16);
                label.push(reg as u16);
            }
        }
        let trellis = Trellis::new(states, 2, 1 << l, next, label)?;
        let dec = TrellisDecoder::new(&trellis, max_steps)?;
        let mut m = Mlse {
            taps: l,
            trellis,
            dec,
            expected: vec![Complex32::default(); 1 << l],
            metrics: vec![0; max_steps << l],
            alpha: vec![0.0; (max_steps + 1) * states],
            beta: vec![0.0; 2 * states],
            max_steps,
        };
        m.set_channel(taps)?;
        Ok(m)
    }

    /// Number of trellis states, `2^(taps - 1)`.
    pub fn states(&self) -> usize {
        self.trellis.states()
    }

    /// Replaces the channel (the same number of taps).
    pub fn set_channel(&mut self, taps: &[Complex32]) -> Result<(), ModemError> {
        let l = self.taps;
        if taps.len() != l {
            return Param::Taps.length(l, taps.len());
        }
        if taps.iter().any(|t| !(t.re.is_finite() && t.im.is_finite())) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        for (reg, e) in self.expected.iter_mut().enumerate() {
            // Bit L-1 is the newest symbol (tap 0), bit L-1-m the symbol m back.
            *e = (0..l)
                .map(|m| {
                    let b = (reg >> (l - 1 - m)) & 1;
                    taps[m] * if b == 0 { 1.0 } else { -1.0 }
                })
                .sum();
        }
        Ok(())
    }

    fn check(&self, n: usize, noise_var: f32, out: usize) -> Result<(), ModemError> {
        if n > self.max_steps {
            return Param::Input.range(0.0, self.max_steps as f64, n as f64);
        }
        if out < n {
            return Err(ModemError::OutputTooShort { need: n });
        }
        if !(noise_var > 0.0 && noise_var.is_finite()) {
            return Param::NoiseVariance.positive(noise_var);
        }
        Ok(())
    }

    /// The most likely bits (0 for `+1`) of the samples `y` into `bits`. `start` is the trellis state before the first sample (the
    /// last `taps - 1` bits, newest in the top bit; for example 0 after a run of zeros) or `None` when unknown.
    pub fn decode(
        &mut self,
        y: &[Complex32],
        noise_var: f32,
        start: Option<u32>,
        bits: &mut [u8],
    ) -> Result<(), ModemError> {
        let n = y.len();
        self.check(n, noise_var, bits.len())?;
        let labels = 1usize << self.taps;
        // Integer metrics: -|y - e|^2 scaled to about 2^16 per noise variance, clamped well inside the decoder's range.
        let scale = 65_536.0 / noise_var;
        for (k, &v) in y.iter().enumerate() {
            if !(v.re.is_finite() && v.im.is_finite()) {
                return Err(ModemError::NotFinite {
                    param: Param::Input,
                });
            }
            for (lab, e) in self.expected.iter().enumerate() {
                let d = ((v - e).norm_sqr() * scale).min(1.6e7);
                self.metrics[k * labels + lab] = -(d as i32);
            }
        }
        let ends = match start {
            Some(s) if (s as usize) < self.states() => Ends::Start(s),
            Some(s) => return Param::StartState.range(0.0, (self.states() - 1) as f64, s as f64),
            None => Ends::Open,
        };
        let mut lm = LabelMetrics {
            labels,
            metrics: &self.metrics[..n * labels],
        };
        self.dec.decode(&mut lm, n, ends, None, bits)?;
        Ok(())
    }

    /// The LLR of every bit (positive means 0) by the max-log BCJR into `llr`, for noise of variance `noise_var` (`E|n|^2`).
    pub fn soft(
        &mut self,
        y: &[Complex32],
        noise_var: f32,
        start: Option<u32>,
        llr: &mut [f32],
    ) -> Result<(), ModemError> {
        let n = y.len();
        self.check(n, noise_var, llr.len())?;
        let s = self.states();
        if let Some(st) = start {
            if st as usize >= s {
                return Param::StartState.range(0.0, (s - 1) as f64, st as f64);
            }
        }
        let w = 1.0 / noise_var;
        let gamma = |e: Complex32, v: Complex32| -(v - e).norm_sqr() * w;
        // Forward.
        for (i, a) in self.alpha[..s].iter_mut().enumerate() {
            *a = match start {
                Some(st) if st as usize != i => f32::NEG_INFINITY,
                _ => 0.0,
            };
        }
        for (k, &v) in y.iter().enumerate() {
            if !(v.re.is_finite() && v.im.is_finite()) {
                return Err(ModemError::NotFinite {
                    param: Param::Input,
                });
            }
            let (cur, nxt) = self.alpha[k * s..(k + 2) * s].split_at_mut(s);
            nxt.iter_mut().for_each(|x| *x = f32::NEG_INFINITY);
            for (st, &a) in cur.iter().enumerate() {
                if a == f32::NEG_INFINITY {
                    continue;
                }
                for b in 0..2 {
                    let t = a + gamma(self.expected[self.trellis.label(st, b)], v);
                    let ns = self.trellis.next(st, b);
                    if t > nxt[ns] {
                        nxt[ns] = t;
                    }
                }
            }
            let m = nxt.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            nxt.iter_mut().for_each(|x| *x -= m);
        }
        // Backward, with the LLRs on the way.
        let (mut bcur, mut bnext) = self.beta.split_at_mut(s);
        bnext.iter_mut().for_each(|x| *x = 0.0);
        for k in (0..n).rev() {
            let v = y[k];
            let a = &self.alpha[k * s..(k + 1) * s];
            let (mut m0, mut m1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
            bcur.iter_mut().for_each(|x| *x = f32::NEG_INFINITY);
            for st in 0..s {
                for b in 0..2 {
                    let g = gamma(self.expected[self.trellis.label(st, b)], v);
                    let ns = self.trellis.next(st, b);
                    let bb = g + bnext[ns];
                    if bb > bcur[st] {
                        bcur[st] = bb;
                    }
                    let t = a[st] + bb;
                    if b == 0 {
                        m0 = m0.max(t);
                    } else {
                        m1 = m1.max(t);
                    }
                }
            }
            llr[k] = m0 - m1;
            let m = bcur.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            bcur.iter_mut().for_each(|x| *x -= m);
            std::mem::swap(&mut bcur, &mut bnext);
        }
        Ok(())
    }
}

/// Settings of [`GfskDemod`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GfskConfig {
    /// Input samples per symbol (at least 2).
    pub sps: f64,
    /// Peak deviation in cycles per sample (`h / (2 sps)` for index `h`).
    pub deviation: f64,
    /// Gaussian bandwidth-time product of the transmitter, for the timing detector's gain (`None` for plain FSK).
    pub bt: Option<f64>,
    /// Timing loop noise bandwidth per symbol.
    pub bn_t: f64,
    /// Clock range of the timing loop, ppm.
    pub max_ppm: f64,
    /// Forgetting factor of the level slicer's fit (about 1 / the symbols averaged).
    pub level_alpha: f32,
}

impl GfskConfig {
    /// Defaults: timing bandwidth 0.01, +-500 ppm, levels averaged over about 100 symbols.
    pub fn new(sps: f64, deviation: f64, bt: Option<f64>) -> Self {
        GfskConfig {
            sps,
            deviation,
            bt,
            bn_t: 0.01,
            max_ppm: 500.0,
            level_alpha: 0.01,
        }
    }
}

impl GfskConfig {
    /// The discriminator front end these settings describe.
    fn levels(&self) -> DiscriminatorConfig {
        DiscriminatorConfig {
            sps: self.sps,
            deviation: Some(self.deviation),
            bt: self.bt,
            bn_t: self.bn_t,
            max_ppm: self.max_ppm,
        }
    }

    fn check(&self) -> Result<(), ModemError> {
        Param::Sps.check_in(self.sps, 2.0, 4096.0)?;
        Param::Deviation.check_open(self.deviation, 0.0, 0.5)?;
        Ok(())
    }
}

/// Samples the GFSK receivers work on at a time, so their scratch buffers keep the size made at construction.
const PIECE: usize = 4096;

/// The discriminator receiver for GFSK, GMSK and binary FSK (module documentation): IQ in, LLRs of the bits out (positive means
/// 0, the lower frequency is bit 1 unless inverted). [`DiscriminatorLevels`] then a binary [`LevelSlicer`].
#[derive(Clone, Debug)]
pub struct GfskDemod {
    front: DiscriminatorLevels,
    slicer: LevelSlicer,
    levels: Vec<f32>,
    invert: bool,
}

impl GfskDemod {
    /// A receiver with these settings. Errors for `sps < 2`, a deviation not in `(0, 0.5)`, or a bad loop or slicer setting.
    pub fn new(cfg: GfskConfig) -> Result<Self, ModemError> {
        cfg.check()?;
        Ok(GfskDemod {
            front: DiscriminatorLevels::new(cfg.levels())?,
            slicer: LevelSlicer::binary(cfg.level_alpha, 1.0)?,
            levels: Vec::with_capacity(PIECE),
            invert: false,
        })
    }

    /// Makes the higher frequency bit 1 instead of 0.
    pub fn with_inverted(mut self, yes: bool) -> Self {
        self.invert = yes;
        self
    }

    /// The timing loop.
    pub fn timing(&self) -> &SymbolSync<f32, ZeroCrossing, Cubic> {
        self.front.timing()
    }

    /// The level slicer (its centre is the frequency offset, in units of the deviation, after the AGC).
    pub fn slicer(&self) -> &LevelSlicer {
        &self.slicer
    }

    /// Forgets the signal (the slicer keeps its fitted levels, a good start for the next burst).
    pub fn reset(&mut self) {
        self.front.reset();
    }

    /// Demodulates `x`, appending one LLR per symbol to `out`. Works in pieces of up to 4096 samples, so nothing is allocated.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) -> Result<(), ModemError> {
        for piece in x.chunks(PIECE) {
            self.levels.clear();
            self.front.process_iq(piece, &mut self.levels);
            let mut l = [0f32];
            for &v in &self.levels {
                self.slicer.slice(v, &mut l)?;
                out.push(if self.invert { -l[0] } else { l[0] });
            }
        }
        Ok(())
    }
}

/// The intersymbol interference a one-symbol integrate-and-dump leaves on the instantaneous frequency of GFSK with
/// bandwidth-time product `bt`, sampled at the symbol centres: `g_m` is the integral of the unit-area Gaussian frequency pulse
/// (`sdrkit_algo_dsp::pulse::gaussian_freq`) over `[m - 1/2, m + 1/2]` symbols, scaled so the centre tap is 1, as `2 K + 1` taps
/// (`K` 1 or 2: enough to hold every tap above 1 % of the centre). These are the taps an [`MlseStream`] takes for the levels of
/// [`DiscriminatorLevels`].
///
/// ```
/// let t = sdrkit_algo_modem::gmsk::discriminator_taps(0.3).unwrap();
/// assert_eq!(t.len(), 3); // GSM's BT 0.3: the taps two symbols away are below 1 %
/// assert_eq!(t[1], 1.0);
/// assert!((t[0] - 0.265).abs() < 1e-3 && t[2] == t[0]); // about a quarter of a symbol leaks into each neighbour
/// assert_eq!(sdrkit_algo_modem::gmsk::discriminator_taps(0.2).unwrap().len(), 5);
/// ```
pub fn discriminator_taps(bt: f64) -> Result<Vec<f32>, ModemError> {
    if !(bt > 0.0 && bt.is_finite()) {
        return Param::Bt.positive(bt);
    }
    let area = |m: f64| {
        const STEPS: usize = 256;
        (0..STEPS)
            .map(|i| pulse::gaussian_freq(m - 0.5 + (i as f64 + 0.5) / STEPS as f64, bt))
            .sum::<f64>()
            / STEPS as f64
    };
    let g0 = area(0.0);
    let k = if area(2.0) / g0 > 0.01 { 2 } else { 1 };
    Ok((-k..=k).map(|m| (area(m as f64) / g0) as f32).collect())
}

/// Streaming [`Mlse`] soft output (max-log BCJR, true LLRs, positive means 0) over an endless stream of samples, one per symbol:
/// the sliding-window BCJR. The forward recursion runs on without a break; every `block` samples, the backward recursion starts
/// `context` samples past the end of the block from an unknown state and returns the block's LLRs. With `context` a few times the
/// channel memory the LLRs equal those of the whole-burst [`Mlse::soft`] to within rounding; [`finish`](Self::finish) decides
/// what is left at the end of a burst exactly as `Mlse::soft` would.
///
/// LLR `k` is the bit that is newest in sample `k` (`g_0` multiplies it), so with a channel centred on tap `K` the bit of the
/// symbol at sample `k` is LLR `k - K`. An LLR leaves at most `block + context - 1` samples after its sample. Streaming: any cut
/// of the input returns the same output; nothing is allocated once it is built.
///
/// ```
/// use sdrkit_algo_modem::gmsk::{Mlse, MlseStream};
/// use sdrkit_algo_modem::Complex32;
/// let g = [Complex32::new(0.4, 0.0), Complex32::new(1.0, 0.0), Complex32::new(0.4, 0.0)];
/// let bits: Vec<u8> = (0..200).map(|k| ((k * 7 + k / 3) % 2) as u8).collect();
/// let y: Vec<Complex32> = (0..bits.len())
///     .map(|k| (0..3).filter(|&m| k >= m).map(|m| g[m] * (1.0 - 2.0 * bits[k - m] as f32)).sum())
///     .collect();
/// let mut s = MlseStream::new(&g, 32, 16).unwrap();
/// let mut llr = Vec::new();
/// for piece in y.chunks(7) {
///     s.process(piece, 0.1, &mut llr).unwrap();
/// }
/// s.finish(&mut llr);
/// let got: Vec<u8> = llr.iter().map(|&l| (l < 0.0) as u8).collect();
/// assert_eq!(got, bits);
/// ```
#[derive(Clone, Debug)]
pub struct MlseStream {
    mlse: Mlse,
    block: usize,
    context: usize,
    /// Samples not yet decided, and the weight `1 / noise_var` of each (0 for an erasure).
    y: Vec<Complex32>,
    w: Vec<f32>,
    /// Forward state metrics before `y[0]`.
    start: Vec<f32>,
    alpha: Vec<f32>,
    beta: Vec<f32>,
    llr: Vec<f32>,
}

impl MlseStream {
    /// A stream over channel `taps` (as [`Mlse::new`]) deciding `block` samples (1 to 65 536) at a time with `context` samples
    /// (0 to 65 536) of look-ahead for the backward recursion.
    pub fn new(taps: &[Complex32], block: usize, context: usize) -> Result<Self, ModemError> {
        Param::BlockLen.check_count(block, 1, 1 << 16)?;
        Param::Context.check_count(context, 0, 1 << 16)?;
        let mlse = Mlse::new(taps, 1)?;
        let s = mlse.states();
        let n = block + context;
        Ok(MlseStream {
            mlse,
            block,
            context,
            y: Vec::with_capacity(n),
            w: Vec::with_capacity(n),
            start: vec![0.0; s],
            alpha: vec![0.0; (n + 1) * s],
            beta: vec![0.0; 2 * s],
            llr: vec![0.0; n],
        })
    }

    /// Number of trellis states, `2^(taps - 1)`.
    pub fn states(&self) -> usize {
        self.mlse.states()
    }

    /// Samples decided at a time.
    pub fn block(&self) -> usize {
        self.block
    }

    /// Samples of look-ahead.
    pub fn context(&self) -> usize {
        self.context
    }

    /// Samples taken but not yet decided.
    pub fn pending(&self) -> usize {
        self.y.len()
    }

    /// Replaces the channel (the same number of taps); samples already taken are decided with the new one.
    pub fn set_channel(&mut self, taps: &[Complex32]) -> Result<(), ModemError> {
        self.mlse.set_channel(taps)
    }

    /// Forgets the samples not yet decided and the state: the next sample starts a new stream.
    pub fn reset(&mut self) {
        self.y.clear();
        self.w.clear();
        self.start.fill(0.0);
    }

    /// Takes one sample with noise of variance `noise_var` (`E|n|^2`, positive), appending any LLRs that become due to `out`. A
    /// sample that is not finite is an erasure: it carries no information but keeps its place.
    pub fn push(
        &mut self,
        y: Complex32,
        noise_var: f32,
        out: &mut Vec<f32>,
    ) -> Result<(), ModemError> {
        if !(noise_var > 0.0 && noise_var.is_finite()) {
            return Param::NoiseVariance.positive(noise_var);
        }
        if y.re.is_finite() && y.im.is_finite() {
            self.y.push(y);
            self.w.push(1.0 / noise_var);
        } else {
            self.y.push(Complex32::default());
            self.w.push(0.0);
        }
        if self.y.len() == self.block + self.context {
            self.run(self.block, out);
        }
        Ok(())
    }

    /// [`push`](Self::push) for every sample of `y`, all with noise of variance `noise_var`.
    pub fn process(
        &mut self,
        y: &[Complex32],
        noise_var: f32,
        out: &mut Vec<f32>,
    ) -> Result<(), ModemError> {
        for &v in y {
            self.push(v, noise_var, out)?;
        }
        Ok(())
    }

    /// Decides every sample still pending as the end of a burst (open end), appends their LLRs to `out`, and starts a new
    /// stream.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        let n = self.y.len();
        if n > 0 {
            self.run(n, out);
        }
        self.reset();
    }

    /// The max-log BCJR over the pending samples, from `start`, open at the end; appends the LLRs of the first `decide` and
    /// drops them.
    fn run(&mut self, decide: usize, out: &mut Vec<f32>) {
        let n = self.y.len();
        let s = self.mlse.states();
        let trellis = &self.mlse.trellis;
        let expected = &self.mlse.expected;
        let gamma = |lab: usize, v: Complex32, w: f32| -(v - expected[lab]).norm_sqr() * w;
        // Forward over the samples to decide.
        self.alpha[..s].copy_from_slice(&self.start);
        for k in 0..decide {
            let (v, w) = (self.y[k], self.w[k]);
            let (cur, nxt) = self.alpha[k * s..(k + 2) * s].split_at_mut(s);
            nxt.fill(f32::NEG_INFINITY);
            for (st, &a) in cur.iter().enumerate() {
                if a == f32::NEG_INFINITY {
                    continue;
                }
                for b in 0..2 {
                    let t = a + gamma(trellis.label(st, b), v, w);
                    let ns = trellis.next(st, b);
                    if t > nxt[ns] {
                        nxt[ns] = t;
                    }
                }
            }
            let m = nxt.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            nxt.iter_mut().for_each(|x| *x -= m);
        }
        // Backward from the end of the context (every state equally likely), with the LLRs over the decided part.
        let (mut bcur, mut bnext) = self.beta.split_at_mut(s);
        bnext.fill(0.0);
        for k in (0..n).rev() {
            let (v, w) = (self.y[k], self.w[k]);
            let (mut m0, mut m1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
            bcur.fill(f32::NEG_INFINITY);
            for st in 0..s {
                for b in 0..2 {
                    let bb = gamma(trellis.label(st, b), v, w) + bnext[trellis.next(st, b)];
                    if bb > bcur[st] {
                        bcur[st] = bb;
                    }
                    if k < decide {
                        let t = self.alpha[k * s + st] + bb;
                        if b == 0 {
                            m0 = m0.max(t);
                        } else {
                            m1 = m1.max(t);
                        }
                    }
                }
            }
            if k < decide {
                self.llr[k] = m0 - m1;
            }
            let m = bcur.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            bcur.iter_mut().for_each(|x| *x -= m);
            std::mem::swap(&mut bcur, &mut bnext);
        }
        out.extend_from_slice(&self.llr[..decide]);
        self.start
            .copy_from_slice(&self.alpha[decide * s..(decide + 1) * s]);
        self.y.drain(..decide);
        self.w.drain(..decide);
    }
}

/// GFSK and GMSK by [`DiscriminatorLevels`] and an [`MlseStream`] over the intersymbol interference the Gaussian pulse leaves at
/// the symbol centres ([`discriminator_taps`]): better than slicing at low BT (GSM 0.3, AIS 0.4), still non-coherent.
///
/// A binary [`LevelSlicer`] follows the centre (a frequency offset) and the spacing of the levels; each level is normalised by
/// them (`(v - centre) / spacing`, so the symbols are near +-1) and goes to the MLSE with its noise variance `2 noise / spacing^2`
/// (at least 1e-3). The higher frequency is bit 0 unless inverted. The soft bits come [`delay`](Self::delay) symbols late at
/// most, and the first `K` symbols (half the taps less one) have no LLR, since the trellis has no state for them. Streaming: any
/// cut of the input returns the same output; nothing is allocated once it is built.
///
/// ```
/// use sdrkit_algo_modem::gmsk::{GfskConfig, GfskMlseDemod};
/// use sdrkit_algo_modem::Complex32;
/// // GMSK BT 0.3 at 4 samples per symbol (deviation h / (2 sps) = 1/16).
/// let mut d = GfskMlseDemod::new(GfskConfig::new(4.0, 1.0 / 16.0, Some(0.3)), 64, 16).unwrap();
/// let x = vec![Complex32::new(1.0, 0.0); 4000];
/// let mut llr = Vec::new();
/// d.process(&x, &mut llr).unwrap();
/// assert!(llr.len() <= 1000);
/// ```
#[derive(Clone, Debug)]
pub struct GfskMlseDemod {
    front: DiscriminatorLevels,
    slicer: LevelSlicer,
    mlse: MlseStream,
    levels: Vec<f32>,
    invert: bool,
}

impl GfskMlseDemod {
    /// A receiver with these settings (`bt` sets the taps; `None`, a plain FSK, has no ISI and one tap), deciding `block`
    /// symbols at a time with `context` symbols of look-ahead (64 and 16 suit BT 0.3 to 0.5).
    pub fn new(cfg: GfskConfig, block: usize, context: usize) -> Result<Self, ModemError> {
        cfg.check()?;
        let taps: Vec<Complex32> = match cfg.bt {
            Some(bt) => discriminator_taps(bt)?
                .into_iter()
                .map(|t| Complex32::new(t, 0.0))
                .collect(),
            None => vec![Complex32::new(1.0, 0.0)],
        };
        Ok(GfskMlseDemod {
            front: DiscriminatorLevels::new(cfg.levels())?,
            slicer: LevelSlicer::binary(cfg.level_alpha, 1.0)?,
            mlse: MlseStream::new(&taps, block, context)?,
            levels: Vec::with_capacity(PIECE),
            invert: false,
        })
    }

    /// Makes the higher frequency bit 1 instead of 0.
    pub fn with_inverted(mut self, yes: bool) -> Self {
        self.invert = yes;
        self
    }

    /// The timing loop.
    pub fn timing(&self) -> &SymbolSync<f32, ZeroCrossing, Cubic> {
        self.front.timing()
    }

    /// The level slicer that follows the centre and spacing.
    pub fn slicer(&self) -> &LevelSlicer {
        &self.slicer
    }

    /// The most symbols an LLR comes after its level.
    pub fn delay(&self) -> usize {
        self.mlse.block() + self.mlse.context() - 1
    }

    /// Forgets the signal and the symbols not yet decided.
    pub fn reset(&mut self) {
        self.front.reset();
        self.mlse.reset();
    }

    /// Demodulates `x`, appending one LLR per symbol (once due) to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) -> Result<(), ModemError> {
        for piece in x.chunks(PIECE) {
            self.levels.clear();
            self.front.process_iq(piece, &mut self.levels);
            let mut l = [0f32];
            for &v in &self.levels {
                // The slicer follows the centre and the spacing; its own decision is not used.
                self.slicer.slice(v, &mut l)?;
                let s = self.slicer.spacing();
                let z = (v - self.slicer.centre()) / s;
                let z = if self.invert { -z } else { z };
                let var = (2.0 * self.slicer.noise_var() / (s * s)).max(1e-3);
                self.mlse.push(Complex32::new(z, 0.0), var, out)?;
            }
        }
        Ok(())
    }

    /// Decides the symbols still pending (the end of a burst) and starts the MLSE afresh.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        self.mlse.finish(out);
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn gfsk(seconds: f64) -> BenchResult {
        // BLE 1M at 4 Msps.
        let n = 65_536;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::from_polar(1.0, (i as f32 * 0.4).sin() * 3.0))
            .collect();
        let mut d = GfskDemod::new(GfskConfig::new(4.0, 0.0625, Some(0.5))).expect("valid");
        let mut out = Vec::with_capacity(n);
        measure(seconds, n, n as f64 / 4e6, || {
            out.clear();
            d.process(&x, &mut out).unwrap_or(());
        })
    }
    fn mlse(seconds: f64) -> BenchResult {
        // A GSM normal burst: 148 symbols, 5 taps (16 states), soft output; 216.7 bursts per second on one timeslot... counted per
        // symbol at 270.833 ksym/s.
        let g: Vec<Complex32> = [0.2f32, 0.6, 1.0, 0.6, 0.2]
            .iter()
            .map(|&v| Complex32::new(v, 0.1))
            .collect();
        let y: Vec<Complex32> = (0..148)
            .map(|k| Complex32::new(((k * 7) % 5) as f32 - 2.0, 0.3))
            .collect();
        let mut m = Mlse::new(&g, 160).expect("valid");
        let mut llr = vec![0f32; 148];
        measure(seconds, 148, 148.0 / 270_833.0, || {
            m.soft(&y, 0.5, None, &mut llr).unwrap_or(());
        })
    }
    fn mlse_stream(seconds: f64) -> BenchResult {
        // The streaming max-log BCJR over 3 taps (4 states, AIS and BLE), 64-symbol blocks with 16 of look-ahead, per symbol
        // at 270.833 ksym/s.
        let g = [
            Complex32::new(0.27, 0.0),
            Complex32::new(1.0, 0.0),
            Complex32::new(0.27, 0.0),
        ];
        let n = 65_536;
        let y: Vec<Complex32> = (0..n)
            .map(|k| Complex32::new(((k * 7 + k / 3) % 5) as f32 * 0.5 - 1.0, 0.0))
            .collect();
        let mut m = MlseStream::new(&g, 64, 16).expect("valid");
        let mut llr = Vec::with_capacity(n + 128);
        measure(seconds, n, n as f64 / 270_833.0, || {
            llr.clear();
            m.process(&y, 0.5, &mut llr).unwrap_or(());
        })
    }
    fn gfsk_mlse(seconds: f64) -> BenchResult {
        // AIS (GMSK BT 0.4, 9600 baud) at 48 kHz: discriminator front end, level tracking and the streaming MLSE.
        let n = 48_000;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::from_polar(1.0, (i as f32 * 0.07).sin() * 3.0))
            .collect();
        let mut d =
            GfskMlseDemod::new(GfskConfig::new(5.0, 0.05, Some(0.4)), 64, 16).expect("valid");
        let mut out = Vec::with_capacity(n);
        measure(seconds, n, n as f64 / 48_000.0, || {
            out.clear();
            d.process(&x, &mut out).unwrap_or(());
        })
    }
    vec![
        Benchmark {
            name: "modem.mlse4_stream_270k",
            run: mlse_stream,
        },
        Benchmark {
            name: "modem.gmsk_mlse_ais_48k",
            run: gfsk_mlse,
        },
        Benchmark {
            name: "modem.gfsk_ble_4000k",
            run: gfsk,
        },
        Benchmark {
            name: "modem.mlse16_soft_gsm_270k",
            run: mlse,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::meter::db_to_power;
    use sdrkit_algo_sim::modulate::Cpm;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    fn isi(g: &[Complex32], a: &[f32]) -> Vec<Complex32> {
        (0..a.len())
            .map(|k| {
                (0..g.len())
                    .filter(|&m| k >= m)
                    .map(|m| g[m] * a[k - m])
                    .sum()
            })
            .collect()
    }

    #[test]
    fn ls_finds_the_channel() {
        let g = [
            Complex32::new(0.2, -0.1),
            Complex32::new(1.0, 0.3),
            Complex32::new(-0.4, 0.2),
        ];
        let mut rng = Rng::new(3);
        let a: Vec<f32> = (0..200)
            .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut y = isi(&g, &a);
        for v in &mut y {
            *v += Complex32::new(0.5, -0.25);
        }
        Awgn::with_power(0.01, 4).unwrap().process(&mut y);
        let s: Vec<Complex32> = a.iter().map(|&v| Complex32::new(v, 0.0)).collect();
        let mut t = [Complex32::default(); 3];
        let fit = ls_channel(&y, &s, &mut t, true).unwrap();
        for (a, b) in t.iter().zip(&g) {
            assert!((a - b).norm() < 0.03, "{a} {b}");
        }
        assert!((fit.dc - Complex32::new(0.5, -0.25)).norm() < 0.03);
        assert!((fit.noise_var - 0.01).abs() < 0.003, "{}", fit.noise_var);
        // A constant input cannot excite three taps.
        let flat = vec![Complex32::new(1.0, 0.0); 50];
        assert!(ls_channel(&y[..50], &flat, &mut t, false).is_err());
        assert!(ls_channel(&y[..10], &s[..9], &mut t, false).is_err());
    }

    #[test]
    fn mlse_hard_and_soft_agree_and_llrs_are_calibrated() {
        let g = [
            Complex32::new(0.5, 0.0),
            Complex32::new(1.0, 0.0),
            Complex32::new(0.5, 0.0),
        ];
        let mut rng = Rng::new(9);
        let n = 1000;
        let mut m = Mlse::new(&g, n).unwrap();
        let nv = 0.5f32;
        let mut awgn = Awgn::with_power(nv as f64, 5).unwrap();
        let (mut bins, mut hard_err) = ([(0usize, 0usize); 8], 0usize);
        let mut bits = vec![0u8; n];
        let mut out = vec![0u8; n];
        let mut llr = vec![0f32; n];
        for _ in 0..60 {
            rng.fill_bits(&mut bits);
            let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
            let mut y = isi(&g, &a);
            awgn.process(&mut y);
            m.decode(&y, nv, Some(0), &mut out).unwrap();
            m.soft(&y, nv, Some(0), &mut llr).unwrap();
            for k in 0..n {
                hard_err += (out[k] != bits[k]) as usize;
                let b = ((llr[k].abs() / 1.5) as usize).min(7);
                bins[b].0 += 1;
                bins[b].1 += ((llr[k] < 0.0) as u8 != bits[k]) as usize;
            }
        }
        assert!(hard_err > 0);
        // Max-log LLRs are close to calibrated where errors are common enough to count.
        for (k, &(cnt, err)) in bins.iter().enumerate().take(4) {
            if cnt < 3000 {
                continue;
            }
            let p = err as f64 / cnt as f64;
            let lo = 1.0 / (1.0 + (1.5 * (k + 1) as f64).exp());
            let hi = 1.0 / (1.0 + (1.5 * k as f64).exp());
            assert!(
                p > 0.6 * lo && p < 1.25 * hi,
                "bin {k}: {p} vs [{lo}, {hi}]"
            );
        }
    }

    #[test]
    fn gmsk_taps_have_the_known_shape() {
        // BT 0.3: the side taps of C0 at +-1 symbol are a little under 0.2 of the peak (GSM's ISI); BT 0.5 has less.
        let t3 = gmsk_taps(0.3, 3).unwrap();
        let t5 = gmsk_taps(0.5, 3).unwrap();
        assert_eq!(t3[1], 1.0);
        assert!((t3[0] - t3[2]).abs() < 1e-5);
        assert!(t3[0] > 0.1 && t3[0] < 0.3, "{t3:?}");
        assert!(t5[0] < t3[0]);
        assert!(gmsk_taps(0.3, 4).is_err());
    }

    /// GMSK BT 0.3 at 4 samples per symbol through AWGN, sampled at the Laurent pulse peaks.
    fn gmsk_run(ebn0_db: f64, nsym: usize, seed: u64) -> (f64, f64) {
        let sps = 4.0;
        let mut rng = Rng::new(seed);
        let a: Vec<f32> = (0..nsym)
            .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::gmsk(0.3, sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        // Eb = sps (unit power, one bit per symbol).
        let n0 = sps / db_to_power(ebn0_db);
        Awgn::with_power(n0, seed + 1).unwrap().process(&mut x);
        // Matched filter, then the samples at the C0 peaks: half a symbol after each frequency pulse's centre, plus the delay.
        let h = gmsk_matched(0.3, sps, 2.0).unwrap();
        let delay = (h.len() - 1) as f64 / 2.0;
        let mut fir = sdrkit_algo_dsp::fir::Fir::<Complex32>::new(&h);
        let mut xf = vec![Complex32::default(); x.len()];
        fir.process(&x, &mut xf);
        let y: Vec<Complex32> = (0..nsym)
            .map(|k| {
                let p = m.symbol_centre(k as u64) + sps / 2.0 + delay;
                xf.get(p.round() as usize).copied().unwrap_or_default()
            })
            .collect();
        let mut yr = vec![Complex32::default(); nsym];
        derotate(&y, 0, &mut yr).unwrap();
        // c_k = c_(k-1) a_k with c_(-1) = 1; the channel and its phase are estimated from the training (first 100 symbols).
        let mut c = vec![0f32; nsym];
        let mut prev = 1.0;
        for k in 0..nsym {
            prev *= a[k];
            c[k] = prev;
        }
        let s: Vec<Complex32> = c.iter().map(|&v| Complex32::new(v, 0.0)).collect();
        // yr[k] sees c(k+2) .. c(k-2) (C0 through its matched filter spans five symbols): c(k+2) is the newest, tap 0 first.
        let mut g = [Complex32::default(); 5];
        let fit = ls_channel(&yr[..200], &s[2..202], &mut g, false).unwrap();
        let mut mlse = Mlse::new(&g, nsym).unwrap();
        // cb[k] is the bit of c(k+2).
        let mut cb = vec![0u8; nsym - 2];
        mlse.decode(&yr[..nsym - 2], fit.noise_var.max(1e-3), None, &mut cb)
            .unwrap();
        let sym = |b: u8| 1.0 - 2.0 * b as f32;
        let mut err_mlse = 0usize;
        for k in 200..nsym - 10 {
            // a(k) = c(k) c(k-1).
            let ah = sym(cb[k - 2]) * sym(cb[k - 3]);
            err_mlse += (ah != a[k]) as usize;
        }
        // Discriminator plus integrate-and-dump at the same instants.
        let mut d = sdrkit_algo_dsp::fm::Discriminator::new(1.0, 1.0 / 16.0).unwrap();
        let mut f = vec![0f32; xf.len()];
        d.process_into(&xf, &mut f);
        let mut err_disc = 0usize;
        for (k, &ak) in a.iter().enumerate().take(nsym - 10).skip(200) {
            let c0 = m.symbol_centre(k as u64) + delay;
            let lo = (c0 - sps / 2.0).round() as usize + 1;
            let v: f32 = f[lo..lo + 4].iter().sum();
            err_disc += ((v > 0.0) != (ak > 0.0)) as usize;
        }
        let n = (nsym - 210) as f64;
        (err_mlse as f64 / n, err_disc as f64 / n)
    }

    #[test]
    fn gmsk_mlse_beats_the_discriminator() {
        let (mlse, disc) = gmsk_run(7.0, 40_000, 21);
        // Coherent detection of GMSK BT 0.3 is within about 1 dB of BPSK (BPSK at 7 dB: 7.7e-4, doubled by the differential
        // decoding); the discriminator is several dB worse.
        assert!(mlse < 0.4 * disc, "MLSE {mlse} vs discriminator {disc}");
        assert!(mlse < 6e-3, "{mlse}");
    }

    #[test]
    fn gfsk_streaming_receiver_with_offset_and_clock_error() {
        // BLE-like: GFSK BT 0.5, h 0.5, 4 samples per symbol, 100 ppm clock error, a frequency offset of 5 % of the symbol rate.
        let sps = 4.0 * (1.0 + 100e-6);
        let mut rng = Rng::new(31);
        let bits: Vec<u8> = (0..6000).map(|_| rng.bit()).collect();
        let a: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::gfsk(0.5, 0.5, sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        let mut ph = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(0.05 / 4.0);
        for v in &mut x {
            *v *= ph.next_phasor();
        }
        Awgn::with_power(0.05, 7).unwrap().process(&mut x);
        let mut d = GfskDemod::new(GfskConfig::new(4.0, 0.5 / 8.0, Some(0.5))).unwrap();
        let mut llr = Vec::new();
        for piece in x.chunks(1000) {
            d.process(piece, &mut llr).unwrap();
        }
        let got: Vec<u8> = llr.iter().map(|&l| (l < 0.0) as u8).collect();
        let want = &bits[1000..5000];
        // Every bit after the first few hundred symbols, a few symbols late (the filters' delay).
        let found =
            (1000..1010).any(|o| got.len() >= o + want.len() && &got[o..o + want.len()] == want);
        assert!(found, "clock {} ppm", d.timing().clock_ppm());
        // The clock estimate is noisy at this loop bandwidth (the zero-crossing detector's pattern noise), but holds the sign.
        assert!(
            d.timing().clock_ppm().abs() < 300.0,
            "{}",
            d.timing().clock_ppm()
        );
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(Mlse::new(&[], 10).is_err());
        let mut m = Mlse::new(&[Complex32::new(1.0, 0.0); 2], 4).unwrap();
        let mut b = [0u8; 8];
        assert!(m
            .decode(&[Complex32::default(); 8], 1.0, None, &mut b)
            .is_err());
        assert!(m
            .decode(&[Complex32::default(); 4], 0.0, None, &mut b)
            .is_err());
        assert!(m
            .decode(&[Complex32::default(); 4], 1.0, Some(5), &mut b)
            .is_err());
        assert!(m.set_channel(&[Complex32::default(); 3]).is_err());
        assert!(GfskDemod::new(GfskConfig::new(1.0, 0.1, None)).is_err());
        assert!(derotate(
            &[Complex32::default(); 3],
            0,
            &mut [Complex32::default(); 2]
        )
        .is_err());
        let g = [Complex32::new(1.0, 0.0); 3];
        assert!(MlseStream::new(&g, 0, 4).is_err());
        assert!(MlseStream::new(&g, 8, 1 << 17).is_err());
        assert!(MlseStream::new(&[], 8, 4).is_err());
        let mut s = MlseStream::new(&g, 8, 4).unwrap();
        let mut out = Vec::new();
        assert!(s.push(Complex32::new(1.0, 0.0), 0.0, &mut out).is_err());
        assert!(s
            .push(Complex32::new(1.0, 0.0), f32::NAN, &mut out)
            .is_err());
        assert_eq!(s.pending(), 0);
        assert!(discriminator_taps(0.0).is_err() && discriminator_taps(f64::NAN).is_err());
        assert!(GfskMlseDemod::new(GfskConfig::new(4.0, 0.7, Some(0.3)), 64, 16).is_err());
    }

    #[test]
    fn discriminator_taps_are_symmetric_and_shrink_with_bt() {
        let t3 = discriminator_taps(0.3).unwrap();
        let t5 = discriminator_taps(0.5).unwrap();
        assert_eq!(t3[t3.len() / 2], 1.0);
        assert!((t3[0] - t3[t3.len() - 1]).abs() < 1e-6);
        let side = |t: &[f32]| t[t.len() / 2 - 1];
        assert!(side(&t3) > side(&t5) && side(&t5) > 0.0);
        assert!(side(&t3) < 0.5);
        // Known values by direct quadrature of the Gaussian frequency pulse (erf form): for BT 0.5 the pulse
        // `g(t) = [erf(c (t + 1/2)) - erf(c (t - 1/2))] / 2`, `c = pi BT sqrt(2 / ln 2)`, integrated over each symbol.
        let c = std::f64::consts::PI * 0.5 * (2.0 / std::f64::consts::LN_2).sqrt();
        let g = |t: f64| {
            (sdrkit_algo_dsp::pulse::erf(c * (t + 0.5))
                - sdrkit_algo_dsp::pulse::erf(c * (t - 0.5)))
                / 2.0
        };
        let area = |m: f64| {
            (0..4096)
                .map(|i| g(m - 0.5 + (i as f64 + 0.5) / 4096.0))
                .sum::<f64>()
                / 4096.0
        };
        let want = area(1.0) / area(0.0);
        assert!(
            (side(&t5) as f64 - want).abs() < 1e-4,
            "{} vs {want}",
            side(&t5)
        );
    }

    /// Binary symbols through `g` with noise of variance `nv`.
    fn isi_noisy(g: &[Complex32], n: usize, nv: f64, seed: u64) -> (Vec<u8>, Vec<Complex32>) {
        let mut rng = Rng::new(seed);
        let mut bits = vec![0u8; n];
        rng.fill_bits(&mut bits);
        let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
        let mut y = isi(g, &a);
        Awgn::with_power(nv, seed + 1).unwrap().process(&mut y);
        (bits, y)
    }

    #[test]
    fn mlse_stream_finish_equals_the_whole_burst() {
        let g = [
            Complex32::new(0.3, 0.1),
            Complex32::new(1.0, 0.0),
            Complex32::new(-0.4, 0.2),
        ];
        let (_, y) = isi_noisy(&g, 500, 0.3, 41);
        let mut whole = vec![0f32; 500];
        Mlse::new(&g, 500)
            .unwrap()
            .soft(&y, 0.3, None, &mut whole)
            .unwrap();
        // A block longer than the burst: everything is decided at `finish`, by the same recursions.
        let mut s = MlseStream::new(&g, 600, 0).unwrap();
        let mut out = Vec::new();
        s.process(&y, 0.3, &mut out).unwrap();
        assert!(out.is_empty());
        s.finish(&mut out);
        assert_eq!(out, whole);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn mlse_stream_is_cut_invariant_and_close_to_the_whole_burst() {
        let g = [
            Complex32::new(0.5, 0.0),
            Complex32::new(1.0, 0.0),
            Complex32::new(0.5, 0.0),
        ];
        let n = 5000;
        let (bits, y) = isi_noisy(&g, n, 0.5, 51);
        let run = |cuts: &[usize]| {
            let mut s = MlseStream::new(&g, 64, 16).unwrap();
            let mut out = Vec::new();
            let mut at = 0;
            let mut k = 0;
            while at < n {
                let step = cuts[k % cuts.len()].min(n - at);
                s.process(&y[at..at + step], 0.5, &mut out).unwrap();
                at += step;
                k += 1;
            }
            s.finish(&mut out);
            out
        };
        let a = run(&[n]);
        assert_eq!(a.len(), n);
        for cuts in [&[1usize][..], &[7, 1, 130], &[64], &[1000, 3]] {
            assert_eq!(run(cuts), a, "{cuts:?}");
        }
        // With 16 symbols of look-ahead on a 2-symbol memory, the windowed LLRs match the whole burst's.
        let mut whole = vec![0f32; n];
        Mlse::new(&g, n)
            .unwrap()
            .soft(&y, 0.5, None, &mut whole)
            .unwrap();
        let worst = a
            .iter()
            .zip(&whole)
            .map(|(p, q)| (p - q).abs())
            .fold(0f32, f32::max);
        assert!(worst < 1e-3, "{worst}");
        let errs = a
            .iter()
            .zip(&bits)
            .filter(|(l, &b)| ((**l < 0.0) as u8) != b)
            .count();
        assert!(errs > 0 && errs < n / 20, "{errs}");
    }

    #[test]
    fn mlse_stream_erasures_keep_their_place() {
        let g = [Complex32::new(1.0, 0.0), Complex32::new(0.5, 0.0)];
        let (bits, mut y) = isi_noisy(&g, 300, 0.05, 61);
        y[100] = Complex32::new(f32::NAN, 0.0);
        let mut s = MlseStream::new(&g, 32, 8).unwrap();
        let mut out = Vec::new();
        s.process(&y, 0.05, &mut out).unwrap();
        s.finish(&mut out);
        assert_eq!(out.len(), 300);
        let errs = out
            .iter()
            .zip(&bits)
            .filter(|(l, &b)| ((**l < 0.0) as u8) != b)
            .count();
        // The bit at the erasure is still known from the next sample (tap 1).
        assert_eq!(errs, 0);
    }

    /// GMSK BT 0.3 at 4 samples per symbol with AWGN at `ebn0_db`: the bit errors of the discriminator receiver and of the MLSE
    /// receiver over the bits after the first 500.
    fn gmsk_disc_vs_mlse(ebn0_db: f64, nsym: usize, seed: u64) -> (usize, usize, usize) {
        let sps = 4.0;
        let mut rng = Rng::new(seed);
        let bits: Vec<u8> = (0..nsym).map(|_| rng.bit()).collect();
        let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
        let mut m = Cpm::gmsk(0.3, sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        Awgn::with_power(sps / db_to_power(ebn0_db), seed + 1)
            .unwrap()
            .process(&mut x);
        let cfg = GfskConfig::new(sps, 1.0 / 16.0, Some(0.3));
        let mut disc = GfskDemod::new(cfg).unwrap();
        let mut ld = Vec::new();
        disc.process(&x, &mut ld).unwrap();
        let mut mlse = GfskMlseDemod::new(cfg, 64, 16).unwrap();
        let mut lm = Vec::new();
        mlse.process(&x, &mut lm).unwrap();
        mlse.finish(&mut lm);
        // Each receiver's output is the bits shifted by its delay: find the shift, then count the errors.
        let want = &bits[500..nsym - 40];
        let errors = |l: &[f32]| {
            let got: Vec<u8> = l.iter().map(|&v| (v < 0.0) as u8).collect();
            // got[480 + o] against bits[500]: shifts of -20 to +19 symbols.
            (0..40usize)
                .filter_map(|o| got.get(480 + o..480 + o + want.len()))
                .map(|g| g.iter().zip(want).filter(|(p, q)| p != q).count())
                .min()
                .unwrap_or(usize::MAX)
        };
        (errors(&ld), errors(&lm), nsym - 540)
    }

    #[test]
    fn gfsk_mlse_receiver_beats_slicing_at_bt_0_3() {
        let (disc, mlse, n) = gmsk_disc_vs_mlse(12.0, 30_000, 71);
        assert!(disc > 0 && disc < n / 10, "discriminator {disc} of {n}");
        assert!(mlse < disc * 2 / 3, "MLSE {mlse} vs discriminator {disc}");
    }

    #[test]
    fn gfsk_mlse_receiver_is_cut_invariant() {
        let sps = 4.0;
        let mut rng = Rng::new(81);
        let a: Vec<f32> = (0..3000)
            .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::gmsk(0.4, sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        Awgn::with_power(0.2, 82).unwrap().process(&mut x);
        let run = |cut: usize| {
            let mut d =
                GfskMlseDemod::new(GfskConfig::new(sps, 1.0 / 16.0, Some(0.4)), 64, 16).unwrap();
            let mut out = Vec::new();
            for p in x.chunks(cut) {
                d.process(p, &mut out).unwrap();
            }
            d.finish(&mut out);
            out
        };
        let whole = run(x.len());
        assert!(whole.len() > 2900);
        for cut in [1, 13, 4096, 5000] {
            assert_eq!(run(cut), whole, "{cut}");
        }
    }
}
