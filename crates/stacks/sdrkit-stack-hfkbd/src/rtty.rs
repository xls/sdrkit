//! RTTY: ITA2 text sent by two-tone frequency shift keying. The receiver finds the tone pair, demodulates it with two tone
//! filters, frames each character and decodes ITA2. With Auto on it also finds the speed, the shift and the polarity: it keeps
//! the last 20 s of audio, ranks trial decodes of candidate settings on a stretch of it and applies the best. The generator
//! sends the clean signal.
//!
//! Standard: ITU-T S.1 (ITA2), with the figures case of US teleprinters. The mark is the higher radio frequency: on the upper
//! sideband it is the upper audio tone, unless `reverse` is on.

use crate::auto::Trials;
use crate::decoder::{
    self, last_call, put_text, retain_rows, Heard, HfGenerator, Modulator, GEN_MESSAGE, GEN_SECTION,
};
use crate::typed::{RttyFields, RttyGuessRow, AUTO_GUESSES, AUTO_LOCK, REMOVE};
use crate::util::{db, median, Tone};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::fskauto::{BinaryFskConfig, BinaryFskFront};
use sdrkit_algo_modem::fskscan::{run_speed, tone_pairs, PairScan, TonePair};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port, RunContext};
use sdrkit_util_text::ita2;
use sdrkit_util_text::score::teleprinter_score;
use sdrkit_util_text::store::TextStore;
use std::sync::Arc;

/// The test text.
pub const TEST_TEXT: &str = "RYRYRY CQ DE K0TST TEST 0123456789 1/2 -.:? THE QUICK BROWN FOX\n";

const RATE: f64 = 8000.0;
/// The time before a lock that the framer reads again when the pair is found, seconds.
const REPLAY_S: f64 = 1.5;
const NFFT: usize = 2048;
const HOP: usize = 1024;

sdrkit_core::choice_enum! {
    /// The speeds.
    pub enum Baud {
        /// 45.45 Bd.
        B45 = ("45.45", "45.45 Bd"),
        /// 50 Bd.
        B50 = ("50", "50 Bd"),
        /// 75 Bd.
        B75 = ("75", "75 Bd"),
        /// 100 Bd.
        B100 = ("100", "100 Bd"),
    }
}

/// Every speed, slowest first.
pub const BAUDS: [Baud; 4] = [Baud::B45, Baud::B50, Baud::B75, Baud::B100];

