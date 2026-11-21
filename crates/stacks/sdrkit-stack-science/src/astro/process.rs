//! Processing of power spectra: velocity, the DC bridge, the band shape and the fold, a polynomial baseline, smoothing and the
//! peak. Also the calibration, the RFI flags, Gaussian line fits, the longitude-velocity map, and the bytes of the files a host
//! writes (CSV and SigMF text; FITS files are written with `sdrkit-codec-fits`).

// Index loops read the spectra and matrices the way the formulas are written.
#![allow(clippy::needless_range_loop)]

use sdrkit_algo_dsp::speckurt::{sk_flag, sk_sigma, spectral_kurtosis};

/// The speed of light, km/s.
pub const C_KMS: f64 = 299_792.458;
/// H I rest frequency, Hz.
pub const HI_HZ: f64 = 1_420_405_751.0;

/// Radio-convention velocity of a frequency (positive receding), km/s.
pub fn velocity(f: f64, f0: f64) -> f64 {
    C_KMS * (f0 - f) / f0
}

/// The frequency of a velocity.
pub fn freq_of(v: f64, f0: f64) -> f64 {
    f0 * (1.0 - v / C_KMS)
}

/// Replaces bins centre ± `n` by a straight line between their neighbours (n < 0: nothing).
pub fn dc_bridge(p: &mut [f64], n: i32) {
    if n < 0 || p.len() < 4 {
        return;
    }
    let c = p.len() / 2;
    let (a, b) = (
        c.saturating_sub(n as usize + 1),
        (c + n as usize + 1).min(p.len() - 1),
    );
    let (va, vb) = (p[a], p[b]);
    for i in a + 1..b {
        p[i] = va + (vb - va) * (i - a) as f64 / (b - a) as f64;
    }
}

/// Solves a small symmetric positive definite system (normal equations) with `sdrkit_algo_dsp::linalg::cholesky_solve`. Returns
/// `None` when the matrix is not positive definite.
pub fn solve(a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    let mut flat: Vec<f64> = a.into_iter().flatten().collect();
    sdrkit_algo_dsp::linalg::cholesky_solve(&mut flat, &mut b, n).ok()?;
    Some(b)
}

/// A least-squares polynomial of `degree` through the points with `use_pt` true, x scaled to −1…1; returns its values everywhere.
pub fn baseline(y: &[f64], degree: i32, use_pt: &[bool]) -> Option<Vec<f64>> {
    if degree < 0 {
        return Some(vec![0.0; y.len()]);
    }
    let n = y.len();
    let d = degree as usize + 1;
    let xs = |i: usize| 2.0 * i as f64 / (n - 1).max(1) as f64 - 1.0;
    let mut ata = vec![vec![0.0; d]; d];
    let mut atb = vec![0.0; d];
    for i in 0..n {
        if !use_pt[i] {
            continue;
        }
        let x = xs(i);
        let mut pw = vec![1.0; d];
        for k in 1..d {
            pw[k] = pw[k - 1] * x;
        }
        for r in 0..d {
            atb[r] += pw[r] * y[i];
            for c in 0..d {
                ata[r][c] += pw[r] * pw[c];
            }
        }
    }
    let c = solve(ata, atb)?;
    Some(
        (0..n)
            .map(|i| {
                let x = xs(i);
                c.iter().rev().fold(0.0, |acc, &k| acc * x + k)
            })
            .collect(),
    )
}

/// A boxcar of `n` bins (edges shortened). An even `n` puts one bin more before the centre than after, so this is not
/// `sdrkit_algo_dsp::stats::centred_mean`, whose window is always odd.
pub fn smooth(y: &[f64], n: usize) -> Vec<f64> {
    if n <= 1 {
        return y.to_vec();
    }
    let h = n / 2;
    (0..y.len())
        .map(|i| {
            let a = i.saturating_sub(h);
            let b = (i + n - h).min(y.len());
            y[a..b].iter().sum::<f64>() / (b - a) as f64
        })
        .collect()
}

/// How the band shape is taken out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BandShape {
    /// The spectrum over its median, minus 1.
    None,
    /// (on − off) / off with a captured off-source average.
    OffSource,
    /// (a − b) / b with the shifted phase, then folded by `shift_bins`.
    Switching(isize),
}

