//! Broadband burst monitors on a spectrogram of the whole capture: `jove` (Radio Jove: Jupiter and solar bursts near 20 MHz,
//! 31.25 kHz bins, 0.1 s rows, a burst covers half the band or more) and `callisto` (solar radio bursts in 62.5 kHz bins and
//! 0.25 s rows, a burst covers 2 bins or more and its drift rate is measured; optional e-CALLISTO FITS files).
//!
//! Each finished burst is an event row (`science.event`): `frequency_hz` the centroid of its first row (Hz from the centre),
//! `peak_db` its largest mean excess, `duration_s`, `aux1` its drift (Hz/s, 0 with fewer than 4 rows), `aux2` its largest
//! bandwidth (Hz).
//!
//! Documents: NASA Radio JOVE receiver manual (20.1 MHz); e-CALLISTO network pages (45 to 870 MHz, 62.5 kHz resolution,
//! 200 channels a sweep, 4 sweeps a second, files of 900 s) and the ETH CALLISTO Operating Manual (the FITS file, see
//! [`file`]).

pub mod detect;
pub mod file;
pub mod gen;

use crate::events::{EventLog, EventRow, TrendLog};
use crate::typed::{BurstFields, BurstTrendRow, BURST_TREND};
use sdrkit_core::interface::{Hertz, Seconds};
use crate::family::REPORT_S;
use crate::mscatter::START_UTC;
use detect::{fft_size, Burst, BurstConfig, BurstDetector, Spectrogram};
use file::{Bitpix, FileSpec};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::{field, Published, RasterDef};
use sdrkit_core::results::{copy_if_changed, Axis, Raster};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Choice, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState,
    RasterId, Receiver, Record, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, Station, Status, Storage, StorePath, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// What tells the two monitors apart.
#[derive(Debug, PartialEq)]
pub struct Profile {
    /// The stack id.
    pub id: &'static str,
    /// The component name.
    pub component: &'static str,
    /// The bin the transform size aims at, Hz.
    pub bin_hz: f64,
    /// The time of a row, s.
    pub row_s: f64,
    /// A row is active when half the bins are (`true`) or when 2 are.
    pub broadband: bool,
    /// The bursts are typed by their drift.
    pub drifting: bool,
}

/// `jove`.
pub static JOVE_PROFILE: Profile = Profile {
    id: "jove",
    component: "science.jove",
    bin_hz: 31_250.0,
    row_s: 0.1,
    broadband: true,
    drifting: false,
};
/// `callisto`.
pub static CALLISTO_PROFILE: Profile = Profile {
    id: "callisto",
    component: "science.callisto",
    bin_hz: 62_500.0,
    row_s: 0.25,
    broadband: false,
    drifting: true,
};

/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The threshold of a bin when the setting is 0, dB.
pub const DEFAULT_THRESHOLD_DB: f64 = 1.0;
/// Display rows kept.
pub const DISPLAY_KEPT: usize = 150;
/// The limits of a display value, dB.
const DISPLAY_DB: (f32, f32) = (-5.0, 30.0);
/// A burst shorter than this is `short` (`jove`), s.
const SHORT_S: f64 = 1.0;
/// A trend minute is written this long after its end, s.
const TREND_DELAY_S: f64 = 5.0;

/// What no public document confirms.
pub const NOTE_JOVE: &str = "Not confirmed by a public document: every rule of the detector (the 31.25 kHz bins and 0.1 s \
rows, the background learned from 20 quiet rows with a 20 s time constant, a bin active over max(threshold, 5 standard \
deviations), a burst over half the band, closed after 2 quiet rows, short under 1 s). L and S bursts are not classified.";
/// What no public document confirms.
pub const NOTE_CALLISTO: &str = "Not confirmed by a public document: every rule of the detector (the background learned from \
20 quiet rows with a 20 s time constant, a bin active over max(threshold, 5 standard deviations), a burst over 2 bins, closed \
after 2 quiet rows, drifting when 4 rows or more move 2 bins or more). Burst types I to V are not named. The capture has \
no frequency sweep, so a file holds the bins of the capture, not 200 channels. In the file: the digits (dB per digit and the \
level of digit 0 are settings), the 16-bit pixels (the manual's example; 8 bits is a setting), one binary table with TIME in \
seconds of the day (the readers in use; the manual describes two tables), FRQFILE 'NONE', PWM_VAL 0, DATE the date of the \
first row, files of whole multiples of the file length in UTC; the archive gzips the files, the stack does not.";

