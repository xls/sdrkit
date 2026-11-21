//! The test card: Y, U and V planes at the picture size with EBU bars, a grey ramp, a staircase, a multiburst, the ident text, a
//! resolution wedge, a checkerboard, a circle with the system name, a moving box and a clock. The text uses the 5 x 7 font of
//! `sdrkit-codec-image`.

use sdrkit_codec_image::colour::{rgb_from_differences, yuv_from_rgb, BT601, KU, KV};
use sdrkit_codec_image::font::FIXED_5X7;

sdrkit_core::choice_enum! {
    /// What the picture shows.
    pub enum Pattern {
        /// The full test card.
        Card = ("card", "Test card"),
        /// EBU colour bars over the whole picture.
        Bars = ("bars", "Colour bars"),
        /// A grey ramp over the whole picture.
        Ramp = ("ramp", "Grey ramp"),
    }
}

/// The EBU 100/0/75/0 bars, R'G'B', white first.
pub const EBU_BARS: [[f64; 3]; 8] = [
    [1.0, 1.0, 1.0],
    [0.75, 0.75, 0.0],
    [0.0, 0.75, 0.75],
    [0.0, 0.75, 0.0],
    [0.75, 0.0, 0.75],
    [0.75, 0.0, 0.0],
    [0.0, 0.0, 0.75],
    [0.0, 0.0, 0.0],
];

/// The multiburst area (fractions of width and height): x0, x1, y0, y1.
pub const MULTIBURST: [f64; 4] = [0.53, 0.94, 0.33, 0.47];

/// Y, U, V planes of a picture.
#[derive(Debug, PartialEq)]
pub struct Planes {
    /// Width.
    pub w: usize,
    /// Height.
    pub h: usize,
    /// Luminance, 0..1.
    pub y: Vec<f32>,
    /// E'U.
    pub u: Vec<f32>,
    /// E'V.
    pub v: Vec<f32>,
}

impl Clone for Planes {
    fn clone(&self) -> Self {
        Planes {
            w: self.w,
            h: self.h,
            y: self.y.clone(),
            u: self.u.clone(),
            v: self.v.clone(),
        }
    }
    /// Copies `source` into the buffers of `self`, so that planes of the same size allocate nothing.
    fn clone_from(&mut self, source: &Self) {
        self.w = source.w;
        self.h = source.h;
        self.y.clone_from(&source.y);
        self.u.clone_from(&source.u);
        self.v.clone_from(&source.v);
    }
}

impl Planes {
    /// Mid grey.
    pub fn new(w: usize, h: usize, grey: f32) -> Self {
        Planes {
            w,
            h,
            y: vec![grey; w * h],
            u: vec![0.0; w * h],
            v: vec![0.0; w * h],
        }
    }
    /// Fills a rectangle with R'G'B'.
    pub fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, rgb: [f64; 3]) {
        let [y, u, v] = yuv_from_rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);
        for r in y0.min(self.h)..y1.min(self.h) {
            for c in x0.min(self.w)..x1.min(self.w) {
                let i = r * self.w + c;
                self.y[i] = y;
                self.u[i] = u;
                self.v[i] = v;
            }
        }
    }
    /// Sets one grey pixel.
    #[inline]
    pub fn grey(&mut self, c: usize, r: usize, g: f32) {
        if c < self.w && r < self.h {
            let i = r * self.w + c;
            self.y[i] = g;
            self.u[i] = 0.0;
            self.v[i] = 0.0;
        }
    }
    /// R'G'B' of a pixel.
    pub fn rgb(&self, c: usize, r: usize) -> [f64; 3] {
        let i = r * self.w + c;
        let [r, g, b] = rgb_from_differences(BT601, self.y[i], self.u[i] / KU, self.v[i] / KV);
        [r as f64, g as f64, b as f64]
    }
}

/// Draws `text` in the 5 x 7 font of `sdrkit-codec-image`, centred on (`cx`, `cy`), with pixel scale `s`, in grey `g` on a box of grey
/// `bg`.
pub fn draw_text(
    p: &mut Planes,
    text: &str,
    cx: usize,
    cy: usize,
    s: usize,
    g: f32,
    bg: Option<f32>,
) {
    let n = text.chars().count();
    let tw = n * 6 * s;
    let th = 9 * s;
    let x0 = cx.saturating_sub(tw / 2);
    let y0 = cy.saturating_sub(th / 2);
    if let Some(b) = bg {
        for r in y0..y0 + th {
            for c in x0.saturating_sub(s)..x0 + tw + s {
                p.grey(c, r, b);
            }
        }
    }
    for (k, ch) in text.chars().enumerate() {
        let gl = FIXED_5X7.glyph_or_missing(ch.to_ascii_uppercase());
        for row in 0..7usize {
            for col in 0..5usize {
                if gl.pixel(col as u8, row as u8) {
                    for dy in 0..s {
                        for dx in 0..s {
                            p.grey(x0 + (k * 6 + col) * s + dx, y0 + (row + 1) * s + dy, g);
                        }
                    }
                }
            }
        }
    }
}

