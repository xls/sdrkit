//! FIR filter design (Kaiser-windowed sinc) and streaming FIR filters.
//!
//! Design works in `f64` and returns `f32` taps. Lowpass and band-pass designs have unity gain in the passband. Lengths come from
//! Kaiser's formula `N = (A - 7.95) / (14.36 * df) + 1`, rounded up to odd so the delay is a whole number of samples.

use crate::window::{kaiser, kaiser_beta};

/// Kaiser's formulas are within a few dB of the requested attenuation near the band edges; designs add this margin so they meet it.
pub const DESIGN_MARGIN_DB: f64 = 3.0;

/// Kaiser's beta for a design meeting `atten_db` (with [`DESIGN_MARGIN_DB`]).
pub fn design_beta(atten_db: f64) -> f64 {
    kaiser_beta(atten_db + DESIGN_MARGIN_DB)
}
use crate::{Complex32, DspError, Param, Sample};
use num_complex::Complex64;
use sdrkit_core::backend::{Backend, Registry};
use std::f64::consts::{PI, TAU};

/// The largest number of taps a design returns (2^20).
pub const MAX_TAPS: usize = 1 << 20;

/// The number of taps (odd, at least 3) for a stopband attenuation `atten_db` (plus [`DESIGN_MARGIN_DB`]) and a transition width
/// `transition` in cycles per sample. Errors unless `transition` is in `(0, 0.5)` and `atten_db` is finite.
pub fn kaiser_len(atten_db: f64, transition: f64) -> Result<usize, crate::DspError> {
    kaiser_len_with_margin(atten_db, transition, DESIGN_MARGIN_DB)
}

/// The number of taps [`lowpass`] returns for this specification. Errors as [`lowpass`] does.
pub fn lowpass_len(
    rate: f64,
    pass_hz: f64,
    stop_hz: f64,
    atten_db: f64,
) -> Result<usize, crate::DspError> {
    check_edges(rate, pass_hz, stop_hz)?;
    kaiser_len(atten_db, (stop_hz - pass_hz) / rate)
}

fn check_edges(rate: f64, pass_hz: f64, stop_hz: f64) -> Result<(), DspError> {
    if !(rate > 0.0 && rate.is_finite()) {
        return Param::Rate.positive(rate);
    }
    if pass_hz < 0.0 || pass_hz.is_nan() {
        return Param::Pass.at_least(0.0, pass_hz);
    }
    if stop_hz > rate / 2.0 + 1e-9 * rate || stop_hz.is_nan() {
        return Param::Stop.range(0.0, rate / 2.0, stop_hz);
    }
    if pass_hz >= stop_hz || pass_hz.is_nan() || stop_hz.is_nan() {
        return Param::Pass.conflict(Param::Stop);
    }
    Ok(())
}

/// A Kaiser-windowed sinc lowpass of `num_taps` taps with its -6 dB point at `cutoff` cycles per sample. It is not renormalised: the DC
/// gain is 1 within the passband ripple (renormalising to exactly 1 would move the whole passband and up to double the ripple).
/// Errors unless `num_taps` is 1 to [`MAX_TAPS`], `cutoff` is in `(0, 0.5]` and `beta` is at least 0.
pub fn windowed_sinc(num_taps: usize, cutoff: f64, beta: f64) -> Result<Vec<f64>, crate::DspError> {
    if !(1..=MAX_TAPS).contains(&num_taps) {
        return Param::Taps.range(1.0, MAX_TAPS as f64, num_taps as f64);
    }
    if !(cutoff > 0.0 && cutoff <= 0.5) {
        return Param::Cutoff.range(0.0, 0.5, cutoff);
    }
    if !(beta >= 0.0 && beta.is_finite()) {
        return Param::Beta.at_least(0.0, beta);
    }
    let w = kaiser(num_taps, beta);
    let c = (num_taps - 1) as f64 / 2.0;
    Ok((0..num_taps)
        .map(|i| {
            let m = i as f64 - c;
            let s = if m == 0.0 {
                2.0 * cutoff
            } else {
                (TAU * cutoff * m).sin() / (PI * m)
            };
            s * w[i]
        })
        .collect())
}

/// A lowpass: flat (within the ripple `10^(-atten/20)`) to `pass_hz`, attenuated by at least about `atten_db` from `stop_hz`.
/// Errors unless `0 <= pass_hz < stop_hz <= rate / 2`, `atten_db` is finite and the filter has at most [`MAX_TAPS`] taps.
///
/// ```
/// use sdrkit_algo_dsp::fir::lowpass;
/// assert!(lowpass(48_000.0, 3_000.0, 4_000.0, 60.0).is_ok());
/// assert!(lowpass(48_000.0, 3_000.0, 30_000.0, 60.0).is_err());
/// ```
pub fn lowpass(
    rate: f64,
    pass_hz: f64,
    stop_hz: f64,
    atten_db: f64,
) -> Result<Vec<f32>, crate::DspError> {
    lowpass_with_margin(rate, pass_hz, stop_hz, atten_db, DESIGN_MARGIN_DB)
}

/// The number of taps (odd, at least 3) for a stopband attenuation `atten_db` plus a design margin `margin_db`, and a transition width
/// `transition` in cycles per sample. Kaiser's estimate `N = (A - 7.95) / (14.36 df) + 1` with `A = atten_db + margin_db` (at least
/// 21 dB), rounded up to odd. A margin of 0 is the published estimate. [`kaiser_len`] is this function with
/// [`DESIGN_MARGIN_DB`]. Errors unless `transition` is in `(0, 0.5)` and `atten_db` and `margin_db` are finite with `margin_db >= 0`, and unless the length is at most [`MAX_TAPS`].
///
/// Standard: J. F. Kaiser, "Nonrecursive digital filter design using the I0-sinh window function", IEEE ISCAS 1974, the length
/// estimate.
///
/// ```
/// use sdrkit_algo_dsp::fir::kaiser_len_with_margin;
/// // 60 dB over a transition of 0.05: (60 - 7.95) / (14.36 * 0.05) = 72.5, rounded up to 73, plus 1, made odd: 75 taps.
/// assert_eq!(kaiser_len_with_margin(60.0, 0.05, 0.0).unwrap(), 75);
/// ```
pub fn kaiser_len_with_margin(
    atten_db: f64,
    transition: f64,
    margin_db: f64,
) -> Result<usize, crate::DspError> {
    if !(transition > 0.0 && transition < 0.5) {
        return Param::Transition.range(0.0, 0.5, transition);
    }
    if !atten_db.is_finite() {
        return Param::AttenuationDb.not_finite();
    }
    if !(margin_db >= 0.0 && margin_db.is_finite()) {
        return Param::Margin.at_least(0.0, margin_db);
    }
    let a = (atten_db + margin_db).max(21.0);
    let n = ((a - 7.95) / (14.36 * transition)).ceil() + 1.0;
    if n > MAX_TAPS as f64 {
        return Param::Taps.range(1.0, MAX_TAPS as f64, n);
    }
    Ok((n as usize).max(3) | 1)
}

