//! `tones.front`: the FM, USB, LSB, AM and audio-input paths of the tone stack. Each path outputs 8 kHz audio scaled so that
//! 1.0 is 5 kHz of FM deviation (a full-scale sine).

use crate::front::CarrierDetector;
use crate::{typed_format, typed_port};
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed,
};
use std::f64::consts::TAU;

/// The audio rate of the tone decoders.
pub const AUDIO_RATE: f64 = 8000.0;

/// The audio scale: Hz of FM deviation for 1.0.
pub const FULL_SCALE_HZ: f32 = 5000.0;

sdrkit_core::choice_enum! {
    /// The demodulation.
    pub enum Demod {
        /// USB below 30 MHz, FM above (and FM when the dial is unknown).
        Auto = ("auto", "auto (USB on HF, FM above 30 MHz)"),
        /// Narrow FM.
        Fm = ("fm", "FM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// AM.
        Am = ("am", "AM"),
    }
}

impl Demod {
    /// What automatic demodulation picks for a dial frequency (0: unknown).
    pub fn resolve(self, dial_hz: f64) -> Demod {
        match self {
            Demod::Auto => {
                if dial_hz > 0.0 && dial_hz < 30e6 {
                    Demod::Usb
                } else {
                    Demod::Fm
                }
            }
            d => d,
        }
    }
}

/// What the front end reports every 0.25 s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrontReport {
    /// Audio level (rms over the last 2000 samples), dB relative to a full-scale sine.
    pub audio_db: f32,
    /// FM: the SNR, dB.
    pub snr_db: f32,
    /// FM: the carrier offset, Hz.
    pub offset_hz: f32,
    /// FM: a carrier was seen in the last 5 s.
    pub carrier_recent: bool,
    /// The input is audio (a sound card or an audio file).
    pub audio_input: bool,
    /// The demodulation in use.
    pub demod: Demod,
    /// The input rate.
    pub rate: f64,
    /// The dial frequency (0 when unknown).
    pub dial_hz: f64,
}

impl Item for FrontReport {
    const KIND: &'static str = "tone.front";
}

struct FmPath {
    nco: Nco,
    res: Resampler<Complex32>,
    chan: Fir<Complex32>,
    last: Complex32,
    decim: Decimator<f32>,
    afc: f64,
    afc_nco: Nco,
    det: CarrierDetector,
    acc_f: f64,
    acc_n: usize,
    since_carrier: f64,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
    d: Vec<f32>,
}

struct SsbPath {
    nco: Nco,
    res: Resampler<Complex32>,
    lp: Fir<Complex32>,
    shift: f64,
    n: u64,
    am: bool,
    am_mean: f32,
    agc_p: f32,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
}

struct AudioPath {
    res: Option<Resampler<f32>>,
    a: Vec<f32>,
}

enum Path {
    Fm(Box<FmPath>),
    Ssb(Box<SsbPath>),
    Audio(Box<AudioPath>),
    None,
}

/// The front end.
pub struct ToneFront {
    demod: Demod,
    audio_input: bool,
    rate: f64,
    offset: f64,
    dial: f64,
    path: Path,
    ring: Vec<f32>,
    ring_pos: usize,
    ring_sum: f64,
    since_report: usize,
    actual: Demod,
}

impl Default for ToneFront {
    fn default() -> Self {
        ToneFront {
            demod: Demod::Auto,
            audio_input: false,
            rate: 0.0,
            offset: 0.0,
            dial: 0.0,
            path: Path::None,
            ring: vec![0.0; 2000],
            ring_pos: 0,
            ring_sum: 0.0,
            since_report: 0,
            actual: Demod::Fm,
        }
    }
}

/// The demodulation.
pub const DEMOD: Setting<field::Choice<Demod>> =
    Setting::choice("demod", "Demodulation", Demod::Auto);
/// The input is audio.
pub const AUDIO_INPUT: Setting<field::Bool> = Setting::new("audio_input", "Audio input", "", false)
    .help("The input is audio (a sound card or an audio recording): real samples, 1.0 is 5 kHz of deviation. The source sets it.")
    .restart();

sdrkit_core::settings! {
    /// The settings of `tones.front`.
    pub enum ToneFrontSetting {
        /// The demodulation.
        Demod = DEMOD,
        /// The input is audio.
        AudioInput = AUDIO_INPUT,
    }
}

