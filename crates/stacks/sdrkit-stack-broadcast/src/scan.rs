//! Band scans for the broadcast stacks. The host gives the bands as plain values ([`Band`]). A [`BandScan`] steps the radio
//! over a band with [`HostRequest::Retune`], surveys the spectrum at each centre and lists the stations that each stack
//! decodes.

use crate::{CDR, CQUAM, DRM, HDRADIO};
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_core::interface::{Column, Fields, Hertz, Table};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{
    Complex32, ConfigError, HostRequest, InputInfo, LockState, Output, Owner, Published, Receiver,
    Results, StackConfig, StackDescriptor, Status, Values,
};
use std::sync::Arc;

/// The progress of a scan. No frequency to go to before the scan is over.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ProgressFields<'a> {
    /// The centres visited of all.
    #[field(label = "Progress")]
    pub progress: &'a str,
    /// The radio centre now.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Hertz,
    /// The stations found so far.
    #[field(label = "Found")]
    pub found: i64,
    /// Where the radio goes when the scan is over.
    #[field(label = "Tune to", unit = "Hz")]
    pub tune_to: Option<Hertz>,
    /// Whether the scan is over.
    #[field(label = "Done")]
    pub done: bool,
}

/// A station that the scan found. The row key is the frequency in whole hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct StationRow<'a> {
    /// The frequency of the station.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
    /// How far the station got.
    #[field(label = "State")]
    pub state: StationState,
    /// The name of the station.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The receiver's SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The receiver's level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// How far the signal stood above the noise floor.
    #[field(label = "Over floor", unit = "dB")]
    pub over_floor_db: f64,
}

/// The stations found, keyed by their frequency in Hz.
pub static STATIONS: Table<StationRow<'static>> = Table::new("stations", "Stations", 256);

/// One band that a scanned stack covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// The stack id (`drm`, `hdradio`, `cdr`, `cquam`).
    pub stack: &'static str,
    /// A short stable id (`sw31`, `fm`, `mw`).
    pub id: String,
    /// The name on the screen ("SW 31 m").
    pub name: String,
    /// The lowest frequency of the band, Hz.
    pub low_hz: f64,
    /// The highest frequency of the band, Hz.
    pub high_hz: f64,
    /// The first channel, Hz; the channels are `first_hz + k * raster_hz` up to `high_hz`.
    pub first_hz: f64,
    /// The channel raster, Hz.
    pub raster_hz: f64,
    /// Where the band's shortcut tunes.
    pub shortcut_hz: f64,
    /// The width one station takes, Hz.
    pub width_hz: f64,
    /// Gaps in a station's spectrum narrower than this are closed when signals are found (HD Radio and CDR leave room between an
    /// analogue FM station and their digital sidebands).
    pub merge_hz: f64,
}

impl Band {
    /// The band `id` named `name` of the scanned stack `stack`, from `low_hz` to `high_hz` with channels from `first_hz` on
    /// `raster_hz`. The width of a station and the gap that the signal finder closes come from [`station_width`]; the
    /// shortcut is the first channel. `None` for a stack without a band scan.
    pub fn new(
        stack: &str,
        id: &str,
        name: &str,
        low_hz: f64,
        high_hz: f64,
        first_hz: f64,
        raster_hz: f64,
    ) -> Option<Band> {
        let stack = descriptor(stack)?.id;
        let (width_hz, merge_hz) = station_width(stack, low_hz)?;
        Some(Band {
            stack,
            id: id.to_owned(),
            name: name.to_owned(),
            low_hz,
            high_hz,
            first_hz,
            raster_hz,
            shortcut_hz: first_hz,
            width_hz,
            merge_hz,
        })
    }
    /// The same band with its shortcut at `hz`, kept inside the band.
    #[must_use]
    pub fn shortcut(mut self, hz: f64) -> Band {
        self.shortcut_hz = hz.clamp(self.low_hz, self.high_hz);
        self
    }
    /// Every channel of the band, Hz.
    pub fn channels(&self) -> Vec<f64> {
        let n = ((self.high_hz - self.first_hz) / self.raster_hz + 1e-6).floor();
        (0..=n.max(-1.0) as i64)
            .map(|k| round_hz(self.first_hz + k as f64 * self.raster_hz))
            .collect()
    }
    /// True when `hz` lies in the band.
    pub fn contains(&self, hz: f64) -> bool {
        hz >= self.low_hz - 0.5 && hz <= self.high_hz + 0.5
    }
    /// The channel of the raster nearest `hz`, kept inside the band.
    pub fn nearest_channel(&self, hz: f64) -> f64 {
        let last = ((self.high_hz - self.first_hz) / self.raster_hz + 1e-6).floor();
        let k = ((hz - self.first_hz) / self.raster_hz)
            .round()
            .clamp(0.0, last.max(0.0));
        round_hz(self.first_hz + k * self.raster_hz)
    }
}

