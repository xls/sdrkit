//! Radio astronomy (`astro`): long integrations of power spectra around the hydrogen line (or the OH lines), with the band shape
//! taken out, velocities, calibration, RFI flags, line fits and the longitude-velocity map. The graph is an integrator and a
//! processor.

pub mod process;
pub mod sky;

use crate::family::{lock_state_of, status, write_series, REPORT_S};
use process::{BandShape, LvMap, ProcOptions, Processed};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::Command;
use crate::typed::{
    AstroCalibratedRow, AstroCaptureArgs, AstroFields, AstroFitRow, AstroMapRow, AstroSessionRow,
    ASTRO_CALIBRATED, ASTRO_CAPTURE, ASTRO_DRIFT, ASTRO_FIT, ASTRO_LV_MAP, ASTRO_LV_ROWS,
    ASTRO_SESSION, ASTRO_SPECTRUM, ASTRO_TOTAL_POWER,
};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::{copy_if_changed, Axis, Raster};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    GenConfig, Generator, Hz, RasterId, Receiver, Record, Results, ResultsDecl, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// One finished integration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AstroSpectrum {
    /// Power per bin, lowest frequency first.
    pub power: Vec<f64>,
    /// The phase: 0 on, 1 off (frequency switching).
    pub phase: u8,
    /// Seconds integrated.
    pub seconds: f64,
    /// Signal time at the end.
    pub time: f64,
    /// Finished (else the running average, for the screen).
    pub finished: bool,
    /// Total power, dBFS.
    pub total_dbfs: f64,
    /// The input rate.
    pub rate: f64,
    /// The centre frequency.
    pub centre_hz: f64,
    /// From a radio.
    pub live: bool,
}

impl Item for AstroSpectrum {
    const KIND: &'static str = "astro.spectrum";
}

sdrkit_core::choice_enum! {
    /// The FFT size of the integrator.
    pub enum FftSize {
        /// 1024 bins.
        S1024 = ("1024", "1024"),
        /// 2048 bins.
        S2048 = ("2048", "2048"),
        /// 4096 bins.
        S4096 = ("4096", "4096"),
        /// 8192 bins.
        S8192 = ("8192", "8192"),
        /// 16384 bins.
        S16384 = ("16384", "16384"),
    }
}

impl FftSize {
    /// The number of bins.
    pub fn bins(self) -> usize {
        1024 << self.index()
    }
}

/// The FFT size.
pub const FFT: Setting<field::Choice<FftSize>> = Setting::choice("fft", "FFT size", FftSize::S4096);
/// The length of one integration.
pub const INTEGRATION: Setting<field::Duration> =
    Setting::new("integration_s", "Integration", "s", 10.0).range(0.01, 3600.0, 0.01);

sdrkit_core::settings! {
    /// The settings of [`Integrator`].
    pub enum IntegratorSetting {
        /// The FFT size.
        Fft = FFT,
        /// The length of one integration.
        Integration = INTEGRATION,
    }
}

/// Starts the integration again.
pub const RESTART: Command = Command::new("restart", "Restart the integration");

sdrkit_core::commands! {
    /// The commands of [`Integrator`].
    pub enum IntegratorCommand {
        /// Starts the integration again.
        Restart = RESTART,
    }
}

/// The integrator (`astro.integrator`: iq to `astro.spectrum`): Hann-windowed FFTs back to back, the power averaged over the
/// integration in double precision. It keeps its own FFT: the graph's spectrum outputs frames in dB at a fixed frame rate,
/// where an integration needs the power of every frame of a size the setting chooses.
pub struct Integrator {
    size: FftSize,
    n: usize,
    rate: f64,
    win: Vec<f32>,
    fft: Option<Fft>,
    buf: Vec<Complex32>,
    fill: usize,
    acc: Vec<f64>,
    count: usize,
    target_s: f64,
    done_s: f64,
    tp: (f64, u64),
    restart: bool,
    centre: f64,
    live: bool,
}

impl Default for Integrator {
    fn default() -> Self {
        Integrator {
            size: FFT.default,
            n: FFT.default.bins(),
            rate: 0.0,
            win: Vec::new(),
            fft: None,
            buf: Vec::new(),
            fill: 0,
            acc: Vec::new(),
            count: 0,
            target_s: INTEGRATION.default,
            done_s: 0.0,
            tp: (0.0, 0),
            restart: false,
            centre: 0.0,
            live: false,
        }
    }
}

impl Integrator {
    fn reset(&mut self) {
        self.n = self.size.bins();
        self.win = Window::Hann.coefficients(self.n);
        self.fft = Some(Fft::forward(self.n));
        self.buf = vec![Complex32::new(0.0, 0.0); self.n];
        self.acc = vec![0.0; self.n];
        self.fill = 0;
        self.count = 0;
        self.done_s = 0.0;
    }

    fn spectrum(&self, time: f64, finished: bool) -> AstroSpectrum {
        let c = self.count.max(1) as f64;
        AstroSpectrum {
            power: self.acc.iter().map(|v| v / c).collect(),
            phase: 0,
            seconds: self.done_s,
            time,
            finished,
            total_dbfs: power_to_db(self.tp.0 / self.tp.1.max(1) as f64),
            rate: self.rate,
            centre_hz: self.centre,
            live: self.live,
        }
    }
}

