//! The sound carrier: an FM discriminator or an AM envelope, de-emphasis, a 15 kHz low-pass filter and resampling to 48 kHz. The
//! A2 decoder and the FPV sound use the same FM sound chain.

use super::{last, SoundReport, Tune};
use crate::systems::System;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::iir::Deemphasis;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{
    ChannelsFrom, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use std::f64::consts::TAU;

/// The audio rate.
pub const AUDIO_HZ: f64 = 48_000.0;

/// One sound chain from a carrier in complex samples to 48 kHz mono: mixer, two decimators to about 250 kHz, the detector, a third
/// decimator with a 15 kHz low-pass to about 50 kHz and the exact resampler.
pub struct SoundChain {
    mix: Nco,
    d1: FirDecimator<Complex32>,
    d2: FirDecimator<Complex32>,
    d3: FirDecimator<f32>,
    res: Resampler<f32>,
    fs2: f64,
    fs3: f64,
    fm: sdrkit_algo_dsp::fm::Discriminator,
    deemph: Option<Deemphasis>,
    deemph_us: f64,
    am: bool,
    am_mean: f64,
    dev_hz: f64,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
    c: Vec<f32>,
    d: Vec<f32>,
    /// The discriminator output at about 250 kHz, in Hz (for presence, deviation and pilots).
    pub disc: Vec<f32>,
    /// The envelope at about 250 kHz.
    pub env: Vec<f32>,
}

impl SoundChain {
    /// A chain for input rate `fs`.
    pub fn new(fs: f64) -> Self {
        let d1n = (fs / 1.25e6).round().max(1.0) as usize;
        let fs1 = fs / d1n as f64;
        let d2n = (fs1 / 250e3).round().max(1.0) as usize;
        let fs2 = fs1 / d2n as f64;
        let d3n = (fs2 / 50e3).round().max(1.0) as usize;
        let fs3 = fs2 / d3n as f64;
        let d1 = if d1n > 1 {
            FirDecimator::design(
                fs,
                d1n,
                250e3,
                (fs1 - 1e6).max(260e3).min(fs1 - 250e3),
                55.0,
            )
            .expect("the filter settings are valid")
        } else {
            FirDecimator::new(&[1.0], 1)
        };
        let d2 = if d2n > 1 {
            FirDecimator::design(fs1, d2n, 100e3, (fs2 - 150e3).max(110e3), 55.0)
                .expect("the filter settings are valid")
        } else {
            FirDecimator::new(&[1.0], 1)
        };
        let d3 = if d3n > 1 {
            FirDecimator::design(fs2, d3n, 15e3, (fs3 - 22e3).max(16e3), 50.0)
                .expect("the filter settings are valid")
        } else {
            FirDecimator::new(
                &sdrkit_algo_dsp::fir::lowpass(fs2, 15e3, 22e3, 50.0)
                    .expect("the filter settings are valid"),
                1,
            )
        };
        SoundChain {
            mix: Nco::new(fs, 0.0),
            d1,
            d2,
            d3,
            res: Resampler::new(fs3, AUDIO_HZ).expect("audio resampler"),
            fs2,
            fs3,
            fm: crate::util::phase_steps(),
            deemph: None,
            deemph_us: 0.0,
            am: false,
            am_mean: 0.0,
            dev_hz: 50e3,
            a: Vec::new(),
            b: Vec::new(),
            c: Vec::new(),
            d: Vec::new(),
            disc: Vec::new(),
            env: Vec::new(),
        }
    }
    /// The rate of the discriminator output.
    pub fn disc_rate(&self) -> f64 {
        self.fs2
    }
    /// Sets the carrier (Hz from the centre), the deviation that is full scale, the de-emphasis (µs, 0 none) and AM.
    pub fn set(&mut self, carrier_hz: f64, dev_hz: f64, deemph_us: f64, am: bool) {
        self.mix.set_freq(-carrier_hz);
        self.dev_hz = dev_hz.max(1.0);
        self.am = am;
        if deemph_us != self.deemph_us {
            self.deemph_us = deemph_us;
            self.deemph = (deemph_us > 0.0).then(|| Deemphasis::new(deemph_us * 1e-6, self.fs2));
        }
    }
    /// Moves the carrier only.
    pub fn retune(&mut self, carrier_hz: f64) {
        self.mix.set_freq(-carrier_hz);
    }
    /// Runs `x`, appending 48 kHz audio to `out` (full scale 1 at the deviation).
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        self.a.clear();
        self.a.extend_from_slice(x);
        self.mix.mix_in_place(&mut self.a);
        self.b.clear();
        self.d1.process(&self.a, &mut self.b);
        self.a.clear();
        self.d2.process(&self.b, &mut self.a);
        self.c.clear();
        self.disc.clear();
        self.env.clear();
        let k = self.fs2 / TAU;
        for &s in &self.a {
            let f = (self.fm.process_sample(s) as f64 * k) as f32;
            let e = s.norm();
            self.disc.push(f);
            self.env.push(e);
            let y = if self.am {
                self.am_mean += 2e-4 * (e as f64 - self.am_mean);
                if self.am_mean > 0.0 {
                    ((e as f64 / self.am_mean - 1.0) / 0.8) as f32
                } else {
                    0.0
                }
            } else {
                let y = f / self.dev_hz as f32;
                match &mut self.deemph {
                    Some(d) => d.process_sample(y),
                    None => y,
                }
            };
            self.c.push(y);
        }
        self.d.clear();
        self.d3.process(&self.c, &mut self.d);
        self.res.process(&self.d, out);
    }
    /// Forgets the state.
    pub fn reset(&mut self) {
        self.d1.reset();
        self.d2.reset();
        self.d3.reset();
        self.res.reset();
        self.fm.reset();
        self.am_mean = 0.0;
        if let Some(d) = &mut self.deemph {
            d.reset();
        }
    }
    /// The audio rate before the resampler.
    pub fn audio_rate(&self) -> f64 {
        self.fs3
    }
}