impl Baud {
    /// The speed in baud.
    pub fn baud(self) -> f64 {
        match self {
            Baud::B45 => 45.45,
            Baud::B50 => 50.0,
            Baud::B75 => 75.0,
            Baud::B100 => 100.0,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The shifts.
    pub enum Shift {
        /// 170 Hz.
        S170 = ("170", "170 Hz"),
        /// 200 Hz.
        S200 = ("200", "200 Hz"),
        /// 425 Hz.
        S425 = ("425", "425 Hz"),
        /// 450 Hz.
        S450 = ("450", "450 Hz"),
        /// 850 Hz.
        S850 = ("850", "850 Hz"),
    }
}

/// Every shift, narrowest first.
pub const SHIFTS: [Shift; 5] = [
    Shift::S170,
    Shift::S200,
    Shift::S425,
    Shift::S450,
    Shift::S850,
];

impl Shift {
    /// The shift in Hz.
    pub fn hz(self) -> f64 {
        match self {
            Shift::S170 => 170.0,
            Shift::S200 => 200.0,
            Shift::S425 => 425.0,
            Shift::S450 => 450.0,
            Shift::S850 => 850.0,
        }
    }
}

/// The `baud` setting.
pub const BAUD: Setting<field::Choice<Baud>> = Setting::choice("baud", "Speed", Baud::B45);
/// The `shift` setting.
pub const SHIFT: Setting<field::Choice<Shift>> = Setting::choice("shift", "Shift", Shift::S170);
/// The `reverse` setting.
pub const REVERSED: Setting<field::Bool> =
    Setting::new("reverse", "Reverse", "", false)
        .help("The mark is the lower audio tone. Off, the mark is the upper tone: the higher radio frequency on USB.");
/// The `unshift_on_space` setting.
pub const UNSHIFT_ON_SPACE: Setting<field::Bool> =
    Setting::new("unshift_on_space", "Unshift on space", "", false)
        .help("A space returns the figures case to letters.");

/// The `auto` setting.
pub const AUTO: Setting<field::Bool> = Setting::new("auto", "Automatic", "", true).help(
    "Finds the speed, the shift and the polarity from the signal. Setting one of them by hand turns it off.",
);

sdrkit_core::settings! {
    /// The settings of [`RttyDecoder`].
    pub enum RttySetting {
        /// Automatic speed, shift and polarity. It comes first: a speed, shift or polarity given with it turns it off.
        Auto = AUTO,
        /// The speed.
        Baud = BAUD,
        /// The shift.
        Shift = SHIFT,
        /// The mark is the lower tone.
        Reverse = REVERSED,
        /// A space returns to letters.
        UnshiftOnSpace = UNSHIFT_ON_SPACE,
    }
}

/// The lower tone of the test signal.
pub const GEN_LOWER_HZ: Setting<field::Hz> =
    Setting::new("gen_lower_hz", "Lower tone", "Hz", 2125.0)
        .range(100.0, 3500.0, 0.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the RTTY stack.
    pub enum RttyGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
        /// The lower tone.
        LowerHz = GEN_LOWER_HZ,
    }
}

sdrkit_core::commands! {
    /// The commands of [`RttyDecoder`].
    pub enum RttyCommand {
        /// Forgets one station.
        Remove = REMOVE.command,
        /// One analysis, applied.
        AutoLock = AUTO_LOCK,
    }
}

/// The two-tone demodulator: mixers, bit-length integrators and the automatic threshold decision. SITOR uses it too.
#[derive(Clone, Debug)]
pub struct FskDemod {
    nco: [Nco; 2],
    /// Tones (lower, upper), Hz.
    pub tones: [f64; 2],
    acc: [WindowSum<Complex32>; 2],
    n: usize,
    peak: [f32; 2],
    decay: f32,
    presence: f32,
    levels: [f32; 2],
    quiet_n: usize,
    hold: usize,
    per_tone: bool,
}

impl FskDemod {
    /// A demodulator at `rate` integrating over one bit at `baud`.
    pub fn new(rate: f64, baud: f64) -> Self {
        let n = (rate / baud).round().max(2.0) as usize;
        let sum = || WindowSum::new(n).expect("at least two samples per bit");
        FskDemod {
            nco: [Nco::new(rate, 0.0), Nco::new(rate, 0.0)],
            tones: [0.0, 0.0],
            acc: [sum(), sum()],
            n,
            peak: [1e-9; 2],
            decay: (1.0 - 1.0 / (rate / baud * 16.0)) as f32,
            presence: 0.0,
            levels: [0.0; 2],
            quiet_n: 0,
            hold: rate.round() as usize,
            per_tone: true,
        }
    }
    /// The same, always judging against the mean of the two peaks. By default, when one peak is more than twice the other
    /// (selective fading), each tone is judged against its own peak; in bursts from two stations of different levels (an ARQ
    /// link) the peak of a tone not yet keyed in a burst is stale, and that judgement is wrong.
    pub fn balanced(mut self) -> Self {
        self.per_tone = false;
        self
    }
    /// Samples per bit.
    pub fn bit_len(&self) -> usize {
        self.n
    }
    /// Sets the two tones (lower, upper), Hz.
    pub fn set_tones(&mut self, tones: [f64; 2]) {
        self.tones = tones;
    }
    /// One sample: the decision (positive when the lower tone is stronger), scaled about ±1, and the decision's quality.
    #[inline]
    pub fn step(&mut self, x: Complex32) -> f32 {
        let n = self.n as f32;
        let mut e = [0.0f32; 2];
        for k in 0..2 {
            if self.nco[k].freq() != -self.tones[k] {
                self.nco[k].set_freq(-self.tones[k]);
            }
            let s = self.acc[k].push(x * self.nco[k].next_sample());
            e[k] = s.norm() as f32 / n;
        }
        // The peaks hold while both tones are under 0.3 of the larger peak (a pause in bursts of an ARQ link or a fade
        // of both), so the first unit after the pause is judged against the levels before it. They hold for 1 s at most: a
        // longer quiet is a new level (the peak of a transient would else hold for good).
        self.levels = e;
        let top = self.peak[0].max(self.peak[1]);
        self.presence = e[0].max(e[1]) / top;
        let quiet = self.presence < 0.3;
        self.quiet_n = if quiet { self.quiet_n + 1 } else { 0 };
        if !quiet || self.quiet_n > self.hold {
            for k in 0..2 {
                self.peak[k] = (self.peak[k] * self.decay).max(e[k]).max(1e-9);
            }
        }
        let (p0, p1) = (self.peak[0], self.peak[1]);
        if !self.per_tone || (p0 < 2.0 * p1 && p1 < 2.0 * p0) {
            (e[0] - e[1]) / (0.5 * (p0 + p1))
        } else {
            (e[0] / p0 - 0.5) - (e[1] / p1 - 0.5)
        }
    }
    /// The stronger tone of the last unit against the larger peak, about 1 while a tone is keyed and small in a pause.
    pub fn presence(&self) -> f32 {
        self.presence
    }
    /// The amplitudes of the lower and the upper tone over the last unit.
    pub fn levels(&self) -> [f32; 2] {
        self.levels
    }
    /// Clears the integrators.
    pub fn reset(&mut self) {
        self.acc[0].reset();
        self.acc[1].reset();
        self.peak = [1e-9; 2];
        self.quiet_n = 0;
    }
}

/// What one sample gave the framer of [`RttyCore`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Framed {
    /// No character ended.
    Nothing,
    /// A character, and the mean size of its seven decisions (about 0 to 1.5).
    Char(char, f32),
    /// A character whose start or stop element was wrong.
    FramingError,
}

/// The RTTY demodulator and framer on a known tone pair: the receiver and the trial decodes of Auto run the same code.
pub struct RttyCore {
    demod: FskDemod,
    reverse: bool,
    t: u64,
    prev: f32,
    in_char: bool,
    edge: u64,
    samples: [f32; 7],
    ita2: ita2::Decoder<'static>,
    quality: f32,
}

