//! `tx.*`: the parts that every sender of a stack shares. A sender is a component with input `in` (the session's samples),
//! input `mic` (host audio, optional) and output `out` (complex baseband for `tx.sink`); with output `keyed` it mutes the
//! receive audio of its stack while it sends.
//!
//! - [`MicInput`]: host audio of any rate and channel count as mono audio at the rate of the sender, kept while it waits for
//!   the frame clock of the sender.
//! - [`BurstTiming`] and [`BurstOut`]: the rate change of a transmission to the session's rate, paced to the samples that each
//!   pass brings in, with the stated start of a burst ([`TagKind::BurstStart`], [`start_mark`]) and its end mark
//!   ([`TagKind::BurstEnd`], [`end_mark`]). The start is the earliest time the host can send, a time after the bursts
//!   that wait before it, or a time set by the sender.
//! - [`Ramp`] and [`soft_limit`]: the raised-cosine rise and fall of a transmission, and the soft limiter of every sample.
//! - [`KeyedOut`] and [`TxKeyed`]: the keyed state on output `keyed`, from key down to a short hold after the end mark.
//! - [`TxMute`] (`tx.mute`): mutes audio while [`TxKeyed`] says so, for a stack whose receive audio has no gate of its own.

use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::{Clock, Complex32, ConfigError, Values};
use sdrkit_graph::components::{HostPort, HostStreams};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, Registry, RunContext,
    TagKind, Timed,
};
use std::sync::Arc;

/// Adds the `tx.*` components of this module: `tx.mute`.
pub fn register(r: &mut Registry) {
    r.add("tx.mute", || Box::new(TxMute::default()));
}

/// Whether a sender holds the receive audio of its stack muted: from key down to a short hold after the end mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxKeyed(pub bool);

impl Item for TxKeyed {
    const KIND: &'static str = "tx.keyed";
}

/// Host audio for a sender: the mean of the channels of input `mic`, at the rate of the sender, waiting to be read.
///
/// Audio arrives with each pass; the sender reads it in its own frames. Audio older than the keep time is dropped, so
/// the delay between host audio and the air stays bounded. [`MicInput::restart`] at key down starts from silence.
/// Each pass that keeps audio tells the host so ([`HostStreams::kept`]), once the sender gave its run context
/// ([`MicInput::attach`]).
pub struct MicInput {
    rate: f64,
    keep: usize,
    host: Option<Arc<dyn HostStreams>>,
    input_rate: f64,
    resampler: Option<Resampler<f32>>,
    mono: Vec<f32>,
    waiting: Vec<f32>,
}

impl MicInput {
    /// Audio at `rate` Hz, keeping at most `keep_s` seconds of it.
    pub fn new(rate: f64, keep_s: f64) -> Self {
        MicInput {
            rate,
            keep: (keep_s * rate) as usize,
            host: None,
            input_rate: 0.0,
            resampler: None,
            mono: Vec::new(),
            waiting: Vec::new(),
        }
    }

