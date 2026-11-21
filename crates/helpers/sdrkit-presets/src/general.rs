//! Data of the `general` stack: its presets (`general/presets.csv`), the band keys of a phone screen
//! (`general/band_keys.csv`), the amateur voice segments by IARU region (`general/ham_voice.csv`), and the
//! shortwave and medium-wave broadcast bands of the AM broadcasting plans `bc_r1` to `bc_r3` (`plans/broadcast.json`).
//!
//! [`presets_setting`] gives the stack setting `presets`. The other functions give what a host shows: keys, segments
//! and bands, with the frequency and the demodulator key (`am`, `nfm`, `usb` ...) that a key sets.

use crate::data::{Data, Table};
use crate::error::DataError;
use sdrkit_core::Value;
use sdrkit_data_plans::Band;
use std::fmt::Write as _;
use std::path::Path;

/// The path of the presets in the data folder.
pub const PRESETS: &str = "general/presets.csv";
/// The path of the band keys in the data folder.
pub const BAND_KEYS: &str = "general/band_keys.csv";
/// The path of the amateur voice segments in the data folder.
pub const HAM_VOICE: &str = "general/ham_voice.csv";

/// The id of the AM broadcasting plan of an ITU region (1 to 3).
fn broadcast_id(region: u8) -> String {
    format!("bc_r{region}")
}

/// One preset of the general receiver.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralPreset {
    /// The row key ("pmr446").
    pub key: String,
    /// The name for people.
    pub name: String,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The demodulator key ("nfm").
    pub demod: String,
    /// The filter width, Hz.
    pub width_hz: f64,
}

/// One key of a band row.
#[derive(Clone, Debug, PartialEq)]
pub struct BandKey {
    /// The row that shows it: a demodulator key (`usb` for the USB and LSB row) or `mw`.
    pub group: String,
    /// The label ("40 m", "MARINE", "88.1").
    pub label: String,
    /// The frequency it tunes, Hz.
    pub freq_hz: f64,
    /// The demodulator key it sets.
    pub demod: String,
}

/// One voice segment of an amateur band.
#[derive(Clone, Debug, PartialEq)]
pub struct HamSegment {
    /// The band ("40 m").
    pub band: String,
    /// The key ("40m", "2m FM").
    pub key: String,
    /// The lower edge, Hz.
    pub low_hz: f64,
    /// The upper edge, Hz.
    pub high_hz: f64,
    /// The frequency a key tunes, Hz.
    pub tune_hz: f64,
    /// The demodulator key it sets.
    pub demod: String,
    /// The IARU regions it applies to.
    pub regions: Vec<u8>,
    /// Extra text (channels, repeaters).
    pub note: String,
}

/// A shortwave broadcast band over the three ITU regions.
#[derive(Clone, Debug, PartialEq)]
pub struct ShortwaveBand {
    /// "31 m".
    pub name: String,
    /// The widest edges over the three regions, Hz.
    pub hull: (f64, f64),
    /// The edges in Regions 1, 2 and 3, Hz. `None` for no broadcasting.
    pub regions: [Option<(f64, f64)>; 3],
    /// Tropical zone only.
    pub tropical: bool,
    /// The plan's note for the key's help (empty for none).
    pub note: String,
}

impl ShortwaveBand {
    /// The edges in `region` (1 to 3). `None` for no broadcasting there.
    pub fn edges(&self, region: u8) -> Option<(f64, f64)> {
        self.regions
            .get((region as usize).wrapping_sub(1))
            .copied()
            .flatten()
    }

    /// The centre, Hz: of the range of `region` when it is given and broadcasts there, else of the hull.
    pub fn centre_hz(&self, region: Option<u8>) -> f64 {
        let (lo, hi) = region.and_then(|r| self.edges(r)).unwrap_or(self.hull);
        (lo + hi) / 2.0
    }