/// The bin threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Burst threshold per bin", "dB", DEFAULT_THRESHOLD_DB)
        .range(0.0, 20.0, 0.0)
        .zero_means("1 dB")
        .help("The least excess of an active bin over its background: 0.5 to 20 dB, 0 for the default of 1 dB.");
/// Write FITS files.
pub const FITS: Setting<field::Bool> = Setting::new("fits", "Write FITS files", "", false)
    .help("Writes e-CALLISTO FITS files of the spectrogram into the file folder of the host's storage.")
    .section("Files");
/// The folder of the files.
pub const FITS_FOLDER: Setting<field::Text> =
    Setting::new("fits_folder", "FITS folder", "", "callisto")
        .help("A folder of the host's storage.")
        .section("Files");
/// The instrument code.
pub const STATION_CODE: Setting<field::Text> =
    Setting::new("station_code", "Instrument code", "", "LOCAL")
        .help("Letters and digits: the start of the file name, INSTRUME and ORIGIN.")
        .section("Files");
/// The focus code.
pub const FOCUS_CODE: Setting<field::I64> = Setting::new("focus_code", "Focus code", "", 59)
    .range(0.0, 63.0, 1.0)
    .help("The two digits at the end of the file name (the manual lists 00 to 63, default 59).")
    .section("Files");
/// The length of a file.
pub const FILE_S: Setting<field::F64> = Setting::new("file_s", "File length", "s", 900.0)
    .range(10.0, 3600.0, 0.0)
    .help("Files cover whole multiples of this length in UTC (e-CALLISTO: 900 s).")
    .section("Files");
/// The pixel type.
pub const FITS_BITPIX: Setting<field::Choice<Bitpix>> =
    Setting::choice("fits_bitpix", "Pixel type", Bitpix::Sixteen)
        .help("BITPIX of the image: 16 as the manual's example header (values 0 to 255), or 8.")
        .section("Files")
        .advanced();
/// dB per digit.
pub const DB_PER_DIGIT: Setting<field::F64> =
    Setting::new("db_per_digit", "dB per digit", "dB", 0.4)
        .range(0.01, 10.0, 0.0)
        .help("The power step of one digit. Not given by a public document that was read.")
        .section("Files")
        .advanced();