/// A lowpass like [`lowpass`], designed for `atten_db` plus a margin of `margin_db` (both the length and Kaiser's beta use the sum).
/// [`lowpass`] is this function with [`DESIGN_MARGIN_DB`]. A margin of 0 produces the shortest filter by Kaiser's estimate, which can
/// miss the attenuation by a few dB near the stop edge. Errors unless `0 <= pass_hz < stop_hz <= rate / 2` and the margin is at
/// least 0.
///
/// ```
/// use sdrkit_algo_dsp::fir::{lowpass, lowpass_with_margin};
/// let short = lowpass_with_margin(240e3, 53e3, 75e3, 60.0, 0.0).unwrap();
/// let usual = lowpass(240e3, 53e3, 75e3, 60.0).unwrap();
/// assert!(short.len() < usual.len());
/// ```
pub fn lowpass_with_margin(
    rate: f64,
    pass_hz: f64,
    stop_hz: f64,
    atten_db: f64,
    margin_db: f64,
) -> Result<Vec<f32>, crate::DspError> {
    check_edges(rate, pass_hz, stop_hz)?;
    let n = kaiser_len_with_margin(atten_db, (stop_hz - pass_hz) / rate, margin_db)?;
    Ok(to_f32(&windowed_sinc(
        n,
        (pass_hz + stop_hz) / 2.0 / rate,
        kaiser_beta(atten_db + margin_db),
    )?))
}

/// A highpass: attenuated below `stop_hz`, flat from `pass_hz` (spectral inversion of the matching lowpass). Errors unless
/// `0 <= stop_hz < pass_hz <= rate / 2` and `atten_db` is finite.
pub fn highpass(
    rate: f64,
    stop_hz: f64,
    pass_hz: f64,
    atten_db: f64,
) -> Result<Vec<f32>, crate::DspError> {
    let n = lowpass_len(rate, stop_hz, pass_hz, atten_db)?;
    let mut h = windowed_sinc(n, (pass_hz + stop_hz) / 2.0 / rate, design_beta(atten_db))?;
    h.iter_mut().for_each(|v| *v = -*v);
    h[n / 2] += 1.0;
    Ok(to_f32(&h))
}

fn bandpass_proto(
    rate: f64,
    low_hz: f64,
    high_hz: f64,
    transition_hz: f64,
    atten_db: f64,
) -> Result<(Vec<f64>, f64), crate::DspError> {
    if !(rate > 0.0 && rate.is_finite()) {
        return Param::Rate.positive(rate);
    }
    if high_hz <= low_hz || high_hz.is_nan() || low_hz.is_nan() {
        return Param::LowHz.conflict(Param::HighHz);
    }
    if !(transition_hz > 0.0 && transition_hz.is_finite()) {
        return Param::Transition.positive(transition_hz);
    }
    let half = (high_hz - low_hz) / 2.0;
    let n = kaiser_len(atten_db, transition_hz / rate)?;
    let cutoff = ((half + transition_hz / 2.0) / rate).min(0.5);
    Ok((
        windowed_sinc(n, cutoff, design_beta(atten_db))?,
        (low_hz + high_hz) / 2.0 / rate,
    ))
}

/// A complex band-pass for complex signals: passes `low_hz..high_hz` (either may be negative), with transition bands of
/// `transition_hz` outside each edge. It is the lowpass of half the width, shifted to the band's centre. Errors unless the rate
/// is positive, `low_hz < high_hz` and the transition is positive and below half the rate.
pub fn bandpass_complex(
    rate: f64,
    low_hz: f64,
    high_hz: f64,
    transition_hz: f64,
    atten_db: f64,
) -> Result<Vec<Complex32>, crate::DspError> {
    let (h, f0) = bandpass_proto(rate, low_hz, high_hz, transition_hz, atten_db)?;
    let c = (h.len() - 1) as f64 / 2.0;
    Ok(h.iter()
        .enumerate()
        .map(|(i, &v)| {
            let p = TAU * f0 * (i as f64 - c);
            Complex32::new((v * p.cos()) as f32, (v * p.sin()) as f32)
        })
        .collect())
}

/// A real band-pass passing `low_hz..high_hz` (and its mirror at negative frequencies). Errors unless
/// `0 < low_hz < high_hz < rate / 2` and the transition is positive and below half the rate.
pub fn bandpass_real(
    rate: f64,
    low_hz: f64,
    high_hz: f64,
    transition_hz: f64,
    atten_db: f64,
) -> Result<Vec<f32>, crate::DspError> {
    if low_hz <= 0.0 || low_hz.is_nan() {
        return Param::LowHz.positive(low_hz);
    }
    if high_hz >= rate / 2.0 || high_hz.is_nan() {
        return Param::HighHz.range(0.0, rate / 2.0, high_hz);
    }
    let (h, f0) = bandpass_proto(rate, low_hz, high_hz, transition_hz, atten_db)?;
    let c = (h.len() - 1) as f64 / 2.0;
    Ok(h.iter()
        .enumerate()
        .map(|(i, &v)| (2.0 * v * (TAU * f0 * (i as f64 - c)).cos()) as f32)
        .collect())
}

/// A Hilbert transformer of `num_taps` taps (odd; best of the form 4k+3, so the end taps are not zero): `2 / (pi m)` for odd offsets
/// `m` from the centre, Kaiser-windowed. The response is `-j sign(f)`: it turns `cos` into `sin`. Delay the other path by
/// `(num_taps - 1) / 2` samples to build an analytic signal. Errors unless `num_taps` is odd and at most [`MAX_TAPS`], and
/// `beta` is at least 0.
pub fn hilbert(num_taps: usize, beta: f64) -> Result<Vec<f32>, crate::DspError> {
    if num_taps > MAX_TAPS {
        return Param::Taps.range(1.0, MAX_TAPS as f64, num_taps as f64);
    }
    if num_taps % 2 != 1 {
        return Param::Taps.not_allowed(num_taps as f64);
    }
    if !(beta >= 0.0 && beta.is_finite()) {
        return Param::Beta.at_least(0.0, beta);
    }
    let w = kaiser(num_taps, beta);
    let c = (num_taps / 2) as i64;
    Ok((0..num_taps)
        .map(|i| {
            let m = i as i64 - c;
            if m % 2 == 0 {
                0.0
            } else {
                (2.0 / (PI * m as f64) * w[i]) as f32
            }
        })
        .collect())
}

