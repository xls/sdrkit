//! Meteor scatter (`mscatter`): echoes of a CW beacon counted and measured. The graph is the standard resampler to
//! 12 kHz, the echo detector on the shared spectrum of that stream, and the ping table.

use crate::calendar;
use crate::family::{lock_state_of, made, status, write_series, REPORT_S};
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::dsp::Resample;
use sdrkit_core::publish::keys::Family;
use crate::typed::{
    MscatterFields, MscatterPingEvent, MscatterPingRow, MscatterShowerRow, MSCATTER_AROUND,
    MSCATTER_DAILY, MSCATTER_HOURLY, MSCATTER_PINGS, MSCATTER_SHOWERS, MS_PING,
};
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Event,
    GenConfig, Generator, Hz, Receiver, Record, Results, ResultsDecl, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status,
    Value, Values,
};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, SpectrumSpec, StackGraph, Timed,
};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use std::fmt::Write as _;

/// FFT size of the spectrogram.
pub const NFFT: usize = 2048;
/// Hop.
pub const HOP: usize = 256;
/// The rate the detector works at.
pub const WORK: f64 = 12_000.0;

/// The spectrogram the detector reads: 2048 bins with a Hann window, a frame every 256 samples at 12 kHz.
pub const SPECTROGRAM: SpectrumSpec = SpectrumSpec {
    size: sdrkit_graph::SizeRule::Bins(SpectrumSize::S2048),
    window: SpectrumWindow::Hann,
    average: 1,
    frames: sdrkit_graph::FrameRule::PerSecond(WORK / HOP as f64),
    scale: sdrkit_graph::components::SpectrumScale::Db,
};

/// One meteor echo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ping {
    /// Number, from 1.
    pub id: u64,
    /// Signal time of the start (centre of the first frame), s.
    pub start_s: f64,
    /// UTC of the start, Unix s.
    pub utc: f64,
    /// Duration, s.
    pub duration_s: f64,
    /// Peak SNR over the background, dB.
    pub peak_snr_db: f64,
    /// Doppler at the peak relative to the beacon, Hz.
    pub doppler_hz: f64,
    /// Drift, Hz/s (None with fewer than 4 frames).
    pub drift_hz_s: Option<f64>,
    /// Overdense (long, flat) rather than underdense.
    pub overdense: bool,
    /// Level profile (dB), at most 32 points.
    pub profile: Vec<f32>,
    /// Beacon frequency, absolute Hz (0 unknown).
    pub beacon_hz: f64,
}

impl Item for Ping {
    const KIND: &'static str = "ms.ping";
}

sdrkit_core::choice_enum! {
    /// The kind of a meteor trail.
    pub enum PingKind {
        /// A short echo that fades at once.
        Underdense = ("underdense", "Underdense"),
        /// A long, flat echo.
        Overdense = ("overdense", "Overdense"),
    }
}

impl Ping {
    /// Underdense or overdense.
    pub fn kind(&self) -> PingKind {
        if self.overdense {
            PingKind::Overdense
        } else {
            PingKind::Underdense
        }
    }
    /// The CSV line (`id,utc,duration_s,peak_snr_db,doppler_hz,drift_hz_per_s,type,beacon_hz`).
    pub fn csv(&self) -> String {
        format!(
            "{},{},{:.2},{:.1},{:.1},{},{},{:.0}",
            self.id,
            calendar::iso(self.utc.floor() as i64),
            self.duration_s,
            self.peak_snr_db,
            self.doppler_hz,
            self.drift_hz_s
                .map(|d| format!("{d:+.1}"))
                .unwrap_or_default(),
            PingKind::CHOICES[usize::from(self.kind().index())].key,
            self.beacon_hz
        )
    }
}

/// The CSV header.
pub const CSV_HEADER: &str =
    "id,utc,duration_s,peak_snr_db,doppler_hz,drift_hz_per_s,type,beacon_hz";

/// What the detector measures each check.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectorStatus {
    /// Beacon offset from the centre, Hz.
    pub beacon_hz: Option<f64>,
    /// Beacon SNR per bin, dB.
    pub beacon_snr_db: f64,
    /// Beacon dBFS.
    pub beacon_dbfs: f64,
    /// Noise dBFS per bin.
    pub noise_dbfs: f64,
    /// A ping in progress and its level.
    pub ping_now: Option<f64>,
    /// Bursts rejected.
    pub rejected: u64,
    /// The long-term spectrum around the beacon (86 columns, dB over the noise).
    pub around: Vec<f32>,
}

impl Item for DetectorStatus {
    const KIND: &'static str = "ms.status";
}

struct Track {
    start_frame: u64,
    last_bin: usize,
    frames: Vec<(u64, f64, f64)>, // frame index, level dB, doppler Hz
    below_since: Option<u64>,
}