fn round_hz(hz: f64) -> f64 {
    (hz * 1000.0).round() / 1000.0
}

/// The width one station of `stack` takes in a band from `low_hz` up, and the widest gap inside its spectrum that the signal
/// finder closes, Hz: DRM30 10 kHz below 30 MHz and DRM+ 100 kHz above (ES 201 980), HD Radio 30 kHz on AM and 400 kHz on FM
/// (NRSC-5), CDR 500 kHz (GY/T 268.1), C-QUAM 20 kHz. `None` for a stack without a band scan.
pub fn station_width(stack: &str, low_hz: f64) -> Option<(f64, f64)> {
    let vhf = low_hz > 30e6;
    match stack {
        "drm" if vhf => Some((100e3, 5e3)),
        "drm" => Some((10e3, 1e3)),
        "hdradio" if vhf => Some((400e3, 80e3)),
        "hdradio" => Some((30e3, 6e3)),
        "cdr" => Some((500e3, 80e3)),
        "cquam" => Some((20e3, 1e3)),
        _ => None,
    }
}

/// What the step buttons of a stack move by at `hz`: DRM 100 kHz (the DRM+ channel raster, 1.3), HD Radio and C-QUAM one channel of
/// the band (the AM raster `am_raster_hz` below 30 MHz), CDR 100 kHz.
pub fn step_hz(stack: &str, hz: f64, am_raster_hz: f64) -> f64 {
    match stack {
        "drm" | "cdr" => 100e3,
        "hdradio" if hz > 30e6 => 200e3,
        "hdradio" | "cquam" => {
            if am_raster_hz > 9.5e3 {
                10e3
            } else {
                9e3
            }
        }
        _ => 0.0,
    }
}

/// One press of a step button (`up` or down) from `hz`: inside one of `bands` of the stack the next channel of its raster (from
/// between two channels, the nearest one in that direction), elsewhere `hz` plus or minus [`step_hz`].
pub fn step(bands: &[Band], stack: &str, hz: f64, up: bool, am_raster_hz: f64) -> f64 {
    let s = step_hz(stack, hz, am_raster_hz);
    if s == 0.0 {
        return hz;
    }
    let dir = if up { 1.0 } else { -1.0 };
    if stack != "drm" {
        if let Some(b) = bands.iter().find(|b| b.stack == stack && b.contains(hz)) {
            let k = (hz - b.first_hz) / b.raster_hz;
            let k = if (k - k.round()).abs() < 1e-6 {
                k.round() + dir
            } else if up {
                k.ceil()
            } else {
                k.floor()
            };
            let f = round_hz(b.first_hz + k * b.raster_hz);
            if b.contains(f) {
                return f;
            }
            return hz;
        }
    }
    round_hz(hz + dir * s)
}

/// The last frequency of each band (HD Radio 3.3: FM and AM are remembered apart; switching band goes to the remembered one).
#[derive(Clone, Debug, PartialEq)]
pub struct BandMemory {
    /// The last frequency per band id.
    pub last: Vec<(String, f64)>,
    bands: Vec<Band>,
}

impl BandMemory {
    /// The memory of `bands`, each band at its shortcut.
    pub fn new(bands: Vec<Band>) -> BandMemory {
        BandMemory {
            last: bands
                .iter()
                .map(|b| (b.id.clone(), b.shortcut_hz))
                .collect(),
            bands,
        }
    }
    /// Notes a tuned frequency in its band.
    pub fn tuned(&mut self, hz: f64) {
        if let Some(b) = self.bands.iter().find(|b| b.contains(hz)) {
            if let Some(e) = self.last.iter_mut().find(|e| e.0 == b.id) {
                e.1 = hz;
            }
        }
    }
    /// Where switching to band `id` goes.
    pub fn switch_to(&self, id: &str) -> Option<f64> {
        self.last.iter().find(|e| e.0 == id).map(|e| e.1)
    }
}

/// How long a stack's receiver gets at one candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanProfile {
    /// Without any sign of the signal (lock state searching) the candidate is left after this long, s.
    pub search_s: f64,
    /// From the start, a signal found but not decoded (lock state tracking) is left after this long, s.
    pub sync_s: f64,
    /// Once decoded, the scan waits at most this long for the station's name, s.
    pub label_s: f64,
    /// A found signal (lock state tracking) is already a station (C-QUAM: an AM carrier is a station, mono until the pilot comes).
    pub tracking_is_station: bool,
    /// The station must sit on one side of the radio's DC spike (DRM and C-QUAM); HD Radio and CDR keep their centre free.
    pub one_sided: bool,
}

