//! Transmitter identification (EN 300 401 clause 14.8): the carrier pairs each transmitter switches on in the null symbol give its
//! main identifier (the pattern) and sub-identifier (the comb).

use crate::generator::tii_patterns;
use crate::ofdm::NullSpectrum;

/// A transmitter heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmitter {
    /// MainId (pattern).
    pub main: u8,
    /// SubId (comb).
    pub sub: u8,
    /// Level relative to the strongest, dB.
    pub level_db: f32,
    /// Its position from FIG 0/22, when known: latitude, longitude.
    pub position: Option<(f64, f64)>,
    /// Its delay from FIG 0/22, µs.
    pub delay_us: Option<u16>,
}

/// Averages null symbols and finds the transmitters in them.
#[derive(Clone, Debug)]
pub struct TiiDecoder {
    acc: Vec<f64>,
    frames: u32,
    patterns: Vec<[bool; 8]>,
    /// Null symbols analysed since the reset.
    pub analysed: u32,
    /// The transmitters of the last analysis.
    pub found: Vec<Transmitter>,
}

impl Default for TiiDecoder {
    fn default() -> Self {
        TiiDecoder {
            acc: vec![0.0; 1537],
            frames: 0,
            patterns: tii_patterns(),
            analysed: 0,
            found: Vec::new(),
        }
    }
}

/// Null symbols averaged per analysis.
pub const AVERAGE: u32 = 16;

impl TiiDecoder {
    /// A decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// One null symbol; returns true when a new analysis was made.
    pub fn push(&mut self, n: &NullSpectrum) -> bool {
        // Frames without TII carry only noise: use the frames whose comb positions stand out.
        let p = &n.power;
        let total: f32 = p.iter().sum();
        if total <= 0.0 {
            return false;
        }
        self.analysed += 1;
        for (a, &v) in self.acc.iter_mut().zip(p.iter()) {
            *a += v as f64;
        }
        self.frames += 1;
        if self.frames < AVERAGE {
            return false;
        }
        self.analyse();
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.frames = 0;
        true
    }

    fn pair(&self, k: i32) -> f64 {
        let i = (k + 768) as usize;
        self.acc.get(i).copied().unwrap_or(0.0) + self.acc.get(i + 1).copied().unwrap_or(0.0)
    }

    fn analyse(&mut self) {
        let mut sorted: Vec<f64> = self.acc.iter().copied().filter(|&v| v > 0.0).collect();
        if sorted.is_empty() {
            self.found.clear();
            return;
        }
        sorted.sort_by(|a, b| a.total_cmp(b));
        let floor = sorted[sorted.len() / 2] * 2.0 * 4.0;
        let mut found = Vec::new();
        for c in 0..24i32 {
            let mut e = [0f64; 8];
            for (b, eb) in e.iter_mut().enumerate() {
                let b = b as i32;
                *eb = self.pair(-768 + 2 * c + 48 * b)
                    + self.pair(-384 + 2 * c + 48 * b)
                    + self.pair(1 + 2 * c + 48 * b)
                    + self.pair(385 + 2 * c + 48 * b);
            }
            let mut idx: Vec<usize> = (0..8).collect();
            idx.sort_by(|&a, &b| e[b].total_cmp(&e[a]));
            let fourth = e[idx[3]];
            let fifth = e[idx[4]];
            if fourth < 8.0 * floor || fourth < 4.0 * fifth.max(floor) {
                continue;
            }
            let mut pat = [false; 8];
            for &i in &idx[..4] {
                pat[i] = true;
            }
            if let Some(main) = self.patterns.iter().position(|p| *p == pat) {
                let level: f64 = idx[..4].iter().map(|&i| e[i]).sum();
                found.push((main as u8, c as u8, level));
            }
        }
        let best = found.iter().map(|f| f.2).fold(0f64, f64::max).max(1e-30);
        self.found = found
            .into_iter()
            .map(|(main, sub, l)| Transmitter {
                main,
                sub,
                level_db: (10.0 * (l / best).log10()) as f32,
                position: None,
                delay_us: None,
            })
            .collect();
        self.found.sort_by(|a, b| b.level_db.total_cmp(&a.level_db));
    }
}