/// Options of the processing.
#[derive(Clone, Debug)]
pub struct ProcOptions {
    /// Rest frequency.
    pub rest_hz: f64,
    /// Centre frequency.
    pub centre_hz: f64,
    /// Sample rate.
    pub rate: f64,
    /// DC bridge half-width.
    pub dc_bins: i32,
    /// How the band shape is taken out.
    pub band_shape: BandShape,
    /// Fold the negative image back (switching).
    pub fold: bool,
    /// Baseline degree (−1 none).
    pub degree: i32,
    /// Exclude and search the window.
    pub window: bool,
    /// Window, km/s.
    pub window_kms: (f64, f64),
    /// Edge fraction.
    pub edge: f64,
    /// Smoothing bins.
    pub smooth: usize,
}

impl Default for ProcOptions {
    fn default() -> Self {
        ProcOptions {
            rest_hz: HI_HZ,
            centre_hz: HI_HZ,
            rate: 2.5e6,
            dc_bins: 2,
            band_shape: BandShape::None,
            fold: true,
            degree: 3,
            window: true,
            window_kms: (-150.0, 150.0),
            edge: 0.08,
            smooth: 1,
        }
    }
}

/// A processed spectrum.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Processed {
    /// Frequency of each bin.
    pub freq: Vec<f64>,
    /// Velocity of each bin.
    pub vel: Vec<f64>,
    /// The ratio after the band shape and the baseline are taken out.
    pub ratio: Vec<f64>,
    /// The baseline that was subtracted.
    pub base: Vec<f64>,
    /// Peak frequency (centroid of the part above half).
    pub peak_hz: f64,
    /// Peak velocity.
    pub peak_kms: f64,
    /// Excess at the peak (ratio).
    pub excess: f64,
    /// Residual rms outside the window.
    pub rms: f64,
    /// SNR = peak / rms.
    pub snr: f64,
}

/// Processes an averaged power spectrum (lowest frequency first, centre bin N/2).
pub fn process(on: &[f64], off: Option<&[f64]>, o: &ProcOptions) -> Processed {
    let n = on.len();
    let df = o.rate / n as f64;
    let freq: Vec<f64> = (0..n)
        .map(|i| o.centre_hz + (i as f64 - (n / 2) as f64) * df)
        .collect();
    let vel: Vec<f64> = freq.iter().map(|&f| velocity(f, o.rest_hz)).collect();
    let mut a = on.to_vec();
    dc_bridge(&mut a, o.dc_bins);
    let mut r: Vec<f64> = match (o.band_shape, off) {
        (BandShape::OffSource, Some(b)) | (BandShape::Switching(_), Some(b)) => {
            let mut b = b.to_vec();
            dc_bridge(&mut b, o.dc_bins);
            a.iter()
                .zip(&b)
                .map(|(x, y)| if *y > 0.0 { (x - y) / y } else { 0.0 })
                .collect()
        }
        _ => {
            let mut s = a.clone();
            s.sort_by(|x, y| x.total_cmp(y));
            let med = s[n / 2].max(1e-300);
            a.iter().map(|x| x / med - 1.0).collect()
        }
    };
    if let (BandShape::Switching(shift), true) = (o.band_shape, o.fold) {
        // (a − b)/b puts the line at +1 and its negative image `shift` bins away: average the line with the inverted image.
        let r0 = r.clone();
        for (i, v) in r.iter_mut().enumerate() {
            let j = i as isize - shift;
            if j >= 0 && (j as usize) < n {
                *v = 0.5 * (r0[i] - r0[j as usize]);
            }
        }
    }
    let ew = (o.edge * n as f64) as usize;
    let in_window = |i: usize| o.window && vel[i] >= o.window_kms.0 && vel[i] <= o.window_kms.1;
    let use_pt: Vec<bool> = (0..n)
        .map(|i| i >= ew && i + ew < n && !in_window(i))
        .collect();
    let base = baseline(&r, o.degree, &use_pt).unwrap_or_else(|| vec![0.0; n]);
    for (v, b) in r.iter_mut().zip(&base) {
        *v -= b;
    }
    let r = smooth(&r, o.smooth);
    let search: Vec<usize> = (ew..n.saturating_sub(ew))
        .filter(|&i| !o.window || in_window(i))
        .collect();
    let pk = search
        .iter()
        .copied()
        .max_by(|&x, &y| r[x].total_cmp(&r[y]))
        .unwrap_or(n / 2);
    let peak = r[pk];
    // Centroid of the contiguous part above half the peak.
    let (mut lo, mut hi) = (pk, pk);
    while lo > 0 && r[lo - 1] > peak / 2.0 {
        lo -= 1;
    }
    while hi + 1 < n && r[hi + 1] > peak / 2.0 {
        hi += 1;
    }
    let (mut sw, mut sf) = (0.0, 0.0);
    for i in lo..=hi {
        sw += r[i];
        sf += r[i] * freq[i];
    }
    let peak_hz = if sw > 0.0 { sf / sw } else { freq[pk] };
    let outside: Vec<f64> = (0..n).filter(|&i| use_pt[i]).map(|i| r[i]).collect();
    let rms = (outside.iter().map(|v| v * v).sum::<f64>() / outside.len().max(1) as f64).sqrt();
    Processed {
        peak_kms: velocity(peak_hz, o.rest_hz),
        peak_hz,
        excess: peak,
        rms,
        snr: if rms > 0.0 { peak / rms } else { 0.0 },
        freq,
        vel,
        ratio: r,
        base,
    }
}

