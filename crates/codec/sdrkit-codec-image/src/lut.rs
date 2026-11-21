//! Look-up tables for grey levels, and colour palettes. The tables keep level 0 ("no data") at 0, so a gap stays black.

/// A table of 256 output levels, one for each input level.
///
/// ```
/// use sdrkit_codec_image::lut::Lut;
///
/// let samples: Vec<u8> = (0..10_000).map(|i| (50 + i % 100) as u8).collect();
/// let lut = Lut::stretch(&samples, None);
/// assert_eq!(lut.get(0), 0);       // no data stays no data
/// assert!(lut.get(52) <= 2);       // the 2 % point goes to the bottom
/// assert!(lut.get(149) >= 254);    // the 98 % point goes to the top
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lut(pub [u8; 256]);

impl Default for Lut {
    fn default() -> Self {
        Lut::identity()
    }
}

impl Lut {
    /// The table that changes nothing.
    pub fn identity() -> Lut {
        Lut(std::array::from_fn(|i| i as u8))
    }

    /// The table that inverts the levels 1 to 255 (level `v` becomes `256 - v`) and keeps 0 at 0 (a thermal channel shown with
    /// cold clouds white). A valid level never becomes 0, so it never turns into "no data".
    pub fn invert() -> Lut {
        Lut(std::array::from_fn(|i| {
            if i == 0 {
                0
            } else {
                (256 - i) as u8
            }
        }))
    }

    /// A gamma table: level `v` becomes `255 (v / 255)^gamma`, rounded. If `gamma` is not finite or not above 0, the table is
    /// the identity.
    pub fn gamma(gamma: f64) -> Lut {
        if !gamma.is_finite() || gamma <= 0.0 {
            return Lut::identity();
        }
        Lut(std::array::from_fn(|i| {
            (255.0 * (i as f64 / 255.0).powf(gamma)).round() as u8
        }))
    }

    /// A contrast stretch of the non-zero `samples`: the 2 % point goes to 1 and the 98 % point to 255. Level 0 stays 0. If
    /// fewer than 2000 samples are not 0, the table is the identity. If the two points are less than 8 apart, each moves 4
    /// levels out. A `gamma` applies after the stretch.
    pub fn stretch(samples: &[u8], gamma: Option<f64>) -> Lut {
        let mut hist = [0u64; 256];
        for &s in samples {
            hist[s as usize] += 1;
        }
        Lut::stretch_histogram(&hist, gamma)
    }

    /// The same as [`Lut::stretch`], from a histogram of 256 counts.
    pub fn stretch_histogram(hist: &[u64; 256], gamma: Option<f64>) -> Lut {
        let n: u64 = hist[1..].iter().sum();
        if n < 2000 {
            return Lut::identity();
        }
        let pick = |frac: f64| -> usize {
            let target = (frac * n as f64) as u64;
            let mut acc = 0;
            for (i, &h) in hist.iter().enumerate().skip(1) {
                acc += h;
                if acc > target {
                    return i;
                }
            }
            255
        };
        let (mut lo, mut hi) = (pick(0.02) as f64, pick(0.98) as f64);
        if hi - lo < 8.0 {
            lo = (lo - 4.0).max(1.0);
            hi = (hi + 4.0).min(255.0);
        }
        let gamma = gamma.filter(|g| g.is_finite() && *g > 0.0);
        let mut lut = Lut::identity();
        for (i, v) in lut.0.iter_mut().enumerate().skip(1) {
            let mut t = ((i as f64 - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0);
            if let Some(g) = gamma {
                t = t.powf(g);
            }
            *v = (1.0 + t * 254.0).round() as u8;
        }
        lut
    }

    /// The output level of `v`.
    #[inline]
    pub fn get(&self, v: u8) -> u8 {
        self.0[v as usize]
    }

    /// This table after `first`: the result maps `v` to `self(first(v))`.
    pub fn after(&self, first: &Lut) -> Lut {
        Lut(std::array::from_fn(|i| self.0[first.0[i] as usize]))
    }

    /// Applies the table to every byte of `data` in place. It allocates nothing.
    pub fn apply(&self, data: &mut [u8]) {
        for v in data {
            *v = self.0[*v as usize];
        }
    }

    /// Writes the table's output for each byte of `src` into `dst`. If the two differ in length, the shorter one sets the count.
    pub fn apply_to(&self, src: &[u8], dst: &mut [u8]) {
        for (d, s) in dst.iter_mut().zip(src) {
            *d = self.0[*s as usize];
        }
    }
}

/// A colour for each of the 256 grey levels, as red, green and blue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette(pub [[u8; 3]; 256]);

impl Palette {
    /// The grey palette (red, green and blue all equal the level).
    pub fn grey() -> Palette {
        Palette(std::array::from_fn(|i| [i as u8; 3]))
    }

    /// The thermal palette for an infrared channel. The level is inverted first (cold is bright in infrared). Then the colour
    /// runs through white, red, yellow, green, cyan and dark blue, at equal steps. Level 0 (no data) stays black.
    pub fn thermal() -> Palette {
        const STOPS: [[f64; 3]; 6] = [
            [255.0, 255.0, 255.0],
            [200.0, 40.0, 40.0],
            [240.0, 220.0, 40.0],
            [60.0, 200.0, 80.0],
            [40.0, 200.0, 220.0],
            [20.0, 40.0, 140.0],
        ];
        Palette(std::array::from_fn(|g| {
            if g == 0 {
                return [0, 0, 0];
            }
            let t = 1.0 - g as f64 / 255.0;
            let pos = (t / 0.2).min(4.999_999);
            let k = pos.floor() as usize;
            let f = pos - k as f64;
            let (a, b) = (STOPS[k], STOPS[k + 1]);
            std::array::from_fn(|c| (a[c] + (b[c] - a[c]) * f).round() as u8)
        }))
    }

    /// The colour of level `v`.
    #[inline]
    pub fn get(&self, v: u8) -> [u8; 3] {
        self.0[v as usize]
    }

    /// Writes the BGRA colour (alpha 255) of each grey byte of `src` into `bgra`, 4 bytes for each pixel. If the two differ in
    /// pixel count, the shorter one sets the count. It allocates nothing.
    pub fn to_bgra(&self, src: &[u8], bgra: &mut [u8]) {
        for (d, &s) in bgra.as_chunks_mut::<4>().0.iter_mut().zip(src) {
            let [r, g, b] = self.0[s as usize];
            d.copy_from_slice(&[b, g, r, 255]);
        }
    }
}