    /// The help of the band's key.
    pub fn help(&self) -> String {
        let mut s = self.name.clone();
        if self.tropical {
            s.push_str(", tropical zone only");
        }
        for r in 1..=3u8 {
            match self.edges(r) {
                Some((lo, hi)) => {
                    let _ = write!(s, "; Region {r}: {:.0}-{:.0} kHz", lo / 1e3, hi / 1e3);
                }
                None => {
                    let _ = write!(s, "; Region {r}: no broadcasting");
                }
            }
        }
        if !self.note.is_empty() {
            s.push_str("; ");
            s.push_str(&self.note);
        }
        s
    }
}

/// A band of an AM broadcast band plan, Hz.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanBand {
    /// The band's name ("MW", "31 m").
    pub name: String,
    /// The lower edge (for MW the first channel), Hz.
    pub low_hz: f64,
    /// The upper edge (for MW the last channel), Hz.
    pub high_hz: f64,
    /// The channel spacing, Hz.
    pub spacing_hz: f64,
}

/// The AM broadcast band plan of one ITU region: medium wave and the region's shortwave bands.
#[derive(Clone, Debug, PartialEq)]
pub struct AmBandPlan {
    /// "AM broadcast, Region 1".
    pub name: String,
    /// The ITU region.
    pub region: u8,
    /// The bands, ascending.
    pub bands: Vec<PlanBand>,
}

impl AmBandPlan {
    /// Has a medium-wave band.
    pub fn has_medium_wave(&self) -> bool {
        self.bands.iter().any(|b| b.name == "MW")
    }
}

/// The spacing of the shortwave broadcast bands (the HFCC raster), Hz.
pub const SW_STEP_HZ: f64 = 5000.0;

/// The presets of the file `path` (the format of `general/presets.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_presets(path: &Path) -> Result<Vec<GeneralPreset>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &["key", "name", "freq_mhz", "demod", "width_hz"],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(GeneralPreset {
            key: t.text(r, "key").to_owned(),
            name: t.text(r, "name").to_owned(),
            freq_hz: (t.required(*line, r, "freq_mhz")? * 1e6).round(),
            demod: t.text(r, "demod").to_ascii_lowercase(),
            width_hz: t.required(*line, r, "width_hz")?,
        });
    }
    Ok(out)
}

/// The text of the stack setting `presets`: `key=frequency_hz,demod,width_hz,name` entries separated by `;`.
pub fn presets_text(presets: &[GeneralPreset]) -> String {
    let mut out = String::new();
    for p in presets {
        if !out.is_empty() {
            out.push(';');
        }
        let _ = write!(
            out,
            "{}={},{},{},{}",
            p.key, p.freq_hz, p.demod, p.width_hz, p.name
        );
    }
    out
}

/// The setting `presets` of the general stack.
pub fn presets_setting(presets: &[GeneralPreset]) -> (String, Value) {
    ("presets".to_owned(), Value::Text(presets_text(presets)))
}

/// The band keys of the file `path` (the format of `general/band_keys.csv`), in the order of the file.
///
/// # Errors
///
/// As [`read_presets`].
pub fn read_band_keys(path: &Path) -> Result<Vec<BandKey>, DataError> {
    let t = Table::read(path.to_owned(), &["group", "label", "freq_mhz", "demod"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(BandKey {
            group: t.text(r, "group").to_ascii_lowercase(),
            label: t.text(r, "label").to_owned(),
            freq_hz: (t.required(*line, r, "freq_mhz")? * 1e6).round(),
            demod: t.text(r, "demod").to_ascii_lowercase(),
        });
    }
    Ok(out)
}

/// The voice segments of the file `path` (the format of `general/ham_voice.csv`), in the order of the file.
///
/// # Errors
///
/// As [`read_presets`], and [`DataError::Format`] for a region that is not 1, 2 or 3.
pub fn read_ham_voice(path: &Path) -> Result<Vec<HamSegment>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "band", "key", "low_mhz", "high_mhz", "tune_mhz", "demod", "regions", "note",
        ],
    )?;
    let mhz = |line: usize, r: &[String], c: &str| -> Result<f64, DataError> {
        Ok((t.required(line, r, c)? * 1e6).round())
    };
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let mut regions = Vec::new();
        for w in t.text(r, "regions").split_whitespace() {
            match w.parse::<u8>() {
                Ok(n @ 1..=3) => regions.push(n),
                _ => {
                    return Err(DataError::Format {
                        path: path.to_owned(),
                        reason: format!(
                            "line {line}: `{w}` in the column `regions` is not 1, 2 or 3"
                        ),
                    })
                }
            }
        }
        out.push(HamSegment {
            band: t.text(r, "band").to_owned(),
            key: t.text(r, "key").to_owned(),
            low_hz: mhz(*line, r, "low_mhz")?,
            high_hz: mhz(*line, r, "high_mhz")?,
            tune_hz: mhz(*line, r, "tune_mhz")?,
            demod: t.text(r, "demod").to_ascii_lowercase(),
            regions,
            note: t.text(r, "note").to_owned(),
        });
    }
    Ok(out)
}