    /// Takes the host of `ctx`: the host then learns of each pass that keeps audio.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.host.clone_from(&ctx.streams);
    }

    /// Changes the rate of the audio read to `rate` Hz and the keep time to `keep_s` seconds, and starts from silence.
    pub fn set_rate(&mut self, rate: f64, keep_s: f64) {
        self.rate = rate;
        self.keep = (keep_s * rate) as usize;
        self.restart();
    }

    /// The rate of the audio read (Hz).
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Takes the format of the input: audio of any rate, or no input (anything that is not audio).
    pub fn configure(&mut self, f: &Format) {
        self.input_rate = if f.kind == Kind::Audio { f.rate } else { 0.0 };
        self.restart();
    }

    /// Starts from silence: empties the waiting audio and the state of the rate change.
    pub fn restart(&mut self) {
        self.resampler = (self.input_rate > 0.0 && (self.input_rate - self.rate).abs() > 1e-6)
            .then(|| Resampler::new(self.input_rate, self.rate).ok())
            .flatten();
        self.waiting.clear();
    }

    /// Takes the audio of input `i` of `io`; keeps it only while `keep` (a voice transmission is on), drops it otherwise.
    pub fn take(&mut self, io: &Io, i: usize, keep: bool) {
        if !io.is_connected(i) || self.input_rate <= 0.0 {
            return;
        }
        if !keep {
            self.waiting.clear();
            return;
        }
        let a = io.audio(i);
        self.push(&a.samples, a.channels);
    }

    /// Adds interleaved audio of `channels` channels at the input rate, and tells the host that its audio was kept in
    /// this pass, also when `samples` is empty.
    pub fn push(&mut self, samples: &[f32], channels: u16) {
        if let Some(h) = &self.host {
            h.kept(HostPort::Microphone);
        }
        let ch = usize::from(channels.max(1));
        self.mono.clear();
        self.mono.extend(
            samples
                .chunks_exact(ch)
                .map(|f| f.iter().sum::<f32>() / ch as f32),
        );
        match &mut self.resampler {
            Some(r) => r.process(&self.mono, &mut self.waiting),
            None => self.waiting.extend_from_slice(&self.mono),
        }
        if self.waiting.len() > self.keep {
            let drop = self.waiting.len() - self.keep;
            self.waiting.drain(..drop);
        }
    }

    /// The samples that wait.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// Fills `out` with the next samples, silence where none wait.
    pub fn read(&mut self, out: &mut [f32]) {
        let k = out.len().min(self.waiting.len());
        for (o, s) in out.iter_mut().zip(self.waiting.drain(..k)) {
            *o = s;
        }
        out[k..].fill(0.0);
    }
}

/// Whether the first sample of a transmission states its time on air, and which time. Set from the run context.
pub struct BurstTiming {
    live: bool,
    wall: Clock,
    host: Option<Arc<dyn HostStreams>>,
    free_at: f64,
}

impl Default for BurstTiming {
    fn default() -> Self {
        BurstTiming {
            live: false,
            wall: Clock::fixed(0.0),
            host: None,
            free_at: 0.0,
        }
    }
}

impl BurstTiming {
    /// Takes whether the radio is live, the clock and the host of `ctx`.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.wall = ctx.clock.clone();
        self.host.clone_from(&ctx.streams);
    }

    /// The time on air of a transmission whose first sample is written in a pass of `pass_s` seconds of samples: the time
    /// of the pass, the lead of the host and two passes, as the next pass can come one pass late. `None` if the start is
    /// not stated: the radio is not live or the host gives no lead ([`HostStreams::transmit_lead`]).
    pub fn start_time(&self, pass_s: f64) -> Option<f64> {
        let lead = self.host.as_ref()?.transmit_lead().as_secs_f64();
        (self.live && lead > 0.0).then(|| self.wall.now() + lead + 2.0 * pass_s)
    }

    /// The time on air of a burst of `len_s` seconds that waits behind the bursts stated before it by this function: the
    /// time of [`BurstTiming::start_time`], but not before the end on air of the last of them. `None` as there.
    pub fn start_queued(&mut self, pass_s: f64, len_s: f64) -> Option<f64> {
        let at = self.start_time(pass_s)?.max(self.free_at);
        self.free_at = at + len_s;
        Some(at)
    }

    /// How long before its time on air a burst due at a set time is written, for a stack whose own clock puts its newest
    /// sample at `stream_utc`: the lead of the host, the age of that sample by the clock of the host, and two passes of
    /// `pass_s` seconds. `None` if the start is not stated: the radio is not live, the host gives no lead, or the two clocks
    /// are `tolerance_s` or more apart.
    pub fn lead_before(&self, stream_utc: f64, pass_s: f64, tolerance_s: f64) -> Option<f64> {
        let lead = self.host.as_ref()?.transmit_lead().as_secs_f64();
        let age = self.wall.now() - stream_utc;
        (self.live && lead > 0.0 && age.abs() < tolerance_s)
            .then(|| lead + age.max(0.0) + 2.0 * pass_s)
    }
}

