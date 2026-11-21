//! JT6M: 6 m scatter with 44 tones at 21.53 baud, a sync tone every third symbol and one character in each other symbol. Pings
//! are read from windows of 6 s every 3 s; a long weak signal is folded into its repeating message.
//!
//! Standard: WSJT user guide, version 5, section JT6M.

use super::{Board, Kept, ScatterDecode, Shown, GEN_TEXT, TEST_TEXT};
use crate::front::AUDIO_RATE;
use crate::typed::{Jt6mDecodeEvent, Jt6mDecodeRow, Jt6mFields, JT6M_DECODE, JT6M_DECODES};
use num_complex::Complex32;
use sdrkit_algo_modem::tonegrid::{Fold, GridSpec, GridWindow, ToneGrid};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, Event, GenConfig, Generator, Hz, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, Value, Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The tone spacing and symbol rate, Hz: 11025 / 512.
pub const SPACING_HZ: f64 = 11025.0 / 512.0;
/// The symbol length, seconds.
pub const SYMBOL_S: f64 = 512.0 / 11025.0;
/// The frame: a sync symbol and two characters, seconds.
pub const FRAME_S: f64 = 3.0 * SYMBOL_S;
/// Data tones.
pub const DATA_TONES: usize = 43;
/// The character set: tone n sends character n.
pub const CHARSET: &str = "0123456789., /#?$ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The longest message, characters.
pub const MAX_LEN: usize = 58;

/// The frequency of tone `n` (−1 is the sync tone, 0 to 42 the characters): (n + 51) × 11025 / 512 Hz.
pub fn tone_hz(n: i32) -> f64 {
    f64::from(n + 51) * SPACING_HZ
}

/// The sync tone, Hz.
pub fn sync_hz() -> f64 {
    tone_hz(-1)
}

/// The tone of a character (lower case folded), `None` outside the set.
pub fn index_of(c: char) -> Option<u8> {
    CHARSET.find(c.to_ascii_uppercase()).map(|i| i as u8)
}

/// The message as it is sent: upper case, characters outside the set as spaces, trailing spaces removed, at most 58
/// characters, one space added, then padded with a space to an even length.
pub fn message(text: &str) -> String {
    let mut m: String = text
        .chars()
        .map(|c| {
            let u = c.to_ascii_uppercase();
            if index_of(u).is_some() {
                u
            } else {
                ' '
            }
        })
        .collect();
    let trimmed = m.trim_end().len();
    m.truncate(trimmed);
    m = m.chars().take(MAX_LEN).collect();
    m.push(' ');
    if m.chars().count() % 2 == 1 {
        m.push(' ');
    }
    m
}

/// The first `n` tones of the transmission of `text` (−1 for sync): sync, two characters, again, the message repeating.
pub fn tones(text: &str, n: usize) -> Vec<i32> {
    let chars: Vec<i32> = message(text)
        .chars()
        .map(|c| i32::from(index_of(c).unwrap_or(12)))
        .collect();
    (0..n)
        .map(|k| {
            if k % 3 == 0 {
                -1
            } else {
                chars[(2 * (k / 3) + (k % 3) - 1) % chars.len()]
            }
        })
        .collect()
}

/// A frame read from the audio.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Signal time of its sync symbol, seconds from the start of the audio given.
    pub time_s: f64,
    /// Its index in the window: its sync symbol is symbol 3 × index of the window's frame phase.
    pub index: usize,
    /// The sync tone over the noise level.
    pub sync_ratio: f32,
    /// The two characters (`None`: the sync tone was the strongest).
    pub chars: [Option<u8>; 2],
    /// Strongest tone over the second strongest, for each character.
    pub conf: [f32; 2],
    /// The 43 data tones over the noise, minus 1 (at least 0), for each character.
    pub ratios: [[f32; DATA_TONES]; 2],
    /// Clear: a strong sync tone that stops in the next symbol, and two data tones.
    pub clear: bool,
}

