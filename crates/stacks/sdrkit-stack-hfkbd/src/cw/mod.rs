//! CW (Morse): a skimmer that finds every keyed carrier in a span of 14 to 192 kHz around the dial and decodes each at its own
//! speed, a decoder of one carrier for other graphs, and the test signal.
//!
//! The skimmer finds carriers in the power spectrum of the wide stream (bins of about 12 Hz at every span). A shared
//! fast-convolution bank cuts a channel of ±300 Hz at 2 or 3 kHz out of the wide stream for each carrier, so the cost of a
//! carrier does not grow with the span. The receive chain of one carrier is [`receiver::MorseReceiver`]: the channel filter of
//! the chosen width, a key detector and a timing decoder. A new carrier gets the last second of its channel from the bank,
//! so its first elements are not lost.
//!
//! Standard: ITU-R M.1677-1 (International Morse code).

pub mod receiver;

use crate::decoder::HeardCommand;
use crate::decoder::{
    self, last_call, put_text, retain_rows, text_into, Emit, Heard, Modulator, GEN_MESSAGE,
    GEN_SECTION,
};
use crate::front::{Span, AUDIO_RATE, SPAN};
use crate::typed::{
    CwFields, CwSignalRow, FrontFields, MorseFields, Playing, CW_GAPS, CW_KEY, CW_MARKS,
    CW_SIGNALS, CW_SPECTRUM, LINE, SPOT, STATIONS,
};
use crate::util::{db, median};
use receiver::{ChannelFilter, FilterWidth, MorseReceiver};
use sdrkit_algo_dsp::agc::Agc;
use sdrkit_algo_dsp::bank::OverlapSaveBank;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::modulate::Ook;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, ResultsDecl, Schema, SeriesId,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_text::morse::{Event, Symbol, HIST_BINS, HIST_MIN, HIST_SPAN};
use sdrkit_util_text::morse::{Key, Keyer};
use sdrkit_util_text::store::TextStore;
use std::sync::Arc;

/// The most carriers followed at once.
pub const MAX_SIGNALS: usize = 100;
/// The most signals gone whose rows stay in the table.
const GONE_CAP: usize = 100;
/// The rows of the signals table: the signals heard and those gone whose text stays.
pub const SIGNALS_CAP: usize = MAX_SIGNALS + GONE_CAP;
/// The newest characters kept of each signal.
const SIGNAL_CHARS: usize = 600;
/// A signal with no key down for this long is gone: its receiver stops and its row stays, marked gone.
pub const QUIET_S: f64 = 30.0;
/// A signal shows in the table once it decoded this many clear characters other than E, T and I (the readings of noise), at
/// least [`MIXED_CHARS`] of them of both dots and dashes, at a known speed of at most [`SPEED_MARGIN`] times the top speed of
/// the filter.
const GOOD_CHARS: u32 = 4;
/// See [`GOOD_CHARS`]: a text of characters of one element kind (S, H, M, O and the like) does not prove a signal.
const MIXED_CHARS: u32 = 2;
/// See [`GOOD_CHARS`]: a filter does not pass keying much faster than its top speed, so a faster reading is noise.
const SPEED_MARGIN: f64 = 1.25;
/// A signal that has not shown in the table after this long is dropped, s.
const UNPROVEN_S: f64 = 30.0;
/// The key history of a signal: one bit every 10 ms, the newest 128.
const TICK_S: f64 = 0.01;
/// A signal whose key follows a stronger one within this many of the 128 ticks is part of it.
const FOLLOW_TICKS: u32 = 8;
/// A signal whose key changes when the keys of much stronger signals change, far more often than chance gives, is a mixing
/// product of them (an intermodulation or converter product of the radio). A change coincides with one of a stronger signal
/// within [`EDGE_S`]; chance gives 1 - exp(-2 [`EDGE_S`] R) for the rate R of the changes of the stronger signals. After
/// [`PRODUCT_EDGES`] changes it is dropped when four in five of them coincide and the count is four standard deviations over
/// chance; it is not shown while three in five coincide and the count is two standard deviations over chance.
const PRODUCT_EDGES: u64 = 40;
/// See [`PRODUCT_EDGES`], s.
const EDGE_S: f64 = 0.010;
/// The power of a signal over that of its products, at least (30 dB).
const PRODUCT_POWER: f32 = 1000.0;
/// The key changes of a signal that are kept for [`PRODUCT_EDGES`].
const EDGES: usize = 32;
/// A signal not shown whose key has not changed since it started, for this long, is dropped (a steady carrier, such as the
/// centre of a direct-conversion radio, or a quiet bin), s. Its bin is not tried again while its level stays within [`STEADY_DB`].
const STEADY_S: f64 = 5.0;
/// See [`STEADY_S`], dB.
const STEADY_DB: f64 = 6.0;
/// The most points of the spectrum series.
const SPECTRUM_POINTS: usize = 4096;
/// The key changes of the signal listened to that the key series holds, and the time they span, s.
const KEY_EDGES: usize = 64;
const KEY_SPAN_S: f64 = 2.0;
/// The pass and stop edges of the channel that the bank cuts for each carrier, Hz. The bank gives it at 2 to 4 times the stop
/// edge: 2 kHz, or 3 kHz from 24 kHz.
const CHANNEL_PASS: f64 = 300.0;
const CHANNEL_STOP: f64 = 1000.0;
/// The attenuation of the bank's channel filter, dB.
const CHANNEL_ATTEN: f64 = 60.0;
/// The bank's channel of the dial. The bank's channel `i + 1` is that of signal `i`.
const DIAL: usize = 0;
/// The level, attack (s) and decay (s) of the gain control of the real signal. The rebuilt tone is at the same level.
const AGC_TARGET: f32 = 0.3;
const AGC_ATTACK_S: f64 = 0.05;
const AGC_DECAY_S: f64 = 3.0;
/// At a change of what the audio plays, the gain control starts again with this attack for [`HANDOVER_S`], the level is held
/// at most at [`AGC_TARGET`] meanwhile, and the audio fades in over [`FADE_S`], s.
const HANDOVER_ATTACK_S: f64 = 0.002;
const HANDOVER_S: f64 = 0.5;
const FADE_S: f64 = 0.03;
/// Carriers closer than this are one signal, Hz.
const SAME_SIGNAL_HZ: f64 = 60.0;
/// The keying sidebands of a carrier: a peak within 100 Hz of a stronger one and 10 dB weaker, or within 250 Hz and 20 dB
/// weaker, belongs to it.
const SKIRT: [(f64, f32); 2] = [(100.0, 0.1), (250.0, 0.01)];

sdrkit_core::choice_enum! {
    /// The timing decoder of a Morse receiver.
    pub enum TimingDecoder {
        /// Adaptive.
        Adaptive = ("adaptive", "Adaptive"),
        /// Clustering.
        Classic = ("classic", "Clustering"),
    }
}

sdrkit_core::choice_enum! {
    /// What the skimmer plays as audio.
    pub enum Listen {
        /// Silence.
        Off = ("off", "Off"),
        /// A clean tone at the pitch, keyed as the signal listened to; silence while there is none.
        Tone = ("tone", "Rebuilt tone"),
        /// The signal listened to through its channel filter, moved to the pitch, with a slow gain control; the dial the
        /// same way while there is none.
        Signal = ("signal", "Real signal"),
    }
}

/// The `threshold` setting.
pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Threshold", "dB", 10.0)
    .range(6.0, 24.0, 0.5)
    .help("How far above the noise floor a carrier must be to be decoded.");
/// The `decoder` setting.
pub const DECODER: Setting<field::Choice<TimingDecoder>> =
    Setting::choice("decoder", "Timing decoder", TimingDecoder::Adaptive);
/// The `wpm` setting.
pub const WPM_SETTING: Setting<field::F64> = Setting::new("wpm", "Speed", "wpm", 0.0)
    .range(0.0, 50.0, 1.0)
    .help("0 finds the speed by itself. 5 to 50 fixes it.");
/// The `filter` setting.
pub const FILTER: Setting<field::Choice<FilterWidth>> =
    Setting::choice("filter", "Filter width", FilterWidth::Hz100).help(
        "The channel filter of each carrier. 50 Hz is the most sensitive and passes keying up to 20 WPM, 100 Hz up to \
         40 WPM, 250 and 500 Hz up to 60 WPM.",
    );
/// The `listen` setting.
pub const LISTEN: Setting<field::Choice<Listen>> = Setting::choice("listen", "Listen", Listen::Off)
    .help(
        "Off is silence. Tone is a clean tone at the pitch keyed as the signal listened to, and silence while there is \
         none, as there is no key to rebuild. Signal is the signal listened to through its filter at the pitch, and the \
         dial through the filter while there is none. The signal listened to is the focus, else the strongest signal in \
         the table.",
    );
/// The `pitch` setting.
pub const PITCH: Setting<field::Hz> = Setting::new("pitch", "Pitch", "Hz", 700.0)
    .range(400.0, 1000.0, 10.0)
    .help("The audio frequency of the signal listened to, and of a carrier at the dial.");