impl Component for Integrator {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "astro.integrator",
            vec![Port::iq("in")],
            vec![Port::typed::<AstroSpectrum>("spectra").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(IntegratorSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.reset();
        Ok(vec![Format::new(Kind::typed::<AstroSpectrum>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.restart) {
            self.reset();
        }
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre = centre_hz;
                self.reset();
            }
        }
        let x = io.iq(0);
        let first = io.input(0).first();
        let end = io.input(0).end();
        let n = self.n;
        let Some(mut fft) = self.fft.take() else {
            return;
        };
        let out = io.items_out::<AstroSpectrum>(0);
        for (i, &s) in x.iter().enumerate() {
            self.tp.0 += s.norm_sqr() as f64;
            self.tp.1 += 1;
            self.buf[self.fill] = s * self.win[self.fill];
            self.fill += 1;
            if self.fill == n {
                self.fill = 0;
                fft.process(&mut self.buf);
                for k in 0..n {
                    self.acc[k] += self.buf[(k + n / 2) % n].norm_sqr() as f64;
                }
                self.count += 1;
                self.done_s += n as f64 / self.rate;
                if self.done_s >= self.target_s - 1e-9 {
                    let t = (first + i as u64 + 1) as f64 / self.rate;
                    out.push(Timed {
                        time: t,
                        value: self.spectrum(t, true),
                    });
                    self.acc.iter_mut().for_each(|v| *v = 0.0);
                    self.count = 0;
                    self.done_s = 0.0;
                }
            }
        }
        self.fft = Some(fft);
        if io.tick() && self.count > 0 {
            let t = end as f64 / self.rate;
            let s = self.spectrum(t, false);
            io.items_out::<AstroSpectrum>(0)
                .push(Timed { time: t, value: s });
            self.tp = (0.0, 0);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IntegratorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IntegratorSetting::Fft => {
                self.size = FFT.read(id, v)?;
                self.reset();
            }
            IntegratorSetting::Integration => {
                self.target_s = INTEGRATION.read(id, v)?;
                self.restart = true;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (IntegratorSetting::Fft, Value::Choice(self.size.index())),
            (
                IntegratorSetting::Integration,
                Value::Duration(self.target_s),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        IntegratorCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match IntegratorCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            IntegratorCommand::Restart => self.restart = true,
        }
        Ok(None)
    }
}

sdrkit_core::choice_enum! {
    /// The rest line of the velocities.
    pub enum RestLine {
        /// H I at 1420.405751 MHz.
        Hi = ("hi", "H I 1420.405751 MHz"),
        /// OH at 1612.231 MHz.
        Oh1612 = ("oh1612", "OH 1612.231 MHz"),
        /// OH at 1665.402 MHz.
        Oh1665 = ("oh1665", "OH 1665.402 MHz"),
        /// OH at 1667.359 MHz.
        Oh1667 = ("oh1667", "OH 1667.359 MHz"),
        /// OH at 1720.530 MHz.
        Oh1720 = ("oh1720", "OH 1720.530 MHz"),
        /// The custom rest frequency.
        Custom = ("custom", "Custom"),
    }
}

impl RestLine {
    /// The rest frequency of the line, `None` for the custom one.
    pub fn freq_hz(self) -> Option<f64> {
        match self {
            RestLine::Hi => Some(1_420_405_751.0),
            RestLine::Oh1612 => Some(1_612_231_000.0),
            RestLine::Oh1665 => Some(1_665_402_000.0),
            RestLine::Oh1667 => Some(1_667_359_000.0),
            RestLine::Oh1720 => Some(1_720_530_000.0),
            RestLine::Custom => None,
        }
    }
}

sdrkit_core::choice_enum! {
    /// Where the band shape comes from.
    pub enum BandShapeChoice {
        /// The spectrum itself.
        None = ("none", "The spectrum itself"),
        /// The captured off-source spectrum.
        OffSource = ("off_source", "Off source"),
        /// Frequency switching.
        Switching = ("switching", "Frequency switching"),
    }
}

sdrkit_core::choice_enum! {
    /// The standard of rest of the velocities.
    pub enum LsrChoice {
        /// The kinematic standard (IAU).
        Kinematic = ("kinematic", "Kinematic (IAU)"),
        /// The dynamical standard (Schönrich).
        Dynamical = ("dynamical", "Dynamical (Schönrich)"),
    }
}

sdrkit_core::choice_enum! {
    /// What the antenna points at.
    pub enum Target {
        /// Galactic l and b.
        Galactic = ("galactic", "Galactic l/b"),
        /// The Sun.
        Sun = ("sun", "Sun"),
        /// The Moon.
        Moon = ("moon", "Moon"),
        /// Cas A.
        CasA = ("casa", "Cas A"),
        /// Cyg A.
        CygA = ("cyga", "Cyg A"),
        /// Sgr A*.
        SgrA = ("sgra", "Sgr A*"),
        /// A custom RA and Dec.
        RaDec = ("radec", "Custom RA/Dec"),
    }
}

sdrkit_core::choice_enum! {
    /// What a capture keeps.
    pub enum CaptureChoice {
        /// The off-source spectrum.
        Off = ("off", "Off source"),
        /// The hot-load power.
        Hot = ("hot", "Hot load"),
        /// The cold-load power.
        Cold = ("cold", "Cold load"),
    }
}

sdrkit_core::choice_enum! {
    /// A note on the processing.
    pub enum Note {
        /// Frequency switching needs a radio.
        SwitchingNeedsRadio = ("switching_needs_radio", "Frequency switching needs a radio"),
        /// The hot power does not exceed the cold power.
        YFactor = ("y_factor", "The hot power must exceed the cold power"),
        /// No clear line to fit.
        NoLine = ("no_line", "No clear line to fit"),
    }
}

/// The rest line.
pub const LINE: Setting<field::Choice<RestLine>> =
    Setting::choice("line", "Rest line", RestLine::Hi);
/// The custom rest frequency.
pub const REST_HZ: Setting<field::Hz> =
    Setting::new("rest_hz", "Custom rest frequency", "Hz", process::HI_HZ).range(10e6, 6e9, 1.0);
/// Where the band shape comes from.
pub const BAND_SHAPE: Setting<field::Choice<BandShapeChoice>> =
    Setting::choice("band_shape", "Band shape from", BandShapeChoice::None);
/// Fold the negative image back.
pub const FOLD: Setting<field::Bool> = Setting::new("fold", "Fold", "", true);
/// The half-width of the DC bridge, -1 for none.
pub const DC_BINS: Setting<field::I64> =
    Setting::new("dc_bins", "DC bridge", "bins", 2).range(-1.0, 16.0, 1.0);
/// The degree of the baseline, -1 for none.
pub const BASELINE_DEGREE: Setting<field::I64> =
    Setting::new("baseline_degree", "Baseline degree", "", 3).range(-1.0, 9.0, 1.0);
/// Leave the line window out of the baseline and search it.
pub const WINDOW: Setting<field::Bool> = Setting::new("window", "Line window", "", true);
/// The low end of the line window.
pub const WINDOW_LO: Setting<field::F64> =
    Setting::new("window_lo_kms", "Window from", "km/s", -150.0).range(-500.0, 500.0, 0.1);
/// The high end of the line window.
pub const WINDOW_HI: Setting<field::F64> =
    Setting::new("window_hi_kms", "Window to", "km/s", 150.0).range(-500.0, 500.0, 0.1);
/// The share of each edge left out.
pub const EDGE: Setting<field::F64> =
    Setting::new("edge", "Edges left out", "", 0.08).range(0.0, 0.3, 0.01);
/// The smoothing.
pub const SMOOTH: Setting<field::I64> =
    Setting::new("smooth_bins", "Smoothing", "bins", 1).range(1.0, 64.0, 1.0);
/// The system temperature.
pub const TSYS: Setting<field::F64> =
    Setting::new("tsys_k", "System temperature", "K", 100.0).range(1.0, 2000.0, 1.0);
/// The standard of rest.
pub const LSR_SETTING: Setting<field::Choice<LsrChoice>> =
    Setting::choice("lsr", "LSR", LsrChoice::Kinematic);
/// What the antenna points at.
pub const TARGET_SETTING: Setting<field::Choice<Target>> =
    Setting::choice("target", "Target", Target::Galactic);
/// The galactic longitude of the target.
pub const TARGET_L: Setting<field::F64> =
    Setting::new("target_l", "Target l", "°", 0.0).range(0.0, 360.0, 0.01);
/// The galactic latitude of the target.
pub const TARGET_B: Setting<field::F64> =
    Setting::new("target_b", "Target b", "°", 0.0).range(-90.0, 90.0, 0.01);
/// The right ascension of the target (J2000).
pub const TARGET_RA: Setting<field::F64> =
    Setting::new("target_ra", "Target RA (J2000)", "°", 0.0).range(0.0, 360.0, 0.001);
/// The declination of the target (J2000).
pub const TARGET_DEC: Setting<field::F64> =
    Setting::new("target_dec", "Target Dec (J2000)", "°", 0.0).range(-90.0, 90.0, 0.001);
/// The test signal has no line.
pub const GEN_OFF_SOURCE: Setting<field::Bool> = Setting::new(
    "gen_off_source",
    "Test signal: off source (no line)",
    "",
    false,
)
.section("Test signal")
.advanced();
/// The velocity of the line of the test signal.
pub const GEN_LINE_KMS: Setting<field::F64> =
    Setting::new("gen_line_kms", "Test signal: line velocity", "km/s", -42.0)
        .range(-250.0, 250.0, 0.1)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`Processor`].
    pub enum ProcessorSetting {
        /// The rest line.
        Line = LINE,
        /// The custom rest frequency.
        RestHz = REST_HZ,
        /// Where the band shape comes from.
        BandShape = BAND_SHAPE,
        /// Fold the negative image back.
        Fold = FOLD,
        /// The DC bridge.
        DcBins = DC_BINS,
        /// The baseline degree.
        BaselineDegree = BASELINE_DEGREE,
        /// The line window.
        Window = WINDOW,
        /// The low end of the line window.
        WindowLo = WINDOW_LO,
        /// The high end of the line window.
        WindowHi = WINDOW_HI,
        /// The edges left out.
        Edge = EDGE,
        /// The smoothing.
        Smooth = SMOOTH,
        /// The system temperature.
        Tsys = TSYS,
        /// The standard of rest.
        Lsr = LSR_SETTING,
        /// The target.
        Target = TARGET_SETTING,
        /// The galactic longitude of the target.
        TargetL = TARGET_L,
        /// The galactic latitude of the target.
        TargetB = TARGET_B,
        /// The right ascension of the target.
        TargetRa = TARGET_RA,
        /// The declination of the target.
        TargetDec = TARGET_DEC,
        /// The length of one integration.
        Integration = INTEGRATION,
    }
}

sdrkit_core::settings! {
    /// The settings of the radio astronomy stack.
    pub enum AstroSetting {
        /// The FFT size.
        Fft = FFT,
        /// The length of one integration.
        Integration = INTEGRATION,
        /// The rest line.
        Line = LINE,
        /// The custom rest frequency.
        RestHz = REST_HZ,
        /// Where the band shape comes from.
        BandShape = BAND_SHAPE,
        /// Fold the negative image back.
        Fold = FOLD,
        /// The DC bridge.
        DcBins = DC_BINS,
        /// The baseline degree.
        BaselineDegree = BASELINE_DEGREE,
        /// The line window.
        Window = WINDOW,
        /// The low end of the line window.
        WindowLo = WINDOW_LO,
        /// The high end of the line window.
        WindowHi = WINDOW_HI,
        /// The edges left out.
        Edge = EDGE,
        /// The smoothing.
        Smooth = SMOOTH,
        /// The system temperature.
        Tsys = TSYS,
        /// The standard of rest.
        Lsr = LSR_SETTING,
        /// The target.
        Target = TARGET_SETTING,
        /// The galactic longitude of the target.
        TargetL = TARGET_L,
        /// The galactic latitude of the target.
        TargetB = TARGET_B,
        /// The right ascension of the target.
        TargetRa = TARGET_RA,
        /// The declination of the target.
        TargetDec = TARGET_DEC,
        /// The test signal has no line.
        GenOffSource = GEN_OFF_SOURCE,
        /// The velocity of the line of the test signal.
        GenLineKms = GEN_LINE_KMS,
    }
}

/// Flags the RFI of the next spectrum.
pub const FLAG_RFI: Command = Command::new("flag_rfi", "Flag RFI");
/// Fits Gaussians to the next spectrum.
pub const FIT_LINES: Command = Command::new("fit_lines", "Fit lines");
/// Adds the next spectrum to the longitude-velocity map at the target's longitude.
pub const ADD_MAP: Command = Command::new("add_map", "Add to the l-v map");
/// Clears the longitude-velocity map.
pub const CLEAR_MAP: Command = Command::new("clear_map", "Clear the map");

sdrkit_core::commands! {
    /// The commands of [`Processor`].
    pub enum ProcessorCommand {
        /// Forgets the session.
        Clear = objcmd::CLEAR,
        /// Keeps the next integration.
        Capture = ASTRO_CAPTURE.command,
        /// Flags the RFI of the next spectrum.
        FlagRfi = FLAG_RFI,
        /// Fits Gaussians to the next spectrum.
        FitLines = FIT_LINES,
        /// Adds the next spectrum to the map.
        AddMap = ADD_MAP,
        /// Clears the map.
        ClearMap = CLEAR_MAP,
    }
}

/// The most bins of a spectrum (the largest FFT).
pub const MAX_BINS: usize = 16384;
/// The most integrations the session keeps.
pub const MAX_SESSION: usize = 500;
/// The most rows of the longitude-velocity map.
pub const MAX_MAP_ROWS: usize = 721;

/// One finished integration of the session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRow {
    /// Signal time at the end of the integration, s.
    pub time_s: f64,
    /// The peak velocity, km/s.
    pub peak_kms: f64,
    /// The excess at the peak (a ratio, 0.01 is 1 %).
    pub excess: f64,
    /// The peak over the residual rms.
    pub snr: f64,
}

