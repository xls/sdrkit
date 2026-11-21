//! The colour products of the `eodb` instruments, made on request from the 16-bit channel pictures: composites of three
//! stretched channels, uncalibrated difference products, and for the MetOp AVHRR the products of the brightness temperature.

use super::avhrr_dump::{temperature, LineCal, SatCal, SCALE};
use super::planes::{Plane, Planes};

/// One colour of a difference product: a channel, or the difference of two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    /// A channel.
    One(&'static str),
    /// The first channel minus the second.
    Minus(&'static str, &'static str),
}

/// What a product is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// Three stretched channels as red, green and blue.
    Composite([&'static str; 3]),
    /// Three terms, each stretched on its own: uncalibrated, relative contrast only.
    Difference([Term; 3]),
    /// AVHRR channel 4 temperature on a blue-to-red scale from 190 K to 320 K.
    CloudTop,
    /// The split-window sea surface temperature, above 240 K, on the scale over 270 K to 305 K.
    SeaSurface,
    /// Hot spots of the lines with channel 3B.
    Fire,
}

/// A product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductDef {
    /// The key of the `product` command.
    pub key: &'static str,
    /// The name.
    pub name: &'static str,
    /// The recipe.
    pub recipe: Recipe,
}

const fn def(key: &'static str, name: &'static str, recipe: Recipe) -> ProductDef {
    ProductDef { key, name, recipe }
}

/// The VIIRS products.
pub const VIIRS: [ProductDef; 4] = [
    def("natural", "Natural colour", Recipe::Composite(["M10", "M7", "M5"])),
    def("true", "True colour", Recipe::Composite(["M5", "M4", "M3"])),
    def(
        "dust",
        "Dust (uncalibrated, relative contrast only)",
        Recipe::Difference([Term::Minus("M16", "M15"), Term::Minus("M15", "M14"), Term::One("M15")]),
    ),
    def(
        "fog",
        "Fog (uncalibrated, relative contrast only)",
        Recipe::Difference([Term::Minus("M16", "M15"), Term::Minus("M15", "M12"), Term::One("M15")]),
    ),
];

/// The MODIS products.
pub const MODIS: [ProductDef; 5] = [
    def("natural", "Natural colour", Recipe::Composite(["7", "2", "1"])),
    def("true", "True colour", Recipe::Composite(["1", "4", "3"])),
    def(
        "airmass",
        "Airmass (uncalibrated, relative contrast only)",
        Recipe::Difference([Term::Minus("27", "28"), Term::Minus("30", "31"), Term::One("27")]),
    ),
    def(
        "dust",
        "Dust (uncalibrated, relative contrast only)",
        Recipe::Difference([Term::Minus("32", "31"), Term::Minus("31", "29"), Term::One("31")]),
    ),
    def(
        "fog",
        "Fog (uncalibrated, relative contrast only)",
        Recipe::Difference([Term::Minus("32", "31"), Term::Minus("31", "22"), Term::One("31")]),
    ),
];

/// The AVHRR products of the MetOp dump.
pub const AVHRR: [ProductDef; 4] = [
    def("natural", "Natural colour", Recipe::Composite(["2", "1", "4"])),
    def("cloud_top", "Cloud-top temperature", Recipe::CloudTop),
    def("sst", "Sea surface temperature (split window, clear sky, no land mask)", Recipe::SeaSurface),
    def("fire", "Fire", Recipe::Fire),
];

/// An RGB picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgb {
    /// Columns.
    pub width: usize,
    /// Rows.
    pub height: usize,
    /// Red, green, blue for each pixel, row by row.
    pub data: Vec<u8>,
}

/// The 1 % and 99 % values of `v`.
fn percentiles(mut v: Vec<f64>) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 1.0);
    }
    v.sort_by(f64::total_cmp);
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    let (lo, hi) = (at(0.01), at(0.99));
    (lo, if hi > lo { hi } else { lo + 1.0 })
}

/// The output size: the size of the first plane, cut to `max_width` columns (0 for no limit) with the same aspect.
fn size(first: &Plane, max_width: usize) -> (usize, usize) {
    let (w, h) = (first.width, first.rows);
    if max_width == 0 || w <= max_width {
        (w, h)
    } else {
        (max_width, (h * max_width / w).max(1))
    }
}

/// The value of plane `p` at the output pixel (x, y) of an output of `w` x `h`, nearest neighbour.
fn at(p: &Plane, x: usize, y: usize, w: usize, h: usize) -> f64 {
    f64::from(p.get(x * p.width / w.max(1), y * p.rows.max(1) / h.max(1)))
}

fn scale_rgb(t: f64) -> [f64; 3] {
    [
        (1.5 - (4.0 * t - 3.0).abs()).clamp(0.0, 1.0),
        (1.5 - (4.0 * t - 2.0).abs()).clamp(0.0, 1.0),
        (1.5 - (4.0 * t - 1.0).abs()).clamp(0.0, 1.0),
    ]
}

/// The calibration of the AVHRR lines that a temperature product needs.
pub struct Thermal<'a> {
    /// The calibration views of each line.
    pub lines: &'a [Option<LineCal>],
    /// The coefficients of the satellite.
    pub cal: &'a SatCal,
}

