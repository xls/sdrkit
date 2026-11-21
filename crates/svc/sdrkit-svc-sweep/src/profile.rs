//! The settings of the sweep service, and the profile of one sweep that a start command makes from them: the bands, the
//! resolution, the dwell, the hit rules, the channel raster, the recording rules and the repetition.

use crate::channels::Raster;
use crate::grid::{resolution_choices, MAX_FFT};
use crate::plan::Range;
use crate::record::{RecordKind, RecordOptions};
use crate::schedule::Schedule;
use crate::stitch::StitchConfig;
use crate::SweepError;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

sdrkit_core::choice_enum! {
    /// What a sweep is for.
    pub enum Purpose {
        /// Finds signals: hits, channels and the pseudo-spectrum.
        Scan = ("scan", "Scan"),
        /// Also builds a band survey over time.
        Survey = ("survey", "Survey"),
        /// Learns the band, then hunts for new interference.
        Hunt = ("hunt", "Interference hunt"),
        /// Sweeps a few fixed bands once and returns advice about the station.
        Health = ("health", "Station health"),
    }
}

pub(crate) const BIN: Setting<field::Hz> = Setting::new("bin_hz", "Resolution", "Hz", 10e3)
    .range(1.0, 1e9, 0.0)
    .help("The width of one bin of the sweep. A preset and the whole range choose their own.");
pub(crate) const RATE: Setting<field::Hz> = Setting::new("rate_hz", "Sample rate", "Hz", 0.0)
    .range(0.0, 1e10, 0.0)
    .zero_means("The rate of the radio")
    .help("The sample rate to ask the radio for.");
pub(crate) const AVERAGES: Setting<field::I64> = Setting::new("averages", "Spectra per hop", "", 4)
    .range(1.0, 1000.0, 1.0)
    .help("The spectra averaged on each hop (the dwell).");
pub(crate) const SETTLE_AUTO: Setting<field::Bool> =
    Setting::new("settle_auto", "Measure the settle time", "", true)
        .help("Measures the settle time of the radio after a retune when the sweep starts.");
pub(crate) const SETTLE: Setting<field::Duration> =
    Setting::new("settle", "Settle time", "s", 0.004)
        .range(0.0, 1.0, 0.0)
        .help("The time after a retune before the spectra are used, if it is not measured.");
pub(crate) const FLUSH: Setting<field::I64> =
    Setting::new("flush_samples", "Samples in flight", "", 0)
        .range(0.0, 1e8, 1.0)
        .help("The samples dropped after the settle time.");
pub(crate) const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Threshold", "dB", 10.0)
        .range(0.1, 100.0, 0.0)
        .help("A hit opens this far over the noise floor. The whole range chooses its own.");
pub(crate) const HYSTERESIS: Setting<field::F64> =
    Setting::new("hysteresis_db", "Hysteresis", "dB", 4.0)
        .range(0.0, 100.0, 0.0)
        .help("A hit closes this far under the threshold.");
pub(crate) const MIN_DURATION: Setting<field::Duration> =
    Setting::new("min_duration", "Shortest hit", "s", 0.0).range(0.0, 3600.0, 0.0);
pub(crate) const HANG: Setting<field::Duration> = Setting::new("hang", "Hang time", "s", 2.0)
    .range(0.0, 3600.0, 0.0)
    .help("A hit stays open this long after it was last seen.");
pub(crate) const MIN_SWEEPS: Setting<field::I64> =
    Setting::new("min_sweeps", "Sweeps to confirm", "", 2)
        .range(1.0, 1e6, 1.0)
        .help("A channel shows once it was seen in this many sweeps.");
pub(crate) const RASTER_STEP: Setting<field::Hz> =
    Setting::new("raster_step_hz", "Channel raster", "Hz", 0.0)
        .range(0.0, 1e9, 0.0)
        .help("Channel centres snap to this raster. 0 for none.");
pub(crate) const RASTER_ORIGIN: Setting<field::Hz> =
    Setting::new("raster_origin_hz", "Raster origin", "Hz", 0.0)
        .range(0.0, 1e12, 0.0)
        .help("A channel centre of the raster.");
pub(crate) const USABLE: Setting<field::F64> = Setting::new("usable", "Usable share", "", 0.75)
    .range(0.05, 1.0, 0.0)
    .help("The share of the sample rate kept on each hop.");
