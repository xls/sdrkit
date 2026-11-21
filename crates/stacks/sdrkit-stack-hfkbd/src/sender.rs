//! The transmit side of the keyboard text stacks. A stack's coder turns characters into [`Element`]s (a carrier at an audio
//! frequency for a time, with an envelope), which the [`Writer`] makes samples with a continuous phase, or into samples of its
//! own at [`WAVE_RATE`], which the [`WaveWriter`] brings to the session rate; the [`TextSender`]
//! component queues the typed text, frames each burst (0.15 s of silence, the signal with 10 ms raised-cosine fades at both
//! ends, 0.1 s of silence), sets the peak to the level and writes into the transmit sink.

use crate::typed::{
    KeyboardSendArgs, KeyboardTxEvent, KeyboardTxFields, KEYBOARD_SEND, KEYBOARD_STOP,
    KEYBOARD_TUNE, KEYBOARD_TX,
};
use crate::util::Tone;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys::Family as SourceFamily;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, Complex32, ConfigError, Record, Results, Schema, Setting, SettingError, SettingId,
    StageError, Value, Values,
};
use sdrkit_graph::components::HostStreams;
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext, TagKind,
};
use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::Arc;

/// The most characters that may wait to be sent.
pub const MAX_WAITING: usize = 600;
/// Silence before the signal of a burst, seconds: time for the transmitter to switch.
pub const LEAD_S: f64 = 0.15;
/// Silence after the signal of a burst, seconds.
pub const TAIL_S: f64 = 0.1;
/// The raised-cosine fade at each end of the signal, seconds.
pub const FADE_S: f64 = 0.01;

/// The section of the transmit settings.
pub const TX_SECTION: &str = "Transmit";

/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> = Setting::new("tx_level", "Level", "of full scale", 0.5)
    .range(0.05, 0.9, 0.0)
    .section(TX_SECTION)
    .help("The peak magnitude of the signal sent, of full scale.");

/// The time of the tune carrier.
pub const TX_TUNE_LIMIT: Setting<field::Duration> =
    Setting::new("tx_tune_limit", "Tune time", "s", 10.0)
        .range(1.0, 60.0, 1.0)
        .section(TX_SECTION)
        .help("The tune carrier ends after this time.");

/// Why a text or a tune carrier is not sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// A tune carrier is on (for `send`) or a burst is on air (for `tune`).
    Busy,
    /// The stream rate is below the family's minimum.
    Rate,
    /// The signal of the stack cannot carry this character.
    Character(char),
    /// More than [`MAX_WAITING`] characters would wait.
    Long,
    /// The text has no character to send.
    Empty,
    /// The variant chosen has no sender.
    Variant,
    /// The demodulator is NFM or AM: the signal is sent on USB or LSB only.
    Sideband,
    /// The station callsign is not set.
    NoCall,
    /// The signal at the audio centre chosen does not fit [`BAND_HZ`].
    Band,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a burst or a tune carrier is on air"),
            TxError::Rate => f.write_str("the transmit rate must be at least 12 kHz"),
            TxError::Character(c) => write!(f, "the character {c:?} cannot be sent in this stack"),
            TxError::Long => write!(f, "more than {MAX_WAITING} characters would wait"),
            TxError::Empty => f.write_str("nothing in the text can be sent"),
            TxError::Variant => f.write_str("the variant chosen cannot be sent"),
            TxError::Sideband => f.write_str("the signal is sent on USB or LSB only"),
            TxError::NoCall => f.write_str("the station callsign is not set"),
            TxError::Band => f.write_str("the signal does not fit 200 to 3800 Hz above the dial"),
        }
    }
}

impl std::error::Error for TxError {}

/// The envelope of an element: the complex amplitude that multiplies the carrier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Envelope {
    /// `a` throughout. Zero is silence; the carrier phase runs on.
    Steady(Complex32),
    /// From `from` to `to` by a raised cosine over the element: `from (0.5 + 0.5 cos pi u) + to (0.5 - 0.5 cos pi u)`, u from 0
    /// to 1 (the PSK31 pulse).
    Cosine(Complex32, Complex32),
    /// Amplitude 1 with a raised-cosine rise over the first `rise` seconds and a fall over the last `fall` seconds; 0 is a
    /// square edge. The gain of sample k of an edge of R samples is `0.5 (1 - cos(pi (k + 1) / (R + 1)))`.
    Keyed {
        /// The rise, seconds.
        rise: f64,
        /// The fall, seconds.
        fall: f64,
    },
}