impl Shaped for SessionRow {
    type Row = AstroSessionRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AstroSessionRow {
            signal_time_s: self.time_s,
            peak_kms: self.peak_kms,
            excess_pct: 100.0 * self.excess,
            line_snr: self.snr,
        });
    }
}

impl Shift for SessionRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// What a session keeps, saved and restored through the processor's state: the finished integrations and the captured
/// off-source spectrum and load powers.
#[derive(Serialize, Deserialize)]
pub struct Session {
    /// The finished integrations, oldest first.
    pub rows: LogRing<SessionRow>,
    /// The captured off-source spectrum.
    pub off: Option<Vec<f64>>,
    /// The captured hot-load power.
    pub hot: Option<f64>,
    /// The captured cold-load power.
    pub cold: Option<f64>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            rows: LogRing::new(MAX_SESSION),
            off: None,
            hot: None,
            cold: None,
        }
    }
}

impl Shift for Session {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.iter_mut().for_each(|r| r.shift_times(by_s));
    }
}

impl Tables for Session {
    const FORMAT: StateFormat = StateFormat::new("science.astro_session", 1);
    fn clear(&mut self) {
        self.rows.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.rows.adopt(saved.rows);
        self.off = saved.off;
        self.hot = saved.hot;
        self.cold = saved.cold;
    }
}

