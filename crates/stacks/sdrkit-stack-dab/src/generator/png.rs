//! The test signal's slide, drawn as a raster and written by the shared PNG encoder.

use sdrkit_codec_image::{png, Raster};

/// The generator's slide: 80 × 60, a white 1-pixel border, eight vertical bars (white, yellow, cyan, green, magenta, red, blue,
/// dark grey) darkening from top to bottom in 30 steps, and the two diagonals in black. A PNG file.
pub fn test_slide() -> Vec<u8> {
    let (w, h) = (80usize, 60usize);
    let bars: [[u8; 3]; 8] = [
        [255, 255, 255],
        [255, 255, 0],
        [0, 255, 255],
        [0, 255, 0],
        [255, 0, 255],
        [255, 0, 0],
        [0, 0, 255],
        [96, 96, 96],
    ];
    let mut px = Vec::with_capacity(3 * w * h);
    for y in 0..h {
        for x in 0..w {
            let border = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let (xi, yi, wi, hi) = (x as i32, y as i32, w as i32, h as i32);
            let diag = (xi * (hi - 1) - yi * (wi - 1)).abs() < 40
                || (xi * (hi - 1) - (hi - 1 - yi) * (wi - 1)).abs() < 40;
            let c = if border {
                [255; 3]
            } else if diag {
                [0; 3]
            } else {
                let bar = bars[(x * 8 / w).min(7)];
                let f = 1.0 - (y * 30 / h).min(29) as f32 / 32.0;
                bar.map(|v| (v as f32 * f) as u8)
            };
            px.extend_from_slice(&c);
        }
    }
    Raster::from_rgb(w as u32, h as u32, &px)
        .and_then(|r| png::encode(&r))
        .unwrap_or_default()
}