/// The `focus_hz` setting.
pub const FOCUS_HZ: Setting<field::Hz> = Setting::new("focus_hz", "Focus", "Hz", 0.0)
    .range(-96000.0, 96000.0, 0.0)
    .zero_means("No focus")
    .help("The signal nearest this offset is the focus. A focus where no signal is starts one there, marked as picked.");
/// The `forget_s` setting.
pub const FORGET_S: Setting<field::F64> = Setting::new("forget_s", "Forget after", "s", 600.0)
    .range(30.0, 3600.0, 10.0)
    .help("A signal not heard for this long leaves the signals table. After 30 s without a key down it is marked gone.");

sdrkit_core::settings! {
    /// The settings of [`CwDecoder`].
    pub enum CwSetting {
        /// The threshold.
        Threshold = THRESHOLD,
        /// The timing decoder.
        Decoder = DECODER,
        /// The speed.
        Wpm = WPM_SETTING,
        /// The filter width.
        Filter = FILTER,
        /// Listen.
        Listen = LISTEN,
        /// The pitch.
        Pitch = PITCH,
        /// The focus.
        FocusHz = FOCUS_HZ,
        /// The time after which a signal leaves the table.
        ForgetS = FORGET_S,
    }
}

/// The carrier of the test signal that sends `gen_message`.
pub const GEN_CARRIER_HZ: Setting<field::Hz> =
    Setting::new("gen_carrier_hz", "Carrier", "Hz", 400.0)
        .range(-96000.0, 96000.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The character speed of the test signal.
pub const GEN_WPM: Setting<field::F64> = Setting::new("gen_wpm", "Speed", "wpm", 20.0)
    .range(5.0, 60.0, 0.0)
    .section(GEN_SECTION)
    .advanced();
/// The Farnsworth speed of the test signal, 0 for the character speed.
pub const GEN_FARNSWORTH_WPM: Setting<field::F64> =
    Setting::new("gen_farnsworth_wpm", "Farnsworth speed", "wpm", 0.0)
        .range(0.0, 60.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The timing jitter of the test signal, a share of a dot.
pub const GEN_JITTER: Setting<field::F64> = Setting::new("gen_jitter", "Jitter", "", 0.0)
    .range(0.0, 0.5, 0.0)
    .section(GEN_SECTION)
    .advanced();
/// The number of carriers of the test signal, 0 for the default signal.
pub const GEN_SIGNALS: Setting<field::I64> = Setting::new("gen_signals", "Carriers", "", 0)
    .range(0.0, 100.0, 1.0)
    .section(GEN_SECTION)
    .help("0: the default test signal. 1 to 100: carriers spread evenly over the span, each with its own call sign.")
    .advanced();
/// The top speed of a test signal of several carriers.
pub const GEN_WPM_MAX: Setting<field::F64> = Setting::new("gen_wpm_max", "Top speed", "wpm", 0.0)
    .range(0.0, 60.0, 0.0)
    .section(GEN_SECTION)
    .help("With several carriers, the speeds step from the speed up to this (0: all at the speed).")
    .advanced();
/// No carrier in the test signal.
pub const GEN_NOISE_ONLY: Setting<field::Bool> =
    Setting::new("gen_noise_only", "Noise only", "", false)
        .section(GEN_SECTION)
        .help(
            "No carrier: only the noise of the test signal settings, as loud as with the carriers.",
        )
        .advanced();
/// The level spread of a test signal of several carriers.
pub const GEN_LEVEL_SPREAD_DB: Setting<field::F64> =
    Setting::new("gen_level_spread_db", "Level spread", "dB", 0.0)
        .range(0.0, 40.0, 0.0)
        .section(GEN_SECTION)
        .help("With several carriers, the levels step down evenly over this many dB.")
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the CW stack.
    pub enum CwGenSetting {
        /// The text sent. Empty: three carriers.
        Message = GEN_MESSAGE,
        /// The carrier.
        CarrierHz = GEN_CARRIER_HZ,
        /// The character speed.
        Wpm = GEN_WPM,
        /// The Farnsworth speed.
        FarnsworthWpm = GEN_FARNSWORTH_WPM,
        /// The jitter.
        Jitter = GEN_JITTER,
        /// The number of carriers.
        Signals = GEN_SIGNALS,
        /// The top speed of several carriers.
        WpmMax = GEN_WPM_MAX,
        /// The level spread of several carriers.
        LevelSpreadDb = GEN_LEVEL_SPREAD_DB,
        /// No carrier.
        NoiseOnly = GEN_NOISE_ONLY,
    }
}

/// What a decoded symbol becomes in a text store and the outputs: its bytes as characters, each with the confidence of the symbol.
#[allow(clippy::too_many_arguments)]
fn emit_symbol(
    sym: &Event,
    store: &mut TextStore,
    out: &mut Outbox,
    heard: &mut Heard,
    time: f64,
    id: u32,
    freq_hz: f64,
    snr_db: f32,
) -> u64 {
    let mut chars = 0;
    let (text, confidence) = match sym {
        Event::Char(s) => (s.text_or("*"), s.confidence),
        Event::Space => (" ", 9),
    };
    for &b in text.as_bytes() {
        chars += (text != " ") as u64;
        heard.char(store, out, b, time, confidence * 11, id, freq_hz, snr_db);
    }
    chars
}

/// Whether `k` is a known character of both dots and dashes.
fn mixed(k: &Symbol) -> bool {
    let all = (1u32 << k.code.len) - 1;
    k.text.is_some() && k.code.bits != 0 && u32::from(k.code.bits) != all
}

/// One carrier of the skimmer: its receiver, its text and what the screen shows of it.
pub struct CwChannel {
    /// Stable id.
    pub id: u32,
    /// The offset of the carrier from the dial, Hz.
    freq: f64,
    /// The receiver of the channel that the bank cuts.
    pub rx: MorseReceiver,
    /// The text.
    pub store: TextStore,
    /// Confidence digits of the newest text.
    pub confidence: String,
    /// Alternative readings of the newest text.
    pub alts: String,
    /// Picked by the user.
    pub manual: bool,
    /// The signal was heard on a frequency before the present one. Its text is a log entry: no new signal takes it.
    pub past: bool,
    /// Signal time when it was first heard.
    pub first: f64,
    /// Signal time when its key was last down.
    pub last: f64,
    /// The input index of its next channel sample, and the input samples between two channel samples.
    clock: u64,
    step: u64,
    /// It decoded [`GOOD_CHARS`] clear characters at a known speed: it shows in the table.
    pub valid: bool,
    good: u32,
    /// The characters counted in `good` that have both dots and dashes.
    mixed: u32,
    /// The key state every 10 ms, newest in bit 0; the ticks taken; the input index of the next tick.
    keys: u128,
    ticks: u32,
    next_tick: u64,
    /// The power of the filtered carrier while the key is down, followed over about 50 envelope samples.
    power: f32,
    /// The key state now.
    on: bool,
    /// The signal times of its newest key changes (a ring of [`EDGES`]), the changes so far, the changes compared with
    /// those of the stronger signals, those of them that coincided, and the mean and variance of that count by chance (see
    /// [`PRODUCT_EDGES`]).
    edges: [f64; EDGES],
    n_edges: u64,
    compared: u64,
    coinciding: u64,
    chance: f64,
    chance_var: f64,
}

impl CwChannel {
    /// The level of the carrier while the key is down, dB relative to full scale (-200 before the key was down).
    pub fn level_dbfs(&self) -> f64 {
        db(f64::from(self.power))
    }
}

impl CwChannel {
    /// The offset of the carrier from the dial, Hz.
    pub fn freq_hz(&self) -> f64 {
        self.freq
    }
}

/// Keeps the confidence digit and the other reading of a symbol: the newest 60 digits and 120 bytes of readings.
fn remember(confidence: &mut String, alts: &mut String, sym: &Event) {
    let (c, alt) = match sym {
        Event::Char(s) => (s.confidence, s.alt),
        Event::Space => (9, None),
    };
    confidence.push((b'0' + c) as char);
    alts.push_str(alt.unwrap_or("."));
    if confidence.len() > 60 {
        confidence.drain(..confidence.len() - 60);
    }
    if alts.len() > 120 {
        let cut = alts.len() - 120;
        let cut = (cut..alts.len())
            .find(|&i| alts.is_char_boundary(i))
            .unwrap_or(0);
        alts.drain(..cut);
    }
}

/// What the audio plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Nothing,
    Dial,
    Signal(u32),
}

/// The audio of the skimmer: a baseband at the envelope rate of what it plays (the filtered carrier of the signal listened to
/// or of the dial, or a keyed level for the rebuilt tone), brought to 8 kHz, kept at an even level, moved up to the pitch.
/// Silence fills the time that no baseband covers, so the audio keeps the pace of the input with a fixed delay. A change of
/// source fades in.
struct AudioOut {
    source: Source,
    /// The gain of the fade, 0 to 1, and the audio samples left with the fast attack of a change of source.
    fade: f32,
    handover: u32,
    base: Vec<Complex32>,
    rs: Option<Resampler<Complex32>>,
    rs_rate: f64,
    up: Vec<Complex32>,
    agc: Agc,
    nco: Nco,
    ramp: f32,
    /// Audio samples the input asks for so far, and those given.
    owed: f64,
    given: u64,
    /// The audio that may wait for the bank, samples.
    lag: f64,
}