/// What `astro` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("astro").results_decl()
}

/// The processor (`astro.processor`).
pub struct Processor {
    opt: ProcOptions,
    line: RestLine,
    rest_custom: f64,
    band_shape: BandShapeChoice,
    integration_s: f64,
    finished: u64,
    progress: f64,
    last_power: Option<Vec<f64>>,
    last: Option<Processed>,
    last_finished: Option<Processed>,
    session: Session,
    flags: Vec<bool>,
    fit: Option<process::Fit>,
    lv: LvMap,
    lv_dirty: bool,
    total: VecDeque<(f32, f32)>,
    target: (f64, f64),
    target_kind: Target,
    target_lb: (f64, f64),
    target_radec: (f64, f64),
    calibrated_due: bool,
    calibrated_rows: usize,
    time: f64,
    site: Option<(f64, f64)>,
    dynamical: bool,
    tsys: f64,
    note: Option<Note>,
    pending_capture: Option<CaptureChoice>,
    want_fit: bool,
    want_rfi: bool,
    want_map: bool,
    live: bool,
    clock: Clock,
    published: Published,
    series: [SeriesId; 3],
    lv_raster: RasterId,
    raster: Raster,
    shown: Status,
    key: String,
}

impl Default for Processor {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let series = [ASTRO_SPECTRUM.key, ASTRO_TOTAL_POWER.key, ASTRO_DRIFT.key]
            .map(|k| decl.series_id(k).expect("the declaration holds the series"));
        let lv_raster = decl
            .raster_id(ASTRO_LV_MAP.key)
            .expect("the declaration holds the map");
        Processor {
            opt: ProcOptions::default(),
            line: LINE.default,
            rest_custom: REST_HZ.default,
            band_shape: BAND_SHAPE.default,
            integration_s: INTEGRATION.default,
            finished: 0,
            progress: 0.0,
            last_power: None,
            last: None,
            last_finished: None,
            session: Session::default(),
            flags: Vec::new(),
            fit: None,
            lv: LvMap::default(),
            lv_dirty: true,
            total: VecDeque::new(),
            target: (0.0, 0.0),
            target_kind: TARGET_SETTING.default,
            target_lb: (0.0, 0.0),
            target_radec: (0.0, 0.0),
            calibrated_due: false,
            calibrated_rows: 0,
            time: 0.0,
            site: None,
            dynamical: false,
            tsys: TSYS.default,
            note: None,
            pending_capture: None,
            want_fit: false,
            want_rfi: false,
            want_map: false,
            live: false,
            clock: Clock::fixed(0.0),
            published: Published::from_decl(decl),
            series,
            lv_raster,
            raster: Raster::default(),
            shown: Status::default(),
            key: String::new(),
        }
    }
}