/// The number of taps (4k+3) for a Hilbert transformer flat within the ripple from `transition_hz` to `rate/2 - transition_hz`.
/// Errors unless the rate and the transition are positive and `atten_db` is finite.
pub fn hilbert_len(rate: f64, transition_hz: f64, atten_db: f64) -> Result<usize, crate::DspError> {
    let n = kaiser_len(atten_db, (2.0 * transition_hz / rate).min(0.49))?;
    let k = (n + 1).div_ceil(4);
    Ok(4 * k - 1)
}

/// A Hilbert transformer for this specification (see [`hilbert_len`]). Errors as [`hilbert_len`] does.
pub fn hilbert_design(
    rate: f64,
    transition_hz: f64,
    atten_db: f64,
) -> Result<Vec<f32>, crate::DspError> {
    hilbert(
        hilbert_len(rate, transition_hz, atten_db)?,
        design_beta(atten_db),
    )
}

/// A windowed-sinc lowpass of exactly `num_taps` taps with its -6 dB point at `cutoff` cycles per sample (in `(0, 0.5]`), tapered
/// by the symmetric form of `window`, and scaled to unity gain at DC (`sum h = 1`). This is the design by tap count, beside
/// [`lowpass`] (design by pass band, stop band and attenuation). An even `num_taps` has a half-sample delay.
///
/// ```
/// use sdrkit_algo_dsp::fir::design_by_taps;
/// use sdrkit_algo_dsp::window::Window;
/// let h = design_by_taps(31, 0.1, Window::Hamming).unwrap();
/// assert!((h.iter().sum::<f32>() - 1.0).abs() < 1e-6);
/// ```
pub fn design_by_taps(
    num_taps: usize,
    cutoff: f64,
    window: crate::window::Window,
) -> Result<Vec<f32>, crate::DspError> {
    if num_taps == 0 || num_taps > 1 << 20 {
        return Param::Taps.range(1.0, (1u32 << 20) as f64, num_taps as f64);
    }
    if !(cutoff > 0.0 && cutoff <= 0.5) {
        return Param::Cutoff.range(0.0, 0.5, cutoff);
    }
    let c = (num_taps - 1) as f64 / 2.0;
    let h: Vec<f64> = (0..num_taps)
        .map(|i| {
            let m = i as f64 - c;
            2.0 * cutoff
                * crate::pulse::sinc(2.0 * cutoff * m)
                * window.value_symmetric(i, num_taps)
        })
        .collect();
    let s: f64 = h.iter().sum();
    if s.abs() < 1e-12 {
        return Param::Cutoff.degenerate();
    }
    Ok(h.iter().map(|&v| (v / s) as f32).collect())
}

/// The number of frequency points [`design_by_response`] and [`design_complex_by_response`] sample.
pub const RESPONSE_POINTS: usize = 4096;

/// A linear-phase FIR with real taps whose magnitude follows `h(f)`, `f` in Hz from 0 to `rate / 2` (the response is the same at
/// `-f`). It samples `h` at [`RESPONSE_POINTS`] frequencies, takes the inverse transform and tapers it with a Kaiser window of
/// `beta`. `num_taps` is made odd, so the delay is `num_taps / 2` samples. Errors unless `num_taps` is 1 to 65535, the rate is
/// positive and finite and `beta` is at least 0.
///
/// ```
/// use sdrkit_algo_dsp::fir::design_by_response;
/// // A flat response produces a single tap of 1 in the centre.
/// let h = design_by_response(11, 48_000.0, 6.0, |_| 1.0).unwrap();
/// assert!((h[5] - 1.0).abs() < 1e-6 && h[0].abs() < 1e-6);
/// ```
pub fn design_by_response(
    num_taps: usize,
    rate: f64,
    beta: f64,
    h: impl Fn(f64) -> f64,
) -> Result<Vec<f32>, crate::DspError> {
    let n = response_taps(num_taps, rate, beta)?;
    let m = RESPONSE_POINTS;
    let resp: Vec<f64> = (0..m)
        .map(|i| h((i as f64 + 0.5) / m as f64 * rate / 2.0))
        .collect();
    let w = kaiser(n, beta);
    let c = (n / 2) as f64;
    Ok((0..n)
        .map(|k| {
            let d = k as f64 - c;
            let s: f64 = resp
                .iter()
                .enumerate()
                .map(|(i, r)| r * (PI * (i as f64 + 0.5) / m as f64 * d).cos())
                .sum();
            (s / m as f64 * w[k]) as f32
        })
        .collect())
}

/// A complex FIR whose response follows `h(f)`, `f` in Hz from `-rate / 2` to `rate / 2`, for complex signals. It samples `h` at
/// [`RESPONSE_POINTS`] frequencies, takes the inverse transform and tapers it with a Kaiser window of `beta`. The phase of `h` is
/// kept, on top of the delay of `num_taps / 2` samples. `num_taps` is made odd. Errors as [`design_by_response`] does.
///
/// ```
/// use num_complex::Complex64;
/// use sdrkit_algo_dsp::fir::{design_complex_by_response, response};
/// // Pass only positive frequencies.
/// let h = design_complex_by_response(63, 1.0, 6.0, |f| Complex64::new(if f > 0.0 { 1.0 } else { 0.0 }, 0.0)).unwrap();
/// assert!(h.len() == 63);
/// ```
pub fn design_complex_by_response(
    num_taps: usize,
    rate: f64,
    beta: f64,
    h: impl Fn(f64) -> Complex64,
) -> Result<Vec<Complex32>, crate::DspError> {
    let n = response_taps(num_taps, rate, beta)?;
    let m = RESPONSE_POINTS;
    let freq = |i: usize| ((i as f64 + 0.5) / m as f64 - 0.5) * rate;
    let resp: Vec<Complex64> = (0..m).map(|i| h(freq(i))).collect();
    let w = kaiser(n, beta);
    let c = (n / 2) as f64;
    Ok((0..n)
        .map(|k| {
            let d = k as f64 - c;
            let s: Complex64 = resp
                .iter()
                .enumerate()
                .map(|(i, r)| r * Complex64::from_polar(1.0, TAU * freq(i) / rate * d))
                .sum();
            let v = s / m as f64 * w[k];
            Complex32::new(v.re as f32, v.im as f32)
        })
        .collect())
}

fn response_taps(num_taps: usize, rate: f64, beta: f64) -> Result<usize, crate::DspError> {
    if !(1..=65_535).contains(&num_taps) {
        return Param::Taps.range(1.0, 65_535.0, num_taps as f64);
    }
    if !(rate > 0.0 && rate.is_finite()) {
        return Param::Rate.positive(rate);
    }
    if !(beta >= 0.0 && beta.is_finite()) {
        return Param::Beta.at_least(0.0, beta);
    }
    Ok(num_taps | 1)
}

