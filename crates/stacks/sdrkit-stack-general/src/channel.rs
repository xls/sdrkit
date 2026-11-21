//! The channel selector and the channel filter. [`ChannelDecimator`] (`general.decimate`) shifts the channel to 0 Hz and halves
//! its rate, [`ExactResampler`] (`general.resample`) brings it to 48 kHz (WFM 240 kHz), and [`ChannelFilter`] (`general.filter`)
//! keeps the passband of the demodulator.

use crate::types::{DEMOD_RATE, WFM_RATE};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::{design_beta, kaiser_len, ComplexFir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Port};

const OFFSET: Setting<field::Hz> =
    Setting::new("offset", "Channel offset", "Hz", 0.0).range(-1e9, 1e9, 0.0);
const WIDE: Setting<field::Bool> = Setting::new("wide", "Wide (WFM)", "", false);

sdrkit_core::settings! {
    /// The settings of [`ChannelDecimator`].
    pub enum ChannelDecimatorSetting {
        /// The channel relative to the dial (Hz). A change is live and phase continuous.
        Offset = OFFSET,
        /// The wide path of WFM. A change reformats.
        Wide = WIDE,
    }
}

/// The pass band each halving keeps (Hz).
pub fn stage_pass(wide: bool) -> f64 {
    if wide {
        115e3
    } else {
        26e3
    }
}

/// The decimated rate `fd` for an input rate: halved while each halving keeps it at or above 96 kHz (WFM 240 kHz) and above 2.2 x
/// the stage pass band.
pub fn decimated_rate(fs: f64, wide: bool) -> (f64, usize) {
    let min = if wide { 240e3 } else { 96e3 };
    let pass = stage_pass(wide);
    let (mut fd, mut k) = (fs, 0);
    while fd / 2.0 >= min && fd / 2.0 > 2.2 * pass {
        fd /= 2.0;
        k += 1;
    }
    (fd, k)
}

/// The taps of one halving stage at `rate`: Kaiser-windowed low-pass, pass band `pass`, stop band from `rate/2 - pass`, 70 dB, 9 to
/// 255 taps.
pub fn halving_taps(rate: f64, pass: f64) -> Vec<f32> {
    let stop = rate / 2.0 - pass;
    let n = kaiser_len(70.0, ((stop - pass) / rate).clamp(1e-4, 0.49))
        .expect("the filter settings are valid")
        .clamp(9, 255)
        | 1;
    sdrkit_algo_dsp::fir::windowed_sinc(n, (pass + stop) / 2.0 / rate, design_beta(70.0))
        .expect("the filter settings are valid")
        .iter()
        .map(|&v| v as f32)
        .collect()
}

/// The first part of the channel selector (`general.decimate`): `Iq` at the input rate to `Iq` at `fd`, the listening channel at
/// 0 Hz.
pub struct ChannelDecimator {
    offset: f64,
    wide: bool,
    signal_offset: f64,
    nco: Nco,
    stages: Vec<FirDecimator<Complex32>>,
    work: Vec<Complex32>,
    tmp: Vec<Complex32>,
}

impl Default for ChannelDecimator {
    fn default() -> Self {
        ChannelDecimator {
            offset: 0.0,
            wide: false,
            signal_offset: 0.0,
            nco: Nco::new(1.0, 0.0),
            stages: Vec::new(),
            work: Vec::new(),
            tmp: Vec::new(),
        }
    }
}

impl ChannelDecimator {
    fn shift(&self) -> f64 {
        -(self.signal_offset + self.offset)
    }
}

impl Component for ChannelDecimator {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.decimate",
            vec![Port::iq("in")],
            vec![Port::iq("out")],
        )
        .with_settings(ChannelDecimatorSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.signal_offset = f.signal_offset_hz();
        self.nco = Nco::new(f.rate, self.shift());
        let (fd, k) = decimated_rate(f.rate, self.wide);
        let pass = stage_pass(self.wide);
        let mut r = f.rate;
        self.stages.clear();
        for _ in 0..k {
            self.stages
                .push(FirDecimator::new(&halving_taps(r, pass), 2));
            r /= 2.0;
        }
        let listen = f.tuned_hz + self.offset;
        Ok(vec![Format {
            rate: fd,
            centre_hz: listen,
            tuned_hz: listen,
            bandwidth_hz: 2.0 * pass,
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.work.clear();
        self.work.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.work);
        for st in &mut self.stages {
            self.tmp.clear();
            st.process(&self.work, &mut self.tmp);
            std::mem::swap(&mut self.work, &mut self.tmp);
        }
        io.iq_out(0).extend_from_slice(&self.work);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelDecimatorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelDecimatorSetting::Offset => {
                self.offset = OFFSET.read(id, v)?;
                self.nco.set_freq(self.shift());
                Ok(Applied::Live)
            }
            ChannelDecimatorSetting::Wide => {
                let w = WIDE.read(id, v)?;
                if w == self.wide {
                    return Ok(Applied::Live);
                }
                self.wide = w;
                Ok(Applied::Reformat)
            }
        }
    }
    fn values(&self) -> Values {
        values_of([
            (ChannelDecimatorSetting::Offset, Value::Hz(self.offset)),
            (ChannelDecimatorSetting::Wide, Value::Bool(self.wide)),
        ])
    }
}

