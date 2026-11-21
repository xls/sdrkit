//! The voice sender that stacks share: the speech processor and the modulators of one voice mode ([`VoiceChain`]), the
//! push-to-talk command ([`ptt`], [`PttArgs`]), and the transmissions of a keyed sender ([`KeyedSender`]): speech, a keyed
//! carrier or a steady carrier, each with a raised-cosine rise and fall, at the session's rate, with its end mark.
//!
//! The speech processor works at 48 kHz: a high-pass, the pre-emphasis of FM, the audio low-pass, a slow level control, for
//! SSB an optional compressor, and a look-ahead limiter. The modulators are FM with an optional CTCSS tone, AM, and SSB by
//! a complex band-pass filter or by phasing. The output is complex baseband whose magnitude never passes [`PEAK`].

use crate::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, MicInput};
use sdrkit_algo_dsp::dynamics::{Leveller, SmoothLimiter};
use sdrkit_algo_dsp::fir::{bandpass_complex, ComplexFir};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4, PreEmphasis, BUTTERWORTH4_Q};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::{Am, Fm, Ook, Sideband, Ssb};
use sdrkit_core::interface::{Arguments, CommandKind};
use sdrkit_core::{Complex32, TxContent};
use sdrkit_graph::{Format, Io, RunContext};

/// The rate of the speech processor and of the modulators.
pub const RATE: f64 = 48_000.0;
/// The largest magnitude of a transmitted sample (0.9, -0.9 dBFS), below full scale by the overshoot of the rate change.
pub const PEAK: f32 = 0.9;
/// The rise and fall of every transmission and of each element of a keyed carrier (raised cosine).
pub const RISE_S: f64 = 0.005;
/// The share of the FM deviation that the CTCSS tone takes.
pub const CTCSS_SHARE: f32 = 0.12;
/// The top of FM speech: Carson's rule for the occupied bandwidth uses it.
pub const NFM_AUDIO_HZ: f64 = 3000.0;
/// The usual audio band of SSB speech.
pub const SSB_BAND_HZ: (f64, f64) = (300.0, 2700.0);
/// The corner of the speech high-pass.
pub const HIGHPASS_HZ: f64 = 250.0;
/// The RMS that the level control aims at, as a share of full modulation.
const LEVEL_RMS: f32 = 0.3;
/// The range of the level control (dB either way).
const LEVEL_RANGE_DB: f32 = 20.0;
/// The look-ahead of the limiters.
const AHEAD_S: f64 = 0.002;
/// The release of the limiters.
const RELEASE_S: f64 = 0.05;
/// The look-ahead and the release of the SSB compressor.
const COMPRESS_AHEAD_S: f64 = 0.005;
const COMPRESS_RELEASE_S: f64 = 0.15;
/// The transition of the SSB filter outside each edge, and its stop-band.
const SSB_TRANSITION_HZ: f64 = 400.0;
const SSB_ATTEN_DB: f64 = 70.0;
/// The flat range of the Hilbert transformer of the phasing method starts here.
const HILBERT_TRANSITION_HZ: f64 = 150.0;
/// The 48 kHz samples made at one time.
const BLOCK: usize = 240;
/// The microphone audio kept while it waits (s); older audio is dropped.
const MIC_KEEP_S: f64 = 0.25;

sdrkit_core::choice_enum! {
    /// How SSB keeps one sideband.
    #[derive(Default)]
    pub enum SsbMethod {
        /// A complex band-pass filter passes one sideband.
        #[default]
        Filter = ("filter", "Filter"),
        /// A Hilbert transformer cancels the other sideband.
        Phasing = ("phasing", "Phasing"),
    }
}