/// The segments of `region`, in the order of `segments`.
pub fn ham_plan(segments: &[HamSegment], region: u8) -> Vec<&HamSegment> {
    segments
        .iter()
        .filter(|s| s.regions.contains(&region))
        .collect()
}

/// The segment of `region` holding `hz` with the demodulator key `demod` (the narrower one when two hold it): the key
/// that is lit.
pub fn ham_segment_at<'a>(
    segments: &'a [HamSegment],
    region: u8,
    hz: f64,
    demod: &str,
) -> Option<&'a HamSegment> {
    ham_plan(segments, region)
        .into_iter()
        .filter(|s| s.demod == demod && hz >= s.low_hz && hz <= s.high_hz)
        .min_by(|a, b| (a.high_hz - a.low_hz).total_cmp(&(b.high_hz - b.low_hz)))
}

/// The band a segment name belongs to: "60 m (upper)", the part of 60 m above the standard-frequency service, is 60 m.
fn band_name(segment: &str) -> &str {
    segment.split(" (").next().unwrap_or(segment)
}

/// The first and last channel and the spacing of a medium-wave band, Hz; 9 kHz when the band has no raster.
fn medium_wave_channels(b: &Band) -> (f64, f64, f64) {
    let step = b.raster.as_ref().map_or(9000.0, |r| r.step_hz);
    let first = b.raster.as_ref().map_or(b.start_hz, |r| r.first_hz);
    (
        first,
        first + ((b.end_hz - first) / step).floor() * step,
        step,
    )
}

impl Data {
    /// The presets of `general/presets.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_presets`].
    pub fn general_presets(&self) -> Result<Vec<GeneralPreset>, DataError> {
        read_presets(&self.folder().join(PRESETS))
    }

    /// The voice segments of `general/ham_voice.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_ham_voice`].
    pub fn ham_voice(&self) -> Result<Vec<HamSegment>, DataError> {
        read_ham_voice(&self.folder().join(HAM_VOICE))
    }

    /// The band keys of the row of `group` (a demodulator key, `usb` for the USB and LSB row, or `mw`). In the `am`
    /// row the shortwave broadcast bands follow the first key, each tuning the centre of its hull.
    ///
    /// # Errors
    ///
    /// As [`read_band_keys`].
    pub fn general_band_keys(&self, group: &str) -> Result<Vec<BandKey>, DataError> {
        let group = match group {
            "lsb" => "usb",
            g => g,
        };
        let mut keys: Vec<BandKey> = read_band_keys(&self.folder().join(BAND_KEYS))?
            .into_iter()
            .filter(|k| k.group == group)
            .collect();
        if group == "am" && !keys.is_empty() {
            let sw = self.shortwave_bands().into_iter().map(|b| BandKey {
                group: "am".to_owned(),
                freq_hz: b.centre_hz(None),
                label: b.name,
                demod: "am".to_owned(),
            });
            keys.splice(1..1, sw);
        }
        Ok(keys)
    }

    /// The medium-wave first channel, last channel and spacing of the AM broadcasting plan of `region` (1 to 3), Hz.
    pub fn medium_wave(&self, region: u8) -> Option<(f64, f64, f64)> {
        self.plans
            .group(&broadcast_id(region))?
            .bands
            .iter()
            .find(|b| b.name == "MW")
            .map(medium_wave_channels)
    }