/// What one window gave.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowReading {
    /// Every frame of the window, in time order.
    pub frames: Vec<Frame>,
    /// The offset of the sync tone from 1076.66 Hz, Hz.
    pub df_hz: f64,
    /// The sync score.
    pub score: f64,
    /// The folded message and its period, when the window has one.
    pub fold: Option<(String, usize)>,
}

/// Quarter steps in a symbol and in a tone spacing.
const Q: usize = 4;

/// The least sync score of a window. The best of the about 900 hypotheses of a 6 s window of noise scores 4 to 6.
pub const SYNC_GATE: f64 = 5.5;

/// Reads a window of audio at `rate`: the sync search over the symbol phase, the frequency shift (within ±`tol_hz`) and the
/// frame phase, the frames, and the fold. `None` when the sync score is below [`SYNC_GATE`].
pub fn read_window(x: &[f32], rate: f64, tol_hz: f64) -> Option<WindowReading> {
    read_window_gated(x, rate, tol_hz, SYNC_GATE)
}

/// [`read_window`] with the least sync score `gate`.
fn read_window_gated(x: &[f32], rate: f64, tol_hz: f64, gate: f64) -> Option<WindowReading> {
    let sym = SYMBOL_S * rate;
    let step = sym / Q as f64;
    let qs = SPACING_HZ / Q as f64;
    let shifts = (2.0 * tol_hz / qs).floor() as usize + 1;
    let f0 = sync_hz() - tol_hz;
    let spec = GridSpec {
        rate,
        symbol: sym,
        pad: 4,
        window: GridWindow::Hann,
        step,
        f_lo: (f0 - 2.0 * SPACING_HZ).max(0.0),
        f_hi: (f0 + 2.0 * tol_hz + 46.0 * SPACING_HZ).min(rate / 2.0),
    };
    let grid = ToneGrid::new(x, &spec).ok()?;
    let cols = grid.columns();
    if cols < 3 * Q * 4 {
        return None;
    }
    let ntones = DATA_TONES + 1;
    // Energies and noise levels: e[(c * shifts + s) * 44 + n], level[c * shifts + s].
    let mut e = vec![0f32; cols * shifts * ntones];
    let mut level = vec![0f32; cols * shifts];
    let mut top = Vec::with_capacity(ntones);
    for c in 0..cols {
        for s in 0..shifts {
            let base = (c * shifts + s) * ntones;
            let f = f0 + s as f64 * qs;
            top.clear();
            for n in 0..ntones {
                let v = grid.power(c, f + n as f64 * SPACING_HZ);
                e[base + n] = v;
                top.push(v);
            }
            top.sort_unstable_by(|a, b| b.total_cmp(a));
            level[c * shifts + s] = top[3..].iter().sum::<f32>() / (ntones - 3) as f32;
        }
    }
    let mut sorted = level.clone();
    let floor = 0.02 * super::median(&mut sorted);
    for l in &mut level {
        *l = (*l + floor).max(1e-30);
    }
    let ratio = |c: usize, s: usize, n: usize| -> f32 {
        if c >= cols {
            return 0.0;
        }
        e[(c * shifts + s) * ntones + n] / level[c * shifts + s]
    };
    // Sync search.
    let ln3 = 3f32.ln();
    let mut best = (f64::MIN, 0usize, 0usize, 0usize);
    for q in 0..Q {
        for s in 0..shifts {
            for fp in 0..3 {
                let mut score = 0.0f64;
                let mut k = fp;
                loop {
                    let c = q + Q * k;
                    if c >= cols {
                        break;
                    }
                    let r = ratio(c, s, 0);
                    let d = r.max(1e-12).ln() - ln3;
                    if d > 0.0 && ratio(c + Q, s, 0) <= 0.5 * r {
                        score += f64::from(d.min(4.0) + 0.02 * d);
                    }
                    k += 3;
                }
                if score > best.0 {
                    best = (score, q, s, fp);
                }
            }
        }
    }
    let (score, q, s0, fp) = best;
    if score < gate {
        return None;
    }
    // Shift refinement on the strong sync symbols.
    let mut s = s0;
    let mut most = f64::MIN;
    let lo = s0.saturating_sub(6);
    let hi = (s0 + 6).min(shifts - 1);
    for cand in lo..=hi {
        let mut sum = 0.0f64;
        let mut k = fp;
        while q + Q * k < cols {
            let c = q + Q * k;
            if ratio(c, s0, 0) > 5.0 {
                sum += f64::from(e[(c * shifts + cand) * ntones]);
            }
            k += 3;
        }
        if sum > most {
            most = sum;
            s = cand;
        }
    }
    // Frames.
    let mut frames = Vec::new();
    let mut k = fp;
    let mut index = 0;
    while q + Q * (k + 2) < cols {
        let c = q + Q * k;
        let sync_ratio = ratio(c, s, 0);
        let next = ratio(c + Q, s, 0);
        let mut chars = [None; 2];
        let mut conf = [0f32; 2];
        let mut ratios = [[0f32; DATA_TONES]; 2];
        for i in 0..2 {
            let cc = c + Q * (i + 1);
            let (mut b1, mut b2, mut v1, mut v2) = (0usize, 0usize, f32::MIN, f32::MIN);
            for n in 0..ntones {
                let v = ratio(cc, s, n);
                if n > 0 {
                    ratios[i][n - 1] = (v - 1.0).max(0.0);
                }
                if v > v1 {
                    (b2, v2) = (b1, v1);
                    (b1, v1) = (n, v);
                } else if v > v2 {
                    (b2, v2) = (n, v);
                }
            }
            let _ = b2;
            chars[i] = (b1 > 0).then(|| (b1 - 1) as u8);
            conf[i] = v1 / v2.max(1e-12);
        }
        let clear =
            sync_ratio >= 5.0 && next < 0.5 * sync_ratio && chars.iter().all(Option::is_some);
        frames.push(Frame {
            time_s: grid.start_of(c) as f64 / rate,
            index,
            sync_ratio,
            chars,
            conf,
            ratios,
            clear,
        });
        k += 3;
        index += 1;
    }
    let fold = fold(&frames);
    Some(WindowReading {
        frames,
        df_hz: s as f64 * qs - tol_hz,
        score,
        fold,
    })
}