/// A voice mode with its values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VoiceMode {
    /// FM speech up to [`NFM_AUDIO_HZ`].
    Fm {
        /// The peak deviation of the speech with its tone (Hz).
        deviation_hz: f64,
        /// The time constant of the pre-emphasis (s), 0 for none.
        preemphasis_s: f64,
        /// The CTCSS tone (Hz), 0 for none. It takes [`CTCSS_SHARE`] of the deviation.
        ctcss_hz: f64,
    },
    /// AM speech with its carrier.
    Am {
        /// The largest modulation depth, 0 to 1.
        depth: f64,
        /// The top of the audio (Hz).
        audio_hz: f64,
    },
    /// SSB speech with no carrier.
    Ssb {
        /// The upper sideband (true) or the lower one.
        upper: bool,
        /// How the other sideband is removed.
        method: SsbMethod,
        /// The audio band: its low and its high edge (Hz).
        band_hz: (f64, f64),
        /// The gain of the compressor (dB), 0 for none.
        compression_db: f64,
    },
}

/// The modulator of a voice mode.
enum Modulator {
    Fm(Fm),
    Am(Am, f32),
    Filter(ComplexFir),
    Phasing(Ssb),
}

/// The meters of the speech processor since the last read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceMeters {
    /// The peak of the microphone audio (full scale 1).
    pub mic_peak: f32,
    /// The peak of the modulation after the limiter, as a share of full modulation.
    pub peak: f32,
    /// The gain of the level control (dB).
    pub level_db: f32,
    /// The most gain reduction of the limiters (dB, 0 or below).
    pub limit_db: f32,
}

impl VoiceMeters {
    /// The peak of the microphone audio (dBFS), -120 at the lowest.
    pub fn mic_dbfs(&self) -> f64 {
        f64::from(20.0 * self.mic_peak.max(1e-6).log10())
    }

    /// The peak of the modulation against full modulation (dB), -120 at the lowest.
    pub fn peak_db(&self) -> f64 {
        f64::from(20.0 * self.peak.max(1e-6).log10())
    }
}

/// The speech processor and the modulator of one voice mode.
pub struct VoiceChain {
    highpass: [Biquad; 2],
    pre: Option<PreEmphasis>,
    lowpass: Butterworth4,
    level: Leveller,
    compress: Option<(f32, SmoothLimiter<f32>)>,
    limit: SmoothLimiter<f32>,
    ctcss: Option<(Nco, f32)>,
    modulator: Modulator,
    envelope: Option<SmoothLimiter<Complex32>>,
    audio: Vec<f32>,
    mic_peak: f32,
    peak: f32,
}

impl VoiceChain {
    /// The chain of `mode`. `None` when a modulator or the sideband filter refuses the values.
    pub fn new(mode: &VoiceMode) -> Option<Self> {
        let highpass =
            BUTTERWORTH4_Q.map(|q| Biquad::new(BiquadCoeffs::highpass(RATE, HIGHPASS_HZ, q)));
        let mut pre = None;
        let mut ctcss = None;
        let mut share = 1.0;
        let mut compression = 0.0;
        let hi;
        let modulator = match *mode {
            VoiceMode::Fm {
                deviation_hz,
                preemphasis_s,
                ctcss_hz,
            } => {
                hi = NFM_AUDIO_HZ;
                pre = (preemphasis_s > 0.0)
                    .then(|| PreEmphasis::new(preemphasis_s, RATE).ok())
                    .flatten();
                if ctcss_hz > 0.0 {
                    share = 1.0 - CTCSS_SHARE;
                    ctcss = Some((Nco::new(RATE, ctcss_hz), CTCSS_SHARE));
                }
                Modulator::Fm(Fm::new(RATE, deviation_hz).ok()?)
            }
            VoiceMode::Am { depth, audio_hz } => {
                hi = audio_hz;
                let depth = depth.clamp(0.0, 1.0);
                Modulator::Am(Am::new(depth).ok()?, PEAK / (1.0 + depth as f32))
            }
            VoiceMode::Ssb {
                upper,
                method,
                band_hz: (lo, high),
                compression_db,
            } => {
                hi = high;
                compression = compression_db;
                match method {
                    SsbMethod::Filter => {
                        let (a, b) = if upper { (lo, hi) } else { (-hi, -lo) };
                        let taps =
                            bandpass_complex(RATE, a, b, SSB_TRANSITION_HZ, SSB_ATTEN_DB).ok()?;
                        // A real tone has half its power on each side: twice the taps keep its amplitude on the side passed.
                        let taps: Vec<Complex32> = taps.iter().map(|t| *t * 2.0).collect();
                        Modulator::Filter(ComplexFir::new(&taps))
                    }
                    SsbMethod::Phasing => {
                        let side = if upper {
                            Sideband::Upper
                        } else {
                            Sideband::Lower
                        };
                        Modulator::Phasing(Ssb::new(side, RATE, HILBERT_TRANSITION_HZ, 60.0).ok()?)
                    }
                }
            }
        };
        let ssb = matches!(mode, VoiceMode::Ssb { .. });
        let compress = (ssb && compression > 0.0).then(|| {
            let gain = 10f32.powf(compression as f32 / 20.0);
            (
                gain,
                SmoothLimiter::new(RATE, COMPRESS_AHEAD_S, 1.0, COMPRESS_RELEASE_S),
            )
        });
        Some(VoiceChain {
            highpass,
            pre,
            lowpass: Butterworth4::lowpass(RATE, hi),
            level: Leveller::with_range(RATE, LEVEL_RMS * share, LEVEL_RANGE_DB),
            compress,
            limit: SmoothLimiter::new(RATE, AHEAD_S, share, RELEASE_S),
            ctcss,
            modulator,
            envelope: ssb.then(|| SmoothLimiter::new(RATE, AHEAD_S, 1.0, RELEASE_S)),
            audio: Vec::new(),
            mic_peak: 0.0,
            peak: 0.0,
        })
    }