/// One stretch of a signal: the carrier at the audio frequency `hz` (above the dial) for `s` seconds, with an envelope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element {
    /// The audio frequency, Hz.
    pub hz: f64,
    /// The length, seconds.
    pub s: f64,
    /// The envelope.
    pub env: Envelope,
}

impl Element {
    /// A tone of amplitude 1.
    pub fn tone(hz: f64, s: f64) -> Self {
        Element {
            hz,
            s,
            env: Envelope::Steady(Complex32::new(1.0, 0.0)),
        }
    }
    /// Silence while the carrier phase at `hz` runs on.
    pub fn silence(hz: f64, s: f64) -> Self {
        Element {
            hz,
            s,
            env: Envelope::Steady(Complex32::default()),
        }
    }
}

/// A stack's coder: writes the elements of one burst. `char` takes only characters that the stack's modem accepted.
pub trait Coder: Send {
    /// The start of a burst.
    fn begin(&mut self, out: &mut Vec<Element>);
    /// One character.
    fn char(&mut self, c: char, out: &mut Vec<Element>);
    /// The end of a burst.
    fn end(&mut self, out: &mut Vec<Element>);
}

/// The elements of a whole burst of `text`, whose characters the stack accepted.
pub fn burst(coder: &mut dyn Coder, text: &str) -> Vec<Element> {
    let mut v = Vec::new();
    coder.begin(&mut v);
    for c in text.chars() {
        coder.char(c, &mut v);
    }
    coder.end(&mut v);
    v
}

/// The rate of the samples of a [`WaveCoder`], S/s.
pub const WAVE_RATE: f64 = 8000.0;

/// A stack's coder that makes the samples of a burst itself: complex baseband at [`WAVE_RATE`] whose 0 Hz is the audio
/// frequency [`WaveCoder::centre_hz`], with its content within 1100 Hz of it. `char` takes only characters that the stack's
/// modem accepted.
pub trait WaveCoder: Send {
    /// The audio frequency of 0 Hz of the samples, Hz.
    fn centre_hz(&self) -> f64;
    /// The start of a burst.
    fn begin(&mut self, out: &mut Vec<Complex32>);
    /// One character.
    fn char(&mut self, c: char, out: &mut Vec<Complex32>);
    /// The end of a burst.
    fn end(&mut self, out: &mut Vec<Complex32>);
}

/// The coder of a burst: elements of carriers, or samples.
pub enum Coding {
    /// A coder of [`Element`]s.
    Elements(Box<dyn Coder>),
    /// A coder of samples.
    Wave(Box<dyn WaveCoder>),
}

/// The taps of the interpolator of [`WaveWriter`].
const WAVE_TAPS: usize = 16;

/// Writes the samples of a [`WaveCoder`] at the session rate: output sample n is the input read at n [`WAVE_RATE`] / rate by
/// a 16-tap windowed-sinc interpolator (flat to about 1250 Hz, images at least 70 dB down), moved up to the coder's centre, and
/// mirrored on the lower sideband. Input outside the burst reads as zero. Until the end is known, only the samples whose taps
/// are all queued are written.
pub struct WaveWriter {
    rate: f64,
    lower: bool,
    centre: f64,
    osc: Tone,
    interp: sdrkit_algo_dsp::interp::Polyphase<WAVE_TAPS, 64>,
    x: VecDeque<Complex32>,
    /// The input index of `x[0]`.
    base: u64,
    /// The inputs queued.
    total: u64,
    ended: bool,
    n: u64,
}