pub(crate) const OVERLAP: Setting<field::F64> = Setting::new("overlap", "Overlap", "", 0.1)
    .range(0.0, 0.5, 0.0)
    .help("The least share of a hop that its neighbour also covers.");
pub(crate) const PRIORITY_EVERY: Setting<field::I64> =
    Setting::new("priority_every", "Priority interval", "hops", 8)
        .range(1.0, 1e6, 1.0)
        .help("A priority hop is visited again after this many hops.");
pub(crate) const DC_AUTO: Setting<field::Bool> =
    Setting::new("dc_auto", "DC bins by FFT size", "", true)
        .help("Drops max(5, FFT size / 512) bins at DC. If off, `dc_bins` gives the count.");
pub(crate) const DC_BINS: Setting<field::I64> = Setting::new("dc_bins", "DC bins", "", 5)
    .range(0.0, 4096.0, 1.0)
    .help("The FFT bins dropped at DC when `dc_auto` is off. 0 drops none.");
pub(crate) const INNER_KEEP: Setting<field::Hz> =
    Setting::new("inner_keep_hz", "Unused centre", "Hz", 0.0)
        .range(0.0, 1e9, 0.0)
        .help("Offsets from a hop centre closer than this are not used.");
pub(crate) const FLATTEN: Setting<field::Bool> = Setting::new("flatten", "Flatten hops", "", true)
    .help("Learns the shape that every hop shares and removes it.");
pub(crate) const REMOVE_SPURS: Setting<field::Bool> =
    Setting::new("remove_spurs", "Remove centre spurs", "", true);
pub(crate) const RECORD: Setting<field::Bool> = Setting::new("record", "Record hits", "", false)
    .help("Parks on a hop while its hits are recorded. The recordings table says what to record.");
pub(crate) const RECORD_KIND: Setting<field::Choice<RecordKind>> =
    Setting::choice("record_kind", "Record as", RecordKind::Audio);
pub(crate) const RECORD_MAX: Setting<field::I64> =
    Setting::new("record_max_channels", "Channels recorded at once", "", 4).range(1.0, 64.0, 1.0);
pub(crate) const RECORD_PARK: Setting<field::Duration> =
    Setting::new("record_max_park", "Longest park", "s", 60.0)
        .range(0.0, 86_400.0, 0.0)
        .help("The longest stay on a hop while its hits are recorded.");
pub(crate) const EVERY: Setting<field::Duration> = Setting::new("every", "Repeat every", "s", 0.0)
    .range(0.0, 86_400.0, 0.0)
    .help("The time from the start of one sweep to the next. 0 runs them back to back.");
pub(crate) const SWEEPS: Setting<field::I64> = Setting::new("sweeps", "Sweeps", "", 0)
    .range(0.0, 1e9, 1.0)
    .help("The sweeps in all. 0 sweeps until stopped. A health check sweeps once.");
pub(crate) const RADIO_SWEEP: Setting<field::Bool> =
    Setting::new("radio_sweep", "Use the sweep of the radio", "", true)
        .help("Lets a radio that can sweep by itself do so.");
pub(crate) const AVERAGING: Setting<field::I64> =
    Setting::new("averaging", "Sweeps averaged", "", 8)
        .range(1.0, 1000.0, 1.0)
        .help("The sweeps averaged in the average trace.");
pub(crate) const CPU_LOAD: Setting<field::F64> = Setting::new("cpu_load", "CPU load", "", 0.0)
    .range(0.0, 64.0, 0.0)
    .help("The CPU load of the host, 1 for one full core. It limits how many hits are recorded at once.");