impl RttyCore {
    /// A framer at `rate` and `baud` on the tones `tones` (lower, upper), the mark on the upper tone unless `reverse`.
    pub fn new(rate: f64, baud: f64, tones: [f64; 2], reverse: bool) -> Self {
        let mut demod = FskDemod::new(rate, baud);
        demod.set_tones(tones);
        RttyCore {
            demod,
            reverse,
            t: 0,
            prev: 0.0,
            in_char: false,
            edge: 0,
            samples: [0.0; 7],
            ita2: ita2::Decoder::new(&TABLE),
            quality: 0.0,
        }
    }
    /// The tones (lower, upper), Hz.
    pub fn tones(&self) -> [f64; 2] {
        self.demod.tones
    }
    /// Moves the tones without dropping the character being received.
    pub fn set_tones(&mut self, tones: [f64; 2]) {
        self.demod.set_tones(tones);
    }
    /// Sets the polarity.
    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
    }
    /// A space returns the figures case to letters.
    pub fn set_unshift_on_space(&mut self, on: bool) {
        self.ita2.unshift_on_space = on;
    }
    /// The amplitudes of the lower and the upper tone over the last unit.
    pub fn levels(&self) -> [f32; 2] {
        self.demod.levels()
    }
    /// How clean the decisions are, a slow mean of the decision sizes.
    pub fn quality(&self) -> f32 {
        self.quality
    }
    /// Drops the character being received and the figures case.
    pub fn restart(&mut self) {
        self.in_char = false;
        self.ita2.reset();
    }

    /// One sample.
    #[inline]
    pub fn step(&mut self, x: Complex32) -> Framed {
        let mut d = self.demod.step(x);
        if !self.reverse {
            d = -d;
        }
        let r = self.frame(d);
        self.t += 1;
        r
    }

    fn frame(&mut self, d: f32) -> Framed {
        let n = self.demod.bit_len() as f64;
        let mut r = Framed::Nothing;
        if !self.in_char {
            if self.prev > 0.0 && d <= 0.0 {
                self.in_char = true;
                self.edge = self.t;
            }
        } else {
            let since = (self.t - self.edge) as f64;
            let k = (since / n - 0.5).round();
            if (0.0..7.0).contains(&k) && since == ((k + 0.5) * n).floor() {
                self.samples[k as usize] = d;
                if k == 0.0 && d > 0.0 {
                    // A glitch, not a start bit.
                    self.in_char = false;
                } else if k == 6.0 {
                    self.in_char = false;
                    let q = self.samples.iter().map(|v| v.abs().min(1.5)).sum::<f32>() / 7.0;
                    self.quality += 0.1 * (q - self.quality);
                    if self.samples[0] < 0.0 && self.samples[6] > 0.0 {
                        let mut code = 0u8;
                        for b in 0..5 {
                            if self.samples[1 + b] > 0.0 {
                                code |= 1 << b;
                            }
                        }
                        if let Some(ch) = self.ita2.push(code).filter(char::is_ascii) {
                            r = Framed::Char(ch, q);
                        }
                    } else if q >= 0.3 {
                        r = Framed::FramingError;
                    }
                }
            }
        }
        self.prev = d;
        r
    }
}

/// One guess of Auto: a trial decode of a stretch with fixed settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guess {
    /// The lower tone, Hz.
    pub lower_hz: f64,
    /// The shift.
    pub shift: Shift,
    /// The speed.
    pub baud: Baud,
    /// The mark is the lower tone.
    pub reverse: bool,
    /// The text score less 2.5 for each framing error.
    pub score: f64,
    /// Characters decoded.
    pub chars: u32,
    /// Framing errors.
    pub errors: u32,
    /// The mean amplitude of the stronger tone filter: tones off the signal catch less of it.
    pub level: f64,
}

/// The trials of Auto decode at most this much of the newest audio, seconds.
pub const TRIAL_S: f64 = 20.0;
/// A guess scores at least this much.
pub const MIN_SCORE: f64 = 8.0;
/// A guess has at least this many characters.
pub const MIN_CHARS: u32 = 8;
/// Guesses within this much of the best score are ranked by their level.
const TIE: f64 = 2.0;
/// After a lock, its health is not judged for this long, seconds.
pub const GRACE_S: f64 = 4.0;
/// The health of a lock is judged over this many of the last seconds.
const HEALTH_S: usize = 8;

/// A trial decode of `x` (at `rate`) on the lower tone `lower_hz` with the settings given; [`teleprinter_score`] of the text
/// less 2.5 for each framing error.
pub fn trial(
    x: &[Complex32],
    rate: f64,
    lower_hz: f64,
    shift: Shift,
    baud: Baud,
    reverse: bool,
) -> Guess {
    let mut core = RttyCore::new(
        rate,
        baud.baud(),
        [lower_hz, lower_hz + shift.hz()],
        reverse,
    );
    let (mut text, mut chars, mut errors, mut level) = (0.0, 0u32, 0u32, 0.0f64);
    for &v in x {
        let f = core.step(v);
        let l = core.levels();
        level += f64::from(l[0].max(l[1]));

        match f {
            Framed::Char(c, _) => {
                chars += 1;
                text += teleprinter_score([c]);
            }
            Framed::FramingError => errors += 1,
            Framed::Nothing => {}
        }
    }
    Guess {
        lower_hz,
        shift,
        baud,
        reverse,
        score: text - 2.5 * f64::from(errors),
        chars,
        errors,
        level: level / x.len().max(1) as f64,
    }
}

/// The 2-FSK front's lock on `x`: a pair at a listed shift (within 30 Hz) and its speed, if it locks.
fn front_lock(x: &[Complex32], rate: f64, speeds: &[f64]) -> Option<(TonePair, f64)> {
    let mut cfg = BinaryFskConfig::new(rate, speeds);
    cfg.band_hz = (300.0, 3300.0);
    cfg.shifts_hz = (140.0, 900.0);
    let mut front = BinaryFskFront::new(cfg).ok()?;
    let mut soft = Vec::new();
    front.process(x, &mut soft);
    let lock = front.lock()?;
    let shift = SHIFTS.iter().map(|s| s.hz()).min_by(|a, b| {
        (a - lock.shift())
            .abs()
            .total_cmp(&(b - lock.shift()).abs())
    })?;
    ((shift - lock.shift()).abs() <= 30.0).then_some((
        TonePair {
            lower_hz: lock.f0,
            shift_hz: shift,
            score: 0.0,
        },
        lock.baud,
    ))
}

