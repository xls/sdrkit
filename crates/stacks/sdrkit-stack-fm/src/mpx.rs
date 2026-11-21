//! The FM broadcast multiplex and its modulator, shared by the test signal ([`crate::generator`]) and the sender
//! ([`crate::transmit`]): left and right audio (two tones, or live audio through pre-emphasis, a 15 kHz filter and a limiter),
//! the 19 kHz pilot, the 38 kHz difference subcarrier and RDS on 57 kHz, frequency-modulated at the output rate.
//! Standard: ITU-R BS.450 (multiplex, pilot), IEC 62106 (RDS subcarrier).

use crate::rds::encode::{RdsContent, RdsEncoder, RdsWave};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::iir::PreEmphasis;
use sdrkit_algo_dsp::loopfilter::PhaseAcc;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::sample::Stereo;
use sdrkit_algo_dsp::{DspError, Param};
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The pilot frequency, Hz.
pub const PILOT_HZ: f64 = 19_000.0;
/// The rate of live audio, Hz.
pub const AUDIO_RATE: f64 = 48_000.0;
/// The top of the audio band, Hz.
pub const AUDIO_TOP_HZ: f64 = 15_000.0;
/// Where the audio filter is at least 50 dB down, Hz: the pilot.
const AUDIO_STOP_HZ: f64 = 19_000.0;
/// The rate the audio is limited at, before it is drawn onto the output rate by straight lines, Hz.
pub const LIMIT_RATE: f64 = 384_000.0;
/// The look-ahead of the limiter, s.
pub const LIMIT_AHEAD_S: f64 = 0.001;
/// The release of the limiter, s.
pub const LIMIT_RELEASE_S: f64 = 0.1;
/// The centres of the SCA subcarriers, Hz.
pub const SCA_HZ: [f64; 2] = [67e3, 92e3];
/// The share of full deviation of each SCA subcarrier.
pub const SCA_INJECTION: f64 = 0.1;
/// The deviation of an SCA subcarrier by its 1 kHz tone, Hz.
pub const SCA_DEVIATION_HZ: f64 = 6e3;

/// The levels and the form of a multiplex. Every level is a share of the full deviation.
#[derive(Clone, Debug, PartialEq)]
pub struct MpxParams {
    /// The output rate, Hz: positive and above twice the deviation.
    pub rate_hz: f64,
    /// The deviation of a multiplex of 1, Hz.
    pub deviation_hz: f64,
    /// Stereo: the pilot and the difference subcarrier.
    pub stereo: bool,
    /// The pilot level (stereo only).
    pub pilot: f64,
    /// The RDS level at the peak of its wave; 0 sends no RDS.
    pub rds: f64,
    /// The level of the audio part when left or right is at full scale: `audio/2 (L+R) + audio/2 (L-R) sin 2θ`.
    pub audio: f64,
    /// A constant added to the multiplex: the station's offset from the centre.
    pub offset: f64,
    /// The two SCA subcarriers.
    pub sca: bool,
}

/// The audio of a multiplex.
#[derive(Clone, Debug, PartialEq)]
pub enum Programme {
    /// A tone on each channel: frequency (Hz) and amplitude, pre-emphasised by the time constant (s, 0 for none).
    Tones {
        /// Left frequency and amplitude.
        left: (f64, f64),
        /// Right frequency and amplitude.
        right: (f64, f64),
        /// Pre-emphasis time constant.
        preemphasis_s: f64,
    },
    /// Audio given with [`Multiplex::push_audio`], through an [`AudioChain`] with this pre-emphasis and limiter.
    Live {
        /// Pre-emphasis time constant (s, 0 for none).
        preemphasis_s: f64,
        /// The limiter on; off clips each channel at full scale.
        limiter: bool,
    },
}

/// A look-ahead limiter with one gain for both channels: `max(|l|, |r|)` never passes 1, and the gain has no steps.
///
/// The wanted gain `1 / max(|l|, |r|)` (at most 1) is held at its least over the look-ahead, rises back with the release,
/// and is averaged over the look-ahead. The output lags the input by the look-ahead less one sample.
#[derive(Clone, Debug)]
struct Limiter {
    delay: VecDeque<Stereo>,
    least: VecDeque<(u64, f32)>,
    ring: Vec<f32>,
    at: usize,
    sum: f64,
    held: f32,
    rec_a: f32,
    n: u64,
}

