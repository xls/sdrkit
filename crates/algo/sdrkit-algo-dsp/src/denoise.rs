//! Noise filters for pictures sent as scan lines (slow-scan TV, facsimile, weather satellite lines).
//!
//! | Function | What it does |
//! |---|---|
//! | [`line_wiener`] | a local Wiener filter along a line: each sample moves towards the mean of its neighbourhood by the share of the local variance that the noise explains |
//! | [`line_clicks`], [`robust_sigma`] | a sample far (in robust standard deviations) from the median of the samples around it is replaced by that median |
//! | [`LeeNoise`] | the noise of a picture of 8-bit levels from the median of a 3 x 3 Laplacian |
//! | [`picture_clicks`], [`picture_lee`], [`PictureLee`] | click removal against the median of the eight neighbours, and the 3 x 3 local Wiener (Lee) filter, row by row or over a whole plane |
//!
//! Each filter leaves a clean signal unchanged: a noise of zero (or a measured noise under a small limit) copies the input.
//! Ends of a line and borders of a picture repeat the edge sample. Nothing allocates per sample.
//!
//! ```
//! use sdrkit_algo_dsp::denoise::line_wiener;
//! let line = [10.0f32, 10.0, 10.0, 30.0, 10.0, 10.0, 10.0];
//! let mut out = [0f32; 7];
//! line_wiener(&line, 0.0, 3, &mut out).unwrap();
//! assert_eq!(out, line); // no noise: unchanged
//! ```

use crate::{DspError, Param};

/// The half width of the line Wiener filter: 7 samples.
pub const LINE_WIENER_HALF: usize = 3;
/// A local variance under this is taken as no detail: the gain is 0.
pub const LINE_WIENER_MIN_VAR: f64 = 1e-9;
/// The click limit on a line, in robust standard deviations from the median of 5.
pub const LINE_CLICK_SIGMAS: f32 = 4.0;
/// The factor from the median absolute deviation to the standard deviation of Gaussian noise.
pub const MAD_TO_SIGMA: f64 = 1.4826;
/// The values [`LeeNoise`] needs before it gives a noise.
pub const LEE_MIN_VALUES: u32 = 2000;
/// Under this noise (levels) the picture filters copy the row.
pub const LEE_MIN_SIGMA: f32 = 1.5;
/// From this noise (levels) up, [`PictureLee`] removes clicks before the Wiener step.
pub const LEE_CLICK_SIGMA: f32 = 30.0;
/// The click limit in a picture, in noise standard deviations from the median of the eight neighbours.
pub const PICTURE_CLICK_SIGMAS: f32 = 2.5;
/// The noise variance of the 3 x 3 Wiener step, in units of the measured `sigma^2` (FM noise rises with the pixel rate).
pub const LEE_NOISE_FACTOR: f32 = 2.0;
/// Histogram bins of [`LeeNoise`], one level each; larger values fall in the last bin.
pub const LEE_BINS: usize = 1024;

/// Line Wiener filter: for each sample `i` of `line`, over `line[i - half ..= i + half]` (indices clipped at the ends), the mean
/// `mu` and variance `v`; the gain `g = max(0, v - noise_var) / v` (0 when `v` is under [`LINE_WIENER_MIN_VAR`]); the output
/// `mu + g (line[i] - mu)`. `noise_var` is the noise variance of one sample. A `noise_var` of 0 or less copies the line. Errors
/// unless `out` has the length of `line` and `noise_var` is not NaN.
pub fn line_wiener(
    line: &[f32],
    noise_var: f32,
    half: usize,
    out: &mut [f32],
) -> Result<(), DspError> {
    if out.len() != line.len() {
        return Param::Output.length(line.len(), out.len());
    }
    if noise_var.is_nan() {
        return Param::Power.not_finite();
    }
    if noise_var <= 0.0 || line.is_empty() {
        out.copy_from_slice(line);
        return Ok(());
    }
    let n = line.len() as isize;
    let h = half as isize;
    let at = |j: isize| line[j.clamp(0, n - 1) as usize] as f64;
    let len = (2 * h + 1) as f64;
    let (mut s, mut ss) = (0f64, 0f64);
    for j in -h..=h {
        let v = at(j);
        s += v;
        ss += v * v;
    }
    let nv = noise_var as f64;
    for i in 0..n {
        if i > 0 {
            let (a, b) = (at(i + h), at(i - h - 1));
            s += a - b;
            ss += a * a - b * b;
        }
        // Re-sum exactly now and then, so the running sums do not drift on long lines.
        if i % 1024 == 1023 {
            (s, ss) = (0.0, 0.0);
            for j in i - h..=i + h {
                let v = at(j);
                s += v;
                ss += v * v;
            }
        }
        let mu = s / len;
        let v = (ss / len - mu * mu).max(0.0);
        let g = if v < LINE_WIENER_MIN_VAR {
            0.0
        } else {
            (v - nv).max(0.0) / v
        };
        out[i as usize] = (mu + g * (line[i as usize] as f64 - mu)) as f32;
    }
    Ok(())
}