/// Writes the stated start `unix_s` (UTC, s) on sample `at` of IQ output `o`.
pub fn start_mark(io: &mut Io, o: usize, at: usize, unix_s: f64) {
    io.output(o).tag_here(at, TagKind::BurstStart { unix_s });
}

/// Writes the end mark after the last sample of IQ output `o`, or on one silent sample when the pass wrote none.
pub fn end_mark(io: &mut Io, o: usize) {
    let out = io.iq_out(o);
    if out.is_empty() {
        out.push(Complex32::default());
    }
    let last = out.len() - 1;
    io.output(o).tag_here(last, TagKind::BurstEnd);
}

/// One transmission from the rate of its maker to the session's rate: the rate change, the samples that wait for the
/// output, and the samples written. Each pass writes as many samples as the pass brings in, the rest once the maker is done.
/// At equal rates the samples pass unchanged, with no tail.
pub struct BurstOut {
    up: Resampler<Complex32>,
    rate: f64,
    pending: Vec<Complex32>,
    tail: bool,
    sent: u64,
    bound: f32,
    start: Option<Option<f64>>,
}

impl BurstOut {
    /// A transmission made at `from` Hz, written at `to` Hz, each sample's magnitude cut to `bound` (the rate change can
    /// overshoot by a fraction of a dB).
    ///
    /// # Errors
    ///
    /// [`DspError`] if a rate is not positive.
    pub fn new(from: f64, to: f64, bound: f32) -> Result<Self, DspError> {
        Self::with_quality(from, to, 0.3, 80.0, bound)
    }

    /// As [`BurstOut::new`], with a rate change flat to `pass` (a fraction of the lower rate, below 0.5) and a stop band of
    /// `atten_db`.
    ///
    /// # Errors
    ///
    /// [`DspError`] if a rate is not positive or `pass` is out of range.
    pub fn with_quality(
        from: f64,
        to: f64,
        pass: f64,
        atten_db: f64,
        bound: f32,
    ) -> Result<Self, DspError> {
        Ok(BurstOut {
            up: Resampler::with_quality(from, to, pass, atten_db)?,
            rate: to,
            pending: Vec::new(),
            tail: false,
            sent: 0,
            bound,
            start: None,
        })
    }

    /// The first sample states `unix_s` as its time on air (no start with `None`), in place of the time that the timing
    /// gives in [`BurstOut::write`]. For a burst due at a set time.
    pub fn state_start(&mut self, unix_s: Option<f64>) {
        self.start = Some(unix_s);
    }

    /// The delay of the rate change (s): the first sample written holds the maker's samples of this much before its first.
    pub fn delay_s(&self) -> f64 {
        self.up.delay() / self.up.input_rate()
    }

    /// Whether the maker must add samples before `room` samples can be written.
    pub fn wants(&self, room: usize) -> bool {
        self.pending.len() < room && !self.tail
    }

    /// Adds samples of the maker.
    pub fn push(&mut self, made: &[Complex32]) {
        self.up.process(made, &mut self.pending);
    }

    /// The maker is done: adds the tail of the rate change, if the rates differ. `scratch` is a buffer for the silence that
    /// flushes it.
    pub fn finish(&mut self, scratch: &mut Vec<Complex32>) {
        if self.tail {
            return;
        }
        if self.up.decimation() > 1 || self.up.has_fractional_stage() {
            scratch.clear();
            scratch.resize(self.up.delay().ceil() as usize + 2, Complex32::default());
            self.up.process(scratch, &mut self.pending);
        }
        self.tail = true;
    }

    /// Whether the maker is done.
    pub fn finished(&self) -> bool {
        self.tail
    }

    /// The samples written at the session's rate.
    pub fn sent(&self) -> u64 {
        self.sent
    }