/// The test card painter: the static parts drawn once, the moving box and the clock drawn per field.
#[derive(Clone, Debug)]
pub struct Card {
    base: Planes,
    /// The current picture.
    pub planes: Planes,
    pattern: Pattern,
    box_px: usize,
    panel: [usize; 4],
}

fn frac(x: f64, n: usize) -> usize {
    (x * n as f64).round() as usize
}

impl Card {
    /// A card of `w` x `h` with the ident and the system name in the circle.
    pub fn new(
        w: usize,
        h: usize,
        pattern: Pattern,
        ident: &str,
        system: &str,
        raster: &str,
    ) -> Self {
        let mut p = Planes::new(w, h, 0.5);
        let bars = |p: &mut Planes, x0: usize, x1: usize, y0: usize, y1: usize| {
            for (k, c) in EBU_BARS.iter().enumerate() {
                let a = x0 + (x1 - x0) * k / 8;
                let b = x0 + (x1 - x0) * (k + 1) / 8;
                p.rect(a, y0, b, y1, *c);
            }
        };
        match pattern {
            Pattern::Bars => bars(&mut p, 0, w, 0, h),
            Pattern::Ramp => {
                for r in 0..h {
                    for c in 0..w {
                        p.grey(c, r, c as f32 / (w - 1) as f32);
                    }
                }
            }
            Pattern::Card => {
                // Border.
                for r in 0..h {
                    for c in 0..w {
                        if r < 3 || r >= h - 3 || c < 3 || c >= w - 3 {
                            p.grey(c, r, 1.0);
                        }
                    }
                }
                bars(
                    &mut p,
                    frac(0.06, w),
                    frac(0.94, w),
                    frac(0.07, h),
                    frac(0.30, h),
                );
                let (rx0, rx1) = (frac(0.06, w), frac(0.50, w));
                for r in frac(0.33, h)..frac(0.40, h) {
                    for c in rx0..rx1 {
                        p.grey(c, r, (c - rx0) as f32 / (rx1 - rx0 - 1) as f32);
                    }
                }
                for r in frac(0.40, h)..frac(0.47, h) {
                    for c in rx0..rx1 {
                        let step = ((c - rx0) * 11 / (rx1 - rx0)).min(10);
                        p.grey(c, r, step as f32 / 10.0);
                    }
                }
                // Multiburst area: grey, the generator draws the packets.
                for r in frac(MULTIBURST[2], h)..frac(MULTIBURST[3], h) {
                    for c in frac(MULTIBURST[0], w)..frac(MULTIBURST[1], w) {
                        p.grey(c, r, 0.5);
                    }
                }
                draw_text(
                    &mut p,
                    ident,
                    w / 2,
                    frac(0.50, h),
                    (h / 160).max(1),
                    1.0,
                    Some(0.0),
                );
                // Resolution wedge.
                let (wx0, wx1) = (frac(0.06, w), frac(0.30, w));
                let mut phase = 0.0f64;
                for c in wx0..wx1 {
                    let t = (c - wx0) as f64 / (wx1 - wx0) as f64;
                    let period = 12.0 + (2.2 - 12.0) * t;
                    phase += std::f64::consts::TAU / period;
                    for r in frac(0.60, h)..frac(0.75, h) {
                        p.grey(c, r, (0.5 + 0.4 * phase.sin()) as f32);
                    }
                }
                for r in frac(0.77, h)..frac(0.92, h) {
                    for c in wx0..wx1 {
                        let on = ((c - wx0) / 4 + (r / 4)) % 2 == 0;
                        p.grey(c, r, if on { 0.9 } else { 0.1 });
                    }
                }
                // Circle with a cross.
                let (cx, cy, rad) = (w as f64 * 0.5, h as f64 * 0.76, h as f64 * 0.16);
                for r in 0..h {
                    for c in 0..w {
                        let (dx, dy) = (c as f64 - cx, r as f64 - cy);
                        let d = (dx * dx + dy * dy).sqrt();
                        if (d - rad).abs() < 1.5 || (d < rad && (dx.abs() < 1.0 || dy.abs() < 1.0))
                        {
                            p.grey(c, r, 1.0);
                        }
                    }
                }
                let s = (h / 200).max(1);
                draw_text(
                    &mut p,
                    system,
                    cx as usize,
                    (cy - rad * 0.4) as usize,
                    s,
                    1.0,
                    Some(0.1),
                );
                draw_text(
                    &mut p,
                    raster,
                    cx as usize,
                    (cy + rad * 0.4) as usize,
                    s,
                    1.0,
                    Some(0.1),
                );
                // The panel of the moving box.
                let panel = [frac(0.70, w), frac(0.94, w), frac(0.60, h), frac(0.92, h)];
                for r in panel[2]..panel[3] {
                    for c in panel[0]..panel[1] {
                        let edge = r < panel[2] + 2
                            || r >= panel[3] - 2
                            || c < panel[0] + 2
                            || c >= panel[1] - 2;
                        p.grey(c, r, if edge { 1.0 } else { 0.25 });
                    }
                }
                for r in frac(0.935, h)..frac(0.985, h) {
                    for c in 3..w - 3 {
                        p.grey(c, r, 0.2);
                    }
                }
            }
        }
        Card {
            planes: p.clone(),
            base: p,
            pattern,
            box_px: frac(0.11, h).max(4),
            panel: [frac(0.70, w), frac(0.94, w), frac(0.60, h), frac(0.92, h)],
        }
    }