impl WaveWriter {
    /// A writer at `rate` of samples whose 0 Hz is the audio frequency `centre`, on the lower sideband when `lower`.
    pub fn new(rate: f64, centre: f64, lower: bool) -> Self {
        WaveWriter {
            rate,
            lower,
            centre,
            osc: Tone::new(rate),
            interp: sdrkit_algo_dsp::interp::Polyphase::design(0.6, 70.0)
                .expect("the cut-off is in range"),
            x: VecDeque::new(),
            base: 0,
            total: 0,
            ended: false,
            n: 0,
        }
    }

    /// Queues samples at [`WAVE_RATE`].
    pub fn push(&mut self, s: &[Complex32]) {
        self.x.extend(s.iter().copied());
        self.total += s.len() as u64;
    }

    /// No more samples follow.
    pub fn finish(&mut self) {
        self.ended = true;
    }

    /// The output sample after the last input queued.
    pub fn end_sample(&self) -> u64 {
        (self.total as f64 * self.rate / WAVE_RATE).round() as u64
    }

    /// The samples written.
    pub fn written(&self) -> u64 {
        self.n
    }

    /// The samples that can be written now.
    pub fn pending(&self) -> u64 {
        let ready = if self.ended {
            self.end_sample()
        } else {
            let last = self.total.saturating_sub(WAVE_TAPS as u64 / 2 + 1);
            (last as f64 * self.rate / WAVE_RATE).floor() as u64
        };
        ready.saturating_sub(self.n)
    }

    /// Writes up to `out.len()` samples and returns how many.
    pub fn fill(&mut self, out: &mut [Complex32]) -> usize {
        use sdrkit_algo_dsp::interp::Interpolator as _;
        let k = (self.pending() as usize).min(out.len());
        let half = WAVE_TAPS as i64 / 2 - 1;
        let mut w = [Complex32::default(); WAVE_TAPS];
        for o in &mut out[..k] {
            let pos = self.n as f64 * WAVE_RATE / self.rate;
            let fl = pos.floor();
            let first = fl as i64 - half;
            for (j, v) in w.iter_mut().enumerate() {
                let i = first + j as i64 - self.base as i64;
                *v = if i >= 0 {
                    self.x.get(i as usize).copied().unwrap_or_default()
                } else {
                    Complex32::default()
                };
            }
            let z = self.interp.at(&w, (pos - fl) as f32) * self.osc.next(self.centre, 1.0);
            *o = if self.lower { z.conj() } else { z };
            self.n += 1;
            let keep = (first - 1).max(0) as u64;
            while self.base < keep && !self.x.is_empty() {
                self.x.pop_front();
                self.base += 1;
            }
        }
        k
    }
}

/// Writes elements as complex samples with the dial at 0 Hz, an audio tone of f Hz at +f Hz (at -f Hz on the lower sideband).
/// Element k ends at the sample nearest to the sum of the lengths of elements 0 to k, so no error builds up. The carrier phase
/// is continuous over every element.
pub struct Writer {
    rate: f64,
    lower: bool,
    osc: Tone,
    queue: VecDeque<Element>,
    cur: Option<(Element, u64, u64)>,
    /// The end of the last element taken, seconds.
    taken_s: f64,
    /// The end of the last element queued, seconds.
    queued_s: f64,
    n: u64,
}

impl Writer {
    /// A writer at `rate`, on the lower sideband when `lower`.
    pub fn new(rate: f64, lower: bool) -> Self {
        Writer {
            rate,
            lower,
            osc: Tone::new(rate),
            queue: VecDeque::new(),
            cur: None,
            taken_s: 0.0,
            queued_s: 0.0,
            n: 0,
        }
    }

    /// Queues elements.
    pub fn push(&mut self, e: &[Element]) {
        for x in e {
            self.queued_s += x.s.max(0.0);
            self.queue.push_back(*x);
        }
    }

    /// The sample after the last element queued.
    pub fn end_sample(&self) -> u64 {
        (self.queued_s * self.rate).round() as u64
    }

    /// The samples written.
    pub fn written(&self) -> u64 {
        self.n
    }

    /// The samples queued and not written.
    pub fn pending(&self) -> u64 {
        self.end_sample().saturating_sub(self.n)
    }