impl Limiter {
    fn new(rate: f64, ahead_s: f64, release_s: f64) -> Self {
        let len = ((ahead_s * rate).round() as usize).max(1);
        let mut delay = VecDeque::with_capacity(len);
        delay.extend(std::iter::repeat_n(Stereo::default(), len - 1));
        Limiter {
            delay,
            least: VecDeque::with_capacity(len + 1),
            ring: vec![1.0; len],
            at: 0,
            sum: len as f64,
            held: 1.0,
            rec_a: (1.0 - (-1.0 / (release_s * rate)).exp()) as f32,
            n: 0,
        }
    }

    #[inline]
    fn push(&mut self, x: Stereo) -> Stereo {
        let len = self.ring.len();
        let mag = x.l.abs().max(x.r.abs());
        let want = if mag > 1.0 { 1.0 / mag } else { 1.0 };
        while self.least.back().is_some_and(|&(_, w)| w >= want) {
            self.least.pop_back();
        }
        self.least.push_back((self.n, want));
        while self.least.front().is_some_and(|&(i, _)| i + len as u64 <= self.n) {
            self.least.pop_front();
        }
        self.n += 1;
        let low = self.least.front().map_or(1.0, |p| p.1);
        self.held = if low < self.held {
            low
        } else {
            self.held + self.rec_a * (low - self.held)
        };
        self.sum += f64::from(self.held) - f64::from(self.ring[self.at]);
        self.ring[self.at] = self.held;
        self.at = (self.at + 1) % len;
        let g = ((self.sum / len as f64) as f32).min(1.0);
        self.delay.push_back(x);
        let y = self.delay.pop_front().unwrap_or_default();
        Stereo::new((y.l * g).clamp(-1.0, 1.0), (y.r * g).clamp(-1.0, 1.0))
    }
}

/// Live stereo audio made ready for the multiplex: pre-emphasis, a low-pass to [`AUDIO_TOP_HZ`], the rate change from
/// [`AUDIO_RATE`] to [`LIMIT_RATE`], then the limiter (or a clip at full scale). Nothing after the limiter can raise a peak.
#[derive(Clone, Debug)]
pub struct AudioChain {
    pre: Option<[PreEmphasis; 2]>,
    lp: Fir<Stereo>,
    up: Resampler<Stereo>,
    limit: Option<Limiter>,
    frames: Vec<Stereo>,
    over: Vec<Stereo>,
}

impl AudioChain {
    /// A chain with pre-emphasis `preemphasis_s` (0 for none) and the limiter on or off.
    ///
    /// # Errors
    ///
    /// [`DspError`] if `preemphasis_s` is negative or not finite.
    pub fn new(preemphasis_s: f64, limiter: bool) -> Result<Self, DspError> {
        let pre = if preemphasis_s > 0.0 {
            let p = PreEmphasis::new(preemphasis_s, AUDIO_RATE)?;
            Some([p.clone(), p])
        } else {
            None
        };
        Ok(AudioChain {
            pre,
            lp: Fir::new(&lowpass(AUDIO_RATE, AUDIO_TOP_HZ, AUDIO_STOP_HZ, 50.0)?),
            up: Resampler::new(AUDIO_RATE, LIMIT_RATE)?,
            limit: limiter.then(|| Limiter::new(LIMIT_RATE, LIMIT_AHEAD_S, LIMIT_RELEASE_S)),
            frames: Vec::new(),
            over: Vec::new(),
        })
    }

    /// Takes frames at [`AUDIO_RATE`] and appends the limited audio at [`LIMIT_RATE`] to `out`; each channel within ±1.
    pub fn process(&mut self, input: &[Stereo], out: &mut Vec<Stereo>) {
        self.frames.clear();
        self.frames.extend_from_slice(input);
        if let Some([l, r]) = &mut self.pre {
            for f in &mut self.frames {
                *f = Stereo::new(l.process_sample(f.l), r.process_sample(f.r));
            }
        }
        self.lp.process_in_place(&mut self.frames);
        self.over.clear();
        self.up.process(&self.frames, &mut self.over);
        match &mut self.limit {
            Some(l) => out.extend(self.over.iter().map(|&x| l.push(x))),
            None => out.extend(
                self.over
                    .iter()
                    .map(|x| Stereo::new(x.l.clamp(-1.0, 1.0), x.r.clamp(-1.0, 1.0))),
            ),
        }
    }
}

/// `(cos, sin)` of an oscillator at its current sample, then one step on.
#[inline]
fn osc(p: &mut PhaseAcc) -> (f64, f64) {
    let z = p.next_phasor();
    (z.re as f64, z.im as f64)
}

/// The audio source of a running multiplex.
#[derive(Clone, Debug)]
enum Source {
    Tones {
        left: PhaseAcc,
        right: PhaseAcc,
        amps: (f64, f64),
    },
    Live {
        chain: Box<AudioChain>,
        queue: Vec<Stereo>,
        read: usize,
        step: f64,
        pos: f64,
        a: Stereo,
        b: Stereo,
    },
}