impl Processor {
    /// The terms of the observer's velocity towards the target at Julian date `jd` (Earth rotation, orbit, Sun), km/s; `None`
    /// without a station position.
    fn lsr_terms(&self, jd: f64) -> Option<(f64, f64, f64)> {
        let (lat, lon) = self.site?;
        let (ra, dec) = sky::gal_to_eq(self.target.0, self.target.1);
        Some(sky::lsr_terms(ra, dec, jd, lat, lon, self.dynamical))
    }

    /// Sets the galactic position of the target from its kind: the typed l and b, the typed RA and Dec (J2000), or the
    /// position of the Sun, the Moon or a calibrator now.
    fn update_target(&mut self) {
        let jd = sky::jd_unix(self.clock.now());
        let eq = |(ra, dec): (f64, f64)| sky::eq_to_gal(ra, dec);
        let cal = |i: usize| eq((sky::CALIBRATORS[i].1, sky::CALIBRATORS[i].2));
        self.target = match self.target_kind {
            Target::Sun => eq(sky::sun(jd)),
            Target::Moon => eq(sky::moon(jd)),
            Target::CasA => cal(0),
            Target::CygA => cal(1),
            Target::SgrA => cal(2),
            Target::RaDec => eq(self.target_radec),
            Target::Galactic => self.target_lb,
        };
    }

    fn rest(&self) -> f64 {
        self.line.freq_hz().unwrap_or(self.rest_custom)
    }

    fn band_shape(&self) -> BandShape {
        match self.band_shape {
            BandShapeChoice::None => BandShape::None,
            BandShapeChoice::OffSource => BandShape::OffSource,
            BandShapeChoice::Switching => BandShape::Switching(0),
        }
    }

    fn run(&mut self, s: &AstroSpectrum) {
        self.opt.rest_hz = self.rest();
        self.opt.rate = s.rate;
        self.opt.centre_hz = s.centre_hz;
        self.opt.band_shape = self.band_shape();
        self.live = s.live;
        self.time = s.time;
        if s.finished {
            self.finished += 1;
            self.last_power = Some(s.power.clone());
            if let Some(c) = self.pending_capture.take() {
                let mean = s.power.iter().sum::<f64>() / s.power.len() as f64;
                match c {
                    CaptureChoice::Off => self.session.off = Some(s.power.clone()),
                    CaptureChoice::Hot => self.session.hot = Some(mean),
                    CaptureChoice::Cold => self.session.cold = Some(mean),
                }
            }
        }
        let mut power = s.power.clone();
        if self.want_rfi || !self.flags.is_empty() {
            self.flags = process::rfi_flags(&power);
            process::interpolate_flags(&mut power, &self.flags);
            self.want_rfi = false;
        }
        let off = match self.opt.band_shape {
            BandShape::None => None,
            _ => self.session.off.as_deref(),
        };
        let p = process::process(&power, off, &self.opt);
        self.progress = (s.seconds / self.integration_s).min(1.0);
        if s.finished {
            self.session.rows.push(SessionRow {
                time_s: s.time,
                peak_kms: p.peak_kms,
                excess: p.excess,
                snr: p.snr,
            });
            self.calibrated_due = true;
            self.total.push_back((s.time as f32, s.total_dbfs as f32));
            while self.total.len() > 2000 {
                self.total.pop_front();
            }
        }
        if self.want_fit {
            self.want_fit = false;
            let k = self.tsys;
            let t: Vec<f64> = p.ratio.iter().map(|r| r * k).collect();
            self.fit = process::fit_gaussians(&p.vel, &t, 4);
            if self.fit.is_none() {
                self.note = Some(Note::NoLine);
            }
            self.publish_fit();
        }
        if self.want_map {
            self.want_map = false;
            let t: Vec<f64> = p.ratio.iter().map(|r| r * self.tsys).collect();
            self.lv.add(self.target.0, &p.vel, &t);
            self.lv_dirty = true;
        }
        if s.finished {
            self.last_finished = Some(p.clone());
        }
        self.last = Some(p);
    }