    /// Writes the samples of this pass to IQ output `o`: `room` samples, or all that wait once the maker is done. The first
    /// sample of the transmission states its time on air: the time of [`BurstOut::state_start`] if one was set, else the
    /// time that `timing` gives. True once the last sample and the end mark are written.
    pub fn write(&mut self, io: &mut Io, o: usize, room: usize, timing: &BurstTiming) -> bool {
        let n = if self.tail {
            self.pending.len()
        } else {
            room.min(self.pending.len())
        };
        if self.sent == 0 && n > 0 {
            let stated = self
                .start
                .unwrap_or_else(|| timing.start_time(room as f64 / self.rate));
            if let Some(unix_s) = stated {
                let first = io.iq_out(o).len();
                start_mark(io, o, first, unix_s);
            }
        }
        let bound = self.bound;
        let out = io.iq_out(o);
        for z in self.pending.drain(..n) {
            let m = z.norm();
            out.push(if m > bound { z * (bound / m) } else { z });
        }
        self.sent += n as u64;
        if self.tail && self.pending.is_empty() {
            end_mark(io, o);
            return true;
        }
        false
    }
}

/// The magnitude above which [`soft_limit`] acts.
pub const SOFT_KNEE: f32 = 0.9;

/// The soft limiter of a sample: the magnitude unchanged up to [`SOFT_KNEE`], above it `0.9 + 0.1 tanh((m - 0.9) / 0.1)`,
/// so it never exceeds 1. The phase is kept.
pub fn soft_limit(z: Complex32) -> Complex32 {
    let m = z.norm();
    if m <= SOFT_KNEE {
        return z;
    }
    z * ((SOFT_KNEE + 0.1 * ((m - SOFT_KNEE) / 0.1).tanh()) / m)
}

/// The raised-cosine rise at the start of a transmission and its fall at the end, each `len` samples long.
#[derive(Clone, Copy, Debug)]
pub struct Ramp {
    len: usize,
    made: usize,
    fall: Option<usize>,
}

impl Ramp {
    /// A rise and fall of `len` samples (at least 1).
    pub fn new(len: usize) -> Self {
        Ramp {
            len: len.max(1),
            made: 0,
            fall: None,
        }
    }

    /// A rise and fall of `seconds` at `rate` Hz, rounded to whole samples (at least 1).
    pub fn of_seconds(seconds: f64, rate: f64) -> Self {
        Self::new((seconds * rate).round() as usize)
    }

    /// The length of the rise and of the fall, samples.
    pub fn samples(&self) -> usize {
        self.len
    }

    /// Starts the fall at the next sample. A fall under way goes on.
    pub fn fall(&mut self) {
        if self.fall.is_none() {
            self.fall = Some(0);
        }
    }

    /// Whether the fall has started.
    pub fn falling(&self) -> bool {
        self.fall.is_some()
    }

    /// Whether the fall has ended.
    pub fn done(&self) -> bool {
        self.fall.is_some_and(|f| f >= self.len)
    }

    /// The gain of the next sample: `base` times the rise and the fall. `None` once the fall has ended.
    pub fn gain(&mut self, base: f32) -> Option<f32> {
        let len = self.len as f32;
        let mut g = base;
        if self.made < self.len {
            g *= 0.5 - 0.5 * (std::f32::consts::PI * self.made as f32 / len).cos();
        }
        if let Some(f) = self.fall.as_mut() {
            if *f >= self.len {
                return None;
            }
            g *= 0.5 + 0.5 * (std::f32::consts::PI * *f as f32 / len).cos();
            *f += 1;
        }
        self.made += 1;
        Some(g)
    }

