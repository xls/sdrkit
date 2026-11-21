//! The decimating FIR of the AD9361 for low rates: a 16-tap filter that decimates by 2 and a 32-tap filter that decimates by
//! 4, symmetric low-passes designed by weighted least squares.

/// Number of taps of the decimate-by-2 filter.
pub const TAPS: usize = 16;
/// Number of taps of the decimate-by-4 filter.
pub const TAPS_DEC4: usize = 32;

/// The decimate-by-2 taps in units of 1/32768, symmetric, summing to about 32768 (unity gain at DC).
pub fn taps() -> [i32; TAPS] {
    let v = design::<{ TAPS / 2 }>(0.18, 0.30, 3.0);
    std::array::from_fn(|i| v[i])
}

/// The decimate-by-4 taps, as [`taps`].
pub fn taps_dec4() -> [i32; TAPS_DEC4] {
    let v = design::<{ TAPS_DEC4 / 2 }>(0.097, 0.149, 8.0);
    std::array::from_fn(|i| v[i])
}

/// The taps for a decimation of 2 or 4.
pub fn taps_for(dec: u32) -> Vec<i32> {
    if dec == 4 {
        taps_dec4().to_vec()
    } else {
        taps().to_vec()
    }
}

/// A symmetric low-pass of 2 x `M` taps: passband to `pass`, stopband from `stop` (cycles of the converter rate), stopband weight
/// `weight`. H(f) = 2 sum_k a_k cos(2 pi f (k + 1/2)): the weighted normal equations are solved for a.
fn design<const M: usize>(pass: f64, stop: f64, weight: f64) -> Vec<i32> {
    let mut ata = [[0.0f64; M]; M];
    let mut atb = [0.0f64; M];
    let mut add = |f: f64, want: f64, w: f64| {
        let row: [f64; M] =
            std::array::from_fn(|k| 2.0 * (std::f64::consts::TAU * f * (k as f64 + 0.5)).cos());
        for i in 0..M {
            for j in 0..M {
                ata[i][j] += w * w * row[i] * row[j];
            }
            atb[i] += w * w * row[i] * want;
        }
    };
    for i in 0..200 {
        add(pass * i as f64 / 199.0, 1.0, 1.0);
    }
    for i in 0..400 {
        add(stop + (0.5 - stop) * i as f64 / 399.0, 0.0, weight);
    }
    let a = solve(ata, atb);
    let sum: f64 = 2.0 * a.iter().sum::<f64>();
    (0..2 * M)
        .map(|i| {
            let k = if i < M { M - 1 - i } else { i - M };
            (a[k] / sum * 32768.0).round() as i32
        })
        .collect()
}

/// Gaussian elimination with partial pivoting.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> [f64; N] {
    for c in 0..N {
        let p = (c..N)
            .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
            .unwrap_or(c);
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..N {
            let f = a[r][c] / a[c][c];
            let pivot = a[c];
            for (x, p) in a[r][c..].iter_mut().zip(&pivot[c..]) {
                *x -= f * p;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; N];
    for r in (0..N).rev() {
        let s: f64 = (r + 1..N).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x
}

/// The filter in the AD9361 filter-file format: `RX 3 GAIN 0 DEC <dec>`, `TX 3 GAIN 0 INT <dec>`, then one line `rx,tx` per tap
/// (the same tap for RX and TX). `dec` is 2 or 4.
pub fn config_text(dec: u32) -> String {
    let dec = if dec == 4 { 4 } else { 2 };
    let mut s = format!("RX 3 GAIN 0 DEC {dec}\nTX 3 GAIN 0 INT {dec}\n");
    for t in taps_for(dec) {
        s.push_str(&format!("{t},{t}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain_db(t: &[i32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in t.iter().enumerate() {
            let p = std::f64::consts::TAU * f * i as f64;
            re += v as f64 * p.cos();
            im -= v as f64 * p.sin();
        }
        let dc: f64 = t.iter().map(|&v| v as f64).sum();
        20.0 * ((re * re + im * im).sqrt() / dc).log10()
    }

    #[test]
    fn the_filter_meets_its_numbers() {
        let t = taps();
        assert!(
            t.iter().zip(t.iter().rev()).all(|(a, b)| a == b),
            "not symmetric: {t:?}"
        );
        let sum: i32 = t.iter().sum();
        assert!((sum - 32768).abs() <= 8, "{sum}");
        // In cycles of the converter rate (twice the output rate).
        for i in 0..=190 {
            assert!(gain_db(&t, i as f64 / 1000.0) >= -1.0, "passband at {i}");
        }
        for i in 305..=500 {
            assert!(
                gain_db(&t, i as f64 / 1000.0) <= -42.0,
                "stopband at {i}: {}",
                gain_db(&t, i as f64 / 1000.0)
            );
        }
        let half = (0..500)
            .find(|&i| gain_db(&t, i as f64 / 1000.0) < -6.0)
            .unwrap();
        assert!((220..=250).contains(&half), "cutoff {half}");
        let text = config_text(2);
        assert!(text.starts_with("RX 3 GAIN 0 DEC 2\nTX 3 GAIN 0 INT 2\n"));
        assert_eq!(text.lines().count(), 18);
    }

    #[test]
    fn the_decimate_by_4_filter_meets_the_same_numbers() {
        let t = taps_dec4();
        assert!(t.iter().zip(t.iter().rev()).all(|(a, b)| a == b));
        let sum: i32 = t.iter().sum();
        assert!((sum - 32768).abs() <= 16, "{sum}");
        // In cycles of the converter rate (four times the output rate): -1 dB at 0.38 / 4, 41 dB down from 0.61 / 4.
        for i in 0..=950 {
            let f = i as f64 / 10_000.0;
            assert!(
                gain_db(&t, f) >= -1.0,
                "passband at {f}: {}",
                gain_db(&t, f)
            );
        }
        for i in 1525..=5000 {
            let f = i as f64 / 10_000.0;
            assert!(
                gain_db(&t, f) <= -41.0,
                "stopband at {f}: {}",
                gain_db(&t, f)
            );
        }
        let text = config_text(4);
        assert!(text.starts_with("RX 3 GAIN 0 DEC 4\nTX 3 GAIN 0 INT 4\n"));
        assert_eq!(text.lines().count(), 34);
    }
}
