//! NDB: non-directional beacons on 190 to 1750 kHz. The receiver finds the carrier, reads the Morse identification (a keyed tone
//! or a keyed carrier), plays the beacon, and lists every carrier that stands out in the whole input. A guard refuses words
//! without a carrier or without contrast, and the identification of a weak beacon is read from the average of its repeats
//! ([`crate::fold`]).
//! Standard: ICAO Annex 10 Volume I, 3.4.

use crate::carrier::{AmCarrier, Bfo, BfoAudio, CarrierInfo};
use crate::fields::{carrier_fields, put_carrier_status, put_idents};
use crate::fold::{IdentFold, IdentFolder};
use crate::ident::IdentEnvelope;
use crate::keyer::{key_state, push_key, KeySelect, KeySource, KeyState, Keying, ToneKeyer};
use crate::morse::{IdentTracker, MorseDecoder, MorseWord};
use crate::typed::{NdbFields, NdbScanRow, TunedFields, IDENTS, NDB_SCAN};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::MinFollower;
use sdrkit_algo_dsp::spectrum::CarrierSpectrum;
use sdrkit_components::dsp::{AudioResampler, Channel};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig, Generator,
    LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kept, Kind, Port, RateFrom, StackGraph, Timed,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ndb";
/// The least radio rate.
pub const MIN_RATE_HZ: f64 = 16e3;
/// The channel's width.
pub const CHANNEL_WIDTH_HZ: f64 = 3e3;
/// The channel's rate.
pub const CHANNEL_RATE_HZ: f64 = 8e3;
/// The decision time of the tone keyer: a noise bandwidth of about 50 Hz, a tenth of a dot at 7 WPM.
pub const TONE_DECISION_S: f64 = 0.02;

pub const MARGIN: Setting<field::F64> =
    Setting::new("margin", "Keying margin", "dB", 8.0).range(1.0, 40.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`CarrierKeyer`].
    pub enum CarrierKeyerSetting {
        /// How far above the floor the level must rise for key down.
        Margin = MARGIN,
    }
}

/// Keys on the level of the channel (`avnav.carrier_keyer`, iq -> [`Keying`]): for beacons that key the carrier itself (A1A).
///
/// Every 10 ms the mean power is compared with the noise floor (it follows a drop at once and rises by a factor of 1.002 a
/// period, a time constant of about 5 s) and with the recent peak.
pub struct CarrierKeyer {
    margin_db: f64,
    rate: f64,
    period: u64,
    count: u64,
    acc: f64,
    floor: MinFollower,
    peak: f64,
    state: KeyState,
}

/// The rise of the carrier keyer's floor in each 10 ms period, dB (a factor of 1.002).
const FLOOR_RISE_DB: f32 = 0.008_677;

impl Default for CarrierKeyer {
    fn default() -> Self {
        CarrierKeyer {
            margin_db: MARGIN.default,
            rate: 0.0,
            period: 1,
            count: 0,
            acc: 0.0,
            floor: MinFollower::new(1.0, FLOOR_RISE_DB)
                .unwrap_or_else(|_| unreachable!("the floor settings are constants in range")),
            peak: 0.0,
            state: KeyState::default(),
        }
    }
}

impl Component for CarrierKeyer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.carrier_keyer",
            vec![Port::iq("in")],
            vec![Port::typed::<Keying>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(CarrierKeyerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.rate = f.rate;
        self.period = ((0.01 * f.rate).round() as u64).max(1);
        self.count = 0;
        self.acc = 0.0;
        self.floor.reset();
        self.peak = 0.0;
        self.state = key_state(f.rate);
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<Keying>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.iq(0);
        let out = io.items_out::<Keying>(0);
        let margin = 10f64.powf(self.margin_db / 10.0);
        for (i, s) in x.iter().enumerate() {
            self.acc += s.norm_sqr() as f64;
            self.count += 1;
            if self.count >= self.period {
                let p = self.acc / self.period as f64;
                self.acc = 0.0;
                self.count = 0;
                let floor = self.floor.push(p as f32).unwrap_or(p as f32) as f64;
                self.peak = (self.peak * 0.995).max(p);
                let thr = (floor * margin).max(0.3 * self.peak);
                let want = if self.state.is_on() {
                    p > 0.5 * thr
                } else {
                    p > thr
                };
                let time = (first + i as u64 + 1) as f64 / self.rate;
                // The level of an element is an amplitude, as the tone keyer's is.
                push_key(
                    out,
                    self.state.decide(want, p.sqrt() as f32, self.period, time),
                );
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CarrierKeyerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CarrierKeyerSetting::Margin => self.margin_db = MARGIN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(CarrierKeyerSetting::Margin, Value::F64(self.margin_db))])
    }
}