    /// Writes up to `out.len()` samples of the queued elements and returns how many.
    pub fn fill(&mut self, out: &mut [Complex32]) -> usize {
        let mut k = 0;
        while k < out.len() {
            let (e, start, end) = match self.cur {
                Some(c) if self.n < c.2 => c,
                _ => {
                    let Some(e) = self.queue.pop_front() else {
                        break;
                    };
                    let start = (self.taken_s * self.rate).round() as u64;
                    self.taken_s += e.s.max(0.0);
                    let end = (self.taken_s * self.rate).round() as u64;
                    self.cur = Some((e, start, end));
                    continue;
                }
            };
            let len = (end - start) as f64;
            let edge = |sec: f64| ((sec * self.rate).round() as u64).min(end - start);
            while k < out.len() && self.n < end {
                let i = self.n - start;
                let a = match e.env {
                    Envelope::Steady(a) => a,
                    Envelope::Cosine(from, to) => {
                        let w = (0.5 + 0.5 * (PI * i as f64 / len).cos()) as f32;
                        from * w + to * (1.0 - w)
                    }
                    Envelope::Keyed { rise, fall } => {
                        let mut g = 1.0;
                        let r = edge(rise);
                        if i < r {
                            g = ramp(i, r);
                        }
                        let f = edge(fall);
                        let j = end - 1 - self.n;
                        if j < f {
                            g = g.min(ramp(j, f));
                        }
                        Complex32::new(g as f32, 0.0)
                    }
                };
                let z = self.osc.next(e.hz, 1.0) * a;
                out[k] = if self.lower { z.conj() } else { z };
                self.n += 1;
                k += 1;
            }
        }
        k
    }
}

/// The gain of sample `k` of a raised-cosine edge of `r` samples.
fn ramp(k: u64, r: u64) -> f64 {
    0.5 * (1.0 - (PI * (k + 1) as f64 / (r + 1) as f64).cos())
}

/// The gain of the fade at sample `i` of `f`: 0.5 - 0.5 cos(pi i / f).
fn fade(i: u64, f: u64) -> f32 {
    if i >= f {
        1.0
    } else {
        (0.5 - 0.5 * (PI * i as f64 / f as f64).cos()) as f32
    }
}

/// The sideband of a demodulator setting: lower for LSB.
///
/// # Errors
///
/// [`TxError::Sideband`] for NFM and AM.
pub fn lower_of(d: crate::front::Demod) -> Result<bool, TxError> {
    match d {
        crate::front::Demod::Usb => Ok(false),
        crate::front::Demod::Lsb => Ok(true),
        _ => Err(TxError::Sideband),
    }
}

/// The audio band a signal must fit, Hz above the dial.
pub const BAND_HZ: (f64, f64) = (200.0, 3800.0);

/// What a command of a sender asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Send the text (it is checked by [`Modem::accept`]).
    Send(String),
    /// End the burst on air.
    Stop,
    /// Send the tune carrier.
    Tune,
}

sdrkit_core::commands! {
    /// The commands of a keyboard sender.
    pub enum KeyboardTxCommand {
        /// Sends a text.
        Send = KEYBOARD_SEND.command,
        /// Stops.
        Stop = KEYBOARD_STOP,
        /// The tune carrier.
        Tune = KEYBOARD_TUNE,
    }
}

/// The settings that every sender has: the level and the tune time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Common {
    /// The peak, of full scale.
    pub level: f64,
    /// The tune time, seconds.
    pub tune_s: f64,
}

impl Default for Common {
    fn default() -> Self {
        Common {
            level: TX_LEVEL.default,
            tune_s: TX_TUNE_LIMIT.default,
        }
    }
}