/// The Y-factor: (Trx, gain), or `None` when the hot power does not exceed the cold one.
pub fn y_factor(p_hot: f64, p_cold: f64, t_hot: f64, t_cold: f64) -> Option<(f64, f64)> {
    if p_hot <= p_cold {
        return None;
    }
    let y = p_hot / p_cold;
    Some((
        (t_hot - y * t_cold) / (y - 1.0),
        (p_hot - p_cold) / (t_hot - t_cold),
    ))
}

/// A running median of `w` bins.
pub fn running_median(y: &[f64], w: usize) -> Vec<f64> {
    let h = w / 2;
    let mut buf = Vec::with_capacity(w);
    (0..y.len())
        .map(|i| {
            let a = i.saturating_sub(h);
            let b = (i + h + 1).min(y.len());
            buf.clear();
            buf.extend_from_slice(&y[a..b]);
            buf.sort_by(|x, z| x.total_cmp(z));
            buf[buf.len() / 2]
        })
        .collect()
}

/// RFI flags: the residual from a 31-bin running median beyond 5 sigmas, with sigma from the median deviation (1.4826 × MAD).
pub fn rfi_flags(y: &[f64]) -> Vec<bool> {
    let med = running_median(y, 31);
    let res: Vec<f64> = y.iter().zip(&med).map(|(a, b)| a - b).collect();
    let mut abs: Vec<f64> = res.iter().map(|v| v.abs()).collect();
    abs.sort_by(|a, b| a.total_cmp(b));
    let sigma = 1.4826 * abs[abs.len() / 2];
    res.iter().map(|r| r.abs() > 5.0 * sigma).collect()
}

/// Flagged bins replaced by a straight line between the nearest good ones.
pub fn interpolate_flags(y: &mut [f64], flags: &[bool]) {
    let n = y.len();
    let mut i = 0;
    while i < n {
        if !flags[i] {
            i += 1;
            continue;
        }
        let a = i.checked_sub(1);
        let mut b = i;
        while b < n && flags[b] {
            b += 1;
        }
        let (va, vb) = (a.map(|a| (a, y[a])), (b < n).then(|| y[b]));
        for k in i..b {
            y[k] = match (va, vb) {
                (Some((a, x)), Some(z)) => x + (z - x) * (k - a) as f64 / (b - a) as f64,
                (Some((_, x)), None) => x,
                (None, Some(z)) => z,
                _ => 0.0,
            };
        }
        i = b;
    }
}

/// Spectral kurtosis per channel from the sums `s1`, `s2` of M frames; flagged beyond 5 expected sigmas from 1.
pub fn kurtosis_flags(s1: &[f64], s2: &[f64], m: usize) -> Vec<bool> {
    let sigma = sk_sigma(m, 1);
    s1.iter()
        .zip(s2)
        .map(|(&a, &b)| sk_flag(spectral_kurtosis(a, b, m), sigma, 5.0).is_some())
        .collect()
}

/// One Gaussian component.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Gauss {
    /// Amplitude and its uncertainty.
    pub amp: (f64, f64),
    /// Centre.
    pub centre: (f64, f64),
    /// Sigma.
    pub sigma: (f64, f64),
}