    /// Processes the microphone audio `mic` (48 kHz, full scale 1) into `out` (the same length). Each output sample has a
    /// magnitude of at most [`PEAK`].
    pub fn process(&mut self, mic: &[f32], out: &mut [Complex32]) {
        let n = mic.len().min(out.len());
        self.audio.clear();
        for &m in &mic[..n] {
            let m = if m.is_finite() { m } else { 0.0 };
            self.mic_peak = self.mic_peak.max(m.abs());
            let h = self.highpass[0].process_sample(m);
            let mut x = self.highpass[1].process_sample(h);
            if let Some(p) = &mut self.pre {
                x = p.process_sample(x);
            }
            x = self.lowpass.process_sample(x);
            x = self.level.push(x);
            if let Some((gain, c)) = &mut self.compress {
                let y = x * *gain;
                x = c.push(y, y.abs());
            }
            x = self.limit.push(x, x.abs());
            self.peak = self.peak.max(x.abs());
            if let Some((nco, amp)) = &mut self.ctcss {
                x += *amp * nco.next_sample().im;
            }
            self.audio.push(x);
        }
        let out = &mut out[..n];
        match &mut self.modulator {
            Modulator::Fm(fm) => {
                let _ = fm.process(&self.audio, out);
                out.iter_mut().for_each(|z| *z *= PEAK);
            }
            Modulator::Am(am, scale) => {
                let _ = am.process(&self.audio, out);
                out.iter_mut().for_each(|z| *z *= *scale);
            }
            Modulator::Filter(f) => {
                for (z, &a) in out.iter_mut().zip(&self.audio) {
                    *z = f.process_sample(Complex32::new(a, 0.0));
                }
            }
            Modulator::Phasing(s) => {
                let _ = s.process(&self.audio, out);
            }
        }
        if let Some(env) = &mut self.envelope {
            for z in out.iter_mut() {
                *z = env.push(*z, z.norm()) * PEAK;
            }
        }
    }

    /// The meters since the last call, and starts the next interval.
    pub fn take_meters(&mut self) -> VoiceMeters {
        let mut limit_db = self.limit.take_reduction_db();
        if let Some(e) = &mut self.envelope {
            limit_db = limit_db.min(e.take_reduction_db());
        }
        if let Some((_, c)) = &mut self.compress {
            limit_db = limit_db.min(c.take_reduction_db());
        }
        VoiceMeters {
            mic_peak: std::mem::take(&mut self.mic_peak),
            peak: std::mem::take(&mut self.peak),
            level_db: self.level.gain_db(),
            limit_db,
        }
    }
}