/// A backend of the FIR family: blocks of dot products of real taps with samples of one value (real) or two values (complex IQ
/// or stereo), at a fixed input stride. Filters, decimators and convolvers of this crate make their outputs with it.
///
/// A minimal backend, a scalar loop:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_dsp::fir::{check_backend, FirBackend};
///
/// struct Scalar;
/// impl Backend for Scalar {
///     fn name(&self) -> &'static str { "scalar" }
/// }
/// impl FirBackend for Scalar {
///     fn prepare(&self, taps: &[f32], _lanes: usize) -> Option<Vec<f32>> {
///         Some(taps.to_vec())
///     }
///     fn run(&self, taps: &[f32], lanes: usize, x: &[f32], stride: usize, out: &mut [f32]) {
///         for (i, o) in out.chunks_exact_mut(lanes).enumerate() {
///             for (c, o) in o.iter_mut().enumerate() {
///                 *o = taps.iter().enumerate().map(|(k, t)| t * x[(i * stride + k) * lanes + c]).sum();
///             }
///         }
///     }
/// }
///
/// check_backend(&Scalar);
/// ```
pub trait FirBackend: Backend {
    /// Lays out `taps` (the tap for the oldest sample first) for samples of `lanes` values (1 or 2). `None` when the backend
    /// cannot do it.
    fn prepare(&self, taps: &[f32], lanes: usize) -> Option<Vec<f32>>;
    /// Writes `outputs = out.len() / lanes` outputs. Lane `c` of output `i` is `sum_k taps[k] * x[(i * stride + k) * lanes + c]`
    /// over the `len` taps given to [`FirBackend::prepare`] with the same `lanes`, which made `prepared`. The layout of
    /// `prepared` must give `len` back (the plain backend uses `prepared.len() / lanes`). `x` holds at least
    /// `((outputs - 1) * stride + len) * lanes` values. Allocates nothing.
    fn run(&self, prepared: &[f32], lanes: usize, x: &[f32], stride: usize, out: &mut [f32]);
}

/// The FIR backends of this platform, the default first and the plain backend last.
pub static BACKENDS: Registry<dyn FirBackend> = Registry::new(TABLE);

#[cfg(target_arch = "aarch64")]
const TABLE: &[&dyn FirBackend] = &[&NeonFir, &PlainFir];
#[cfg(not(target_arch = "aarch64"))]
const TABLE: &[&dyn FirBackend] = &[&PlainFir];

/// Makes sure that `backend` agrees with the plain backend: 1, 7, 33 and 114 taps, strides 1, 2 and 5, one and two lanes, 1
/// and 50 outputs, on a fixed random signal. Each output must be within `1e-5` times the sum of the tap magnitudes of the
/// plain output (the input values are below 1).
///
/// # Panics
/// When an output differs by more than the tolerance, or when the backend prepares no layout.
pub fn check_backend(backend: &dyn FirBackend) {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut u = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    let mut prepared_any = false;
    for len in [1usize, 7, 33, 114] {
        let taps: Vec<f32> = (0..len).map(|_| u()).collect();
        let bound = 1e-5 * taps.iter().map(|t| t.abs()).sum::<f32>();
        for lanes in [1usize, 2] {
            let Some(p) = backend.prepare(&taps, lanes) else {
                continue;
            };
            prepared_any = true;
            let plain = PlainFir
                .prepare(&taps, lanes)
                .expect("the plain backend does 1 and 2 lanes");
            for stride in [1usize, 2, 5] {
                for outputs in [1usize, 50] {
                    let x: Vec<f32> = (0..((outputs - 1) * stride + len) * lanes)
                        .map(|_| u())
                        .collect();
                    let mut want = vec![0.0; outputs * lanes];
                    let mut got = vec![f32::NAN; outputs * lanes];
                    PlainFir.run(&plain, lanes, &x, stride, &mut want);
                    backend.run(&p, lanes, &x, stride, &mut got);
                    for (g, w) in got.iter().zip(&want) {
                        assert!(
                            (g - w).abs() <= bound,
                            "{} {len} taps, {lanes} lanes, stride {stride}: {g} for {w}",
                            backend.name()
                        );
                    }
                }
            }
        }
    }
    assert!(prepared_any, "{} prepares no layout", backend.name());
}

/// The plain FIR backend: taps laid out per lane ([`crate::sample::lane_taps`]) and the vectorised dot products of [`Sample`].
struct PlainFir;

impl Backend for PlainFir {
    fn name(&self) -> &'static str {
        "plain"
    }
}

impl FirBackend for PlainFir {
    fn prepare(&self, taps: &[f32], lanes: usize) -> Option<Vec<f32>> {
        match lanes {
            1 => Some(crate::sample::lane_taps::<f32>(taps)),
            2 => Some(crate::sample::lane_taps::<Complex32>(taps)),
            _ => None,
        }
    }

    fn run(&self, prepared: &[f32], lanes: usize, x: &[f32], stride: usize, out: &mut [f32]) {
        let len = prepared.len() / lanes;
        if lanes == 1 {
            for (i, o) in out.iter_mut().enumerate() {
                *o = f32::dot_lanes(prepared, &x[i * stride..i * stride + len]);
            }
        } else {
            let x = x.as_chunks::<2>().0;
            for (i, o) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                let (a, b) = crate::sample::dot_flat2(prepared, &x[i * stride..i * stride + len]);
                *o = [a, b];
            }
        }
    }
}

/// The NEON FIR backend (ARM64): the taps as they are and the dot products of [`crate::neon`], two outputs at a time.
#[cfg(target_arch = "aarch64")]
struct NeonFir;

#[cfg(target_arch = "aarch64")]
impl Backend for NeonFir {
    fn name(&self) -> &'static str {
        "neon"
    }
}

#[cfg(target_arch = "aarch64")]
impl FirBackend for NeonFir {
    fn prepare(&self, taps: &[f32], lanes: usize) -> Option<Vec<f32>> {
        matches!(lanes, 1 | 2).then(|| taps.to_vec())
    }

    fn run(&self, prepared: &[f32], lanes: usize, x: &[f32], stride: usize, out: &mut [f32]) {
        if lanes == 1 {
            crate::neon::fir_real(prepared, x, stride, out);
        } else {
            crate::neon::fir_pairs(prepared, x, stride, out);
        }
    }
}

/// How a [`FirKernel`] computes one output.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// The plain dot products of [`Sample`], on taps laid out per lane.
    Plain,
    /// The dot products of [`crate::neon`], on the taps as they are.
    #[cfg(target_arch = "aarch64")]
    Neon,
    /// A call of the backend.
    Backend,
}

/// The taps of one filter, laid out by the backend that runs it. Sample types without a value view ([`Sample::values`])
/// run on the plain dot products of [`Sample`].
#[derive(Clone)]
pub(crate) struct FirKernel<T: Sample> {
    backend: &'static dyn FirBackend,
    /// The taps as the backend laid them out.
    prepared: Vec<f32>,
    len: usize,
    kind: Kind,
    _t: std::marker::PhantomData<T>,
}

