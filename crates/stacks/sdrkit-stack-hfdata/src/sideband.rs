//! Sideband audio: the 24 kHz channel (dial at 0 Hz) to 8 kHz audio by USB, LSB, NFM, AM or Auto, with the in-band power of
//! each output sample (before any AGC) on a second port. The linear form has no AGC and no clamp, for stacks that measure
//! bursts against the noise between them.

use sdrkit_algo_modem::analog::{
    AgcConfig, AnalogConfig, AnalogDemod, AnalogMode, AudioFilter, BandPass, Gain, LowPass,
};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port};

/// The input rate.
pub const CHANNEL_RATE: f64 = 24_000.0;
/// The output rate.
pub const AUDIO_RATE: f64 = 8_000.0;

sdrkit_core::choice_enum! {
    /// The demodulator.
    pub enum Demod {
        /// USB below 30 MHz, NFM above.
        Auto = ("auto", "Auto"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// Narrow FM.
        Nfm = ("nfm", "NFM"),
        /// AM.
        Am = ("am", "AM"),
    }
}

/// The `demod` setting.
pub const DEMOD: Setting<field::Choice<Demod>> = Setting::choice("demod", "Demodulator", Demod::Auto)
    .help("USB below 30 MHz, NFM above (Auto). The decoders see the same audio whether or not you listen.");

sdrkit_core::choice_enum! {
    /// The sideband of a single-sideband signal.
    pub enum Side {
        /// Upper sideband.
        Upper = ("upper", "Upper"),
        /// Lower sideband.
        Lower = ("lower", "Lower"),
    }
}

/// The `sideband` setting: USB or LSB only.
pub const SIDEBAND: Setting<field::Choice<Side>> = Setting::choice("sideband", "Sideband", Side::Upper)
    .help("The sideband the signal is sent on.");

sdrkit_core::settings! {
    /// The settings of [`SidebandAudio`].
    pub enum SidebandSetting {
        /// Demodulator.
        Demod = DEMOD,
        /// USB or LSB.
        Sideband = SIDEBAND,
    }
}

/// The demodulator of `mode` (not Auto): 24 kHz in, 8 kHz out. USB and LSB cover 200 to 3800 Hz at -6 dB; NFM and AM take a
/// channel low-pass (6.25 or 3.5 kHz), then an audio band-pass of 300 to 3000 Hz at -6 dB. Without `linear`, the audio is held
/// within plus and minus 1 and USB and LSB follow the envelope (attack 0.01, decay 0.0001 per audio sample); with `linear` the
/// gain is 1 and nothing is clamped.
fn demodulator(mode: Demod, linear: bool) -> AnalogDemod {
    let (m, ssb) = match mode {
        Demod::Lsb => (AnalogMode::Lsb, true),
        Demod::Nfm => (AnalogMode::Nfm, false),
        Demod::Am => (AnalogMode::Am, false),
        Demod::Usb | Demod::Auto => (AnalogMode::Usb, true),
    };
    let mut c = AnalogConfig::new(m, CHANNEL_RATE);
    c.decimation = 3;
    c.clamp = (!linear).then_some(1.0);
    if ssb {
        c.sideband = Some(BandPass {
            low_hz: 340.0,
            high_hz: 3660.0,
            transition_hz: 280.0,
            atten_db: 69.0,
        });
        c.gain = if linear {
            Gain::Manual(1.0)
        } else {
            Gain::Auto(AgcConfig::from_factors(AUDIO_RATE, 0.5, 0.01, 0.0001, 1e6))
        };
    } else {
        c.channel = Some(LowPass::Taps {
            taps: 95,
            cutoff_hz: if m == AnalogMode::Nfm {
                6_250.0
            } else {
                3_500.0
            },
        });
        c.audio = Some(AudioFilter::BandPass(BandPass {
            low_hz: 510.0,
            high_hz: 2790.0,
            transition_hz: 420.0,
            atten_db: 69.0,
        }));
        c.am_normalise = true;
    }
    AnalogDemod::new(c).expect("the sideband filters are constants that the demodulator takes")
}

/// The sideband demodulator as a plain object: Auto resolved from the dial.
pub struct Sideband {
    mode: Demod,
    active: Demod,
    dial_hz: f64,
    linear: bool,
    demod: AnalogDemod,
}

impl Sideband {
    /// A demodulator with AGC for a channel whose dial is `dial_hz` (for Auto).
    pub fn new(mode: Demod, dial_hz: f64) -> Sideband {
        Self::with_gain(mode, dial_hz, false)
    }
    /// A demodulator of gain 1 without AGC or clamp for a channel whose dial is `dial_hz` (for Auto).
    pub fn linear(mode: Demod, dial_hz: f64) -> Sideband {
        Self::with_gain(mode, dial_hz, true)
    }
    fn with_gain(mode: Demod, dial_hz: f64, linear: bool) -> Sideband {
        let active = Self::resolve(mode, dial_hz);
        Sideband {
            mode,
            active,
            dial_hz,
            linear,
            demod: demodulator(active, linear),
        }
    }
    /// The demodulator in use (Auto resolved).
    pub fn active(&self) -> Demod {
        self.active
    }
    /// Changes the demodulator; the filters are rebuilt and cleared.
    pub fn set_mode(&mut self, mode: Demod) {
        *self = Sideband::with_gain(mode, self.dial_hz, self.linear);
    }
    /// A new dial. With Auto, the demodulator can change.
    pub fn set_dial(&mut self, dial_hz: f64) {
        self.dial_hz = dial_hz;
        if Self::resolve(self.mode, dial_hz) != self.active {
            *self = Sideband::with_gain(self.mode, dial_hz, self.linear);
        }
    }
    /// USB below 30 MHz and NFM above for Auto, else the demodulator itself.
    fn resolve(mode: Demod, dial_hz: f64) -> Demod {
        match mode {
            Demod::Auto if dial_hz >= 30e6 => Demod::Nfm,
            Demod::Auto => Demod::Usb,
            m => m,
        }
    }
    /// Processes 24 kHz samples, appending 8 kHz audio and the in-band power of each audio sample.
    pub fn process(&mut self, input: &[Complex32], audio: &mut Vec<f32>, power: &mut Vec<f32>) {
        self.demod.process(input, audio, Some(power));
    }
}

/// The sideband audio as a component: `Iq` 24 kHz in; `Real` 8 kHz audio and `Real` 8 kHz in-band power out.
pub struct SidebandAudio {
    mode: Demod,
    linear: bool,
    sb: Option<Sideband>,
    audio: Vec<f32>,
    power: Vec<f32>,
}

impl SidebandAudio {
    /// The component with demodulator `mode` and AGC.
    pub fn new(mode: Demod) -> Self {
        SidebandAudio {
            mode,
            linear: false,
            sb: None,
            audio: Vec::new(),
            power: Vec::new(),
        }
    }
    /// The component with demodulator `mode`, gain 1, no AGC and no clamp.
    pub fn linear(mode: Demod) -> Self {
        SidebandAudio {
            linear: true,
            ..Self::new(mode)
        }
    }
}

impl Default for SidebandAudio {
    fn default() -> Self {
        Self::new(Demod::Auto)
    }
}

impl Component for SidebandAudio {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.sideband_audio",
            vec![Port::iq("in")],
            vec![
                Port::real("audio").rate_from(sdrkit_graph::RateFrom::Fixed(AUDIO_RATE)),
                Port::real("power").rate_from(sdrkit_graph::RateFrom::Fixed(AUDIO_RATE)),
            ],
        )
        .with_settings(SidebandSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if (f.rate - CHANNEL_RATE).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: CHANNEL_RATE,
                got: f.rate,
            });
        }
        self.sb = Some(Sideband::with_gain(self.mode, f.tuned_hz, self.linear));
        let mut a = f.with_kind(Kind::Real).with_rate(AUDIO_RATE);
        a.bandwidth_hz = 3_800.0;
        Ok(vec![a.clone(), a])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(sb) = self.sb.as_mut() else {
            return;
        };
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { tuned_hz, .. } = t.kind {
                sb.set_dial(tuned_hz);
            }
        }
        self.audio.clear();
        self.power.clear();
        sb.process(io.iq(0), &mut self.audio, &mut self.power);
        io.real_out(0).extend_from_slice(&self.audio);
        io.real_out(1).extend_from_slice(&self.power);
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match SidebandSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SidebandSetting::Demod => self.mode = DEMOD.read(id, value)?,
            SidebandSetting::Sideband => {
                self.mode = match SIDEBAND.read(id, value)? {
                    Side::Upper => Demod::Usb,
                    Side::Lower => Demod::Lsb,
                }
            }
        }
        if let Some(sb) = self.sb.as_mut() {
            sb.set_mode(self.mode);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let side = if self.mode == Demod::Lsb {
            Side::Lower
        } else {
            Side::Upper
        };
        values_of([
            (SidebandSetting::Demod, Value::Choice(self.mode.index())),
            (SidebandSetting::Sideband, Value::Choice(side.index())),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn amp_at(x: &[f32], rate: f64, f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in x.iter().enumerate() {
            let p = TAU * f * i as f64 / rate;
            re += v as f64 * p.cos();
            im -= v as f64 * p.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    #[test]
    fn usb_passes_the_upper_sideband_only() {
        let n = 48_000;
        let up: Vec<Complex32> = (0..n)
            .map(|i| {
                let p = TAU * 1500.0 * i as f64 / 24_000.0;
                Complex32::new(p.cos() as f32 * 0.1, p.sin() as f32 * 0.1)
            })
            .collect();
        let down: Vec<Complex32> = up.iter().map(|c| c.conj()).collect();
        let mut sb = Sideband::new(Demod::Usb, 14e6);
        let (mut a, mut p) = (Vec::new(), Vec::new());
        sb.process(&up, &mut a, &mut p);
        let u = amp_at(&a[8000..], 8000.0, 1500.0);
        let mut sb = Sideband::new(Demod::Usb, 14e6);
        let (mut b, mut q) = (Vec::new(), Vec::new());
        sb.process(&down, &mut b, &mut q);
        let mean_p: f32 = q[8000..].iter().sum::<f32>() / (q.len() - 8000) as f32;
        assert!(u > 0.4, "usb amplitude {u}");
        assert!(mean_p < 1e-6, "lsb leak {mean_p}");
        assert_eq!(a.len(), 16_000);
        let mut sb = Sideband::new(Demod::Auto, 31e6);
        assert_eq!(sb.active(), Demod::Nfm);
        sb.set_mode(Demod::Lsb);
        let (mut c, mut r) = (Vec::new(), Vec::new());
        sb.process(&down, &mut c, &mut r);
        assert!(amp_at(&c[8000..], 8000.0, 1500.0) > 0.4);
    }
}