impl AudioOut {
    fn new() -> Self {
        AudioOut {
            source: Source::Nothing,
            fade: 0.0,
            handover: 0,
            base: Vec::with_capacity(256),
            rs: None,
            rs_rate: 0.0,
            up: Vec::with_capacity(2048),
            agc: Agc::new(AUDIO_RATE, AGC_TARGET, AGC_ATTACK_S, AGC_DECAY_S, 1e6),
            nco: Nco::new(AUDIO_RATE, 700.0),
            ramp: 0.0,
            owed: 0.0,
            given: 0,
            lag: 0.0,
        }
    }

    /// Starts again for a wide stream at `rate` whose bank gives a block of channel samples every `hop` input samples.
    fn reset(&mut self, rate: f64, hop: usize) {
        self.source = Source::Nothing;
        self.base.clear();
        self.rs = None;
        self.rs_rate = 0.0;
        self.ramp = 0.0;
        self.owed = 0.0;
        self.given = 0;
        self.lag = (hop as f64 / rate + 0.1) * AUDIO_RATE;
    }

    /// Plays `source` from now: the gain control forgets the level and follows the new one with a fast attack for
    /// [`HANDOVER_S`], meanwhile no peak passes the level of the gain control (a key down after noise would), and the audio
    /// fades in from silence. So no step louder than the signal is heard.
    fn hand_over(&mut self, source: Source) {
        self.source = source;
        self.fade = 0.0;
        self.agc.reset();
        self.agc.set_times(HANDOVER_ATTACK_S, AGC_DECAY_S);
        self.handover = (HANDOVER_S * AUDIO_RATE) as u32;
    }

    /// One baseband sample of the rebuilt tone: the level ramps in 4 ms toward the key.
    fn key(&mut self, on: bool, env_rate: f64) {
        let step = (1.0 / (0.004 * env_rate)) as f32;
        let target = if on { 1.0 } else { 0.0 };
        self.ramp += (target - self.ramp).clamp(-step, step);
        self.base.push(Complex32::new(self.ramp, 0.0));
    }

    /// Writes the audio of the baseband taken since the last call (at `env_rate`) to `out`, then silence up to the pace of
    /// `inputs` more input samples at `rate`, less the delay of the bank.
    fn render(
        &mut self,
        listen: Listen,
        pitch: f64,
        env_rate: f64,
        inputs: usize,
        rate: f64,
        out: &mut Vec<f32>,
    ) {
        self.owed += inputs as f64 * AUDIO_RATE / rate;
        if !self.base.is_empty() && listen != Listen::Off {
            if self.rs.is_none() || self.rs_rate != env_rate {
                self.rs = Resampler::with_quality(env_rate, AUDIO_RATE, 0.4, 60.0).ok();
                self.rs_rate = env_rate;
            }
            if let Some(rs) = &mut self.rs {
                self.up.clear();
                rs.process(&self.base, &mut self.up);
                if listen == Listen::Signal {
                    self.agc.process(&mut self.up);
                } else {
                    for v in &mut self.up {
                        *v *= AGC_TARGET;
                    }
                }
                if self.handover > 0 {
                    let step = (1.0 / (FADE_S * AUDIO_RATE)) as f32;
                    for v in &mut self.up {
                        self.fade = (self.fade + step).min(1.0);
                        // The baseband is complex: holding its magnitude changes the level, not the tone.
                        let m = v.norm();
                        let limit = if m > AGC_TARGET { AGC_TARGET / m } else { 1.0 };
                        *v *= self.fade * limit;
                    }
                    self.handover = self.handover.saturating_sub(self.up.len() as u32);
                    if self.handover == 0 {
                        self.agc.set_times(AGC_ATTACK_S, AGC_DECAY_S);
                    }
                }
                if self.nco.freq() != pitch {
                    self.nco.set_freq(pitch);
                }
                self.nco.mix_in_place(&mut self.up);
                out.extend(self.up.iter().map(|v| v.re));
                self.given += self.up.len() as u64;
            }
        }
        self.base.clear();
        let due = (self.owed - self.lag).floor();
        while (self.given as f64) < due {
            out.push(0.0);
            self.given += 1;
        }
    }
}

/// The CW skimmer.
pub struct CwDecoder {
    threshold: f64,
    adaptive: bool,
    wpm: f64,
    width: FilterWidth,
    listen: Listen,
    pitch: f64,
    /// The focus asked for (0 for none), and the focus inside the span.
    focus_asked: f64,
    focus_hz: Option<f64>,
    forget_s: f64,
    dial: f64,
    /// The rate of the wide stream, and half the span watched, Hz.
    rate: f64,
    half: f64,
    nfft: usize,
    spec: WindowedFft,
    buf: Vec<Complex32>,
    held: Vec<f32>,
    scratch: Vec<f32>,
    scratch2: Vec<f32>,
    peaks: Vec<(f64, f32)>,
    kept: Vec<(f64, f32)>,
    /// Input samples so far, and those before the bank was made (the bank counts its input from 0).
    t: u64,
    bank_t0: u64,
    bank: Option<OverlapSaveBank>,
    /// The new output of the bank: the dial, then one per signal in the order of `chans`.
    chan_out: Vec<Vec<Complex32>>,
    replayed: Vec<Complex32>,
    chans: Vec<CwChannel>,
    gone: Vec<CwChannel>,
    cands: Vec<(f64, u32, u32)>,
    next_id: u32,
    floor: f32,
    chars: u64,
    heard: Heard,
    status: Status,
    key: String,
    text: String,
    stale: Vec<String>,
    audio: AudioOut,
    n_peaks: usize,
    /// Input samples in a tick of the key history.
    tick_len: u64,
    /// Where a signal was dropped as part of a stronger one, with the id of that one: no signal starts there while it lives.
    shadows: Vec<(f64, u32)>,
    /// Bins dropped for steadiness or as unproven, with their averaged level (dB): no signal starts there while the level
    /// stays within [`STEADY_DB`] of it.
    masks: Vec<(f64, f64)>,
    /// The averaged power spectrum of the search, and the transforms in it.
    avg: Vec<f32>,
    ffts: u64,
    /// The signal listened to, its key changes (signal time, key down) and its last key state.
    listened: Option<u32>,
    /// The channel filter of the dial, and the radio frequency that the audio plays, Hz.
    dial_rx: Option<ChannelFilter>,
    played_hz: Option<f64>,
    key_edges: std::collections::VecDeque<(f64, bool)>,
    key_on: bool,
    /// The series ids: marks, gaps, spectrum, key.
    series: Option<[SeriesId; 4]>,
}

impl Default for CwDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The transform of the carrier search for a wide stream at `rate`: a multiple of 512 points with bins of about 12 Hz.
fn search_size(rate: f64) -> usize {
    ((rate / 12.0 / 512.0).round() as usize).max(1) * 512
}

impl CwDecoder {
    /// A skimmer at the default settings.
    pub fn new() -> Self {
        let rate = Span::Khz14.wide_rate();
        let nfft = search_size(rate);
        CwDecoder {
            threshold: 10.0,
            adaptive: true,
            wpm: 0.0,
            width: FilterWidth::Hz100,
            listen: Listen::Off,
            pitch: 700.0,
            focus_asked: 0.0,
            focus_hz: None,
            forget_s: 600.0,
            dial: 0.0,
            rate,
            half: 0.5 * Span::Khz14.width_hz(),
            nfft,
            spec: WindowedFft::new(nfft, nfft, Window::Hann)
                .expect("a window that fits the transform"),
            buf: Vec::with_capacity(nfft),
            held: vec![0.0; nfft],
            scratch: Vec::with_capacity(nfft),
            scratch2: Vec::with_capacity(nfft),
            peaks: Vec::new(),
            kept: Vec::new(),
            t: 0,
            bank_t0: 0,
            bank: None,
            chan_out: Vec::new(),
            replayed: Vec::new(),
            chans: Vec::new(),
            gone: Vec::new(),
            cands: Vec::new(),
            next_id: 1,
            floor: 0.0,
            chars: 0,
            heard: Heard::new("CW"),
            status: Status::default(),
            key: String::new(),
            text: String::new(),
            stale: Vec::new(),
            audio: AudioOut::new(),
            n_peaks: 0,
            tick_len: (rate * TICK_S).round() as u64,
            shadows: Vec::new(),
            masks: Vec::new(),
            avg: vec![0.0; nfft],
            ffts: 0,
            listened: None,
            dial_rx: None,
            played_hz: None,
            key_edges: std::collections::VecDeque::with_capacity(KEY_EDGES),
            key_on: false,
            series: None,
        }
    }

    /// The signals being followed.
    pub fn channels(&self) -> &[CwChannel] {
        &self.chans
    }