/// A speech processor and modulator that a [`KeyedSender`] sends: [`VoiceChain`], or the own chain of a stack whose
/// speech processing differs.
pub trait Speech: Send {
    /// Processes the microphone audio `mic` (48 kHz, full scale 1) into `out` (the same length). Each output sample has a
    /// magnitude of at most [`PEAK`].
    fn process(&mut self, mic: &[f32], out: &mut [Complex32]);
    /// The meters since the last call, and starts the next interval.
    fn take_meters(&mut self) -> VoiceMeters;
}

impl Speech for VoiceChain {
    fn process(&mut self, mic: &[f32], out: &mut [Complex32]) {
        VoiceChain::process(self, mic, out);
    }
    fn take_meters(&mut self) -> VoiceMeters {
        VoiceChain::take_meters(self)
    }
}

/// The arguments of `ptt`.
#[derive(Arguments)]
pub struct PttArgs {
    /// Push-to-talk pressed (on) or released (off).
    #[arg(
        label = "On",
        help = "True starts speech from the microphone input, false ends it."
    )]
    pub on: bool,
}

/// The push-to-talk command `ptt` of a voice sender with the help text `help`: it transmits voice, and `ptt` with `on`
/// false stops it.
pub const fn ptt(help: &'static str) -> CommandKind<PttArgs> {
    CommandKind::new("ptt", "Push to talk")
        .help(help)
        .transmits(TxContent::Voice)
        .stopped_by("ptt")
}

/// What a transmission of a [`KeyedSender`] sends.
pub enum Content {
    /// Speech of the microphone input through this chain, until the stop or the limit.
    Voice(Box<dyn Speech>),
    /// A carrier keyed by these elements (key down or up, seconds), ended after the last.
    Keys(Vec<(bool, f64)>),
    /// A steady carrier, until the stop or the limit.
    Carrier,
}

/// Why a [`KeyedSender`] does not start a transmission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// The engine's transmit setting is off.
    Off,
    /// Another transmission is under way.
    Busy,
    /// The sender has no input yet, so it has no rate.
    NoInput,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SendError::Off => "the transmitter is off",
            SendError::Busy => "another transmission is under way",
            SendError::NoInput => "the stack has no input yet",
        })
    }
}

impl std::error::Error for SendError {}

/// One transmission.
struct Burst {
    voice: Option<Box<dyn Speech>>,
    keys: Option<Vec<(bool, f64)>>,
    next: usize,
    ook: Ook,
    env: Vec<f32>,
    stopping: bool,
    flushed: bool,
    limit: Option<u64>,
    made: u64,
    /// The samples before the carrier is up in which the speech gets silence and the microphone input is not read.
    lead: u64,
    level: Option<f32>,
    out: BurstOut,
}

impl Burst {
    /// Ends the transmission: the key goes up with its fall.
    fn stop(&mut self) {
        if !self.stopping {
            self.stopping = true;
            self.next = self.keys.as_ref().map_or(0, Vec::len);
            let _ = self.ook.push(false, RISE_S, &mut self.env);
        }
    }

    /// Makes more envelope: one element of the keys, or one block of key down.
    fn more_envelope(&mut self) {
        if self.stopping {
            self.ook.flush(&mut self.env);
            self.flushed = true;
            return;
        }
        if self
            .limit
            .is_some_and(|l| self.made + self.env.len() as u64 >= l)
        {
            self.stop();
            return;
        }
        match &self.keys {
            Some(keys) => match keys.get(self.next) {
                Some(&(on, seconds)) => {
                    self.next += 1;
                    let _ = self.ook.push(on, seconds, &mut self.env);
                }
                None => self.stop(),
            },
            None => {
                let _ = self.ook.push(true, BLOCK as f64 / RATE, &mut self.env);
            }
        }
    }