/// The folded message of the clear frames of a window: the best even period of 2 to 60 characters and one period of text from
/// an unknown start. `None` with fewer than 6 clear frames or no period above 0.35.
pub fn fold(frames: &[Frame]) -> Option<(String, usize)> {
    let clear: Vec<&Frame> = frames.iter().filter(|f| f.clear).collect();
    if clear.len() < 6 {
        return None;
    }
    let mut best: Option<(f64, Fold)> = None;
    for p in (2..=60).step_by(2) {
        let mut f = Fold::new(p, DATA_TONES);
        for fr in &clear {
            for i in 0..2 {
                f.add(2 * fr.index + i, &fr.ratios[i]);
            }
        }
        let (score, filled) = f.agreement(2);
        if filled * 2 < p {
            continue;
        }
        if best.as_ref().is_none_or(|(b, _)| score > 1.02 * b) {
            best = Some((score, f));
        }
    }
    let (score, f) = best?;
    if score <= 0.35 {
        return None;
    }
    let text = (0..f.period())
        .map(|p| match f.best(p) {
            Some((t, _, _)) if f.count(p) >= 2 => CHARSET.as_bytes()[t] as char,
            _ => '_',
        })
        .collect();
    Some((text, f.period()))
}

/// A ping: a run of clear frames, with up to two unclear frames bridged.
#[derive(Clone, Debug, PartialEq)]
pub struct Ping {
    /// Signal time of its first frame, seconds.
    pub start_s: f64,
    /// Its length, seconds.
    pub duration_s: f64,
    /// Clear frames.
    pub clear: usize,
    /// The mean sync ratio of its clear frames.
    pub sync_ratio: f64,
    /// The offset of the sync tone, Hz.
    pub df_hz: f64,
    /// The text: each character of each frame, `_` where unclear.
    pub text: String,
}

