//! The A2 decoder: the second FM carrier 242.1875 kHz above the first, its pilot at 54.6875 kHz with the stereo or dual
//! identification, and the matrix.
//!
//! Standard: ITU-R BS.707.

use super::sound::SoundChain;
use super::{last, A2Report, Tune};
use crate::results::A2Mode;
use crate::settings::{DualSound, DualSoundSetting, DUAL_SOUND};
use crate::sound_gen::{A2_DUAL_ID, A2_PILOT, A2_SPACING, A2_STEREO_ID};
use sdrkit_core::{Choice, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom,
    Timed,
};
use std::f64::consts::TAU;

/// The A2 decoder.
pub struct A2Decoder {
    fs: f64,
    first: Option<SoundChain>,
    second: Option<SoundChain>,
    tune: Tune,
    epoch: u32,
    active: bool,
    t: u64,
    // Pilot measurement at the discriminator rate.
    fd: f64,
    ph_p: f64,
    ph_n: f64,
    box1: (f64, f64, usize),
    env: Vec<f64>,
    noise: Vec<f64>,
    sub: (f64, f64, usize),
    win_n: usize,
    present: bool,
    mode: Option<A2Mode>,
    pending_mode: Option<A2Mode>,
    pilot_db: f64,
    level: f64,
    a: Vec<f32>,
    b: Vec<f32>,
    /// Two languages: play the second.
    pub dual_second: bool,
}

impl Default for A2Decoder {
    fn default() -> Self {
        A2Decoder {
            fs: 1.0,
            first: None,
            second: None,
            tune: Tune::default(),
            epoch: u32::MAX,
            active: false,
            t: 0,
            fd: 250e3,
            ph_p: 0.0,
            ph_n: 0.0,
            box1: (0.0, 0.0, 0),
            env: Vec::new(),
            noise: Vec::new(),
            sub: (0.0, 0.0, 0),
            win_n: 0,
            present: false,
            mode: None,
            pending_mode: None,
            pilot_db: 0.0,
            level: 0.0,
            a: Vec::new(),
            b: Vec::new(),
            dual_second: false,
        }
    }
}

fn depth(env: &[f64], rate: f64, f: f64) -> f64 {
    let n = env.len();
    if n == 0 {
        return 0.0;
    }
    let mean = env.iter().sum::<f64>() / n as f64;
    let (mut c, mut s) = (0.0, 0.0);
    for (i, &v) in env.iter().enumerate() {
        let p = TAU * f * i as f64 / rate;
        c += (v - mean) * p.cos();
        s += (v - mean) * p.sin();
    }
    2.0 * (c * c + s * s).sqrt() / n as f64 / mean.max(1e-12)
}

impl A2Decoder {
    fn apply(&mut self) {
        let t = self.tune;
        let c1 = t.carrier_hz + t.spacing_hz;
        if let Some(c) = &mut self.first {
            c.set(c1, 50e3, 50.0, false);
        }
        if let Some(c) = &mut self.second {
            c.set(c1 + A2_SPACING, 50e3, 50.0, false);
        }
    }
    fn reset(&mut self) {
        for c in [&mut self.first, &mut self.second].into_iter().flatten() {
            c.reset();
        }
        self.present = false;
        self.mode = None;
        self.pending_mode = None;
        self.env.clear();
        self.noise.clear();
        self.box1 = (0.0, 0.0, 0);
        self.sub = (0.0, 0.0, 0);
        self.t = 0;
    }
}