/// A carrier found by the band scan.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScanPeak {
    /// Frequency, Hz (absolute: the stream's centre plus the offset).
    pub freq_hz: f64,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// Carrier to noise density, dB-Hz.
    pub cn0_dbhz: f32,
}

/// One scan of the band: the carriers found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanResult {
    /// Carriers, strongest first.
    pub peaks: Vec<ScanPeak>,
}

impl Item for ScanResult {
    const KIND: &'static str = "beacon.scan";
}

pub const INTERVAL: Setting<field::Duration> =
    Setting::new("interval", "Scan every", "s", 2.0).range(0.5, 60.0, 0.0);
pub const RESOLUTION: Setting<field::Hz> =
    Setting::new("resolution", "Resolution", "Hz", 2.0).range(0.5, 100.0, 0.0);
pub const SCAN_THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Scan threshold", "dB-Hz", 35.0).range(10.0, 90.0, 0.0);
pub const SPACING: Setting<field::Hz> =
    Setting::new("spacing", "Least spacing", "Hz", 300.0).range(10.0, 10e3, 0.0);

sdrkit_core::settings! {
    /// The settings of [`BeaconScan`].
    pub enum BeaconScanSetting {
        /// The time between two scans.
        Interval = INTERVAL,
        /// The width of a bin.
        Resolution = RESOLUTION,
        /// How far above the noise a carrier must stand.
        Threshold = SCAN_THRESHOLD,
        /// The least spacing of two carriers.
        Spacing = SPACING,
    }
}

/// The band scan (`avnav.beacon_scan`, iq -> [`ScanResult`]): every `interval` seconds, an averaged windowed FFT of the whole
/// input (bins of about `resolution` Hz) is searched for carriers standing `threshold` dB-Hz above the noise (from the median
/// bin), at least `spacing` Hz apart. A peak far below the strongest (a quantisation floor), or within 1.5 kHz of a carrier and
/// 10 dB below it (a modulation sideband), is not a carrier of its own.
pub struct BeaconScan {
    interval: f64,
    resolution: f64,
    threshold: f64,
    spacing: f64,
    max_peaks: usize,
    rate: f64,
    centre: f64,
    nfft: usize,
    fft: Option<Fft>,
    frame: Vec<Complex32>,
    fill: usize,
    spectrum: CarrierSpectrum,
    acc: Vec<f32>,
    mean: Vec<f32>,
    cands: Vec<(usize, f64)>,
    frames: u32,
    count: u64,
    every: u64,
    skip: usize,
    skipped: usize,
}

impl Default for BeaconScan {
    fn default() -> Self {
        BeaconScan {
            interval: INTERVAL.default,
            resolution: RESOLUTION.default,
            threshold: SCAN_THRESHOLD.default,
            spacing: SPACING.default,
            max_peaks: NDB_SCAN.cap,
            rate: 0.0,
            centre: 0.0,
            nfft: 0,
            fft: None,
            frame: Vec::new(),
            fill: 0,
            spectrum: CarrierSpectrum::default(),
            acc: Vec::new(),
            mean: Vec::new(),
            cands: Vec::new(),
            frames: 0,
            count: 0,
            every: 1,
            skip: 0,
            skipped: 0,
        }
    }
}

