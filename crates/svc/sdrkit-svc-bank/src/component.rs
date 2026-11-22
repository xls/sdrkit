//! The bank's parts as graph components: [`CarrierScan`] (`bank.carriers`) watches the carriers of a raster on the shared spectrum
//! of the wide input, and [`Channelize`] (`bank.channelize`) cuts one output for each channel from the wide input in one pass.

use crate::detect::{CarrierDetector, Gate, Reading};
use crate::pfb::Filterbank;
use crate::plan::{self, Path};
use crate::spec::channel_rate;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::publish::{field, keys, Field, Published, TableDef};
use sdrkit_core::{
    Choice, Complex32, ConfigError, EventDef, Results, ResultsDecl, Setting, SettingError,
    SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::component::{no_values, values_of};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Format, Io, Kind, Port, Registry, SpectrumSpec,
};
use std::sync::Arc;

/// Adds the components that can be made from a description (`bank.carriers`).
pub fn register(r: &mut Registry) {
    r.add("bank.carriers", || Box::new(CarrierScan::default()));
}

/// Parses a raster: `first:step:count` (Hz) or a list of frequencies in Hz separated by commas or spaces.
///
/// # Errors
///
/// The byte position of the first part that does not read.
pub fn parse_raster(text: &str) -> Result<Vec<f64>, usize> {
    let lead = text.len() - text.trim_start().len();
    let t = text.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    if t.contains(':') {
        let mut at = lead;
        let mut parts = Vec::with_capacity(3);
        for p in t.split(':') {
            parts.push((at, p.trim()));
            at += p.len() + 1;
        }
        let [(a0, first), (a1, step), (a2, count)] = parts[..] else {
            return Err(lead + t.len());
        };
        let first: f64 = first.parse().map_err(|_| a0)?;
        let step: f64 = step.parse().map_err(|_| a1)?;
        let count: usize = count.parse().map_err(|_| a2)?;
        if count > 100_000 || !first.is_finite() || !step.is_finite() {
            return Err(a2);
        }
        return Ok((0..count).map(|i| first + i as f64 * step).collect());
    }
    let mut out = Vec::new();
    let mut at = lead;
    for piece in t.split(|c: char| c == ',' || c.is_whitespace()) {
        if !piece.is_empty() {
            out.push(
                piece
                    .parse::<f64>()
                    .ok()
                    .filter(|f| f.is_finite())
                    .ok_or(at)?,
            );
        }
        at += piece.len() + 1;
    }
    Ok(out)
}

const RASTER: Setting<field::Text> = Setting::new("raster", "Raster", "", "")
    .help("first:step:count in Hz, or a list of frequencies in Hz.");
const BANDWIDTH: Setting<field::Hz> =
    Setting::new("bandwidth", "Channel width", "Hz", 12.5e3).range(1.0, 1e8, 0.0);
const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Threshold", "dB", 8.0).range(-20.0, 60.0, 0.0);
const HANG: Setting<field::Duration> =
    Setting::new("hang", "Hang", "s", 1.0).range(0.0, 3600.0, 0.0);
const SIZE: Setting<field::Choice<SpectrumSize>> =
    Setting::choice("size", "Spectrum size", SpectrumSize::S4096);
const FPS: Setting<field::F64> =
    Setting::new("fps", "Spectrum frames", "/s", 50.0).range(1.0, 1000.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`CarrierScan`].
    pub enum CarrierScanSetting {
        /// The raster.
        Raster = RASTER,
        /// The width of one channel.
        Bandwidth = BANDWIDTH,
        /// The signal-to-noise ratio that marks a carrier on.
        Threshold = THRESHOLD,
        /// How long a carrier stays on after it falls.
        Hang = HANG,
        /// The bins of the spectrum that the scan reads.
        Size = SIZE,
        /// The spectrum frames per second.
        Fps = FPS,
    }
}

/// The time constant of the average of the spectrum, s.
const TAU_S: f64 = 0.04;
/// How far a level must fall below the threshold to start the hang, dB.
const HYSTERESIS_DB: f32 = 3.0;

const ON: Field<field::Bool> = Field::new("on", "On", "");
const FLOOR: Field<field::F64> = Field::new("floor_dbfs", "Noise floor in one bin", "dBFS");
const CARRIERS: TableDef = TableDef {
    key: "carriers",
    label: "Carriers",
    columns: &[&keys::FREQ_HZ, &keys::LEVEL_DBFS, &keys::SNR_DB, &ON],
    cap: 100_000,
    marks: sdrkit_core::RowMarks::NONE,
};