/// The analysis of Auto on a stretch `x` of audio at `rate` (at least 3 s): tone pairs at the listed shifts from the spectrum
/// and from the 2-FSK front, candidate speeds of each pair from its run lengths, then trial decodes of the newest
/// [`TRIAL_S`] at most with each pair, candidate speed and polarity. The guesses best first, at most 3, each scoring at least
/// [`MIN_SCORE`] with at least [`MIN_CHARS`] characters; empty when none does.
pub fn analyse(x: &[Complex32], rate: f64) -> Vec<Guess> {
    let shifts: Vec<f64> = SHIFTS.iter().map(|s| s.hz()).collect();
    let speeds: Vec<f64> = BAUDS.iter().map(|b| b.baud()).collect();
    let mut pairs = tone_pairs(x, &PairScan::new(rate, &shifts));
    let front = front_lock(x, rate, &speeds);
    if let Some((p, _)) = front {
        let known = pairs
            .iter()
            .any(|q| q.shift_hz == p.shift_hz && (q.lower_hz - p.lower_hz).abs() <= 25.0);
        if !known {
            pairs.push(p);
        }
    }
    let tail = &x[x.len().saturating_sub((TRIAL_S * rate) as usize)..];
    let mut guesses = Vec::new();
    for p in &pairs {
        let Some(shift) = SHIFTS.iter().copied().find(|s| s.hz() == p.shift_hz) else {
            continue;
        };
        let mut order = run_speed(x, rate, p.lower_hz, p.shift_hz, &speeds)
            .map(|g| g.order)
            .unwrap_or_default();
        if let Some((f, b)) = front {
            let same = f.shift_hz == p.shift_hz && (f.lower_hz - p.lower_hz).abs() <= 25.0;
            if same && !order.contains(&b) {
                order.insert(0, b);
            }
        }
        for v in order {
            let Some(baud) = BAUDS.iter().copied().find(|b| b.baud() == v) else {
                continue;
            };
            for reverse in [false, true] {
                guesses.push(trial(tail, rate, p.lower_hz, shift, baud, reverse));
            }
        }
    }
    guesses.sort_by(|a, b| b.score.total_cmp(&a.score));
    // Tones 25 to 30 Hz apart decode alike: of the guesses within TIE of the best score, the one whose filters catch the
    // most of the signal goes first.
    let top = guesses.first().map_or(0.0, |g| g.score);
    if let Some(i) = guesses
        .iter()
        .enumerate()
        .take_while(|(_, g)| g.score >= top - TIE)
        .max_by(|a, b| a.1.level.total_cmp(&b.1.level))
        .map(|(i, _)| i)
    {
        let g = guesses.remove(i);
        guesses.insert(0, g);
    }
    guesses.retain(|g| g.score >= MIN_SCORE && g.chars >= MIN_CHARS);
    guesses.truncate(3);
    guesses
}

/// The RTTY decoder.
pub struct RttyDecoder {
    baud_choice: Baud,
    shift_choice: Shift,
    baud: f64,
    shift: f64,
    reverse: bool,
    unshift: bool,
    rate: f64,
    dial: f64,
    // search
    spec: WindowedFft,
    buf: Vec<Complex32>,
    avg: Vec<f32>,
    scratch: Vec<f32>,
    frames: u64,
    cand: Option<f64>,
    cand_n: u32,
    locked: bool,
    weak: u32,
    snr_db: f32,
    // demod and framer
    core: RttyCore,
    /// The index in `keep` after the last sample the framer read: a replay starts no earlier.
    framed_to: u64,
    t: u64,
    last_char: f64,
    // Auto
    auto: bool,
    trials: Trials<Vec<Guess>>,
    keep: SampleRing<Complex32>,
    guesses: Vec<Guess>,
    guesses_new: bool,
    health: [(u32, u32); HEALTH_S],
    health_at: usize,
    second: u64,
    key: String,
    stale: Vec<String>,
    // results
    store: TextStore,
    chars: u64,
    framing: u64,
    heard: Heard,
    status: Status,
    level: f64,
    level_n: u64,
}

impl Default for RttyDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RttyDecoder {
    /// A decoder at the default settings.
    pub fn new() -> Self {
        let mut trials = Trials::new();
        if AUTO.default {
            trials.on();
        }
        RttyDecoder {
            baud_choice: BAUD.default,
            shift_choice: SHIFT.default,
            baud: BAUD.default.baud(),
            shift: SHIFT.default.hz(),
            reverse: false,
            unshift: false,
            rate: RATE,
            dial: 0.0,
            spec: WindowedFft::new(NFFT, NFFT, Window::Hann)
                .expect("a window that fits the transform"),
            buf: Vec::with_capacity(NFFT),
            avg: vec![0.0; NFFT],
            scratch: Vec::new(),
            frames: 0,
            cand: None,
            cand_n: 0,
            locked: false,
            weak: 0,
            snr_db: 0.0,
            core: RttyCore::new(RATE, BAUD.default.baud(), [0.0; 2], false),
            framed_to: 0,
            t: 0,
            last_char: f64::NEG_INFINITY,
            auto: AUTO.default,
            trials,
            keep: SampleRing::new((TRIAL_S * RATE) as usize)
                .expect("a ring of at least one sample"),
            guesses: Vec::new(),
            guesses_new: false,
            health: [(0, 0); HEALTH_S],
            health_at: 0,
            second: 0,
            key: String::new(),
            stale: Vec::new(),
            store: decoder::store(8000, 0),
            chars: 0,
            framing: 0,
            heard: Heard::new("RTTY"),
            status: Status::default(),
            level: 0.0,
            level_n: 0,
        }
    }

    fn drop_sync(&mut self) {
        self.locked = false;
        self.cand = None;
        self.cand_n = 0;
        self.weak = 0;
        self.core = RttyCore::new(self.rate, self.baud, [0.0; 2], self.reverse);
        self.core.set_unshift_on_space(self.unshift);
        self.avg.iter_mut().for_each(|a| *a = 0.0);
        self.buf.clear();
        self.frames = 0;
    }