impl<T: Sample> std::fmt::Debug for FirKernel<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FirKernel")
            .field("backend", &self.backend.name())
            .field("len", &self.len)
            .finish()
    }
}

impl<T: Sample> FirKernel<T> {
    /// A kernel of `taps` (the tap for the oldest sample first, at least one), with the first backend of [`BACKENDS`] that
    /// prepares them.
    pub(crate) fn new(taps: &[f32]) -> Self {
        let has_values = T::values(&[T::default()]).is_some();
        let (backend, prepared) = BACKENDS
            .build(|b| {
                if has_values {
                    b.prepare(taps, T::LANES).map(|p| (b, p))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| (BACKENDS.plain(), crate::sample::lane_taps::<T>(taps)));
        let kind = if BACKENDS.is_plain(backend) {
            Kind::Plain
        } else {
            #[cfg(target_arch = "aarch64")]
            if backend.name() == NeonFir.name() {
                return Self::with(backend, prepared, taps.len(), Kind::Neon);
            }
            Kind::Backend
        };
        Self::with(backend, prepared, taps.len(), kind)
    }

    fn with(backend: &'static dyn FirBackend, prepared: Vec<f32>, len: usize, kind: Kind) -> Self {
        FirKernel {
            backend,
            prepared,
            len,
            kind,
            _t: std::marker::PhantomData,
        }
    }

    /// The number of taps.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Replaces the taps with `taps` of the same length, given newest-sample first. Allocates nothing on the plain and the
    /// NEON backends.
    pub(crate) fn set_taps_newest_first(&mut self, taps: &[f32]) {
        debug_assert_eq!(taps.len(), self.len);
        match self.kind {
            Kind::Plain => {
                for (w, t) in self
                    .prepared
                    .chunks_exact_mut(T::LANES)
                    .zip(taps.iter().rev())
                {
                    w.fill(*t);
                }
            }
            #[cfg(target_arch = "aarch64")]
            Kind::Neon => {
                for (w, t) in self.prepared.iter_mut().zip(taps.iter().rev()) {
                    *w = *t;
                }
            }
            Kind::Backend => {
                let rtaps: Vec<f32> = taps.iter().rev().copied().collect();
                *self = Self::new(&rtaps);
            }
        }
    }

    /// The output for the window `w` (exactly `len()` samples, oldest first).
    #[inline(always)]
    pub(crate) fn one(&self, w: &[T]) -> T {
        match self.kind {
            Kind::Plain => T::dot_lanes(&self.prepared, w),
            #[cfg(target_arch = "aarch64")]
            Kind::Neon => self.one_on_neon(w),
            Kind::Backend => self.one_on_backend(w),
        }
    }

    /// [`FirKernel::one`] on the NEON dot products. The kernel has this kind only for samples of one or two values.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    fn one_on_neon(&self, w: &[T]) -> T {
        let mut y = [T::default()];
        if let (Some(x), Some(o)) = (T::values(w), T::values_mut(&mut y)) {
            if T::LANES == 1 {
                o[0] = crate::neon::dot_real(&self.prepared, x);
            } else {
                o[..2].copy_from_slice(&crate::neon::dot_pairs(&self.prepared, x));
            }
        }
        y[0]
    }

    #[cold]
    #[inline(never)]
    fn one_on_backend(&self, w: &[T]) -> T {
        let mut y = [T::default()];
        self.run(w, 1, &mut y);
        y[0]
    }

    /// Writes `out.len()` outputs: output `i` is made from the window `x[i * stride..i * stride + len()]`.
    #[inline]
    pub(crate) fn run(&self, x: &[T], stride: usize, out: &mut [T]) {
        if out.is_empty() {
            return;
        }
        debug_assert!(x.len() >= (out.len() - 1) * stride + self.len);
        if let Some(xv) = T::values(x) {
            if let Some(ov) = T::values_mut(out) {
                self.backend.run(&self.prepared, T::LANES, xv, stride, ov);
                return;
            }
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = T::dot_lanes(&self.prepared, &x[i * stride..i * stride + self.len]);
        }
    }
}

/// Convolution of a whole stored buffer with real taps, for burst receivers: [`Convolver::centred`] returns the zero-phase ("same")
/// output aligned with the input, so a symmetric filter adds no delay and symbol `k` stays at sample `k`. The taps are reversed and
/// laid out per lane once, so each output is one contiguous dot product that vectorises.
#[derive(Clone, Debug)]
pub struct Convolver<T: Sample> {
    kernel: FirKernel<T>,
    /// `taps` reversed, plain (for the edges).
    rtaps: Vec<f32>,
}

impl<T: Sample> Convolver<T> {
    /// A convolver with these taps. Errors for no taps or a tap that is not finite.
    pub fn new(taps: &[f32]) -> Result<Self, crate::DspError> {
        if taps.is_empty() {
            return Param::Taps.at_least(1.0, 0.0);
        }
        if taps.iter().any(|t| !t.is_finite()) {
            return Param::Taps.not_finite();
        }
        let rtaps: Vec<f32> = taps.iter().rev().copied().collect();
        Ok(Convolver {
            kernel: FirKernel::new(&rtaps),
            rtaps,
        })
    }