/// A carrier of the raster rose above the threshold. Its frequency is the raster channel.
pub static CARRIER_ON: EventDef = EventDef::new(
    "carrier.on",
    "Carrier on",
    &[&keys::LEVEL_DBFS, &keys::SNR_DB],
);
/// A carrier of the raster stayed below the threshold for the hang time.
pub static CARRIER_OFF: EventDef = EventDef::new(
    "carrier.off",
    "Carrier off",
    &[&keys::LEVEL_DBFS, &keys::SNR_DB],
);

/// Watches the carriers of a raster (`bank.carriers`) on the spectrum of the wide input, which the graph makes once and shares.
/// Each change emits a [`CARRIER_ON`] or [`CARRIER_OFF`] event at the carrier's frequency. Its results hold a `carriers` table and
/// the noise floor.
pub struct CarrierScan {
    raster_text: String,
    /// Each watched channel: its frequency and width, Hz.
    watch: Vec<(f64, f64)>,
    bandwidth: f64,
    threshold: f64,
    hang: f64,
    size: SpectrumSize,
    fps: f64,
    centre: f64,
    detector: Option<CarrierDetector>,
    gates: Vec<Gate>,
    frames: u64,
    keys: Vec<String>,
    p: Published,
}

impl std::fmt::Debug for CarrierScan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CarrierScan")
            .field("watch", &self.watch.len())
            .finish()
    }
}

fn scan_results() -> Arc<ResultsDecl> {
    Arc::new(
        ResultsDecl::new()
            .with_fields(&[&FLOOR])
            .with_table(&CARRIERS)
            .event(&CARRIER_ON)
            .event(&CARRIER_OFF),
    )
}

impl Default for CarrierScan {
    fn default() -> Self {
        CarrierScan {
            raster_text: String::new(),
            watch: Vec::new(),
            bandwidth: 12.5e3,
            threshold: 8.0,
            hang: 1.0,
            size: SpectrumSize::S4096,
            fps: 50.0,
            centre: 0.0,
            detector: None,
            gates: Vec::new(),
            frames: 0,
            keys: Vec::new(),
            p: Published::from_decl(scan_results()),
        }
    }
}

impl CarrierScan {
    /// Watches these channels: each a frequency and a width in Hz. The readings follow this order.
    pub fn watching(channels: &[(f64, f64)]) -> Self {
        let mut s = CarrierScan {
            watch: channels.to_vec(),
            ..Self::default()
        };
        s.raster_text = channels
            .iter()
            .map(|(f, _)| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        s.setup_rows();
        s
    }

    /// The readings of the watched channels from the last frame, in the order given.
    pub fn readings(&self) -> &[Reading] {
        self.detector.as_ref().map_or(&[], |d| d.readings())
    }

    fn spec(&self) -> SpectrumSpec {
        SpectrumSpec {
            size: sdrkit_graph::SizeRule::Bins(self.size),
            window: SpectrumWindow::BlackmanHarris,
            average: 1,
            frames: sdrkit_graph::FrameRule::PerSecond(self.fps),
            scale: sdrkit_graph::components::SpectrumScale::Db,
        }
    }

    fn setup_rows(&mut self) {
        self.keys = self.watch.iter().map(|(f, _)| format!("{f:.0}")).collect();
        self.p.clear_rows(&CARRIERS);
        for (k, (f, _)) in self.keys.iter().zip(&self.watch) {
            if let Some(mut r) = self.p.row_mut(&CARRIERS, k) {
                r.set(&keys::FREQ_HZ, *f);
                r.set(&ON, false);
            }
        }
        if let Some(d) = &mut self.detector {
            let list: Vec<(f64, f64)> = self
                .watch
                .iter()
                .map(|&(f, b)| (f - self.centre, b))
                .collect();
            d.set_targets(&list);
        }
        self.gates =
            vec![Gate::new(self.threshold as f32, HYSTERESIS_DB, self.hang); self.watch.len()];
    }
}

fn dsp(e: DspError) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}