/// Makes product `d` from `planes`, at most `max_width` columns wide (0 for full size). `None` when a channel is missing or
/// has no rows, or a temperature product has no calibration.
pub fn render(d: &ProductDef, planes: &Planes, thermal: Option<&Thermal<'_>>, max_width: usize) -> Option<Rgb> {
    let plane = |n: &str| planes.index(n).map(|i| &planes.planes[i]).filter(|p| p.rows > 0);
    let mut px: Vec<[f64; 3]>;
    let (w, h);
    match d.recipe {
        Recipe::Composite(names) => {
            let ps: Vec<&Plane> = names.iter().map(|n| plane(n)).collect::<Option<_>>()?;
            (w, h) = size(ps[0], max_width);
            let stretch: Vec<(f64, f64)> = ps
                .iter()
                .map(|p| percentiles(p.data.iter().step_by(13).filter(|&&v| v > 0).map(|&v| f64::from(v)).collect()))
                .collect();
            px = (0..w * h)
                .map(|k| {
                    let (x, y) = (k % w, k / w);
                    let mut c = [0.0; 3];
                    for (i, p) in ps.iter().enumerate() {
                        let (lo, hi) = stretch[i];
                        c[i] = ((at(p, x, y, w, h) - lo) / (hi - lo)).clamp(0.0, 1.0).powf(0.8);
                    }
                    c
                })
                .collect();
        }
        Recipe::Difference(terms) => {
            let first = match terms[0] {
                Term::One(a) | Term::Minus(a, _) => plane(a)?,
            };
            (w, h) = size(first, max_width);
            let mut vals: Vec<Vec<f64>> = Vec::with_capacity(3);
            for t in terms {
                let v: Vec<f64> = match t {
                    Term::One(a) => {
                        let a = plane(a)?;
                        (0..w * h).map(|k| at(a, k % w, k / w, w, h)).collect()
                    }
                    Term::Minus(a, b) => {
                        let (a, b) = (plane(a)?, plane(b)?);
                        (0..w * h).map(|k| at(a, k % w, k / w, w, h) - at(b, k % w, k / w, w, h)).collect()
                    }
                };
                vals.push(v);
            }
            let valid = |k: usize| vals[0][k] != 0.0 || vals[1][k] != 0.0;
            let stretch: Vec<(f64, f64)> = vals
                .iter()
                .map(|v| {
                    percentiles(
                        (0..h)
                            .step_by(4)
                            .flat_map(|y| (0..w).step_by(4).map(move |x| y * w + x))
                            .filter(|&k| valid(k))
                            .map(|k| v[k])
                            .collect(),
                    )
                })
                .collect();
            px = (0..w * h)
                .map(|k| {
                    let mut c = [0.0; 3];
                    for i in 0..3 {
                        let (lo, hi) = stretch[i];
                        c[i] = ((vals[i][k] - lo) / (hi - lo)).clamp(0.0, 1.0);
                    }
                    c
                })
                .collect();
        }
        Recipe::CloudTop | Recipe::SeaSurface | Recipe::Fire => {
            let th = thermal?;
            let (c3, c4, c5) = (plane("3")?, plane("4")?, plane("5")?);
            (w, h) = size(c4, max_width);
            let temp = |p: &Plane, k: usize, x: usize, y: usize| -> Option<f64> {
                let row = y * c4.rows / h.max(1);
                let l = th.lines.get(row)?.as_ref()?;
                temperature(th.cal, l, k, at(p, x, y, w, h) / f64::from(SCALE))
            };
            px = vec![[0.0; 3]; w * h];
            for y in 0..h {
                for x in 0..w {
                    let k = y * w + x;
                    let t4 = temp(c4, 1, x, y);
                    px[k] = match d.recipe {
                        Recipe::CloudTop => t4.map_or([0.0; 3], |t| scale_rgb((t - 190.0) / 130.0)),
                        Recipe::SeaSurface => match (t4, temp(c5, 2, x, y)) {
                            (Some(t4), Some(t5)) => {
                                let t = 1.0346 * t4 + 2.5779 * (t4 - t5) - 283.21 + 273.15;
                                if t > 240.0 {
                                    scale_rgb((t - 270.0) / 35.0)
                                } else {
                                    [0.0; 3]
                                }
                            }
                            _ => [0.0; 3],
                        },
                        _ => {
                            let row = y * c4.rows / h.max(1);
                            let is_3b = th.lines.get(row).and_then(|l| l.as_ref()).is_some_and(|l| l.is_3b);
                            match (is_3b, t4, temp(c3, 0, x, y)) {
                                (true, Some(t4), Some(t3)) if t3 > 310.0 && t3 - t4 > 10.0 => [1.0, 0.15, 0.0],
                                (true, Some(t4), _) => [0.0, (0.3 * (t4 - 190.0) / 130.0).clamp(0.0, 1.0), 0.0],
                                _ => [0.0; 3],
                            }
                        }
                    };
                }
            }
        }
    }
    let data = px
        .iter()
        .flat_map(|c| c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8))
        .collect();
    Some(Rgb {
        width: w,
        height: h,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_composite_stretches_each_channel() {
        let mut p = Planes::new(&[("M10", 1.6, "M", 8), ("M7", 0.8, "M", 8), ("M5", 0.6, "M", 8)], 10);
        for c in 0..3 {
            for y in 0..4 {
                for x in 0..8 {
                    p.set(c, x, y, (100 + 1000 * x + 10 * y) as u16);
                }
            }
        }
        let r = render(&VIIRS[0], &p, None, 4).unwrap();
        assert_eq!((r.width, r.height), (4, 2));
        assert_eq!(r.data.len(), 4 * 2 * 3);
        assert!(r.data[0] < r.data[9], "{:?}", r.data);
        assert!(render(&VIIRS[1], &p, None, 0).is_none());
    }
}