/// RDS of a running multiplex: the groups, the wave, and the wave drawn onto the output rate by straight lines.
#[derive(Clone, Debug)]
struct Rds {
    enc: RdsEncoder,
    wave: RdsWave,
    step: f64,
    pos: f64,
    a: f32,
    b: f32,
}

/// The multiplex and its FM modulator: complex baseband of unit magnitude at [`MpxParams::rate_hz`].
///
/// With the audio within ±1 per channel the multiplex stays within `audio + pilot + rds + offset` (each sample's audio is a
/// straight line between two limited samples, so it cannot pass them). The RDS bit clock starts at the first sample.
#[derive(Clone, Debug)]
pub struct Multiplex {
    p: MpxParams,
    fm: Fm,
    pilot: PhaseAcc,
    source: Source,
    rds: Option<Rds>,
    sca_tone: PhaseAcc,
    sca_phase: [f64; 2],
    n: u64,
    clock_s: Option<f64>,
    mpx: Vec<f32>,
    peak: f32,
    energy: f64,
    count: u64,
}

impl Multiplex {
    /// A multiplex of `p` carrying `programme` and, when `p.rds > 0`, the RDS `content`.
    ///
    /// # Errors
    ///
    /// [`DspError`] if a pre-emphasis time constant is negative, or the rate is not above twice the deviation.
    pub fn new(p: MpxParams, programme: &Programme, content: RdsContent) -> Result<Self, DspError> {
        let rate = p.rate_hz;
        let fm = Fm::new(rate, p.deviation_hz).map_err(|_| DspError::OutOfRange {
            param: Param::Deviation,
            min: 0.0,
            max: rate / 2.0,
            got: p.deviation_hz,
        })?;
        let source = match *programme {
            Programme::Tones {
                left,
                right,
                preemphasis_s,
            } => {
                let pre = |f: f64| (1.0 + (TAU * f * preemphasis_s).powi(2)).sqrt();
                Source::Tones {
                    left: PhaseAcc::new(left.0 / rate),
                    right: PhaseAcc::new(right.0 / rate),
                    amps: (left.1 * pre(left.0), right.1 * pre(right.0)),
                }
            }
            Programme::Live {
                preemphasis_s,
                limiter,
            } => Source::Live {
                chain: Box::new(AudioChain::new(preemphasis_s, limiter)?),
                queue: Vec::new(),
                read: 0,
                step: LIMIT_RATE / rate,
                pos: 0.0,
                a: Stereo::default(),
                b: Stereo::default(),
            },
        };
        let rds = (p.rds > 0.0).then(|| Rds {
            enc: RdsEncoder::new(content),
            wave: RdsWave::new(),
            step: RdsWave::rate() / rate,
            pos: 1.0,
            a: 0.0,
            b: 0.0,
        });
        Ok(Multiplex {
            fm,
            pilot: PhaseAcc::new(PILOT_HZ / rate),
            source,
            rds,
            sca_tone: PhaseAcc::new(1000.0 / rate),
            sca_phase: [0.0; 2],
            n: 0,
            clock_s: None,
            mpx: Vec::new(),
            peak: 0.0,
            energy: 0.0,
            count: 0,
            p,
        })
    }

    /// The parameters.
    pub fn params(&self) -> &MpxParams {
        &self.p
    }

    /// Takes stereo frames at [`AUDIO_RATE`] for live audio; ignored with tones. When the audio runs dry it is silent.
    pub fn push_audio(&mut self, frames: &[Stereo]) {
        if let Source::Live {
            chain, queue, read, ..
        } = &mut self.source
        {
            if *read > 0 && *read * 2 >= queue.len() {
                queue.drain(..*read);
                *read = 0;
            }
            chain.process(frames, queue);
        }
    }

    /// The live audio that waits, in frames at [`LIMIT_RATE`].
    pub fn audio_waiting(&self) -> usize {
        match &self.source {
            Source::Live { queue, read, .. } => queue.len() - read,
            Source::Tones { .. } => 0,
        }
    }

    /// States that the first sample is on air at `unix_s` (UTC): it times the clock group.
    pub fn set_clock(&mut self, unix_s: f64) {
        self.clock_s = Some(unix_s);
    }

    /// The RDS content in force, if RDS is sent.
    pub fn content(&self) -> Option<&RdsContent> {
        self.rds.as_ref().map(|r| r.enc.content())
    }