/// A stack's side of a sender: its settings, the characters it takes and its coder.
pub trait Modem: Send + 'static {
    /// The component's name ("hfkbd.rttytx").
    const NAME: &'static str;
    /// The settings the sender reads.
    fn schema() -> Arc<Schema>;
    /// Applies a setting.
    ///
    /// # Errors
    ///
    /// The errors of the setting.
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError>;
    /// The values of the settings.
    fn values(&self) -> Values;
    /// The level and the tune time.
    fn common(&self) -> Common;
    /// The commands.
    fn commands(&self) -> &'static Commands {
        KeyboardTxCommand::commands()
    }
    /// What command `id` with `args` asks; `station` is the station callsign.
    ///
    /// # Errors
    ///
    /// The errors of the arguments, or [`TxError`] as [`CommandError::Failed`].
    fn action(&self, id: CommandId, args: &Values, station: &str) -> Result<Action, CommandError> {
        let _ = station;
        Ok(
            match KeyboardTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                KeyboardTxCommand::Send => {
                    let a: KeyboardSendArgs = KEYBOARD_SEND.args(args)?;
                    Action::Send(a.text.to_owned())
                }
                KeyboardTxCommand::Stop => Action::Stop,
                KeyboardTxCommand::Tune => Action::Tune,
            },
        )
    }
    /// Appends the characters of `text` that a burst sends, after the folds the stack states (upper case for RTTY, a line end
    /// as CR LF ...).
    ///
    /// # Errors
    ///
    /// [`TxError::Character`] for the first character the signal cannot carry; nothing is appended then.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), TxError>;
    /// A coder for a new burst with the settings now.
    ///
    /// # Errors
    ///
    /// [`TxError::Variant`] when the variant has no sender, [`TxError::Sideband`] when the demodulator is not USB or LSB.
    fn coder(&self) -> Result<Coding, TxError>;
    /// The audio centre, Hz: the tune carrier.
    fn centre_hz(&self) -> f64;
    /// The lowest and the highest audio frequency of the signal, Hz.
    fn band(&self) -> (f64, f64);
    /// The signal is on the lower sideband.
    fn lower(&self) -> bool {
        false
    }
}

/// What makes the samples of a burst.
enum Signal {
    /// The tune carrier.
    Tune(Writer),
    /// A coder of elements and their writer.
    Elements(Box<dyn Coder>, Writer),
    /// A coder of samples and their writer.
    Wave(Box<dyn WaveCoder>, WaveWriter),
}

impl Signal {
    fn written(&self) -> u64 {
        match self {
            Signal::Tune(w) | Signal::Elements(_, w) => w.written(),
            Signal::Wave(_, w) => w.written(),
        }
    }
    fn end_sample(&self) -> u64 {
        match self {
            Signal::Tune(w) | Signal::Elements(_, w) => w.end_sample(),
            Signal::Wave(_, w) => w.end_sample(),
        }
    }
    fn pending(&self) -> u64 {
        match self {
            Signal::Tune(w) | Signal::Elements(_, w) => w.pending(),
            Signal::Wave(_, w) => w.pending(),
        }
    }
    fn fill(&mut self, out: &mut [Complex32]) -> usize {
        match self {
            Signal::Tune(w) | Signal::Elements(_, w) => w.fill(out),
            Signal::Wave(_, w) => w.fill(out),
        }
    }
    /// Whether characters are coded (not the tune carrier).
    fn codes(&self) -> bool {
        !matches!(self, Signal::Tune(_))
    }
    /// The start, one character (`Some`) or the end (`None`) of the coder; `els` and `wave` are scratch.
    fn code(&mut self, step: Step, els: &mut Vec<Element>, wave: &mut Vec<Complex32>) {
        match self {
            Signal::Tune(_) => {}
            Signal::Elements(c, w) => {
                els.clear();
                match step {
                    Step::Begin => c.begin(els),
                    Step::Char(ch) => c.char(ch, els),
                    Step::End => c.end(els),
                }
                w.push(els);
            }
            Signal::Wave(c, w) => {
                wave.clear();
                match step {
                    Step::Begin => c.begin(wave),
                    Step::Char(ch) => c.char(ch, wave),
                    Step::End => c.end(wave),
                }
                w.push(wave);
                if step == Step::End {
                    w.finish();
                }
            }
        }
    }
    /// Ends the signal at sample `at` at the latest: what is queued is kept, and silence follows when it ends sooner.
    fn close(&mut self, at: u64, rate: f64) {
        let short = at.saturating_sub(self.end_sample());
        match self {
            Signal::Tune(w) | Signal::Elements(_, w) => {
                if short > 0 {
                    w.push(&[Element::silence(0.0, short as f64 / rate)]);
                }
            }
            Signal::Wave(_, w) => {
                let n = (short as f64 * WAVE_RATE / rate).ceil() as usize;
                w.push(&vec![Complex32::default(); n]);
                w.finish();
            }
        }
    }
}