/// The profile of a stack, `None` for a stack that is not scanned.
pub fn profile(stack: &str) -> Option<ScanProfile> {
    let p = |search_s, sync_s, label_s, tracking_is_station, one_sided| ScanProfile {
        search_s,
        sync_s,
        label_s,
        tracking_is_station,
        one_sided,
    };
    match stack {
        // DRM: the FAC needs up to 14 frames (5.6 s), the label one super frame more.
        "drm" => Some(p(4.0, 9.0, 3.0, false, true)),
        // HD Radio: block sync within a fraction of a second, a P1 frame every 1.486 s, the station name from the PIDS.
        "hdradio" => Some(p(3.0, 7.0, 8.0, false, false)),
        // CDR: a sub-frame is 160 ms, the system information comes with every sub-frame, the network table with the multiplex.
        "cdr" => Some(p(2.5, 6.0, 3.0, false, false)),
        // C-QUAM: the carrier within 0.1 s; the 25 Hz pilot within about 2 s.
        "cquam" => Some(p(1.0, 3.0, 0.0, true, true)),
        _ => None,
    }
}

/// The descriptor of a scanned stack.
pub fn descriptor(stack: &str) -> Option<&'static StackDescriptor> {
    match stack {
        "drm" => Some(&DRM),
        "hdradio" => Some(&HDRADIO),
        "cdr" => Some(&CDR),
        "cquam" => Some(&CQUAM),
        _ => None,
    }
}

/// The settings of a scan.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanSettings {
    /// After the radio moved, samples are dropped for this long, s.
    pub settle_s: f64,
    /// The spectrum is measured for this long at each centre, s.
    pub survey_s: f64,
    /// A signal counts when its bins stand this far above the noise floor, dB.
    pub min_over_db: f64,
    /// At the end, go to the strongest decoded station (else back to the frequency before the scan).
    pub tune_to_strongest: bool,
    /// The share of the sample band the scan uses (the rest is the radio's filter edges).
    pub usable: f64,
    /// Room kept free around the radio's DC spike for a one-sided stack, Hz.
    pub dc_guard_hz: f64,
}

impl Default for ScanSettings {
    fn default() -> Self {
        ScanSettings {
            settle_s: 0.1,
            survey_s: 0.5,
            min_over_db: 8.0,
            tune_to_strongest: true,
            usable: 0.8,
            dc_guard_hz: 5e3,
        }
    }
}

/// What the host must do after a call of [`BandScan::feed`].
#[derive(Clone, Debug, PartialEq)]
pub enum ScanRequest {
    /// Keep feeding.
    Continue,
    /// A [`HostRequest::Retune`] of the radio's centre: tune there, then call [`BandScan::radio_tuned`]; samples fed before that
    /// are dropped.
    Radio(HostRequest),
    /// The scan is over: tune the stack to this frequency.
    Done(f64),
}

impl ScanRequest {
    /// Returns the centre frequency that a [`ScanRequest::Radio`] requests.
    pub fn radio_hz(&self) -> Option<f64> {
        match self {
            ScanRequest::Radio(HostRequest::Retune { tuned_hz, .. }) => Some(*tuned_hz),
            _ => None,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How far a candidate got.
    #[derive(serde::Serialize)]
    pub enum StationState {
        /// Decoded: the receiver's lock state locked (C-QUAM: stereo).
        Decoded = ("decoded", "Decoded"),
        /// A signal of the stack was found but not decoded in time (C-QUAM: an AM station in mono).
        Found = ("found", "Found"),
    }
}

/// A station the scan found.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Station {
    /// The channel, Hz.
    pub freq_hz: f64,
    /// How far it got.
    pub state: StationState,
    /// The station's name as the stack reads it (DRM service labels, the HD Radio call sign, the CDR network), or what it is
    /// (C-QUAM: "AM stereo" or "AM mono").
    pub label: String,
    /// The receiver's summary line when it was left.
    pub summary: String,
    /// The receiver's SNR, dB.
    pub snr_db: Option<f32>,
    /// The receiver's level, dBFS.
    pub level_dbfs: Option<f32>,
    /// How far the signal stood above the noise floor of the capture, dB.
    pub over_floor_db: f64,
    /// Signal time since the scan started when the station was listed, s.
    pub at_s: f64,
}

/// A signal found in the spectrum of a capture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// The channel it was put on, Hz.
    pub freq_hz: f64,
    /// Its mean power over the noise floor, dB.
    pub over_db: f64,
}