impl BeaconScan {
    fn scan(&mut self) -> ScanResult {
        let n = self.nfft;
        let frames = self.frames.max(1);
        self.mean.clear();
        self.mean.extend(self.acc.iter().map(|p| p / frames as f32));
        let n0 = self
            .spectrum
            .noise_density(&self.mean, frames)
            .unwrap_or(f64::INFINITY);
        let bin = self.rate / n as f64;
        self.cands.clear();
        // Leave out the edges, where the radio's filter rolls off.
        let edge = n / 20;
        for k in edge..n - edge {
            let i = (k + n / 2) % n; // k counts from the lowest frequency
            let p = self.mean[i] as f64;
            let l = self.mean[(i + n - 1) % n] as f64;
            let r = self.mean[(i + 1) % n] as f64;
            if p >= l && p > r {
                let pc = self.spectrum.carrier_power(l, p, r);
                let cn0 = 10.0 * (pc / n0.max(1e-30)).log10();
                if cn0 >= self.threshold {
                    self.cands.push((k, pc));
                }
            }
        }
        self.cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        let strongest = self.cands.first().map_or(0.0, |c| c.1);
        let mut peaks: Vec<ScanPeak> = Vec::new();
        for &(k, pc) in &self.cands {
            let f = self.centre + (k as f64 - (n / 2) as f64) * bin;
            // Far below the strongest (a quantisation floor), too close to a stronger one, or a modulation sideband of one
            // (within 1.5 kHz and 10 dB below it): not a carrier of its own.
            if pc < strongest * 1e-6
                || peaks.iter().any(|p| {
                    let d = (p.freq_hz - f).abs();
                    let ratio = 10f64.powf(p.level_dbfs as f64 / 10.0) / pc;
                    d < self.spacing || (d < 1500.0 && ratio > 10.0)
                })
            {
                continue;
            }
            peaks.push(ScanPeak {
                freq_hz: f,
                level_dbfs: (10.0 * pc.max(1e-20).log10()) as f32,
                cn0_dbhz: (10.0 * (pc / n0.max(1e-30)).log10()) as f32,
            });
            if peaks.len() >= self.max_peaks {
                break;
            }
        }
        ScanResult { peaks }
    }
}