/// The median of five values.
#[inline]
fn median5(mut v: [f32; 5]) -> f32 {
    v.sort_unstable_by(f32::total_cmp);
    v[2]
}

/// Click removal on a line: a sample more than `k sigma` from the median of the 5 samples around it (itself included, indices
/// clipped at the ends) is replaced by that median ([`LINE_CLICK_SIGMAS`] is the usual `k`). A `sigma` of 0 or less copies the
/// line. Returns the number of samples replaced. Errors unless `out` has the length of `line`.
pub fn line_clicks(line: &[f32], sigma: f32, k: f32, out: &mut [f32]) -> Result<usize, DspError> {
    if out.len() != line.len() {
        return Param::Output.length(line.len(), out.len());
    }
    out.copy_from_slice(line);
    if sigma.is_nan() || sigma <= 0.0 || line.is_empty() {
        return Ok(0);
    }
    let n = line.len() as isize;
    let at = |j: isize| line[j.clamp(0, n - 1) as usize];
    let lim = k * sigma;
    let mut count = 0;
    for i in 0..n {
        let m = median5([at(i - 2), at(i - 1), at(i), at(i + 1), at(i + 2)]);
        if (line[i as usize] - m).abs() > lim {
            out[i as usize] = m;
            count += 1;
        }
    }
    Ok(count)
}

/// The robust standard deviation of `values`: [`MAD_TO_SIGMA`] times the median absolute deviation from the median (of a stretch
/// known to be flat). `None` for no values. `scratch` grows to the number of values once.
pub fn robust_sigma(values: &[f32], scratch: &mut Vec<f64>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    scratch.clear();
    scratch.extend(values.iter().map(|&v| v as f64));
    let k = values.len() / 2;
    let med = *scratch.select_nth_unstable_by(k, f64::total_cmp).1;
    for v in scratch.iter_mut() {
        *v = (*v - med).abs();
    }
    let mad = *scratch.select_nth_unstable_by(k, f64::total_cmp).1;
    Some((MAD_TO_SIGMA * mad) as f32)
}

/// The pixel at column `x` (clipped at the ends).
#[inline]
fn px(row: &[u8], x: isize) -> u8 {
    row[x.clamp(0, row.len() as isize - 1) as usize]
}

/// The noise of a picture of 8-bit levels: a histogram of `|L|`, `L` the 3 x 3 Laplacian `[1 -2 1; -2 4 -2; 1 -2 1]` (whose
/// output has 6 times the standard deviation of white noise), over every second pixel of the rows given; the noise is
/// `sigma = 1.4826 median |L| / 6`. Keep one per colour.
#[derive(Clone, Debug)]
pub struct LeeNoise {
    hist: Vec<u32>,
    count: u32,
}

impl Default for LeeNoise {
    fn default() -> Self {
        Self::new()
    }
}

impl LeeNoise {
    /// An empty measurement.
    pub fn new() -> Self {
        LeeNoise {
            hist: vec![0; LEE_BINS],
            count: 0,
        }
    }

    /// Forgets every value.
    pub fn reset(&mut self) {
        self.hist.fill(0);
        self.count = 0;
    }

    /// The values taken.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Adds the Laplacian of every second inner pixel (columns 1, 3, 5, ...) of the raw row `row`, with the raw rows `above` and
    /// `below` (all of one width). Errors when the widths differ.
    pub fn add_row(&mut self, above: &[u8], row: &[u8], below: &[u8]) -> Result<(), DspError> {
        if above.len() != row.len() || below.len() != row.len() {
            return Param::Row.length(row.len(), above.len().max(below.len()));
        }
        let w = row.len();
        let mut x = 1;
        while x + 1 < w {
            let l = |r: &[u8]| r[x - 1] as i32 - 2 * r[x] as i32 + r[x + 1] as i32;
            let lap = l(above) - 2 * l(row) + l(below);
            self.hist[(lap.unsigned_abs() as usize).min(LEE_BINS - 1)] += 1;
            self.count += 1;
            x += 2;
        }
        Ok(())
    }