impl Ping {
    /// The SNR: 10·log10(mean sync ratio − 1), dB (not in 2500 Hz).
    pub fn snr_db(&self) -> f64 {
        10.0 * (self.sync_ratio - 1.0).max(1e-3).log10()
    }
}

/// A frame handed to the ping builder: its frame and the window's shift.
#[derive(Clone, Debug)]
struct Handed {
    frame: Frame,
    time_s: f64,
    df_hz: f64,
}

/// Builds pings from the frames of successive windows, in time order.
#[derive(Clone, Debug, Default)]
pub struct PingBuilder {
    run: Vec<Handed>,
    gap: Vec<Handed>,
    run_start: f64,
}

impl PingBuilder {
    fn ping(&self) -> Option<Ping> {
        let clear: Vec<&Handed> = self.run.iter().filter(|h| h.frame.clear).collect();
        let n = clear.len();
        if n == 0 {
            return None;
        }
        let mean = clear
            .iter()
            .map(|h| f64::from(h.frame.sync_ratio))
            .sum::<f64>()
            / n as f64;
        if !(n >= 3 || (n == 2 && mean >= 10.0)) {
            return None;
        }
        let mut text = String::new();
        for h in &self.run {
            for i in 0..2 {
                let shown = h.frame.clear || h.frame.conf[i] >= 2.0;
                match h.frame.chars[i] {
                    Some(c) if shown => text.push(CHARSET.as_bytes()[usize::from(c)] as char),
                    _ => text.push('_'),
                }
            }
        }
        let first = &self.run[0];
        let last = &self.run[self.run.len() - 1];
        Some(Ping {
            start_s: first.time_s,
            duration_s: last.time_s + FRAME_S - first.time_s,
            clear: n,
            sync_ratio: mean,
            df_hz: clear.iter().map(|h| h.df_hz).sum::<f64>() / n as f64,
            text,
        })
    }

    /// Ends the open ping; returns it when it has enough clear frames.
    pub fn close(&mut self) -> Option<Ping> {
        let p = self.ping();
        self.run.clear();
        self.gap.clear();
        p
    }

    fn push(&mut self, h: Handed, out: &mut Vec<Ping>) {
        if h.frame.clear {
            let joins = self.run.last().is_some_and(|last| {
                self.gap.len() <= 2 && h.time_s - last.time_s <= 3.5 * FRAME_S + 1e-6
            });
            if !joins {
                out.extend(self.close());
                self.run_start = h.time_s;
            }
            self.run.append(&mut self.gap);
            self.run.push(h);
            // A long continuous signal is reported every 10 s.
            if self.run[self.run.len() - 1].time_s - self.run_start >= 10.0 {
                out.extend(self.close());
            }
        } else if !self.run.is_empty() {
            self.gap.push(h);
            if self.gap.len() >= 3 {
                out.extend(self.close());
            }
        }
    }
}

/// The frequency tolerance.
pub const TOL_HZ: Setting<field::Hz> = Setting::new("tol_hz", "Frequency tolerance", "Hz", 200.0)
    .range(50.0, 400.0, 0.0)
    .help("How far the sync tone may lie from 1076.66 Hz.");

sdrkit_core::settings! {
    /// The settings of [`Jt6mDecoder`].
    pub enum Jt6mSetting {
        /// The frequency tolerance.
        Tol = TOL_HZ,
    }
}