impl Gauss {
    /// FWHM.
    pub fn fwhm(&self) -> f64 {
        2.354_820_045 * self.sigma.0
    }
    /// Area and its uncertainty.
    pub fn area(&self) -> (f64, f64) {
        let a = self.amp.0 * self.sigma.0 * 2.506_628_275;
        let rel =
            ((self.amp.1 / self.amp.0).powi(2) + (self.sigma.1 / self.sigma.0).powi(2)).sqrt();
        (a, a.abs() * rel)
    }
}

/// A fit result.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Fit {
    /// Components.
    pub components: Vec<Gauss>,
    /// Constant offset.
    pub offset: (f64, f64),
    /// Residual rms.
    pub rms: f64,
}

fn model(p: &[f64], x: f64) -> f64 {
    let mut y = p[0];
    for k in 0..(p.len() - 1) / 3 {
        let (a, m, s) = (p[1 + 3 * k], p[2 + 3 * k], p[3 + 3 * k]);
        y += a * (-(x - m).powi(2) / (2.0 * s * s)).exp();
    }
    y
}

/// Fits Gaussians (Levenberg–Marquardt) with start values from the strongest separated peaks above 4 × the noise (from the median
/// deviation). `None` when no clear line stands above the noise.
pub fn fit_gaussians(x: &[f64], y: &[f64], max_components: usize) -> Option<Fit> {
    let n = x.len();
    let mut s: Vec<f64> = y.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let med = s[n / 2];
    let mut dev: Vec<f64> = y.iter().map(|v| (v - med).abs()).collect();
    dev.sort_by(|a, b| a.total_cmp(b));
    let noise = 1.4826 * dev[n / 2];
    let sm = smooth(y, 5);
    let dx = (x[n - 1] - x[0]).abs() / (n - 1) as f64;
    let mut peaks: Vec<(f64, usize)> = (1..n - 1)
        .filter(|&i| sm[i] >= sm[i - 1] && sm[i] >= sm[i + 1] && sm[i] - med > 4.0 * noise)
        .map(|i| (sm[i] - med, i))
        .collect();
    peaks.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut starts: Vec<(f64, usize)> = Vec::new();
    for p in peaks {
        if starts
            .iter()
            .all(|s| (x[s.1] - x[p.1]).abs() > 10.0 * dx.max(1e-12) * 4.0)
            && starts.len() < max_components
        {
            starts.push(p);
        }
    }
    if starts.is_empty() {
        return None;
    }
    let mut p = vec![med];
    for &(a, i) in &starts {
        // Width from the half-maximum crossing.
        let mut k = i;
        while k + 1 < n && sm[k] - med > a / 2.0 {
            k += 1;
        }
        let sig = ((x[k] - x[i]).abs() / 1.1774).max(2.0 * dx);
        p.extend_from_slice(&[a, x[i], sig]);
    }
    let np = p.len();
    let mut lambda = 1e-3;
    let resid = |p: &[f64]| -> f64 {
        x.iter()
            .zip(y)
            .map(|(&xi, &yi)| (yi - model(p, xi)).powi(2))
            .sum()
    };
    let mut cost = resid(&p);
    let mut jtj = vec![vec![0.0; np]; np];
    for _ in 0..200 {
        let mut j = vec![vec![0.0; np]; n];
        for (i, &xi) in x.iter().enumerate() {
            for k in 0..np {
                let h = 1e-6 * p[k].abs().max(1e-6);
                let mut q = p.clone();
                q[k] += h;
                j[i][k] = (model(&q, xi) - model(&p, xi)) / h;
            }
        }
        jtj = vec![vec![0.0; np]; np];
        let mut jtr = vec![0.0; np];
        for i in 0..n {
            let r = y[i] - model(&p, x[i]);
            for a in 0..np {
                jtr[a] += j[i][a] * r;
                for b in 0..np {
                    jtj[a][b] += j[i][a] * j[i][b];
                }
            }
        }
        let mut aug = jtj.clone();
        for (a, row) in aug.iter_mut().enumerate() {
            row[a] *= 1.0 + lambda;
        }
        let Some(dp) = solve(aug, jtr) else { break };
        let q: Vec<f64> = p.iter().zip(&dp).map(|(a, b)| a + b).collect();
        let c2 = resid(&q);
        if c2 < cost {
            let done = (cost - c2) / cost < 1e-12;
            p = q;
            cost = c2;
            lambda *= 0.3;
            if done {
                break;
            }
        } else {
            lambda *= 10.0;
            if lambda > 1e12 {
                break;
            }
        }
    }
    let dof = (n as f64 - np as f64).max(1.0);
    let rms = (cost / n as f64).sqrt();
    let s2 = cost / dof;
    // Covariance = s² (JᵀJ)⁻¹ (diagonal by solving unit vectors).
    let mut err = vec![0.0; np];
    for (k, e) in err.iter_mut().enumerate() {
        let mut ek = vec![0.0; np];
        ek[k] = 1.0;
        if let Some(col) = solve(jtj.clone(), ek) {
            *e = (s2 * col[k].abs()).sqrt();
        }
    }
    let components = (0..(np - 1) / 3)
        .map(|k| Gauss {
            amp: (p[1 + 3 * k], err[1 + 3 * k]),
            centre: (p[2 + 3 * k], err[2 + 3 * k]),
            sigma: (p[3 + 3 * k].abs(), err[3 + 3 * k]),
        })
        .collect();
    Some(Fit {
        components,
        offset: (p[0], err[0]),
        rms,
    })
}