    /// Makes samples until at least `room` output samples wait, or until the transmission is made with the tail of the
    /// rate change.
    fn fill(
        &mut self,
        room: usize,
        mic: &mut MicInput,
        audio: &mut Vec<f32>,
        base: &mut Vec<Complex32>,
    ) {
        while self.out.wants(room) {
            if self.env.len() < BLOCK && !self.flushed {
                self.more_envelope();
                continue;
            }
            let k = self.env.len().min(BLOCK);
            if k == 0 {
                self.out.finish(base);
                break;
            }
            base.clear();
            base.resize(k, Complex32::default());
            match &mut self.voice {
                Some(v) => {
                    let quiet = (self.lead.saturating_sub(self.made) as usize).min(k);
                    audio.clear();
                    audio.resize(k, 0.0);
                    mic.read(&mut audio[quiet..]);
                    v.process(audio, base);
                    for (z, e) in base.iter_mut().zip(&self.env[..k]) {
                        *z *= *e;
                    }
                }
                None => {
                    for (z, e) in base.iter_mut().zip(&self.env[..k]) {
                        *z = Complex32::new(PEAK * e, 0.0);
                    }
                }
            }
            if let Some(level) = self.level {
                let gain = level / PEAK;
                base.iter_mut().for_each(|z| *z *= gain);
            }
            self.env.drain(..k);
            self.made += k as u64;
            self.out.push(base);
        }
    }
}

/// The transmissions of a sender whose component has input 0 `in` (the session's samples), input 1 `mic` (audio, optional),
/// output 0 `out` (complex baseband at the session's rate, the signal at 0 Hz) and output 1 `keyed` ([`KeyedOut`]).
///
/// One transmission runs at a time. Each pass writes as many samples as the pass brings in. Each transmission starts with
/// a rise and ends with a fall and an end mark: after [`KeyedSender::stop`], its time limit, the last key element, and
/// at once after [`KeyedSender::cut`], the transmit setting going off, a new format and the end of the input. On a live
/// radio whose host times bursts, the first sample states its time on air.
pub struct KeyedSender {
    rate: f64,
    transmit: bool,
    mic: MicInput,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    audio: Vec<f32>,
    base: Vec<Complex32>,
    timing: BurstTiming,
    after_rise: bool,
}

impl Default for KeyedSender {
    fn default() -> Self {
        KeyedSender {
            rate: 0.0,
            transmit: false,
            mic: MicInput::new(RATE, MIC_KEEP_S),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            audio: Vec::new(),
            base: Vec::new(),
            timing: BurstTiming::default(),
            after_rise: false,
        }
    }
}

impl KeyedSender {
    /// The sender whose speech starts once the carrier is up: the microphone input waits through the leading silence and
    /// the rise, so no speech is lost at key down.
    pub fn speech_after_rise(mut self) -> Self {
        self.after_rise = true;
        self
    }