/// The sound carrier component.
pub struct SoundChannel {
    fs: f64,
    chain: Option<SoundChain>,
    tune: Tune,
    epoch: u32,
    carrier: f64,
    trim: f64,
    present: bool,
    gain: f32,
    // Presence statistics over 20 ms.
    win: usize,
    n: usize,
    s_env: f64,
    s_env2: f64,
    s_f: f64,
    s_f2: f64,
    peak: f64,
    last_peak: f64,
    level: f64,
    level_acc: f64,
    level_n: usize,
    mono: Vec<f32>,
    dev: f64,
    am: bool,
}

impl Default for SoundChannel {
    fn default() -> Self {
        SoundChannel {
            fs: 1.0,
            chain: None,
            tune: Tune::default(),
            epoch: u32::MAX,
            carrier: 0.0,
            trim: 0.0,
            present: false,
            gain: 0.0,
            win: 5000,
            n: 0,
            s_env: 0.0,
            s_env2: 0.0,
            s_f: 0.0,
            s_f2: 0.0,
            peak: 0.0,
            last_peak: 0.0,
            level: -100.0,
            level_acc: 0.0,
            level_n: 0,
            mono: Vec::new(),
            dev: 50e3,
            am: false,
        }
    }
}

impl SoundChannel {
    fn apply(&mut self) {
        let t = self.tune;
        let spacing = t.spacing_hz;
        let Some(chain) = &mut self.chain else { return };
        if spacing == 0.0 {
            self.carrier = f64::NAN;
            return;
        }
        let sys = t.system.map(System::get);
        let am = t.sound_am || spacing.abs() < 4.0e6 || spacing.abs() > 8e6;
        let (dev, de) = match sys {
            Some(s) if !s.am_sound => (s.deviation_hz, s.deemph_us),
            _ => {
                if spacing.abs() < 5e6 {
                    (25e3, 75.0)
                } else {
                    (50e3, 50.0)
                }
            }
        };
        self.dev = dev;
        self.am = am;
        self.carrier = t.carrier_hz + spacing;
        chain.set(self.carrier + self.trim, dev, if am { 0.0 } else { de }, am);
    }
}