    fn publish_fit(&mut self) {
        let p = &mut self.published;
        p.clear_rows(&ASTRO_FIT);
        let Some(f) = &self.fit else { return };
        for (i, g) in f.components.iter().enumerate() {
            self.key.clear();
            let _ = write!(self.key, "{i}");
            ASTRO_FIT.put(
                p,
                &self.key,
                &AstroFitRow {
                    centre_kms: g.centre.0,
                    amplitude_k: g.amp.0,
                    fwhm_kms: g.fwhm(),
                },
            );
        }
    }

    /// Writes the longitude-velocity map: the raster and the longitude of each row. It runs only after the map changed.
    fn publish_map(&mut self) {
        self.lv_dirty = false;
        let grid = LvMap::grid();
        let width = grid.len();
        let height = self.lv.rows.len().min(MAX_MAP_ROWS);
        let first_l = self.lv.rows.first().map_or(0.0, |r| r.0);
        let r = &mut self.raster;
        r.width = width;
        r.height = height;
        r.x = Axis {
            start: grid[0],
            step: 1.0,
            unit: "km/s".into(),
        };
        r.y = Axis {
            start: first_l,
            step: 0.0,
            unit: "°".into(),
        };
        r.unit.clear();
        r.unit.push('K');
        r.values.clear();
        for (_, row) in self.lv.rows.iter().take(height) {
            r.values.extend(row.iter().map(|&v| v as f32));
        }
        let p = &mut self.published;
        p.results_mut()
            .update_raster(self.lv_raster, |d| copy_if_changed(d, &self.raster));
        p.clear_rows(&ASTRO_LV_ROWS);
        let terminal = self.lv.terminal();
        for (i, (l, _)) in self.lv.rows.iter().take(height).enumerate() {
            self.key.clear();
            let _ = write!(self.key, "{i:03}");
            let v = terminal.iter().find(|t| t.0 == *l).map(|t| t.1);
            ASTRO_LV_ROWS.put(
                p,
                &self.key,
                &AstroMapRow {
                    longitude: *l,
                    terminal_kms: v,
                },
            );
        }
    }