    /// A speed, shift or polarity set by hand turns Auto off.
    fn by_hand(&mut self) {
        if self.auto {
            self.auto = false;
            self.trials.off();
        }
    }

    /// Searches the pair in the newest spectrum.
    fn search(&mut self) {
        let p = self.spec.run_power(&self.buf);
        let a = if self.frames == 0 { 1.0 } else { 0.35 };
        for (s, &v) in self.avg.iter_mut().zip(p) {
            *s += a * (v - *s);
        }
        self.frames += 1;
        let bin = self.rate / NFFT as f64;
        let lo = (300.0 / bin) as usize;
        let hi = (3300.0 / bin) as usize;
        let floor = median(&self.avg[lo..hi], &mut self.scratch).max(1e-20);
        let sb = self.shift / bin;
        let s = sb.round() as usize;
        let tone = |i: usize, avg: &[f32]| -> f32 { avg[i - 1] + avg[i] + avg[i + 1] };
        let mut best = (0.0f32, 0usize);
        for b in lo..hi.saturating_sub(s) {
            let (l, u) = (tone(b, &self.avg), tone(b + s, &self.avg));
            if l < 6.0 * floor || u < 6.0 * floor {
                continue;
            }
            let mut sc = (l + u) / (6.0 * floor);
            let mark_stronger = if self.reverse { l > u } else { u > l };
            if mark_stronger {
                sc *= 1.2;
            }
            if sc > best.0 {
                best = (sc, b);
            }
        }
        let score_db = db(best.0 as f64) as f32;
        let centroid = |c: usize, avg: &[f32]| -> f64 {
            let mut num = 0.0;
            let mut den = 0.0;
            for i in c - 2..=c + 2 {
                let w = (avg[i] - floor).max(0.0) as f64;
                num += w * i as f64;
                den += w;
            }
            if den > 0.0 {
                num / den * bin
            } else {
                c as f64 * bin
            }
        };
        if best.0 > 0.0 {
            self.snr_db = score_db;
        }
        if self.locked {
            if score_db >= 15.0 && best.1 > 2 {
                self.weak = 0;
                let f0 = centroid(best.1, &self.avg);
                let f1 = centroid(best.1 + s, &self.avg);
                let t = self.core.tones();
                if (f0 - t[0]).abs() < 3.0 * bin {
                    self.core
                        .set_tones([t[0] + 0.3 * (f0 - t[0]), t[1] + 0.3 * (f1 - t[1])]);
                }
            } else {
                self.weak += 1;
                if self.weak >= 12 {
                    self.locked = false;
                    self.cand = None;
                    self.heard.logs.push("lost the signal".into());
                }
            }
            return;
        }
        if best.0 <= 0.0 || score_db < 16.0 || best.1 < 3 {
            self.cand = None;
            self.cand_n = 0;
            return;
        }
        let f = best.1 as f64 * bin;
        match self.cand {
            Some(c) if (c - f).abs() <= 2.0 * bin => self.cand_n += 1,
            _ => {
                self.cand = Some(f);
                self.cand_n = 1;
            }
        }
        if score_db >= 24.0 || self.cand_n >= 4 {
            let f0 = centroid(best.1, &self.avg);
            let f1 = centroid(best.1 + s, &self.avg);
            self.lock_tones([f0, f1]);
        }
    }

    /// Runs the framer over the kept samples of the last [`REPLAY_S`] before the sample `at` (an index of the kept ring), from
    /// after the last sample it read at most: the pair is found only once both tones were heard, and the first characters of a
    /// burst came before that.
    fn replay(&mut self, at: u64, out: &mut Outbox) {
        let from = at
            .saturating_sub((REPLAY_S * self.rate) as u64)
            .max(self.keep.start())
            .max(self.framed_to);
        for j in from..at {
            if let Some(z) = self.keep.get(j) {
                let f = self.core.step(z);
                let time = self.t.saturating_sub(at - j) as f64 / self.rate;
                self.framed(f, time, out);
            }
        }
    }

    /// Takes what the framer gave for a sample at `time` seconds.
    fn framed(&mut self, f: Framed, time: f64, out: &mut Outbox) {
        match f {
            Framed::Char(ch, q) => {
                self.chars += 1;
                self.health[self.health_at].0 += 1;
                self.last_char = time;
                let freq = self.dial + self.mark_hz().unwrap_or(0.0);
                self.heard.char(
                    &mut self.store,
                    out,
                    ch as u8,
                    time,
                    (q.min(1.0) * 100.0) as u8,
                    0,
                    freq,
                    self.snr_db,
                );
            }
            Framed::FramingError => {
                self.framing += 1;
                self.health[self.health_at].1 += 1;
            }
            Framed::Nothing => {}
        }
    }

    fn lock_tones(&mut self, tones: [f64; 2]) {
        self.locked = true;
        self.weak = 0;
        self.core.set_tones(tones);
        self.core.restart();
        self.heard.logs.push(format!(
            "locked: tones {:.1} / {:.1} Hz",
            tones[0], tones[1]
        ));
    }

    /// The mark tone, `None` while searching.
    fn mark_hz(&self) -> Option<f64> {
        let t = self.core.tones();
        self.locked.then(|| if self.reverse { t[0] } else { t[1] })
    }
    /// The space tone, `None` while searching.
    fn space_hz(&self) -> Option<f64> {
        let t = self.core.tones();
        self.locked.then(|| if self.reverse { t[1] } else { t[0] })
    }

    fn state(&self) -> u8 {
        if !self.locked {
            0
        } else if self.t as f64 / self.rate - self.last_char < 3.0 {
            2
        } else {
            1
        }
    }