impl Component for SoundChannel {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.sound",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![
                Port::audio("out")
                    .rate_from(RateFrom::Fixed(AUDIO_HZ))
                    .channels_from(ChannelsFrom::Fixed(2)),
                Port::typed::<SoundReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        let chain = SoundChain::new(self.fs);
        self.win = (0.02 * chain.disc_rate()) as usize;
        self.chain = Some(chain);
        self.epoch = u32::MAX;
        self.present = false;
        self.gain = 0.0;
        self.trim = 0.0;
        self.carrier = f64::NAN;
        let mut f = Format::audio(AUDIO_HZ, 2);
        f.bandwidth_hz = 15e3;
        Ok(vec![f, Format::new(Kind::typed::<SoundReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 1) {
            let changed = t.epoch != self.epoch
                || t.carrier_hz != self.tune.carrier_hz
                || t.spacing_hz != self.tune.spacing_hz
                || t.sound_am != self.tune.sound_am
                || t.system != self.tune.system;
            if t.epoch != self.epoch {
                self.trim = 0.0;
                self.present = false;
                self.gain = 0.0;
                if let Some(c) = &mut self.chain {
                    c.reset();
                }
            }
            self.epoch = t.epoch;
            self.tune = t;
            if changed {
                self.apply();
            }
        }
        let x = io.iq(0);
        let mut mono = std::mem::take(&mut self.mono);
        mono.clear();
        let chain = self
            .chain
            .as_mut()
            .expect("configure builds the chain before process runs");
        let active = !self.carrier.is_nan();
        if active {
            chain.process(x, &mut mono);
        } else {
            // No carrier known: keep the audio clock running with silence.
            chain.process(&[], &mut mono);
            let n = (x.len() as f64 * AUDIO_HZ / self.fs) as usize;
            mono.resize(n, 0.0);
        }
        // Presence and trim every 20 ms of discriminator output.
        let disc_len = chain.disc.len();
        let mut retune = false;
        for i in 0..disc_len {
            let f = chain.disc[i] as f64;
            let e = chain.env[i] as f64;
            self.s_env += e;
            self.s_env2 += e * e;
            self.s_f += f;
            self.s_f2 += f * f;
            self.peak = self.peak.max(f.abs());
            self.n += 1;
            if self.n >= self.win {
                let n = self.n as f64;
                let me = self.s_env / n;
                let ve = (self.s_env2 / n - me * me).max(0.0);
                let cv = if me > 0.0 { ve.sqrt() / me } else { 10.0 };
                let mf = self.s_f / n;
                let sf = (self.s_f2 / n - mf * mf).max(0.0).sqrt();
                self.present = if self.am {
                    sf < if self.present { 12e3 } else { 8e3 }
                } else {
                    cv < if self.present { 0.45 } else { 0.35 }
                };
                if self.present && !self.am {
                    self.trim = (self.trim + 0.3 * mf).clamp(-20e3, 20e3);
                    retune = true;
                }
                self.last_peak = if self.am {
                    0.0
                } else {
                    (self.peak - mf.abs()).max(0.0)
                };
                self.peak = 0.0;
                self.n = 0;
                self.s_env = 0.0;
                self.s_env2 = 0.0;
                self.s_f = 0.0;
                self.s_f2 = 0.0;
            }
        }
        if retune {
            chain.retune(self.carrier + self.trim);
        }
        let target = if self.present && active { 1.0 } else { 0.0 };
        let out = io.audio_out(0);
        for &m in &mono {
            self.gain += (target - self.gain).clamp(-0.002, 0.002);
            let v = m * self.gain;
            out.push_stereo(v, v);
            self.level_acc += (v as f64) * (v as f64);
            self.level_n += 1;
            if self.level_n >= 4800 {
                self.level = 10.0 * (self.level_acc / self.level_n as f64).max(1e-12).log10();
                self.level_acc = 0.0;
                self.level_n = 0;
            }
        }
        self.mono = mono;
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<SoundReport>(1).push(Timed {
                time: t,
                value: SoundReport {
                    present: self.present && active,
                    am: self.am,
                    deviation_khz: self.last_peak / 1e3,
                    level_dbfs: self.level,
                    trim_hz: self.trim,
                    spacing_hz: self.tune.spacing_hz,
                },
            });
        }
    }
}
