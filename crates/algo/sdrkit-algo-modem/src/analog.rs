//! Analogue voice demodulators: narrow and wide FM, AM, USB, LSB and CW from complex baseband to real audio. One type covers
//! every stack. A configuration sets the filters, the decimation, the gain and the output limits.

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::agc::Agc;
use sdrkit_algo_dsp::dc::RealDcBlocker;
use sdrkit_algo_dsp::decim::{ComplexDecimator, FirDecimator};
use sdrkit_algo_dsp::fir::{bandpass_complex, bandpass_real, design_by_taps, lowpass, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{Deemphasis, OnePole};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;

/// The demodulator kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AnalogMode {
    /// Narrow FM.
    #[default]
    Nfm,
    /// Wide FM (mono).
    Wfm,
    /// AM by envelope.
    Am,
    /// Upper sideband.
    Usb,
    /// Lower sideband.
    Lsb,
    /// CW: the carrier beats with an oscillator at the pitch.
    Cw,
}

impl AnalogMode {
    /// True for narrow and wide FM.
    pub fn is_fm(self) -> bool {
        matches!(self, AnalogMode::Nfm | AnalogMode::Wfm)
    }

    /// True for USB, LSB and CW: the audio is the real part of the complex signal.
    pub fn is_sideband(self) -> bool {
        matches!(self, AnalogMode::Usb | AnalogMode::Lsb | AnalogMode::Cw)
    }
}

/// A low-pass FIR recipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LowPass {
    /// A Kaiser (beta 7) windowed sinc of `taps` taps with its -6 dB point at `cutoff_hz`, unity gain at DC.
    Taps {
        /// Number of taps.
        taps: usize,
        /// The -6 dB point, Hz.
        cutoff_hz: f64,
    },
    /// A Kaiser design from the band edges: flat to `pass_hz`, `atten_db` down from `stop_hz`.
    Edges {
        /// The pass band edge, Hz.
        pass_hz: f64,
        /// The stop band edge, Hz.
        stop_hz: f64,
        /// The stop band attenuation, dB.
        atten_db: f64,
    },
}

impl LowPass {
    fn taps(&self, rate: f64) -> Result<Vec<f32>, ModemError> {
        Ok(match *self {
            LowPass::Taps { taps, cutoff_hz } => {
                if taps == 0 {
                    return Param::Taps.at_least(1.0, 0.0);
                }
                Param::Cutoff.check_positive(cutoff_hz)?;
                design_by_taps(taps, (cutoff_hz / rate).min(0.5), Window::kaiser(7.0))?
            }
            LowPass::Edges {
                pass_hz,
                stop_hz,
                atten_db,
            } => lowpass(rate, pass_hz, stop_hz, atten_db)?,
        })
    }
}

/// A band-pass FIR recipe: a Kaiser design with its -6 dB points `transition_hz / 2` outside `low_hz` and `high_hz`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandPass {
    /// The low edge of the flat part, Hz.
    pub low_hz: f64,
    /// The high edge of the flat part, Hz.
    pub high_hz: f64,
    /// The width of each transition band, Hz.
    pub transition_hz: f64,
    /// The stop band attenuation, dB.
    pub atten_db: f64,
}

/// The real filter on the detected audio of FM and AM.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioFilter {
    /// A low-pass.
    LowPass(LowPass),
    /// A band-pass.
    BandPass(BandPass),
}

/// The automatic gain control: an envelope follower with a fast attack and a slow decay (`sdrkit_algo_dsp::agc::Agc`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgcConfig {
    /// The output envelope the gain aims at.
    pub target: f32,
    /// The attack time constant, seconds.
    pub attack_s: f64,
    /// The decay time constant, seconds.
    pub decay_s: f64,
    /// The largest gain.
    pub max_gain: f32,
}

impl AgcConfig {
    /// Time constants from per-sample smoothing factors `k` (`y += k (x - y)`) at `rate`: `tau = -1 / (rate ln(1 - k))`.
    pub fn from_factors(
        rate: f64,
        target: f32,
        attack_k: f64,
        decay_k: f64,
        max_gain: f32,
    ) -> Self {
        let tau = |k: f64| -1.0 / (rate * (1.0 - k).ln());
        AgcConfig {
            target,
            attack_s: tau(attack_k),
            decay_s: tau(decay_k),
            max_gain,
        }
    }
}