impl ToneFront {
    fn build(&mut self) -> Result<(), ConfigError> {
        self.actual = if self.audio_input {
            Demod::Fm
        } else {
            self.demod.resolve(self.dial)
        };
        self.path = if self.audio_input {
            Path::Audio(Box::new(AudioPath {
                res: if (self.rate - AUDIO_RATE).abs() < 0.5 {
                    None
                } else {
                    Some(Resampler::with_quality(self.rate, AUDIO_RATE, 0.42, 60.0)?)
                },
                a: Vec::new(),
            }))
        } else if self.actual == Demod::Fm {
            Path::Fm(Box::new(FmPath {
                nco: Nco::new(self.rate, -self.offset),
                res: Resampler::with_quality(self.rate, 48000.0, 0.4, 60.0)?,
                chan: Fir::new(&fir::lowpass(48000.0, 9000.0, 12000.0, 50.0)?),
                last: Complex32::new(0.0, 0.0),
                decim: Decimator::with_alias_free(48000.0, 6, 3200.0, 4000.0, 50.0),
                afc: 0.0,
                afc_nco: Nco::new(48000.0, 0.0),
                det: CarrierDetector::default(),
                acc_f: 0.0,
                acc_n: 0,
                since_carrier: 1e9,
                a: Vec::new(),
                b: Vec::new(),
                d: Vec::new(),
            }))
        } else {
            let am = self.actual == Demod::Am;
            Path::Ssb(Box::new(SsbPath {
                nco: Nco::new(self.rate, -self.offset),
                res: Resampler::with_quality(self.rate, AUDIO_RATE, 0.425, 60.0)?,
                lp: Fir::new(&if am {
                    fir::lowpass(AUDIO_RATE, 3400.0, 3900.0, 50.0)?
                } else {
                    fir::lowpass(AUDIO_RATE, 1300.0, 1600.0, 50.0)?
                }),
                shift: match self.actual {
                    Demod::Usb => 1650.0,
                    Demod::Lsb => -1650.0,
                    _ => 0.0,
                },
                n: 0,
                am,
                am_mean: 0.0,
                agc_p: 0.0,
                a: Vec::new(),
                b: Vec::new(),
            }))
        };
        Ok(())
    }
    fn run(&mut self, iq: &[Complex32], out: &mut Vec<f32>) {
        match &mut self.path {
            Path::Audio(p) => {
                if let Some(r) = p.res.as_mut() {
                    p.a.clear();
                    p.a.extend(iq.iter().map(|z| z.re));
                    let start = out.len();
                    r.process(&p.a, out);
                    out[start..].iter_mut().for_each(|v| *v *= 5000.0);
                } else {
                    out.extend(iq.iter().map(|z| z.re * 5000.0));
                }
            }
            Path::Fm(p) => {
                p.a.resize(iq.len(), Complex32::new(0.0, 0.0));
                p.nco.mix(iq, &mut p.a);
                p.b.clear();
                p.res.process(&p.a, &mut p.b);
                p.d.clear();
                let k = (48000.0 / TAU) as f32;
                for z in p.b.iter() {
                    let z = *z * p.afc_nco.next_sample();
                    let y = p.chan.process_sample(z);
                    let d = y * p.last.conj();
                    p.last = y;
                    let hz = atan2(d.im, d.re) * k;
                    p.d.push(hz);
                    p.acc_f += hz as f64;
                    p.acc_n += 1;
                    if p.det.push(y.norm_sqr()) {
                        let mean = p.acc_f / p.acc_n as f64;
                        p.acc_f = 0.0;
                        p.acc_n = 0;
                        if p.det.present {
                            p.since_carrier = 0.0;
                            // Coarse AFC, following up to ±12 kHz.
                            if mean.abs() > 300.0 {
                                p.afc = (p.afc + 0.5 * mean).clamp(-12e3, 12e3);
                                p.afc_nco.set_freq(-p.afc);
                            }
                        } else {
                            p.since_carrier += 0.01;
                        }
                    }
                }
                p.decim.process(&p.d, out);
            }
            Path::Ssb(p) => {
                p.a.clear();
                p.a.extend(iq.iter().map(|z| {
                    if z.re.is_finite()
                        && z.im.is_finite()
                        && z.re.abs() <= 1e4
                        && z.im.abs() <= 1e4
                    {
                        *z
                    } else {
                        Complex32::new(0.0, 0.0)
                    }
                }));
                p.nco.mix_in_place(&mut p.a);
                p.b.clear();
                p.res.process(&p.a, &mut p.b);
                for z in &p.b {
                    let v = if p.am {
                        let y = p.lp.process_sample(*z);
                        let e = y.norm();
                        p.am_mean += 0.0005 * (e - p.am_mean);
                        e - p.am_mean
                    } else {
                        let ph = TAU * ((p.shift * p.n as f64 / AUDIO_RATE).fract());
                        let rot = Complex32::new(ph.cos() as f32, -(ph.sin() as f32));
                        let y = p.lp.process_sample(*z * rot);
                        (y * rot.conj()).re
                    };
                    p.n += 1;
                    // AGC to 0.25 × 5000 rms.
                    let pw = v * v;
                    let k = if pw > p.agc_p { 0.02 } else { 0.0004 };
                    p.agc_p += k * (pw - p.agc_p);
                    let g = if p.agc_p > 1e-20 {
                        1250.0 / p.agc_p.sqrt()
                    } else {
                        0.0
                    };
                    out.push(v * g.min(1e9));
                }
            }
            Path::None => {}
        }
    }
    fn report(&self) -> FrontReport {
        let audio_db = (10.0 * (self.ring_sum / 2000.0).max(1e-12).log10()) as f32 + 3.0103;
        let mut r = FrontReport {
            audio_db,
            snr_db: 0.0,
            offset_hz: 0.0,
            carrier_recent: false,
            audio_input: self.audio_input,
            demod: self.actual,
            rate: self.rate,
            dial_hz: self.dial,
        };
        if let Path::Fm(p) = &self.path {
            r.snr_db = p.det.snr_db() as f32;
            r.offset_hz = p.afc as f32;
            r.carrier_recent = p.since_carrier < 5.0;
        }
        r
    }
}