/// The longitude–velocity map on a −250 … +250 km/s grid (1 km/s), one row per galactic longitude added.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LvMap {
    /// Rows: (longitude, temperatures on the grid).
    pub rows: Vec<(f64, Vec<f64>)>,
}

impl LvMap {
    /// The grid.
    pub fn grid() -> Vec<f64> {
        (-250..=250).map(|v| v as f64).collect()
    }
    /// Adds a spectrum (velocity, temperature) at longitude `l` (resampled on the grid).
    pub fn add(&mut self, l: f64, vel: &[f64], t: &[f64]) {
        let mut pts: Vec<(f64, f64)> = vel.iter().copied().zip(t.iter().copied()).collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let row = Self::grid()
            .iter()
            .map(|&v| {
                let k = pts.partition_point(|p| p.0 < v);
                if k == 0 || k >= pts.len() {
                    0.0
                } else {
                    let (a, b) = (pts[k - 1], pts[k]);
                    a.1 + (b.1 - a.1) * (v - a.0) / (b.0 - a.0)
                }
            })
            .collect();
        self.rows.retain(|r| (r.0 - l).abs() > 1e-9);
        self.rows.push((l, row));
        self.rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    /// Terminal velocities (l, v_term) for the inner galaxy: the extreme velocity where the temperature stays above 5 median-deviation
    /// sigmas for at least 3 channels (highest for l 0–90, lowest for 270–360).
    pub fn terminal(&self) -> Vec<(f64, f64)> {
        let g = Self::grid();
        self.rows
            .iter()
            .filter_map(|(l, row)| {
                let first = *l > 0.0 && *l < 90.0;
                let fourth = *l > 270.0 && *l < 360.0;
                if !first && !fourth {
                    return None;
                }
                let mut dev: Vec<f64> = row.iter().map(|v| v.abs()).collect();
                dev.sort_by(|a, b| a.total_cmp(b));
                let sigma = 1.4826 * dev[dev.len() / 2].max(1e-12);
                let above: Vec<bool> = row.iter().map(|&v| v > 5.0 * sigma).collect();
                let idx: Vec<usize> = (0..row.len() - 2)
                    .filter(|&i| above[i] && above[i + 1] && above[i + 2])
                    .collect();
                let v = if first {
                    idx.last().map(|&i| g[i + 2])
                } else {
                    idx.first().map(|&i| g[i])
                };
                v.map(|v| (*l, v))
            })
            .collect()
    }
}

/// The rotation curve from terminal velocities (tangent points): R = R0 |sin l|, V = |v_term| + V0 |sin l|.
pub fn rotation_curve(term: &[(f64, f64)], r0: f64, v0: f64) -> Vec<(f64, f64)> {
    term.iter()
        .map(|&(l, v)| {
            let s = (l.to_radians()).sin().abs();
            (r0 * s, v.abs() + v0 * s)
        })
        .collect()
}

/// The CSV of a processed spectrum: comment lines, a header, then `freq_hz,velocity_kms,power_db,ratio,baseline`.
pub fn csv(p: &Processed, power: &[f64], o: &ProcOptions, integration_s: f64) -> String {
    let mut s = String::new();
    s += &format!(
        "# centre_hz {}\n# rest_hz {}\n# rate {}\n# fft {}\n# integration_s {}\n",
        o.centre_hz,
        o.rest_hz,
        o.rate,
        power.len(),
        integration_s
    );
    s += "freq_hz,velocity_kms,power_db,ratio,baseline\n";
    for i in 0..power.len() {
        s += &format!(
            "{:.3},{:.4},{:.4},{:.8},{:.8}\n",
            p.freq[i],
            p.vel[i],
            10.0 * power[i].max(1e-300).log10(),
            p.ratio[i],
            p.base[i]
        );
    }
    s
}

/// The header values and the rows of five numbers of a CSV file.
pub type CsvTable = (Vec<(String, f64)>, Vec<[f64; 5]>);

/// Why a file of this module does not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileError {
    /// A value is not a number, on this line (from 1).
    NotANumber {
        /// The line.
        line: usize,
    },
    /// A row does not have the number of columns of the file, on this line (from 1).
    Columns {
        /// The line.
        line: usize,
    },
    /// The column header line is missing.
    NoHeader,
    /// The text is not a session file.
    NotASession,
    /// A line of a session file does not read, on this line (from 1).
    BadLine {
        /// The line.
        line: usize,
    },
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::NotANumber { line } => write!(f, "line {line}: not a number"),
            FileError::Columns { line } => write!(f, "line {line}: wrong number of columns"),
            FileError::NoHeader => f.write_str("no column header"),
            FileError::NotASession => f.write_str("not a session file"),
            FileError::BadLine { line } => write!(f, "line {line} does not read"),
        }
    }
}