/// The gain stage after the detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gain {
    /// A fixed linear gain.
    Manual(f32),
    /// An automatic gain control.
    Auto(AgcConfig),
}

/// Everything an [`AnalogDemod`] needs.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalogConfig {
    /// The demodulator.
    pub mode: AnalogMode,
    /// The input rate, Hz.
    pub input_rate: f64,
    /// The whole factor from the input rate to the audio rate (1 or more).
    pub decimation: usize,
    /// FM and AM: a complex low-pass on the input before the detector.
    pub channel: Option<LowPass>,
    /// USB and LSB: the band-pass of the upper sideband (mirrored for LSB). It also decimates.
    pub sideband: Option<BandPass>,
    /// FM and AM: the real filter on the detected audio. It also decimates.
    pub audio: Option<AudioFilter>,
    /// FM: the full deviation, Hz. It returns an output of 0.5.
    pub fm_deviation_hz: f64,
    /// AM: the time constant of the carrier average, seconds.
    pub am_carrier_s: f64,
    /// AM: divide the detected envelope by the carrier, so a modulation index `m` produces an amplitude of `m / 2`.
    pub am_normalise: bool,
    /// CW: the pitch of the beat oscillator, Hz.
    pub cw_pitch_hz: f64,
    /// The gain stage.
    pub gain: Gain,
    /// A DC blocker on the audio with this corner, Hz.
    pub dc_block_hz: Option<f64>,
    /// A de-emphasis with this time constant, seconds (ITU-R BS.450).
    pub deemphasis_s: Option<f64>,
    /// The output is held within plus and minus this value.
    pub clamp: Option<f32>,
}

impl AnalogConfig {
    /// A configuration for `mode` at `input_rate` with no filters, no decimation, unit gain, 5 kHz deviation (75 kHz for WFM),
    /// a 50 ms AM carrier average and a 700 Hz CW pitch.
    pub fn new(mode: AnalogMode, input_rate: f64) -> Self {
        AnalogConfig {
            mode,
            input_rate,
            decimation: 1,
            channel: None,
            sideband: None,
            audio: None,
            fm_deviation_hz: if mode == AnalogMode::Wfm {
                75_000.0
            } else {
                5_000.0
            },
            am_carrier_s: 0.05,
            am_normalise: false,
            cw_pitch_hz: 700.0,
            gain: Gain::Manual(1.0),
            dc_block_hz: None,
            deemphasis_s: None,
            clamp: None,
        }
    }

    /// The audio rate, Hz.
    pub fn output_rate(&self) -> f64 {
        self.input_rate / self.decimation.max(1) as f64
    }
}

/// The detector with its filters.
enum Path {
    Fm {
        channel: Option<Fir<Complex32>>,
        disc: Discriminator,
        audio: FirDecimator<f32>,
    },
    Am {
        channel: Option<Fir<Complex32>>,
        carrier: OnePole,
        started: bool,
        audio: FirDecimator<f32>,
    },
    Sideband {
        filter: Option<ComplexDecimator>,
        bfo: Option<Nco>,
    },
}

enum GainStage {
    Manual(f32),
    Auto(Agc),
}

/// An analogue demodulator: complex samples in, real audio and its in-band power out.
///
/// The steps are: non-finite samples become 0; FM and AM take the channel low-pass, the detector, then the audio filter with the
/// decimation; USB and LSB take the sideband filter with the decimation, then the real part (CW first beats with the oscillator).
/// Then come the gain, the DC blocker, the de-emphasis and the clamp. For USB, LSB and CW the automatic gain follows the complex
/// envelope. For FM and AM it follows the audio. Nothing allocates after the first call of each input size.
///
/// ```
/// use sdrkit_algo_modem::analog::{AnalogConfig, AnalogDemod, AnalogMode};
/// use sdrkit_algo_modem::Complex32;
/// // NFM at 48 kHz: a steady 2.5 kHz offset is half the full deviation, so the output is 0.25.
/// let mut d = AnalogDemod::new(AnalogConfig::new(AnalogMode::Nfm, 48e3)).unwrap();
/// let x: Vec<Complex32> = (0..480).map(|n| Complex32::from_polar(1.0, (std::f64::consts::TAU * 2500.0 * n as f64 / 48e3) as f32)).collect();
/// let mut audio = Vec::new();
/// d.process(&x, &mut audio, None);
/// assert!((audio[100] - 0.25).abs() < 1e-3);
/// ```
pub struct AnalogDemod {
    cfg: AnalogConfig,
    path: Path,
    gain: GainStage,
    dc: Option<RealDcBlocker>,
    deemph: Option<Deemphasis>,
    offset_sum: f64,
    offset_n: u64,
    clean: Vec<Complex32>,
}