/// Finds the signals of a band in a power spectrum (linear, bins from `-rate/2` up, DC at `n/2`) of a capture centred on
/// `centre_hz`: runs of bins above `floor + min_over_db` with gaps below `band.merge_hz` closed, each put on the raster at its
/// middle (a run wider than 1.5 stations produces one candidate per station width). Only channels that `fits` accepts are
/// returned, strongest first then by frequency, without repeats.
pub fn find_signals(
    power: &[f32],
    rate_hz: f64,
    centre_hz: f64,
    band: &Band,
    min_over_db: f64,
    fits: impl Fn(f64) -> bool,
) -> Vec<Candidate> {
    let n = power.len();
    if n == 0 {
        return Vec::new();
    }
    let bin = rate_hz / n as f64;
    let mut sorted: Vec<f32> = power.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = (sorted[n / 5] as f64).max(1e-30);
    let thr = floor * 10f64.powf(min_over_db / 10.0);
    // A light smoothing (three bins) keeps a noisy bin from splitting a run.
    let above: Vec<bool> = (0..n)
        .map(|i| {
            let a = power[i.saturating_sub(1)] as f64;
            let b = power[i] as f64;
            let c = power[(i + 1).min(n - 1)] as f64;
            (a + b + c) / 3.0 > thr
        })
        .collect();
    let gap = (band.merge_hz / bin).round() as usize;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        if !above[i] {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && above[i] {
            i += 1;
        }
        let e = i - 1;
        match runs.last_mut() {
            Some(last) if s - last.1 - 1 <= gap => last.1 = e,
            _ => runs.push((s, e)),
        }
    }
    let hz_of = |k: f64| centre_hz + (k - (n / 2) as f64) * bin;
    let mut out: Vec<Candidate> = Vec::new();
    for (s, e) in runs {
        // Bins of the DC spike alone are not a station.
        if e - s <= 4 && s <= n / 2 + 1 && e + 1 >= n / 2 {
            continue;
        }
        let lo = hz_of(s as f64 - 0.5);
        let hi = hz_of(e as f64 + 0.5);
        let mean = power[s..=e].iter().map(|&p| p as f64).sum::<f64>() / (e - s + 1) as f64;
        let over_db = 10.0 * (mean / floor).log10();
        let width = hi - lo;
        let mut centres = Vec::new();
        if width > 1.5 * band.width_hz {
            // Several stations side by side (or one with wide skirts): the power in a window of one station's width, slid over
            // the run, is taken at its highest, then again away from the stations already taken, while it holds above the
            // threshold over a whole window.
            let wb = ((band.width_hz / bin).round() as usize).clamp(1, e - s + 1);
            let sums: Vec<f64> = (s..=e + 1 - wb)
                .map(|k| power[k..k + wb].iter().map(|&p| p as f64).sum::<f64>())
                .collect();
            let mut taken: Vec<usize> = Vec::new();
            let keep = (0.75 * band.width_hz / bin) as usize;
            loop {
                let best = sums
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| taken.iter().all(|&t| t.abs_diff(*k) > keep))
                    .max_by(|a, b| a.1.total_cmp(b.1));
                match best {
                    Some((k, &v)) if v / wb as f64 > thr && taken.len() < 64 => {
                        taken.push(k);
                        centres.push(hz_of((s + k) as f64 + 0.5 * (wb as f64 - 1.0)));
                    }
                    _ => break,
                }
            }
        } else {
            centres.push(0.5 * (lo + hi));
        }
        for c in centres {
            if !band.contains(c) {
                continue;
            }
            let f = band.nearest_channel(c);
            if !fits(f) {
                continue;
            }
            match out.iter_mut().find(|x| (x.freq_hz - f).abs() < 1.0) {
                Some(x) => x.over_db = x.over_db.max(over_db),
                None => out.push(Candidate {
                    freq_hz: f,
                    over_db,
                }),
            }
        }
    }
    out.sort_by(|a, b| {
        b.over_db
            .total_cmp(&a.over_db)
            .then(a.freq_hz.total_cmp(&b.freq_hz))
    });
    out
}