/// A step of a coder.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Begin,
    Char(char),
    End,
}

/// A burst on air.
struct TxBurst {
    sig: Signal,
    /// Silent samples before the signal, still to write.
    lead: u64,
    /// The fade, samples.
    fade: u64,
    /// The end of the signal, once known.
    end: Option<u64>,
    /// Silent samples after the signal, still to write.
    tail: u64,
    /// Characters coded.
    sent: usize,
    /// The text of the burst.
    text: String,
    /// Samples written.
    out: u64,
    level: f32,
}

/// The sender (`hfkbd.<stack>tx`): input `in` is the session's samples (their rate, pace and dial), output `out` the complex
/// baseband for `tx.sink`, centred on the dial; output `events` the `hfkbd.tx` events. `send` checks the whole text and queues
/// it; a burst starts when text waits and none is on air, and takes the characters as it goes, so text sent during a burst
/// joins it. When nothing waits the coder's end follows and the burst ends; text sent after that starts a new burst. `stop`
/// drops the waiting text and ends the signal with a 10 ms fade and the end mark, without the silence after it. `tune` sends
/// a carrier at the audio centre for the tune time. Each pass writes as many samples as it brings in. On a live radio whose
/// host times bursts, the first sample states its time on air (`TagKind::BurstStart`): the time of the pass that writes it
/// plus the lead of the host and two passes. The engine's transmit setting is the only switch: off, `send` is refused and a
/// burst ends at once.
pub struct TextSender<M: Modem> {
    modem: M,
    rate: f64,
    dial: f64,
    transmit: bool,
    live: bool,
    wall: Clock,
    host: Option<Arc<dyn HostStreams>>,
    station: String,
    queue: VecDeque<char>,
    burst: Option<TxBurst>,
    end_owed: bool,
    events: Vec<sdrkit_core::Event>,
    results: Published,
    shown: Option<(bool, usize, usize)>,
    scratch: Vec<Element>,
    wave: Vec<Complex32>,
    accepted: Vec<char>,
}

impl<M: Modem + Default> Default for TextSender<M> {
    fn default() -> Self {
        Self::new(M::default())
    }
}

impl<M: Modem> TextSender<M> {
    /// A sender of `modem`.
    pub fn new(modem: M) -> Self {
        let mut s = TextSender {
            modem,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            live: false,
            wall: Clock::fixed(0.0),
            host: None,
            station: String::new(),
            queue: VecDeque::new(),
            burst: None,
            end_owed: false,
            events: Vec::new(),
            results: Published::new(KeyboardTxFields::SHAPE.fields),
            shown: None,
            scratch: Vec::new(),
            wave: Vec::new(),
            accepted: Vec::new(),
        };
        s.publish();
        s
    }

    fn event(&mut self, state: &str, reason: &str, text: &str) {
        let hz = self.dial + self.modem.centre_hz();
        self.events.push(KEYBOARD_TX.event(
            0.0,
            hz,
            &KeyboardTxEvent {
                state,
                reason,
                text,
                family: SourceFamily::Message,
            },
        ));
    }

    /// The time on air of a burst whose first sample is written in a pass of `pass` seconds: the clock, the lead of the host
    /// and two passes. `None` when the radio is not live or the host gives no lead.
    fn start_time(&self, pass: f64) -> Option<f64> {
        let lead = self.host.as_ref()?.transmit_lead().as_secs_f64();
        (self.live && lead > 0.0).then(|| self.wall.now() + lead + 2.0 * pass)
    }