/// The echo detector (`mscatter.detector`: spectrum to `ms.ping` and its status).
pub struct Detector {
    fw: f64,
    fps: f64,
    s: Vec<f32>,
    b: Vec<f32>,
    n: u64,
    noise: f32,
    beacon: Option<usize>,
    challenger: Option<(usize, u32)>,
    weak: u32,
    threshold_db: f64,
    track: Option<Track>,
    pending: Option<(u64, usize, f64)>,
    in_burst: bool,
    rejected: u64,
    next_id: u64,
    scratch: Vec<f32>,
    status: DetectorStatus,
    centre: f64,
    start_setting: f64,
    clock: Clock,
    start_utc: f64,
    frame_buf: Vec<f32>,
}

impl Default for Detector {
    fn default() -> Self {
        Detector {
            fw: 12000.0,
            fps: 12000.0 / HOP as f64,
            s: vec![0.0; NFFT],
            b: vec![0.0; NFFT],
            n: 0,
            noise: 0.0,
            beacon: None,
            challenger: None,
            weak: 0,
            threshold_db: 13.0,
            track: None,
            pending: None,
            in_burst: false,
            rejected: 0,
            next_id: 1,
            scratch: Vec::new(),
            status: DetectorStatus::default(),
            centre: 0.0,
            start_setting: START_UTC.default,
            clock: Clock::fixed(0.0),
            start_utc: 0.0,
            frame_buf: Vec::new(),
        }
    }
}

// Index loops compare each bin with its own background, the way the detector is described.
#[allow(clippy::needless_range_loop)]
impl Detector {
    fn reset(&mut self) {
        self.s.iter_mut().for_each(|v| *v = 0.0);
        self.b.iter_mut().for_each(|v| *v = 0.0);
        self.n = 0;
        self.noise = 0.0;
        self.beacon = None;
        self.challenger = None;
        self.weak = 0;
        self.track = None;
        self.pending = None;
        self.in_burst = false;
    }

    fn df(&self) -> f64 {
        self.fw / NFFT as f64
    }

    fn bin_hz(&self, bin: f64) -> f64 {
        (bin - (NFFT / 2) as f64) * self.df()
    }