    /// Scales each sample of `out` by `base`, the rise and the fall, then passes it through [`soft_limit`]. The samples after
    /// the end of the fall are cut off. False when `out` holds the last samples of the transmission.
    pub fn shape(&mut self, base: f32, out: &mut Vec<Complex32>) -> bool {
        let mut end = out.len();
        for (i, z) in out.iter_mut().enumerate() {
            let Some(g) = self.gain(base) else {
                end = i;
                break;
            };
            *z = soft_limit(*z * g);
        }
        if end < out.len() {
            out.truncate(end);
            return false;
        }
        !self.done()
    }
}

/// The keyed state of a sender on its typed output `keyed` ([`TxKeyed`]): keyed while a transmission is on or its end mark
/// is owed, and for a hold after the end mark; sent when it changes and after each configure.
pub struct KeyedOut {
    hold_s: f64,
    hold: usize,
    last: Option<bool>,
}

impl Default for KeyedOut {
    fn default() -> Self {
        KeyedOut::new(KeyedOut::HOLD_S)
    }
}

impl KeyedOut {
    /// The usual hold after the end mark (s).
    pub const HOLD_S: f64 = 0.1;

    /// A keyed state with a hold of `hold_s` seconds after the end mark.
    pub fn new(hold_s: f64) -> Self {
        KeyedOut {
            hold_s,
            hold: 0,
            last: None,
        }
    }

    /// The output port.
    pub fn port() -> Port {
        Port::typed::<TxKeyed>("keyed")
    }

    /// The format of the output port.
    pub fn format() -> Format {
        Format::new(Kind::typed::<TxKeyed>(), 0.0)
    }

    /// Sends the state again on the next pass (after a configure).
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// Whether the receive audio is muted, with `busy` true while a transmission is on or its end mark is owed.
    pub fn keyed(&self, busy: bool) -> bool {
        busy || self.hold > 0
    }

    /// One pass of `room` samples at `rate` Hz: `ended` when the pass wrote an end mark. Writes the state to typed output
    /// `o` of `io` when it changed.
    pub fn pass(&mut self, io: &mut Io, o: usize, busy: bool, ended: bool, room: usize, rate: f64) {
        if ended {
            self.hold = (self.hold_s * rate) as usize;
        } else if !busy {
            self.hold = self.hold.saturating_sub(room);
        }
        let keyed = self.keyed(busy);
        if self.last != Some(keyed) {
            self.last = Some(keyed);
            io.items_out::<TxKeyed>(o).push(Timed {
                time: 0.0,
                value: TxKeyed(keyed),
            });
        }
    }
}

/// A gain that moves towards 1 (open) or 0 (muted) in a ramp of 5 ms, frame by frame.
#[derive(Clone, Debug)]
pub struct MuteRamp {
    step: f32,
    gain: f32,
}

impl Default for MuteRamp {
    fn default() -> Self {
        MuteRamp::new(48e3)
    }
}

impl MuteRamp {
    /// The length of the ramp (s).
    pub const RAMP_S: f64 = 0.005;

    /// A ramp at `rate` frames per second, open.
    pub fn new(rate: f64) -> Self {
        MuteRamp {
            step: (1.0 / (Self::RAMP_S * rate.max(1.0))) as f32,
            gain: 1.0,
        }
    }

    /// The gain now.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Applies the gain to interleaved frames of `channels` samples in place, moving it towards 0 while `muted`.
    pub fn process(&mut self, muted: bool, samples: &mut [f32], channels: u16) {
        let target = if muted { 0.0 } else { 1.0 };
        for frame in samples.chunks_mut(usize::from(channels.max(1))) {
            if self.gain < target {
                self.gain = (self.gain + self.step).min(1.0);
            } else if self.gain > target {
                self.gain = (self.gain - self.step).max(0.0);
            }
            for s in frame {
                *s *= self.gain;
            }
        }
    }
}

/// Mutes audio while a sender is keyed (`tx.mute`): `Audio` in (`in`) and the keyed state ([`TxKeyed`], `keyed`, optional)
/// in, the same audio out with a gain that ramps to 0 in 5 ms while keyed and back to 1 after. A state applies from the
/// pass in which it comes.
#[derive(Default)]
pub struct TxMute {
    ramp: MuteRamp,
    muted: bool,
}

