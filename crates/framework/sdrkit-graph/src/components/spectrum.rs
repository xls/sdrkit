//! The spectrum component.

use super::dsp;
use crate::component::values_of;
use crate::component::{Applied, Component, ComponentInfo, Io, Port};
use crate::stream::{Format, Kind};
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::field;
use sdrkit_core::Values;
use sdrkit_core::{Choice, ConfigError, Setting, SettingError, SettingId, SettingSet, Value};

sdrkit_core::choice_enum! {
    /// The number of bins of a spectrum frame.
    pub enum SpectrumSize {
        /// 256 bins.
        S256 = ("256", "256"),
        /// 512 bins.
        S512 = ("512", "512"),
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
        /// 32768 bins.
        S32768 = ("32768", "32768"),
        /// 65536 bins.
        S65536 = ("65536", "65536"),
        /// 131072 bins.
        S131072 = ("131072", "131072"),
        /// 262144 bins.
        S262144 = ("262144", "262144"),
    }
}

impl SpectrumSize {
    /// The number of bins.
    pub fn bins(self) -> usize {
        256 << self.index()
    }
    /// The size of `bins` bins, if it is one of the sizes.
    pub fn of(bins: usize) -> Option<SpectrumSize> {
        (0..Self::CHOICES.len() as u16)
            .map_while(Self::from_index)
            .find(|s| s.bins() == bins)
    }
    /// The size whose frame lasts nearest to `seconds` at `rate` samples per second, within the sizes.
    pub fn about(rate: f64, seconds: f64) -> SpectrumSize {
        let want = (rate * seconds).max(1.0).log2();
        let last = Self::CHOICES.len() as u16 - 1;
        let k = (want.round() - 8.0).clamp(0.0, f64::from(last)) as u16;
        Self::from_index(k).unwrap_or(SpectrumSize::S262144)
    }
}

sdrkit_core::choice_enum! {
    /// The values of a spectrum frame.
    pub enum SpectrumScale {
        /// Power in dB (dBFS).
        Db = ("db", "dB"),
        /// Linear power: 1.0 is 0 dBFS.
        Power = ("power", "Linear power"),
    }
}

sdrkit_core::choice_enum! {
    /// When spectrum frames start.
    pub enum FrameTiming {
        /// At the `fps` setting: frames overlap or skip input.
        PerSecond = ("per_second", "Per second"),
        /// One frame after the other, with no overlap and no gap: `rate / size` frames per second.
        BackToBack = ("back_to_back", "Back to back"),
    }
}

sdrkit_core::choice_enum! {
    /// The window of a spectrum.
    pub enum SpectrumWindow {
        /// Blackman-Harris, four terms.
        BlackmanHarris = ("blackman_harris", "Blackman-Harris"),
        /// Hann.
        Hann = ("hann", "Hann"),
        /// No window.
        Rectangular = ("rectangular", "Rectangular"),
    }
}

const SIZE: Setting<field::Choice<SpectrumSize>> =
    Setting::choice("size", "FFT size", SpectrumSize::S1024);
const WINDOW: Setting<field::Choice<SpectrumWindow>> =
    Setting::choice("window", "Window", SpectrumWindow::BlackmanHarris);
const AVERAGE: Setting<field::I64> =
    Setting::new("average", "Averaging", "frames", 1).range(1.0, 1000.0, 1.0);
const FPS: Setting<field::F64> =
    Setting::new("fps", "Frame rate", "/s", 30.0).range(0.1, 1000.0, 0.0);
/// The default of the `fps` setting.
pub(crate) const FPS_DEFAULT: f64 = FPS.default;
const TIMING: Setting<field::Choice<FrameTiming>> =
    Setting::choice("timing", "Frames", FrameTiming::PerSecond);
const SCALE: Setting<field::Choice<SpectrumScale>> =
    Setting::choice("scale", "Values", SpectrumScale::Db);

sdrkit_core::settings! {
    /// The settings of [`Spectrum`].
    pub enum SpectrumSetting {
        /// The bins per frame.
        Size = SIZE,
        /// The window.
        Window = WINDOW,
        /// The frames averaged.
        Average = AVERAGE,
        /// The frames per second.
        Fps = FPS,
        /// When frames start.
        Timing = TIMING,
        /// dB or linear power.
        Scale = SCALE,
    }
}

/// Spectrum (`dsp.spectrum`): `Iq` or `Real` to `Spectrum` frames of `size` bins, lowest frequency first (bin `size/2` is the
/// centre), in dBFS or in linear power (sdrkit-algo-dsp `SpectrumAnalyzer`). A complex tone of magnitude 1.0 on a bin reads
/// 0 dBFS. The spectrum of a `Real` input is one-sided: frames of `size / 2` bins from 0 Hz up to half the rate, over a
/// bandwidth of half the rate.
///
/// Settings: `size` (256 to 262144, a power of two; reformats), `window` (`blackman_harris`, `hann`, `rectangular`), `average`
/// (frames, exponential; live), `fps` (reformats), `timing` (`per_second` at `fps`, or `back_to_back`; reformats), `scale`
/// (`db` or `power`; reformats).
pub struct Spectrum {
    size: SpectrumSize,
    window: SpectrumWindow,
    average: usize,
    fps: f64,
    timing: FrameTiming,
    scale: SpectrumScale,
    rate: f64,
    analyzer: Option<SpectrumAnalyzer>,
    /// The input is `Real`: only the upper half of each frame leaves.
    one_sided: bool,
}