    fn publish(&mut self) {
        if self.lv_dirty {
            self.publish_map();
        }
        let state = if self.finished > 0 { 2 } else { 1 };
        let n = self.last_power.as_ref().map(|p| p.len()).unwrap_or(4096);
        let df = self.opt.rate / n as f64;
        let dv = process::C_KMS * df / self.opt.rest_hz;
        let jd = sky::jd_unix(self.clock.now());
        let lsr = self.lsr_terms(jd);
        let finished = self.last_finished.as_ref();
        let snr_db = finished.map(|x| 20.0 * x.snr.max(1e-6).log10());
        status(
            &mut self.shown,
            lock_state_of(state),
            snr_db.map(|s| s as f32),
            None,
            None,
            |s| {
                let _ = match finished {
                    Some(x) => write!(
                        s,
                        "Radio astronomy: peak {:.1} km/s, {:.2} %, peak over rms {:.1}",
                        x.peak_kms,
                        100.0 * x.excess,
                        x.snr
                    ),
                    None => write!(s, "Radio astronomy: integrating"),
                };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        let [spectrum, total_power, drift] = self.series;
        // The numbers come from the last finished integration; the plot shows the running average too.
        if let Some(x) = &self.last {
            write_series(
                p.results_mut(),
                spectrum,
                true,
                x.vel
                    .iter()
                    .zip(&x.ratio)
                    .map(|(&v, &y)| [v as f32, y as f32]),
            );
        }
        let r = p.results_mut();
        write_series(
            r,
            total_power,
            true,
            self.total.iter().map(|&(a, b)| [a, b]),
        );
        write_series(
            r,
            drift,
            true,
            self.session
                .rows
                .iter()
                .map(|row| [row.time_s as f32, row.excess as f32]),
        );
        let (ra, dec) = sky::gal_to_eq(self.target.0, self.target.1);
        let sky_now = match (self.site, lsr) {
            (Some((lat, lon)), Some(lsr)) => {
                let (raj, decj) = sky::precess(ra, dec, jd);
                Some((sky::horizontal(raj, decj, jd, lat, lon, true), lsr))
            }
            _ => None,
        };
        log_rows(&self.session.rows, &ASTRO_SESSION, self.time, p);
        if std::mem::take(&mut self.calibrated_due) {
            let total = lsr.map(|t| t.0 + t.1 + t.2);
            if let Some(x) = &self.last_finished {
                for (i, (&f, &v)) in x.freq.iter().zip(&x.vel).enumerate() {
                    self.key.clear();
                    let _ = write!(self.key, "{i:05}");
                    ASTRO_CALIBRATED.put(
                        p,
                        &self.key,
                        &AstroCalibratedRow {
                            freq: Hertz(f),
                            vel_topo_kms: v,
                            vel_lsr_kms: total.map(|t| v + t),
                            ta_k: x.ratio[i] * self.tsys,
                            flagged: self.flags.get(i).copied().unwrap_or(false),
                        },
                    );
                }
                for i in x.freq.len()..self.calibrated_rows {
                    self.key.clear();
                    let _ = write!(self.key, "{i:05}");
                    p.remove_row(&ASTRO_CALIBRATED, &self.key);
                }
                self.calibrated_rows = x.freq.len();
            }
        }
        let mut note = None;
        if !self.live && matches!(self.opt.band_shape, BandShape::Switching(_)) {
            note = Some(Note::SwitchingNeedsRadio);
        }
        let y = match (self.session.hot, self.session.cold) {
            (Some(h), Some(c)) => {
                let y = process::y_factor(h, c, 290.0, 10.0);
                if y.is_none() {
                    note = Some(Note::YFactor);
                }
                y
            }
            _ => None,
        };
        p.put(&AstroFields {
            finished: self.finished as i64,
            progress_pct: 100.0 * self.progress,
            bin: Hertz(df),
            bin_km_s: dv,
            span: Hertz(self.opt.rate),
            span_km_s: process::C_KMS * self.opt.rate / self.opt.rest_hz,
            bins_flagged: self.flags.iter().filter(|&&f| f).count() as i64,
            peak: finished.map(|x| Hertz(x.peak_hz)),
            peak_km_s: finished.map(|x| x.peak_kms),
            excess_pct: finished.map(|x| 100.0 * x.excess),
            line_snr: finished.map(|x| x.snr),
            snr_db: finished.and(snr_db),
            target_ra_deg: ra,
            target_dec_deg: dec,
            target_az_deg: sky_now.map(|((az, _), _)| az),
            target_el_deg: sky_now.map(|((_, el), _)| el),
            lsr_rotation_kms: sky_now.map(|(_, lsr)| lsr.0),
            lsr_orbit_kms: sky_now.map(|(_, lsr)| lsr.1),
            lsr_sun_kms: sky_now.map(|(_, lsr)| lsr.2),
            vel_lsr_kms: sky_now
                .and(finished)
                .zip(lsr)
                .map(|(x, lsr)| x.peak_kms + lsr.0 + lsr.1 + lsr.2),
            trx_k: y.map(|(trx, _)| trx),
            gain_per_k: y.map(|(_, g)| g),
            note: note.or(self.note),
        });
    }
}

impl Component for Processor {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "astro.processor",
            vec![Port::typed::<AstroSpectrum>("spectra")],
        )
        .with_settings(ProcessorSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.site = ctx.station.position.map(|p| (p.lat_deg, p.lon_deg));
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<AstroSpectrum>(0);
        for s in items {
            self.run(&s.value);
        }
        if io.tick() || !items.is_empty() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ProcessorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ProcessorSetting::Line => self.line = LINE.read(id, v)?,
            ProcessorSetting::RestHz => self.rest_custom = REST_HZ.read(id, v)?,
            ProcessorSetting::BandShape => self.band_shape = BAND_SHAPE.read(id, v)?,
            ProcessorSetting::Fold => self.opt.fold = FOLD.read(id, v)?,
            ProcessorSetting::DcBins => self.opt.dc_bins = DC_BINS.read(id, v)? as i32,
            ProcessorSetting::BaselineDegree => {
                self.opt.degree = BASELINE_DEGREE.read(id, v)? as i32
            }
            ProcessorSetting::Window => self.opt.window = WINDOW.read(id, v)?,
            ProcessorSetting::WindowLo => self.opt.window_kms.0 = WINDOW_LO.read(id, v)?,
            ProcessorSetting::WindowHi => self.opt.window_kms.1 = WINDOW_HI.read(id, v)?,
            ProcessorSetting::Edge => self.opt.edge = EDGE.read(id, v)?,
            ProcessorSetting::Smooth => self.opt.smooth = SMOOTH.read(id, v)? as usize,
            ProcessorSetting::Tsys => self.tsys = TSYS.read(id, v)?,
            ProcessorSetting::Lsr => {
                self.dynamical = LSR_SETTING.read(id, v)? == LsrChoice::Dynamical
            }
            ProcessorSetting::Target => {
                self.target_kind = TARGET_SETTING.read(id, v)?;
                self.update_target();
            }
            ProcessorSetting::TargetL => {
                self.target_lb.0 = TARGET_L.read(id, v)?;
                self.update_target();
            }
            ProcessorSetting::TargetB => {
                self.target_lb.1 = TARGET_B.read(id, v)?;
                self.update_target();
            }
            ProcessorSetting::TargetRa => {
                self.target_radec.0 = TARGET_RA.read(id, v)?;
                self.update_target();
            }
            ProcessorSetting::TargetDec => {
                self.target_radec.1 = TARGET_DEC.read(id, v)?;
                self.update_target();
            }
            ProcessorSetting::Integration => self.integration_s = INTEGRATION.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let o = &self.opt;
        values_of([
            (ProcessorSetting::Line, Value::Choice(self.line.index())),
            (ProcessorSetting::RestHz, Value::Hz(self.rest_custom)),
            (
                ProcessorSetting::BandShape,
                Value::Choice(self.band_shape.index()),
            ),
            (ProcessorSetting::Fold, Value::Bool(o.fold)),
            (ProcessorSetting::DcBins, Value::I64(i64::from(o.dc_bins))),
            (
                ProcessorSetting::BaselineDegree,
                Value::I64(i64::from(o.degree)),
            ),
            (ProcessorSetting::Window, Value::Bool(o.window)),
            (ProcessorSetting::WindowLo, Value::F64(o.window_kms.0)),
            (ProcessorSetting::WindowHi, Value::F64(o.window_kms.1)),
            (ProcessorSetting::Edge, Value::F64(o.edge)),
            (ProcessorSetting::Smooth, Value::I64(o.smooth as i64)),
            (ProcessorSetting::Tsys, Value::F64(self.tsys)),
            (
                ProcessorSetting::Lsr,
                Value::Choice(
                    if self.dynamical {
                        LsrChoice::Dynamical
                    } else {
                        LsrChoice::Kinematic
                    }
                    .index(),
                ),
            ),
            (
                ProcessorSetting::Target,
                Value::Choice(self.target_kind.index()),
            ),
            (ProcessorSetting::TargetL, Value::F64(self.target_lb.0)),
            (ProcessorSetting::TargetB, Value::F64(self.target_lb.1)),
            (ProcessorSetting::TargetRa, Value::F64(self.target_radec.0)),
            (ProcessorSetting::TargetDec, Value::F64(self.target_radec.1)),
            (
                ProcessorSetting::Integration,
                Value::Duration(self.integration_s),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        ProcessorCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ProcessorCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ProcessorCommand::Clear => self.session.clear(),
            ProcessorCommand::Capture => {
                let AstroCaptureArgs { what } = ASTRO_CAPTURE.args(args)?;
                self.pending_capture = Some(what);
            }
            ProcessorCommand::FlagRfi => self.want_rfi = true,
            ProcessorCommand::FitLines => self.want_fit = true,
            ProcessorCommand::AddMap => self.want_map = true,
            ProcessorCommand::ClearMap => {
                self.lv = LvMap::default();
                self.lv_dirty = true;
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.session.save_state(self.time).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.session.restore_state(state, self.time)?;
        self.publish();
        Ok(())
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 250e3;

/// The receiver: the integrator and the processor.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("integrator", Integrator::default())
        .add("processor", Processor::default())
        .input("integrator")
        .connect("integrator", "processor")
        .clock(REPORT_S)
        .status_from("processor")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut rx = StackGraph::of(crate::typed::shape("astro"), graph)
        .min_rate(
            MIN_RATE,
            "Radio astronomy needs a sample rate of at least 250 ksps",
        )
        .route(FFT.key(), &["integrator.fft"])?
        .route(
            INTEGRATION.key(),
            &["integrator.integration_s", "processor.integration_s"],
        )?;
    for d in ProcessorSetting::schema().settings() {
        let key = &d.field.key;
        if key != INTEGRATION.key() {
            rx = rx.route(key, &[&format!("processor.{key}")])?;
        }
    }
    for c in [
        objcmd::CLEAR,
        ASTRO_CAPTURE.command,
        FLAG_RFI,
        FIT_LINES,
        ADD_MAP,
        CLEAR_MAP,
    ] {
        rx = rx.command(c.key, sdrkit_graph::CommandTarget::of("processor", c.key));
    }
    rx.command(
        RESTART.key,
        sdrkit_graph::CommandTarget::of("integrator", "restart"),
    )
    .with_settings(&cfg.settings)
    .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The generator: Gaussian noise (the sky is noise) shaped by the band 1 + 0.30x − 0.10x² and a Gaussian line 1 % above it at
/// −42 km/s with a FWHM of 20 km/s (for a radio at the H I line), made block by block in the frequency domain, with unit mean
/// power.
pub struct AstroGenerator {
    rate: f64,
    n: usize,
    shape: Vec<f32>,
    scale: f32,
    ifft: Fft,
    block: Vec<Complex32>,
    pos: usize,
    rng: Rng,
}

impl AstroGenerator {
    /// At `rate`, centred on the H I line; `line` false leaves the line out.
    pub fn new(rate: f64, line: bool, line_kms: f64, seed: u64) -> AstroGenerator {
        let n = 16384;
        let f0 = process::HI_HZ;
        let shape: Vec<f32> = (0..n)
            .map(|k| {
                let i = (k + n / 2) % n; // natural FFT order to the index of the lowest frequency first
                let x = 2.0 * i as f64 / (n - 1) as f64 - 1.0;
                let f = f0 + (i as f64 - (n / 2) as f64) * rate / n as f64;
                let v = process::velocity(f, f0);
                let sigma = 20.0 / 2.354_820_045;
                let base = 1.0 + 0.30 * x - 0.10 * x * x;
                let l = if line {
                    0.01 * (-(v - line_kms).powi(2) / (2.0 * sigma * sigma)).exp()
                } else {
                    0.0
                };
                ((base * (1.0 + l)) as f32).sqrt()
            })
            .collect();
        let mean_power = shape.iter().map(|&a| (a * a) as f64).sum::<f64>() / n as f64;
        AstroGenerator {
            rate,
            n,
            scale: (1.0 / (n as f64 * mean_power).sqrt()) as f32,
            shape,
            ifft: Fft::inverse(n),
            block: vec![Complex32::new(0.0, 0.0); n],
            pos: n,
            rng: Rng::new(seed),
        }
    }
    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> AstroGenerator {
        let s = &cfg.settings;
        AstroGenerator::new(
            cfg.rate_hz,
            !s.get(&GEN_OFF_SOURCE),
            s.get(&GEN_LINE_KMS),
            cfg.seed,
        )
    }
}

impl Generator for AstroGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            if self.pos == self.n {
                for (k, b) in self.block.iter_mut().enumerate() {
                    *b = self.rng.complex_normal(1.0) * self.shape[k];
                }
                self.ifft.process(&mut self.block);
                self.pos = 0;
            }
            *o = self.block[self.pos] * self.scale;
            self.pos += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(AstroGenerator::from_config(cfg)))
}

/// The verify case: the hydrogen line of the test signal in 10 s.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, H I line at -42 km/s", o, 0.0);
    c.seconds = o.seconds.max(11.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.settings.push(CaseSetting::new(&SMOOTH, 64));
    c.check = Some(|r| {
        let v = crate::family::field(r, AstroFields::PEAK_KM_S.key);
        vec![CheckLine::new(
            AstroFields::PEAK_KM_S.key,
            (v + 42.0).abs() < 5.0,
            format!("{v:.1} km/s, -42 expected"),
        )]
    });
    vec![c]
}