impl Component for A2Decoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.a2",
            vec![sdrkit_graph::Port::iq("in"), Port::typed::<Tune>("control")],
            vec![
                Port::audio("out")
                    .rate_from(RateFrom::Fixed(48_000.0))
                    .channels_from(ChannelsFrom::Fixed(2)),
                Port::typed::<A2Report>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DualSoundSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.first = Some(SoundChain::new(self.fs));
        let second = SoundChain::new(self.fs);
        self.fd = second.disc_rate();
        self.second = Some(second);
        self.win_n = (0.1 * self.fd) as usize;
        self.epoch = u32::MAX;
        self.active = false;
        self.reset();
        Ok(vec![
            Format::audio(48_000.0, 2),
            Format::new(Kind::typed::<A2Report>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        // The output states its two channels in every pass, also one without sound.
        io.audio_out(0);
        if let Some(t) = last::<Tune>(io, 1) {
            let changed = t.carrier_hz != self.tune.carrier_hz
                || t.spacing_hz != self.tune.spacing_hz
                || t.epoch != self.epoch;
            if t.epoch != self.epoch {
                self.epoch = t.epoch;
                self.reset();
            }
            let sp = t.spacing_hz.abs();
            self.active = t.second_sound
                && ((sp - 5.5e6).abs() < 0.2e6 || (sp - 6.5e6).abs() < 0.2e6)
                && !t.sound_am;
            self.tune = t;
            if changed {
                self.apply();
            }
        }
        let x = io.iq(0);
        // Part-time while nothing is found: 1.2 s of every 6 s.
        let secs = self.t as f64 / self.fs;
        let run = self.active && (self.present || secs % 6.0 < 1.2);
        self.t += x.len() as u64;
        if !run {
            if self.present {
                self.present = false;
            }
            if io.tick() {
                let t = io.input_format(0).time_of(io.input(0).first());
                io.items_out::<A2Report>(1).push(Timed {
                    time: t,
                    value: A2Report {
                        present: false,
                        mode: None,
                        pilot_db: self.pilot_db,
                        level: self.level,
                        running: false,
                    },
                });
            }
            return;
        }
        let (Some(c1), Some(c2)) = (self.first.as_mut(), self.second.as_mut()) else {
            return;
        };
        self.a.clear();
        self.b.clear();
        c1.process(x, &mut self.a);
        c2.process(x, &mut self.b);
        // The pilot on the second carrier's discriminator (Hz), and a measure of the noise beside it.
        let wp = TAU * A2_PILOT / self.fd;
        let wn = TAU * (A2_PILOT + 7e3) / self.fd;
        let dec = (self.fd / 2000.0).round().max(1.0) as usize;
        for &f in &c2.disc {
            let v = f as f64 / 50e3;
            self.ph_p = (self.ph_p + wp) % TAU;
            self.ph_n = (self.ph_n + wn) % TAU;
            let (sp, cp) = self.ph_p.sin_cos();
            let (sn, cn) = self.ph_n.sin_cos();
            self.box1.0 += v * cp;
            self.box1.1 += v * sp;
            self.sub.0 += v * cn;
            self.sub.1 += v * sn;
            self.box1.2 += 1;
            if self.box1.2 == dec {
                let a = 2.0 * (self.box1.0.powi(2) + self.box1.1.powi(2)).sqrt() / dec as f64;
                let b = 2.0 * (self.sub.0.powi(2) + self.sub.1.powi(2)).sqrt() / dec as f64;
                self.env.push(a);
                self.noise.push(b);
                self.box1 = (0.0, 0.0, 0);
                self.sub = (0.0, 0.0, 0);
            }
        }
        let env_rate = self.fd / dec as f64;
        if self.env.len() as f64 >= 0.1 * env_rate {
            let mean = self.env.iter().sum::<f64>() / self.env.len() as f64;
            let nz = self.noise.iter().sum::<f64>() / self.noise.len().max(1) as f64;
            self.level = mean;
            self.pilot_db = 20.0 * (mean / nz.max(1e-9)).log10();
            let here = mean > 1.8 * nz && mean > 0.004;
            let ds = depth(&self.env, env_rate, A2_STEREO_ID);
            let dd = depth(&self.env, env_rate, A2_DUAL_ID);
            let m = if !here {
                None
            } else if ds > 0.2 && ds > 2.0 * dd {
                Some(A2Mode::Stereo)
            } else if dd > 0.2 && dd > 2.0 * ds {
                Some(A2Mode::Dual)
            } else {
                Some(A2Mode::Mono)
            };
            if m == self.pending_mode {
                self.mode = m;
                self.present = m.is_some();
            }
            self.pending_mode = m;
            self.env.clear();
            self.noise.clear();
        }
        let n = self.a.len().min(self.b.len());
        let out = io.audio_out(0);
        if self.present {
            for i in 0..n {
                let (f, s) = (self.a[i], self.b[i]);
                match self.mode {
                    Some(A2Mode::Stereo) => out.push_stereo(2.0 * f - s, s),
                    Some(A2Mode::Dual) => {
                        let v = if self.dual_second { s } else { f };
                        out.push_stereo(v, v);
                    }
                    _ => out.push_stereo(f, f),
                }
            }
        }
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<A2Report>(1).push(Timed {
                time: t,
                value: A2Report {
                    present: self.present,
                    mode: self.mode,
                    pilot_db: self.pilot_db,
                    level: self.level,
                    running: true,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DualSoundSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DualSoundSetting::DualSound => {
                self.dual_second = DUAL_SOUND.read(id, v)? == DualSound::Second;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let d = if self.dual_second {
            DualSound::Second
        } else {
            DualSound::First
        };
        values_of([(DualSoundSetting::DualSound, Value::Choice(d.index()))])
    }
}
