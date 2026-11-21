//! The test signal of the multi-carrier stacks: a scene of transmissions with white noise, in a span around a centre
//! frequency. It repeats when it ends. The scenes of the generators place their transmissions at fixed offsets from the
//! centre of the capture; they follow no channel plan.

use crate::fleetsync::code as fleetsync;
use crate::plans::Service;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::modulate::{Am, Fm, Sideband, Ssb};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::{Complex32, ConfigError, Generator, SignalInfo};
use sdrkit_proto_tones::dcs::DcsEncoder;
use sdrkit_proto_tones::mdc;
use sdrkit_proto_tones::{Param, TonesError};
use sdrkit_util_ids::tones::dcs::Dcs;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The rate the audio and the narrow transmissions are made at, Hz.
const BASE_RATE: f64 = 48_000.0;
/// The rate wide FM is made at, Hz.
const WIDE_RATE: f64 = 192_000.0;
/// The carrier amplitude of the demonstration scenes.
pub const DEMO_AMPLITUDE: f32 = 0.12;

/// How a transmission is modulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modulation {
    /// Narrow FM: audio 1.0 is 5 kHz of deviation times the deviation scale.
    Fm,
    /// AM with a carrier.
    Am,
    /// Upper sideband; the carrier is at the channel frequency.
    Usb,
    /// Lower sideband.
    Lsb,
    /// Wide FM: audio 0.5 is 40 kHz of deviation times the deviation scale.
    WideFm,
}

/// The audio of a transmission.
#[derive(Clone, Debug, PartialEq)]
pub enum Audio {
    /// Speech-like audio.
    Speech,
    /// Test tones of these frequencies, Hz, each at 0.3 / n of full scale.
    Tones(Vec<f64>),
    /// No audio: a carrier.
    Silence,
}

/// One transmission of a scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    /// The channel (carrier) frequency, Hz.
    pub freq_hz: f64,
    /// The modulation.
    pub modulation: Modulation,
    /// When it starts, s.
    pub start_s: f64,
    /// How long it lasts, s.
    pub dur_s: f64,
    /// The carrier amplitude, or the peak of SSB; the full scale of the span is 1.
    pub amplitude: f32,
    /// FM: the fraction of the full deviation.
    pub deviation: f64,
    /// The AM modulation depth.
    pub depth: f64,
    /// The offset of the carrier from the channel frequency, Hz.
    pub offset_hz: f64,
    /// A CTCSS tone, Hz, at 0.08 of full scale.
    pub ctcss_hz: Option<f64>,
    /// A DCS code (octal digits) and whether it is inverted, at 0.1 of full scale.
    pub dcs: Option<(&'static str, bool)>,
    /// The audio.
    pub audio: Audio,
    /// Audio at 48 kHz sent first (a signalling burst). The voice starts 0.15 s after it.
    pub burst: Option<Vec<f32>>,
    /// Makes the speech repeatable.
    pub seed: u64,
}

impl Transmission {
    /// A narrow FM transmission of speech on `freq_hz` from 0 s for 2 s at amplitude 0.3.
    pub fn on(freq_hz: f64) -> Transmission {
        Transmission {
            freq_hz,
            modulation: Modulation::Fm,
            start_s: 0.0,
            dur_s: 2.0,
            amplitude: 0.3,
            deviation: 1.0,
            depth: 0.8,
            offset_hz: 0.0,
            ctcss_hz: None,
            dcs: None,
            audio: Audio::Speech,
            burst: None,
            seed: 1,
        }
    }

    /// The same transmission from `start_s` for `dur_s`.
    pub fn at(mut self, start_s: f64, dur_s: f64) -> Transmission {
        self.start_s = start_s;
        self.dur_s = dur_s;
        self
    }

    /// The voice audio at 48 kHz, `n` samples.
    fn voice(&self, n: usize) -> Vec<f32> {
        match &self.audio {
            Audio::Silence => vec![0.0; n],
            Audio::Tones(hz) => (0..n)
                .map(|i| {
                    let t = i as f64 / BASE_RATE;
                    let a = 0.3 / hz.len().max(1) as f64;
                    hz.iter().map(|f| a * (TAU * f * t).sin()).sum::<f64>() as f32
                })
                .collect(),
            Audio::Speech => {
                let level = if self.modulation == Modulation::Fm {
                    0.35
                } else {
                    0.4
                };
                let low = crate::inversion::speech(n as f64 / BASE_RATE + 0.1, self.seed);
                let mut out = Vec::with_capacity(n + 64);
                if let Ok(mut r) = Resampler::<f32>::new(crate::common::RATE, BASE_RATE) {
                    r.process(&low, &mut out);
                }
                out.resize(n, 0.0);
                let mut rng = Rng::new(self.seed ^ 0x5eed);
                for v in &mut out {
                    *v = *v * (level / 0.5) + 0.01 * rng.normal_f32();
                }
                out
            }
        }
    }