    /// The number of taps.
    pub fn len(&self) -> usize {
        self.rtaps.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The centre tap index `c = (len - 1) / 2` (rounded down) that [`centred`](Self::centred) aligns with the input.
    pub fn centre(&self) -> usize {
        (self.rtaps.len() - 1) / 2
    }

    /// `out[n] = sum_k taps[k] x[n + c - k]`, with `c` the [`centre`](Self::centre) and `x` zero outside the slice. Errors when
    /// `out` is not as long as `x`. Allocates nothing.
    pub fn centred(&self, x: &[T], out: &mut [T]) -> Result<(), crate::DspError> {
        if out.len() != x.len() {
            return Param::Output.length(x.len(), out.len());
        }
        let l = self.rtaps.len();
        let c = self.centre();
        // Output n uses the window x[n + c + 1 - l ..= n + c], oldest first. It lies inside x for n in lo..hi.
        let lo = l - 1 - c;
        let hi = x.len().saturating_sub(c).max(lo);
        if lo < hi {
            self.kernel.run(x, 1, &mut out[lo..hi]);
        }
        for n in (0..lo.min(x.len())).chain(hi..x.len()) {
            let first = (n + c + 1) as isize - l as isize;
            let mut acc = T::default();
            for (j, &t) in self.rtaps.iter().enumerate() {
                let i = first + j as isize;
                if i >= 0 && (i as usize) < x.len() {
                    acc += x[i as usize] * t;
                }
            }
            out[n] = acc;
        }
        Ok(())
    }
}

/// The centred ("same") convolution of `x` with `taps` into `out` (see [`Convolver::centred`]). It builds the reversed taps on each
/// call; keep a [`Convolver`] to reuse them.
pub fn centred_convolve<T: Sample>(
    x: &[T],
    taps: &[f32],
    out: &mut [T],
) -> Result<(), crate::DspError> {
    Convolver::<T>::new(taps)?.centred(x, out)
}

/// The complex response of real taps at `f` cycles per sample (relative to the first tap).
pub fn response(taps: &[f32], f: f64) -> Complex64 {
    taps.iter()
        .enumerate()
        .map(|(i, &h)| Complex64::from_polar(h as f64, -TAU * f * i as f64))
        .sum()
}

fn to_f32(h: &[f64]) -> Vec<f32> {
    h.iter().map(|&v| v as f32).collect()
}

/// A streaming FIR filter with real taps, for `f32`, [`Complex32`] or [`Stereo`](crate::Stereo) samples.
///
/// The history and the new input sit in one buffer, so every output is one contiguous dot product, and a block of outputs is one
/// call to the FIR backend.
#[derive(Clone, Debug)]
pub struct Fir<T: Sample> {
    taps: Vec<f32>,
    kernel: FirKernel<T>,
    /// The last `len()` inputs at the end; older ones before them until the next move.
    buf: Vec<T>,
}

/// The most outputs a block call makes on the stack before it appends them to the caller's output.
pub(crate) const CHUNK: usize = 64;

/// Appends `x` to `v`. When `v` must grow, it gets room for twice what it holds then, so blocks that vary a little in length
/// do not make it grow again.
pub(crate) fn extend_with_room<T: Copy>(v: &mut Vec<T>, x: &[T]) {
    let need = v.len() + x.len();
    if need > v.capacity() {
        v.reserve_exact(2 * need - v.len());
    }
    v.extend_from_slice(x);
}

/// How many samples a streaming filter lets its history grow by before it moves the history back.
#[inline]
pub(crate) fn push_room(n: usize) -> usize {
    n.max(256)
}

impl<T: Sample> Fir<T> {
    /// A filter with these taps (`taps[0]` multiplies the newest sample).
    ///
    /// # Panics
    /// If `taps` is empty.
    pub fn new(taps: &[f32]) -> Self {
        assert!(!taps.is_empty(), "a FIR filter needs at least one tap");
        let n = taps.len();
        let rtaps: Vec<f32> = taps.iter().rev().copied().collect();
        let mut f = Fir {
            taps: taps.to_vec(),
            kernel: FirKernel::new(&rtaps),
            buf: Vec::with_capacity(n + push_room(n)),
        };
        f.reset();
        f
    }

    /// The taps.
    pub fn taps(&self) -> &[f32] {
        &self.taps
    }

    /// The number of taps.
    pub fn len(&self) -> usize {
        self.taps.len()
    }

    /// Always false: a filter has at least one tap.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The delay of a symmetric (linear-phase) design, in samples: `(N - 1) / 2`.
    pub fn group_delay(&self) -> f64 {
        (self.taps.len() - 1) as f64 / 2.0
    }

    /// Clears the history to zeros.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.buf.resize(self.taps.len(), T::default());
    }

    /// Replaces the taps. The history is kept when the length is the same, cleared otherwise.
    pub fn set_taps(&mut self, taps: &[f32]) {
        if taps.len() == self.taps.len() {
            self.taps.copy_from_slice(taps);
            self.kernel.set_taps_newest_first(taps);
        } else {
            *self = Fir::new(taps);
        }
    }

    /// Keeps only the last `len()` samples of the buffer.
    fn trim(&mut self) {
        let n = self.taps.len();
        let drop = self.buf.len() - n;
        if drop > 0 {
            self.buf.copy_within(drop.., 0);
            self.buf.truncate(n);
        }
    }

    /// Pushes one sample into the history without computing an output.
    #[inline]
    pub fn push(&mut self, x: T) {
        let n = self.taps.len();
        if self.buf.len() >= n + push_room(n) {
            self.trim();
        }
        self.buf.push(x);
    }

    /// The output for the samples pushed so far.
    #[inline]
    pub fn output(&self) -> T {
        self.kernel.one(self.window())
    }

    /// The oldest-to-newest history window (`len()` samples).
    #[inline]
    pub fn window(&self) -> &[T] {
        &self.buf[self.buf.len() - self.taps.len()..]
    }

    /// Pushes one sample and returns the output.
    #[inline]
    pub fn process_sample(&mut self, x: T) -> T {
        self.push(x);
        self.output()
    }

    /// Filters `input` into `out` (the same length).
    ///
    /// # Panics
    /// If the lengths differ.
    pub fn process(&mut self, input: &[T], out: &mut [T]) {
        assert_eq!(input.len(), out.len());
        let first = self.buf.len() + 1 - self.taps.len();
        extend_with_room(&mut self.buf, input);
        self.kernel.run(&self.buf[first..], 1, out);
        self.trim();
    }

    /// Filters `buf` in place.
    pub fn process_in_place(&mut self, buf: &mut [T]) {
        let first = self.buf.len() + 1 - self.taps.len();
        extend_with_room(&mut self.buf, buf);
        self.kernel.run(&self.buf[first..], 1, buf);
        self.trim();
    }
}

/// A streaming FIR filter with complex taps on complex samples (for example a [`bandpass_complex`] design).
#[derive(Clone, Debug)]
pub struct ComplexFir {
    rtaps: Vec<Complex32>,
    hist: Vec<Complex32>,
    idx: usize,
}

impl ComplexFir {
    /// A filter with these taps (`taps[0]` multiplies the newest sample).
    pub fn new(taps: &[Complex32]) -> Self {
        assert!(!taps.is_empty(), "a FIR filter needs at least one tap");
        ComplexFir {
            rtaps: taps.iter().rev().copied().collect(),
            hist: vec![Complex32::default(); 2 * taps.len()],
            idx: 0,
        }
    }