pub(crate) const BURST_HZ: Setting<field::Hz> = Setting::new("burst_hz", "Burst watch", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .help("Watches this frequency for bursts, one level per sweep. 0 watches none.");
pub(crate) const BURST_RISE: Setting<field::F64> =
    Setting::new("burst_rise_db", "Burst rise", "dB", 6.0)
        .range(0.1, 100.0, 0.0)
        .help("A burst stands this far over the quiet median.");
pub(crate) const BURST_MIN: Setting<field::Duration> =
    Setting::new("burst_min", "Shortest burst", "s", 0.2).range(0.0, 3600.0, 0.0);
pub(crate) const DRIFT: Setting<field::Bool> =
    Setting::new("drift", "Find drifting carriers", "", false)
        .help("Looks for narrow carriers that drift steadily, in the spectra of each hop.");
pub(crate) const PERSISTENT: Setting<field::F64> =
    Setting::new("persistent_share", "Persistent share", "", 0.8)
        .range(0.0, 1.0, 0.0)
        .help("A survey signal is persistent if it is present this share of the time.");

pub(crate) const SURVEY_ROWS: Setting<field::I64> =
    Setting::new("survey_rows", "Survey rows", "", 512)
        .range(2.0, 4096.0, 1.0)
        .help("The most rows of the survey picture. Older rows are merged in pairs beyond it.");

sdrkit_core::settings! {
    /// The settings of the sweep service. A sweep takes them when it starts. `cpu_load` changes the sweep that runs.
    pub enum SweepSetting {
        /// The resolution.
        Bin = BIN,
        /// The sample rate.
        Rate = RATE,
        /// The spectra averaged per hop.
        Averages = AVERAGES,
        /// Measures the settle time.
        SettleAuto = SETTLE_AUTO,
        /// The settle time if it is not measured.
        Settle = SETTLE,
        /// The samples in flight.
        Flush = FLUSH,
        /// The hit threshold.
        Threshold = THRESHOLD,
        /// The hit hysteresis.
        Hysteresis = HYSTERESIS,
        /// The shortest hit.
        MinDuration = MIN_DURATION,
        /// The hang time.
        Hang = HANG,
        /// The sweeps to confirm a channel.
        MinSweeps = MIN_SWEEPS,
        /// The channel raster.
        RasterStep = RASTER_STEP,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN,
        /// The usable share of a hop.
        Usable = USABLE,
        /// The overlap of hops.
        Overlap = OVERLAP,
        /// The priority interval.
        PriorityEvery = PRIORITY_EVERY,
        /// The DC bins follow the FFT size.
        DcAuto = DC_AUTO,
        /// The DC bins.
        DcBins = DC_BINS,
        /// The unused centre of a hop.
        InnerKeep = INNER_KEEP,
        /// Flattens hops.
        Flatten = FLATTEN,
        /// Removes centre spurs.
        RemoveSpurs = REMOVE_SPURS,
        /// Records hits.
        Record = RECORD,
        /// What a recording holds.
        RecordKind = RECORD_KIND,
        /// The channels recorded at once.
        RecordMax = RECORD_MAX,
        /// The longest park.
        RecordPark = RECORD_PARK,
        /// The repeat time.
        Every = EVERY,
        /// The sweeps in all.
        Sweeps = SWEEPS,
        /// Lets the radio sweep.
        RadioSweep = RADIO_SWEEP,
        /// The sweeps averaged in the average trace.
        Averaging = AVERAGING,
        /// The CPU load of the host.
        CpuLoad = CPU_LOAD,
        /// The burst watch frequency.
        BurstHz = BURST_HZ,
        /// The burst rise.
        BurstRise = BURST_RISE,
        /// The shortest burst.
        BurstMin = BURST_MIN,
        /// Finds drifting carriers.
        Drift = DRIFT,
        /// The persistent share of a survey signal.
        Persistent = PERSISTENT,
        /// The rows of a survey.
        SurveyRows = SURVEY_ROWS,
    }
}

/// A band to sweep.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Band {
    /// A name ("2 m").
    pub label: &'static str,
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
}

/// The profile of one sweep.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScanProfile {
    pub name: &'static str,
    pub purpose: Purpose,
    pub bands: Vec<Band>,
    pub bin_hz: f64,
    pub rate_hz: Option<f64>,
    pub averages: usize,
    pub settle_s: Option<f64>,
    pub flush_samples: usize,
    pub threshold_db: f32,
    pub hysteresis_db: f32,
    pub min_duration_s: f64,
    pub hang_s: f64,
    pub min_sweeps: u64,
    pub raster: Option<Raster>,
    pub usable: f64,
    pub overlap: f64,
    pub exclusions: Vec<Range>,
    pub priority_hz: Vec<f64>,
    pub priority_every: usize,
    pub stitch: StitchConfig,
    pub record: RecordOptions,
    pub schedule: Schedule,
    pub radio_sweep: bool,
    pub averaging: usize,
}