impl AnalogDemod {
    /// A demodulator for `cfg`. Errors when a rate, the decimation, a filter recipe, a time constant or the gain is out of range,
    /// or when the decimation is above 1 and no filter carries it (the sideband filter, or the audio filter for FM and AM).
    pub fn new(cfg: AnalogConfig) -> Result<Self, ModemError> {
        let rate = cfg.input_rate;
        Param::Rate.check_positive(rate)?;
        if cfg.decimation == 0 {
            return Param::Decimation.at_least(1.0, 0.0);
        }
        let out_rate = cfg.output_rate();
        let d = cfg.decimation;
        let channel = |c: &Option<LowPass>| -> Result<Option<Fir<Complex32>>, ModemError> {
            c.as_ref()
                .map(|lp| lp.taps(rate).map(|t| Fir::new(&t)))
                .transpose()
        };
        let audio = |a: &Option<AudioFilter>| -> Result<FirDecimator<f32>, ModemError> {
            let taps = match a {
                None if d > 1 => return Param::AudioFilter.conflict(Param::Decimation),
                None => vec![1.0],
                Some(AudioFilter::LowPass(lp)) => lp.taps(rate)?,
                Some(AudioFilter::BandPass(b)) => {
                    bandpass_real(rate, b.low_hz, b.high_hz, b.transition_hz, b.atten_db)?
                }
            };
            Ok(FirDecimator::new(&taps, d))
        };
        let path = match cfg.mode {
            AnalogMode::Nfm | AnalogMode::Wfm => {
                if cfg.fm_deviation_hz.is_nan() || cfg.fm_deviation_hz <= 0.0 {
                    return Param::Deviation.positive(cfg.fm_deviation_hz);
                }
                Path::Fm {
                    channel: channel(&cfg.channel)?,
                    disc: Discriminator::new(rate, 2.0 * cfg.fm_deviation_hz)?,
                    audio: audio(&cfg.audio)?,
                }
            }
            AnalogMode::Am => {
                if !(cfg.am_carrier_s > 0.0 && cfg.am_carrier_s.is_finite()) {
                    return Param::CarrierTime.positive(cfg.am_carrier_s);
                }
                Path::Am {
                    channel: channel(&cfg.channel)?,
                    carrier: OnePole::with_time_constant(rate, cfg.am_carrier_s),
                    started: false,
                    audio: audio(&cfg.audio)?,
                }
            }
            AnalogMode::Usb | AnalogMode::Lsb | AnalogMode::Cw => {
                let filter = match &cfg.sideband {
                    None if d > 1 => return Param::SidebandFilter.conflict(Param::Decimation),
                    None => None,
                    Some(b) => {
                        let mut t = bandpass_complex(
                            rate,
                            b.low_hz,
                            b.high_hz,
                            b.transition_hz,
                            b.atten_db,
                        )?;
                        if cfg.mode == AnalogMode::Lsb {
                            t.iter_mut().for_each(|v| *v = v.conj());
                        }
                        Some(ComplexDecimator::new(&t, d)?)
                    }
                };
                let bfo = (cfg.mode == AnalogMode::Cw).then(|| Nco::new(out_rate, cfg.cw_pitch_hz));
                Path::Sideband { filter, bfo }
            }
        };
        let gain = Self::gain_stage(cfg.gain, out_rate)?;
        let dc = match cfg.dc_block_hz {
            Some(f) if f > 0.0 && f < out_rate / 4.0 => Some(RealDcBlocker::new(out_rate, f)),
            Some(f) => return Param::DcCorner.range(0.0, out_rate / 4.0, f),
            None => None,
        };
        let deemph = Self::deemphasis(cfg.deemphasis_s, out_rate)?;
        if cfg.clamp.is_some_and(|c| c.is_nan() || c <= 0.0) {
            return Param::Clamp.positive(cfg.clamp.unwrap_or(f32::NAN) as f64);
        }
        Ok(AnalogDemod {
            cfg,
            path,
            gain,
            dc,
            deemph,
            offset_sum: 0.0,
            offset_n: 0,
            clean: Vec::new(),
        })
    }