/// The values that a ping of JT6M adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jt6mExtra {
    /// The length of the ping, s.
    pub duration_s: f64,
    /// The clear frames of the ping.
    pub clear_frames: i64,
}

impl Shown for Jt6mExtra {
    fn table() -> &'static TableDef {
        &JT6M_DECODES
    }
    fn row(d: &ScatterDecode<Self>, utc: f64, key: &str, p: &mut Published) {
        JT6M_DECODES.put(
            p,
            key,
            &Jt6mDecodeRow {
                utc: Time(utc),
                time_s: d.time_s,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                mode: &d.label,
                message: &d.text,
                duration_s: d.extra.duration_s,
                clear_frames: d.extra.clear_frames,
            },
        );
    }
    fn event(d: &ScatterDecode<Self>, utc: f64, rf_hz: f64, callsign: Option<&str>) -> Event {
        JT6M_DECODE.event(
            utc,
            rf_hz,
            &Jt6mDecodeEvent {
                mode: &d.label,
                text: &d.text,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                callsign,
                source: callsign,
                family: Family::Message,
                duration_s: d.extra.duration_s,
                clear_frames: d.extra.clear_frames,
            },
        )
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str =
    "Not confirmed by a public document: that each symbol's tone starts at phase 0, and the cut \
of a message at 58 characters.";

/// The window, seconds.
const WINDOW_S: f64 = 6.0;
/// A window every, seconds.
const HOP_S: f64 = 3.0;
/// A window hands over the frames that start from here...
const HAND_FROM_S: f64 = 1.5;
/// ...to here.
const HAND_TO_S: f64 = 4.5;

/// The JT6M receiver: 8 kHz audio in, a row and an event for each ping, and the folded message.
pub struct Jt6mDecoder {
    tol: f64,
    board: Board<Jt6mExtra>,
    kept: Kept,
    next_window: u64,
    handed_until: f64,
    pings: PingBuilder,
    folded: Option<(String, usize)>,
    tuning: Option<f64>,
    level: bool,
}

impl Default for Jt6mDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Jt6mDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        Jt6mDecoder {
            tol: TOL_HZ.default,
            board: Board::new(),
            kept: Kept::default(),
            next_window: 0,
            handed_until: f64::MIN,
            pings: PingBuilder::default(),
            folded: None,
            tuning: None,
            level: false,
        }
    }

    fn reset(&mut self) {
        let at = self.kept.end();
        self.kept.reset(at);
        self.next_window = at;
        self.handed_until = f64::MIN;
        self.pings = PingBuilder::default();
    }

    fn emit(&mut self, out: &mut Outbox, pings: Vec<Ping>) {
        for p in pings {
            self.tuning = Some(p.df_hz);
            self.board.decode(
                out,
                ScatterDecode {
                    time_s: p.start_s,
                    label: "JT6M".into(),
                    text: p.text.clone(),
                    hz: (p.df_hz * 10.0).round() / 10.0,
                    snr_db: (p.snr_db() * 10.0).round() / 10.0,
                    extra: Jt6mExtra {
                        duration_s: (p.duration_s * 100.0).round() / 100.0,
                        clear_frames: p.clear as i64,
                    },
                },
            );
        }
    }

    /// Decodes the window that starts at sample `a`, handing over the frames that start from `from_s` to `to_s` (seconds of
    /// the window).
    fn window(&mut self, a: u64, from_s: f64, to_s: f64, out: &mut Outbox) {
        let b = a + (WINDOW_S * AUDIO_RATE) as u64;
        let t0 = a as f64 / AUDIO_RATE;
        let mut done = Vec::new();
        match read_window(self.kept.span(a, b), AUDIO_RATE, self.tol) {
            None => {
                self.level = false;
                done.extend(self.pings.close());
            }
            Some(w) => {
                self.level = w.frames.iter().any(|f| f.clear);
                if w.fold.is_some() {
                    self.folded.clone_from(&w.fold);
                }
                for f in w.frames {
                    let t = t0 + f.time_s;
                    if f.time_s < from_s
                        || f.time_s >= to_s
                        || t <= self.handed_until + 0.5 * FRAME_S
                    {
                        continue;
                    }
                    self.handed_until = t;
                    self.pings.push(
                        Handed {
                            frame: f,
                            time_s: t,
                            df_hz: w.df_hz,
                        },
                        &mut done,
                    );
                }
            }
        }
        self.emit(out, done);
    }
}