    /// The bank for the wide stream at `self.rate` with the channel of the dial only, keeping the last second for new
    /// channels.
    fn new_bank(&mut self) {
        let size = OverlapSaveBank::size_for(self.rate, CHANNEL_STOP - CHANNEL_PASS, CHANNEL_ATTEN);
        self.bank = OverlapSaveBank::new(self.rate, size)
            .ok()
            .and_then(|mut b| {
                let blocks = (self.rate / b.hop() as f64).ceil() as usize;
                b.keep_history(blocks);
                let dial = b
                    .add_channel(0.0, CHANNEL_PASS, CHANNEL_STOP, CHANNEL_ATTEN)
                    .ok()?;
                debug_assert_eq!(dial, DIAL);
                Some(b)
            });
        self.dial_rx = self
            .bank
            .as_ref()
            .and_then(|b| ChannelFilter::new(b.output_rate(DIAL), 0.0, self.width).ok());
        self.bank_t0 = self.t;
        self.chan_out.clear();
        self.chan_out.push(Vec::with_capacity(256));
        let hop = self.bank.as_ref().map_or(1, OverlapSaveBank::hop);
        self.audio.reset(self.rate, hop);
    }

    /// Retires every signal and starts the search again from nothing: no candidate, mask, shadow, spectrum or noise floor
    /// of before is kept.
    fn retire_all(&mut self) {
        for c in std::mem::take(&mut self.chans) {
            self.retire(c);
        }
        self.held.fill(0.0);
        self.avg.fill(0.0);
        self.ffts = 0;
        self.floor = 0.0;
        self.n_peaks = 0;
        self.cands.clear();
        self.masks.clear();
        self.shadows.clear();
        self.buf.clear();
        self.listened = None;
        self.key_edges.clear();
        self.key_on = false;
        self.new_bank();
    }

    /// Takes signal `i` out of the bank and the list.
    fn remove_at(&mut self, i: usize) -> CwChannel {
        if let Some(b) = &mut self.bank {
            // The bank holds the dial, then a channel for every signal, in the same order.
            let _ = b.remove_channel(i + 1);
        }
        self.chan_out.remove(i + 1);
        self.chans.remove(i)
    }

    fn retire(&mut self, c: CwChannel) {
        if c.valid {
            self.gone.push(c);
            if self.gone.len() > GONE_CAP {
                self.gone.remove(0);
            }
        }
    }

    /// Starts a signal at offset `f` at input time `t`: a channel of the bank, its last second, and a receiver.
    fn start(&mut self, f: f64, manual: bool, t: u64) {
        let Some(bank) = &mut self.bank else {
            return;
        };
        let Ok(ch) = bank.add_channel(f, CHANNEL_PASS, CHANNEL_STOP, CHANNEL_ATTEN) else {
            return;
        };
        let ch_rate = bank.output_rate(ch);
        let Ok(mut rx) =
            MorseReceiver::new(ch_rate, 0.0, self.adaptive, self.wpm as f32, self.width)
        else {
            let _ = bank.remove_channel(ch);
            return;
        };
        // Start it on the last second, so the first elements are not lost.
        self.replayed.clear();
        let _ = bank.replay(ch, &mut self.replayed);
        rx.start(&self.replayed);
        let time = t as f64 / self.rate;
        let on = rx.key.is_on();
        let mut c = CwChannel {
            id: self.next_id,
            freq: f,
            rx,
            store: decoder::store(SIGNAL_CHARS, 1),
            confidence: String::with_capacity(64),
            alts: String::with_capacity(128),
            manual,
            past: false,
            first: time,
            last: time,
            clock: bank.next_output_index(),
            step: bank.decimation(ch) as u64,
            valid: false,
            good: 0,
            mixed: 0,
            keys: 0,
            ticks: 0,
            next_tick: bank.next_output_index().div_ceil(self.tick_len) * self.tick_len,
            power: 0.0,
            on,
            edges: [0.0; EDGES],
            n_edges: 0,
            compared: 0,
            coinciding: 0,
            chance: 0.0,
            chance_var: 0.0,
        };
        self.next_id += 1;
        if let Some(i) = self
            .gone
            .iter()
            .position(|g| !g.past && (g.freq_hz() - f).abs() < 30.0)
        {
            let g = self.gone.remove(i);
            c.store = g.store;
            c.id = g.id;
            c.first = g.first;
            c.valid = g.valid;
        }
        self.chans.push(c);
        self.chan_out.push(Vec::with_capacity(256));
    }

    /// The averaged power of the search at offset `f` (the greatest of the three nearest bins), dB.
    fn level_db(&self, f: f64) -> f64 {
        let n = self.nfft as i64;
        let i = (f * self.nfft as f64 / self.rate).round() as i64;
        let p = (i - 1..=i + 1)
            .map(|j| self.avg[j.rem_euclid(n) as usize])
            .fold(0.0f32, f32::max);
        db(f64::from(p))
    }