impl Component for TxMute {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "tx.mute",
            vec![
                Port::audio("in"),
                Port::typed::<TxKeyed>("keyed").optional("never muted"),
            ],
            vec![Port::audio("out").same_format_as(0)],
        )
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.ramp = MuteRamp::new(inputs[0].rate);
        self.muted = false;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(k) = io.items::<TxKeyed>(1).last() {
            self.muted = k.value.0;
        }
        let channels = io.audio_out(0).channels;
        let muted = self.muted;
        let x = io.audio_pass_through(0, 0);
        self.ramp.process(muted, x, channels);
    }
    fn values(&self) -> Values {
        no_values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_soft_limiter_keeps_small_samples_and_the_phase() {
        let z = Complex32::new(0.5, -0.6);
        assert_eq!(soft_limit(z), z);
        for m in [0.95f32, 1.2, 3.0, 100.0] {
            let z = Complex32::from_polar(m, 0.7);
            let y = soft_limit(z);
            assert!(y.norm() > SOFT_KNEE && y.norm() <= 1.0 && y.norm() < m);
            assert!((y.arg() - 0.7).abs() < 1e-5);
        }
        let a = soft_limit(Complex32::new(0.95, 0.0)).re;
        let b = soft_limit(Complex32::new(1.0, 0.0)).re;
        assert!(a < b, "the limiter rises with the input");
    }

    #[test]
    fn the_ramp_rises_holds_and_falls_by_raised_cosines() {
        let mut r = Ramp::new(4);
        let rise: Vec<f32> = (0..4).map(|_| r.gain(1.0).unwrap()).collect();
        let want = [
            0.0,
            0.5 - 0.5 * std::f32::consts::FRAC_1_SQRT_2,
            0.5,
            0.5 + 0.5 * std::f32::consts::FRAC_1_SQRT_2,
        ];
        for (g, w) in rise.iter().zip(want) {
            assert!((g - w).abs() < 1e-6, "{rise:?}");
        }
        assert_eq!(r.gain(2.0), Some(2.0));
        r.fall();
        let fall: Vec<f32> = (0..4).map(|_| r.gain(1.0).unwrap()).collect();
        assert!((fall[0] - 1.0).abs() < 1e-6 && (fall[2] - 0.5).abs() < 1e-6);
        assert!(r.done());
        assert_eq!(r.gain(1.0), None);
        assert_eq!(Ramp::of_seconds(0.005, 48e3).samples(), 240);
        assert_eq!(Ramp::new(0).samples(), 1);
    }

    #[test]
    fn shape_cuts_the_block_at_the_end_of_the_fall() {
        let mut r = Ramp::new(3);
        let mut v = vec![Complex32::new(2.0, 0.0); 5];
        assert!(r.shape(1.0, &mut v));
        assert!(v[0].norm() == 0.0 && v[4].norm() <= 1.0 && v[4].norm() > SOFT_KNEE);
        r.fall();
        let mut v = vec![Complex32::new(0.5, 0.0); 5];
        assert!(!r.shape(1.0, &mut v));
        assert_eq!(v.len(), 3);
        let mut v = vec![Complex32::new(0.5, 0.0); 2];
        let mut r = Ramp::new(2);
        r.fall();
        assert!(
            !r.shape(1.0, &mut v),
            "a block that ends with the fall is the last"
        );
        assert_eq!(v.len(), 2);
    }


    /// A ramp 0, 1, 2, ... scaled so it stays inside full scale.
    fn count(n: usize, from: usize) -> Vec<f32> {
        (from..from + n).map(|i| i as f32 * 1e-6).collect()
    }

    /// Host audio at 48 kHz for a sender at 48 kHz comes out sample for sample: no sample is lost or repeated across key
    /// down, and audio before key down is not sent.
    #[test]
    fn audio_at_the_sender_rate_passes_whole_across_key_down() {
        let mut m = MicInput::new(48e3, 0.25);
        m.configure(&Format::audio(48e3, 1));
        // Before key down: the sender drops the audio.
        m.push(&count(480, 0), 1);
        m.restart();
        assert_eq!(m.waiting(), 0);
        // Key down: pieces of host audio in passes of 10 ms, read in frames of 7 ms.
        let mut got = Vec::new();
        let mut frame = [0.0f32; 336];
        let mut next = 0;
        for _ in 0..20 {
            m.push(&count(480, next), 1);
            next += 480;
            while m.waiting() >= frame.len() {
                m.read(&mut frame);
                got.extend_from_slice(&frame);
            }
        }
        let want = count(got.len(), 0);
        assert!(got.len() > 9000, "{} samples", got.len());
        assert_eq!(got, want, "every sample once, in order");
    }

    /// Host audio at 16 kHz in two channels for a sender at 8 kHz: the count is the rate ratio, and a tone comes out whole
    /// with no step where a frame begins.
    #[test]
    fn audio_at_16_khz_comes_out_at_the_frame_rate() {
        let mut m = MicInput::new(8e3, 0.5);
        m.configure(&Format::audio(16e3, 2));
        m.restart();
        let tone = |i: usize| (std::f64::consts::TAU * 400.0 * i as f64 / 16e3).sin() as f32 * 0.5;
        let mut got = Vec::new();
        let mut frame = [0.0f32; 160];
        let mut taken = 0;
        for _ in 0..50 {
            let stereo: Vec<f32> = (taken..taken + 320).flat_map(|i| [tone(i), tone(i)]).collect();
            taken += 320;
            m.push(&stereo, 2);
            while m.waiting() >= frame.len() {
                m.read(&mut frame);
                got.extend_from_slice(&frame);
            }
        }
        let left = m.waiting();
        assert_eq!(got.len() + left, taken / 2 - lag(16e3, 8e3), "half the samples");
        // After the delay of the rate change: a 400 Hz tone at 8 kHz, each sample from the last by the same rotation.
        let c = (std::f64::consts::TAU * 400.0 / 8e3).cos() as f32;
        for w in got[200..].windows(3) {
            let predicted = 2.0 * c * w[1] - w[0];
            assert!((w[2] - predicted).abs() < 2e-3, "{w:?}");
        }
    }

    /// The samples that the rate change from `a` to `b` holds back at the start.
    fn lag(a: f64, b: f64) -> usize {
        let mut r = Resampler::<f32>::new(a, b).unwrap();
        let mut out = Vec::new();
        r.process(&vec![0.0; 16_000], &mut out);
        8000 - out.len()
    }

    /// The keep time bounds the waiting audio; reading more than waits gives silence after it.
    #[test]
    fn old_audio_is_dropped_and_missing_audio_is_silence() {
        let mut m = MicInput::new(48e3, 0.25);
        m.configure(&Format::audio(48e3, 1));
        m.push(&count(24_000, 0), 1);
        assert_eq!(m.waiting(), 12_000);
        let mut x = vec![1.0f32; 12_010];
        m.read(&mut x);
        assert_eq!(x[0], 12_000.0 * 1e-6);
        assert!(x[12_000..].iter().all(|s| *s == 0.0));
        let mut none = MicInput::new(48e3, 0.25);
        none.configure(&Format::iq(48e3, 0.0, 0.0));
        let mut y = [1.0f32; 4];
        none.read(&mut y);
        assert_eq!(y, [0.0; 4]);
    }

    /// A host that times bursts with a lead of 0.2 s.
    struct Lead;

    impl HostStreams for Lead {
        fn write(&self, _: sdrkit_graph::components::HostPort, _: sdrkit_graph::components::HostData<'_>) {}
        fn transmit_lead(&self) -> std::time::Duration {
            std::time::Duration::from_millis(200)
        }
    }

    /// The timing of a live radio at UTC `now`, with a host of lead 0.2 s or none.
    fn timing(now: f64, host: bool) -> BurstTiming {
        let mut t = BurstTiming::default();
        t.attach(&RunContext {
            live: true,
            clock: Clock::fixed(now),
            streams: host.then(|| Arc::new(Lead) as Arc<dyn HostStreams>),
            ..RunContext::default()
        });
        t
    }

    /// A start is stated only on a live radio whose host gives a lead; a burst that waits starts when the one before ends.
    #[test]
    fn bursts_in_a_queue_start_one_after_the_other() {
        assert_eq!(timing(100.0, false).start_time(0.1), None);
        assert_eq!(BurstTiming::default().start_time(0.1), None);
        let mut t = timing(100.0, true);
        let first = t.start_queued(0.1, 0.5).unwrap();
        assert!((first - 100.4).abs() < 1e-9);
        let second = t.start_queued(0.1, 0.25).unwrap();
        assert!((second - (first + 0.5)).abs() < 1e-9);
        let third = t.start_queued(0.1, 0.0).unwrap();
        assert!((third - (second + 0.25)).abs() < 1e-9);
    }

    /// A burst due at a set time is written early by the lead, the age of the newest sample and two passes; none when the
    /// clock of the stack is out of the tolerance or the host gives no lead.
    #[test]
    fn a_burst_due_at_a_set_time_is_written_early_by_the_lead() {
        let t = timing(100.0, true);
        assert!((t.lead_before(99.7, 0.1, 1.0).unwrap() - 0.7).abs() < 1e-9);
        assert!((t.lead_before(100.3, 0.1, 1.0).unwrap() - 0.4).abs() < 1e-9);
        assert_eq!(t.lead_before(98.0, 0.1, 1.0), None);
        assert_eq!(timing(100.0, false).lead_before(99.7, 0.1, 1.0), None);
    }

    /// At equal rates the samples pass unchanged with no tail and no delay; between rates the delay is that of the rate
    /// change in seconds, and the tail flushes it.
    #[test]
    fn equal_rates_pass_the_samples_unchanged() {
        let x: Vec<Complex32> = (0..100).map(|i| Complex32::new(i as f32 * 1e-3, -0.5)).collect();
        let mut same = BurstOut::with_quality(12e3, 12e3, 0.35, 70.0, 1.0).unwrap();
        assert_eq!(same.delay_s(), 0.0);
        same.push(&x);
        same.finish(&mut Vec::new());
        assert!(same.finished() && !same.wants(1000));
        assert_eq!(same.pending, x);
        let mut up = BurstOut::with_quality(12e3, 48e3, 0.35, 70.0, 1.0).unwrap();
        let r = Resampler::<Complex32>::with_quality(12e3, 48e3, 0.35, 70.0).unwrap();
        assert!((up.delay_s() - r.delay() / 12e3).abs() < 1e-12 && up.delay_s() > 0.0);
        up.push(&x);
        let before = up.pending.len();
        up.finish(&mut Vec::new());
        assert!(up.pending.len() > before, "the tail of the rate change");
    }

    /// The ramp reaches 0 within 5 ms of the mute and 1 within 5 ms after it ends, frame by frame.
    #[test]
    fn the_mute_ramps_in_5_ms() {
        let mut r = MuteRamp::new(8e3);
        let mut x = vec![1.0f32; 2 * 80];
        r.process(true, &mut x, 2);
        assert_eq!(x[0], x[1], "both channels of a frame");
        assert!(x[2 * 38] > 0.0 && r.gain() == 0.0);
        let mut y = vec![1.0f32; 100];
        r.process(true, &mut y, 1);
        assert!(y.iter().all(|s| *s == 0.0));
        r.process(false, &mut y, 1);
        let mut z = vec![1.0f32; 50];
        r.process(false, &mut z, 1);
        assert!(z.iter().all(|s| *s == 1.0));
    }
}