impl sdrkit_graph::Decoder for Jt6mDecoder {
    type Input = f32;
    fn name(&self) -> &'static str {
        "wsjt.jt6m"
    }
    fn outputs(&self) -> Vec<Port> {
        super::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        Jt6mSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Jt6mSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Jt6mSetting::Tol => self.tol = TOL_HZ.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(Jt6mSetting::Tol, Value::Hz(self.tol))])
    }
    fn results(&self) -> Published {
        crate::typed::results("jt6m")
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.board.attach(&ctx.clock);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        super::check_rate(input)?;
        self.board.configure(input);
        self.reset();
        Ok(super::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.board.configure(input);
        self.reset();
    }
    fn process(&mut self, x: &[f32], _first: u64, out: &mut Outbox) {
        self.board.saw(self.kept.end() as f64 / AUDIO_RATE);
        self.kept.push(x);
        let win = (WINDOW_S * AUDIO_RATE) as u64;
        while self.next_window + win <= self.kept.end() {
            let a = self.next_window;
            let first = a == self.kept.first() && self.handed_until == f64::MIN;
            self.window(a, if first { 0.0 } else { HAND_FROM_S }, HAND_TO_S, out);
            self.next_window += (HOP_S * AUDIO_RATE) as u64;
        }
        // The last whole window stays for the flush.
        self.kept
            .drop_before(self.next_window.min(self.kept.end().saturating_sub(win)));
    }
    fn finish(&mut self, out: &mut Outbox) {
        // The flush reads the last 6 s and hands over every frame not handed yet.
        let win = (WINDOW_S * AUDIO_RATE) as u64;
        let a = self.kept.end().saturating_sub(win).max(self.kept.first());
        if self.kept.end() > self.next_window.saturating_sub((HOP_S * AUDIO_RATE) as u64) + win
            || self.handed_until == f64::MIN
        {
            self.window(a, 0.0, f64::MAX, out);
        }
        let p = self.pings.close();
        self.emit(out, p.into_iter().collect());
    }
    fn report(&mut self, res: &mut Published) {
        let total = self.board.total();
        let state = if total > 0 {
            LockState::Locked
        } else if self.level {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        match self.tuning {
            Some(t) => self
                .board
                .set_state(state, format_args!("JT6M, tuning {t:.0} Hz")),
            None => self.board.set_state(state, format_args!("JT6M: listening")),
        }
        self.board.status.offset_hz = self.tuning;
        self.board.status.snr_db = self.board.rows().last().map(|d| d.snr_db as f32);
        res.put(&Jt6mFields {
            folded: self.folded.as_ref().map(|f| f.0.as_str()),
            fold_period: self.folded.as_ref().map(|f| f.1 as i64),
            tuning_hz: self.tuning,
        });
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
        self.folded = None;
        self.tuning = None;
    }
    /// The folded message and the tuning are of the frequency before. The pings go unless the stack keeps them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(JT6M_DECODES.key) {
            self.board.clear();
        }
        self.folded = None;
        self.tuning = None;
    }
}

/// Starts and lengths of the test pings in the 24 s loop, seconds.
pub const GEN_PINGS: [(f64, f64); 3] = [(3.0, 0.8), (10.4, 1.6), (17.7, 0.6)];
/// The loop of the test signal, seconds.
pub const GEN_LOOP_S: f64 = 24.0;