    /// One transform of the carrier search over the newest `nfft` samples, ending at input time `t`.
    fn find(&mut self, t: u64) {
        let p = self.spec.run_power(&self.buf);
        for ((h, a), &v) in self.held.iter_mut().zip(&mut self.avg).zip(p) {
            *h = if v > *h { v } else { *h + 0.05 * (v - *h) };
            *a += 0.3 * (v - *a);
        }
        self.ffts += 1;
        let bin = self.rate / self.nfft as f64;
        let n = self.nfft as i64;
        let k = (self.half / bin) as i64;
        let at = |i: i64, h: &[f32]| h[i.rem_euclid(n) as usize];
        self.scratch.clear();
        for i in -k..=k {
            self.scratch.push(at(i, &self.held));
        }
        self.floor = median(&self.scratch, &mut self.scratch2).max(1e-20);
        let thr = self.floor * 10f32.powf(self.threshold as f32 / 10.0);
        let top = self.scratch.iter().copied().fold(0.0f32, f32::max);
        self.peaks.clear();
        for i in -k + 1..k {
            let v = at(i, &self.held);
            if v > thr && v > top * 1e-5 && v >= at(i - 1, &self.held) && v > at(i + 1, &self.held)
            {
                let (a, b, c) = (
                    at(i - 1, &self.held) as f64,
                    v as f64,
                    at(i + 1, &self.held) as f64,
                );
                let den = a - 2.0 * b + c;
                let d = if den.abs() > 1e-30 {
                    (0.5 * (a - c) / den).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                self.peaks.push(((i as f64 + d) * bin, v));
            }
        }
        self.peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.kept.clear();
        for &(f, v) in &self.peaks {
            let owned = self.kept.iter().any(|&(g, w)| {
                let d = (f - g).abs();
                d < SAME_SIGNAL_HZ || SKIRT.iter().any(|&(hz, p)| d < hz && v < p * w)
            });
            if !owned {
                self.kept.push((f, v));
            }
        }
        self.n_peaks = self.kept.len();
        let mut cands = std::mem::take(&mut self.cands);
        let kept = &self.kept;
        cands.retain_mut(|(cf, n, miss)| {
            if let Some(&(f, _)) = kept.iter().find(|p| (p.0 - *cf).abs() < 25.0) {
                *cf = (*cf * *n as f64 + f) / (*n + 1) as f64;
                *n += 1;
                *miss = 0;
            } else {
                *miss += 1;
            }
            *miss <= 20
        });
        for &(f, _) in kept {
            if !cands.iter().any(|c| (c.0 - f).abs() < 25.0) {
                cands.push((f, 1, 0));
            }
        }
        let mut masks = std::mem::take(&mut self.masks);
        masks.retain(|m| (self.level_db(m.0) - m.1).abs() <= STEADY_DB);
        self.masks = masks;
        for c in &cands {
            let near = self
                .chans
                .iter()
                .any(|ch| (ch.freq_hz() - c.0).abs() < SAME_SIGNAL_HZ)
                || self.shadows.iter().any(|s| (s.0 - c.0).abs() < 30.0)
                || self.masks.iter().any(|m| (m.0 - c.0).abs() < 30.0);
            if c.1 >= 2 && !near && self.chans.len() < MAX_SIGNALS {
                self.start(c.0, false, t);
            }
        }
        cands.truncate(2 * MAX_SIGNALS);
        self.cands = cands;
    }

    /// Takes the focus asked for inside the span. Only the signal in focus is picked: one picked before leaves like the others
    /// once it is quiet. A focus where no signal is starts one there.
    fn set_focus(&mut self) {
        let f = self.focus_asked;
        self.focus_hz = (f != 0.0 && f.abs() <= self.half).then_some(f);
        for c in &mut self.chans {
            c.manual = false;
        }
        let Some(f) = self.focus_hz else {
            return;
        };
        match self
            .chans
            .iter_mut()
            .find(|c| (c.freq - f).abs() < SAME_SIGNAL_HZ)
        {
            Some(c) => c.manual = true,
            None => {
                if self.chans.len() >= MAX_SIGNALS {
                    let c = self.remove_at(0);
                    self.retire(c);
                }
                self.start(f, true, self.t);
            }
        }
    }

    /// The signal listened to: the focus, else the strongest signal in the table. Without a focus it stays on one signal
    /// until another is 3 dB stronger.
    fn listened_id(&self) -> Option<u32> {
        self.focus_id().or_else(|| {
            let top = self
                .chans
                .iter()
                .filter(|c| c.valid)
                .max_by(|a, b| a.power.total_cmp(&b.power))?;
            let now = self
                .listened
                .and_then(|id| self.chans.iter().find(|c| c.id == id && c.valid));
            match now {
                Some(c) if top.power < 2.0 * c.power => Some(c.id),
                _ => Some(top.id),
            }
        })
    }

    /// Whether signal `c` shows in the table: it proved itself, or it was picked or is the focus.
    fn shown(c: &CwChannel, focus: Option<u32>) -> bool {
        c.valid || c.manual || Some(c.id) == focus
    }

    /// Drops the signals whose key follows a stronger one within 250 Hz or the width of the filter (its keying sidebands, or
    /// the carrier itself through the edge of the filter), the mixing products of stronger signals ([`PRODUCT_EDGES`]), the
    /// signals not shown whose key has not changed since they started ([`STEADY_S`]), and those that did not prove themselves in
    /// [`UNPROVEN_S`]. None leaves a row. The bins of the last three are masked while their level stays steady.
    fn drop_followers(&mut self, now: f64, focus: Option<u32>) {
        self.count_products(now);
        // A carrier this near passes the channel filter of a signal.
        let follow_hz = SKIRT[1].0.max(self.width.width_hz());
        let mut i = 0;
        while i < self.chans.len() {
            let c = &self.chans[i];
            let kept = c.manual || Some(c.id) == focus;
            let owner = (c.ticks >= 128 && !kept)
                .then(|| {
                    self.chans.iter().find(|o| {
                        o.id != c.id
                            && o.ticks >= 128
                            && (o.freq - c.freq).abs() < follow_hz
                            && o.power > c.power
                            && o.keys.count_ones() >= FOLLOW_TICKS
                            && (o.keys ^ c.keys).count_ones() <= FOLLOW_TICKS
                    })
                })
                .flatten()
                .map(|o| o.id);
            let product = !kept && c.compared >= PRODUCT_EDGES && Self::like_product(c, 0.8, 4.0);
            let steady = !kept && !c.valid && c.n_edges == 0 && now - c.first >= STEADY_S;
            let unproven = !kept && !c.valid && now - c.first > UNPROVEN_S;
            if let Some(o) = owner {
                self.shadows.push((c.freq, o));
                self.remove_at(i);
            } else if product || steady || unproven {
                let f = c.freq;
                self.masks.push((f, self.level_db(f)));
                self.remove_at(i);
            } else {
                i += 1;
            }
        }
        let chans = &self.chans;
        self.shadows.retain(|s| chans.iter().any(|c| c.id == s.1));
    }

    /// Compares the new key changes of each signal with the key changes of the signals [`PRODUCT_POWER`] stronger at signal
    /// time `now`, and counts those that coincide with one of them, and the count that chance gives.
    fn count_products(&mut self, now: f64) {
        for i in 0..self.chans.len() {
            let c = &self.chans[i];
            let first = c.compared.max(c.n_edges.saturating_sub(EDGES as u64));
            if first == c.n_edges {
                continue;
            }
            let stronger = |o: &&CwChannel| o.power > PRODUCT_POWER * c.power;
            let rate: f64 = self
                .chans
                .iter()
                .filter(stronger)
                .map(|o| o.n_edges as f64 / (now - o.first).max(1.0))
                .sum();
            let p = 1.0 - (-2.0 * EDGE_S * rate).exp();
            let mut hit = 0;
            for k in first..c.n_edges {
                let t = c.edges[(k % EDGES as u64) as usize];
                hit += u64::from(self.chans.iter().filter(stronger).any(|o| {
                    o.edges[..o.n_edges.min(EDGES as u64) as usize]
                        .iter()
                        .any(|&e| (e - t).abs() <= EDGE_S)
                }));
            }
            let n = (c.n_edges - first) as f64;
            let c = &mut self.chans[i];
            c.coinciding += hit;
            c.chance += n * p;
            c.chance_var += n * p * (1.0 - p);
            c.compared = c.n_edges;
        }
    }

    /// Whether at least the share `share` of the key changes of signal `c` coincide with those of stronger signals, and their
    /// count is `sigmas` standard deviations over chance (see [`PRODUCT_EDGES`]).
    fn like_product(c: &CwChannel, share: f64, sigmas: f64) -> bool {
        let k = c.coinciding as f64;
        c.compared >= 8
            && k >= share * c.compared as f64
            && k >= c.chance + sigmas * c.chance_var.sqrt()
    }

    fn focus_id(&self) -> Option<u32> {
        let f = self.focus_hz?;
        self.chans
            .iter()
            .min_by(|a, b| (a.freq_hz() - f).abs().total_cmp(&(b.freq_hz() - f).abs()))
            .map(|c| c.id)
    }

    /// Writes the row of signal `i` (of the gone ones when `gone`) into the signals table.
    fn signal_row(&mut self, i: usize, gone: bool, p: &mut Published) {
        let c = if gone { &self.gone[i] } else { &self.chans[i] };
        let t = &c.rx.timing;
        put_text(&mut self.key, format_args!("{}", c.id));
        text_into(&c.store, &mut self.text);
        CW_SIGNALS.put(
            p,
            &self.key,
            &CwSignalRow {
                id: i64::from(c.id),
                offset_hz: Hertz(c.freq_hz()),
                wpm: c.rx.wpm(),
                snr_db: f64::from(c.rx.snr_db()),
                key_down: !gone && c.rx.key.is_on(),
                locked: t.is_locked(),
                farnsworth: t.farnsworth(),
                manual: c.manual,
                first_s: c.first,
                last_s: c.last,
                characters: c.store.total as i64,
                text: &self.text,
                confidence: &c.confidence,
                alternatives: &c.alts,
                gone,
                errors: t.unknown() as i64,
                freq_hz: Hertz(self.dial + c.freq_hz()),
                level_dbfs: c.level_dbfs(),
            },
        );
    }

    fn reset_timing(&mut self) {
        for c in &mut self.chans {
            c.rx.set_timing(self.adaptive, self.wpm as f32);
        }
    }

    /// Writes the averaged spectrum of the span into series `id`: at most [`SPECTRUM_POINTS`] points of offset (Hz) and level
    /// (dB relative to a full-scale carrier), each the greatest of the bins it covers, so that a narrow carrier shows.
    fn spectrum_series(&self, id: SeriesId, res: &mut Published) {
        if self.ffts == 0 {
            return;
        }
        let bin = self.rate / self.nfft as f64;
        let n = self.nfft as i64;
        let k = (self.half / bin) as i64;
        let bins = (2 * k + 1) as usize;
        let per = bins.div_ceil(SPECTRUM_POINTS).max(1);
        // A full-scale carrier through the Hann window has power (nfft / 2)^2.
        let full = (0.5 * self.nfft as f64).powi(2);
        let avg = &self.avg;
        res.results_mut().update_series(id, |sr| {
            sr.points.clear();
            let mut i = -k;
            while i <= k {
                let end = (i + per as i64).min(k + 1);
                let top = (i..end)
                    .map(|j| avg[j.rem_euclid(n) as usize])
                    .fold(0.0f32, f32::max);
                let x = 0.5 * (i + end - 1) as f64 * bin;
                sr.points.push([x as f32, db(f64::from(top) / full) as f32]);
                i = end;
            }
            true
        });
    }

    /// Writes the key of the signal listened to into series `id`: points of signal time (s) and key down (1) or up (0), the
    /// first for the state 2 s ago, then the changes since, then the state now.
    fn key_series(&self, id: SeriesId, now: f64, res: &mut Published) {
        let from = now - KEY_SPAN_S;
        let edges = &self.key_edges;
        let on = self.key_on;
        let any = self.listened.is_some();
        // The changes alternate, so the state before the first change in the window is the opposite of it.
        let before = edges.iter().find(|e| e.0 > from).map_or(on, |e| !e.1);
        res.results_mut().update_series(id, |sr| {
            sr.points.clear();
            if any {
                sr.points.push([from as f32, f32::from(u8::from(before))]);
                for &(t, k) in edges.iter().filter(|e| e.0 > from) {
                    sr.points.push([t as f32, f32::from(u8::from(k))]);
                }
                sr.points.push([now as f32, f32::from(u8::from(on))]);
            }
            true
        });
    }

    /// The envelope rate of every channel, Hz (1 before the bank is made).
    fn env_rate(&self) -> f64 {
        self.dial_rx
            .as_ref()
            .map_or(1.0, ChannelFilter::envelope_rate)
    }

    /// Runs the new channel samples of the dial and of every signal through their filters and receivers. `listened` is the
    /// signal whose audio and key changes are taken; without one, the real signal plays the dial.
    fn decode(&mut self, out: &mut Outbox, listened: Option<u32>) {
        let env_rate = self.env_rate();
        let source = match (self.listen, listened) {
            (Listen::Off, _) | (Listen::Tone, None) => Source::Nothing,
            (_, Some(id)) => Source::Signal(id),
            (Listen::Signal, None) => Source::Dial,
        };
        if source != self.audio.source {
            self.audio.hand_over(source);
        }
        self.played_hz = match source {
            Source::Nothing => None,
            Source::Dial => Some(self.dial),
            Source::Signal(id) => self
                .chans
                .iter()
                .find(|c| c.id == id)
                .map(|c| self.dial + c.freq_hz()),
        };
        if let (Some(rx), Some(o)) = (&mut self.dial_rx, self.chan_out.get_mut(DIAL)) {
            for &x in o.iter() {
                if rx.push(x) && source == Source::Dial {
                    self.audio.base.push(rx.filtered());
                }
            }
            o.clear();
        }
        if listened != self.listened {
            self.listened = listened;
            self.key_edges.clear();
            self.key_on = self
                .chans
                .iter()
                .find(|c| Some(c.id) == listened)
                .is_some_and(|c| c.rx.key.is_on());
        }
        for (c, o) in self.chans.iter_mut().zip(self.chan_out.iter_mut().skip(1)) {
            let heard = Some(c.id) == listened;
            for &x in o.iter() {
                c.clock += c.step;
                let made = c.rx.push(x);
                let on = c.rx.key.is_on();
                while c.clock >= c.next_tick {
                    c.keys = (c.keys << 1) | u128::from(on);
                    c.ticks = c.ticks.saturating_add(1);
                    c.next_tick += self.tick_len;
                }
                if !made {
                    continue;
                }
                let time = (self.bank_t0 + c.clock) as f64 / self.rate;
                if on != c.on {
                    c.on = on;
                    // The detector decides a change after a hold; the change began where its state began.
                    c.edges[(c.n_edges % EDGES as u64) as usize] =
                        time - c.rx.key.seconds_in_state();
                    c.n_edges += 1;
                }
                if on {
                    c.last = time;
                    let p = c.rx.filtered().norm_sqr();
                    c.power += if c.power == 0.0 {
                        p
                    } else {
                        0.02 * (p - c.power)
                    };
                }
                if heard {
                    match self.listen {
                        Listen::Signal => self.audio.base.push(c.rx.filtered()),
                        Listen::Tone => self.audio.key(on, env_rate),
                        Listen::Off => {}
                    }
                    if on != self.key_on {
                        self.key_on = on;
                        if self.key_edges.len() == KEY_EDGES {
                            self.key_edges.pop_front();
                        }
                        self.key_edges.push_back((time, on));
                    }
                }
                if c.rx.has_symbols() {
                    let freq = self.dial + c.freq_hz();
                    let snr = c.rx.snr_db();
                    for sym in c.rx.take_symbols() {
                        if let Event::Char(k) = &sym {
                            let clear = !matches!(k.text_or("*"), "E" | "T" | "I" | "*");
                            let sure = k.confidence >= 5;
                            c.good += u32::from(clear && sure);
                            c.mixed += u32::from(mixed(k) && sure);
                        }
                        self.chars += emit_symbol(
                            &sym,
                            &mut c.store,
                            out,
                            &mut self.heard,
                            time,
                            c.id,
                            freq,
                            snr,
                        );
                        remember(&mut c.confidence, &mut c.alts, &sym);
                    }
                    let wpm = c.rx.wpm();
                    if c.good >= GOOD_CHARS
                        && c.mixed >= MIXED_CHARS
                        && !Self::like_product(c, 0.6, 2.0)
                        && c.rx.timing.is_locked()
                        && (5.0..=SPEED_MARGIN * self.width.max_wpm()).contains(&wpm)
                    {
                        c.valid = true;
                    }
                }
            }
            o.clear();
        }
    }
}

impl sdrkit_graph::Decoder for CwDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.cw"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        CwSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match CwSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CwSetting::Threshold => self.threshold = THRESHOLD.read(id, value)?,
            CwSetting::Decoder => {
                self.adaptive = DECODER.read(id, value)? == TimingDecoder::Adaptive;
                self.reset_timing();
            }
            CwSetting::Wpm => {
                self.wpm = WPM_SETTING.read(id, value)?;
                self.reset_timing();
            }
            CwSetting::Filter => {
                self.width = FILTER.read(id, value)?;
                // The bank's channels are at 2 kHz or more, above every envelope rate.
                for c in &mut self.chans {
                    let _ = c.rx.set_width(self.width);
                }
                if let Some(rx) = &mut self.dial_rx {
                    let _ = rx.set_width(self.width);
                }
            }
            CwSetting::Listen => self.listen = LISTEN.read(id, value)?,
            CwSetting::Pitch => self.pitch = PITCH.read(id, value)?,
            CwSetting::FocusHz => {
                self.focus_asked = FOCUS_HZ.read(id, value)?;
                self.set_focus();
            }
            CwSetting::ForgetS => self.forget_s = FORGET_S.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (CwSetting::Threshold, Value::F64(self.threshold)),
            (
                CwSetting::Decoder,
                Value::Choice(timing(self.adaptive).index()),
            ),
            (CwSetting::Wpm, Value::F64(self.wpm)),
            (CwSetting::Filter, Value::Choice(self.width.index())),
            (CwSetting::Listen, Value::Choice(self.listen.index())),
            (CwSetting::Pitch, Value::Hz(self.pitch)),
            (CwSetting::FocusHz, Value::Hz(self.focus_asked)),
            (CwSetting::ForgetS, Value::F64(self.forget_s)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("cw")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        // Each carrier's channel needs 2 kHz, and the bank gives it at a quarter of the rate or less.
        const LEAST: f64 = 4.0 * CHANNEL_STOP;
        if input.rate < LEAST {
            return Err(ConfigError::RateTooLow {
                need: LEAST,
                got: input.rate,
            });
        }
        self.dial = input.tuned_hz;
        self.rate = input.rate;
        self.half = (0.5 * input.bandwidth_hz).min(0.5 * input.rate - CHANNEL_STOP);
        self.nfft = search_size(self.rate);
        self.spec = WindowedFft::new(self.nfft, self.nfft, Window::Hann)
            .expect("a window that fits the transform");
        self.buf = Vec::with_capacity(self.nfft);
        self.held = vec![0.0; self.nfft];
        self.avg = vec![0.0; self.nfft];
        self.ffts = 0;
        self.tick_len = (self.rate * TICK_S).round().max(1.0) as u64;
        self.scratch = Vec::with_capacity(self.nfft);
        self.scratch2 = Vec::with_capacity(self.nfft);
        self.t = 0;
        self.retire_all();
        self.set_focus();
        Ok(decoder::formats())
    }
    /// The signals are of the frequency before: their rows stay as gone, and no new signal takes one over.
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.retire_all();
        self.gone.iter_mut().for_each(|g| g.past = true);
        self.set_focus();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        // The carrier search; a carrier found starts with the last second that the bank kept.
        let mut rest = x;
        let mut t = self.t;
        while !rest.is_empty() {
            let take = (self.nfft - self.buf.len()).min(rest.len());
            self.buf.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            t += take as u64;
            if self.buf.len() == self.nfft {
                self.find(t);
                self.buf.drain(..self.nfft / 2);
            }
        }
        if let Some(bank) = &mut self.bank {
            // One output vector per channel: the two lists change together.
            let _ = bank.process(x, &mut self.chan_out);
        }
        self.t = t;
        let focus = self.focus_id();
        let listened = self.listened_id();
        self.decode(out, listened);
        let env_rate = self.env_rate();
        self.audio.render(
            self.listen,
            self.pitch,
            env_rate,
            x.len(),
            self.rate,
            out.sound(),
        );
        let now = self.t as f64 / self.rate;
        let mut i = 0;
        while i < self.chans.len() {
            let c = &self.chans[i];
            if now - c.last.max(c.first) > QUIET_S && Some(c.id) != focus && !c.manual {
                let c = self.remove_at(i);
                self.retire(c);
            } else {
                i += 1;
            }
        }
        self.drop_followers(now, focus);
        let forget = self.forget_s;
        self.gone.retain(|g| now - g.last.max(g.first) <= forget);
    }
    fn report(&mut self, res: &mut Published) {
        let focus = self.focus_id();
        let now = self.t as f64 / self.rate;
        let decoding = self
            .chans
            .iter()
            .filter(|c| c.valid && now - c.last < 5.0)
            .count();
        let shown = self.chans.iter().filter(|c| Self::shown(c, focus)).count();
        // A signal picked or in focus shows before it proves itself; the count of the heard signals takes only those proven.
        let proven = self.chans.iter().filter(|c| c.valid).count();
        let lock_state = if decoding > 0 {
            LockState::Locked
        } else if !self.chans.is_empty() || self.n_peaks > 0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let fc = focus.and_then(|id| self.chans.iter().find(|c| c.id == id));
        let st = &mut self.status;
        st.lock_state = lock_state;
        // The strongest proven carrier, from the dial.
        st.offset_hz = self
            .chans
            .iter()
            .filter(|c| c.valid)
            .max_by(|a, b| a.power.total_cmp(&b.power))
            .map(CwChannel::freq_hz);
        st.snr_db = fc
            .or_else(|| {
                self.chans
                    .iter()
                    .max_by(|a, b| a.rx.snr_db().total_cmp(&b.rx.snr_db()))
            })
            .map(|c| c.rx.snr_db());
        match (self.chans.is_empty(), fc) {
            (true, _) => put_text(&mut st.summary, format_args!("searching for CW")),
            (false, None) => put_text(
                &mut st.summary,
                format_args!("{shown} signals, {decoding} decoding"),
            ),
            (false, Some(c)) => put_text(
                &mut st.summary,
                format_args!(
                    "{shown} signals, {decoding} decoding; focus {:+.0} Hz, {:.0} WPM",
                    c.freq_hz(),
                    c.rx.wpm()
                ),
            ),
        }
        res.results_mut().set_status(st);
        let unknown: u64 = self.chans.iter().map(|c| c.rx.timing.unknown()).sum();
        for i in 0..self.gone.len() {
            self.signal_row(i, true, res);
        }
        for i in 0..self.chans.len() {
            if Self::shown(&self.chans[i], focus) {
                self.signal_row(i, false, res);
            }
        }
        let (chans, gone) = (&self.chans, &self.gone);
        retain_rows(res, &CW_SIGNALS, &mut self.stale, |k| {
            chans
                .iter()
                .filter(|c| Self::shown(c, focus))
                .chain(gone)
                .any(|c| k.parse() == Ok(c.id))
        });
        if self.series.is_none() {
            let d = res.results().decl();
            self.series = match (
                d.series_id(CW_MARKS.key),
                d.series_id(CW_GAPS.key),
                d.series_id(CW_SPECTRUM.key),
                d.series_id(CW_KEY.key),
            ) {
                (Some(m), Some(g), Some(s), Some(k)) => Some([m, g, s, k]),
                _ => None,
            };
        }
        // The learned sender of the focused signal.
        let fc = focus.and_then(|id| self.chans.iter().find(|c| c.id == id));
        let t = fc.map(|c| &c.rx.timing);
        res.put(&CwFields {
            span_hz: Hertz(2.0 * self.half),
            floor_db: db(self.floor as f64),
            focus: focus.map(i64::from),
            characters: self.chars as i64,
            errors: unknown as i64,
            dot_ms: t.map(|t| t.dot() * 1000.0),
            dash_ratio: t.map(|t| t.dash_ratio()),
            char_gap: t.map(|t| t.char_gap()),
            word_gap: t.map(|t| t.word_gap()),
            steadiness_pct: t.map(|t| (t.steadiness() * 100.0).round()),
            wpm: fc.map(|c| c.rx.wpm()),
            signals: proven as i64,
            now_s: now,
            listened: self.listened.map(i64::from),
            playing: match self.audio.source {
                Source::Nothing => Playing::Nothing,
                Source::Dial => Playing::Dial,
                Source::Signal(_) => Playing::Signal,
            },
            listened_hz: self.played_hz.map(Hertz),
        });
        if let Some([.., spectrum, key]) = self.series {
            self.spectrum_series(spectrum, res);
            self.key_series(key, now, res);
        }
        if let (Some(t), Some([marks, gaps, ..])) = (t, self.series) {
            for (id, h) in [marks, gaps]
                .into_iter()
                .zip([t.mark_histogram(), t.gap_histogram()])
            {
                res.results_mut().update_series(id, |sr| {
                    sr.points.clear();
                    for (i, &n) in h.iter().enumerate() {
                        let x = (HIST_MIN + (i as f64 + 0.5) * HIST_SPAN / HIST_BINS as f64) as f32;
                        sr.points.push([x, n as f32]);
                    }
                    true
                });
            }
        }
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        for c in &mut self.chans {
            c.store.clear();
            c.confidence.clear();
            c.alts.clear();
        }
        self.gone.clear();
        self.heard.clear();
    }
    /// The signals are of the frequency before. Their texts stay as rows of gone signals if the stack keeps the
    /// table.
    fn forget(&mut self, kept: &Kept) {
        self.retire_all();
        if kept.table(CW_SIGNALS.key) {
            self.gone.iter_mut().for_each(|g| g.past = true);
        } else {
            self.gone.clear();
        }
        self.heard.forget(kept);
    }
}