    /// The noise standard deviation in levels once at least `min_values` values are in ([`LEE_MIN_VALUES`] is the usual number);
    /// `None` before.
    pub fn sigma(&self, min_values: u32) -> Option<f32> {
        if self.count == 0 || self.count < min_values {
            return None;
        }
        let half = self.count.div_ceil(2);
        let mut acc = 0u32;
        let med = self
            .hist
            .iter()
            .position(|&c| {
                acc += c;
                acc >= half
            })
            .unwrap_or(LEE_BINS - 1);
        Some((MAD_TO_SIGMA * med as f64 / 6.0) as f32)
    }
}

/// The eight neighbours of column `x`.
#[inline]
fn neighbours(above: &[u8], row: &[u8], below: &[u8], x: isize) -> [u8; 8] {
    [
        px(above, x - 1),
        px(above, x),
        px(above, x + 1),
        px(row, x - 1),
        px(row, x + 1),
        px(below, x - 1),
        px(below, x),
        px(below, x + 1),
    ]
}

/// Checks three rows and an output of one width.
fn check_rows(above: &[u8], row: &[u8], below: &[u8], out: &[u8]) -> Result<(), DspError> {
    let w = row.len();
    for len in [above.len(), below.len(), out.len()] {
        if len != w {
            return Param::Row.length(w, len);
        }
    }
    Ok(())
}

/// Click removal in a picture row: a pixel more than `k sigma` from the median of its eight neighbours (the mean of the 4th and
/// 5th of the sorted eight, rounded) is replaced by that median ([`PICTURE_CLICK_SIGMAS`] is the usual `k`). `above` and `below`
/// are the rows around `row` (pass `row` itself at the top or bottom border). A `sigma` of 0 or less copies the row. Returns the
/// pixels replaced. Errors when the widths differ.
pub fn picture_clicks(
    above: &[u8],
    row: &[u8],
    below: &[u8],
    sigma: f32,
    k: f32,
    out: &mut [u8],
) -> Result<usize, DspError> {
    check_rows(above, row, below, out)?;
    out.copy_from_slice(row);
    if sigma.is_nan() || sigma <= 0.0 {
        return Ok(0);
    }
    let lim = k * sigma;
    let mut count = 0;
    for x in 0..row.len() {
        let mut nb = neighbours(above, row, below, x as isize);
        nb.sort_unstable();
        let med = (nb[3] as f32 + nb[4] as f32) / 2.0;
        if (row[x] as f32 - med).abs() > lim {
            out[x] = med.round() as u8;
            count += 1;
        }
    }
    Ok(count)
}

/// The 3 x 3 local Wiener (Lee) filter of a picture row: over the nine pixels around each pixel, the mean `mu` and variance `v`
/// (exact integer sums); the gain `max(0, v - noise_var) / v` with `noise_var = factor sigma^2` ([`LEE_NOISE_FACTOR`] is the usual
/// factor); the output `mu + g (centre - mu)`, rounded and held to 0..=255. A `sigma` under [`LEE_MIN_SIGMA`] copies the row.
/// `above` and `below` as for [`picture_clicks`]. Errors when the widths differ.
pub fn picture_lee(
    above: &[u8],
    row: &[u8],
    below: &[u8],
    sigma: f32,
    factor: f32,
    out: &mut [u8],
) -> Result<(), DspError> {
    check_rows(above, row, below, out)?;
    if sigma.is_nan() || sigma < LEE_MIN_SIGMA {
        out.copy_from_slice(row);
        return Ok(());
    }
    let nv = (factor * sigma * sigma) as f64;
    for x in 0..row.len() {
        let c = row[x];
        let nb = neighbours(above, row, below, x as isize);
        let s: u32 = nb.iter().map(|&v| v as u32).sum::<u32>() + c as u32;
        let ss: u32 = nb.iter().map(|&v| v as u32 * v as u32).sum::<u32>() + c as u32 * c as u32;
        // 81 v = 9 ss - s^2, exact in integers.
        let v81 = (9 * ss as u64).saturating_sub(s as u64 * s as u64);
        let mu = s as f64 / 9.0;
        let v = v81 as f64 / 81.0;
        let g = if v <= 0.0 { 0.0 } else { (v - nv).max(0.0) / v };
        out[x] = (mu + g * (c as f64 - mu)).round().clamp(0.0, 255.0) as u8;
    }
    Ok(())
}

