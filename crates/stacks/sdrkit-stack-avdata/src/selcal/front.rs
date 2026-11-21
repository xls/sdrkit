//! The SELCAL front end: the channel at the signal offset, the demodulator (FM, USB, LSB, AM. Auto takes USB below 30 MHz and
//! AM above) or a sound card input, to 8 kHz audio scaled so 1.0 is 5 kHz of FM deviation.

use super::{DemodChoice, Measures, AUDIO_INPUT, AUDIO_RATE, DEMOD, MIN_RATE_HZ};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir::{bandpass_complex, ComplexFir, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

/// Which demodulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demod {
    /// FM, 1.0 = 5 kHz.
    Fm,
    /// Upper sideband.
    Usb,
    /// Lower sideband.
    Lsb,
    /// AM envelope without its carrier.
    Am,
}

const CH_RATE: f64 = 32_000.0;

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// The samples are sound card audio.
        AudioInput = AUDIO_INPUT,
    }
}

/// The front end.
pub struct Front {
    choice: DemodChoice,
    audio_input: bool,
    rate: f64,
    dial: f64,
    offset: f64,
    demod: Demod,
    chan: Option<Channelizer>,
    disc: Discriminator,
    ssb: Option<ComplexFir>,
    lp: Fir<f32>,
    phase: u32,
    carrier: f32,
    agc: f32,
    resampler: Option<Resampler<f32>>,
    real: Vec<f32>,
    s1: Vec<Complex32>,
    a: Vec<f32>,
    chan_power: f32,
    refused: String,
    t: f64,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            choice: DEMOD.default,
            audio_input: AUDIO_INPUT.default,
            rate: 0.0,
            dial: 0.0,
            offset: 0.0,
            demod: Demod::Am,
            chan: None,
            disc: Discriminator::new(CH_RATE, 5000.0).expect("the rate and deviation are positive"),
            ssb: None,
            lp: Fir::new(&[1.0]),
            phase: 0,
            carrier: 0.0,
            agc: 0.0,
            resampler: None,
            real: Vec::new(),
            s1: Vec::new(),
            a: Vec::new(),
            chan_power: 0.0,
            refused: String::new(),
            t: 0.0,
        }
    }
}

impl Front {
    fn build(&mut self) {
        self.refused.clear();
        self.chan = None;
        self.resampler = None;
        self.demod = match self.choice {
            DemodChoice::Fm => Demod::Fm,
            DemodChoice::Usb => Demod::Usb,
            DemodChoice::Lsb => Demod::Lsb,
            DemodChoice::Am => Demod::Am,
            DemodChoice::Auto => {
                if self.dial > 0.0 && self.dial < 30e6 {
                    Demod::Usb
                } else {
                    Demod::Am
                }
            }
        };
        if !(self.rate.is_finite() && self.rate > 0.0) {
            self.refused = "no samples".into();
            return;
        }
        if self.audio_input {
            if (self.rate - AUDIO_RATE).abs() > 0.5 {
                self.resampler = Resampler::new(self.rate, AUDIO_RATE).ok();
            }
            return;
        }
        if self.rate < MIN_RATE_HZ {
            self.refused = format!(
                "sample rate {:.0} kHz is too low, use at least {:.0} kHz",
                self.rate / 1e3,
                MIN_RATE_HZ / 1e3
            );
            return;
        }
        let mut c = ChannelConfig::new(self.rate, self.offset, 20e3, CH_RATE);
        c.exact_rate = true;
        self.chan = Channelizer::new(c).ok();
        self.disc =
            Discriminator::new(CH_RATE, 5000.0).expect("the rate and deviation are positive");
        self.ssb = match self.demod {
            Demod::Usb => Some(ComplexFir::new(
                &bandpass_complex(CH_RATE, 300.0, 3000.0, 250.0, 60.0)
                    .expect("the filter settings are valid"),
            )),
            Demod::Lsb => Some(ComplexFir::new(
                &bandpass_complex(CH_RATE, -3000.0, -300.0, 250.0, 60.0)
                    .expect("the filter settings are valid"),
            )),
            _ => None,
        };
        self.lp = Fir::new(
            &sdrkit_algo_dsp::fir::lowpass(CH_RATE, 3400.0, 4400.0, 60.0)
                .expect("the filter settings are valid"),
        );
        self.phase = 0;
        self.carrier = 0.0;
        self.agc = 0.0;
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.selcal_front",
            vec![Port::iq("in")],
            vec![
                Port::real("audio").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::typed::<Measures>("info").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = f.tuned_hz;
        self.offset = f.tuned_hz - f.centre_hz;
        self.t = 0.0;
        self.build();
        let mut o = f.with_kind(Kind::Real).with_rate(AUDIO_RATE);
        o.bandwidth_hz = 3500.0;
        Ok(vec![o, Format::new(Kind::typed::<Measures>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let tick = io.tick();
        self.a.clear();
        if self.rate.is_finite() && self.rate > 0.0 {
            self.t += x.len() as f64 / self.rate;
        }
        if self.audio_input {
            self.real.clear();
            self.real.extend(x.iter().map(|v| v.re));
            match self.resampler.as_mut() {
                Some(r) => r.process(&self.real, &mut self.a),
                None => self.a.extend_from_slice(&self.real),
            }
        } else if let Some(ch) = self.chan.as_mut() {
            self.s1.clear();
            ch.process(x, &mut self.s1);
            for &z in &self.s1 {
                self.chan_power += 0.001 * (z.norm_sqr() - self.chan_power);
                let v = match self.demod {
                    Demod::Fm => self.disc.process_sample(z),
                    Demod::Am => {
                        let e = z.norm();
                        self.carrier += 0.0002 * (e - self.carrier);
                        if self.carrier > 0.0 {
                            e / self.carrier - 1.0
                        } else {
                            0.0
                        }
                    }
                    Demod::Usb | Demod::Lsb => {
                        let y = self.ssb.as_mut().map_or(z, |f| f.process_sample(z)).re;
                        // The AGC brings the sideband to the FM path's level (an rms of 0.35).
                        self.agc += 0.0005 * (y * y - self.agc);
                        if self.agc > 0.0 {
                            y * 0.35 / self.agc.sqrt()
                        } else {
                            0.0
                        }
                    }
                };
                let f = self.lp.process_sample(v);
                self.phase += 1;
                if self.phase == 4 {
                    self.phase = 0;
                    self.a.push(if f.is_finite() { f } else { 0.0 });
                }
            }
        }
        io.real_out(0).extend_from_slice(&self.a);
        if tick {
            let carrier =
                self.demod == Demod::Fm && 10.0 * self.chan_power.max(1e-20).log10() > -60.0;
            let t = self.t;
            io.items_out::<Measures>(1).push(Timed {
                time: t,
                value: Measures {
                    time_s: t,
                    dial_hz: if self.audio_input { 0.0 } else { self.dial },
                    carrier,
                    refused: self.refused.clone(),
                    ssb: matches!(self.demod, Demod::Usb | Demod::Lsb) && !self.audio_input,
                    audio_input: self.audio_input,
                    ..Default::default()
                },
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Demod => self.choice = DEMOD.read(id, v)?,
            FrontSetting::AudioInput => self.audio_input = AUDIO_INPUT.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.build();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Demod, Value::Choice(self.choice.index())),
            (FrontSetting::AudioInput, Value::Bool(self.audio_input)),
        ])
    }
}