    fn refine(p: &[f32], i: usize) -> f64 {
        if i == 0 || i + 1 >= p.len() {
            return i as f64;
        }
        let (a, b, c) = (p[i - 1] as f64, p[i] as f64, p[i + 1] as f64);
        let den = a - 2.0 * b + c;
        if den.abs() < 1e-30 {
            i as f64
        } else {
            i as f64 + (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        }
    }

    fn frame(&mut self, p: &[f32], pings: &mut Vec<Timed<Ping>>) {
        let n = NFFT;
        self.n += 1;
        let w = (1.0 / self.n as f64).max(1.0 / (2.0 * self.fps)) as f32;
        for (s, &v) in self.s.iter_mut().zip(p) {
            *s += w * (v - *s);
        }
        if self.n % 32 == 1 || self.noise == 0.0 {
            self.noise = percentile(&self.s, 0.5, &mut self.scratch).max(1e-30);
        }
        let warm = self.n as f64 / self.fps >= 5.0;
        let lim = ((0.35 * self.fw / self.df()) as usize).min(n / 2 - 8);
        let (lo, hi) = (n / 2 - lim, n / 2 + lim);
        // Beacon follower, every 4 frames.
        if warm && self.n.is_multiple_of(4) {
            let (mut best, mut bi) = (0f32, n / 2);
            for (i, &v) in self.s.iter().enumerate().take(hi + 1).skip(lo) {
                if v > best {
                    best = v;
                    bi = i;
                }
            }
            match self.beacon {
                None => {
                    if best > 16.0 * self.noise {
                        self.beacon = Some(bi);
                        self.weak = 0;
                    }
                }
                Some(b) => {
                    let cur = self.s[b.clamp(2, n - 3) - 2..=b.clamp(2, n - 3) + 2]
                        .iter()
                        .cloned()
                        .fold(0f32, f32::max);
                    if bi.abs_diff(b) >= 2 && best > 1.4 * cur {
                        let c = match self.challenger {
                            Some((cb, k)) if cb.abs_diff(bi) <= 1 => (bi, k + 1),
                            _ => (bi, 1),
                        };
                        self.challenger = Some(c);
                        if c.1 > 20 {
                            self.beacon = Some(bi);
                            self.challenger = None;
                        }
                    } else {
                        self.challenger = None;
                        // Follow the peak within ±2 bins.
                        let a = b.saturating_sub(2);
                        let e = (b + 2).min(n - 1);
                        let mut m = b;
                        for i in a..=e {
                            if self.s[i] > self.s[m] {
                                m = i;
                            }
                        }
                        self.beacon = Some(m);
                    }
                    if cur < 3.0 * self.noise && self.track.is_none() {
                        self.weak += 1;
                        if self.weak > 40 {
                            self.beacon = None;
                        }
                    } else {
                        self.weak = 0;
                    }
                }
            }
            if let Some(b) = self.beacon {
                let f = Self::refine(&self.s, b);
                self.status.beacon_hz = Some(self.bin_hz(f));
                let a = b.saturating_sub(3);
                let e = (b + 3).min(n - 1);
                let pw: f64 = self.s[a..=e].iter().map(|&v| v as f64).sum::<f64>() / 1.5;
                self.status.beacon_snr_db = db(pw / self.noise as f64);
                self.status.beacon_dbfs = db(pw);
            } else {
                self.status.beacon_hz = None;
            }
            // The noise in one bin of the Hann window, whose noise width is 1.5 bins.
            self.status.noise_dbfs = db(self.noise as f64 / 1.5);
        }
        let Some(b) = self.beacon.filter(|_| warm) else {
            // Warm-up or no beacon: the background still learns.
            self.learn(p, None);
            return;
        };
        // Echo search within ±500 Hz, outside ±3 bins.
        let span = (500.0 / self.df()) as usize;
        let (a, e) = (b.saturating_sub(span).max(1), (b + span).min(n - 2));
        let mut cand = (0f32, b);
        for i in a..=e {
            if i.abs_diff(b) <= 3 {
                continue;
            }
            let r = p[i] / self.b[i].max(1e-30);
            if r > cand.0 {
                cand = (r, i);
            }
        }
        let thr10 = 10f32;
        let mut others = 0;
        if cand.0 > thr10 {
            for i in a..=e {
                if i.abs_diff(b) <= 3 || i.abs_diff(cand.1) <= 5 {
                    continue;
                }
                if p[i] / self.b[i].max(1e-30) > thr10 {
                    others += 1;
                }
            }
        }
        let burst = cand.0 > thr10 && others > 8;
        if burst {
            if !self.in_burst {
                self.rejected += 1;
            }
            self.in_burst = true;
            self.status.rejected = self.rejected;
            return; // not counted, background paused
        }
        self.in_burst = false;
        let level = db(cand.0 as f64);
        let thr = self.threshold_db;
        let dop =
            |s: &Self, i: usize| s.bin_hz(Self::refine(p, i)) - s.bin_hz(Self::refine(&s.s, b));
        match self.track.take() {
            None => {
                if level >= thr {
                    match self.pending {
                        Some((pf, pb, d0)) if pf + 1 == self.n && pb.abs_diff(cand.1) <= 3 => {
                            let d = dop(self, cand.1);
                            self.track = Some(Track {
                                start_frame: pf,
                                last_bin: cand.1,
                                frames: vec![(pf, d0, 0.0), (self.n, level, d)],
                                below_since: None,
                            });
                            // fix the first frame's doppler
                            if let Some(t) = self.track.as_mut() {
                                t.frames[0].2 = d;
                            }
                            self.pending = None;
                        }
                        _ => self.pending = Some((self.n, cand.1, level)),
                    }
                } else {
                    self.pending = None;
                }
                self.status.ping_now = None;
            }
            Some(mut t) => {
                // Track within ±3 bins of the last position.
                let lb = t.last_bin;
                let (ta, te) = (lb.saturating_sub(3), (lb + 3).min(n - 1));
                let mut best = (0f32, lb);
                for i in ta..=te {
                    if i.abs_diff(b) <= 3 {
                        continue;
                    }
                    let r = p[i] / self.b[i].max(1e-30);
                    if r > best.0 {
                        best = (r, i);
                    }
                }
                let lv = db(best.0 as f64);
                let d = dop(self, best.1);
                let fps = self.fps;
                t.frames.push((self.n, lv, d));
                if lv >= thr - 6.0 {
                    t.last_bin = best.1;
                    t.below_since = None;
                } else if t.below_since.is_none() {
                    t.below_since = Some(self.n);
                }
                self.status.ping_now = Some(lv);
                let ended = t
                    .below_since
                    .is_some_and(|s| (self.n - s) as f64 / fps >= 0.26)
                    || (self.n - t.start_frame) as f64 / fps >= 30.0;
                if ended {
                    if let Some(pg) = self.finish(t) {
                        pings.push(Timed {
                            time: pg.start_s,
                            value: pg,
                        });
                    }
                    self.status.ping_now = None;
                } else {
                    self.track = Some(t);
                }
            }
        }
        self.learn(p, Some(b));
        // The long-term spectrum: ±500 Hz around the beacon in 86 columns.
        let cols = 86usize;
        self.status.around.clear();
        for c in 0..cols {
            let i = (b as f64 - span as f64 + c as f64 * 2.0 * span as f64 / (cols - 1) as f64)
                .round() as usize;
            let v = db((self.s[i.clamp(0, n - 1)] / self.noise) as f64) as f32;
            self.status.around.push(v);
        }
    }

    fn learn(&mut self, p: &[f32], beacon: Option<usize>) {
        let alpha = (1.0 / self.n as f64).max(1.0 / 376.0) as f32;
        let frozen = self
            .track
            .as_ref()
            .map(|t| t.last_bin)
            .or(self.pending.map(|x| x.1));
        let _ = beacon;
        for (i, (bv, &v)) in self.b.iter_mut().zip(p).enumerate() {
            if frozen.is_some_and(|f| f.abs_diff(i) <= 8) {
                continue;
            }
            let x = if *bv > 0.0 { v.min(4.0 * *bv) } else { v };
            *bv += alpha * (x - *bv);
        }
    }

    fn finish(&mut self, t: Track) -> Option<Ping> {
        let thr_end = self.threshold_db - 6.0;
        let above: Vec<&(u64, f64, f64)> = t.frames.iter().filter(|f| f.1 >= thr_end).collect();
        if t.frames.len() < 3 {
            return None;
        }
        let last_above = above.last()?.0;
        let duration = (last_above - t.start_frame + 1) as f64 / self.fps;
        let peak = t
            .frames
            .iter()
            .cloned()
            .fold((0u64, f64::MIN, 0.0), |a, f| if f.1 > a.1 { f } else { a });
        let drift = if above.len() >= 4 {
            let xs: Vec<f64> = above.iter().map(|f| f.0 as f64 / self.fps).collect();
            let ys: Vec<f64> = above.iter().map(|f| f.2).collect();
            let mx = xs.iter().sum::<f64>() / xs.len() as f64;
            let my = ys.iter().sum::<f64>() / ys.len() as f64;
            let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
            let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
            (sxx > 0.0).then(|| sxy / sxx)
        } else {
            None
        };
        let near = t
            .frames
            .iter()
            .filter(|f| f.0 <= last_above && f.1 >= peak.1 - 8.0)
            .count();
        let span_frames = t.frames.iter().filter(|f| f.0 <= last_above).count();
        let overdense = duration >= 1.5 && 2 * near >= span_frames;
        let levels: Vec<f32> = t
            .frames
            .iter()
            .filter(|f| f.0 <= last_above)
            .map(|f| f.1 as f32)
            .collect();
        let slices = levels.len().min(32);
        let profile = (0..slices)
            .map(|k| {
                let a = k * levels.len() / slices;
                let b = ((k + 1) * levels.len() / slices).max(a + 1);
                levels[a..b].iter().cloned().fold(f32::MIN, f32::max)
            })
            .collect();
        let start = (t.start_frame - 1) as f64 / self.fps + NFFT as f64 / 2.0 / self.fw;
        let id = self.next_id;
        self.next_id += 1;
        Some(Ping {
            id,
            start_s: start,
            utc: self.start_utc + start,
            duration_s: duration,
            peak_snr_db: peak.1,
            doppler_hz: peak.2,
            drift_hz_s: drift,
            overdense,
            profile,
            beacon_hz: self
                .status
                .beacon_hz
                .map(|b| self.centre + b)
                .filter(|_| self.centre != 0.0)
                .unwrap_or(0.0),
        })
    }
}

/// The detection level over the background.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Detection level", "dB", 13.0).range(6.0, 30.0, 0.5);
/// The UTC of sample 0 of a file, 0 for the host clock.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc",
    "UTC of sample 0 (files; 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0);
/// The beacon's offset in the test signal.
pub const GEN_BEACON_HZ: Setting<field::Hz> =
    Setting::new("gen_beacon_hz", "Test signal: beacon offset", "Hz", 420.0)
        .range(-4000.0, 4000.0, 1.0)
        .section("Test signal")
        .advanced();
/// The mean gap between the pings of the test signal.
pub const GEN_MEAN_GAP_S: Setting<field::Duration> = Setting::new(
    "gen_mean_gap_s",
    "Test signal: mean gap between pings",
    "s",
    3.0,
)
.range(0.0, 600.0, 0.5)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of [`Detector`] and [`PingTable`].
    pub enum DetectorSetting {
        /// The detection level.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the meteor-scatter stack.
    pub enum MscatterSetting {
        /// The detection level.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The beacon's offset in the test signal.
        GenBeaconHz = GEN_BEACON_HZ,
        /// The mean gap between the pings of the test signal.
        GenMeanGapS = GEN_MEAN_GAP_S,
    }
}

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

impl Component for Detector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mscatter.detector",
            vec![Port::spectrum("spectrum", SPECTROGRAM)],
            vec![
                Port::typed::<Ping>("pings").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DetectorStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
        .with_settings(DetectorSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fps = inputs[0].rate;
        self.fw = inputs[0].bandwidth_hz;
        self.centre = inputs[0].centre_hz;
        self.start_utc = start_utc(self.start_setting, &self.clock);
        self.reset();
        Ok(vec![
            Format::new(Kind::typed::<Ping>(), 0.0),
            Format::new(Kind::typed::<DetectorStatus>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre = centre_hz;
                self.reset();
            }
        }
        let fr = io.spectrum(0);
        let pings = io.items_out::<Ping>(0);
        let mut f = std::mem::take(&mut self.frame_buf);
        for k in 0..fr.len() {
            // The frames are dBFS: the detector works on power.
            f.clear();
            f.extend(fr.frame(k).iter().map(|&d| 10f32.powf(d / 10.0)));
            self.frame(&f, pings);
        }
        self.frame_buf = f;
        if io.tick() {
            let mut st = self.status.clone();
            st.rejected = self.rejected;
            io.items_out::<DetectorStatus>(1).push(Timed {
                time: self.n as f64 / self.fps,
                value: st,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DetectorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DetectorSetting::Threshold => self.threshold_db = THRESHOLD.read(id, v)?,
            DetectorSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, v)?;
                self.start_utc = start_utc(self.start_setting, &self.clock);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DetectorSetting::Threshold, Value::F64(self.threshold_db)),
            (DetectorSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
}

/// A meteor shower of the IMO working list.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Shower {
    /// Name.
    pub name: &'static str,
    /// Peak (month, day).
    pub peak: (u32, u32),
    /// Active from.
    pub from: (u32, u32),
    /// Active to.
    pub to: (u32, u32),
    /// Zenithal hourly rate.
    pub zhr: u32,
    /// Daytime (strong for radio, invisible to the eye).
    pub daytime: bool,
}

/// The showers (approximate dates).
pub const SHOWERS: &[Shower] = &[
    Shower {
        name: "Quadrantids",
        peak: (1, 4),
        from: (12, 28),
        to: (1, 12),
        zhr: 110,
        daytime: false,
    },
    Shower {
        name: "Lyrids",
        peak: (4, 22),
        from: (4, 14),
        to: (4, 30),
        zhr: 18,
        daytime: false,
    },
    Shower {
        name: "Eta Aquariids",
        peak: (5, 6),
        from: (4, 19),
        to: (5, 28),
        zhr: 50,
        daytime: false,
    },
    Shower {
        name: "Arietids",
        peak: (6, 7),
        from: (5, 22),
        to: (7, 2),
        zhr: 30,
        daytime: true,
    },
    Shower {
        name: "Zeta Perseids",
        peak: (6, 9),
        from: (5, 20),
        to: (7, 5),
        zhr: 40,
        daytime: true,
    },
    Shower {
        name: "Beta Taurids",
        peak: (6, 28),
        from: (6, 5),
        to: (7, 17),
        zhr: 25,
        daytime: true,
    },
    Shower {
        name: "Southern Delta Aquariids",
        peak: (7, 30),
        from: (7, 12),
        to: (8, 23),
        zhr: 25,
        daytime: false,
    },
    Shower {
        name: "Perseids",
        peak: (8, 12),
        from: (7, 17),
        to: (8, 24),
        zhr: 100,
        daytime: false,
    },
    Shower {
        name: "Daytime Sextantids",
        peak: (9, 27),
        from: (9, 9),
        to: (10, 9),
        zhr: 5,
        daytime: true,
    },
    Shower {
        name: "Orionids",
        peak: (10, 21),
        from: (10, 2),
        to: (11, 7),
        zhr: 20,
        daytime: false,
    },
    Shower {
        name: "Southern Taurids",
        peak: (11, 5),
        from: (9, 28),
        to: (12, 2),
        zhr: 5,
        daytime: false,
    },
    Shower {
        name: "Northern Taurids",
        peak: (11, 12),
        from: (10, 20),
        to: (12, 10),
        zhr: 5,
        daytime: false,
    },
    Shower {
        name: "Leonids",
        peak: (11, 17),
        from: (11, 6),
        to: (11, 30),
        zhr: 15,
        daytime: false,
    },
    Shower {
        name: "Geminids",
        peak: (12, 14),
        from: (12, 4),
        to: (12, 20),
        zhr: 150,
        daytime: false,
    },
    Shower {
        name: "Ursids",
        peak: (12, 22),
        from: (12, 17),
        to: (12, 26),
        zhr: 10,
        daytime: false,
    },
];

/// The day count since 1970-01-01 of a date, or 0 for a date out of range.
fn day_number(y: i64, m: u32, d: u32) -> i64 {
    calendar::unix(y, m, d, 0, 0, 0).map_or(0, |u| u.div_euclid(86_400))
}

/// The showers active on a day or peaking within 45 days, nearest peak first: (shower, days to the peak, active).
pub fn showers_on(unix: i64) -> Vec<(Shower, i64, bool)> {
    let c = calendar::civil(unix);
    let day = day_number(c.year, c.month, c.day);
    let wrap = |d: i64| {
        let mut d = d.rem_euclid(365);
        if d > 182 {
            d -= 365;
        }
        d
    };
    let doy = |m: u32, d: u32| day_number(c.year, m, d);
    let mut out: Vec<(Shower, i64, bool)> = SHOWERS
        .iter()
        .filter_map(|s| {
            let to_peak = wrap(doy(s.peak.0, s.peak.1) - day);
            let a = wrap(doy(s.from.0, s.from.1) - day);
            let b = wrap(doy(s.to.0, s.to.1) - day);
            let active = a <= 0 && b >= 0;
            (active || (0..=45).contains(&to_peak)).then_some((*s, to_peak, active))
        })
        .collect();
    out.sort_by_key(|x| x.1.abs());
    out
}

/// Counts per UTC hour (last 24) and day (last 30).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Counts {
    /// Hour bins, the current last.
    pub hourly: Vec<u32>,
    /// Start of the first hour bin, Unix s.
    pub hour_start: i64,
    /// Day bins, today last.
    pub daily: Vec<u32>,
    /// Start of the first day bin.
    pub day_start: i64,
    /// Pings in the last 60 minutes.
    pub rate: u32,
}

/// Counts of pings (UTC times) as of `now`.
pub fn counts(utcs: &[f64], now: f64) -> Counts {
    let hour_now = (now / 3600.0).floor() as i64;
    let day_now = (now / 86400.0).floor() as i64;
    let mut c = Counts {
        hourly: vec![0; 24],
        hour_start: (hour_now - 23) * 3600,
        daily: vec![0; 30],
        day_start: (day_now - 29) * 86400,
        rate: 0,
    };
    for &u in utcs {
        let h = (u / 3600.0).floor() as i64;
        let d = (u / 86400.0).floor() as i64;
        if (0..24).contains(&(hour_now - h)) {
            c.hourly[(23 - (hour_now - h)) as usize] += 1;
        }
        if (0..30).contains(&(day_now - d)) {
            c.daily[(29 - (day_now - d)) as usize] += 1;
        }
        if now - u <= 3600.0 && u <= now {
            c.rate += 1;
        }
    }
    c
}

impl Shift for Ping {
    fn shift_times(&mut self, by_s: f64) {
        self.start_s += by_s;
    }
}

/// Pings held.
pub const MAX_PINGS: usize = 500;

/// What `mscatter` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("mscatter").results_decl()
}

impl Shaped for Ping {
    type Row = MscatterPingRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MscatterPingRow {
            id: self.id as i64,
            utc: Time(self.utc),
            duration: Seconds(self.duration_s),
            peak_snr_db: self.peak_snr_db,
            doppler: Hertz(self.doppler_hz),
            drift_hz_s: self.drift_hz_s,
            kind: self.kind(),
        });
    }
}