impl Default for Spectrum {
    fn default() -> Self {
        Spectrum {
            size: SIZE.default,
            window: WINDOW.default,
            average: AVERAGE.default as usize,
            fps: FPS.default,
            timing: TIMING.default,
            scale: SCALE.default,
            rate: 0.0,
            analyzer: None,
            one_sided: false,
        }
    }
}

impl Spectrum {
    /// `size` bins at `fps` frames per second, Blackman-Harris, no averaging.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `fps` setting refuses `fps`.
    pub fn new(size: SpectrumSize, fps: f64) -> Result<Self, SettingError> {
        SpectrumSetting::Fps.check(&Value::F64(fps))?;
        Ok(Spectrum {
            size,
            fps,
            ..Self::default()
        })
    }
    fn window(&self) -> Window {
        match self.window {
            SpectrumWindow::BlackmanHarris => Window::BlackmanHarris4,
            SpectrumWindow::Hann => Window::Hann,
            SpectrumWindow::Rectangular => Window::Rectangular,
        }
    }
    /// The frames per second at the input rate.
    fn frame_rate(&self) -> f64 {
        match self.timing {
            FrameTiming::PerSecond => self.fps,
            FrameTiming::BackToBack => self.rate / self.size.bins() as f64,
        }
    }
    fn build(&mut self) -> Result<(), ConfigError> {
        let mut cfg = SpectrumConfig::new(self.size.bins(), self.rate);
        cfg.window = self.window();
        cfg.frames_per_second = self.frame_rate();
        cfg.average = self.average.max(1);
        self.analyzer = Some(SpectrumAnalyzer::new(cfg).map_err(dsp)?);
        Ok(())
    }
}

impl Component for Spectrum {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.spectrum",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Real])],
            vec![Port::new("out", Kind::Spectrum)],
        )
        .with_settings(SpectrumSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.one_sided = f.kind == Kind::Real;
        self.build()?;
        let share = if self.one_sided { 2 } else { 1 };
        Ok(vec![Format {
            rate: self.frame_rate(),
            bandwidth_hz: f.rate / share as f64,
            frame_len: self.size.bins() / share,
            scale: Some(self.scale),
            one_sided: self.one_sided,
            ..f.with_kind(Kind::Spectrum)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(a) = &mut self.analyzer else { return };
        let scale = self.scale;
        let from = if self.one_sided {
            self.size.bins() / 2
        } else {
            0
        };
        // Pieces no longer than the hop hold at most one frame end each, so every frame is kept, however the input is cut.
        let piece = (a.hop().floor() as usize).max(1);
        if io.input_format(0).kind == Kind::Real {
            let x = io.real(0);
            let out = io.spectrum_out(0);
            for chunk in x.chunks(piece) {
                if a.push_real(chunk) > 0 {
                    push_frame(a, scale, from, out);
                }
            }
        } else {
            let x = io.iq(0);
            let out = io.spectrum_out(0);
            for chunk in x.chunks(piece) {
                if a.push(chunk) > 0 {
                    push_frame(a, scale, from, out);
                }
            }
        }
    }
    fn values(&self) -> Values {
        values_of([
            (SpectrumSetting::Size, Value::Choice(self.size.index())),
            (SpectrumSetting::Window, Value::Choice(self.window.index())),
            (SpectrumSetting::Average, Value::I64(self.average as i64)),
            (SpectrumSetting::Fps, Value::F64(self.fps)),
            (SpectrumSetting::Timing, Value::Choice(self.timing.index())),
            (SpectrumSetting::Scale, Value::Choice(self.scale.index())),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SpectrumSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SpectrumSetting::Size => {
                self.size = SIZE.read(id, v)?;
                Ok(Applied::Reformat)
            }
            SpectrumSetting::Window => {
                let old = std::mem::replace(&mut self.window, WINDOW.read(id, v)?);
                if self.rate > 0.0 {
                    if let Err(e) = self.build() {
                        self.window = old;
                        return Err(SettingError::Config {
                            id,
                            source: Box::new(e),
                        });
                    }
                }
                Ok(Applied::Live)
            }
            SpectrumSetting::Average => {
                self.average = AVERAGE.read(id, v)? as usize;
                if let Some(a) = &mut self.analyzer {
                    a.set_average(self.average);
                }
                Ok(Applied::Live)
            }
            SpectrumSetting::Fps => {
                self.fps = FPS.read(id, v)?;
                Ok(Applied::Reformat)
            }
            SpectrumSetting::Timing => {
                self.timing = TIMING.read(id, v)?;
                Ok(Applied::Reformat)
            }
            SpectrumSetting::Scale => {
                self.scale = SCALE.read(id, v)?;
                Ok(Applied::Reformat)
            }
        }
    }
}

/// Appends the bins from `from` of the newest frame of `a` in `scale` to `out`.
fn push_frame(
    a: &mut SpectrumAnalyzer,
    scale: SpectrumScale,
    from: usize,
    out: &mut crate::stream::Frames,
) {
    match scale {
        SpectrumScale::Db => out.push(&a.spectrum_db()[from..]),
        SpectrumScale::Power => out.push(&a.power()[from..]),
    }
}