fn count(v: i64) -> usize {
    usize::try_from(v).unwrap_or(0)
}

impl ScanProfile {
    /// The profile of the settings `v` over `bands`, named `name`.
    pub fn from_settings(
        v: &Values,
        purpose: Purpose,
        name: &'static str,
        bands: Vec<Band>,
    ) -> Self {
        let rate = v.get(&RATE);
        let step = v.get(&RASTER_STEP);
        let sweeps = v.get(&SWEEPS);
        ScanProfile {
            name,
            purpose,
            bands,
            bin_hz: v.get(&BIN),
            rate_hz: (rate > 0.0).then_some(rate),
            averages: count(v.get(&AVERAGES)).max(1),
            settle_s: (!v.get(&SETTLE_AUTO)).then(|| v.get(&SETTLE)),
            flush_samples: count(v.get(&FLUSH)),
            threshold_db: v.get(&THRESHOLD) as f32,
            hysteresis_db: v.get(&HYSTERESIS) as f32,
            min_duration_s: v.get(&MIN_DURATION),
            hang_s: v.get(&HANG),
            min_sweeps: u64::try_from(v.get(&MIN_SWEEPS)).unwrap_or(1),
            raster: (step > 0.0).then(|| Raster {
                origin_hz: v.get(&RASTER_ORIGIN),
                step_hz: step,
            }),
            usable: v.get(&USABLE),
            overlap: v.get(&OVERLAP),
            exclusions: Vec::new(),
            priority_hz: Vec::new(),
            priority_every: count(v.get(&PRIORITY_EVERY)).max(1),
            stitch: StitchConfig {
                dc_bins: (!v.get(&DC_AUTO)).then(|| count(v.get(&DC_BINS))),
                inner_keep_hz: v.get(&INNER_KEEP),
                flatten: v.get(&FLATTEN),
                remove_spurs: v.get(&REMOVE_SPURS),
            },
            record: RecordOptions {
                enabled: v.get(&RECORD),
                kind: v.get(&RECORD_KIND),
                max_channels: count(v.get(&RECORD_MAX)).max(1),
                max_park_s: v.get(&RECORD_PARK),
            },
            schedule: Schedule {
                every_s: v.get(&EVERY),
                count: u32::try_from(sweeps).ok().filter(|&n| n > 0),
            },
            radio_sweep: v.get(&RADIO_SWEEP),
            averaging: count(v.get(&AVERAGING)).max(1),
        }
    }

    /// Chooses the resolution, the dwell and the threshold for everything a radio tunes (`min_hz .. max_hz`) at `rate_hz`: the
    /// resolution keeps the grid near 200 000 bins (one of the resolution choices of the rate), the dwell and the threshold grow
    /// with the span.
    pub fn whole_range(&mut self, min_hz: f64, max_hz: f64, rate_hz: f64) {
        let span = (max_hz - min_hz).max(1.0);
        let want = span / 200_000.0;
        let choices = resolution_choices(rate_hz, MAX_FFT);
        self.bin_hz = choices
            .iter()
            .copied()
            .rev()
            .find(|&b| b >= want)
            .or_else(|| choices.first().copied())
            .unwrap_or(want);
        let (averages, threshold) = if span > 500e6 {
            (2, 12.0)
        } else if span > 50e6 {
            (4, 10.0)
        } else {
            (8, 8.0)
        };
        self.averages = averages;
        self.threshold_db = threshold;
        self.rate_hz = Some(rate_hz);
        self.bands = vec![Band {
            label: "Whole range",
            low_hz: min_hz,
            high_hz: max_hz,
        }];
    }

    /// The range covered: the lowest band's low edge to the highest band's high edge.
    pub fn span(&self) -> Option<Range> {
        let lo = self
            .bands
            .iter()
            .map(|b| b.low_hz)
            .fold(f64::INFINITY, f64::min);
        let hi = self
            .bands
            .iter()
            .map(|b| b.high_hz)
            .fold(f64::NEG_INFINITY, f64::max);
        (hi > lo).then(|| Range::new(lo, hi))
    }

    /// The exclusions plus the gaps between bands.
    pub fn all_exclusions(&self) -> Vec<Range> {
        let mut out = self.exclusions.clone();
        let mut bands: Vec<&Band> = self.bands.iter().collect();
        bands.sort_by(|a, b| a.low_hz.total_cmp(&b.low_hz));
        let mut reach = f64::NEG_INFINITY;
        for b in bands {
            if reach.is_finite() && b.low_hz > reach {
                out.push(Range::new(reach, b.low_hz));
            }
            reach = reach.max(b.high_hz);
        }
        out
    }

