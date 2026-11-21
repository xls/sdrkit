//! The settings of the `fm` stack from the FM band groups of the plan files (`plans/fm.json`): the dial range and the
//! channel raster of a band.

use crate::data::Data;
use sdrkit_core::Value;
use sdrkit_data_plans::{BandGroup, PlanKind};

/// The dial range and the channel raster of one FM band group.
#[derive(Clone, Debug, PartialEq)]
pub struct FmBand {
    /// The id of the group (`fm_europe`).
    pub id: String,
    /// The name of the group.
    pub name: String,
    /// The lowest frequency, Hz.
    pub low_hz: f64,
    /// The highest frequency, Hz.
    pub high_hz: f64,
    /// The first channel, Hz.
    pub first_hz: f64,
    /// The channel step, Hz.
    pub raster_hz: f64,
}

impl FmBand {
    /// The band of the group `g`: its first band, with a 100 kHz raster from the band start if the band names none.
    /// `None` for a group that is not of the FM kind or has no band.
    pub(crate) fn of(g: &BandGroup) -> Option<FmBand> {
        let b = (g.kind == PlanKind::Fm).then(|| g.bands.first()).flatten()?;
        Some(FmBand {
            id: g.id.clone(),
            name: g.name.clone(),
            low_hz: b.start_hz,
            high_hz: b.end_hz,
            first_hz: b.raster.as_ref().map_or(b.start_hz, |r| r.first_hz),
            raster_hz: b.raster.as_ref().map_or(100e3, |r| r.step_hz),
        })
    }

    /// The `fm` settings of the band: `band_low_hz`, `band_high_hz`, `first_hz` and `raster_hz`.
    pub fn settings(&self) -> Vec<(String, Value)> {
        [
            ("band_low_hz", self.low_hz),
            ("band_high_hz", self.high_hz),
            ("first_hz", self.first_hz),
            ("raster_hz", self.raster_hz),
        ]
        .into_iter()
        .map(|(k, hz)| (k.to_owned(), Value::Hz(hz)))
        .collect()
    }
}

impl Data {
    /// Every FM band of the plan files, in their order.
    pub fn fm_bands(&self) -> Vec<FmBand> {
        self.plans.groups().filter_map(FmBand::of).collect()
    }

    /// The FM band with the group id `id`, with or without `fm_` in front.
    pub fn fm_band(&self, id: &str) -> Option<FmBand> {
        let id = id.trim();
        self.fm_bands()
            .into_iter()
            .find(|b| b.id == id || b.id.strip_prefix("fm_") == Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    }

    #[test]
    fn every_fm_band_of_the_file_loads() {
        let d = data();
        let all = d.fm_bands();
        assert_eq!(all.len(), 16);
        for b in &all {
            assert!(b.low_hz >= 60e6 && b.high_hz <= 110e6 && b.low_hz < b.high_hz, "{}", b.id);
            assert!(b.raster_hz >= 10e3 && b.first_hz >= b.low_hz && b.first_hz < b.high_hz, "{}", b.id);
            assert_eq!(b.settings().len(), 4);
        }
        let us = d.fm_band("north_america").unwrap();
        assert_eq!((us.first_hz, us.raster_hz), (88.1e6, 200e3));
        assert_eq!(d.fm_band("fm_europe").unwrap().settings()[0], ("band_low_hz".to_owned(), Value::Hz(87.5e6)));
        assert!(d.fm_band("mars").is_none());
    }
}