impl Component for BeaconScan {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.beacon_scan",
            vec![Port::iq("in")],
            vec![Port::typed::<ScanResult>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(BeaconScanSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.rate = f.rate;
        self.centre = f.centre_hz;
        let mut n = 256usize;
        while (f.rate / n as f64) > self.resolution && n < 1 << 18 {
            n *= 2;
        }
        self.nfft = n;
        self.fft = Some(Fft::forward(n));
        self.frame = vec![Complex32::default(); n];
        self.fill = 0;
        // The edges, where the radio's filter rolls off, do not count for the noise.
        self.spectrum = CarrierSpectrum::new(n, f.rate, 0.9 * f.rate)?;
        self.acc = vec![0.0; n];
        self.mean = Vec::with_capacity(n);
        self.cands = Vec::with_capacity(n / 2);
        self.frames = 0;
        self.count = 0;
        self.every = (self.interval * f.rate).round().max(n as f64) as u64;
        // At most about a quarter of the samples go through the FFT (one frame in four is skipped when the rate is high).
        self.skip = if f.rate > 4e5 { 3 } else { 0 };
        self.skipped = 0;
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<ScanResult>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.iq(0);
        let n = self.nfft;
        let mut i = 0;
        while i < x.len() {
            let take = (n - self.fill).min(x.len() - i);
            if self.skipped == 0 {
                let w = self.spectrum.window();
                for k in 0..take {
                    self.frame[self.fill + k] = x[i + k] * w[self.fill + k];
                }
            }
            self.fill += take;
            self.count += take as u64;
            i += take;
            if self.fill == n {
                self.fill = 0;
                if self.skipped == 0 {
                    if let Some(fft) = &mut self.fft {
                        fft.process(&mut self.frame);
                    }
                    for (a, z) in self.acc.iter_mut().zip(&self.frame) {
                        *a += z.norm_sqr();
                    }
                    self.frames += 1;
                }
                self.skipped = if self.skipped >= self.skip {
                    0
                } else {
                    self.skipped + 1
                };
            }
            if self.count >= self.every && self.frames > 0 {
                self.count = 0;
                let r = self.scan();
                self.acc.iter_mut().for_each(|a| *a = 0.0);
                self.frames = 0;
                let time = (first + i as u64) as f64 / self.rate;
                io.items_out::<ScanResult>(0).push(Timed { time, value: r });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BeaconScanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BeaconScanSetting::Interval => self.interval = INTERVAL.read(id, v)?,
            BeaconScanSetting::Resolution => self.resolution = RESOLUTION.read(id, v)?,
            BeaconScanSetting::Threshold => {
                self.threshold = SCAN_THRESHOLD.read(id, v)?;
                return Ok(Applied::Live);
            }
            BeaconScanSetting::Spacing => {
                self.spacing = SPACING.read(id, v)?;
                return Ok(Applied::Live);
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (BeaconScanSetting::Interval, Value::Duration(self.interval)),
            (BeaconScanSetting::Resolution, Value::Hz(self.resolution)),
            (BeaconScanSetting::Threshold, Value::F64(self.threshold)),
            (BeaconScanSetting::Spacing, Value::Hz(self.spacing)),
        ])
    }
}

const WORD_CONTRAST: Setting<field::F64> = Setting::new("contrast", "Least contrast", "dB", 5.0)
    .range(0.0, 40.0, 0.0)
    .help("How far the marks of a word must stand above its gaps.");

sdrkit_core::settings! {
    /// The settings of [`WordGuard`].
    pub enum WordGuardSetting {
        /// The least contrast between the marks and the gaps of a word.
        Contrast = WORD_CONTRAST,
    }
}

/// A carrier seen this long before a word began still counts for it, s.
const CARRIER_HOLD_S: f64 = 1.0;

/// The repeats that confirm an identification of `letters` letters: five for one letter, three for two, else `confirm`.
fn repeats_needed(letters: usize, confirm: u32) -> u32 {
    match letters {
        0 | 1 => confirm.max(5),
        2 => confirm.max(3),
        _ => confirm,
    }
}

/// The guard against false words (`avnav.word_guard`): [`MorseWord`] and [`CarrierInfo`] in, [`MorseWord`] out. A word passes
/// when a carrier stood out during it and its marks stand at least the `contrast` setting above its gaps.
pub struct WordGuard {
    contrast_db: f64,
    /// When a carrier was last present, s.
    present_at: Option<f64>,
    /// Words refused since the stack was configured.
    refused: u64,
}

impl Default for WordGuard {
    fn default() -> Self {
        WordGuard {
            contrast_db: WORD_CONTRAST.default,
            present_at: None,
            refused: 0,
        }
    }
}

impl WordGuard {
    /// The words refused.
    pub fn refused(&self) -> u64 {
        self.refused
    }
}

impl Component for WordGuard {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.word_guard",
            vec![
                Port::typed::<MorseWord>("words"),
                Port::typed::<CarrierInfo>("carrier"),
            ],
            vec![Port::typed::<MorseWord>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(WordGuardSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.present_at = None;
        self.refused = 0;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(c) = io
            .items::<CarrierInfo>(1)
            .iter()
            .rev()
            .find(|c| c.value.stands_out)
        {
            self.present_at = Some(c.time);
        }
        let words = io.items::<MorseWord>(0);
        let out = io.items_out::<MorseWord>(0);
        for w in words {
            let carrier = self
                .present_at
                .is_some_and(|t| t >= w.time - CARRIER_HOLD_S);
            let contrast = w
                .value
                .contrast_db
                .is_some_and(|c| f64::from(c) >= self.contrast_db);
            if carrier && contrast {
                out.push(w.clone());
            } else {
                self.refused += 1;
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match WordGuardSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            WordGuardSetting::Contrast => self.contrast_db = WORD_CONTRAST.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(WordGuardSetting::Contrast, Value::F64(self.contrast_db))])
    }
}

pub const CONFIRM: Setting<field::I64> = Setting::new("confirm", "Hearings to confirm", "", 2)
    .range(1.0, 10.0, 1.0)
    .help("How many times an identification must be heard before it is shown.");

sdrkit_core::settings! {
    /// The settings of [`NdbReport`].
    pub enum NdbReportSetting {
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
    }
}

/// The NDB report (`avnav.ndb_report`): carrier, identification, the band scan and the average of the repeats in, results
/// out. An identification is confirmed after the hearings of [`repeats_needed`]; the word that the average of the repeats
/// reads while a carrier stands out goes before it. Its command `clear` forgets the station.
pub struct NdbReport {
    /// The last evaluation of the average of the repeats.
    fold: IdentFold,
    /// The identification that the average read.
    folded: Option<String>,
    tracker: IdentTracker,
    carrier: Option<CarrierInfo>,
    wpm: Option<f32>,
    tuned: f64,
    published: Published,
    summary: String,
    key: String,
}

fn new_published() -> Published {
    crate::typed::store(
        &[
            TunedFields::SHAPE,
            crate::typed::IdentFields::SHAPE,
            crate::typed::CarrierFields::SHAPE,
            NdbFields::SHAPE,
        ],
        &[&IDENTS, &NDB_SCAN],
    )
}

impl Default for NdbReport {
    fn default() -> Self {
        NdbReport {
            fold: IdentFold::default(),
            folded: None,
            tracker: IdentTracker::new(CONFIRM.default as u32),
            carrier: None,
            wpm: None,
            tuned: 0.0,
            published: new_published(),
            summary: String::with_capacity(64),
            key: String::with_capacity(16),
        }
    }
}

impl NdbReport {
    /// Forgets the station: the words heard, the average of the repeats, the carrier and the speed.
    fn forget_station(&mut self) {
        self.tracker.clear();
        self.fold = IdentFold::default();
        self.folded = None;
        self.carrier = None;
        self.wpm = None;
    }

    fn publish(&mut self) {
        let p = &mut self.published;
        p.put(&TunedFields {
            tuned_hz: Hertz(self.tuned),
        });
        p.put(&carrier_fields(self.carrier.as_ref(), self.tuned));
        let c = self.carrier.unwrap_or_default();
        // A short word needs more hearings than the tracker asks for; the average of the repeats confirms on its own.
        let heard = self.tracker.confirmed().filter(|id| {
            let n = self
                .tracker
                .words()
                .find(|(w, _)| w == id)
                .map_or(0, |(_, n)| n);
            n >= repeats_needed(id.chars().count(), self.tracker.confirm())
        });
        let ident = self.folded.as_deref().or(heard);
        put_idents(p, &self.tracker, ident, self.wpm);
        let r = self.fold.read.as_ref();
        p.put(&NdbFields {
            ident_period_s: r.map(|r| r.period_s),
            ident_period_z: r.map(|r| r.z),
            ident_fold_quality: r.map(|r| r.quality),
            ident_fold_text: r.map(|r| r.text.as_str()),
        });
        let carrier_khz = (self.tuned + c.offset_hz) / 1e3;
        self.summary.clear();
        // Writing into a String cannot fail.
        let _ = match (ident, c.present) {
            (Some(id), _) => write!(self.summary, "{id}, {carrier_khz:.1} kHz"),
            (None, true) => write!(self.summary, "Carrier at {carrier_khz:.3} kHz"),
            _ => write!(self.summary, "Searching"),
        };
        // A weak carrier that stands out in the synchronous detector holds the lock of its identification.
        let lock_state = match (c.present || c.stands_out, ident.is_some()) {
            (true, true) => LockState::Locked,
            (true, false) => LockState::Tracking,
            _ => LockState::Searching,
        };
        put_carrier_status(
            p,
            lock_state,
            self.carrier.as_ref(),
            CHANNEL_WIDTH_HZ,
            &self.summary,
        );
    }

    /// Writes the rows of a scan, one per peak it found, and removes the rows of carriers it did not find.
    fn put_scan(&mut self, s: &ScanResult) {
        let p = &mut self.published;
        for b in &s.peaks {
            self.key.clear();
            // Writing into a String cannot fail.
            let _ = write!(self.key, "{:.0}", b.freq_hz);
            NDB_SCAN.put(
                p,
                &self.key,
                &NdbScanRow {
                    freq_hz: Hertz(b.freq_hz),
                    level_dbfs: f64::from(b.level_dbfs),
                    cn0_dbhz: f64::from(b.cn0_dbhz),
                },
            );
        }
        let Some(table) = p
            .results()
            .decl()
            .table_id(NDB_SCAN.key)
            .and_then(|id| p.results().table(id))
        else {
            return;
        };
        if table.len() > s.peaks.len() {
            let gone: Vec<String> = table
                .rows_newest_first()
                .map(|(k, _)| k)
                .filter(|k| {
                    !s.peaks
                        .iter()
                        .any(|b| k.parse::<f64>().is_ok_and(|f| f == b.freq_hz.round()))
                })
                .map(str::to_owned)
                .collect();
            for k in gone {
                p.remove_row(&NDB_SCAN, &k);
            }
        }
    }
}

impl Component for NdbReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "avnav.ndb_report",
            vec![
                Port::typed::<CarrierInfo>("carrier"),
                Port::typed::<MorseWord>("ident"),
                Port::typed::<ScanResult>("scan"),
                Port::typed::<IdentFold>("fold"),
            ],
        )
        .with_settings(NdbReportSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned = inputs[0].tuned_hz;
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        if let Some(c) = io.items::<CarrierInfo>(0).last() {
            self.carrier = Some(c.value);
            changed = true;
        }
        for w in io.items::<MorseWord>(1) {
            self.tracker.heard(&w.value.text, w.time);
            self.wpm = Some(w.value.wpm);
            self.published
                .results_mut()
                .log(format!("{:.1} s: ident {}", w.time, w.value.text));
            changed = true;
        }
        if let Some(s) = io.items::<ScanResult>(2).last() {
            self.put_scan(&s.value);
            changed = true;
        }
        for f in io.items::<IdentFold>(3) {
            // The reading counts while a carrier stands out.
            let present = self.carrier.is_some_and(|c| c.stands_out);
            self.fold = IdentFold {
                read: f.value.read.clone().filter(|_| present),
            };
            if let Some(r) = &self.fold.read {
                if self.folded.as_deref() != Some(r.text.as_str()) {
                    self.published.results_mut().log(format!(
                        "{:.1} s: ident {} from the average of its repeats (period {:.2} s, quality {:.1})",
                        f.time, r.text, r.period_s, r.quality
                    ));
                    self.folded = Some(r.text.clone());
                }
            }
            changed = true;
        }
        if changed {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NdbReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NdbReportSetting::Confirm => self.tracker.set_confirm(CONFIRM.read(id, v)? as u32),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            NdbReportSetting::Confirm,
            Value::I64(i64::from(self.tracker.confirm())),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.forget_station();
                self.published = new_published();
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        self.forget_station();
        self.published.clear_rows(&IDENTS);
        if !kept.table(NDB_SCAN.key) {
            self.published.clear_rows(&NDB_SCAN);
        }
        self.published.results_mut().clear_log();
        self.publish();
    }
}

/// The NDB graph: the channel to the carrier stage, the tone keyer and the carrier keyer to the Morse decoder, the beat
/// oscillator to the audio, and the band scan of the whole input.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add(
            "chan",
            GraphError::checked("chan", Channel::new(CHANNEL_WIDTH_HZ, CHANNEL_RATE_HZ))?,
        )
        .set("chan.wide", Value::Hz(CHANNEL_WIDTH_HZ))
        .add("carrier", GraphError::checked("carrier", AmCarrier::new(1000.0))?)
        .add("keyer", GraphError::checked("keyer", ToneKeyer::new(0.0))?)
        .set("keyer.period", Value::Duration(TONE_DECISION_S))
        .add("ckey", CarrierKeyer::default())
        .add("select", KeySelect::new(KeySource::Auto))
        .add("morse", MorseDecoder::default())
        .add("guard", WordGuard::default())
        .add("fold", IdentFolder::default())
        .add("bfo", Bfo::new(BfoAudio::Am))
        .add("audio", GraphError::checked("audio", AudioResampler::new(48e3))?)
        .add("scan", BeaconScan::default())
        .add("report", NdbReport::default())
        .input("chan")
        .input("scan")
        .connect("chan", "carrier")
        .connect("chan", "ckey")
        .connect("chan", "bfo.in")
        .connect("carrier.info", "bfo.info")
        .connect("carrier.am", "keyer")
        .connect("keyer", "select.a")
        .connect("ckey", "select.b")
        .connect("select", "morse")
        .connect("morse", "guard.words")
        .connect("carrier.info", "guard.carrier")
        .connect("carrier.am", "fold.am")
        .connect("chan", "fold.iq")
        .connect("bfo", "audio")
        .connect("carrier.info", "report.carrier")
        .connect("guard", "report.ident")
        .connect("scan", "report.scan")
        .connect("fold", "report.fold")
        .output("audio")
        .output("guard")
        .clock(0.1)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

pub const IDENT_KEYING: Setting<field::Choice<KeySource>> =
    Setting::choice("ident_keying", "Identification", KeySource::Auto)
        .help("A keyed tone, a keyed carrier, or the one that keys.");
pub const TONE: Setting<field::Hz> = Setting::new("tone", "Identification tone", "Hz", 0.0)
    .range(0.0, 3000.0, 0.0)
    .help("0 finds it (400 or 1020 Hz).");
pub const AUDIO: Setting<field::Choice<BfoAudio>> = Setting::choice("audio", "Audio", BfoAudio::Am);
pub const PITCH: Setting<field::Hz> =
    Setting::new("pitch", "Beat pitch", "Hz", 800.0).range(100.0, 3000.0, 0.0);
pub const CARRIER_THRESHOLD: Setting<field::F64> =
    Setting::new("carrier_threshold", "Carrier threshold", "dB-Hz", 30.0)
        .range(0.0, 90.0, 0.0)
        .section("Advanced");
pub const STACK_SCAN_THRESHOLD: Setting<field::F64> =
    Setting::new("scan_threshold", "Scan threshold", "dB-Hz", 35.0)
        .range(10.0, 90.0, 0.0)
        .section("Band scan")
        .help("How far above the noise a carrier must stand to be listed.");
pub const SYNC_TONES: Setting<field::Bool> = Setting::new(
    "sync_tones",
    "Synchronous tone detection",
    "",
    true,
)
.help(
    "The tone is read from the in-phase part against the followed carrier, not from the envelope.",
);
pub const IDENT_AVERAGE: Setting<field::Bool> =
    Setting::new("ident_average", "Average repeated idents", "", true).help(
        "The identification is also read from the average of its repeats over up to three minutes.",
    );
pub const SCAN_INTERVAL: Setting<field::Duration> =
    Setting::new("scan_interval", "Scan every", "s", 2.0)
        .range(0.5, 60.0, 0.0)
        .section("Band scan");

sdrkit_core::choice_enum! {
    /// How the test signal keys its identification.
    pub enum GenKeying {
        /// A keyed tone (A2A).
        A2a = ("a2a", "Keyed tone"),
        /// A keyed carrier (A1A).
        A1a = ("a1a", "Keyed carrier"),
    }
}

pub const GEN_IDENT: Setting<field::Text> =
    Setting::new("gen_ident", "Test signal: identification", "", "BK")
        .section("Test signal")
        .advanced();
pub const GEN_KEYING: Setting<field::Choice<GenKeying>> =
    Setting::choice("gen_keying", "Test signal: keying", GenKeying::A2a)
        .section("Test signal")
        .advanced();
pub const GEN_TONE: Setting<field::Hz> =
    Setting::new("gen_tone", "Test signal: tone", "Hz", 1020.0)
        .range(100.0, 3000.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_DEPTH: Setting<field::F64> =
    Setting::new("gen_depth", "Test signal: modulation depth", "", 0.3)
        .range(0.05, 1.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_WPM: Setting<field::F64> =
    Setting::new("gen_wpm", "Test signal: Morse speed", "wpm", 10.0)
        .range(3.0, 20.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_PAUSE: Setting<field::Duration> =
    Setting::new("gen_pause", "Test signal: pause", "s", 2.0)
        .range(0.5, 30.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_OTHERS: Setting<field::I64> =
    Setting::new("gen_others", "Test signal: other beacons", "", 2)
        .range(0.0, 8.0, 1.0)
        .section("Test signal")
        .help("Beacons beside the tuned one, for the band scan.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the NDB stack.
    pub enum NdbSetting {
        /// The keying of the identification.
        IdentKeying = IDENT_KEYING,
        /// The identification tone.
        Tone = TONE,
        /// The envelope or a beat note.
        Audio = AUDIO,
        /// The pitch of the beat note.
        Pitch = PITCH,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
        /// The least C/N0 of a carrier.
        CarrierThreshold = CARRIER_THRESHOLD,
        /// Synchronous tone detection.
        SyncTones = SYNC_TONES,
        /// The identification from the average of its repeats.
        IdentAverage = IDENT_AVERAGE,
        /// The least C/N0 of a carrier in the band scan.
        ScanThreshold = STACK_SCAN_THRESHOLD,
        /// The time between two band scans.
        ScanInterval = SCAN_INTERVAL,
        /// The identification of the test signal.
        GenIdent = GEN_IDENT,
        /// The keying of the test signal.
        GenKeying = GEN_KEYING,
        /// The tone of the test signal.
        GenTone = GEN_TONE,
        /// The modulation depth of the test signal's tone.
        GenDepth = GEN_DEPTH,
        /// The Morse speed of the test signal.
        GenWpm = GEN_WPM,
        /// The pause between identifications of the test signal.
        GenPause = GEN_PAUSE,
        /// The other beacons of the test signal.
        GenOthers = GEN_OTHERS,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "NDB needs a sample rate of at least 16 kHz")
        .route("ident_keying", &["select.source"])?
        .route("tone", &["keyer.tone"])?
        .route("audio", &["bfo.mode"])?
        .route("pitch", &["bfo.pitch"])?
        .route("confirm", &["report.confirm"])?
        .route("carrier_threshold", &["carrier.threshold"])?
        .route("sync_tones", &["carrier.sync"])?
        .route("ident_average", &["fold.enabled"])?
        .route("scan_threshold", &["scan.threshold"])?
        .route("scan_interval", &["scan.interval"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The NDB receiver.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// One beacon of the test signal.
struct Beacon {
    offset_hz: f64,
    amplitude: f32,
    keyed_carrier: bool,
    tone_hz: f64,
    depth: f64,
    keying: IdentEnvelope,
    phase: f64,
}

/// The NDB test signal: the tuned beacon at 0 Hz and others beside it, at unit mean carrier power in all.
pub struct NdbGenerator {
    rate: f64,
    beacons: Vec<Beacon>,
    env: Vec<f32>,
    n: u64,
}

impl NdbGenerator {
    /// The tuned beacon `ident` (`keyed_carrier`: A1A, else a tone of `depth`) and `others` more at other offsets (depth 0.3),
    /// each with its own identification.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rate: f64,
        ident: &str,
        keyed_carrier: bool,
        tone_hz: f64,
        depth: f64,
        wpm: f64,
        pause_s: f64,
        others: usize,
        seed: u64,
    ) -> Self {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        // Edges of 4 ms, so the keying does not click.
        let rise = 0.004;
        let mut beacons = vec![Beacon {
            offset_hz: 0.0,
            amplitude: 1.0,
            keyed_carrier,
            tone_hz,
            depth,
            keying: IdentEnvelope::new(ident, wpm, rate, 0.3, pause_s, rise),
            phase: rng.uniform() * TAU,
        }];
        let names = ["ML", "WG", "CB", "ARM", "TW", "KU", "NWA", "GTH"];
        for k in 0..others {
            // Spread over the input, clear of the tuned channel.
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let off = side * (4e3 + 3.1e3 * (k / 2) as f64).min(0.4 * rate);
            beacons.push(Beacon {
                offset_hz: off,
                amplitude: (1.0 + rng.uniform() as f32) / 6.0,
                keyed_carrier: false,
                tone_hz: if k % 3 == 0 { 400.0 } else { 1020.0 },
                depth: 0.3,
                keying: IdentEnvelope::new(names[k % names.len()], wpm, rate, 1.0, pause_s, rise),
                phase: rng.uniform() * TAU,
            });
        }
        // Unit mean power: the carriers' powers add up to 1.
        let total: f32 = beacons.iter().map(|b| b.amplitude * b.amplitude).sum();
        for b in &mut beacons {
            b.amplitude /= total.sqrt();
        }
        NdbGenerator {
            rate,
            beacons,
            env: Vec::new(),
            n: 0,
        }
    }
    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        NdbGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_IDENT),
            s.get(&GEN_KEYING) == GenKeying::A1a,
            s.get(&GEN_TONE),
            s.get(&GEN_DEPTH),
            s.get(&GEN_WPM),
            s.get(&GEN_PAUSE),
            s.get(&GEN_OTHERS).max(0) as usize,
            cfg.seed,
        )
    }
    /// The beacons' offsets, Hz.
    pub fn offsets(&self) -> Vec<f64> {
        self.beacons.iter().map(|b| b.offset_hz).collect()
    }
}

impl Generator for NdbGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(CHANNEL_WIDTH_HZ))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        for b in self.beacons.iter_mut() {
            self.env.clear();
            self.env.extend(b.keying.next(out.len()));
            for (i, (o, &k)) in out.iter_mut().zip(&self.env).enumerate() {
                let t = (self.n + i as u64) as f64 / self.rate;
                let e = if b.keyed_carrier {
                    k as f64
                } else {
                    1.0 + b.depth * k as f64 * (TAU * ((b.tone_hz * t) % 1.0)).cos()
                };
                let ph = b.phase + TAU * ((b.offset_hz * t) % 1.0);
                *o += Complex32::new((e * ph.cos()) as f32, (e * ph.sin()) as f32) * b.amplitude;
            }
        }
        self.n += out.len() as u64;
        out.len()
    }
}

/// Makes the NDB generator.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(NdbGenerator::from_config(cfg)))
}