/// A continuous test signal instead of pings.
pub const GEN_CONTINUOUS: Setting<field::Bool> =
    Setting::new("gen_continuous", "Continuous test signal", "", false)
        .section(super::GEN_SECTION)
        .help(
            "Off: pings of 0.8, 1.6 and 0.6 s in a loop of 24 s. On: the message without a break.",
        )
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the JT6M stack.
    pub enum Jt6mGenSetting {
        /// The text sent.
        Text = GEN_TEXT,
        /// A continuous signal.
        Continuous = GEN_CONTINUOUS,
    }
}

/// The test signal: the message sent without a break, heard as pings of 0.8, 1.6 and 0.6 s at 3.0, 10.4 and 17.7 s of a 24 s
/// loop (or all the time), complex with the dial where [`super::dial_in_samples`] puts it, at unit power while heard.
pub struct Jt6mGenerator {
    rate: f64,
    dial: f64,
    tones: Vec<i32>,
    continuous: bool,
    n: usize,
}

impl Jt6mGenerator {
    /// The test signal of `text` at `rate`.
    pub fn new(rate: f64, text: &str, continuous: bool) -> Self {
        let per_loop = (GEN_LOOP_S / SYMBOL_S).ceil() as usize + 1;
        Jt6mGenerator {
            rate,
            dial: super::dial_in_samples(rate),
            tones: tones(text, per_loop),
            continuous,
            n: 0,
        }
    }
}

impl Generator for Jt6mGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2500.0)
                .with_symbol_rate(SPACING_HZ)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let loop_n = (GEN_LOOP_S * self.rate) as usize;
        for o in out.iter_mut() {
            let i = self.n % loop_n;
            let t = i as f64 / self.rate;
            self.n += 1;
            let heard = self.continuous || GEN_PINGS.iter().any(|&(s, d)| t >= s && t < s + d);
            if !heard {
                *o = Complex32::default();
                continue;
            }
            let k = (t / SYMBOL_S) as usize;
            let since = t - k as f64 * SYMBOL_S;
            let f = tone_hz(self.tones[k.min(self.tones.len() - 1)]);
            // Each symbol starts at phase 0; the dial offset runs on.
            let p = TAU * (f * since + (self.dial * t).fract());
            *o = Complex32::new(p.cos() as f32, p.sin() as f32);
        }
        out.len()
    }
}

/// The generator of the stack: [`TEST_TEXT`] or `gen_text`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let t = cfg.settings.get(&GEN_TEXT);
    let text = if t.trim().is_empty() { TEST_TEXT } else { t };
    Ok(Box::new(Jt6mGenerator::new(
        cfg.rate_hz,
        text,
        cfg.settings.get(&GEN_CONTINUOUS),
    )))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static CELL: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Arc::new(Schema::of(&settings())))
        .clone()
}

/// The settings of the stack, in the order of its schema.
pub fn settings() -> Vec<&'static dyn sdrkit_core::settings::DeclareSetting> {
    super::settings(Jt6mSetting::SETTINGS, Jt6mGenSetting::SETTINGS)
}

/// The receiver of the stack.
pub fn make_receiver(
    cfg: &sdrkit_core::StackConfig,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    super::receiver("jt6m", Jt6mDecoder::new(), cfg)
}