sdrkit_core::commands! {
    /// The commands of [`PingTable`].
    pub enum PingTableCommand {
        /// Forgets every ping.
        Clear = objcmd::CLEAR,
    }
}

/// What the ping table keeps: the pings and the UTC of every ping of the last 31 days (for the counts).
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The pings, oldest first.
    pub pings: LogRing<Ping>,
    /// UTC of the pings of the last 31 days.
    pub utcs: Vec<f64>,
    /// The number of the next ping.
    pub next_id: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            pings: LogRing::new(MAX_PINGS),
            utcs: Vec::new(),
            next_id: 1,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.pings.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.mscatter_pings", 1);
    fn clear(&mut self) {
        self.pings.clear();
        self.utcs.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.pings.adopt(saved.pings);
        self.utcs = saved.utcs;
        self.next_id = saved.next_id.max(1);
    }
}

/// The ping table (`mscatter.counts`): numbers the pings, counts them per hour and day, names the showers and publishes.
pub struct PingTable {
    store: Store,
    status: DetectorStatus,
    threshold: f64,
    t: f64,
    start_setting: f64,
    clock: Clock,
    start_utc: f64,
    rate: f64,
    published: Published,
    series: [SeriesId; 3],
    shown: Status,
    shower_day: i64,
    events: Vec<Event>,
}