impl std::error::Error for FileError {}

/// Reads the CSV back: the header values and the rows of five numbers.
///
/// # Errors
///
/// [`FileError`] for a line that does not read, or a file without the column header.
pub fn read_csv(text: &str) -> Result<CsvTable, FileError> {
    let mut head = Vec::new();
    let mut rows = Vec::new();
    let mut header_seen = false;
    for (n, l) in text.lines().enumerate() {
        let line = n + 1;
        if let Some(c) = l.strip_prefix("# ") {
            let mut it = c.split_whitespace();
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                let v = v
                    .parse::<f64>()
                    .map_err(|_| FileError::NotANumber { line })?;
                head.push((k.to_string(), v));
            }
            continue;
        }
        if l.starts_with("freq_hz") {
            header_seen = true;
            continue;
        }
        if l.trim().is_empty() {
            continue;
        }
        let v: Result<Vec<f64>, _> = sdrkit_util_text::csv::split_line(l, ',')
            .iter()
            .map(|x| x.parse::<f64>())
            .collect();
        let v = v.map_err(|_| FileError::NotANumber { line })?;
        if v.len() != 5 {
            return Err(FileError::Columns { line });
        }
        rows.push([v[0], v[1], v[2], v[3], v[4]]);
    }
    if !header_seen {
        return Err(FileError::NoHeader);
    }
    Ok((head, rows))
}

/// The SigMF metadata text with the line fits as annotations.
pub fn sigmf_meta(centre_hz: f64, rate: f64, fit: &Fit) -> String {
    let ann: Vec<String> = fit
        .components
        .iter()
        .map(|g| {
            format!(
                "{{\"core:sample_start\": 0, \"core:comment\": \"Gaussian at {:.2} km/s, FWHM {:.2} km/s, {:.3} K\"}}",
                g.centre.0,
                g.fwhm(),
                g.amp.0
            )
        })
        .collect();
    format!(
        "{{\n  \"global\": {{\"core:datatype\": \"cf32_le\", \"core:sample_rate\": {rate:.1}, \"core:version\": \"1.0.0\"}},\n  \"captures\": [{{\"core:sample_start\": 0, \"core:frequency\": {centre_hz:.3}}}],\n  \"annotations\": [{}]\n}}\n",
        ann.join(", ")
    )
}