    /// Draws the moving parts for picture time `t` seconds and frame number `frame`.
    pub fn update(&mut self, t: f64, frame: u64) {
        if self.pattern != Pattern::Card {
            return;
        }
        let (w, h) = (self.planes.w, self.planes.h);
        // Restore the dynamic regions from the base.
        let restore =
            |pl: &mut Planes, base: &Planes, r0: usize, r1: usize, c0: usize, c1: usize| {
                for r in r0..r1.min(h) {
                    let a = r * w + c0;
                    let b = r * w + c1.min(w);
                    pl.y[a..b].copy_from_slice(&base.y[a..b]);
                    pl.u[a..b].copy_from_slice(&base.u[a..b]);
                    pl.v[a..b].copy_from_slice(&base.v[a..b]);
                }
            };
        let [px0, px1, py0, py1] = self.panel;
        restore(
            &mut self.planes,
            &self.base,
            py0 + 2,
            py1 - 2,
            px0 + 2,
            px1 - 2,
        );
        let (cr0, cr1) = (frac(0.935, h), frac(0.985, h));
        restore(&mut self.planes, &self.base, cr0, cr1, 3, w - 3);
        let b = self.box_px;
        let span_x = (px1 - px0 - 4 - b) as f64;
        let span_y = (py1 - py0 - 4 - b) as f64;
        let bounce = |pos: f64, span: f64| {
            let p = pos % (2.0 * span);
            if p < span {
                p
            } else {
                2.0 * span - p
            }
        };
        let bx = px0 + 2 + bounce(150.0 * t, span_x) as usize;
        let by = py0 + 2 + bounce(110.0 * t, span_y) as usize;
        self.planes.rect(bx, by, bx + b, by + b, [0.75, 0.75, 0.0]);
        let secs = t.max(0.0);
        let text = format!(
            "{:02}:{:02}:{:02}.{:02}  {}",
            (secs / 3600.0) as u64 % 24,
            (secs / 60.0) as u64 % 60,
            secs as u64 % 60,
            ((secs.fract()) * 100.0) as u64,
            frame
        );
        let s = ((cr1 - cr0) / 9).max(1);
        draw_text(
            &mut self.planes,
            &text,
            w / 2,
            (cr0 + cr1) / 2,
            s,
            0.9,
            None,
        );
    }

    /// Whether the card changes from picture to picture.
    pub fn moves(&self) -> bool {
        self.pattern == Pattern::Card
    }

    /// The rows the moving parts can touch.
    pub fn dynamic_rows(&self) -> std::ops::Range<usize> {
        if self.pattern == Pattern::Card {
            self.panel[2]..frac(0.985, self.planes.h)
        } else {
            0..0
        }
    }
}

/// The NBTV test picture: `pattern` 0 is bars across the lines in the top part and a ramp along them in the bottom
/// part, 1 bars only, 2 the ramp only. `lines` x `samples` cells, value at line `l` and cell `s` (0 at the start of a line).
pub fn nbtv_cell(pattern: u32, lines: usize, samples: usize, l: usize, s: usize) -> f32 {
    const LUMA: [f32; 8] = [1.0, 0.6651, 0.5305, 0.4405, 0.3095, 0.2195, 0.0849, 0.0];
    let bar = LUMA[(l * 8 / lines.max(1)).min(7)];
    let ramp = s as f32 / (samples.max(2) - 1) as f32;
    match pattern {
        1 => bar,
        2 => ramp,
        _ => {
            if s < samples / 2 {
                bar
            } else {
                ramp
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bars_sit_where_the_vectors_measure_them() {
        let c = Card::new(768, 576, Pattern::Card, "TEST CARD", "PAL B/G", "625/50");
        let p = &c.planes;
        // Middle of the white bar and of the red bar.
        let white = p.rgb(46 + 42, 100);
        assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-3), "{white:?}");
        let red = p.rgb(46 + 5 * 84 + 42, 100);
        assert!((red[0] - 0.75).abs() < 1e-3 && red[1].abs() < 1e-3 && red[2].abs() < 1e-3);
        assert_eq!(p.y[39 * 768 + 100], 0.5);
    }

    #[test]
    fn the_box_moves() {
        let mut c = Card::new(768, 576, Pattern::Card, "TEST CARD", "PAL", "625/50");
        c.update(0.0, 0);
        let a = c.planes.clone();
        c.update(0.2, 5);
        assert_ne!(a.y, c.planes.y);
        assert!(c.dynamic_rows().start > 300);
    }
}