    /// Takes a finished analysis, starts one that is due, and applies an answer.
    fn run_auto(&mut self) {
        if let Some((g, asked)) = self.trials.poll() {
            self.take(g, asked);
        }
        if let Some(due) = self.trials.due(self.auto, None) {
            let kept = self.keep.end().min(self.keep.capacity() as u64) as usize;
            let n = ((due.seconds * self.rate) as usize).min(kept);
            let stretch = self.keep.last(n).to_vec();
            let rate = self.rate;
            self.trials.start(due, stretch, move |x| analyse(&x, rate));
            if let Some((g, asked)) = self.trials.poll() {
                self.take(g, asked);
            }
        }
    }

    /// The guesses `g` of an analysis: the best is applied when Auto is on or a command asked for it.
    fn take(&mut self, g: Vec<Guess>, asked: bool) {
        self.guesses = g;
        self.guesses_new = true;
        if !asked && !self.auto {
            return;
        }
        let Some(&best) = self.guesses.first() else {
            self.trials.nothing();
            return;
        };
        let same = best.baud == self.baud_choice
            && best.shift == self.shift_choice
            && best.reverse == self.reverse;
        if !same {
            self.baud_choice = best.baud;
            self.baud = best.baud.baud();
            self.shift_choice = best.shift;
            self.shift = best.shift.hz();
            self.reverse = best.reverse;
            self.drop_sync();
        }
        if !self.locked {
            self.lock_tones([best.lower_hz, best.lower_hz + self.shift]);
        }
        self.heard.logs.push(format!(
            "automatic: {} Bd, {} Hz, lower tone {:.0} Hz{}",
            best.baud.key(),
            best.shift.key(),
            best.lower_hz,
            if best.reverse { ", reversed" } else { "" }
        ));
        self.trials.lock();
        self.health = [(0, 0); HEALTH_S];
    }

    /// Once a second while Auto holds a lock (after the grace): the lock is lost when the last 8 s gave no character, or at
    /// least 12 characters and framing errors of which most were errors.
    fn health_tick(&mut self) {
        if self.trials.since_lock_s().is_some_and(|s| s >= GRACE_S) {
            let (c, e) = self
                .health
                .iter()
                .fold((0, 0), |(c, e), h| (c + h.0, e + h.1));
            if c == 0 || (c + e >= 12 && e > c) {
                self.trials.unlock();
                self.heard.logs.push("automatic lock lost".into());
            }
        }
        self.health_at = (self.health_at + 1) % HEALTH_S;
        self.health[self.health_at] = (0, 0);
    }

    fn guess_rows(&mut self, res: &mut Published) {
        let n = self.guesses.len();
        retain_rows(res, &AUTO_GUESSES, &mut self.stale, |k| {
            k.parse::<usize>().is_ok_and(|i| (1..=n).contains(&i))
        });
        for (i, g) in self.guesses.iter().enumerate() {
            put_text(&mut self.key, format_args!("{}", i + 1));
            AUTO_GUESSES.put(
                res,
                &self.key,
                &RttyGuessRow {
                    lower_hz: Hertz(g.lower_hz),
                    shift_hz: Hertz(g.shift.hz()),
                    symbol_rate_hz: Hertz(g.baud.baud()),
                    reverse: g.reverse,
                    score: g.score,
                    characters: i64::from(g.chars),
                    framing_errors: i64::from(g.errors),
                },
            );
        }
    }
}

