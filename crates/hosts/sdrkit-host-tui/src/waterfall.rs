//! The history of spectrum frames and its drawing as a waterfall of half-block characters, newest line at the top.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use std::collections::VecDeque;

/// The number of columns that each frame is reduced to.
const COLUMNS: usize = 512;
/// The number of frames that the history keeps.
const LINES: usize = 128;

/// The last spectrum frames, each reduced to 512 values.
#[derive(Debug, Default)]
pub struct Waterfall {
    lines: VecDeque<Vec<f32>>,
    /// The value drawn darkest.
    lo: f32,
    /// The value drawn brightest.
    hi: f32,
    /// The width of the last frame in Hz.
    pub span_hz: f64,
    /// The frequency at the middle of the last frame in Hz.
    pub centre_hz: f64,
}

impl Waterfall {
    /// Adds the frame `bins` (lowest frequency first). The oldest line's buffer holds the new line once the history is
    /// full.
    pub fn push(&mut self, bins: &[f32], span_hz: f64, centre_hz: f64) {
        if bins.is_empty() {
            return;
        }
        let mut line = if self.lines.len() >= LINES {
            self.lines.pop_back().unwrap_or_default()
        } else {
            Vec::with_capacity(COLUMNS)
        };
        line.clear();
        for c in 0..COLUMNS {
            let a = c * bins.len() / COLUMNS;
            let b = ((c + 1) * bins.len() / COLUMNS).max(a + 1).min(bins.len());
            let peak = bins[a..b].iter().copied().fold(f32::NEG_INFINITY, f32::max);
            line.push(peak);
        }
        let (min, max) = line
            .iter()
            .filter(|x| x.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &x| {
                (lo.min(x), hi.max(x))
            });
        if min.is_finite() && max.is_finite() {
            if self.lines.is_empty() {
                (self.lo, self.hi) = (min, max);
            } else {
                self.lo += (min - self.lo) * 0.05;
                self.hi += (max - self.hi) * 0.05;
            }
        }
        self.lines.push_front(line);
        self.span_hz = span_hz;
        self.centre_hz = centre_hz;
    }

    /// Returns true if no frame came yet.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Returns the colour of `x` on the scale from `lo` to `hi`.
    fn colour(&self, x: f32) -> Color {
        let range = (self.hi - self.lo).max(1e-3);
        let t = ((x - self.lo) / range).clamp(0.0, 1.0);
        // Black, blue, cyan, yellow, red.
        let stops: [(f32, f32, f32); 5] = [
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 200.0),
            (0.0, 200.0, 220.0),
            (240.0, 230.0, 0.0),
            (255.0, 30.0, 0.0),
        ];
        let p = t * 4.0;
        // `p` is in 0..=4, so the cast is in range.
        let i = (p.floor() as usize).min(3);
        let f = p - i as f32;
        let (a, b) = (stops[i], stops[i + 1]);
        let mix = |x: f32, y: f32| (x + (y - x) * f) as u8;
        Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
    }
}

impl Widget for &Waterfall {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = usize::from(area.width).max(1);
        for y in 0..area.height {
            let upper = self.lines.get(usize::from(y) * 2);
            let lower = self.lines.get(usize::from(y) * 2 + 1);
            for x in 0..area.width {
                let c = usize::from(x) * COLUMNS / width;
                let top = upper
                    .and_then(|l| l.get(c))
                    .map_or(Color::Reset, |&v| self.colour(v));
                let bottom = lower
                    .and_then(|l| l.get(c))
                    .map_or(Color::Reset, |&v| self.colour(v));
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char('▀').set_fg(top).set_bg(bottom);
                }
            }
        }
    }
}