    /// Ends the burst on air at once; its end mark goes out on the next pass when samples of it were written.
    fn cut(&mut self, reason: &str) {
        self.queue.clear();
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out > 0;
            self.event("finished", reason, &b.text);
        }
    }

    /// A new burst: the tune carrier when `tune`, else the coder's start.
    fn start(&mut self, tune: bool) -> Result<(), TxError> {
        let lower = self.modem.lower();
        let mut end = None;
        let sig = if tune {
            let mut w = Writer::new(self.rate, lower);
            let s = self.modem.common().tune_s;
            w.push(&[Element::tone(self.modem.centre_hz(), s)]);
            end = Some(w.end_sample());
            Signal::Tune(w)
        } else {
            match self.modem.coder()? {
                Coding::Elements(c) => Signal::Elements(c, Writer::new(self.rate, lower)),
                Coding::Wave(c) => {
                    let w = WaveWriter::new(self.rate, c.centre_hz(), lower);
                    Signal::Wave(c, w)
                }
            }
        };
        let mut b = TxBurst {
            sig,
            lead: (LEAD_S * self.rate).round() as u64,
            fade: (FADE_S * self.rate).round() as u64,
            end,
            tail: (TAIL_S * self.rate).round() as u64,
            sent: 0,
            text: String::new(),
            out: 0,
            level: self.modem.common().level as f32,
        };
        b.sig.code(Step::Begin, &mut self.scratch, &mut self.wave);
        self.burst = Some(b);
        Ok(())
    }

    fn check_send(&self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.rate < crate::front::MIN_RATE {
            return Err(TxError::Rate);
        }
        let (lo, hi) = self.modem.band();
        if lo < BAND_HZ.0 || hi > BAND_HZ.1 {
            return Err(TxError::Band);
        }
        Ok(())
    }

    /// Queues `text`; starts a burst when none is on air.
    fn send(&mut self, text: &str) -> Result<bool, TxError> {
        self.check_send()?;
        if self.burst.as_ref().is_some_and(|b| !b.sig.codes()) {
            return Err(TxError::Busy);
        }
        if self.burst.is_none() {
            // The settings are checked first, so a refusal leaves nothing queued.
            self.modem.coder()?;
        }
        let mut acc = std::mem::take(&mut self.accepted);
        acc.clear();
        let r = self.modem.accept(text, &mut acc);
        let r = r.and_then(|()| {
            if acc.is_empty() {
                Err(TxError::Empty)
            } else if self.queue.len() + acc.len() > MAX_WAITING {
                Err(TxError::Long)
            } else {
                Ok(())
            }
        });
        if r.is_ok() {
            self.queue.extend(acc.iter().copied());
        }
        self.accepted = acc;
        r?;
        if self.burst.is_none() {
            self.start(false)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn tune(&mut self) -> Result<(), TxError> {
        self.check_send()?;
        if self.burst.is_some() {
            return Err(TxError::Busy);
        }
        self.start(true)
    }

    /// Stops the burst on air: the waiting text is dropped and the signal ends with the fade.
    fn stop(&mut self) {
        self.queue.clear();
        let Some(b) = &mut self.burst else {
            return;
        };
        if b.sig.written() == 0 {
            // No signal yet: the burst ends now.
            let out = b.out;
            let text = std::mem::take(&mut b.text);
            self.burst = None;
            self.end_owed |= out > 0;
            self.event("finished", "stopped", &text);
            return;
        }
        let now = b.sig.written() + b.fade;
        b.end = Some(b.end.map_or(now, |e| e.min(now)));
        b.tail = 0;
        b.sig.close(now, self.rate);
    }

    /// Writes the result fields when they changed.
    fn publish(&mut self) {
        let now = (
            self.burst.is_some(),
            self.burst.as_ref().map_or(0, |b| b.sent),
            self.queue.len(),
        );
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.results.put(&KeyboardTxFields {
            tx_sending: now.0,
            tx_sent: now.1 as i64,
            tx_waiting: now.2 as i64,
        });
    }

    /// Writes the end mark on the last sample of output 0, or on one silent sample when the pass wrote none.
    fn end_mark(io: &mut Io) {
        let out = io.iq_out(0);
        if out.is_empty() {
            out.push(Complex32::default());
        }
        let last = out.len() - 1;
        io.output(0).tag_here(last, TagKind::BurstEnd);
    }

    /// Writes `room` samples of the burst into `out` from `first`; returns how many and whether the burst ended.
    fn write(&mut self, out: &mut Vec<Complex32>, room: usize) -> (usize, bool) {
        let first = out.len();
        out.resize(first + room, Complex32::default());
        let Some(b) = &mut self.burst else {
            out.truncate(first);
            return (0, false);
        };
        let mut k = 0;
        let mut done = false;
        while k < room {
            if b.lead > 0 {
                let n = (b.lead as usize).min(room - k);
                b.lead -= n as u64;
                k += n;
                continue;
            }
            if b.end.is_none() {
                let want = (room - k) as u64 + b.fade;
                while b.sig.pending() < want && b.sig.codes() {
                    match self.queue.pop_front() {
                        Some(ch) => {
                            b.sig
                                .code(Step::Char(ch), &mut self.scratch, &mut self.wave);
                            b.sent += 1;
                            b.text.push(ch);
                        }
                        None => {
                            b.sig.code(Step::End, &mut self.scratch, &mut self.wave);
                            b.end = Some(b.sig.end_sample());
                            break;
                        }
                    }
                }
            }
            let end = b.end.unwrap_or(u64::MAX);
            if b.sig.written() < end {
                let n0 = b.sig.written();
                let lim = ((end - n0) as usize).min(room - k);
                let n = b.sig.fill(&mut out[first + k..first + k + lim]);
                for (i, z) in out[first + k..first + k + n].iter_mut().enumerate() {
                    let at = n0 + i as u64;
                    let mut g = b.level * fade(at, b.fade);
                    if let Some(e) = b.end {
                        g *= fade(e - 1 - at, b.fade);
                    }
                    *z *= g;
                }
                k += n;
                if n == 0 {
                    break;
                }
                continue;
            }
            if b.tail > 0 {
                let n = (b.tail as usize).min(room - k);
                b.tail -= n as u64;
                k += n;
                continue;
            }
            done = true;
            break;
        }
        out.truncate(first + k);
        b.out += k as u64;
        (k, done)
    }
}

impl<M: Modem> Component for TextSender<M> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            M::NAME,
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(M::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        if self.burst.is_some() {
            self.cut("the format changed");
        }
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.live = ctx.live;
        self.wall = ctx.clock.clone();
        self.host.clone_from(&ctx.streams);
        ctx.station.callsign.trim().clone_into(&mut self.station);
        if !ctx.transmit && (self.burst.is_some() || !self.queue.is_empty()) {
            self.cut("the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.end_owed) {
            Self::end_mark(io);
        }
        let room = io.input_len(0);
        if self.burst.is_none() && !self.queue.is_empty() && self.transmit {
            if let Err(e) = self.start(false) {
                self.queue.clear();
                self.event("refused", &e.to_string(), "");
            }
        }
        let stated = self
            .burst
            .as_ref()
            .filter(|b| b.out == 0)
            .and_then(|_| self.start_time(room as f64 / self.rate.max(1.0)));
        let fresh = self.burst.as_ref().is_some_and(|b| b.out == 0);
        let mut out = std::mem::take(io.iq_out(0));
        let first = out.len();
        let (n, done) = self.write(&mut out, room);
        *io.iq_out(0) = out;
        if fresh && n > 0 {
            if let Some(unix_s) = stated {
                io.output(0).tag_here(first, TagKind::BurstStart { unix_s });
            }
        }
        if done {
            Self::end_mark(io);
            if let Some(b) = self.burst.take() {
                self.event("finished", "", &b.text);
            }
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.modem.apply(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.modem.values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        self.modem.commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let action = self.modem.action(id, args, &self.station)?;
        let r = match action {
            Action::Send(text) => match self.send(&text) {
                Ok(started) => {
                    self.event(if started { "started" } else { "queued" }, "", &text);
                    Ok(())
                }
                Err(e) => {
                    self.event("refused", &e.to_string(), &text);
                    Err(e)
                }
            },
            Action::Stop => {
                self.stop();
                Ok(())
            }
            Action::Tune => match self.tune() {
                Ok(()) => {
                    self.event("started", "tune carrier", "");
                    Ok(())
                }
                Err(e) => {
                    self.event("refused", &e.to_string(), "");
                    Err(e)
                }
            },
        };
        self.publish();
        r.map(|()| None).map_err(|e| CommandError::Failed {
            source: StageError::new(e),
        })
    }
}