/// A session file: key=value lines then the rows.
pub fn session_write(values: &[(String, String)], rows: &[Vec<f64>]) -> String {
    let mut s = String::from("hline-session 1\n");
    for (k, v) in values {
        s += &format!("{k}={v}\n");
    }
    s += "rows\n";
    for r in rows {
        s += &r
            .iter()
            .map(|x| format!("{x:e}"))
            .collect::<Vec<_>>()
            .join(" ");
        s += "\n";
    }
    s
}

/// Reads a session file.
pub type Session = (Vec<(String, String)>, Vec<Vec<f64>>);

/// Reads a session file back.
///
/// # Errors
///
/// [`FileError::NotASession`] for text of another kind, [`FileError::BadLine`] for a line that does not read.
pub fn session_read(text: &str) -> Result<Session, FileError> {
    let mut lines = text.lines();
    if lines.next() != Some("hline-session 1") {
        return Err(FileError::NotASession);
    }
    let mut vals = Vec::new();
    let mut rows = Vec::new();
    let mut in_rows = false;
    for (n, l) in lines.enumerate() {
        let line = n + 2;
        if in_rows {
            let r: Result<Vec<f64>, _> = l.split_whitespace().map(|x| x.parse()).collect();
            rows.push(r.map_err(|_| FileError::BadLine { line })?);
        } else if l == "rows" {
            in_rows = true;
        } else {
            let (k, v) = l.split_once('=').ok_or(FileError::BadLine { line })?;
            vals.push((k.to_string(), v.to_string()));
        }
    }
    Ok((vals, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocities() {
        let f = freq_of(-42.0, HI_HZ);
        assert!((velocity(f, HI_HZ) + 42.0).abs() < 1e-9);
        assert!((velocity(HI_HZ + 100e3, HI_HZ) + 21.106).abs() < 0.01);
    }

    #[test]
    fn calibration_and_rfi() {
        let (trx, gain) = y_factor(2.0 * 390.0, 2.0 * 90.0, 300.0, 0.0).unwrap();
        assert!((trx - 90.0).abs() < 1e-9 && (gain - 2.0).abs() < 1e-12);
        assert!(y_factor(1.0, 2.0, 300.0, 10.0).is_none());
        let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
        let mut y: Vec<f64> = (0..2048).map(|_| 1.0 + 0.01 * rng.normal()).collect();
        y[300] += 1.0;
        y[1500] += 0.5;
        let f = rfi_flags(&y);
        assert!(f[300] && f[1500]);
        assert!(f.iter().filter(|&&x| x).count() <= 6);
        let mut z = y.clone();
        interpolate_flags(&mut z, &f);
        assert!((z[300] - 1.0).abs() < 0.1);
        // Kurtosis: channel 40 of 256 is a strong steady tone (not Gaussian).
        let m = 400;
        let mut s1 = vec![0.0; 256];
        let mut s2 = vec![0.0; 256];
        for _ in 0..m {
            for c in 0..256 {
                let p = if c == 40 { 1.0 } else { rng.exponential() };
                s1[c] += p;
                s2[c] += p * p;
            }
        }
        let k = kurtosis_flags(&s1, &s2, m);
        assert!(k[40] && k.iter().filter(|&&x| x).count() <= 3);
    }

    #[test]
    fn two_gaussians() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(11);
        let x: Vec<f64> = (0..400).map(|i| -150.0 + i as f64).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|&v| {
                0.2 + 6.0 * (-(v + 20.0f64).powi(2) / 128.0).exp()
                    + 3.0 * (-(v - 60.0f64).powi(2) / 450.0).exp()
                    + 0.3 * rng.normal()
            })
            .collect();
        let f = fit_gaussians(&x, &y, 4).unwrap();
        assert_eq!(f.components.len(), 2, "{f:?}");
        let mut c = f.components.clone();
        c.sort_by(|a, b| a.centre.0.total_cmp(&b.centre.0));
        assert!((c[0].centre.0 + 20.0).abs() < 4.0 * c[0].centre.1 && c[0].centre.1 < 0.5);
        assert!((c[1].centre.0 - 60.0).abs() < 4.0 * c[1].centre.1);
        assert!((c[0].amp.0 - 6.0).abs() < 4.0 * c[0].amp.1);
        assert!((c[0].sigma.0 - 8.0).abs() < 4.0 * c[0].sigma.1);
        let (a, ae) = c[0].area();
        assert!((a - 6.0 * 8.0 * 2.5066).abs() < 4.0 * ae);
        assert!((f.rms - 0.3).abs() < 0.03);
    }

    #[test]
    fn files() {
        let p = Processed {
            freq: vec![1.0, 2.0],
            vel: vec![3.0, 4.0],
            ratio: vec![0.5, 0.25],
            base: vec![0.0, 0.1],
            ..Default::default()
        };
        let t = csv(&p, &[1.0, 2.0], &ProcOptions::default(), 10.0);
        let (h, rows) = read_csv(&t).unwrap();
        assert_eq!(h.len(), 5);
        assert!((rows[1][3] - 0.25).abs() < 1e-8);
        assert!(read_csv("garbage\n1,2").is_err());
        // The FITS files of the stack go through the shared writer: a primary header, an image and a binary table.
        use sdrkit_codec_fits::{self as fits, Card, Column, ColumnType, Data, Hdu};
        let mut prim = Hdu::empty();
        prim.cards = vec![Card::value("TELESCOP", "sdrkit"), Card::value("RESTFRQ", HI_HZ)];
        let mut img = Hdu::image(-32, vec![3, 2], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        img.cards.push(Card::value("EXTNAME", "DRIFT"));
        let col = |name: &str, unit: &str, values: Vec<f64>| Column {
            name: name.into(),
            unit: unit.into(),
            kind: ColumnType::F64,
            repeat: 1,
            values,
        };
        let mut tab = Hdu::table(2, vec![col("VELO", "km/s", vec![1.5, 2.5]), col("TA", "K", vec![0.1, 0.2])]);
        tab.cards.push(Card::value("EXTNAME", "SPECTRUM"));
        let f = fits::write(&[prim, img, tab]);
        assert_eq!(f.len() % 2880, 0);
        assert!(f.starts_with(b"SIMPLE  =                    T"));
        let h = fits::read(&f).unwrap();
        assert_eq!(h[0].str("TELESCOP"), Some("sdrkit"));
        let Data::Image { pixels, .. } = &h[1].data else {
            panic!("no image")
        };
        assert_eq!(pixels, &vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let ta = h[2].column("TA").unwrap();
        assert_eq!((ta.unit.as_str(), ta.values[1]), ("K", 0.2));
        assert!(fits::read(b"junk").is_err());
        let m = sigmf_meta(
            HI_HZ,
            2.5e6,
            &Fit {
                components: vec![Gauss {
                    amp: (1.0, 0.0),
                    centre: (-42.0, 0.0),
                    sigma: (8.5, 0.0),
                }],
                ..Default::default()
            },
        );
        assert!(m.contains("\"core:frequency\": 1420405751.000") && m.contains("FWHM"));
        let s = session_write(&[("rest".into(), "1420405751".into())], &[vec![1.0, 2.0]]);
        let (v, r) = session_read(&s).unwrap();
        assert_eq!((v[0].1.as_str(), r[0][1]), ("1420405751", 2.0));
        assert!(session_read("x").is_err());
    }

    #[test]
    fn lv_map_of_a_model_galaxy() {
        let (r0, v0) = (8.5, 220.0);
        let mut map = LvMap::default();
        for l in (5..90).step_by(5).chain((275..360).step_by(5)) {
            let l = l as f64;
            let s = l.to_radians().sin();
            let vt = v0 - v0 * s.abs();
            let vel: Vec<f64> = (-250..=250).map(|v| v as f64).collect();
            let t: Vec<f64> = vel
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let inside = if s > 0.0 {
                        v >= 0.0 && v <= vt
                    } else {
                        v <= 0.0 && v >= -vt
                    };
                    (if inside { 20.0 } else { 0.0 })
                        + 0.2 * (((i * 7919) % 13) as f64 / 13.0 - 0.5)
                })
                .collect();
            map.add(l, &vel, &t);
        }
        let mut outer = map.clone();
        outer.add(180.0, &[0.0, 1.0], &[1.0, 1.0]);
        let term = map.terminal();
        assert!(term.len() >= 12);
        for (rr, vv) in rotation_curve(&term, r0, v0) {
            assert!((vv - v0).abs() < 12.0, "{rr} {vv}");
        }
        assert!(outer.terminal().iter().all(|t| t.0 != 180.0));
    }
}