/// The radio centres that cover `channels` (sorted): for a two-sided stack the channels of a group lie within `±reach_hz` of the
/// centre; for a one-sided one they lie from `guard_hz` to `reach_hz` below it (the radio is tuned above them). Centres are on
/// whole kHz.
pub fn centres(channels: &[f64], reach_hz: f64, guard_hz: f64, one_sided: bool) -> Vec<f64> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < channels.len() {
        let first = channels[i];
        let span = if one_sided {
            reach_hz - guard_hz - 1e3
        } else {
            2.0 * reach_hz - 2e3
        };
        let mut j = i;
        while j + 1 < channels.len() && channels[j + 1] - first <= span {
            j += 1;
        }
        let last = channels[j];
        let c = if one_sided {
            ((last + guard_hz) / 1e3).ceil() * 1e3
        } else {
            (0.5 * (first + last) / 1e3).round() * 1e3
        };
        out.push(c);
        i = j + 1;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Waiting for the host to move the radio.
    Radio,
    /// Dropping samples after a move.
    Settle,
    /// Measuring the spectrum.
    Survey,
    /// A receiver listens on a candidate.
    Listen,
    Done,
}

/// A band scan with a stack's receiver (see the module documentation).
pub struct BandScan {
    band: Band,
    desc: &'static StackDescriptor,
    profile: ScanProfile,
    settings: ScanSettings,
    stack_settings: Values,
    channels: Vec<f64>,
    centres: Vec<f64>,
    centre_idx: usize,
    rate: f64,
    live: bool,
    centre_hz: f64,
    start_hz: f64,
    phase: Phase,
    /// Samples left in the current phase (settling, or measuring the spectrum).
    left: u64,
    analyser: Option<SpectrumAnalyzer>,
    candidates: Vec<Candidate>,
    cand_idx: usize,
    rx: Option<Box<dyn Receiver>>,
    /// Samples the current receiver has had.
    heard: u64,
    /// Samples fed when the receiver first locked.
    locked_at: Option<u64>,
    fed: u64,
    stations: Vec<Station>,
    results: Published,
    reach: f64,
}