    fn gain_stage(g: Gain, rate: f64) -> Result<GainStage, ModemError> {
        Ok(match g {
            Gain::Manual(v) if v.is_finite() => GainStage::Manual(v),
            Gain::Manual(_) => {
                return Err(ModemError::NotFinite {
                    param: Param::Input,
                })
            }
            Gain::Auto(a) => {
                let ok = |t: f64| t > 0.0 && t.is_finite();
                Param::AgcTarget.check_positive(a.target as f64)?;
                Param::AgcGain.check_positive(a.max_gain as f64)?;
                if !ok(a.attack_s) {
                    return Param::AgcTime.positive(a.attack_s);
                }
                if !ok(a.decay_s) {
                    return Param::AgcTime.positive(a.decay_s);
                }
                GainStage::Auto(Agc::new(rate, a.target, a.attack_s, a.decay_s, a.max_gain))
            }
        })
    }

    fn deemphasis(tau: Option<f64>, rate: f64) -> Result<Option<Deemphasis>, ModemError> {
        match tau {
            None => Ok(None),
            Some(t) if t > 0.0 && t.is_finite() => Ok(Some(Deemphasis::new(t, rate))),
            Some(t) => Param::Deemphasis.positive(t),
        }
    }

    /// The configuration in use.
    pub fn config(&self) -> &AnalogConfig {
        &self.cfg
    }

    /// The demodulator kind.
    pub fn mode(&self) -> AnalogMode {
        self.cfg.mode
    }

    /// The audio rate, Hz.
    pub fn output_rate(&self) -> f64 {
        self.cfg.output_rate()
    }

    /// Changes the gain stage. The envelope of a new AGC starts again. Errors as [`AnalogDemod::new`] does for the gain.
    pub fn set_gain(&mut self, gain: Gain) -> Result<(), ModemError> {
        self.gain = Self::gain_stage(gain, self.output_rate())?;
        self.cfg.gain = gain;
        Ok(())
    }

    /// AM: divide the envelope by the carrier or not.
    pub fn set_am_normalise(&mut self, on: bool) {
        self.cfg.am_normalise = on;
    }

    /// Changes the de-emphasis (`None` is off). Errors for a time that is not positive.
    pub fn set_deemphasis(&mut self, tau_s: Option<f64>) -> Result<(), ModemError> {
        self.deemph = Self::deemphasis(tau_s, self.output_rate())?;
        self.cfg.deemphasis_s = tau_s;
        Ok(())
    }

    /// CW: changes the pitch, Hz. The oscillator's phase runs on.
    pub fn set_pitch(&mut self, hz: f64) {
        self.cfg.cw_pitch_hz = hz;
        if let Path::Sideband { bfo: Some(b), .. } = &mut self.path {
            b.set_freq(hz);
        }
    }

    /// FM: the mean frequency of the input since the last call, Hz, over the samples that are not 0. Other modes return 0.
    pub fn take_offset_hz(&mut self) -> f64 {
        let m = if self.offset_n > 0 {
            self.offset_sum / self.offset_n as f64
        } else {
            0.0
        };
        self.offset_sum = 0.0;
        self.offset_n = 0;
        m
    }