    /// The session's rate (Hz), 0 before the first format.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Whether the engine's transmit setting is on.
    pub fn allowed(&self) -> bool {
        self.transmit
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// Whether a voice transmission is under way and not yet stopped.
    pub fn talking(&self) -> bool {
        self.burst
            .as_ref()
            .is_some_and(|b| b.voice.is_some() && !b.stopping)
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.keyed.keyed(self.burst.is_some() || self.end_owed)
    }

    /// The time on air of the transmission (s), 0 with none.
    pub fn time_s(&self) -> f64 {
        self.burst
            .as_ref()
            .map_or(0.0, |b| b.out.sent() as f64 / self.rate.max(1.0))
    }

    /// The meters of the speech processor since the last call. `None` without a voice transmission.
    pub fn take_meters(&mut self) -> Option<VoiceMeters> {
        self.burst
            .as_mut()
            .and_then(|b| b.voice.as_mut())
            .map(|v| v.take_meters())
    }

    /// Takes the formats of the inputs `in` and `mic`. A transmission on air is dropped.
    pub fn configure(&mut self, session: &Format, mic: &Format) {
        self.rate = session.rate;
        self.mic.configure(mic);
        self.cut();
        self.keyed.reset();
    }

    /// Takes the transmit setting and the burst timing of the run. With the setting off, a transmission on air is dropped.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        self.mic.attach(ctx);
        if !ctx.transmit {
            self.cut();
        }
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    pub fn cut(&mut self) {
        if self.burst.take().is_some() {
            self.end_owed = true;
        }
    }

    /// Starts a transmission of `content` that ends by itself after `limit_s` seconds on air (`None`: no limit). Its peak
    /// is `level` of full scale (`None`: [`PEAK`]).
    ///
    /// # Errors
    ///
    /// [`SendError`] when the transmit setting is off, another transmission runs, or the sender has no input.
    pub fn start(
        &mut self,
        content: Content,
        limit_s: Option<f64>,
        level: Option<f32>,
    ) -> Result<(), SendError> {
        if !self.transmit {
            return Err(SendError::Off);
        }
        if self.burst.is_some() {
            return Err(SendError::Busy);
        }
        if self.rate <= 0.0 {
            return Err(SendError::NoInput);
        }
        let (voice, keys) = match content {
            Content::Voice(v) => (Some(v), None),
            Content::Keys(k) => (None, Some(k)),
            Content::Carrier => (None, None),
        };
        let out = BurstOut::new(RATE, self.rate, 1.0).map_err(|_| SendError::NoInput)?;
        let mut ook = Ook::new(RATE, RISE_S).map_err(|_| SendError::NoInput)?;
        let mut env = Vec::with_capacity(4 * BLOCK);
        // Silence first, so the first rise is whole.
        let _ = ook.push(false, RISE_S, &mut env);
        self.mic.restart();
        let lead = if self.after_rise {
            (2.0 * RISE_S * RATE).round() as u64
        } else {
            0
        };
        self.burst = Some(Burst {
            voice,
            keys,
            next: 0,
            ook,
            env,
            stopping: false,
            flushed: false,
            limit: limit_s.map(|s| (s * RATE) as u64),
            made: 0,
            lead,
            level,
            out,
        });
        Ok(())
    }

    /// Ends the transmission with its fall and end mark.
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.burst {
            b.stop();
        }
    }

    /// One pass: the samples of the transmission, the end mark, and the keyed state. True at a report tick and when a
    /// transmission ended: the owner writes its results then.
    pub fn run(&mut self, io: &mut Io) -> bool {
        let room = io.input_len(0);
        let voice = self.burst.as_ref().is_some_and(|b| b.voice.is_some());
        self.mic.take(io, 1, voice);
        let mut ended = false;
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            ended = true;
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, &mut self.mic, &mut self.audio, &mut self.base);
            if b.out.write(io, 0, room, &self.timing) {
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        io.tick() || ended
    }

    /// The end of the input, after the last [`KeyedSender::run`]: drops a transmission on air and writes its end mark. True
    /// when it did: the owner writes its results then.
    pub fn end(&mut self, io: &mut Io) -> bool {
        if self.burst.take().is_some() || std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// The amplitude of the complex tone at `hz` in `x` (48 kHz).
    fn tone(x: &[Complex32], hz: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, z) in x.iter().enumerate() {
            let (s, c) = (TAU * hz * i as f64 / RATE).sin_cos();
            re += f64::from(z.re) * c + f64::from(z.im) * s;
            im += f64::from(z.im) * c - f64::from(z.re) * s;
        }
        re.hypot(im) / x.len() as f64
    }

    /// Three seconds of a 1 kHz tone through the chain of `mode`; the last second.
    fn settled(mode: VoiceMode) -> Vec<Complex32> {
        let mut chain = VoiceChain::new(&mode).unwrap();
        let mic: Vec<f32> = (0..3 * 48_000)
            .map(|i| (0.25 * (TAU * 1000.0 * i as f64 / RATE).sin()) as f32)
            .collect();
        let mut out = vec![Complex32::default(); mic.len()];
        for (m, o) in mic.chunks(BLOCK).zip(out.chunks_mut(BLOCK)) {
            chain.process(m, o);
        }
        assert!(out.iter().all(|z| z.norm() <= PEAK + 1e-6));
        out.split_off(2 * 48_000)
    }