    /// The transmission at its own rate, the carrier at 0 Hz, and that rate.
    fn baseband(&self) -> Result<(Vec<Complex32>, f64), ConfigError> {
        let n = (self.dur_s * BASE_RATE).round() as usize;
        let lead = self
            .burst
            .as_ref()
            .map_or(0, |b| b.len() + (0.15 * BASE_RATE) as usize)
            .min(n);
        let mut audio = vec![0.0f32; lead];
        audio.extend(self.voice(n - lead));
        if let Some(b) = &self.burst {
            for (a, v) in audio.iter_mut().zip(b) {
                *a = *v;
            }
        }
        let mut out = vec![Complex32::default(); n];
        let mut rate = BASE_RATE;
        match self.modulation {
            Modulation::Fm => {
                if let Some(hz) = self.ctcss_hz {
                    for (i, a) in audio.iter_mut().enumerate() {
                        *a += 0.08 * (TAU * hz * i as f64 / BASE_RATE).sin() as f32;
                    }
                }
                if let Some((code, inverted)) = self.dcs {
                    let code =
                        Dcs::from_octal(code).ok_or(TonesError::BadParameter(Param::DcsSignal))?;
                    let mut enc =
                        DcsEncoder::new(BASE_RATE, code, inverted, 0.1, self.dur_s, false)?;
                    let mut bits = vec![0.0f32; n];
                    enc.fill(&mut bits);
                    for (a, b) in audio.iter_mut().zip(&bits) {
                        *a += *b;
                    }
                }
                Fm::new(BASE_RATE, 5000.0 * self.deviation)
                    .and_then(|mut m| m.process(&audio, &mut out))
                    .map_err(ConfigError::from)?;
            }
            Modulation::WideFm => {
                rate = WIDE_RATE;
                let mut wide = Vec::with_capacity(4 * n + 64);
                Resampler::<f32>::new(BASE_RATE, WIDE_RATE)?.process(&audio, &mut wide);
                wide.resize(4 * n, 0.0);
                out = vec![Complex32::default(); 4 * n];
                Fm::new(WIDE_RATE, 80e3 * self.deviation)
                    .and_then(|mut m| m.process(&wide, &mut out))
                    .map_err(ConfigError::from)?;
            }
            Modulation::Am => {
                band_limit(&mut audio);
                let peak = audio.iter().fold(1e-6f32, |m, v| m.max(v.abs()));
                audio.iter_mut().for_each(|v| *v /= peak);
                Am::new(self.depth)
                    .and_then(|m| m.process(&audio, &mut out))
                    .map_err(ConfigError::from)?;
            }
            Modulation::Usb | Modulation::Lsb => {
                band_limit(&mut audio);
                let peak = audio.iter().fold(1e-6f32, |m, v| m.max(v.abs()));
                audio.iter_mut().for_each(|v| *v /= peak);
                let side = if self.modulation == Modulation::Usb {
                    Sideband::Upper
                } else {
                    Sideband::Lower
                };
                Ssb::new(side, BASE_RATE, 150.0, 60.0)
                    .and_then(|mut m| m.process(&audio, &mut out))
                    .map_err(ConfigError::from)?;
            }
        }
        // A 2 ms ramp at each end keeps the keying inside the channel.
        let ramp = ((0.002 * rate) as usize).min(out.len() / 2);
        let last = out.len().saturating_sub(1);
        for i in 0..ramp {
            let g = i as f32 / ramp as f32;
            out[i] *= g;
            out[last - i] *= g;
        }
        out.iter_mut().for_each(|v| *v *= self.amplitude);
        Ok((out, rate))
    }
}

/// The audio filter of an AM or SSB transmitter: 300 to 2800 Hz. Without it the noise under the speech reaches 24 kHz and
/// makes carriers beside the channel.
fn band_limit(audio: &mut [f32]) {
    let mut hp = Biquad::new(BiquadCoeffs::highpass(BASE_RATE, 300.0, 0.707));
    let mut lp = Butterworth4::lowpass(BASE_RATE, 2800.0);
    for v in audio {
        *v = lp.process_sample(hp.process_sample(*v));
    }
}

