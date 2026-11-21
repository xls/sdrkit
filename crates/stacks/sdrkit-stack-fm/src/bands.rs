//! The FM band that the host sets: the dial range and the channel raster.

/// The dial range and the channel raster of the band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandPlan {
    /// Lowest frequency of the band, Hz.
    pub min_hz: f64,
    /// Highest frequency of the band, Hz.
    pub max_hz: f64,
    /// Channel step, Hz.
    pub step_hz: f64,
    /// The first channel of the raster (channels are `first + k * step`), Hz.
    pub first_hz: f64,
}

impl Default for BandPlan {
    /// The band of ITU-R BS.450: 87.5 to 108 MHz with a 100 kHz step.
    fn default() -> BandPlan {
        BandPlan {
            min_hz: 87.5e6,
            max_hz: 108e6,
            step_hz: 100e3,
            first_hz: 87.5e6,
        }
    }
}

impl BandPlan {
    /// Every channel of the plan, lowest first.
    pub fn channels(&self) -> Vec<f64> {
        let n = ((self.max_hz - self.first_hz) / self.step_hz + 1e-6).floor() as usize;
        (0..=n)
            .map(|k| ((self.first_hz + k as f64 * self.step_hz) / 1e3).round() * 1e3)
            .collect()
    }

    /// The channel nearest to `hz`, held to the band.
    pub fn snap(&self, hz: f64) -> f64 {
        let k = ((hz - self.first_hz) / self.step_hz).round().max(0.0);
        let last = ((self.max_hz - self.first_hz) / self.step_hz + 1e-6).floor();
        ((self.first_hz + k.min(last) * self.step_hz) / 1e3).round() * 1e3
    }

    /// Whether `hz` is inside the band.
    pub fn contains(&self, hz: f64) -> bool {
        hz >= self.min_hz - 1.0 && hz <= self.max_hz + 1.0
    }
}