impl Component for ToneFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "tones.front",
            vec![Port::iq("in")],
            vec![Port::real("out"), typed_port::<FrontReport>("report")],
        )
        .with_settings(ToneFrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let min = if self.audio_input { 8000.0 } else { 16000.0 };
        if f.rate < min {
            return Err(ConfigError::RateTooLow {
                need: min,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.offset = f.signal_offset_hz();
        self.dial = f.tuned_hz;
        self.ring.iter_mut().for_each(|v| *v = 0.0);
        self.ring_sum = 0.0;
        self.since_report = 0;
        self.build()?;
        Ok(vec![
            Format {
                kind: Kind::Real,
                rate: AUDIO_RATE,
                bandwidth_hz: 4000.0,
                ..f.clone()
            },
            typed_format::<FrontReport>(f),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut out = std::mem::take(io.real_out(0));
        let start = out.len();
        self.run(iq, &mut out);
        out[start..].iter_mut().for_each(|v| *v /= FULL_SCALE_HZ);
        let first = io.output(0).first();
        let mut reports = std::mem::take(io.items_out::<FrontReport>(1));
        for (i, &v) in out[start..].iter().enumerate() {
            let old = self.ring[self.ring_pos];
            self.ring[self.ring_pos] = v * v;
            self.ring_sum += (v * v) as f64 - old as f64;
            self.ring_pos = (self.ring_pos + 1) % 2000;
            self.since_report += 1;
            if self.since_report >= 2000 {
                self.since_report = 0;
                self.ring_sum = self.ring.iter().map(|&x| x as f64).sum();
                reports.push(Timed {
                    time: (first + (start + i) as u64) as f64 / AUDIO_RATE,
                    value: self.report(),
                });
            }
        }
        *io.real_out(0) = out;
        *io.items_out::<FrontReport>(1) = reports;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ToneFrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ToneFrontSetting::Demod => self.demod = DEMOD.read(id, v)?,
            ToneFrontSetting::AudioInput => self.audio_input = AUDIO_INPUT.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (ToneFrontSetting::Demod, Value::Choice(self.demod.index())),
            (ToneFrontSetting::AudioInput, Value::Bool(self.audio_input)),
        ])
    }
}