/// Why a band scan cannot start.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ScanError {
    /// The stack of the band has no band scan.
    #[error("{stack} has no band scan")]
    NoScan {
        /// The stack id.
        stack: &'static str,
    },
    /// The input does not suit the scan.
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl BandScan {
    /// A scan of `band` with the band's stack, its receivers made with `stack_settings`.
    ///
    /// # Errors
    ///
    /// [`ScanError::NoScan`] if the stack of the band has no band scan.
    pub fn new(
        band: Band,
        stack_settings: Values,
        settings: ScanSettings,
    ) -> Result<BandScan, ScanError> {
        let (Some(desc), Some(profile)) = (descriptor(band.stack), profile(band.stack)) else {
            return Err(ScanError::NoScan { stack: band.stack });
        };
        let channels = band.channels();
        Ok(BandScan {
            band,
            desc,
            profile,
            settings,
            stack_settings,
            channels,
            centres: Vec::new(),
            centre_idx: 0,
            rate: 0.0,
            live: true,
            centre_hz: 0.0,
            start_hz: 0.0,
            phase: Phase::Done,
            left: 0,
            analyser: None,
            candidates: Vec::new(),
            cand_idx: 0,
            rx: None,
            heard: 0,
            locked_at: None,
            fed: 0,
            stations: Vec::new(),
            results: published(),
            reach: 0.0,
        })
    }

    /// The band.
    pub fn band(&self) -> &Band {
        &self.band
    }

    /// The radio centres a live scan visits (known after [`BandScan::configure`]).
    pub fn centres(&self) -> &[f64] {
        &self.centres
    }

    /// The stations found so far, in the order found.
    pub fn stations(&self) -> &[Station] {
        &self.stations
    }

    /// The scan's results: a summary, progress fields, the table `stations` and a log.
    pub fn results(&self) -> &Results {
        self.results.results()
    }

    /// The receiver listening now, if any.
    pub fn receiver(&self) -> Option<&dyn Receiver> {
        self.rx.as_deref()
    }

    /// True once the scan is over.
    pub fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Starts the scan on the radio's input. A live radio is stepped over the band's centres (the first request is returned); a
    /// recording (`input.live == false`) is scanned as it is, over the channels inside it. Refuses a rate the stack refuses, and a
    /// rate too low for one station of the band.
    pub fn configure(&mut self, input: &InputInfo) -> Result<ScanRequest, ConfigError> {
        // HD Radio's FM band needs both sidebands and room for the filter (3.3).
        let min = if self.band.stack == "hdradio" && self.band.low_hz > 30e6 {
            crate::hd::MIN_RATE_FM
        } else {
            self.desc.tuning.min_rate_hz
        };
        if input.rate_hz < min {
            return Err(ConfigError::RateTooLow {
                need: min,
                got: input.rate_hz,
            });
        }
        if self.band.stack == "drm" && self.band.raster_hz >= 100e3 && input.rate_hz < 100e3 {
            // DRM+ (mode E, 100 kHz).
            return Err(ConfigError::RateTooLow {
                need: crate::drm::MIN_RATE_E,
                got: input.rate_hz,
            });
        }
        self.rate = input.rate_hz;
        self.live = input.live;
        self.start_hz = input.tuned_hz;
        self.reach = 0.5 * self.settings.usable * input.rate_hz - 0.5 * self.band.width_hz;
        let guard = self.guard();
        if self.reach <= guard {
            return Err(ConfigError::RateTooLow {
                need: ((guard + 0.5 * self.band.width_hz) * 2.0 / self.settings.usable / 1e3)
                    .ceil()
                    * 1e3,
                got: input.rate_hz,
            });
        }
        // The spectrum: bins of at most an eighth of the raster or of a station, whichever is less.
        let res = (self.band.raster_hz.min(self.band.width_hz) / 8.0).max(50.0);
        let n = ((input.rate_hz / res).ceil() as usize)
            .next_power_of_two()
            .clamp(256, 65_536);
        let mut cfg = SpectrumConfig::spec(n, input.rate_hz);
        cfg.frames_per_second = input.rate_hz / n as f64;
        cfg.average = 1 << 20;
        self.analyser = Some(SpectrumAnalyzer::new(cfg)?);
        self.stations.clear();
        self.results = published();
        self.fed = 0;
        self.rx = None;
        if self.live {
            self.centres = centres(&self.channels, self.reach, guard, self.profile.one_sided);
            self.centre_idx = 0;
            self.results.results_mut().log(format!(
                "scanning {} {}: {} channels from {} radio centres",
                self.desc.name,
                self.band.name,
                self.channels.len(),
                self.centres.len()
            ));
            Ok(self.next_centre())
        } else {
            self.centres = vec![input.centre_hz];
            self.centre_idx = 0;
            self.centre_hz = input.centre_hz;
            self.results.results_mut().log(format!(
                "scanning a recording for {} stations of {}",
                self.desc.name, self.band.name
            ));
            self.begin_settle();
            Ok(ScanRequest::Continue)
        }
    }

    fn guard(&self) -> f64 {
        if self.profile.one_sided {
            self.settings.dc_guard_hz + 0.5 * self.band.width_hz
        } else {
            0.0
        }
    }

    /// True when a station on `f` fits the capture at `centre` (inside the usable band, off the DC spike for a one-sided stack).
    fn fits(&self, centre: f64, f: f64) -> bool {
        let d = f - centre;
        if d.abs() > self.reach {
            return false;
        }
        !(self.profile.one_sided && d.abs() < self.guard())
    }

    /// The radio is on `centre_hz` now.
    pub fn radio_tuned(&mut self, centre_hz: f64) {
        if self.phase == Phase::Radio {
            self.centre_hz = centre_hz;
            self.begin_settle();
        }
    }

    /// Ends the scan; the request says where to go.
    pub fn stop(&mut self) -> ScanRequest {
        if self.phase != Phase::Done {
            self.results.results_mut().log("scan stopped");
            self.finish()
        } else {
            ScanRequest::Done(self.final_hz())
        }
    }

    fn begin_settle(&mut self) {
        self.phase = Phase::Settle;
        self.left = (self.settings.settle_s * self.rate) as u64;
        self.candidates.clear();
        self.cand_idx = 0;
        if let Some(a) = &mut self.analyser {
            a.reset();
        }
        self.progress();
    }

    fn next_centre(&mut self) -> ScanRequest {
        if !self.live || self.centre_idx >= self.centres.len() {
            return self.finish();
        }
        let c = self.centres[self.centre_idx];
        self.centre_idx += 1;
        self.phase = Phase::Radio;
        self.progress();
        ScanRequest::Radio(HostRequest::Retune {
            tuned_hz: c,
            reason: format!("band scan of {}: next centre", self.band.name),
        })
    }

    fn final_hz(&self) -> f64 {
        if self.settings.tune_to_strongest {
            if let Some(s) = self
                .stations
                .iter()
                .filter(|s| s.state == StationState::Decoded)
                .max_by(|a, b| a.over_floor_db.total_cmp(&b.over_floor_db))
            {
                return s.freq_hz;
            }
        }
        self.start_hz
    }

    fn finish(&mut self) -> ScanRequest {
        self.phase = Phase::Done;
        self.rx = None;
        let to = self.final_hz();
        let line = match self
            .stations
            .iter()
            .filter(|s| s.state == StationState::Decoded)
            .max_by(|a, b| a.over_floor_db.total_cmp(&b.over_floor_db))
        {
            Some(s) => format!(
                "Scan done, {} stations. Strongest: {}{}",
                self.stations.len(),
                freq_text(s.freq_hz),
                if s.label.is_empty() {
                    String::new()
                } else {
                    format!(", {}", s.label)
                }
            ),
            None if self.stations.is_empty() => "Scan done. No station found.".into(),
            None => format!(
                "Scan done, {} signals found, none decoded.",
                self.stations.len()
            ),
        };
        self.write_progress(Some(to));
        let r = self.results.results_mut();
        r.log(line.clone());
        r.set_status(&Status::new(LockState::Off).with_summary(&line));
        ScanRequest::Done(to)
    }

    fn progress(&mut self) {
        let what = match self.phase {
            Phase::Radio => "moving the radio".to_string(),
            Phase::Settle | Phase::Survey => "measuring the spectrum".to_string(),
            Phase::Listen => match self.candidates.get(self.cand_idx) {
                Some(c) => format!(
                    "listening on {} ({} / {})",
                    freq_text(c.freq_hz),
                    self.cand_idx + 1,
                    self.candidates.len()
                ),
                None => String::new(),
            },
            Phase::Done => return,
        };
        self.write_progress(None);
        let summary = format!(
            "scanning {}: centre {} / {}, {}",
            self.band.name,
            self.centre_idx.max(1),
            self.centres.len(),
            what
        );
        self.results
            .results_mut()
            .set_status(&Status::new(LockState::Searching).with_summary(&summary));
    }

    /// Writes the progress fields; `tune_to` is where the radio goes once the scan is over.
    fn write_progress(&mut self, tune_to: Option<f64>) {
        let progress = format!("{} / {}", self.centre_idx.max(1), self.centres.len());
        self.results.put(&ProgressFields {
            progress: &progress,
            centre: Hertz(self.centre_hz),
            found: self.stations.len() as i64,
            tune_to: tune_to.map(Hertz),
            done: tune_to.is_some(),
        });
    }

    /// Feeds samples of the radio (any length). The listening receiver's audio and media go to `out`.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Output) -> ScanRequest {
        let mut i = 0;
        while i < iq.len() {
            let rest = &iq[i..];
            match self.phase {
                Phase::Done => return ScanRequest::Continue,
                Phase::Radio => return ScanRequest::Continue,
                Phase::Settle => {
                    let take = (self.left as usize).min(rest.len());
                    self.left -= take as u64;
                    i += take;
                    self.fed += take as u64;
                    if self.left == 0 {
                        self.phase = Phase::Survey;
                        self.left = (self.settings.survey_s * self.rate) as u64;
                    }
                }
                Phase::Survey => {
                    let take = (self.left as usize).min(rest.len());
                    if let Some(a) = &mut self.analyser {
                        a.push(&rest[..take]);
                    }
                    self.left -= take as u64;
                    i += take;
                    self.fed += take as u64;
                    if self.left == 0 {
                        self.survey_done();
                        if let Some(r) = self.next_candidate() {
                            return r;
                        }
                    }
                }
                Phase::Listen => {
                    // Small steps, so the receiver is judged between pieces as often as it reports.
                    let step = ((0.05 * self.rate) as usize).max(1);
                    let take = step.min(rest.len());
                    if let Some(rx) = &mut self.rx {
                        rx.feed(&rest[..take], out);
                    }
                    self.heard += take as u64;
                    i += take;
                    self.fed += take as u64;
                    if self.judge() {
                        if let Some(r) = self.next_candidate() {
                            return r;
                        }
                    }
                }
            }
        }
        ScanRequest::Continue
    }

    fn survey_done(&mut self) {
        let centre = self.centre_hz;
        let mut cands = match &self.analyser {
            Some(a) if a.frames() > 0 => find_signals(
                a.power(),
                self.rate,
                centre,
                &self.band,
                self.settings.min_over_db,
                |f| self.fits(centre, f),
            ),
            _ => Vec::new(),
        };
        // Listen in frequency order; the strongest-first order only decided which channel a run was given.
        cands.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        // A channel already listed (seen from another centre) is not visited again.
        cands.retain(|c| {
            !self
                .stations
                .iter()
                .any(|s| (s.freq_hz - c.freq_hz).abs() < 1.0)
        });
        self.results.results_mut().log(format!(
            "centre {}: {} signals",
            freq_text(centre),
            cands.len()
        ));
        self.candidates = cands;
        self.cand_idx = 0;
        self.rx = None;
    }

    /// Starts the next candidate, or moves on to the next centre (returned).
    fn next_candidate(&mut self) -> Option<ScanRequest> {
        if self.rx.is_some() {
            self.cand_idx += 1;
            self.rx = None;
        }
        while self.cand_idx < self.candidates.len() {
            let f = self.candidates[self.cand_idx].freq_hz;
            let Ok(mut rx) =
                (self.desc.make_receiver)(&StackConfig::new(self.stack_settings.clone()))
            else {
                self.cand_idx += 1;
                continue;
            };
            let mut input = InputInfo::new(self.rate, self.centre_hz, f);
            input.live = self.live;
            if rx.configure(&input).is_ok() {
                self.rx = Some(rx);
                self.heard = 0;
                self.locked_at = None;
                self.phase = Phase::Listen;
                self.progress();
                return None;
            }
            self.cand_idx += 1;
        }
        Some(self.next_centre())
    }

    /// Looks at the listening receiver: true when the candidate is finished (a station listed or given up).
    fn judge(&mut self) -> bool {
        let Some(rx) = &self.rx else { return true };
        let t = self.heard as f64 / self.rate;
        let r = Owner::results(rx.as_ref());
        let lock_state = r.status().lock_state;
        let p = self.profile;
        let state = match lock_state {
            LockState::Locked => {
                let at = *self.locked_at.get_or_insert(self.heard);
                let label = label_of(self.band.stack, r);
                if !label.is_empty() || (self.heard - at) as f64 / self.rate >= p.label_s {
                    Some(StationState::Decoded)
                } else {
                    None
                }
            }
            LockState::Tracking if t >= p.sync_s => Some(StationState::Found),
            LockState::Tracking => None,
            _ if t >= p.search_s => {
                self.results.results_mut().log(format!(
                    "{}: nothing decodable",
                    freq_text(self.candidates[self.cand_idx].freq_hz)
                ));
                return true;
            }
            _ => None,
        };
        let Some(state) = state else { return false };
        let c = self.candidates[self.cand_idx];
        let mut label = label_of(self.band.stack, r);
        if self.band.stack == "cquam" {
            label = if state == StationState::Decoded {
                "AM stereo".into()
            } else {
                "AM mono".into()
            };
        }
        let s = Station {
            freq_hz: c.freq_hz,
            state,
            label,
            summary: r.status().summary.clone(),
            snr_db: r.status().snr_db,
            level_dbfs: r.status().level_dbfs,
            over_floor_db: c.over_db,
            at_s: self.fed as f64 / self.rate,
        };
        self.results.results_mut().log(format!(
            "{}: {}{}",
            freq_text(s.freq_hz),
            match s.state {
                StationState::Decoded => "decoded",
                StationState::Found => "signal found, not decoded",
            },
            if s.label.is_empty() {
                String::new()
            } else {
                format!(", {}", s.label)
            }
        ));
        let key = format!("{:.0}", s.freq_hz);
        STATIONS.put(
            &mut self.results,
            &key,
            &StationRow {
                freq_hz: Hertz(s.freq_hz),
                state: s.state,
                name: &s.label,
                snr_db: s.snr_db.map(f64::from),
                level_dbfs: s.level_dbfs.map(f64::from),
                over_floor_db: s.over_floor_db,
            },
        );
        self.stations.push(s);
        true
    }
}