/// A scene: transmissions and noise in a span.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneConfig {
    /// The sample rate of the span, Hz.
    pub rate_hz: f64,
    /// The frequency at the centre of the span, Hz.
    pub centre_hz: f64,
    /// The length of the scene, s. It repeats after that.
    pub length_s: f64,
    /// The rms of the white noise in each of I and Q.
    pub noise_rms: f64,
    /// The transmissions.
    pub tx: Vec<Transmission>,
    /// Makes the noise repeatable.
    pub seed: u64,
}

/// The noise of a demonstration scene at `rate_hz` for a carrier-to-noise ratio of `cnr_db` in 10 kHz: rms in each of I and Q.
/// It is never below 0.3 / 128 (a radio of 8 bits needs some noise).
pub fn demo_noise_rms(rate_hz: f64, cnr_db: f64) -> f64 {
    (f64::from(DEMO_AMPLITUDE) * (rate_hz / 20e3).sqrt() * 10f64.powf(-cnr_db / 20.0))
        .max(0.3 / 128.0)
}

sdrkit_core::choice_enum! {
    /// The scene of the test signal.
    pub enum GenScene {
        /// The scene of the submode, or the mixed scene when the generator knows no submode.
        Auto = ("auto", "Automatic"),
        /// One narrow FM transmission with a tone and one on the upper sideband: every submode opens on one of them.
        Mixed = ("mixed", "Mixed"),
        /// AM, narrow FM, upper sideband and lower sideband on a 10 kHz raster.
        AmFmSsb = ("am_fm_ssb", "AM, FM and SSB"),
        /// Three narrow FM transmissions: with a CTCSS tone, with a DCS code, and without a tone.
        FmTones = ("fm_tones", "FM with tones"),
        /// One transmission on the upper sideband, 12 kHz below the centre.
        Sideband = ("sideband", "Upper sideband"),
    }
}

impl GenScene {
    /// The scene of `service`.
    pub fn of(service: Service) -> GenScene {
        match service {
            Service::Cb => GenScene::AmFmSsb,
            Service::Frs | Service::Pmr446 | Service::HamRepeaters => GenScene::FmTones,
            Service::HfVoice => GenScene::Sideband,
        }
    }
}

/// The test scene `kind` at `rate_hz` (`Auto` is the mixed scene). The frequencies of its transmissions are offsets from
/// the centre of the capture: its centre frequency is 0. Each scene is 6 s long and has a transmission on the air 2 s
/// after its start.
pub fn demo(kind: GenScene, rate_hz: f64, cnr_db: f64, seed: u64) -> SceneConfig {
    let tx = |offset_hz: f64, start: f64, dur: f64| Transmission {
        amplitude: DEMO_AMPLITUDE,
        ..Transmission::on(offset_hz).at(start, dur)
    };
    let mut list = match kind {
        GenScene::Auto | GenScene::Mixed => vec![
            Transmission {
                deviation: 0.5,
                ctcss_hz: Some(88.5),
                ..tx(100e3, 0.3, 2.6)
            },
            Transmission {
                modulation: Modulation::Usb,
                amplitude: 0.15,
                ..tx(-60e3, 1.0, 3.0)
            },
        ],
        GenScene::AmFmSsb => vec![
            Transmission {
                modulation: Modulation::Am,
                ..tx(-30e3, 0.3, 2.2)
            },
            Transmission {
                deviation: 0.6,
                ..tx(-150e3, 1.5, 2.0)
            },
            Transmission {
                modulation: Modulation::Usb,
                ..tx(-60e3, 3.2, 2.4)
            },
            Transmission {
                modulation: Modulation::Lsb,
                ..tx(-50e3, 4.4, 1.4)
            },
        ],
        GenScene::FmTones => vec![
            Transmission {
                deviation: 0.5,
                ctcss_hz: Some(88.5),
                ..tx(-81.25e3, 0.3, 2.0)
            },
            Transmission {
                deviation: 0.5,
                dcs: Some(("023", false)),
                ..tx(93.75e3, 1.8, 2.0)
            },
            Transmission {
                deviation: 0.5,
                ..tx(168.75e3, 3.8, 1.8)
            },
        ],
        GenScene::Sideband => vec![Transmission {
            modulation: Modulation::Usb,
            amplitude: 0.15,
            ..tx(-crate::plans::SIDEBAND_TUNE_OFFSET_HZ, 0.3, 4.5)
        }],
    };
    // Each transmission has its own speaker.
    for (i, t) in list.iter_mut().enumerate() {
        t.seed = seed.wrapping_add(i as u64);
    }
    SceneConfig {
        rate_hz,
        centre_hz: 0.0,
        length_s: 6.0,
        noise_rms: demo_noise_rms(rate_hz, cnr_db),
        tx: list,
        seed,
    }
}