    /// Makes sure that the profile has a band and a positive resolution.
    ///
    /// # Errors
    ///
    /// [`SweepError::EmptyRange`] for no band or an empty band, [`SweepError::BadResolution`] for a resolution that is not
    /// positive.
    pub fn validate(&self) -> Result<(), SweepError> {
        if let Some(b) = self
            .bands
            .iter()
            .find(|b| b.high_hz.is_nan() || b.low_hz.is_nan() || b.high_hz <= b.low_hz)
        {
            return Err(SweepError::EmptyRange {
                low_hz: b.low_hz,
                high_hz: b.high_hz,
            });
        }
        if self.span().is_none() {
            return Err(SweepError::EmptyRange {
                low_hz: 0.0,
                high_hz: 0.0,
            });
        }
        if !(self.bin_hz.is_finite() && self.bin_hz > 0.0) {
            return Err(SweepError::BadResolution {
                bin_hz: self.bin_hz,
            });
        }
        Ok(())
    }
}

/// The range presets: name, low edge and high edge in MHz.
pub(crate) const PRESET_RANGES: &[(&str, f64, f64)] = &[
    ("Full", 1.0, 6000.0),
    ("FM broadcast", 87.5, 108.0),
    ("Airband", 108.0, 137.0),
    ("2 m", 144.0, 146.0),
    ("70 cm", 430.0, 440.0),
    ("ISM 433", 433.0, 435.0),
    ("ISM 868", 863.0, 870.0),
    ("Mobile", 700.0, 2700.0),
    ("L-band", 1000.0, 2000.0),
    ("2.4 GHz", 2400.0, 2500.0),
    ("5.8 GHz", 5650.0, 5925.0),
    ("Satellite IF", 950.0, 2150.0),
    ("Ku LNB IF", 739.0, 2150.0),
];

/// The resolution of a preset from `low_mhz` to `high_mhz`: about 1/2000 of its span, at least 1 kHz.
pub(crate) fn preset_bin_hz(low_mhz: f64, high_mhz: f64) -> f64 {
    ((high_mhz - low_mhz) * 1e6 / 2000.0).max(1e3)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sdrkit_core::SettingSet;

    /// The profile of the default settings over one range.
    pub(crate) fn profile(low_hz: f64, high_hz: f64, bin_hz: f64) -> ScanProfile {
        let mut p = ScanProfile::from_settings(
            &Values::defaults(&SweepSetting::schema()),
            Purpose::Scan,
            "test",
            vec![Band {
                label: "test",
                low_hz,
                high_hz,
            }],
        );
        p.bin_hz = bin_hz;
        p
    }

    #[test]
    fn defaults_and_raster() {
        let p = profile(144e6, 146e6, 5e3);
        assert_eq!(p.threshold_db, 10.0);
        assert_eq!(p.averages, 4);
        assert_eq!(p.settle_s, None);
        assert_eq!(p.raster, None);
        assert!(p.validate().is_ok());
        let mut empty = p.clone();
        empty.bands.clear();
        assert!(empty.validate().is_err());
    }

    #[test]
    fn whole_range_and_gaps() {
        let mut p = profile(0.0, 1.0, 1.0);
        p.whole_range(24e6, 1766e6, 2.4e6);
        assert!(
            p.bin_hz >= (1742e6 / 200_000.0) && p.bin_hz < 20e3,
            "{}",
            p.bin_hz
        );
        assert_eq!(p.averages, 2);
        assert_eq!(p.threshold_db, 12.0);
        let mut p = profile(100e6, 110e6, 1e4);
        p.bands.push(Band {
            label: "b",
            low_hz: 120e6,
            high_hz: 130e6,
        });
        assert_eq!(p.span(), Some(Range::new(100e6, 130e6)));
        assert_eq!(p.all_exclusions(), vec![Range::new(110e6, 120e6)]);
        assert!(PRESET_RANGES
            .iter()
            .all(|&(_, lo, hi)| hi > lo && preset_bin_hz(lo, hi) >= 1e3));
    }
}