/// The real audio of the first `seconds` of the transmission of `text` at `rate`, the tones `df_hz` from their nominal
/// frequencies (plus `drift_hz_s` × t), amplitude `amp`.
pub fn audio(
    text: &str,
    rate: f64,
    seconds: f64,
    df_hz: f64,
    drift_hz_s: f64,
    amp: f32,
) -> Vec<f32> {
    let n = (seconds * rate) as usize;
    let t = tones(text, (seconds / SYMBOL_S) as usize + 2);
    let mut phase = 0.0f64;
    let mut sym = usize::MAX;
    (0..n)
        .map(|i| {
            let tt = i as f64 / rate;
            let k = (tt / SYMBOL_S) as usize;
            if k != sym {
                sym = k;
                phase = 0.0;
            }
            let v = amp * phase.cos() as f32;
            phase += TAU * (tone_hz(t[k]) + df_hz + drift_hz_s * tt) / rate;
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_and_the_character_set() {
        assert!((tone_hz(-1) - 1076.66).abs() < 0.01);
        assert!((tone_hz(0) - 1098.19).abs() < 0.01);
        assert!((tone_hz(42) - 2002.59).abs() < 0.01);
        let idx = |c| index_of(c);
        assert_eq!(
            ['0', '.', ',', ' ', '/', '#', '?', '$', 'A', 'z'].map(idx),
            [0, 10, 11, 12, 13, 14, 15, 16, 17, 42].map(Some)
        );
        assert_eq!(idx('@'), None);
    }

    #[test]
    fn the_message_rule() {
        assert_eq!(message("K1JT W9XYZ 26"), "K1JT W9XYZ 26 ");
        assert_eq!(message("AB"), "AB  ");
        assert_eq!(message("abc"), "ABC ");
        assert_eq!(message(&"A".repeat(70)).len(), 60);
    }

    #[test]
    fn the_worked_example() {
        let t = tones("K1JT W9XYZ 26", 12);
        assert_eq!(t, vec![-1, 27, 1, -1, 26, 36, -1, 12, 39, -1, 9, 40]);
        let hz: Vec<f64> = t
            .iter()
            .map(|&n| (tone_hz(n) * 100.0).round() / 100.0)
            .collect();
        assert_eq!(
            hz,
            vec![
                1076.66, 1679.59, 1119.73, 1076.66, 1658.06, 1873.39, 1076.66, 1356.59, 1937.99,
                1076.66, 1291.99, 1959.52
            ]
        );
    }

    /// 50 windows of 6 s of noise: the gate of 4 lets about one in five through, [`SYNC_GATE`] none.
    #[test]
    fn the_sync_gate_on_noise() {
        let mut through = [0usize; 2];
        for seed in 0..50u64 {
            let mut x = vec![0f32; 6 * 8000];
            sdrkit_algo_sim::noise::Awgn::with_power(0.05 * 0.05, 9000 + seed)
                .unwrap()
                .process_real(&mut x);
            through[0] += usize::from(read_window_gated(&x, 8000.0, 200.0, 4.0).is_some());
            through[1] += usize::from(read_window(&x, 8000.0, 200.0).is_some());
        }
        assert!((5..=20).contains(&through[0]), "{through:?}");
        assert_eq!(through[1], 0, "{through:?}");
    }

    /// A continuous signal at -17 dB in 2500 Hz: every 6 s window reads the same with the gate of 4 and with [`SYNC_GATE`].
    #[test]
    fn a_continuous_weak_signal_passes_both_gates() {
        let sigma = 0.05f64;
        let amp = (2.0 * 10f64.powf(-1.7) * sigma * sigma * 2500.0 / 4000.0).sqrt() as f32;
        let mut x = audio("K1JT W9XYZ 26", 8000.0, 30.0, 60.0, 0.0, amp);
        let mut n = vec![0f32; x.len()];
        sdrkit_algo_sim::noise::Awgn::with_power(sigma * sigma, 40)
            .unwrap()
            .process_real(&mut n);
        for (v, e) in x.iter_mut().zip(&n) {
            *v += e;
        }
        let mut read = 0;
        for k in 0..5 {
            let w = &x[k * 48_000..(k + 1) * 48_000];
            let old = read_window_gated(w, 8000.0, 200.0, 4.0).map(|r| (r.score, r.frames.len()));
            let new = read_window(w, 8000.0, 200.0).map(|r| (r.score, r.frames.len()));
            assert_eq!(old, new);
            read += usize::from(new.is_some());
        }
        assert_eq!(read, 5);
    }
}