impl sdrkit_graph::Decoder for RttyDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.rtty"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        RttySetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match RttySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RttySetting::Baud => {
                let b = BAUD.read(id, value)?;
                if b != self.baud_choice {
                    self.by_hand();
                    self.baud_choice = b;
                    self.baud = b.baud();
                    self.drop_sync();
                }
            }
            RttySetting::Shift => {
                let s = SHIFT.read(id, value)?;
                if s != self.shift_choice {
                    self.by_hand();
                    self.shift_choice = s;
                    self.shift = s.hz();
                    self.drop_sync();
                }
            }
            RttySetting::Reverse => {
                let r = REVERSED.read(id, value)?;
                if r != self.reverse {
                    self.by_hand();
                    self.reverse = r;
                    self.core.set_reverse(r);
                }
            }
            RttySetting::UnshiftOnSpace => {
                self.unshift = UNSHIFT_ON_SPACE.read(id, value)?;
                self.core.set_unshift_on_space(self.unshift);
            }
            RttySetting::Auto => {
                let a = AUTO.read(id, value)?;
                if a != self.auto {
                    self.auto = a;
                    if a {
                        self.trials.on();
                    } else {
                        self.trials.off();
                    }
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (RttySetting::Baud, Value::Choice(self.baud_choice.index())),
            (RttySetting::Shift, Value::Choice(self.shift_choice.index())),
            (RttySetting::Reverse, Value::Bool(self.reverse)),
            (RttySetting::UnshiftOnSpace, Value::Bool(self.unshift)),
            (RttySetting::Auto, Value::Bool(self.auto)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("rtty")
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.trials.attach(ctx);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.dial = input.tuned_hz;
        if self.keep.capacity() != (TRIAL_S * self.rate) as usize {
            self.keep = SampleRing::new((TRIAL_S * self.rate).max(1.0) as usize)
                .expect("a ring of at least one sample");
        }
        self.keep.reset();
        self.framed_to = 0;
        self.trials.configure(self.rate);
        self.drop_sync();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.keep.reset();
        self.framed_to = 0;
        self.trials.reset();
        self.drop_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.keep.extend(x);
        let second = self.rate.round() as u64;
        for (i, &s) in x.iter().enumerate() {
            self.level += s.norm_sqr() as f64;
            self.level_n += 1;
            self.buf.push(s);
            if self.buf.len() == NFFT {
                let was = self.locked;
                self.search();
                self.buf.drain(..HOP);
                if self.locked && !was {
                    self.replay(self.keep.end() - (x.len() - i) as u64, out);
                }
            }
            if self.locked {
                let f = self.core.step(s);
                self.framed(f, self.t as f64 / self.rate, out);
                self.framed_to = self.keep.end() - (x.len() - i - 1) as u64;
            }
            self.t += 1;
            self.second += 1;
            if self.second >= second {
                self.second = 0;
                self.health_tick();
            }
        }
        self.trials.advance(x.len());
        self.run_auto();
    }
    fn report(&mut self, res: &mut Published) {
        let state = self.state();
        let st = &mut self.status;
        st.lock_state = match state {
            0 => LockState::Searching,
            1 => LockState::Tracking,
            _ => LockState::Locked,
        };
        st.snr_db = self.locked.then_some(self.snr_db);
        st.offset_hz = Some(0.0);
        if self.level_n > 0 {
            st.level_dbfs = Some(db(self.level / self.level_n as f64) as f32);
            self.level = 0.0;
            self.level_n = 0;
        }
        let what = match state {
            0 => "searching",
            1 => "locked",
            _ => "receiving",
        };
        put_text(
            &mut st.summary,
            format_args!("RTTY {}/{} {what}", self.baud, self.shift),
        );
        res.results_mut().set_status(st);
        self.heard.update(&self.store);
        res.put(&RttyFields {
            text: self.heard.text(),
            callsign: last_call(&self.store),
            mark_hz: self.mark_hz().map(Hertz),
            space_hz: self.space_hz().map(Hertz),
            symbol_rate_hz: Hertz(self.baud),
            shift_hz: Hertz(self.shift),
            reverse: self.reverse,
            characters: self.chars as i64,
            framing_errors: self.framing as i64,
            quality_pct: (self.core.quality() as f64 * 100.0).round(),
            auto: self.auto,
            auto_locked: self.trials.locked(),
            auto_state: self.trials.state() as i64,
        });
        if std::mem::take(&mut self.guesses_new) {
            self.guess_rows(res);
        }
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        RttyCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match RttyCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RttyCommand::Remove => {
                let p = self.results();
                self.heard.invoke(id, args, &p)
            }
            RttyCommand::AutoLock => {
                self.trials.ask();
                Ok(None)
            }
        }
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
        self.guesses.clear();
        self.guesses_new = true;
        self.keep.reset();
        self.trials.reset();
        self.drop_sync();
    }
}

/// The RTTY test signal: ITA2 FSK, phase-continuous, at unit power.
pub struct RttyModulator {
    rate: f64,
    lower: f64,
    shift: f64,
    baud: f64,
    reverse: bool,
    tone: Tone,
    bits: Vec<(bool, f64)>,
    i: usize,
    left: f64,
}

impl RttyModulator {
    /// Sends `text` again and again, the lower tone at `lower` Hz. Characters that ITA2 does not have are left out.
    pub fn new(rate: f64, text: &str, lower: f64, baud: f64, shift: f64, reverse: bool) -> Self {
        let mut codes = vec![];
        let mut enc = ita2::Encoder::new(&TABLE);
        for c in text.chars() {
            let _ = enc.encode_char(c, &mut |b| codes.push(b));
        }
        let mut bits = vec![(true, 15.0)];
        for c in codes {
            bits.push((false, 1.0));
            for b in 0..5 {
                bits.push(((c >> b) & 1 == 1, 1.0));
            }
            bits.push((true, 1.5));
        }
        bits.push((true, 15.0));
        RttyModulator {
            rate,
            lower,
            shift,
            baud,
            reverse,
            tone: Tone::new(rate),
            bits,
            i: 0,
            left: 0.0,
        }
    }
}

impl Modulator for RttyModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            while self.left <= 0.0 {
                self.i = (self.i + 1) % self.bits.len();
                self.left += self.bits[self.i].1 * self.rate / self.baud;
            }
            let mark = self.bits[self.i].0;
            let upper = mark != self.reverse;
            let f = self.lower + if upper { self.shift } else { 0.0 };
            *o = self.tone.next(f, 1.0);
            self.left -= 1.0;
        }
    }
}

/// The RTTY generator. It reads the settings `baud`, `shift`, `reverse`, `gen_message` and `gen_lower_hz`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let baud = v.get(&BAUD).baud();
    let shift = v.get(&SHIFT).hz();
    let m = RttyModulator::new(
        cfg.rate_hz,
        &decoder::message(cfg, TEST_TEXT),
        v.get(&GEN_LOWER_HZ),
        baud,
        shift,
        v.get(&REVERSED),
    );
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(shift + baud)
            .with_symbol_rate(baud)
            .with_bit_rate(baud),
    ))
}

/// The audio centre of the RTTY sender: the middle of the two tones.
pub const TX_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("tx_centre_hz", "Audio centre", "Hz", 2210.0)
        .range(300.0, 3000.0, 1.0)
        .section(crate::sender::TX_SECTION)
        .help("The middle of the two tones sent: 2210 Hz gives 2125 and 2295 Hz at 170 Hz shift.");