/// The picture filter of a whole colour plane: click removal from [`LEE_CLICK_SIGMA`] up, then [`picture_lee`] on the cleaned
/// rows. A row filtered as soon as the rows around it are final uses [`picture_clicks`] and [`picture_lee`] directly. Holds the
/// cleaned rows it needs.
#[derive(Clone, Debug, Default)]
pub struct PictureLee {
    clean: [Vec<u8>; 3],
}

impl PictureLee {
    /// A filter with no rows held.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters a whole plane of `width` levels a row (`plane.len()` a multiple of `width`) with noise `sigma` into `out` (the same
    /// length); the top and bottom rows repeat as their own neighbours. Errors for a width of 0 or lengths that do not fit.
    pub fn filter_plane(
        &mut self,
        plane: &[u8],
        width: usize,
        sigma: f32,
        out: &mut [u8],
    ) -> Result<(), DspError> {
        if width == 0 || !plane.len().is_multiple_of(width) {
            return Param::Row.length(width, plane.len());
        }
        if out.len() != plane.len() {
            return Param::Output.length(plane.len(), out.len());
        }
        let rows = plane.len() / width;
        if rows == 0 {
            return Ok(());
        }
        let row = |y: isize| {
            let y = y.clamp(0, rows as isize - 1) as usize;
            &plane[y * width..(y + 1) * width]
        };
        let clicks = sigma >= LEE_CLICK_SIGMA;
        for c in &mut self.clean {
            c.resize(width, 0);
        }
        // The cleaned rows y - 1, y, y + 1 in a ring of three.
        let clean_row = |c: &mut Vec<u8>, y: isize| -> Result<(), DspError> {
            if clicks {
                picture_clicks(
                    row(y - 1),
                    row(y),
                    row(y + 1),
                    sigma,
                    PICTURE_CLICK_SIGMAS,
                    c,
                )
                .map(|_| ())
            } else {
                c.copy_from_slice(row(y));
                Ok(())
            }
        };
        clean_row(&mut self.clean[0], 0)?;
        let [a, b, _] = &mut self.clean;
        b.clone_from(a);
        for y in 0..rows {
            let next = (y + 1).min(rows - 1) as isize;
            let [a, b, c] = &mut self.clean;
            clean_row(c, next)?;
            picture_lee(
                a,
                b,
                c,
                sigma,
                LEE_NOISE_FACTOR,
                &mut out[y * width..(y + 1) * width],
            )?;
            self.clean.rotate_left(1);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;

    /// A test card of 8-bit levels: flat areas, a ramp, and sharp edges.
    fn card(w: usize, h: usize) -> Vec<u8> {
        let mut p = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                p[y * w + x] = if y < h / 3 {
                    [40u8, 200, 120, 80][(4 * x / w).min(3)]
                } else if y < 2 * h / 3 {
                    (30 + 190 * x / w) as u8
                } else if (x / 16 + y / 16) % 2 == 0 {
                    60
                } else {
                    180
                };
            }
        }
        p
    }