    /// Replaces the RDS content at the next boundaries of its parts ([`RdsEncoder::set_content`]).
    pub fn set_content(&mut self, c: RdsContent) {
        if let Some(r) = &mut self.rds {
            r.enc.set_content(c);
        }
    }

    /// The RDS groups made so far.
    pub fn groups(&self) -> u64 {
        self.rds.as_ref().map_or(0, |r| r.enc.groups())
    }

    /// The largest magnitude of the multiplex, the mean of its square and the samples since the last call, and starts
    /// the next interval.
    pub fn take_level(&mut self) -> (f32, f64, u64) {
        let out = (self.peak, self.energy, self.count);
        (self.peak, self.energy, self.count) = (0.0, 0.0, 0);
        out
    }

    /// The live audio of the next sample.
    #[inline]
    fn live(source: &mut Source) -> (f64, f64) {
        let Source::Live {
            queue,
            read,
            step,
            pos,
            a,
            b,
            ..
        } = source
        else {
            return (0.0, 0.0);
        };
        *pos += *step;
        while *pos >= 1.0 {
            *pos -= 1.0;
            *a = *b;
            *b = match queue.get(*read) {
                Some(x) => {
                    *read += 1;
                    *x
                }
                None => Stereo::default(),
            };
        }
        let t = *pos as f32;
        (
            f64::from(a.l + (b.l - a.l) * t),
            f64::from(a.r + (b.r - a.r) * t),
        )
    }

    /// The multiplex at the next sample, full deviation = 1.
    #[inline]
    fn sample(&mut self) -> f32 {
        let n = self.n;
        self.n += 1;
        let (pc, ps) = osc(&mut self.pilot);
        let (l, r) = match &mut self.source {
            Source::Tones { left, right, amps } => {
                let (_, ls) = osc(left);
                let (_, rs) = osc(right);
                (amps.0 * ls, amps.1 * rs)
            }
            live => Self::live(live),
        };
        let p = &self.p;
        let half = p.audio / 2.0;
        let mut mpx = if p.stereo {
            let sin2 = 2.0 * ps * pc;
            half * (l + r) + half * (l - r) * sin2 + p.pilot * ps
        } else {
            half * (l + r)
        };
        if let Some(rds) = &mut self.rds {
            rds.pos += rds.step;
            while rds.pos >= 1.0 {
                rds.pos -= 1.0;
                rds.a = rds.b;
                let t = self.clock_s.map(|c| c + n as f64 / p.rate_hz);
                let enc = &mut rds.enc;
                rds.b = rds.wave.next(|| enc.next_bit(t));
            }
            let m = f64::from(rds.a + (rds.b - rds.a) * rds.pos as f32);
            // The subcarrier is cos 3θ: locked to the third harmonic of the pilot sin θ.
            let cos3 = 4.0 * pc * pc * pc - 3.0 * pc;
            mpx += p.rds * m * cos3;
        }
        if p.sca {
            let (_, ts) = osc(&mut self.sca_tone);
            for (ph, fc) in self.sca_phase.iter_mut().zip(SCA_HZ) {
                *ph = (*ph + TAU * (fc + SCA_DEVIATION_HZ * ts) / p.rate_hz) % TAU;
                mpx += SCA_INJECTION * ph.cos();
            }
        }
        let mpx = (mpx + p.offset) as f32;
        self.peak = self.peak.max(mpx.abs());
        self.energy += f64::from(mpx) * f64::from(mpx);
        self.count += 1;
        mpx
    }

    /// Fills `out` with the next samples, magnitude 1.
    pub fn generate(&mut self, out: &mut [Complex32]) {
        let mut mpx = std::mem::take(&mut self.mpx);
        mpx.clear();
        for _ in 0..out.len() {
            mpx.push(self.sample());
        }
        if self.fm.process(&mpx, out).is_err() {
            out.fill(Complex32::default());
        }
        self.mpx = mpx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_holds_full_scale_without_steps() {
        let mut l = Limiter::new(LIMIT_RATE, LIMIT_AHEAD_S, LIMIT_RELEASE_S);
        let mut last = 0.0f32;
        let mut most = 0.0f32;
        for i in 0..LIMIT_RATE as usize / 10 {
            let t = i as f64 / LIMIT_RATE;
            let amp = if t > 0.05 { 4.0 } else { 0.5 };
            let x = Stereo::new((amp * (TAU * 1000.0 * t).sin()) as f32, 0.0);
            let y = l.push(x);
            most = most.max(y.l.abs());
            assert!((y.l - last).abs() < 0.05, "a step at {t}");
            last = y.l;
        }
        assert!(most <= 1.0 && most > 0.95, "{most}");
    }
}