sdrkit_core::settings! {
    /// The settings of the RTTY sender.
    pub enum RttyTxSetting {
        /// The speed.
        Baud = BAUD,
        /// The shift.
        Shift = SHIFT,
        /// The mark is the lower tone.
        Reverse = REVERSED,
        /// The audio centre.
        CentreHz = TX_CENTRE_HZ,
        /// USB or LSB.
        Demod = crate::front::DEMOD,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

/// The ITA2 table of the stack, receiver and sender: the figures case of US teleprinters.
pub const TABLE: ita2::Table = ita2::US_TTY;
/// Mark before the first character of a burst, bit times.
pub const TX_LEAD_BITS: f64 = 20.0;
/// Mark after the last character of a burst, bit times.
pub const TX_TAIL_BITS: f64 = 10.0;

/// The coder of RTTY: ITA2 with the US figures case, one start bit (space), five data bits with the least significant first,
/// 1.5 stop bits (mark). A burst is 20 bit times of mark and LTRS, the characters with a case change before each character that
/// needs the other case (space, CR and LF need none), then LTRS and 10 bit times of mark. A line end is CR LF.
pub struct RttyCoder {
    baud: f64,
    mark_hz: f64,
    space_hz: f64,
    figures: bool,
}

impl RttyCoder {
    /// A coder at `baud` with the mark at `mark_hz` and the space at `space_hz`.
    pub fn new(baud: f64, mark_hz: f64, space_hz: f64) -> Self {
        RttyCoder {
            baud,
            mark_hz,
            space_hz,
            figures: false,
        }
    }
    fn bit(&self, mark: bool, bits: f64, out: &mut Vec<crate::sender::Element>) {
        let hz = if mark { self.mark_hz } else { self.space_hz };
        out.push(crate::sender::Element::tone(hz, bits / self.baud));
    }
    /// One character frame of `code`.
    fn frame(&self, code: u8, out: &mut Vec<crate::sender::Element>) {
        self.bit(false, 1.0, out);
        for b in 0..5 {
            self.bit((code >> b) & 1 == 1, 1.0, out);
        }
        self.bit(true, 1.5, out);
    }
    fn code(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        let Ok((code, need)) = TABLE.code_of(c) else {
            return;
        };
        if let Some(n) = need {
            let figures = n == ita2::Shift::Figures;
            if figures != self.figures {
                self.frame(if figures { ita2::FIGS } else { ita2::LTRS }, out);
                self.figures = figures;
            }
        }
        self.frame(code, out);
    }
}

impl crate::sender::Coder for RttyCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.bit(true, TX_LEAD_BITS, out);
        self.frame(ita2::LTRS, out);
        self.figures = false;
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        if c == '\n' {
            self.code('\r', out);
        }
        self.code(c, out);
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.frame(ita2::LTRS, out);
        self.bit(true, TX_TAIL_BITS, out);
    }
}

/// The RTTY side of the sender.
#[derive(Clone, Debug)]
pub struct RttyModem {
    baud: Baud,
    shift: Shift,
    reverse: bool,
    centre: f64,
    demod: crate::front::Demod,
    common: crate::sender::Common,
}

impl Default for RttyModem {
    fn default() -> Self {
        RttyModem {
            baud: BAUD.default,
            shift: SHIFT.default,
            reverse: REVERSED.default,
            centre: TX_CENTRE_HZ.default,
            demod: crate::front::DEMOD.default,
            common: crate::sender::Common::default(),
        }
    }
}

impl RttyModem {
    /// The mark and the space tone, Hz.
    pub fn tones(&self) -> (f64, f64) {
        let half = self.shift.hz() / 2.0;
        let (lo, hi) = (self.centre - half, self.centre + half);
        if self.reverse {
            (lo, hi)
        } else {
            (hi, lo)
        }
    }
}

impl crate::sender::Modem for RttyModem {
    const NAME: &'static str = "hfkbd.rttytx";
    fn schema() -> Arc<Schema> {
        RttyTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RttyTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RttyTxSetting::Baud => self.baud = BAUD.read(id, v)?,
            RttyTxSetting::Shift => self.shift = SHIFT.read(id, v)?,
            RttyTxSetting::Reverse => self.reverse = REVERSED.read(id, v)?,
            RttyTxSetting::CentreHz => self.centre = TX_CENTRE_HZ.read(id, v)?,
            RttyTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            RttyTxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            RttyTxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (RttyTxSetting::Baud, Value::Choice(self.baud.index())),
            (RttyTxSetting::Shift, Value::Choice(self.shift.index())),
            (RttyTxSetting::Reverse, Value::Bool(self.reverse)),
            (RttyTxSetting::CentreHz, Value::Hz(self.centre)),
            (RttyTxSetting::Demod, Value::Choice(self.demod.index())),
            (RttyTxSetting::Level, Value::F64(self.common.level)),
            (
                RttyTxSetting::TuneLimit,
                Value::Duration(self.common.tune_s),
            ),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.common
    }
    /// Letters are sent in upper case; CR LF in the text is one line end. Every other character must be in the table.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        let start = out.len();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let c = if c == '\r' && chars.peek() == Some(&'\n') {
                continue;
            } else {
                c.to_ascii_uppercase()
            };
            if c != '\n' && TABLE.code_of(c).is_err() {
                out.truncate(start);
                return Err(crate::sender::TxError::Character(c));
            }
            out.push(c);
        }
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        crate::sender::lower_of(self.demod)?;
        let (mark, space) = self.tones();
        Ok(crate::sender::Coding::Elements(Box::new(RttyCoder::new(
            self.baud.baud(),
            mark,
            space,
        ))))
    }
    fn centre_hz(&self) -> f64 {
        self.centre
    }
    /// The tones and two baud of keying sidebands each side.
    fn band(&self) -> (f64, f64) {
        let half = self.shift.hz() / 2.0 + 2.0 * self.baud.baud();
        (self.centre - half, self.centre + half)
    }
    fn lower(&self) -> bool {
        crate::sender::lower_of(self.demod).unwrap_or(false)
    }
}

/// The RTTY sender component.
pub fn sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<RttyModem>::default())
}