    fn mean_abs(a: &[u8], b: &[u8]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(&x, &y)| (x as f64 - y as f64).abs())
            .sum::<f64>()
            / a.len() as f64
    }

    fn noisy(p: &[u8], sigma: f64, rng: &mut Rng) -> Vec<u8> {
        p.iter()
            .map(|&v| (v as f64 + sigma * rng.gauss()).round().clamp(0.0, 255.0) as u8)
            .collect()
    }

    #[test]
    fn lee_noise_reads_the_noise_and_the_filter_lowers_the_error() {
        let (w, h) = (320usize, 240usize);
        let clean = card(w, h);
        let mut rng = Rng(4242);
        let pic = noisy(&clean, 12.0, &mut rng);
        let mut m = LeeNoise::new();
        for y in 1..h - 1 {
            m.add_row(
                &pic[(y - 1) * w..y * w],
                &pic[y * w..(y + 1) * w],
                &pic[(y + 1) * w..(y + 2) * w],
            )
            .unwrap();
            if m.count() < LEE_MIN_VALUES {
                assert!(m.sigma(LEE_MIN_VALUES).is_none());
            }
        }
        let sigma = m.sigma(LEE_MIN_VALUES).unwrap();
        // The edges of the card add a little to the median.
        assert!((sigma - 12.0).abs() < 1.5, "{sigma}");
        let mut out = vec![0u8; pic.len()];
        PictureLee::new()
            .filter_plane(&pic, w, sigma, &mut out)
            .unwrap();
        let (before, after) = (mean_abs(&pic, &clean), mean_abs(&out, &clean));
        // Noise of 12 levels: the mean error falls by more than half.
        assert!(after < 0.5 * before, "{before} -> {after}");
    }

    #[test]
    fn picture_filters_leave_a_clean_picture_and_remove_clicks() {
        let (w, h) = (128usize, 96usize);
        let clean = card(w, h);
        let mut out = vec![0u8; clean.len()];
        PictureLee::new()
            .filter_plane(&clean, w, 1.0, &mut out)
            .unwrap();
        assert_eq!(out, clean);
        let mut m = LeeNoise::new();
        for y in 1..h - 1 {
            m.add_row(
                &clean[(y - 1) * w..y * w],
                &clean[y * w..(y + 1) * w],
                &clean[(y + 1) * w..(y + 2) * w],
            )
            .unwrap();
        }
        // The Laplacian is zero on flat areas and ramps: a clean card measures under the copy limit.
        assert!(m.sigma(LEE_MIN_VALUES).unwrap() < LEE_MIN_SIGMA);
        // Strong noise with clicks: click removal comes first.
        let mut rng = Rng(77);
        let mut pic = noisy(&clean, 30.0, &mut rng);
        for i in (0..pic.len()).step_by(97) {
            pic[i] = if clean[i] > 128 { 0 } else { 255 };
        }
        let mut row = vec![0u8; w];
        let y = h / 2;
        let n = picture_clicks(
            &pic[(y - 1) * w..y * w],
            &pic[y * w..(y + 1) * w],
            &pic[(y + 1) * w..(y + 2) * w],
            30.0,
            2.5,
            &mut row,
        )
        .unwrap();
        assert!(n >= 1);
        PictureLee::new()
            .filter_plane(&pic, w, 30.0, &mut out)
            .unwrap();
        let (before, after) = (mean_abs(&pic, &clean), mean_abs(&out, &clean));
        assert!(after < 0.5 * before, "{before} -> {after}");
    }

    #[test]
    fn line_wiener_and_line_clicks() {
        let n = 909;
        let clean: Vec<f32> = (0..n)
            .map(|i| {
                100.0 + 60.0 * (i as f32 / 70.0).sin() + if i % 300 < 150 { 0.0 } else { 40.0 }
            })
            .collect();
        let mut rng = Rng(5);
        let sd = 8.0f32;
        let mut line: Vec<f32> = clean.iter().map(|&v| v + sd * rng.gauss() as f32).collect();
        let mse = |a: &[f32]| {
            a.iter()
                .zip(&clean)
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f32>()
                / n as f32
        };
        let mut out = vec![0f32; n];
        line_wiener(&line, sd * sd, LINE_WIENER_HALF, &mut out).unwrap();
        // The mean square error falls to under 0.3 of the noise's.
        let (before, after) = (mse(&line), mse(&out));
        assert!(after < 0.3 * before, "{before} -> {after}");
        // A clean line is unchanged.
        line_wiener(&clean, 0.0, LINE_WIENER_HALF, &mut out).unwrap();
        assert_eq!(out, clean);
        // Clicks: a robust sigma from a flat stretch, then samples beyond 4 sigma of the median of 5 go.
        let flat: Vec<f32> = (0..31).map(|_| 50.0 + sd * rng.gauss() as f32).collect();
        let mut scratch = Vec::new();
        let s = robust_sigma(&flat, &mut scratch).unwrap();
        assert!((s - sd).abs() < 3.5, "{s}");
        for i in (10..n).step_by(50) {
            line[i] += 300.0;
        }
        let k = line_clicks(&line, s, LINE_CLICK_SIGMAS, &mut out).unwrap();
        // 18 clicks, and a few noise samples beyond 4 robust sigmas of a median of 5 on a curved line.
        assert!((18..=30).contains(&k), "{k}");
        assert!(out.iter().zip(&clean).all(|(a, b)| (a - b).abs() < 100.0));
        assert_eq!(line_clicks(&clean, 0.0, 4.0, &mut out).unwrap(), 0);
        assert_eq!(out, clean);
        assert!(line_wiener(&line, 1.0, 3, &mut out[..3]).is_err());
    }
}