    /// Demodulates `x` and appends the audio to `audio`. If `power` is given, it gets the in-band power `|y|^2` of each audio
    /// sample before the gain: the sideband sample, or the channel sample the decimator kept.
    pub fn process(
        &mut self,
        x: &[Complex32],
        audio: &mut Vec<f32>,
        mut power: Option<&mut Vec<f32>>,
    ) {
        self.clean.clear();
        self.clean.extend(x.iter().map(|&s| {
            if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            }
        }));
        let start = audio.len();
        let normalise = self.cfg.am_normalise;
        match &mut self.path {
            Path::Sideband { filter, bfo } => {
                let mut one = |mut z: Complex32, gain: &mut GainStage| {
                    if let Some(p) = power.as_deref_mut() {
                        p.push(z.norm_sqr());
                    }
                    if let Some(b) = bfo.as_mut() {
                        z *= b.next_sample();
                    }
                    audio.push(match gain {
                        GainStage::Manual(g) => z.re * *g,
                        GainStage::Auto(agc) => {
                            let mut v = [z];
                            agc.process(&mut v);
                            v[0].re
                        }
                    });
                };
                for &s in &self.clean {
                    match filter.as_mut() {
                        Some(f) => {
                            if let Some(y) = f.push(s) {
                                one(y, &mut self.gain);
                            }
                        }
                        None => one(s, &mut self.gain),
                    }
                }
            }
            Path::Fm {
                channel,
                disc,
                audio: dec,
            } => {
                let to_hz = 2.0 * self.cfg.fm_deviation_hz;
                let (mut sum, mut n) = (0.0f64, 0u64);
                for &s in &self.clean {
                    let y = match channel.as_mut() {
                        Some(c) => c.process_sample(s),
                        None => s,
                    };
                    let v = disc.process_sample(y);
                    if y.norm_sqr() > 0.0 {
                        sum += v as f64 * to_hz;
                        n += 1;
                    }
                    if let Some(a) = dec.push(v) {
                        audio.push(a);
                        if let Some(p) = power.as_deref_mut() {
                            p.push(y.norm_sqr());
                        }
                    }
                }
                self.offset_sum += sum;
                self.offset_n += n;
            }
            Path::Am {
                channel,
                carrier,
                started,
                audio: dec,
            } => {
                for &s in &self.clean {
                    let y = match channel.as_mut() {
                        Some(c) => c.process_sample(s),
                        None => s,
                    };
                    let e = y.norm();
                    if !*started {
                        carrier.set_value(e);
                        *started = true;
                    }
                    let c = carrier.process_sample(e);
                    let v = if !normalise {
                        e - c
                    } else if c > 1e-12 {
                        (e - c) / c * 0.5
                    } else {
                        0.0
                    };
                    if let Some(a) = dec.push(v) {
                        audio.push(a);
                        if let Some(p) = power.as_deref_mut() {
                            p.push(y.norm_sqr());
                        }
                    }
                }
            }
        }
        let out = &mut audio[start..];
        if !self.cfg.mode.is_sideband() {
            match &mut self.gain {
                GainStage::Manual(g) if *g != 1.0 => out.iter_mut().for_each(|v| *v *= *g),
                GainStage::Manual(_) => {}
                GainStage::Auto(agc) => agc.process_real(out),
            }
        }
        if let Some(d) = &mut self.dc {
            d.process(out);
        }
        if let Some(d) = &mut self.deemph {
            d.process(out);
        }
        if let Some(c) = self.cfg.clamp {
            out.iter_mut().for_each(|v| *v = v.clamp(-c, c));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn tone(rate: f64, f: f64, a: f32, n: usize) -> Vec<Complex32> {
        (0..n)
            .map(|i| Complex32::from_polar(a, (TAU * f * i as f64 / rate) as f32))
            .collect()
    }

    fn amp_at(x: &[f32], rate: f64, f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in x.iter().enumerate() {
            let p = TAU * f * i as f64 / rate;
            re += v as f64 * p.cos();
            im -= v as f64 * p.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    fn ssb(mode: AnalogMode) -> AnalogConfig {
        let mut c = AnalogConfig::new(mode, 24e3);
        c.decimation = 3;
        c.sideband = Some(BandPass {
            low_hz: 340.0,
            high_hz: 3660.0,
            transition_hz: 280.0,
            atten_db: 69.0,
        });
        c.gain = Gain::Auto(AgcConfig::from_factors(8e3, 0.5, 0.01, 0.0001, 1e4));
        c.clamp = Some(1.0);
        c
    }

    #[test]
    fn usb_passes_the_upper_sideband_and_lsb_the_lower() {
        let up = tone(24e3, 1500.0, 0.1, 48_000);
        let down: Vec<Complex32> = up.iter().map(|c| c.conj()).collect();
        let run = |mode, x: &[Complex32]| {
            let mut d = AnalogDemod::new(ssb(mode)).unwrap();
            let (mut a, mut p) = (Vec::new(), Vec::new());
            d.process(x, &mut a, Some(&mut p));
            (a, p)
        };
        let (a, p) = run(AnalogMode::Usb, &up);
        assert_eq!(a.len(), 16_000);
        assert_eq!(p.len(), 16_000);
        // The AGC holds the envelope at 0.5, and the in-band power is 0.1^2 times the pass band gain of 1.
        assert!((amp_at(&a[8000..], 8e3, 1500.0) - 0.5).abs() < 0.02);
        assert!((p[12_000] - 0.01).abs() < 2e-4);
        let (_, q) = run(AnalogMode::Usb, &down);
        assert!(q[8000..].iter().sum::<f32>() / 8000.0 < 1e-8);
        let (b, _) = run(AnalogMode::Lsb, &down);
        assert!((amp_at(&b[8000..], 8e3, 1500.0) - 0.5).abs() < 0.02);
    }

    #[test]
    fn fm_full_deviation_returns_half_scale_and_the_offset() {
        // A 1 kHz tone at 2.5 kHz peak deviation through NFM (5 kHz full) produces an amplitude of 0.25.
        let rate = 48e3;
        let mut ph = 0.0f64;
        let x: Vec<Complex32> = (0..48_000)
            .map(|i| {
                ph += 2500.0 * (TAU * 1000.0 * i as f64 / rate).cos() / rate + 300.0 / rate;
                Complex32::from_polar(1.0, (TAU * ph) as f32)
            })
            .collect();
        let mut d = AnalogDemod::new(AnalogConfig::new(AnalogMode::Nfm, rate)).unwrap();
        let mut a = Vec::new();
        d.process(&x, &mut a, None);
        assert!((amp_at(&a[4800..], rate, 1000.0) - 0.25).abs() < 1e-3);
        assert!((d.take_offset_hz() - 300.0).abs() < 1.0);
        assert_eq!(d.take_offset_hz(), 0.0);
    }

    #[test]
    fn am_index_returns_its_ratio() {
        // m = 0.6 at 1 kHz on a carrier of 0.2: normalised, the audio amplitude is m / 2 = 0.3.
        let rate = 48e3;
        let x: Vec<Complex32> = (0..96_000)
            .map(|i| {
                let m = 1.0 + 0.6 * (TAU * 1000.0 * i as f64 / rate).cos();
                Complex32::new((0.2 * m) as f32, 0.0)
            })
            .collect();
        let mut c = AnalogConfig::new(AnalogMode::Am, rate);
        c.am_normalise = true;
        let mut d = AnalogDemod::new(c.clone()).unwrap();
        let mut a = Vec::new();
        d.process(&x, &mut a, None);
        assert!((amp_at(&a[48_000..], rate, 1000.0) - 0.3).abs() < 3e-3);
        // Not normalised: the amplitude is m times the carrier, 0.12, times the gain.
        c.am_normalise = false;
        c.gain = Gain::Manual(2.0);
        let mut d = AnalogDemod::new(c).unwrap();
        let mut a = Vec::new();
        d.process(&x, &mut a, None);
        assert!((amp_at(&a[48_000..], rate, 1000.0) - 0.24).abs() < 3e-3);
    }

    #[test]
    fn cw_beats_at_the_pitch() {
        let rate = 48e3;
        let mut c = AnalogConfig::new(AnalogMode::Cw, rate);
        c.cw_pitch_hz = 600.0;
        let mut d = AnalogDemod::new(c).unwrap();
        let mut a = Vec::new();
        d.process(&tone(rate, 0.0, 0.5, 4800), &mut a, None);
        assert!((amp_at(&a, rate, 600.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn decimation_needs_a_filter_and_bad_values_fail() {
        let mut c = AnalogConfig::new(AnalogMode::Usb, 24e3);
        c.decimation = 3;
        assert!(AnalogDemod::new(c).is_err());
        let mut c = AnalogConfig::new(AnalogMode::Nfm, 24e3);
        c.decimation = 3;
        assert!(AnalogDemod::new(c.clone()).is_err());
        c.decimation = 1;
        c.deemphasis_s = Some(-1.0);
        assert!(AnalogDemod::new(c.clone()).is_err());
        c.deemphasis_s = None;
        c.gain = Gain::Manual(f32::NAN);
        assert_eq!(
            AnalogDemod::new(c).err(),
            Some(ModemError::NotFinite {
                param: Param::Input
            })
        );
    }

    #[test]
    fn bad_samples_become_silence() {
        let mut d = AnalogDemod::new(AnalogConfig::new(AnalogMode::Am, 8e3)).unwrap();
        let mut a = Vec::new();
        d.process(&[Complex32::new(f32::NAN, 0.0); 16], &mut a, None);
        assert!(a.iter().all(|v| *v == 0.0));
    }
}