    /// The shortwave broadcast bands of the AM broadcasting plans (ITU Radio Regulations Article 5), from 2300 to
    /// 30 000 kHz, sorted. A band in two parts (60 m) is one band whose edges span both parts.
    pub fn shortwave_bands(&self) -> Vec<ShortwaveBand> {
        let mut out: Vec<ShortwaveBand> = Vec::new();
        for region in 1..=3u8 {
            let Some(group) = self.plans.group(&broadcast_id(region)) else {
                continue;
            };
            for seg in group
                .bands
                .iter()
                .filter(|s| s.start_hz >= 2.3e6 && s.end_hz <= 30e6)
            {
                let name = band_name(&seg.name);
                let (lo, hi) = (seg.start_hz, seg.end_hz);
                let i = match out.iter().position(|b| b.name == name) {
                    Some(i) => i,
                    None => {
                        out.push(ShortwaveBand {
                            name: name.to_owned(),
                            hull: (lo, hi),
                            regions: [None; 3],
                            tropical: false,
                            note: String::new(),
                        });
                        out.len() - 1
                    }
                };
                let b = &mut out[i];
                let r = &mut b.regions[region as usize - 1];
                *r = Some(match *r {
                    Some((a, c)) => (a.min(lo), c.max(hi)),
                    None => (lo, hi),
                });
                b.hull = (b.hull.0.min(lo), b.hull.1.max(hi));
                b.tropical |= seg.note.contains("tropical");
                if b.note.is_empty() && !seg.note.is_empty() && !seg.note.contains("tropical") {
                    b.note.clone_from(&seg.note);
                }
            }
        }
        out.sort_by(|a, b| a.hull.0.total_cmp(&b.hull.0));
        out
    }

    /// The shortwave broadcast band whose hull holds `hz`.
    pub fn shortwave_band_of(&self, hz: f64) -> Option<ShortwaveBand> {
        self.shortwave_bands()
            .into_iter()
            .find(|b| hz >= b.hull.0 && hz <= b.hull.1)
    }

    /// The AM broadcast band plans, one per ITU region: medium wave (its first and last channel, on the region's
    /// raster) and the region's shortwave bands.
    pub fn am_band_plans(&self) -> Vec<AmBandPlan> {
        let sw = self.shortwave_bands();
        (1..=3u8)
            .map(|r| {
                let mut bands = Vec::new();
                if let Some((first, last, step)) = self.medium_wave(r) {
                    bands.push(PlanBand {
                        name: "MW".into(),
                        low_hz: first,
                        high_hz: last,
                        spacing_hz: step,
                    });
                }
                for b in &sw {
                    if let Some((lo, hi)) = b.edges(r) {
                        bands.push(PlanBand {
                            name: b.name.clone(),
                            low_hz: lo,
                            high_hz: hi,
                            spacing_hz: SW_STEP_HZ,
                        });
                    }
                }
                AmBandPlan {
                    name: format!("AM broadcast, Region {r}"),
                    region: r,
                    bands,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    }

    #[test]
    fn every_row_of_the_files_loads() {
        let d = data();
        let p = d.general_presets().unwrap();
        assert_eq!(p.len(), 11);
        assert!(p
            .iter()
            .all(|p| p.freq_hz > 0.0 && p.width_hz > 0.0 && !p.name.is_empty()));
        assert_eq!(
            presets_setting(&p[3..4]).1,
            Value::Text("pmr446=446006250,nfm,8500,PMR446".into())
        );
        let keys = read_band_keys(&d.folder().join(BAND_KEYS)).unwrap();
        assert_eq!(keys.len(), 43);
        let h = d.ham_voice().unwrap();
        assert_eq!(h.len(), 30);
        assert!(h
            .iter()
            .all(|s| !s.regions.is_empty() && s.low_hz < s.high_hz));
        assert_eq!(h[4].note, "channels 5.3305, 5.3465, 5.3570, 5.3715, 5.4035");
    }
}