/// The exact resampler (`general.resample`): `Iq` at `fd` to exactly 48 kHz (WFM: 240 kHz, chosen by the input's rate class).
#[derive(Default)]
pub struct ExactResampler {
    rs: Option<FractionalResampler<Complex32>>,
    passthrough: bool,
}

impl Component for ExactResampler {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.resample",
            vec![Port::iq("in")],
            vec![Port::iq("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let wide = f.bandwidth_hz > 100e3;
        let out = if wide { WFM_RATE } else { DEMOD_RATE };
        self.passthrough = (f.rate - out).abs() < 1e-6;
        self.rs = None;
        if !self.passthrough {
            // A windowed-sinc bank flat over the widest channel (+-110 kHz WFM, +-20 kHz narrow), alias-free there.
            let (pass, stop) = if wide { (100e3, 140e3) } else { (20e3, 28e3) };
            self.rs = Some(FractionalResampler::new(f.rate, out, pass, stop, 70.0)?);
        }
        Ok(vec![Format {
            rate: out,
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        match &mut self.rs {
            Some(r) => r.process(x, io.iq_out(0)),
            None => io.iq_out(0).extend_from_slice(x),
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The complex channel filter taps for edges `lo..hi` at `rate` with `n` taps (Kaiser beta 7, unity DC gain of the prototype), or
/// from a custom prototype.
pub fn channel_taps(
    rate: f64,
    lo: f64,
    hi: f64,
    n: usize,
    custom: Option<&[f32]>,
) -> Vec<Complex32> {
    let proto: Vec<f64> = match custom {
        Some(p) if !p.is_empty() => p.iter().map(|&v| v as f64).collect(),
        _ => {
            let fc = ((hi - lo) / 2.0 / rate).clamp(1e-5, 0.5);
            sdrkit_algo_dsp::fir::design_by_taps(n.max(1), fc, Window::kaiser(7.0))
                .unwrap_or_else(|_| vec![1.0])
                .iter()
                .map(|&v| v as f64)
                .collect()
        }
    };
    let fc = (lo + hi) / 2.0 / rate;
    let c = (proto.len() - 1) as f64 / 2.0;
    proto
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            let p = std::f64::consts::TAU * fc * (i as f64 - c);
            Complex32::new((h * p.cos()) as f32, (h * p.sin()) as f32)
        })
        .collect()
}

/// Reads prototype taps typed by a person: numbers separated by commas, spaces or semicolons. Empty text returns no taps.
///
/// # Errors
///
/// The byte position of the first part that is not a finite number.
pub fn parse_taps(text: &str) -> Result<Vec<f32>, usize> {
    let mut taps = Vec::new();
    let mut at = 0;
    for part in text.split([',', ' ', ';']) {
        let t = part.trim();
        if !t.is_empty() {
            match t.parse::<f32>() {
                Ok(v) if v.is_finite() => taps.push(v),
                _ => return Err(at),
            }
        }
        at += part.len() + 1;
    }
    Ok(taps)
}

const LO: Setting<field::Hz> =
    Setting::new("lo", "Low edge", "Hz", -6250.0).range(-150e3, 150e3, 0.0);
const HI: Setting<field::Hz> =
    Setting::new("hi", "High edge", "Hz", 6250.0).range(-150e3, 150e3, 0.0);
const TAPS: Setting<field::I64> = Setting::new("taps", "Taps", "", 0)
    .range(0.0, 2047.0, 1.0)
    .help("The tap count. 0 picks it from the width: 63 at 240 kHz, 511 below 1500 Hz, else 255.");
const CUSTOM: Setting<field::Text> = Setting::new("custom", "Custom prototype taps", "", "")
    .help("Low-pass prototype taps, separated by commas. Empty for the built-in filter.");

sdrkit_core::settings! {
    /// The settings of [`ChannelFilter`].
    pub enum ChannelFilterSetting {
        /// The low edge relative to the channel (Hz).
        Lo = LO,
        /// The high edge relative to the channel (Hz).
        Hi = HI,
        /// The tap count.
        Taps = TAPS,
        /// Custom prototype taps.
        Custom = CUSTOM,
    }
}

/// The channel filter (`general.filter`): `Iq` to `Iq` at the same rate. A change of a setting designs the filter again with
/// empty history.
pub struct ChannelFilter {
    lo: f64,
    hi: f64,
    taps: usize,
    custom_text: String,
    custom: Vec<f32>,
    rate: f64,
    fir: Option<ComplexFir>,
    dirty: bool,
}

impl Default for ChannelFilter {
    fn default() -> Self {
        ChannelFilter {
            lo: -6250.0,
            hi: 6250.0,
            taps: 0,
            custom_text: String::new(),
            custom: Vec::new(),
            rate: 0.0,
            fir: None,
            dirty: true,
        }
    }
}

impl ChannelFilter {
    fn design(&mut self) {
        if self.rate <= 0.0 {
            return;
        }
        let n = if self.taps > 0 {
            self.taps | 1
        } else if self.rate > 100e3 {
            63
        } else if self.hi - self.lo < 1500.0 {
            511
        } else {
            255
        };
        let custom = (!self.custom.is_empty()).then_some(self.custom.as_slice());
        self.fir = Some(ComplexFir::new(&channel_taps(
            self.rate, self.lo, self.hi, n, custom,
        )));
        self.dirty = false;
    }
}

impl Component for ChannelFilter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.filter",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(ChannelFilterSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.design();
        Ok(vec![Format {
            bandwidth_hz: self.hi - self.lo,
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        if self.dirty {
            self.design();
        }
        let buf = io.pass_through::<Complex32>(0, 0);
        if let Some(f) = &mut self.fir {
            f.process_in_place(buf);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelFilterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelFilterSetting::Lo => {
                let lo = LO.read(id, v)?;
                if lo >= self.hi {
                    return Err(SettingError::Conflict {
                        id,
                        with: ChannelFilterSetting::Hi.id(),
                    });
                }
                self.lo = lo;
            }
            ChannelFilterSetting::Hi => {
                let hi = HI.read(id, v)?;
                if hi <= self.lo {
                    return Err(SettingError::Conflict {
                        id,
                        with: ChannelFilterSetting::Lo.id(),
                    });
                }
                self.hi = hi;
            }
            ChannelFilterSetting::Taps => self.taps = TAPS.read(id, v)? as usize,
            ChannelFilterSetting::Custom => {
                let t = CUSTOM.read(id, v)?;
                self.custom =
                    parse_taps(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.custom_text);
            }
        }
        self.dirty = true;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ChannelFilterSetting::Lo, Value::Hz(self.lo)),
            (ChannelFilterSetting::Hi, Value::Hz(self.hi)),
            (ChannelFilterSetting::Taps, Value::I64(self.taps as i64)),
            (
                ChannelFilterSetting::Custom,
                Value::Text(self.custom_text.clone()),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_text_is_read_with_the_place_of_a_fault() {
        assert_eq!(parse_taps("0.5, 1; 0.5"), Ok(vec![0.5, 1.0, 0.5]));
        assert_eq!(parse_taps(""), Ok(vec![]));
        assert_eq!(parse_taps("0.5,x"), Err(4));
    }

    #[test]
    fn decimated_rates() {
        assert_eq!(decimated_rate(2e6, false), (125e3, 4));
        assert_eq!(decimated_rate(2e6, true), (500e3, 2));
        assert_eq!(decimated_rate(250e3, false), (125e3, 1));
        assert_eq!(decimated_rate(250e3, true), (250e3, 0));
    }

    #[test]
    fn the_channel_filter_passes_its_band() {
        let taps = channel_taps(48e3, 300.0, 3000.0, 255, None);
        let resp = |f: f64| {
            let mut s = num_complex::Complex64::new(0.0, 0.0);
            for (i, t) in taps.iter().enumerate() {
                s += num_complex::Complex64::new(t.re as f64, t.im as f64)
                    * num_complex::Complex64::from_polar(
                        1.0,
                        -std::f64::consts::TAU * f * i as f64 / 48e3,
                    );
            }
            s.norm()
        };
        assert!((resp(1650.0) - 1.0).abs() < 0.01);
        assert!(resp(-1000.0) < 1e-3, "the other sideband is out");
        assert!(resp(5000.0) < 1e-3);
    }
}