/// The `offset_hz` setting of [`MorseDecoder`].
pub const OFFSET_HZ: Setting<field::Hz> = Setting::new("offset_hz", "Carrier", "Hz", 0.0)
    .range(-12000.0, 12000.0, 0.0)
    .help("Where the carrier is in the input.");

sdrkit_core::settings! {
    /// The settings of the decoder of one carrier ([`MorseDecoder`]).
    pub enum MorseSetting {
        /// The carrier.
        OffsetHz = OFFSET_HZ,
        /// The timing decoder.
        Decoder = DECODER,
        /// The speed.
        Wpm = WPM_SETTING,
    }
}

/// The timing decoder of the switch `adaptive`.
fn timing(adaptive: bool) -> TimingDecoder {
    if adaptive {
        TimingDecoder::Adaptive
    } else {
        TimingDecoder::Classic
    }
}

/// The declaration of the results of [`MorseDecoder`].
pub fn morse_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(FrontFields::SHAPE.fields)
        .with_fields(MorseFields::SHAPE.fields)
        .with_table(&STATIONS)
        .event(&SPOT)
        .event(&LINE)
}

/// The decoder of one keyed carrier (`hfkbd.morse`): complex samples at 1 kHz or more, the carrier at `offset_hz`, through a
/// 100 Hz filter, to the family's four outputs. Other graphs use it for idents and beacons.
pub struct MorseDecoder {
    rx: Option<MorseReceiver>,
    offset_hz: f64,
    adaptive: bool,
    wpm: f64,
    rate: f64,
    tuned_hz: f64,
    t: u64,
    store: TextStore,
    chars: u64,
    heard: Heard,
    status: Status,
}