/// The test scene of the MDC-1200 stack at `rate_hz`, 6 s long: two narrow FM transmissions at fixed offsets from the centre
/// of the capture, each with a PTT ID burst and then speech. The second one starts at 1.55 s: a receiver that gives its blocks out 0.3 s
/// late has its burst open 2 s after the start.
pub fn mdc_demo(rate_hz: f64, cnr_db: f64, seed: u64) -> SceneConfig {
    let tx = |offset_hz: f64, start: f64, dur: f64, arg: u8, unit: u16, seed: u64| Transmission {
        amplitude: DEMO_AMPLITUDE,
        deviation: 0.7,
        burst: Some(mdc::burst_audio(
            &mdc::Packet::new(0x01, arg, unit),
            BASE_RATE,
            0.2,
            7,
        )),
        seed,
        ..Transmission::on(offset_hz).at(start, dur)
    };
    SceneConfig {
        rate_hz,
        centre_hz: 0.0,
        length_s: 6.0,
        noise_rms: demo_noise_rms(rate_hz, cnr_db),
        tx: vec![
            tx(-37.5e3, 0.3, 2.2, 0x80, 0x1A2B, seed),
            tx(62.5e3, 1.55, 2.4, 0x00, 0xC0DE, seed.wrapping_add(1)),
        ],
        seed,
    }
}

/// The time of unmodulated carrier before the burst in the FleetSync test scene, s. A choice of the scene: the
/// specification's test signal starts with the burst, and no public document gives the delay of a radio.
pub const FLEETSYNC_DELAY_S: f64 = 0.3;

/// The test scene of the FleetSync stack at `rate_hz`, 6 s long: two narrow FM transmissions at the offsets of
/// [`mdc_demo`], each a burst of `fleet` and `unit` (the second one of the next unit) at `bit_rate` and then speech. The
/// first burst is a plain frame and the second a FleetSync II frame, unless `kind` names one type for both.
pub fn fleetsync_demo(
    rate_hz: f64,
    cnr_db: f64,
    seed: u64,
    fleet: u16,
    unit: u16,
    bit_rate: fleetsync::BitRate,
    kind: Option<fleetsync::Kind>,
) -> SceneConfig {
    let tx = |offset_hz: f64, start: f64, dur: f64, unit: u16, k: fleetsync::Kind, seed: u64| {
        let m = fleetsync::Message::new(fleet, unit, 0, 0).unwrap_or_default();
        let o = fleetsync::BurstOptions {
            kind: kind.unwrap_or(k),
            bit_rate,
            amplitude: 0.2,
            ..fleetsync::BurstOptions::default()
        };
        Transmission {
            amplitude: DEMO_AMPLITUDE,
            deviation: 0.7,
            burst: Some({
                // Unmodulated carrier first: the squelch of the receiver opens before the burst.
                let mut a = vec![0.0; (FLEETSYNC_DELAY_S * BASE_RATE) as usize];
                a.extend(fleetsync::burst_audio(&m, BASE_RATE, &o));
                a
            }),
            seed,
            ..Transmission::on(offset_hz).at(start, dur)
        }
    };
    let second = if unit < fleetsync::UNIT_WIRE.1 {
        unit + 1
    } else {
        unit - 1
    };
    SceneConfig {
        rate_hz,
        centre_hz: 0.0,
        length_s: 6.0,
        noise_rms: demo_noise_rms(rate_hz, cnr_db),
        tx: vec![
            tx(-37.5e3, 0.3, 2.2, unit, fleetsync::Kind::Plain, seed),
            tx(
                62.5e3,
                1.55,
                2.4,
                second,
                fleetsync::Kind::Fs2,
                seed.wrapping_add(1),
            ),
        ],
        seed,
    }
}

/// The carrier amplitude of the wide FM test scene.
pub const WIDE_AMPLITUDE: f32 = 0.3;

/// The test scene of the wireless microphone finder at `rate_hz`: three wide FM carriers at fixed offsets from the centre
/// of the capture, on the air for the whole scene. It is 6 s long, and shorter above 3.3 Msps (20 million samples).
pub fn wide_demo(rate_hz: f64, cnr_db: f64, seed: u64) -> SceneConfig {
    let length_s = (20e6 / rate_hz).min(6.0);
    let tx = |offset_hz: f64, deviation: f64, audio: Audio| Transmission {
        modulation: Modulation::WideFm,
        amplitude: WIDE_AMPLITUDE,
        deviation,
        audio,
        seed,
        ..Transmission::on(offset_hz).at(0.0, length_s)
    };
    SceneConfig {
        rate_hz,
        centre_hz: 0.0,
        length_s,
        noise_rms: demo_noise_rms(rate_hz, cnr_db),
        tx: vec![
            tx(-0.6e6, 1.0, Audio::Tones(vec![440.0, 880.0])),
            tx(0.85e6, 0.7, Audio::Speech),
            tx(0.25e6, 0.5, Audio::Tones(vec![1000.0])),
        ],
        seed,
    }
}