/// The level of digit 0.
pub const ZERO_DBFS: Setting<field::F64> =
    Setting::new("zero_dbfs", "Level of digit 0", "dBFS", -100.0)
        .range(-200.0, 0.0, 0.0)
        .help("The power of digit 0, dB of full scale.")
        .section("Files")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`BurstDecoder`].
    pub enum DecoderSetting {
        /// The bin threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// Write FITS files.
        Fits = FITS,
        /// The file folder.
        FitsFolder = FITS_FOLDER,
        /// The instrument code.
        StationCode = STATION_CODE,
        /// The focus code.
        FocusCode = FOCUS_CODE,
        /// The file length.
        FileS = FILE_S,
        /// The pixel type.
        FitsBitpix = FITS_BITPIX,
        /// dB per digit.
        DbPerDigit = DB_PER_DIGIT,
        /// The level of digit 0.
        ZeroDbfs = ZERO_DBFS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `jove` stack.
    pub enum JoveSetting {
        /// The bin threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The level of the test signal.
        GenLevel = gen::GEN_LEVEL_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `callisto` stack.
    pub enum CallistoSetting {
        /// The bin threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// Write FITS files.
        Fits = FITS,
        /// The file folder.
        FitsFolder = FITS_FOLDER,
        /// The instrument code.
        StationCode = STATION_CODE,
        /// The focus code.
        FocusCode = FOCUS_CODE,
        /// The file length.
        FileS = FILE_S,
        /// The pixel type.
        FitsBitpix = FITS_BITPIX,
        /// dB per digit.
        DbPerDigit = DB_PER_DIGIT,
        /// The level of digit 0.
        ZeroDbfs = ZERO_DBFS,
        /// The level of the test signal.
        GenLevel = gen::GEN_LEVEL_DB,
        /// The drift of the test signal.
        GenDrift = gen::GEN_DRIFT_HZ_S,
    }
}

/// The threshold of a setting value: 0 is the default.
fn threshold_of(v: f64) -> f64 {
    if v > 0.0 {
        v
    } else {
        DEFAULT_THRESHOLD_DB
    }
}

/// A burst as an event row of `p`.
pub fn event_row(p: &Profile, b: &Burst, bin_hz: f64) -> EventRow {
    let drift = b.drift_hz_s.unwrap_or(0.0);
    let (kind, note) = if p.drifting {
        let drifting = b.drift_hz_s.is_some() && drift.abs() * b.duration_s >= 2.0 * bin_hz;
        if drifting {
            ("drifting burst", format!("drift {:.1} kHz/s", drift / 1e3))
        } else {
            ("burst", String::new())
        }
    } else if b.duration_s < SHORT_S {
        ("burst", "short".to_string())
    } else {
        ("burst", "long".to_string())
    };
    EventRow {
        signal_time_s: b.start_s,
        kind: kind.into(),
        frequency_hz: b.centroid_hz,
        peak_db: b.peak_db,
        duration_s: b.duration_s,
        aux1: drift,
        aux2: b.bandwidth_hz,
        note,
    }
}

/// The spectrogram and the detector at one input rate, with the trend, the display rows and the rows of the open file.
pub struct Monitor {
    profile: &'static Profile,
    rate: f64,
    spec: Spectrogram,
    det: BurstDetector,
    bin_hz: f64,
    row_s: f64,
    origin_s: f64,
    rows: u64,
    found: Vec<EventRow>,
    display: VecDeque<Vec<f32>>,
    display_changed: bool,
    // Trend: per minute the sum and count of the levels and the bursts.
    minute_level: Vec<(u64, f64, u64, u32)>,
    first_minute_level: Option<f64>,
    next_minute: u64,
    trend: Vec<(u64, [f64; 2])>,
    keep_rows: bool,
    kept: Vec<(f64, Vec<f32>)>,
    scratch: Vec<f32>,
}

impl Monitor {
    /// A monitor of `profile` for input at `rate` whose first sample is at `origin_s`, with the bin threshold `threshold_db`
    /// (0 for the default). With `keep_rows`, the power rows are kept for a file ([`Monitor::take_rows`]).
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`].
    pub fn new(
        profile: &'static Profile,
        rate: f64,
        threshold_db: f64,
        origin_s: f64,
        keep_rows: bool,
    ) -> Result<Self, ConfigError> {
        if !rate.is_finite() || rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: rate,
            });
        }
        let n = fft_size(rate, profile.bin_hz);
        let frames = ((profile.row_s * rate / n as f64).round() as usize).max(1);
        let bin_hz = rate / n as f64;
        let row_s = (frames * n) as f64 / rate;
        let cfg = BurstConfig {
            bins: n,
            bin_hz,
            f0_hz: -rate / 2.0,
            row_s,
            threshold_db: threshold_of(threshold_db),
            min_active: if profile.broadband { n / 2 } else { 2 },
        };
        Ok(Monitor {
            profile,
            rate,
            spec: Spectrogram::new(n, frames),
            det: BurstDetector::new(cfg),
            bin_hz,
            row_s,
            origin_s,
            rows: 0,
            found: Vec::new(),
            display: VecDeque::with_capacity(DISPLAY_KEPT),
            display_changed: false,
            minute_level: Vec::new(),
            first_minute_level: None,
            next_minute: (origin_s / 60.0).floor().max(0.0) as u64,
            trend: Vec::new(),
            keep_rows,
            kept: Vec::new(),
            scratch: Vec::new(),
        })
    }

    /// The input rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }

    /// The time of a row, s.
    pub fn row_s(&self) -> f64 {
        self.row_s
    }

    /// Bins of a row.
    pub fn bins(&self) -> usize {
        self.spec.bins()
    }

    /// The centre of bin `k`, Hz from the centre of the capture.
    pub fn bin_centre(&self, k: usize) -> f64 {
        -self.rate / 2.0 + (k as f64 + 0.5) * self.bin_hz
    }

    /// Sets the bin threshold (0 for the default).
    pub fn set_threshold(&mut self, db: f64) {
        self.det.set_threshold(threshold_of(db));
    }

    /// Rows made.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Keeps the power rows for a file from the next row on (`true`), or stops keeping them.
    pub fn keep_rows(&mut self, on: bool) {
        self.keep_rows = on;
        if !on {
            self.kept.clear();
        }
    }

    /// The detector.
    pub fn detector(&self) -> &BurstDetector {
        &self.det
    }

    /// Feeds samples.
    pub fn process(&mut self, x: &[Complex32]) {
        let mut pos = 0;
        while self.spec.fill(x, &mut pos) {
            let mut r = std::mem::take(&mut self.scratch);
            r.clear();
            r.extend_from_slice(self.spec.row());
            self.row(&r);
            self.scratch = r;
        }
    }

    fn row(&mut self, row: &[f32]) {
        let t = self.origin_s + self.rows as f64 * self.row_s;
        self.rows += 1;
        if let Some(b) = self.det.push(t, row) {
            let m = (b.start_s / 60.0).floor().max(0.0) as u64;
            self.minute(m).3 += 1;
            self.found.push(event_row(self.profile, &b, self.bin_hz));
        }
        if let Some(l) = self.det.level_db() {
            let m = (t / 60.0).floor().max(0.0) as u64;
            let e = self.minute(m);
            e.1 += l;
            e.2 += 1;
        }
        if self.keep_rows {
            self.kept.push((t, row.to_vec()));
        }
        if self.display.len() >= DISPLAY_KEPT {
            self.display.pop_front();
        }
        self.display.push_back(
            self.det
                .excess()
                .iter()
                .map(|e| e.clamp(DISPLAY_DB.0, DISPLAY_DB.1))
                .collect(),
        );
        self.display_changed = true;
        let open_from = self.det.open_start().map(|s| (s / 60.0).floor() as u64);
        while (self.next_minute + 1) as f64 * 60.0 + TREND_DELAY_S <= t
            && open_from.is_none_or(|m| m > self.next_minute)
        {
            let m = self.next_minute;
            self.next_minute += 1;
            let (lev, n, bursts) = match self.minute_level.iter().position(|e| e.0 == m) {
                Some(i) => {
                    let e = self.minute_level.remove(i);
                    (e.1, e.2, e.3)
                }
                None => (0.0, 0, 0),
            };
            let mean = (n > 0).then(|| lev / n as f64);
            if self.first_minute_level.is_none() {
                self.first_minute_level = mean;
            }
            let rel = match (mean, self.first_minute_level) {
                (Some(a), Some(b)) => a - b,
                _ => 0.0,
            };
            self.trend.push((m, [rel, f64::from(bursts)]));
        }
    }

    fn minute(&mut self, m: u64) -> &mut (u64, f64, u64, u32) {
        let i = match self.minute_level.iter().position(|e| e.0 == m) {
            Some(i) => i,
            None => {
                self.minute_level.push((m, 0.0, 0, 0));
                self.minute_level.len() - 1
            }
        };
        &mut self.minute_level[i]
    }

    /// The bursts finished since the last call.
    pub fn take_found(&mut self) -> Vec<EventRow> {
        std::mem::take(&mut self.found)
    }

    /// The trend rows written since the last call: the minute, its mean level minus the first minute's (dB), its bursts.
    pub fn take_trend(&mut self) -> Vec<(u64, [f64; 2])> {
        std::mem::take(&mut self.trend)
    }

    /// The power rows kept since the last call, with their start times (s).
    pub fn take_rows(&mut self) -> Vec<(f64, Vec<f32>)> {
        std::mem::take(&mut self.kept)
    }

    /// Writes the display rows into `r` when they changed since the last call. Returns whether they changed.
    pub fn display(&mut self, r: &mut Raster) -> bool {
        if !std::mem::take(&mut self.display_changed) {
            return false;
        }
        r.width = self.bins();
        r.height = self.display.len();
        r.x = Axis::new(self.bin_centre(0), self.bin_hz, "Hz");
        let last = self.origin_s + (self.rows.saturating_sub(1)) as f64 * self.row_s;
        r.y = Axis::new(
            last - self.display.len().saturating_sub(1) as f64 * self.row_s,
            self.row_s,
            "s",
        );
        r.unit.clear();
        r.unit.push_str("dB");
        r.values.clear();
        for row in &self.display {
            r.values.extend_from_slice(row);
        }
        true
    }
}

/// The spectrogram: excess over the background per bin, one row per spectrogram row.
pub const SPECTROGRAM: RasterDef = RasterDef {
    key: "spectrogram",
    label: "Spectrogram",
    x_unit: "Hz",
    y_unit: "s",
    unit: "dB",
};

/// What the stack `id` publishes.
pub fn declaration(id: &str) -> ResultsDecl {
    crate::typed::shape(id).results_decl()
}

/// The `reset` command: the detector starts again (the background is learned again); the tables are emptied.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Empties the tables and starts the detector again; the background is learned again.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The open file: its UTC period and the rows in it.
#[derive(Default)]
struct FileRows {
    period: Option<i64>,
    start_utc: f64,
    rows: Vec<Vec<f32>>,
}

/// The decoder of `jove` and `callisto`: the [`Monitor`] with its tables, trend, display and files.
pub struct BurstDecoder {
    profile: &'static Profile,
    mon: Option<Monitor>,
    rate: f64,
    centre_hz: f64,
    threshold_set: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    station: Arc<Station>,
    storage: Option<Arc<dyn Storage>>,
    input: u64,
    log: EventLog,
    trend: TrendLog,
    bursts: u64,
    events: Vec<Event>,
    raster: Raster,
    raster_id: Option<RasterId>,
    started: bool,
    status: Status,
    fits: bool,
    folder: String,
    station_code: String,
    focus: i64,
    file_s: f64,
    bitpix: Bitpix,
    db_per_digit: f64,
    zero_dbfs: f64,
    file: FileRows,
    files: u64,
    last_file: String,
    file_fault: Option<String>,
}

impl BurstDecoder {
    /// A decoder of `profile`.
    pub fn new(profile: &'static Profile) -> Self {
        BurstDecoder {
            profile,
            mon: None,
            rate: 0.0,
            centre_hz: 0.0,
            threshold_set: THRESHOLD.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            station: Arc::new(Station::default()),
            storage: None,
            input: 0,
            log: EventLog::default(),
            trend: TrendLog::default(),
            bursts: 0,
            events: Vec::new(),
            raster: Raster::default(),
            raster_id: None,
            started: false,
            status: Status::default(),
            fits: FITS.default,
            folder: FITS_FOLDER.default.to_string(),
            station_code: STATION_CODE.default.to_string(),
            focus: FOCUS_CODE.default,
            file_s: FILE_S.default,
            bitpix: FITS_BITPIX.default,
            db_per_digit: DB_PER_DIGIT.default,
            zero_dbfs: ZERO_DBFS.default,
            file: FileRows::default(),
            files: 0,
            last_file: String::new(),
            file_fault: None,
        }
    }

    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn make_monitor(&self, origin_s: f64) -> Option<Monitor> {
        (self.rate > 0.0)
            .then(|| {
                Monitor::new(
                    self.profile,
                    self.rate,
                    self.threshold_set,
                    origin_s,
                    self.fits,
                )
                .ok()
            })
            .flatten()
    }

    fn forget(&mut self) {
        self.log.clear();
        self.trend.clear();
        self.bursts = 0;
    }

    fn reset(&mut self) {
        self.forget();
        self.flush_file();
        self.mon = self.make_monitor(self.now());
        self.raster = Raster::default();
    }

    fn spec(&self) -> FileSpec {
        FileSpec {
            station: self.station_code.clone(),
            focus: self.focus.clamp(0, 63) as u8,
            bitpix: self.bitpix,
            db_per_digit: self.db_per_digit,
            zero_dbfs: self.zero_dbfs,
            position: self
                .station
                .position
                .map(|p| (p.lat_deg, p.lon_deg, self.station.height_m.unwrap_or(p.alt_m))),
        }
    }

    /// Writes the open file, if it holds rows.
    fn flush_file(&mut self) {
        let rows = std::mem::take(&mut self.file.rows);
        self.file.period = None;
        let (Some(mon), Some(storage)) = (self.mon.as_ref(), self.storage.as_ref()) else {
            return;
        };
        if rows.is_empty() {
            return;
        }
        let spec = self.spec();
        let freqs: Vec<f64> = (0..mon.bins())
            .map(|k| self.centre_hz + mon.bin_centre(k))
            .collect();
        let bytes = file::file_bytes(&spec, self.file.start_utc, mon.row_s(), &freqs, &rows);
        let name = spec.name(self.file.start_utc);
        let path = if self.folder.trim().is_empty() {
            StorePath::new(&name)
        } else {
            StorePath::new(self.folder.trim()).and_then(|f| f.join(&name))
        };
        match path {
            Ok(p) => match storage.write(&p, &bytes) {
                Ok(()) => {
                    self.files += 1;
                    self.last_file = p.to_string();
                    self.file_fault = None;
                }
                Err(e) => self.file_fault = Some(format!("cannot write {name}: {e}")),
            },
            Err(e) => self.file_fault = Some(format!("bad file path for {name}: {e}")),
        }
    }

    fn file_rows(&mut self) {
        let Some(mon) = self.mon.as_mut() else {
            return;
        };
        let rows = mon.take_rows();
        if !self.fits {
            return;
        }
        let file_s = self.file_s.max(1.0);
        for (t, row) in rows {
            let utc = self.start_utc + t;
            let period = (utc / file_s).floor() as i64;
            if self.file.period.is_some_and(|p| p != period) {
                self.flush_file();
            }
            if self.file.period.is_none() {
                self.file.period = Some(period);
                self.file.start_utc = utc;
            }
            self.file.rows.push(row);
        }
    }

    fn take(&mut self, out: &mut Outbox) {
        self.file_rows();
        let Some(mon) = self.mon.as_mut() else {
            return;
        };
        for row in mon.take_found() {
            self.bursts += 1;
            self.log.push(row, self.start_utc, &mut self.events);
        }
        for (m, v) in mon.take_trend() {
            self.trend.push(m, v);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
}

impl Decoder for BurstDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        self.profile.component
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Threshold => {
                let v = THRESHOLD.read(id, value)?;
                if v > 0.0 && v < 0.5 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 0.5,
                        max: 20.0,
                        got: v,
                    });
                }
                self.threshold_set = v;
                if let Some(m) = self.mon.as_mut() {
                    m.set_threshold(v);
                }
            }
            DecoderSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
            DecoderSetting::Fits => {
                self.fits = FITS.read(id, value)?;
                if !self.fits {
                    self.flush_file();
                }
                if let Some(m) = self.mon.as_mut() {
                    m.keep_rows(self.fits);
                }
            }
            DecoderSetting::FitsFolder => {
                let f = FITS_FOLDER.read(id, value)?;
                if !f.trim().is_empty() && StorePath::new(f.trim()).is_err() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.folder = f.to_string();
            }
            DecoderSetting::StationCode => {
                let s = STATION_CODE.read(id, value)?;
                if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.station_code = s.to_string();
            }
            DecoderSetting::FocusCode => self.focus = FOCUS_CODE.read(id, value)?,
            DecoderSetting::FileS => self.file_s = FILE_S.read(id, value)?,
            DecoderSetting::FitsBitpix => self.bitpix = FITS_BITPIX.read(id, value)?,
            DecoderSetting::DbPerDigit => self.db_per_digit = DB_PER_DIGIT.read(id, value)?,
            DecoderSetting::ZeroDbfs => self.zero_dbfs = ZERO_DBFS.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Threshold, Value::F64(self.threshold_set)),
            (DecoderSetting::StartUtc, Value::F64(self.start_setting)),
            (DecoderSetting::Fits, Value::Bool(self.fits)),
            (DecoderSetting::FitsFolder, Value::Text(self.folder.clone())),
            (
                DecoderSetting::StationCode,
                Value::Text(self.station_code.clone()),
            ),
            (DecoderSetting::FocusCode, Value::I64(self.focus)),
            (DecoderSetting::FileS, Value::F64(self.file_s)),
            (DecoderSetting::FitsBitpix, Value::Choice(self.bitpix.index())),
            (DecoderSetting::DbPerDigit, Value::F64(self.db_per_digit)),
            (DecoderSetting::ZeroDbfs, Value::F64(self.zero_dbfs)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration(self.profile.id)))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
        self.station = ctx.station.clone();
        self.storage = Some(ctx.storage.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.flush_file();
        self.rate = input.rate;
        self.centre_hz = input.tuned_hz;
        self.input = 0;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.mon = Some(Monitor::new(
            self.profile,
            input.rate,
            self.threshold_set,
            0.0,
            self.fits,
        )?);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        if let Some(m) = self.mon.as_mut() {
            m.process(x);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let (level, bin, row, open, learned) = self.mon.as_ref().map_or(
            (None, None, None, false, false),
            |m| {
                (
                    m.detector().level_db(),
                    Some(m.bin_hz()),
                    Some(m.row_s()),
                    m.detector().is_open(),
                    m.detector().learned(),
                )
            },
        );
        self.log.publish(p, self.start_utc);
        self.trend.publish(
            p,
            &BURST_TREND,
            self.start_utc,
            |utc, [level_change_db, bursts_per_min]| BurstTrendRow {
                utc,
                level_change_db,
                bursts_per_min,
            },
        );
        if let Some(m) = self.mon.as_mut() {
            if m.display(&mut self.raster) {
                if self.raster_id.is_none() {
                    self.raster_id = p.results().decl().raster_id(SPECTROGRAM.key);
                }
                if let Some(id) = self.raster_id {
                    p.results_mut()
                        .update_raster(id, |r| copy_if_changed(r, &self.raster));
                }
            }
        }
        let name = if self.profile.drifting {
            "Solar bursts"
        } else {
            "Radio Jove"
        };
        let st = &mut self.status;
        st.lock_state = match (self.started, learned, self.log.total() > 0) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.level_dbfs = level.map(|l| l as f32);
        st.summary.clear();
        st.summary.push_str(&if !self.started {
            format!("{name}: waiting for samples")
        } else if let Some(f) = &self.file_fault {
            format!("{name}: {f}")
        } else if !learned {
            format!("{name}: learning the background")
        } else {
            format!(
                "{name}: {} bursts{}",
                self.bursts,
                if open { ", a burst now" } else { "" }
            )
        });
        p.put(&BurstFields {
            text: &st.summary,
            bursts: i64::try_from(self.bursts).unwrap_or(i64::MAX),
            events_total: i64::try_from(self.log.total()).unwrap_or(i64::MAX),
            level_db: level,
            bin_hz: bin.map(Hertz),
            row_s: row.map(Seconds),
            burst_open: open,
            files: i64::try_from(self.files).unwrap_or(i64::MAX),
            last_file: &self.last_file,
            note: if self.profile.drifting {
                NOTE_CALLISTO
            } else {
                NOTE_JOVE
            },
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.forget();
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.take(out);
        self.flush_file();
    }
}

/// The receiver of `profile`: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(profile: &'static Profile, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(BurstDecoder::new(profile)))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(profile.id), graph).min_rate(
        MIN_RATE_HZ,
        "The burst monitors need a sample rate of at least 250 ksps",
    );
    if profile.drifting {
        b = b
            .route(FITS.key(), &["dec.fits"])?
            .route(FITS_FOLDER.key(), &["dec.fits_folder"])?
            .route(STATION_CODE.key(), &["dec.station_code"])?
            .route(FOCUS_CODE.key(), &["dec.focus_code"])?
            .route(FILE_S.key(), &["dec.file_s"])?
            .route(FITS_BITPIX.key(), &["dec.fits_bitpix"])?
            .route(DB_PER_DIGIT.key(), &["dec.db_per_digit"])?
            .route(ZERO_DBFS.key(), &["dec.zero_dbfs"])?;
    }
    b.route(THRESHOLD.key(), &["dec.threshold_db"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("dec", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stacks' components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add(JOVE_PROFILE.component, || {
        Box::new(DecoderComponent::new(BurstDecoder::new(&JOVE_PROFILE)))
    });
    r.add(CALLISTO_PROFILE.component, || {
        Box::new(DecoderComponent::new(BurstDecoder::new(&CALLISTO_PROFILE)))
    });
}

fn make_jove(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(&JOVE_PROFILE, cfg))
}

fn make_callisto(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(&CALLISTO_PROFILE, cfg))
}

fn gen_jove(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = gen::GenParams::jove(cfg.rate_hz, cfg.settings.get(&gen::GEN_LEVEL_DB));
    Ok(Box::new(
        gen::BurstGenerator::new(cfg.rate_hz, p, cfg.seed).with_own_noise(crate::family::own_noise(cfg)),
    ))
}

fn gen_callisto(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = gen::GenParams::callisto(
        cfg.rate_hz,
        cfg.settings.get(&gen::GEN_LEVEL_DB),
        cfg.settings.get(&gen::GEN_DRIFT_HZ_S),
    );
    Ok(Box::new(
        gen::BurstGenerator::new(cfg.rate_hz, p, cfg.seed).with_own_noise(crate::family::own_noise(cfg)),
    ))
}

fn bursts_at_least(r: &sdrkit_core::Results, n: f64) -> Vec<CheckLine> {
    let b = crate::family::field(r, BurstFields::BURSTS.key);
    vec![CheckLine::new(
        BurstFields::BURSTS.key,
        b >= n,
        format!("{b}, at least {n} expected"),
    )]
}

fn jove_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, bursts at 0.8 s and 2.2 s of every 4 s from 4 s", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| bursts_at_least(r, 1.0));
    vec![c]
}

fn callisto_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, drifting bursts at 1.6 s and 4.8 s of every 8 s from 8 s", o, 0.0);
    c.seconds = o.seconds.max(16.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| bursts_at_least(r, 1.0));
    vec![c]
}

/// The engine's checks of the `jove` test signal.
pub static JOVE_VERIFY: VerifyHooks = VerifyHooks {
    cases: jove_cases,
    vector_settings: None,
    check_vector: None,
};

/// The engine's checks of the `callisto` test signal.
pub static CALLISTO_VERIFY: VerifyHooks = VerifyHooks {
    cases: callisto_cases,
    vector_settings: None,
    check_vector: None,
};

/// Radio Jove.
pub static JOVE: StackDescriptor = StackDescriptor {
    id: "jove",
    name: "Radio Jove",
    group: "Science, time and utility",
    blurb: "A spectrogram of the whole capture near 20 MHz and the broadband bursts of Jupiter and the Sun in it: time, \
            duration, strength and width, with the background level minute by minute.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 10e6,
        max_hz: 40e6,
        default_hz: 20.1e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: JoveSetting::schema,
    make_receiver: make_jove,
    make_generator: Some(gen_jove),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&JOVE_VERIFY),
    benchmarks: None,
};

/// Solar burst spectrogram.
pub static CALLISTO: StackDescriptor = StackDescriptor {
    id: "callisto",
    name: "Solar burst spectrogram",
    group: "Science, time and utility",
    blurb: "A spectrogram in 62.5 kHz bins and 0.25 s rows, the e-CALLISTO resolution; solar radio bursts with start, \
            duration, strength, bandwidth and drift rate, and e-CALLISTO FITS files.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 45e6,
        max_hz: 870e6,
        default_hz: 200e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: CallistoSetting::schema,
    make_receiver: make_callisto,
    make_generator: Some(gen_callisto),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&CALLISTO_VERIFY),
    benchmarks: None,
};