impl Default for MorseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MorseDecoder {
    /// A decoder at the default settings: the carrier at 0 Hz, the adaptive timing decoder, the speed found by itself.
    pub fn new() -> Self {
        MorseDecoder {
            rx: None,
            offset_hz: 0.0,
            adaptive: true,
            wpm: 0.0,
            rate: 0.0,
            tuned_hz: 0.0,
            t: 0,
            store: decoder::store(SIGNAL_CHARS, 1),
            chars: 0,
            heard: Heard::new("CW"),
            status: Status::default(),
        }
    }

    fn rebuild(&mut self) {
        if self.rate > 0.0 {
            self.rx = MorseReceiver::new(
                self.rate,
                self.offset_hz,
                self.adaptive,
                self.wpm as f32,
                FilterWidth::Hz100,
            )
            .ok();
        }
    }
}

impl sdrkit_graph::Decoder for MorseDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.morse"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        MorseSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MorseSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MorseSetting::OffsetHz => {
                self.offset_hz = OFFSET_HZ.read(id, value)?;
                if let Some(rx) = &mut self.rx {
                    rx.set_offset_hz(self.offset_hz);
                }
                return Ok(());
            }
            MorseSetting::Decoder => {
                self.adaptive = DECODER.read(id, value)? == TimingDecoder::Adaptive;
            }
            MorseSetting::Wpm => self.wpm = WPM_SETTING.read(id, value)?,
        }
        if let Some(rx) = &mut self.rx {
            rx.set_timing(self.adaptive, self.wpm as f32);
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (MorseSetting::OffsetHz, Value::Hz(self.offset_hz)),
            (
                MorseSetting::Decoder,
                Value::Choice(timing(self.adaptive).index()),
            ),
            (MorseSetting::Wpm, Value::F64(self.wpm)),
        ])
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(morse_declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.tuned_hz = input.tuned_hz;
        let width = FilterWidth::Hz100;
        self.rx = Some(
            MorseReceiver::new(
                self.rate,
                self.offset_hz,
                self.adaptive,
                self.wpm as f32,
                width,
            )
            .map_err(|_| ConfigError::RateTooLow {
                need: width.envelope_rate(),
                got: self.rate,
            })?,
        );
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.tuned_hz = input.tuned_hz;
        self.rebuild();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let Some(rx) = &mut self.rx else {
            return;
        };
        for &s in x {
            self.t += 1;
            if rx.push(s) && rx.has_symbols() {
                let time = self.t as f64 / self.rate;
                let freq = self.tuned_hz + self.offset_hz;
                let snr = rx.snr_db();
                for sym in rx.take_symbols() {
                    self.chars += emit_symbol(
                        &sym,
                        &mut self.store,
                        out,
                        &mut self.heard,
                        time,
                        0,
                        freq,
                        snr,
                    );
                }
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        let Some(rx) = &self.rx else {
            return;
        };
        let t = &rx.timing;
        let st = &mut self.status;
        st.lock_state = if t.is_locked() {
            LockState::Locked
        } else if rx.snr_db() > 6.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        st.snr_db = Some(rx.snr_db());
        res.results_mut().set_status(st);
        self.heard.update(&self.store);
        res.put(&MorseFields {
            text: self.heard.text(),
            callsign: last_call(&self.store),
            wpm: rx.wpm(),
            snr_db: rx.snr_db() as f64,
            locked: t.is_locked(),
            farnsworth: t.farnsworth(),
            characters: self.chars as i64,
            errors: t.unknown() as i64,
        });
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
    }
}

/// One keyed carrier of the test signal.
#[derive(Clone, Debug)]
pub struct CwSignalSpec {
    /// Offset from the dial, Hz.
    pub offset_hz: f64,
    /// Character speed, WPM.
    pub wpm: f64,
    /// Overall (Farnsworth) speed, WPM. Equal to `wpm` for none.
    pub farnsworth_wpm: f64,
    /// Text, with `<XX>` procedure signals.
    pub text: String,
    /// Amplitude while the key is down.
    pub amp: f64,
    /// The relative standard deviation of each element and gap of a hand-keyed sender (0 for a machine).
    pub jitter: f64,
    /// Seconds before it starts.
    pub start_s: f64,
    /// Seconds between repeats.
    pub pause_s: f64,
}

impl CwSignalSpec {
    /// A machine-keyed carrier of amplitude 1.
    pub fn new(offset_hz: f64, wpm: f64, text: &str) -> Self {
        CwSignalSpec {
            offset_hz,
            wpm,
            farnsworth_wpm: wpm,
            text: text.into(),
            amp: 1.0,
            jitter: 0.0,
            start_s: 0.0,
            pause_s: 2.0,
        }
    }
}

/// One carrier of the test signal: the shared keyer produces the key intervals, a hand-keyed sender stretches each by its own
/// random factor, and the shared on-off keying shapes the edges (4 ms raised cosine).
struct CwTx {
    keys: Vec<Key>,
    next: usize,
    ook: Ook,
    nco: Nco,
    env: Vec<f32>,
    read: usize,
    amp: f32,
}

impl CwTx {
    fn new(s: &CwSignalSpec, rate: f64, seed: u64) -> Self {
        let keyer = Keyer::farnsworth(s.wpm, s.farnsworth_wpm.min(s.wpm))
            .or_else(|_| Keyer::new(20.0))
            .expect("20 WPM is a valid speed");
        let mut rng = Rng::new(seed);
        let mut keys: Vec<Key> = keyer
            .key(&s.text)
            .map(|k| {
                let floor = if k.on { 0.3 } else { 0.5 };
                let f = (1.0 + s.jitter * rng.normal()).max(floor);
                Key {
                    on: k.on,
                    seconds: if s.jitter > 0.0 {
                        k.seconds * f
                    } else {
                        k.seconds
                    },
                }
            })
            .collect();
        // The word gap after the text, then the pause.
        keys.push(Key {
            on: false,
            seconds: 7.0 * keyer.dot() + s.pause_s,
        });
        let mut ook = Ook::new(rate, 0.004).expect("a valid rate and rise time");
        let mut env = Vec::with_capacity(4096);
        if s.start_s > 0.0 {
            let _ = ook.push(false, s.start_s, &mut env);
        }
        CwTx {
            keys,
            next: 0,
            ook,
            nco: Nco::new(rate, s.offset_hz),
            env,
            read: 0,
            amp: s.amp as f32,
        }
    }

    fn sample(&mut self) -> Complex32 {
        while self.read == self.env.len() {
            self.env.clear();
            self.read = 0;
            while self.env.len() < 1024 {
                let k = self.keys[self.next];
                self.next = (self.next + 1) % self.keys.len();
                let _ = self.ook.push(k.on, k.seconds, &mut self.env);
            }
        }
        let e = self.env[self.read];
        self.read += 1;
        self.nco.next_sample() * (e * self.amp)
    }
}

/// The CW test signal: keyed carriers.
pub struct CwModulator {
    tx: Vec<CwTx>,
}

impl CwModulator {
    /// The carriers `signals` at `rate`. `seed` makes the hand keying repeatable.
    pub fn new(rate: f64, signals: &[CwSignalSpec], seed: u64) -> Self {
        CwModulator {
            tx: signals
                .iter()
                .enumerate()
                .map(|(i, s)| CwTx::new(s, rate, seed.wrapping_add(i as u64 * 7919)))
                .collect(),
        }
    }
}

impl Modulator for CwModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            *o = self.tx.iter_mut().map(CwTx::sample).sum();
        }
    }
}