    /// The number of taps.
    pub fn len(&self) -> usize {
        self.rtaps.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Clears the history.
    pub fn reset(&mut self) {
        self.hist.iter_mut().for_each(|v| *v = Complex32::default());
        self.idx = 0;
    }

    /// Pushes one sample and returns the output.
    #[inline]
    pub fn process_sample(&mut self, x: Complex32) -> Complex32 {
        let n = self.rtaps.len();
        self.hist[self.idx] = x;
        self.hist[self.idx + n] = x;
        self.idx += 1;
        if self.idx == n {
            self.idx = 0;
        }
        let w = &self.hist[self.idx..self.idx + n];
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (t, s) in self.rtaps.iter().zip(w) {
            re += t.re * s.re - t.im * s.im;
            im += t.re * s.im + t.im * s.re;
        }
        Complex32::new(re, im)
    }

    /// Filters `buf` in place.
    pub fn process_in_place(&mut self, buf: &mut [Complex32]) {
        for v in buf.iter_mut() {
            *v = self.process_sample(*v);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_backend_agrees_with_the_plain_one() {
        for &b in super::BACKENDS.all() {
            super::check_backend(b);
        }
    }

    #[test]
    fn design_by_response_matches_the_windowed_sinc_and_keeps_the_phase() {
        // An ideal lowpass sampled at the grid produces the windowed sinc (to the grid's sampling error).
        let (n, rate, fc, beta) = (61, 48_000.0, 6_000.0, 6.0);
        let h = design_by_response(n, rate, beta, |f| if f < fc { 1.0 } else { 0.0 }).unwrap();
        let s = windowed_sinc(n, fc / rate, beta).unwrap();
        for (a, b) in h.iter().zip(&s) {
            assert!((*a as f64 - b).abs() < 1e-3, "{a} {b}");
        }
        assert_eq!(
            design_by_response(60, rate, beta, |_| 1.0).unwrap().len(),
            61
        );
        // The complex design of a real, even response produces the same taps.
        let c = design_complex_by_response(n, rate, beta, |f| {
            Complex64::new(if f.abs() < fc { 1.0 } else { 0.0 }, 0.0)
        })
        .unwrap();
        for (a, b) in c.iter().zip(&h) {
            assert!((a.re - b).abs() < 1e-3 && a.im.abs() < 1e-3);
        }
        // A complex response passes one side only.
        let c = design_complex_by_response(127, rate, beta, |f| {
            Complex64::new(if f > 1_000.0 && f < 9_000.0 { 1.0 } else { 0.0 }, 0.0)
        })
        .unwrap();
        let g = |f: f64| {
            c.iter()
                .enumerate()
                .map(|(k, t)| {
                    Complex64::new(t.re as f64, t.im as f64)
                        * Complex64::from_polar(1.0, -TAU * f / rate * k as f64)
                })
                .sum::<Complex64>()
                .norm()
        };
        assert!((g(5_000.0) - 1.0).abs() < 0.01);
        assert!(g(-5_000.0) < 0.01);
        assert!(design_by_response(0, rate, beta, |_| 1.0).is_err());
        assert!(
            design_complex_by_response(11, f64::NAN, beta, |_| Complex64::new(1.0, 0.0)).is_err()
        );
    }

    #[test]
    fn bad_design_values_give_errors_not_panics() {
        assert!(lowpass(48_000.0, 3_000.0, 30_000.0, 60.0).is_err());
        assert!(lowpass(48_000.0, 4_000.0, 3_000.0, 60.0).is_err());
        assert!(lowpass(0.0, 1.0, 2.0, 60.0).is_err());
        assert!(lowpass(48_000.0, 3_000.0, 3_000.001, 60.0).is_err());
        assert!(lowpass_len(48_000.0, 3_000.0, 4_000.0, f64::NAN).is_err());
        assert!(highpass(48_000.0, 300.0, 200.0, 60.0).is_err());
        assert!(bandpass_complex(48_000.0, 2_000.0, 1_000.0, 100.0, 60.0).is_err());
        assert!(bandpass_real(48_000.0, -1.0, 1_000.0, 100.0, 60.0).is_err());
        assert!(windowed_sinc(0, 0.1, 6.0).is_err());
        assert!(hilbert(10, 6.0).is_err());
        assert!(kaiser_len(60.0, 0.0).is_err());
    }

    #[test]
    fn the_margin_option_returns_kaisers_published_estimate() {
        // Kaiser 1974: N = (A - 7.95) / (14.36 df) + 1, the quotient rounded up. A = 80, df = 0.1: 50.2 -> 51 + 1 = 52 -> 53 (odd).
        // A = 40, df = 0.02: 111.6 -> 112 + 1 = 113.
        assert_eq!(kaiser_len_with_margin(80.0, 0.1, 0.0).unwrap(), 53);
        assert_eq!(kaiser_len_with_margin(40.0, 0.02, 0.0).unwrap(), 113);
        // The usual design is the same formula with the 3 dB margin.
        for (a, df) in [(60.0, 0.05), (80.0, 0.013), (30.0, 0.2)] {
            assert_eq!(
                kaiser_len_with_margin(a, df, DESIGN_MARGIN_DB).unwrap(),
                kaiser_len(a, df).unwrap()
            );
        }
        assert_eq!(
            lowpass_with_margin(2.4e6, 110e3, 190e3, 62.0, DESIGN_MARGIN_DB).unwrap(),
            lowpass(2.4e6, 110e3, 190e3, 62.0).unwrap()
        );
        // Without the margin the filter is shorter, still flat in the pass band and close to the requested attenuation.
        let h = lowpass_with_margin(240e3, 53e3, 75e3, 60.0, 0.0).unwrap();
        assert_eq!(
            h.len(),
            kaiser_len_with_margin(60.0, 22e3 / 240e3, 0.0).unwrap()
        );
        assert!(h.len() < lowpass(240e3, 53e3, 75e3, 60.0).unwrap().len());
        let pass = (0..=53)
            .map(|k| response(&h, k as f64 * 1e3 / 240e3).norm())
            .fold(0.0f64, |m, v| m.max((v - 1.0).abs()));
        assert!(pass < 0.01, "pass band ripple {pass}");
        let stop = (75..=120)
            .map(|k| response(&h, k as f64 * 1e3 / 240e3).norm())
            .fold(0.0f64, f64::max);
        assert!(
            20.0 * stop.log10() < -56.0,
            "stop band {} dB",
            20.0 * stop.log10()
        );
        assert!(kaiser_len_with_margin(60.0, 0.0, 0.0).is_err());
        assert!(kaiser_len_with_margin(60.0, 0.1, -1.0).is_err());
        assert!(lowpass_with_margin(240e3, 80e3, 75e3, 60.0, 0.0).is_err());
        assert!(lowpass_with_margin(240e3, 53e3, 130e3, 60.0, 0.0).is_err());
    }

    use super::*;
    use crate::testutil::*;

    /// Feeds a tone through a streaming filter and returns the steady-state gain in dB.
    fn tone_gain_db(taps: &[f32], f: f64) -> f64 {
        let n = taps.len() + 2048;
        let x = ctone(n, f, 1.0);
        let mut fir = Fir::<Complex32>::new(taps);
        let mut y = vec![Complex32::default(); n];
        fir.process(&x, &mut y);
        db(camp(&y[taps.len()..], f))
    }

    #[test]
    fn lowpass_meets_spec_measured_with_tones() {
        let (rate, pass, stop, att) = (48_000.0, 4_000.0, 6_000.0, 70.0);
        let h = lowpass(rate, pass, stop, att).unwrap();
        assert_eq!(h.len(), lowpass_len(rate, pass, stop, att).unwrap());
        for f in [0.0, 1000.0, 2500.0, 3900.0] {
            let g = tone_gain_db(&h, f / rate);
            assert!(g.abs() < 0.05, "passband {f} Hz: {g} dB");
        }
        for f in [6000.0, 6500.0, 9000.0, 15000.0, 23000.0] {
            let g = tone_gain_db(&h, f / rate);
            assert!(g < -att, "stopband {f} Hz: {g} dB");
        }
        // Dense check of the design with the analytic response.
        let mut worst_pass: f64 = 0.0;
        let mut worst_stop: f64 = -400.0;
        for i in 0..=400 {
            let f = i as f64 / 800.0;
            let g = db(response(&h, f).norm());
            if f * rate <= pass {
                worst_pass = worst_pass.max(g.abs());
            } else if f * rate >= stop {
                worst_stop = worst_stop.max(g);
            }
        }
        assert!(worst_pass < 0.01, "ripple {worst_pass}");
        assert!(worst_stop < -att, "stopband {worst_stop}");
    }

    #[test]
    fn highpass_and_bandpass() {
        let rate = 48_000.0;
        let h = highpass(rate, 300.0, 1000.0, 60.0).unwrap();
        assert!(db(response(&h, 100.0 / rate).norm()) < -59.0);
        assert!(db(response(&h, 3000.0 / rate).norm()).abs() < 0.02);

        let bp = bandpass_real(rate, 5000.0, 8000.0, 1000.0, 60.0).unwrap();
        assert!(db(response(&bp, 6500.0 / rate).norm()).abs() < 0.02);
        assert!(db(response(&bp, 3500.0 / rate).norm()) < -59.0);
        assert!(db(response(&bp, 9500.0 / rate).norm()) < -59.0);

        // Complex band-pass on the negative side: passes -8..-5 kHz, rejects +6.5 kHz.
        let cb = bandpass_complex(rate, -8000.0, -5000.0, 1000.0, 60.0).unwrap();
        let mut f = ComplexFir::new(&cb);
        let n = cb.len() + 4096;
        for (freq, want_pass) in [(-6500.0, true), (6500.0, false), (-2000.0, false)] {
            f.reset();
            let mut x = ctone(n, freq / rate, 1.0);
            f.process_in_place(&mut x);
            let g = db(camp(&x[cb.len()..], freq / rate));
            if want_pass {
                assert!(g.abs() < 0.05, "{freq}: {g}");
            } else {
                assert!(g < -59.0, "{freq}: {g}");
            }
        }
    }

    #[test]
    fn hilbert_builds_an_analytic_signal() {
        let rate = 48_000.0;
        let h = hilbert_design(rate, 500.0, 60.0).unwrap();
        assert_eq!(h.len() % 4, 3);
        let d = h.len() / 2;
        for f in [1000.0, 7000.0, 18000.0] {
            let n = h.len() + 4096;
            let x = rtone(n, f / rate, 1.0);
            let mut fir = Fir::<f32>::new(&h);
            let mut q = vec![0.0; n];
            fir.process(&x, &mut q);
            let a: Vec<Complex32> = (h.len()..n)
                .map(|i| Complex32::new(x[i - d], q[i]))
                .collect();
            let pos = camp(&a, f / rate);
            let neg = camp(&a, -f / rate);
            assert!((db(pos)).abs() < 0.02, "{f}: positive {}", db(pos));
            assert!(db(neg) < -55.0, "{f}: image {}", db(neg));
        }
    }

    #[test]
    fn design_by_taps_has_unit_dc_and_its_cutoff() {
        use crate::window::Window;
        for w in [
            Window::Hamming,
            Window::Blackman,
            Window::Hann,
            Window::kaiser(6.0),
        ] {
            let h = design_by_taps(101, 0.125, w).unwrap();
            assert_eq!(h.len(), 101);
            assert!((response(&h, 0.0).norm() - 1.0).abs() < 1e-6);
            // -6 dB at the cut-off (the windowed sinc's half-amplitude point).
            let g = db(response(&h, 0.125).norm());
            assert!((g + 6.02).abs() < 0.1, "{w:?}: {g}");
            assert!(db(response(&h, 0.25).norm()) < -40.0, "{w:?}");
            for i in 0..101 {
                assert!((h[i] - h[100 - i]).abs() < 1e-7);
            }
        }
        // Hamming's stop band is near its -53 dB design figure; Blackman's near -74 dB.
        let worst = |h: &[f32]| {
            (0..200)
                .map(|i| db(response(h, 0.2 + 0.3 * i as f64 / 200.0).norm()))
                .fold(-400.0, f64::max)
        };
        assert!(worst(&design_by_taps(101, 0.125, Window::Hamming).unwrap()) < -50.0);
        assert!(worst(&design_by_taps(101, 0.125, Window::Blackman).unwrap()) < -70.0);
        assert!(design_by_taps(0, 0.1, Window::Hann).is_err());
        assert!(design_by_taps(11, 0.6, Window::Hann).is_err());
    }

    #[test]
    fn centred_convolution_is_aligned_and_matches_the_definition() {
        let taps = [0.1f32, 0.2, 0.4, 0.2, 0.1, -0.05];
        let x: Vec<f32> = (0..50).map(|i| ((i * 7) % 11) as f32 - 5.0).collect();
        let mut out = vec![0.0; 50];
        centred_convolve(&x, &taps, &mut out).unwrap();
        let c = 2isize;
        for n in 0..50isize {
            let mut want = 0.0;
            for (k, &t) in taps.iter().enumerate() {
                let i = n + c - k as isize;
                if (0..50).contains(&i) {
                    want += t * x[i as usize];
                }
            }
            assert!((out[n as usize] - want).abs() < 1e-5, "{n}");
        }
        // A symmetric filter keeps an impulse where it was.
        let mut imp = vec![Complex32::default(); 21];
        imp[10] = Complex32::new(1.0, 0.0);
        let mut y = vec![Complex32::default(); 21];
        let conv = Convolver::<Complex32>::new(&[0.25, 0.5, 0.25]).unwrap();
        conv.centred(&imp, &mut y).unwrap();
        assert_eq!(y[10].re, 0.5);
        assert_eq!(y[9].re, 0.25);
        assert!(conv.centred(&imp, &mut y[..3]).is_err());
    }

    #[test]
    fn streaming_in_blocks_matches_one_block() {
        let h = lowpass(1.0, 0.1, 0.2, 50.0).unwrap();
        let x = ctone(500, 0.13, 1.0);
        let mut a = Fir::<Complex32>::new(&h);
        let mut ya = vec![Complex32::default(); 500];
        a.process(&x, &mut ya);
        let mut b = Fir::<Complex32>::new(&h);
        let mut yb = Vec::new();
        for chunk in x.chunks(7) {
            let mut c = chunk.to_vec();
            b.process_in_place(&mut c);
            yb.extend(c);
        }
        assert_eq!(ya, yb);
    }
}