    fn ssb(upper: bool, method: SsbMethod) -> VoiceMode {
        VoiceMode::Ssb {
            upper,
            method,
            band_hz: SSB_BAND_HZ,
            compression_db: 0.0,
        }
    }

    #[test]
    fn ssb_keeps_one_sideband_and_no_carrier() {
        for method in [SsbMethod::Filter, SsbMethod::Phasing] {
            for upper in [true, false] {
                let x = settled(ssb(upper, method));
                let side = if upper { 1000.0 } else { -1000.0 };
                let kept = tone(&x, side);
                assert!(kept > 0.1, "{method:?} {upper}: {kept}");
                let other = 20.0 * (tone(&x, -side) / kept).log10();
                let carrier = 20.0 * (tone(&x, 0.0) / kept).log10();
                assert!(other < -50.0, "{method:?} {upper}: other side {other} dB");
                assert!(carrier < -50.0, "{method:?} {upper}: carrier {carrier} dB");
            }
        }
    }

    #[test]
    fn am_keeps_its_carrier_and_both_sidebands() {
        let x = settled(VoiceMode::Am {
            depth: 0.9,
            audio_hz: 3000.0,
        });
        let carrier = tone(&x, 0.0);
        assert!((carrier - f64::from(PEAK) / 1.9).abs() < 0.01, "{carrier}");
        let (up, down) = (tone(&x, 1000.0), tone(&x, -1000.0));
        assert!(up > 0.02 && (up - down).abs() < 1e-3 * up, "{up} {down}");
    }

    #[test]
    fn fm_has_a_constant_envelope_and_the_ctcss_tone_takes_its_share() {
        let x = settled(VoiceMode::Fm {
            deviation_hz: 5000.0,
            preemphasis_s: 0.0,
            ctcss_hz: 88.5,
        });
        assert!(x.iter().all(|z| (z.norm() - PEAK).abs() < 1e-4));
        // The deviation of silence is the tone alone: 12 % of 5 kHz.
        let mut chain = VoiceChain::new(&VoiceMode::Fm {
            deviation_hz: 5000.0,
            preemphasis_s: 0.0,
            ctcss_hz: 88.5,
        })
        .unwrap();
        let mut out = vec![Complex32::default(); 48_000];
        chain.process(&vec![0.0; 48_000], &mut out);
        let peak = out
            .windows(2)
            .map(|w| f64::from((w[1] * w[0].conj()).arg()) * RATE / TAU)
            .fold(0.0f64, |a, d| a.max(d.abs()));
        assert!((peak - 600.0).abs() < 10.0, "{peak}");
    }

    #[test]
    fn the_meters_start_again_after_a_read() {
        let mut chain = VoiceChain::new(&ssb(true, SsbMethod::Filter)).unwrap();
        let mut out = vec![Complex32::default(); 480];
        chain.process(&[0.5; 480], &mut out);
        let m = chain.take_meters();
        assert_eq!(m.mic_peak, 0.5);
        assert!(m.limit_db <= 0.0);
        let m = chain.take_meters();
        assert_eq!((m.mic_peak, m.peak), (0.0, 0.0));
    }

    #[test]
    fn a_sender_refuses_without_the_transmit_setting_and_without_input() {
        let mut s = KeyedSender::default();
        assert_eq!(s.start(Content::Carrier, None, None), Err(SendError::Off));
        s.transmit = true;
        assert_eq!(
            s.start(Content::Carrier, None, None),
            Err(SendError::NoInput)
        );
        s.rate = 48_000.0;
        assert_eq!(s.start(Content::Carrier, None, None), Ok(()));
        assert!(s.transmitting() && !s.talking());
        assert_eq!(s.start(Content::Carrier, None, None), Err(SendError::Busy));
        s.cut();
        assert!(!s.transmitting() && s.keyed());
    }
}