/// `n` carriers spread evenly over ±0.95 `half_hz`, each sending its own call sign. Their speeds step from `wpm` to `wpm_max`
/// and their levels from `1 / n` of full scale down by `spread_db` dB, each in an order of its own, so that neighbours differ
/// and their sum never passes full scale.
pub fn spread_signals(
    n: usize,
    half_hz: f64,
    wpm: f64,
    wpm_max: f64,
    spread_db: f64,
) -> Vec<CwSignalSpec> {
    let top = wpm_max.max(wpm);
    let frac = |i: usize, by: usize| {
        if n > 1 {
            ((i * by) % n) as f64 / (n - 1) as f64
        } else {
            0.0
        }
    };
    (0..n)
        .map(|i| {
            let f = if n > 1 {
                -0.95 * half_hz + 1.9 * half_hz * i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            let call = spread_call(i);
            let mut s = CwSignalSpec::new(
                f,
                wpm + (top - wpm) * frac(i, 3),
                &format!("CQ DE {call} {call} K "),
            );
            s.amp = 10f64.powf(-spread_db * frac(i, 7) / 20.0) / n as f64;
            s.start_s = 0.3 * (i % 7) as f64;
            s
        })
        .collect()
}

/// The call sign that carrier `i` of [`spread_signals`] sends.
pub fn spread_call(i: usize) -> String {
    const PREFIX: [char; 8] = ['K', 'W', 'N', 'G', 'F', 'D', 'I', 'J'];
    format!(
        "{}{}{}{}",
        PREFIX[i % 8],
        (i / 8) % 10,
        (b'A' + (i % 26) as u8) as char,
        (b'A' + (i / 26 % 26) as u8) as char
    )
}

/// The three carriers of the default test signal.
pub fn demo_signals() -> Vec<CwSignalSpec> {
    vec![
        CwSignalSpec::new(400.0, 20.0, "CQ CQ DE K0TST K0TST K "),
        CwSignalSpec::new(-1500.0, 28.0, "TEST DE N0XYZ 599 <BT> 73 <SK> "),
        {
            let mut s = CwSignalSpec::new(2500.0, 15.0, "VVV DE K0TST ");
            s.farnsworth_wpm = 8.0;
            s.jitter = 0.08;
            s
        },
    ]
}

/// The generator. It reads the settings `gen_message`, `gen_wpm`, `gen_carrier_hz`, `gen_farnsworth_wpm`, `gen_jitter`,
/// `gen_signals`, `gen_wpm_max`, `gen_level_spread_db` and the span. With `gen_signals` above 0 it sends that many carriers over
/// the span ([`spread_signals`]); else `gen_message` on one carrier; else three carriers. A carrier beyond 0.45 of the rate is
/// left out. The SNR is that of all key-down carriers together in 500 Hz. With `gen_noise_only` no carrier is sent, and the
/// noise is that of the signal with its carriers.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let n = v.get(&GEN_SIGNALS).clamp(0, MAX_SIGNALS as i64) as usize;
    let mut signals = match v.get(&GEN_MESSAGE) {
        _ if n > 0 => {
            let half = 0.5 * v.get(&SPAN).width_hz();
            spread_signals(
                n,
                half,
                v.get(&GEN_WPM),
                v.get(&GEN_WPM_MAX),
                v.get(&GEN_LEVEL_SPREAD_DB),
            )
        }
        t if !t.is_empty() => {
            let mut s = CwSignalSpec::new(v.get(&GEN_CARRIER_HZ), v.get(&GEN_WPM), t);
            let farnsworth = v.get(&GEN_FARNSWORTH_WPM);
            s.farnsworth_wpm = if farnsworth > 0.0 { farnsworth } else { s.wpm };
            s.jitter = v.get(&GEN_JITTER);
            vec![s]
        }
        _ => demo_signals(),
    };
    signals.retain(|s| s.offset_hz.abs() <= 0.45 * cfg.rate_hz);
    // The power with every key down.
    let p: f64 = signals.iter().map(|s| s.amp * s.amp).sum();
    let wpm = signals.iter().map(|s| s.wpm).fold(0.0, f64::max);
    if v.get(&GEN_NOISE_ONLY) {
        signals.clear();
    }
    Ok(decoder::HfGenerator::boxed(
        cfg.rate_hz,
        CwModulator::new(cfg.rate_hz, &signals, cfg.seed),
        SignalInfo::new(500.0)
            .with_symbol_rate(wpm / 1.2)
            .with_power(p),
    ))
}