impl Component for CarrierScan {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bank.carriers",
            vec![Port::spectrum("in", self.spec())],
            vec![Port::new("out", Kind::Messages)],
        )
        .with_settings(CarrierScanSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        // A spectrum format carries the rate of the samples it was made from as its bandwidth.
        if f.bandwidth_hz.is_nan() || f.bandwidth_hz <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.centre = f.centre_hz;
        self.frames = 0;
        self.detector = Some(
            CarrierDetector::new(
                f.bandwidth_hz,
                self.size.bins(),
                Window::BlackmanHarris4,
                self.fps,
                TAU_S,
            )
            .map_err(dsp)?,
        );
        self.setup_rows();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let frames = io.spectrum(0);
        let Some(d) = &mut self.detector else { return };
        if frames.is_empty() {
            return;
        }
        for i in 0..frames.len() {
            d.push_frame(frames.frame(i));
            self.frames += 1;
            let t = self.frames as f64 / self.fps;
            let out = io.messages_out(0);
            for (g, (r, &(f, _))) in self
                .gates
                .iter_mut()
                .zip(d.readings().iter().zip(&self.watch))
            {
                let was = g.is_open();
                let now = g.update(r.snr_db, t);
                if was != now {
                    let def = if now { &CARRIER_ON } else { &CARRIER_OFF };
                    out.push(
                        def.event(t, f)
                            .set(&keys::LEVEL_DBFS, f64::from(r.level_dbfs))
                            .set(&keys::SNR_DB, f64::from(r.snr_db)),
                    );
                }
            }
        }
        self.p.set(&FLOOR, f64::from(d.floor_dbfs()));
        for ((k, g), r) in self.keys.iter().zip(&self.gates).zip(d.readings()) {
            if let Some(mut row) = self.p.row_mut(&CARRIERS, k) {
                row.set(&keys::LEVEL_DBFS, f64::from(r.level_dbfs).round());
                row.set(&keys::SNR_DB, f64::from(r.snr_db).round());
                row.set(&ON, g.is_open());
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CarrierScanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CarrierScanSetting::Raster => {
                let t = RASTER.read(id, v)?;
                let freqs =
                    parse_raster(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                self.raster_text = t.to_owned();
                self.watch = freqs.into_iter().map(|f| (f, self.bandwidth)).collect();
            }
            CarrierScanSetting::Bandwidth => {
                self.bandwidth = BANDWIDTH.read(id, v)?;
                for w in &mut self.watch {
                    w.1 = self.bandwidth;
                }
            }
            CarrierScanSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            CarrierScanSetting::Hang => self.hang = HANG.read(id, v)?,
            CarrierScanSetting::Size => {
                self.size = SIZE.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            CarrierScanSetting::Fps => {
                self.fps = FPS.read(id, v)?;
                return Ok(Applied::Reformat);
            }
        }
        self.setup_rows();
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                CarrierScanSetting::Raster,
                Value::Text(self.raster_text.clone()),
            ),
            (CarrierScanSetting::Bandwidth, Value::Hz(self.bandwidth)),
            (CarrierScanSetting::Threshold, Value::F64(self.threshold)),
            (CarrierScanSetting::Hang, Value::Duration(self.hang)),
            (CarrierScanSetting::Size, Value::Choice(self.size.index())),
            (CarrierScanSetting::Fps, Value::F64(self.fps)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// One output of a [`Channelize`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelOut {
    /// Its frequency, Hz.
    pub freq_hz: f64,
    /// The width kept flat and free of aliases, Hz.
    pub bandwidth_hz: f64,
    /// The output rate, Hz: at least 1.25 times the width, at most the input rate.
    pub rate_hz: f64,
}

enum Route {
    Bin {
        slot: usize,
        nco: Nco,
        resampler: Option<Resampler<Complex32>>,
    },
    Direct(Channelizer),
    Full(Nco),
    Silent,
}

/// Cuts the wide input into fixed channels (outputs `ch0`, `ch1` ...), each moved to 0 Hz at its own rate. All channels that fit a
/// bin come from one shared polyphase filterbank, which reads the input once. A channel too wide for a bin gets its own filter. A
/// channel outside the input outputs nothing.
pub struct Channelize {
    channels: Vec<ChannelOut>,
    max_bins: usize,
    pfb: Option<Filterbank>,
    bins: Vec<usize>,
    bin_outs: Vec<Vec<Complex32>>,
    routes: Vec<Route>,
    scratch: Vec<Complex32>,
    passes: usize,
}

impl std::fmt::Debug for Channelize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channelize")
            .field("channels", &self.channels.len())
            .field("bins", &self.bins())
            .finish()
    }
}

impl Channelize {
    /// Channels at these frequencies, widths and rates, from a filterbank of at most `max_bins` bins.
    pub fn new(channels: &[ChannelOut], max_bins: usize) -> Self {
        Channelize {
            channels: channels.to_vec(),
            max_bins,
            pfb: None,
            bins: Vec::new(),
            bin_outs: Vec::new(),
            routes: Vec::new(),
            scratch: Vec::new(),
            passes: 0,
        }
    }

    /// The bins of the filterbank, when it is used.
    pub fn bins(&self) -> Option<usize> {
        self.pfb.as_ref().map(Filterbank::bins)
    }

    /// How many times the last piece of wide input was read: once for the filterbank, and once for each channel with its own
    /// filter.
    pub fn passes(&self) -> usize {
        self.passes
    }
}

impl Component for Channelize {
    fn info(&self) -> ComponentInfo {
        let outs = (0..self.channels.len())
            .map(|i| Port::iq(&format!("ch{i}")))
            .collect();
        ComponentInfo::processor("bank.channelize", vec![Port::iq("in")], outs)
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let rate = f.rate;
        if rate.is_nan() || rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        let list: Vec<(f64, f64)> = self
            .channels
            .iter()
            .map(|c| (c.freq_hz - f.centre_hz, c.bandwidth_hz))
            .collect();
        let plan = plan::plan(rate, &list, self.max_bins);
        self.pfb = match plan.bins {
            Some(m) => Some(Filterbank::new(rate, m, 80.0).map_err(dsp)?),
            None => None,
        };
        let fb_rate = self.pfb.as_ref().map_or(rate, Filterbank::output_rate);
        self.bins.clear();
        self.routes.clear();
        let mut formats = Vec::with_capacity(self.channels.len());
        for (c, path) in self.channels.iter().zip(&plan.paths) {
            let out_rate = channel_rate(c.rate_hz, 0.0, c.bandwidth_hz, rate);
            let offset = c.freq_hz - f.centre_hz;
            let route = match *path {
                Path::Outside => Route::Silent,
                _ if out_rate >= rate * 0.999 => Route::Full(Nco::new(rate, -offset)),
                Path::Bin { bin, residual_hz } => {
                    let slot = self.bins.iter().position(|&b| b == bin).unwrap_or_else(|| {
                        self.bins.push(bin);
                        self.bins.len() - 1
                    });
                    let resampler = if (fb_rate - out_rate).abs() < 1e-6 {
                        None
                    } else {
                        Some(Resampler::new(fb_rate, out_rate).map_err(dsp)?)
                    };
                    Route::Bin {
                        slot,
                        nco: Nco::new(fb_rate, -residual_hz),
                        resampler,
                    }
                }
                Path::Direct => Route::Direct(
                    Channelizer::new(ChannelConfig {
                        exact_rate: true,
                        ..ChannelConfig::new(rate, offset, c.bandwidth_hz, out_rate)
                    })
                    .map_err(dsp)?,
                ),
            };
            self.routes.push(route);
            formats.push(Format {
                bandwidth_hz: c.bandwidth_hz,
                ..Format::iq(out_rate, c.freq_hz, c.freq_hz)
            });
        }
        self.bin_outs = vec![Vec::new(); self.bins.len()];
        Ok(formats)
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.passes = 0;
        if let Some(fb) = &mut self.pfb {
            for v in &mut self.bin_outs {
                v.clear();
            }
            fb.process(x, &self.bins, &mut self.bin_outs);
            self.passes += 1;
        }
        for (o, route) in self.routes.iter_mut().enumerate() {
            let out = io.iq_out(o);
            match route {
                Route::Silent => {}
                Route::Bin {
                    slot,
                    nco,
                    resampler,
                } => {
                    let src = &self.bin_outs[*slot];
                    match resampler {
                        Some(r) => {
                            self.scratch.resize(src.len(), Complex32::default());
                            nco.mix(src, &mut self.scratch);
                            r.process(&self.scratch, out);
                        }
                        None => {
                            let n = out.len();
                            out.resize(n + src.len(), Complex32::default());
                            nco.mix(src, &mut out[n..]);
                        }
                    }
                }
                Route::Direct(ch) => {
                    ch.process(x, out);
                    self.passes += 1;
                }
                Route::Full(nco) => {
                    let n = out.len();
                    out.resize(n + x.len(), Complex32::default());
                    nco.mix(x, &mut out[n..]);
                    self.passes += 1;
                }
            }
        }
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