/// One transmission as a stream at the rate of the span.
struct Stream {
    rate: f64,
    base: Arc<Vec<Complex32>>,
    at: usize,
    resampler: Resampler<Complex32>,
    ready: Vec<Complex32>,
    pos: usize,
}

impl Generator for Stream {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut done = 0;
        while done < out.len() {
            if self.pos == self.ready.len() {
                if self.at >= self.base.len() {
                    break;
                }
                let end = (self.at + 256).min(self.base.len());
                self.ready.clear();
                self.pos = 0;
                self.resampler
                    .process(&self.base[self.at..end], &mut self.ready);
                self.at = end;
                continue;
            }
            let n = (out.len() - done).min(self.ready.len() - self.pos);
            out[done..done + n].copy_from_slice(&self.ready[self.pos..self.pos + n]);
            self.pos += n;
            done += n;
        }
        done
    }
}

/// The scene generator.
pub struct SceneGenerator {
    cfg: SceneConfig,
    info: SignalInfo,
    parts: Vec<(Arc<Vec<Complex32>>, f64)>,
    scene: Scene,
    noise: Option<Awgn>,
}

impl SceneGenerator {
    /// A generator of the scene `cfg`.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] if a transmission cannot be made or the rate is refused.
    pub fn new(cfg: SceneConfig) -> Result<SceneGenerator, ConfigError> {
        let mut parts = Vec::with_capacity(cfg.tx.len());
        for t in &cfg.tx {
            let (x, rate) = t.baseband()?;
            parts.push((Arc::new(x), rate));
        }
        let noise = if cfg.noise_rms > 0.0 {
            Some(Awgn::with_power(
                2.0 * cfg.noise_rms * cfg.noise_rms,
                cfg.seed,
            )?)
        } else {
            None
        };
        let scene = Self::build(&cfg, &parts)?;
        Ok(SceneGenerator {
            info: SignalInfo::new(10e3).with_power(f64::from(DEMO_AMPLITUDE * DEMO_AMPLITUDE)),
            cfg,
            parts,
            scene,
            noise,
        })
    }

    fn build(
        cfg: &SceneConfig,
        parts: &[(Arc<Vec<Complex32>>, f64)],
    ) -> Result<Scene, ConfigError> {
        let mut scene = Scene::new(cfg.rate_hz)
            .and_then(|s| s.with_duration(cfg.length_s))
            .map_err(ConfigError::from)?;
        for (t, (base, rate)) in cfg.tx.iter().zip(parts) {
            let stream = Stream {
                rate: cfg.rate_hz,
                base: Arc::clone(base),
                at: 0,
                resampler: Resampler::new(*rate, cfg.rate_hz)?,
                ready: Vec::new(),
                pos: 0,
            };
            scene
                .add_stream(
                    t.start_s,
                    None,
                    t.freq_hz - cfg.centre_hz + t.offset_hz,
                    0.0,
                    Box::new(stream),
                )
                .map_err(ConfigError::from)?;
        }
        Ok(scene)
    }

    /// The same generator for a scene whose transmissions are on the air at once: the signal power it declares is the sum
    /// of the carrier powers, in `bandwidth_hz`. A host that sets the level then keeps the sum inside full scale.
    #[must_use]
    pub fn with_total_power(mut self, bandwidth_hz: f64) -> SceneGenerator {
        let power: f64 = self
            .cfg
            .tx
            .iter()
            .map(|t| f64::from(t.amplitude * t.amplitude))
            .sum();
        self.info = SignalInfo::new(bandwidth_hz).with_power(power);
        self
    }

    /// The scene.
    pub fn config(&self) -> &SceneConfig {
        &self.cfg
    }
}

impl Generator for SceneGenerator {
    fn rate_hz(&self) -> f64 {
        self.cfg.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut done = 0;
        while done < out.len() {
            let n = self.scene.generate(&mut out[done..]);
            done += n;
            if done < out.len() {
                match Self::build(&self.cfg, &self.parts) {
                    Ok(s) if self.cfg.length_s > 0.0 => self.scene = s,
                    _ => break,
                }
            }
        }
        if let Some(noise) = &mut self.noise {
            noise.process(&mut out[..done]);
        }
        done
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
    /// The scene places its transmissions itself.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