/// An empty result set of a scan, as the description of the family declares it.
fn published() -> Published {
    Published::from_decl(Arc::new(crate::typed::scan_shape().results_decl()))
}

/// A frequency as the screen shows it: kHz below 30 MHz, MHz above.
pub fn freq_text(hz: f64) -> String {
    if hz < 30e6 {
        format!("{:.0} kHz", hz / 1e3)
    } else {
        format!("{:.1} MHz", hz / 1e6)
    }
}

/// The station's name in a receiver's results: the DRM service labels, the HD Radio call sign (or long name), the CDR network
/// name.
pub fn label_of(stack: &str, r: &Results) -> String {
    let text = |c: &Column<field::Text>| {
        r.fields()
            .at(c)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    match stack {
        "drm" => {
            let table = r
                .decl()
                .table_id(crate::drm::results::SERVICES.key)
                .and_then(|id| r.table(id));
            let mut names: Vec<String> = table
                .map(|t| {
                    t.rows_newest_first()
                        .filter_map(|(_, row)| row.at(&crate::drm::results::DrmServiceRow::LABEL))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            names.dedup();
            names.join(" / ")
        }
        "hdradio" => {
            let c = text(&crate::hd::results::HdradioFields::CALL_SIGN);
            if c.is_empty() {
                text(&crate::hd::results::HdradioFields::LONG_NAME)
            } else {
                c
            }
        }
        "cdr" => {
            let n = text(&crate::cdr::results::CdrFields::NETWORK);
            match n.find(" (") {
                Some(i) => n[..i].to_string(),
                None => n,
            }
        }
        _ => String::new(),
    }
}