impl Default for PingTable {
    fn default() -> Self {
        let decl = std::sync::Arc::new(declaration());
        let series = [MSCATTER_HOURLY.key, MSCATTER_DAILY.key, MSCATTER_AROUND.key]
            .map(|k| decl.series_id(k).expect("the declaration holds the series"));
        PingTable {
            store: Store::default(),
            status: DetectorStatus::default(),
            threshold: THRESHOLD.default,
            t: 0.0,
            start_setting: START_UTC.default,
            clock: Clock::fixed(0.0),
            start_utc: 0.0,
            rate: 0.0,
            published: Published::from_decl(decl),
            series,
            shown: Status::default(),
            shower_day: i64::MIN,
            events: Vec::new(),
        }
    }
}

impl PingTable {
    /// What the component keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn publish(&mut self) {
        let now = self.start_utc + self.t;
        let n = self.store.pings.len();
        let state = if n > 0 {
            2
        } else if self.status.beacon_hz.is_some() {
            1
        } else {
            0
        };
        let c = counts(&self.store.utcs, now);
        let beacon = self.status.beacon_hz;
        status(
            &mut self.shown,
            lock_state_of(state),
            Some(self.status.beacon_snr_db as f32),
            Some(self.status.beacon_dbfs as f32),
            beacon,
            |s| {
                let _ = if beacon.is_none() && n == 0 {
                    write!(s, "Meteor scatter: no beacon in the capture")
                } else {
                    write!(s, "Meteor scatter: {n} pings, {} per hour", c.rate)
                };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        p.put(&MscatterFields {
            pings: n as i64,
            rate: i64::from(c.rate),
            rejected: self.status.rejected as i64,
            beacon: self.status.beacon_hz.map(Hertz),
            beacon_snr_db: self.status.beacon_snr_db,
            noise_dbfs: self.status.noise_dbfs,
            threshold_db: self.threshold,
            ping_now_db: self.status.ping_now,
        });
        let day = (now / 86_400.0).floor() as i64;
        if day != self.shower_day {
            self.shower_day = day;
            p.clear_rows(&MSCATTER_SHOWERS);
            for (s, d, active) in showers_on(now as i64) {
                MSCATTER_SHOWERS.put(
                    p,
                    s.name,
                    &MscatterShowerRow {
                        name: s.name,
                        peak_in_days: d,
                        active,
                        zhr: i64::from(s.zhr),
                    },
                );
            }
        }
        log_rows(&self.store.pings, &MSCATTER_PINGS, self.t, p);
        let r = p.results_mut();
        let [hourly, daily, around] = self.series;
        let at = |(i, &v): (usize, &u32)| [i as f32, v as f32];
        write_series(r, hourly, false, c.hourly.iter().enumerate().map(at));
        write_series(r, daily, false, c.daily.iter().enumerate().map(at));
        write_series(
            r,
            around,
            true,
            self.status
                .around
                .iter()
                .enumerate()
                .map(|(i, &v)| [i as f32, v]),
        );
    }
}

impl Component for PingTable {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mscatter.counts",
            vec![
                Port::iq("in"),
                Port::typed::<Ping>("pings"),
                Port::typed::<DetectorStatus>("status"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DetectorSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.start_utc = start_utc(self.start_setting, &self.clock);
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        self.t = (first + io.input_len(0) as u64) as f64 / self.rate.max(1.0);
        for s in io.items::<DetectorStatus>(2) {
            self.status = s.value.clone();
        }
        let mut any = false;
        for p in io.items::<Ping>(1) {
            let mut p = p.value.clone();
            p.id = self.store.next_id;
            self.store.next_id += 1;
            self.store.utcs.push(p.utc);
            let keep_from = p.utc - 31.0 * 86_400.0;
            self.store.utcs.retain(|&u| u >= keep_from);
            let kind = &PingKind::CHOICES[usize::from(p.kind().index())].label;
            let text = format!(
                "Meteor ping {:.1} s, {:.0} dB, {:+.0} Hz ({kind})",
                p.duration_s, p.peak_snr_db, p.doppler_hz
            );
            self.events.push(MS_PING.event(
                p.start_s,
                p.beacon_hz,
                &MscatterPingEvent {
                    text: &text,
                    family: Family::Other,
                    utc: Time(p.utc),
                    duration: Seconds(p.duration_s),
                    peak_snr_db: p.peak_snr_db,
                    doppler: Hertz(p.doppler_hz),
                    kind: p.kind(),
                },
            ));
            self.published.results_mut().log(text);
            self.store.pings.push(p);
            any = true;
        }
        if io.tick() || any {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DetectorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DetectorSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            DetectorSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, v)?;
                self.start_utc = start_utc(self.start_setting, &self.clock);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DetectorSetting::Threshold, Value::F64(self.threshold)),
            (DetectorSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn commands(&self) -> &Commands {
        PingTableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match PingTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PingTableCommand::Clear => self.store.clear(),
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.t)?;
        self.publish();
        Ok(())
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 24e3;

/// The receiver: the resampler to 12 kHz, the detector on the spectrogram of its output, and the ping table.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let resample = made("resample.rate", Resample::new(WORK))?;
    let graph = GraphBuilder::new()
        .add("resample", resample)
        .add("detector", Detector::default())
        .add("counts", PingTable::default())
        .input("resample")
        .input("counts.in")
        .connect("resample", "detector.spectrum")
        .connect("detector.pings", "counts.pings")
        .connect("detector.status", "counts.status")
        .output("counts.events")
        .clock(REPORT_S)
        .status_from("counts")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("mscatter"), graph)
        .min_rate(
            MIN_RATE,
            "Meteor scatter needs a sample rate of at least 24 ksps",
        )
        .route(
            THRESHOLD.key(),
            &["detector.threshold_db", "counts.threshold_db"],
        )?
        .route(START_UTC.key(), &["detector.start_utc", "counts.start_utc"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("counts", "clear"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// A ping the generator places.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GenPing {
    /// Start, s.
    pub start: f64,
    /// Duration, s.
    pub duration: f64,
    /// Doppler, Hz.
    pub doppler: f64,
    /// The peak level of the echo against the beacon, dB.
    pub peak_dbc: f64,
    /// Overdense (long and flat).
    pub overdense: bool,
}

/// The meteor-scatter generator: the beacon with amplitude 1 and its meteor echoes. The beacon is `beacon_hz` from the station
/// frequency (the tuned frequency).
pub struct MsGenerator {
    rate: f64,
    beacon_hz: f64,
    pings: Vec<GenPing>,
    n: u64,
    ph_b: f64,
    rng: Rng,
    mean_gap: f64,
    next_rand: f64,
}

impl MsGenerator {
    /// A beacon at `beacon_hz` and these pings, plus random ones about every `mean_gap` s when it is above 0.
    pub fn new(
        rate: f64,
        beacon_hz: f64,
        pings: Vec<GenPing>,
        mean_gap: f64,
        seed: u64,
    ) -> MsGenerator {
        MsGenerator {
            rate,
            beacon_hz,
            pings,
            n: 0,
            ph_b: 0.0,
            rng: Rng::new(seed),
            mean_gap,
            next_rand: 4.0,
        }
    }

    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> MsGenerator {
        let s = &cfg.settings;
        MsGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_BEACON_HZ),
            vec![],
            s.get(&GEN_MEAN_GAP_S),
            cfg.seed,
        )
    }

    /// The pings placed so far.
    pub fn placed(&self) -> &[GenPing] {
        &self.pings
    }

    fn echo_amp(p: &GenPing, t: f64) -> f64 {
        let dt = t - p.start;
        if dt < 0.0 || dt > p.duration * 3.0 + 0.5 {
            return 0.0;
        }
        let a_peak = 10f64.powf(p.peak_dbc / 20.0);
        let shape = if p.overdense {
            let rise = (dt / 0.05).min(1.0);
            let fall = if dt > p.duration {
                (-(dt - p.duration) / 0.08).exp()
            } else {
                1.0
            };
            rise * fall
        } else {
            let rise = (dt / 0.02).min(1.0);
            rise * (-dt / (p.duration / 2.3)).exp()
        };
        a_peak * shape
    }
}

impl Generator for MsGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            if self.mean_gap > 0.0 && t >= self.next_rand {
                let over = self.rng.uniform() < 0.15;
                self.pings.push(GenPing {
                    start: t,
                    duration: if over {
                        1.5 + 2.0 * self.rng.uniform()
                    } else {
                        0.2 + 0.5 * self.rng.uniform()
                    },
                    doppler: (self.rng.uniform() - 0.5) * 400.0,
                    peak_dbc: -32.0 + 12.0 * self.rng.uniform(),
                    overdense: over,
                });
                self.next_rand = t + 3.0 + self.mean_gap * self.rng.exponential();
            }
            self.ph_b = (self.ph_b + self.beacon_hz / self.rate).rem_euclid(1.0);
            let mut s = Complex32::from_polar(1.0, (TAU * self.ph_b) as f32);
            for p in &self.pings {
                let a = Self::echo_amp(p, t);
                if a > 0.0 {
                    let ph = TAU * ((self.beacon_hz + p.doppler) * (t - p.start));
                    s += Complex32::from_polar(a as f32, ph as f32);
                }
            }
            *o = s;
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the beacon is its carrier to noise in 100 Hz.
        Some(SignalInfo::new(100.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(MsGenerator::from_config(cfg)))
}

/// The verify case: the beacon with a ping every few seconds.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, beacon and pings", o, 0.0);
    c.seconds = o.seconds.max(30.0);
    c.expect_audio = Some(false);
    c.snr_db = Some(40.0);
    c.check = Some(|r| {
        let n = crate::family::field(r, MscatterFields::PINGS.key);
        vec![CheckLine::new(
            MscatterFields::PINGS.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_showers() {
        let base = calendar::unix(2026, 1, 10, 23, 30, 0).unwrap() as f64;
        let utcs = [
            base,
            base + 60.0,
            base - 3600.0,
            base - 4.0 * 3600.0,
            base - 3.0 * 86400.0,
        ];
        let c = counts(&utcs, base + 600.0);
        assert_eq!(c.hourly[23], 2);
        assert_eq!(c.hourly[22], 1);
        assert_eq!(c.hourly[19], 1);
        assert_eq!(c.hourly.iter().sum::<u32>(), 4);
        assert_eq!(c.daily.iter().sum::<u32>(), 5);
        assert_eq!(
            c.hour_start,
            (((base + 600.0) / 3600.0).floor() as i64 - 23) * 3600
        );
        let p = showers_on(calendar::unix(2026, 8, 13, 0, 0, 0).unwrap());
        assert!(p.iter().any(|s| s.0.name == "Perseids"));
        assert!(showers_on(calendar::unix(2026, 1, 1, 0, 0, 0).unwrap())
            .iter()
            .any(|s| s.0.name == "Quadrantids"));
        assert_eq!(
            showers_on(calendar::unix(2026, 12, 14, 0, 0, 0).unwrap())[0]
                .0
                .name,
            "Geminids"
        );
        let d0 = calendar::civil(calendar::unix(2026, 1, 1, 0, 0, 0).unwrap()).yday;
        assert_eq!(d0, 1);
        assert_eq!(
            calendar::civil(calendar::unix(2026, 1, 1, 0, 0, 0).unwrap() + 59 * 86400).yday,
            60
        );
    }

    #[test]
    fn csv_line() {
        let p = Ping {
            id: 7,
            start_s: 0.0,
            utc: calendar::unix(2026, 1, 1, 1, 1, 1).unwrap() as f64,
            duration_s: 0.75,
            peak_snr_db: 21.3,
            doppler_hz: -88.2,
            drift_hz_s: Some(3.4),
            overdense: true,
            profile: vec![],
            beacon_hz: 143_050_420.0,
        };
        assert_eq!(
            p.csv(),
            "7,2026-01-01T01:01:01Z,0.75,21.3,-88.2,+3.4,overdense,143050420"
        );
        assert!(CSV_HEADER.contains("utc,duration_s"));
    }
}
