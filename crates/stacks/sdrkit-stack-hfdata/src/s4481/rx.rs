//! The STANAG 4481 FSK receiver: tone filters on the 16 sub-carrier pairs, the single-channel pair found by a spectrum
//! search, noise filters between and outside the sub-carriers, a bit decision with hysteresis on each channel, the speed from
//! the run lengths, and four start-stop readers a channel (one a speed) of which the best is read.

use super::{Alphabet, ChannelMode, S4481RxSetting, Speed, ALPHABET, BAUD, CHANNEL_MODE};
use crate::typed::{S4481ChannelRow, S4481Event, S4481Fields, S4481_CHANNELS, S4481_EVENT};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Clock, Complex32, ConfigError, LockState, ResultsDecl, Schema, SettingError, SettingId,
    SettingSet, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kind, Outbox, Port, RateFrom, RunContext};
use sdrkit_util_text::ita2;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The input rate.
const RATE: f64 = 24_000.0;
/// Input samples a decision.
const DECIM: usize = 4;
/// The decision rate.
const DRATE: f64 = RATE / DECIM as f64;
/// The tone filter of the single channel, samples (10 ms, a bit at 100 Bd).
const TONE_LEN: usize = 240;
/// The tone filter of a sub-carrier, samples: 1/85 s, so that every other tone of the 85 Hz grid of the sub-carriers falls on
/// one of its nulls.
const SUB_LEN: usize = 282;
/// The noise filter, samples.
const NOISE_LEN: usize = 960;
/// The input kept for the single-channel search and for the replay into a channel just found, samples (0.5 s).
const HIST: usize = 12_000;
/// The speeds tried.
pub const SPEEDS: [f64; 4] = [45.45, 50.0, 75.0, 100.0];

/// The sub-carrier centre of channel `n` (1 to 16), Hz above the dial.
pub fn subcarrier(n: usize) -> f64 {
    255.0 + 170.0 * n as f64
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "STANAG 4481 is sold, so nothing here is confirmed by a public document: the single channel of \
+-425 Hz with the space tone above, the sub-carriers at 255 + 170 n Hz of +-42.5 Hz with the space tone above, the speeds and \
the start-stop ITA2 alphabet (not in the standard). Synchronous (encrypted) channels are measured only.";

/// The declaration of the results.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(S4481Fields::SHAPE.fields)
        .with_table(&S4481_CHANNELS)
        .event(&S4481_EVENT)
}

/// A tone filter: the input mixed to 0 Hz and averaged over its length.
#[derive(Clone)]
struct ToneFilter {
    step: Complex32,
    osc: Complex32,
    ring: Vec<Complex32>,
    at: usize,
    sum: Complex32,
    n: u32,
}

impl ToneFilter {
    fn new(f: f64, len: usize) -> Self {
        let w = -TAU * f / RATE;
        ToneFilter {
            step: Complex32::new(w.cos() as f32, w.sin() as f32),
            osc: Complex32::new(1.0, 0.0),
            ring: vec![Complex32::default(); len],
            at: 0,
            sum: Complex32::default(),
            n: 0,
        }
    }
    fn push(&mut self, x: Complex32) {
        let v = x * self.osc;
        self.osc *= self.step;
        self.n += 1;
        if self.n.is_multiple_of(1024) {
            self.osc /= self.osc.norm();
        }
        self.sum += v - self.ring[self.at];
        self.ring[self.at] = v;
        self.at = (self.at + 1) % self.ring.len();
    }
    fn level(&self) -> f32 {
        (self.sum / self.ring.len() as f32).norm_sqr()
    }
}

/// A start-stop reader at one speed.
#[derive(Clone, Debug, Default)]
struct Reader {
    bit: f64,
    /// Decisions since the start edge; `None` while waiting for one.
    since: Option<usize>,
    hold: usize,
    code: u8,
    good: f32,
    bad: f32,
    pending: Vec<u8>,
}

impl Reader {
    fn new(baud: f64) -> Self {
        Reader {
            bit: DRATE / baud,
            ..Reader::default()
        }
    }
    /// One decision `b` (1 mark), `edge` true on a mark-to-space edge. Returns a code read, or a framing error as `Err`.
    fn push(&mut self, b: u8, edge: bool) -> Option<Result<u8, ()>> {
        if self.hold > 0 {
            self.hold -= 1;
        }
        let Some(n) = self.since else {
            if edge && self.hold == 0 {
                self.since = Some(0);
                self.code = 0;
            }
            return None;
        };
        let n = n + 1;
        self.since = Some(n);
        let at = |k: f64| (k * self.bit).round() as usize;
        if n == at(0.5) && b != 0 {
            self.since = None;
            return None;
        }
        for i in 0..5 {
            if n == at(1.5 + i as f64) {
                self.code |= b << i;
            }
        }
        if n == at(6.5) {
            self.since = None;
            self.good *= 0.97;
            self.bad *= 0.97;
            if b == 1 {
                self.good += 1.0;
                self.hold = at(7.0) - at(6.5);
                return Some(Ok(self.code));
            }
            self.bad += 1.0;
            self.hold = 0;
            return Some(Err(()));
        }
        None
    }
}

/// One channel: its tones, decisions, speed and readers.
struct Chan {
    number: usize,
    mark_hz: f64,
    space_hz: f64,
    mark: ToneFilter,
    space: ToneFilter,
    e: f32,
    fill: f32,
    on: bool,
    last: u8,
    run: usize,
    edges: usize,
    errs: [f32; 4],
    speed: Option<usize>,
    readers: Vec<Reader>,
    locked: Option<usize>,
    decoder_figs: bool,
    chars: i64,
    errors: i64,
    bits: i64,
    text: String,
    /// The characters since the last text event, and the decisions since the last character.
    line: String,
    quiet: usize,
    snr: f32,
}

impl Chan {
    fn new(number: usize, mark_hz: f64, space_hz: f64) -> Chan {
        let len = if number == 0 { TONE_LEN } else { SUB_LEN };
        Chan {
            number,
            mark_hz,
            space_hz,
            mark: ToneFilter::new(mark_hz, len),
            space: ToneFilter::new(space_hz, len),
            e: 0.0,
            fill: 0.0,
            on: false,
            last: 1,
            run: 0,
            edges: 0,
            errs: [1.0; 4],
            speed: None,
            readers: SPEEDS.iter().map(|&b| Reader::new(b)).collect(),
            locked: None,
            decoder_figs: false,
            chars: 0,
            errors: 0,
            bits: 0,
            text: String::new(),
            line: String::new(),
            quiet: 0,
            snr: 0.0,
        }
    }
}

/// The receiver.
pub struct S4481Decoder {
    mode: ChannelMode,
    baud: Speed,
    alphabet: Alphabet,
    chans: Vec<Chan>,
    single: Chan,
    noise_in: Vec<ToneFilter>,
    noise_out: Vec<ToneFilter>,
    floors: Vec<f32>,
    floor: f32,
    hist: Vec<Complex32>,
    n: u64,
    reception: &'static str,
    single_on_since: bool,
    text: String,
    last_tag: Option<usize>,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    status: Status,
    rows_dirty: bool,
    replay: String,
}

impl Default for S4481Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl S4481Decoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        let mut d = S4481Decoder {
            mode: ChannelMode::Auto,
            baud: Speed::Auto,
            alphabet: Alphabet::Us,
            chans: Vec::new(),
            single: Chan::new(0, 1275.0, 2125.0),
            noise_in: Vec::new(),
            noise_out: Vec::new(),
            floors: vec![0.0; 20],
            floor: 0.0,
            hist: Vec::new(),
            n: 0,
            reception: "none",
            single_on_since: false,
            text: String::new(),
            last_tag: None,
            clock: None,
            utc0: None,
            dial: 0.0,
            status: Status::default(),
            rows_dirty: true,
            replay: String::new(),
        };
        d.reset();
        d
    }

    fn reset(&mut self) {
        self.chans = (1..=16)
            .map(|n| Chan::new(n, subcarrier(n) - 42.5, subcarrier(n) + 42.5))
            .collect();
        self.single = Chan::new(0, 1275.0, 2125.0);
        self.single.on = false;
        self.noise_in = (1..=16)
            .map(|n| ToneFilter::new(subcarrier(n) + 85.0, NOISE_LEN))
            .collect();
        self.noise_out = [150.0, 220.0, 3120.0, 3200.0]
            .iter()
            .map(|&f| ToneFilter::new(f, NOISE_LEN))
            .collect();
        self.floors = vec![0.0; 20];
        self.hist.clear();
        self.n = 0;
        self.reception = "none";
        self.single_on_since = false;
    }

    fn table(&self) -> &'static ita2::Table {
        match self.alphabet {
            Alphabet::Us => &ita2::US_TTY,
            Alphabet::Itu => &ita2::ITU,
        }
    }

    fn event(&self, out: &mut Outbox, text: &str) {
        let t = self.n as f64 / RATE;
        let e = S4481_EVENT.event(
            self.utc0.unwrap_or(0.0) + t,
            self.dial,
            &S4481Event { text },
        );
        out.messages(0).push(e);
    }

    /// The decisions of one channel at a decision instant; returns the characters to show.
    fn decide(
        c: &mut Chan,
        floor: f32,
        fixed: Option<usize>,
        table: &ita2::Table,
        out: &mut String,
    ) {
        let m = c.mark.level();
        let s = c.space.level();
        let tot = m + s;
        c.e = 0.995 * c.e + 0.005 * tot;
        let filled = if tot > 0.3 * c.e { 1.0 } else { 0.0 };
        c.fill = 0.995 * c.fill + 0.005 * filled;
        c.snr = 10.0 * (c.e / floor.max(1e-12)).max(1e-6).log10();
        let b = if (m - s).abs() < 0.2 * tot {
            c.last
        } else {
            u8::from(m > s)
        };
        let edge = c.last == 1 && b == 0;
        if b != c.last {
            // A run between edges: its error against each speed.
            if c.run > 0 {
                for (i, &sp) in SPEEDS.iter().enumerate() {
                    let len = DRATE / sp;
                    let k = (c.run as f64 / len).round().max(1.0);
                    let err = ((c.run as f64 / len - k).abs() / k.min(2.0)) as f32;
                    c.errs[i] = 0.98 * c.errs[i] + 0.02 * err;
                }
                c.edges += 1;
            }
            c.run = 0;
        }
        c.run += 1;
        c.last = b;
        if c.on {
            c.bits += 1;
        }
        if let Some(f) = fixed.or(c.locked) {
            c.speed = Some(f);
        } else if c.edges >= 20 {
            // A run of a slow speed is also whole bits of a faster one: the slowest speed that fits as well as the best.
            let best = c.errs.iter().copied().fold(f32::MAX, f32::min);
            c.speed = (0..4).find(|&i| c.errs[i] < 0.12 && c.errs[i] <= best + 0.02);
        }
        for i in 0..c.readers.len() {
            match c.readers[i].push(b, edge) {
                Some(Ok(code)) => {
                    if c.locked == Some(i) {
                        c.chars += 1;
                        push_char(c, code, table, out);
                    } else if c.readers[i].pending.len() < 200 {
                        c.readers[i].pending.push(code);
                    }
                }
                Some(Err(())) if c.locked == Some(i) => c.errors += 1,
                Some(Err(())) => {}
                None => {}
            }
        }
        // A line of text is an event once the channel has been quiet for 30 bits or the line is long.
        c.quiet += 1;
        let quiet_end = (30.0 * DRATE / SPEEDS[c.speed.unwrap_or(0)]) as usize;
        if !c.line.is_empty() && (c.quiet > quiet_end || c.line.len() >= 200) {
            let tag = if c.number == 0 {
                "[single]".to_string()
            } else {
                format!("[ch {}]", c.number)
            };
            let line = std::mem::take(&mut c.line);
            out.push_str(&format!("\u{3}{tag} {}\u{4}", line.trim_end()));
        }
        // The reader with the best score locks.
        let score = |r: &Reader| r.good - 3.0 * r.bad;
        let best = (0..4)
            .filter(|&i| fixed.is_none_or(|f| f == i))
            .max_by(|&a, &b| score(&c.readers[a]).total_cmp(&score(&c.readers[b])));
        match c.locked {
            Some(l) if c.readers[l].bad > c.readers[l].good => c.locked = None,
            None => {
                if let Some(i) = best {
                    let r = &c.readers[i];
                    if r.good >= 10.0 && r.bad < 0.35 * r.good {
                        c.locked = Some(i);
                        let pend = std::mem::take(&mut c.readers[i].pending);
                        for code in pend {
                            c.chars += 1;
                            push_char(c, code, table, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The single-channel search over the last 0.1 s: the pair 850 Hz apart, both strong, with the largest sum.
    fn search_single(&mut self, out: &mut Outbox) {
        let n = self.hist.len().min(2400);
        if n < 2400 {
            return;
        }
        let x = &self.hist[self.hist.len() - n..];
        let freqs: Vec<f64> = (0..=300).map(|i| 300.0 + 10.0 * i as f64).collect();
        let pw: Vec<f32> = freqs
            .iter()
            .map(|&f| {
                let w = -TAU * f / RATE;
                let st = Complex32::new(w.cos() as f32, w.sin() as f32);
                let mut o = Complex32::new(1.0, 0.0);
                let mut acc = Complex32::default();
                for v in x {
                    acc += v * o;
                    o *= st;
                }
                acc.norm_sqr()
            })
            .collect();
        let mut sorted = pw.clone();
        sorted.sort_by(f32::total_cmp);
        let med = sorted[sorted.len() / 2].max(1e-20);
        let mut best: Option<(usize, f32)> = None;
        for i in 0..pw.len() {
            let j = i + 85;
            if j >= pw.len() {
                break;
            }
            if pw[i] > 30.0 * med
                && pw[j] > 30.0 * med
                && best.is_none_or(|(_, s)| pw[i] + pw[j] > s)
            {
                best = Some((i, pw[i] + pw[j]));
            }
        }
        let Some((i, _)) = best else { return };
        let centre = |k: usize| {
            let lo = k.saturating_sub(6);
            let hi = (k + 6).min(pw.len() - 1);
            let s: f32 = pw[lo..=hi].iter().sum();
            (lo..=hi).map(|q| freqs[q] * f64::from(pw[q])).sum::<f64>() / f64::from(s.max(1e-20))
        };
        let (m, sp) = (centre(i), centre(i + 85));
        let moved = (m - self.single.mark_hz).abs();
        let limit = if self.single.on { 100.0 } else { 40.0 };
        if moved > limit || !self.single_on_since {
            // The new channel reads the kept input again, so the start of its text is not lost.
            let mut c = Chan::new(0, m, sp);
            let fixed = self.baud.index().checked_sub(1).map(usize::from);
            let table = self.table();
            let keep = self.hist.len().min(HIST);
            let start = self.hist.len() - keep;
            let mut text = String::new();
            for (i, &v) in self.hist[start..].iter().enumerate() {
                c.mark.push(v);
                c.space.push(v);
                if (i + 1) % DECIM == 0 {
                    Self::decide(&mut c, self.floor, fixed, table, &mut text);
                }
            }
            self.single = c;
            self.replay = text;
            self.single_on_since = true;
            self.event(
                out,
                &format!(
                    "single channel: mark {m:.0} Hz, space {sp:.0} Hz (shift {:.0} Hz)",
                    sp - m
                ),
            );
        }
    }

    fn tag(&mut self, ch: usize) {
        if self.last_tag != Some(ch) {
            if !self.text.is_empty() && !self.text.ends_with('\n') {
                self.text.push('\n');
            }
            if ch == 0 {
                self.text.push_str("[single] ");
            } else {
                self.text.push_str(&format!("[ch {ch}] "));
            }
            self.last_tag = Some(ch);
        }
    }
}

fn push_char(c: &mut Chan, code: u8, table: &ita2::Table, out: &mut String) {
    match code {
        ita2::LTRS => c.decoder_figs = false,
        ita2::FIGS => c.decoder_figs = true,
        _ => {
            if let Some(ch) = table.char_of(code, c.decoder_figs) {
                if ch == '\n' || ch == '\r' || !ch.is_control() {
                    out.push(ch);
                    c.text.push(ch);
                    c.line.push(ch);
                    c.quiet = 0;
                    if c.text.len() > 400 {
                        let cut = c.text.len() - 400;
                        let cut = (cut..c.text.len())
                            .find(|&i| c.text.is_char_boundary(i))
                            .unwrap_or(0);
                        c.text.drain(..cut);
                    }
                }
            }
        }
    }
}

impl sdrkit_graph::Decoder for S4481Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.s4481"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        S4481RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                S4481RxSetting::ChannelMode,
                Value::Choice(self.mode.index()),
            ),
            (S4481RxSetting::Baud, Value::Choice(self.baud.index())),
            (
                S4481RxSetting::Alphabet,
                Value::Choice(self.alphabet.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match S4481RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S4481RxSetting::ChannelMode => self.mode = CHANNEL_MODE.read(id, value)?,
            S4481RxSetting::Baud => self.baud = BAUD.read(id, value)?,
            S4481RxSetting::Alphabet => self.alphabet = ALPHABET.read(id, value)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = Some(ctx.clock.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        crate::serial::check_rate(input)?;
        self.dial = input.tuned_hz;
        self.reset();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - self.n as f64 / RATE);
        }
        let fixed = self.baud.index().checked_sub(1).map(usize::from);
        let table = self.table();
        let mut shown = String::new();
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.n += 1;
            for c in &mut self.chans {
                c.mark.push(v);
                c.space.push(v);
            }
            self.single.mark.push(v);
            self.single.space.push(v);
            for f in self.noise_in.iter_mut().chain(self.noise_out.iter_mut()) {
                f.push(v);
            }
            self.hist.push(v);
            if self.hist.len() > 2 * HIST {
                self.hist.drain(..HIST);
            }
            if !self.n.is_multiple_of(DECIM as u64) {
                continue;
            }
            for (i, f) in self
                .noise_in
                .iter()
                .chain(self.noise_out.iter())
                .enumerate()
            {
                self.floors[i] = 0.995 * self.floors[i] + 0.005 * f.level();
            }
            let mut inb: Vec<f32> = self.floors[..16].to_vec();
            inb.sort_by(f32::total_cmp);
            let mut outer: Vec<f32> = self.floors[16..].to_vec();
            outer.sort_by(f32::total_cmp);
            self.floor = inb[8].min(0.5 * (outer[1] + outer[2]));
            let floor = self.floor;
            let mut s_ch = String::new();
            for i in 0..16 {
                let mut text = String::new();
                Self::decide(&mut self.chans[i], floor, fixed, table, &mut text);
                // Only a channel that is on is shown.
                if !text.is_empty() && self.chans[i].on {
                    s_ch.push_str(&format!("\u{1}{}\u{2}{text}", i + 1));
                }
            }
            let mut st = std::mem::take(&mut self.replay);
            Self::decide(&mut self.single, floor, fixed, table, &mut st);
            if !self.single_on_since {
                st.clear();
            }
            // Which channels are on.
            let strongest = self.chans.iter().map(|c| c.e).fold(0.0f32, f32::max);
            let mut ons = 0;
            for c in &mut self.chans {
                let on = c.e > 40.0 * floor && c.e > 0.3 * strongest && c.fill > 0.7;
                if on && !c.on {
                    shown.push_str(&format!(
                        "\u{3}channel {} ({:.0} Hz) on\u{4}",
                        c.number,
                        subcarrier(c.number)
                    ));
                }
                c.on = on;
                ons += usize::from(on);
            }
            let s_on =
                self.single_on_since && self.single.e > 40.0 * floor && self.single.fill > 0.7;
            self.single.on = s_on;
            let reception = match self.mode {
                ChannelMode::Multi => "multi",
                ChannelMode::Single => "single",
                ChannelMode::Auto => {
                    if ons >= 3 {
                        "multi"
                    } else if s_on {
                        "single"
                    } else if ons > 0 {
                        "multi"
                    } else {
                        "none"
                    }
                }
            };
            if reception != self.reception {
                self.reception = reception;
                let what = match reception {
                    "multi" => "multi-channel FSK (+-42.5 Hz sub-carriers)",
                    "single" => "single-channel FSK (+-425 Hz)",
                    _ => "no FSK broadcast",
                };
                shown.push_str(&format!("\u{3}{what}\u{4}"));
            }
            if reception == "multi" {
                shown.push_str(&s_ch);
            } else if reception == "single" && !st.is_empty() {
                shown.push_str(&format!("\u{1}0\u{2}{st}"));
            }
            if self.mode != ChannelMode::Multi && ons < 3 && self.n.is_multiple_of(RATE as u64 / 4)
            {
                self.search_single(out);
            }
        }
        // Text and events, in order.
        let mut rest = shown.as_str();
        while let Some(c) = rest.chars().next() {
            match c {
                '\u{3}' => {
                    let end = rest.find('\u{4}').unwrap_or(rest.len());
                    let msg = rest[1..end].to_string();
                    self.event(out, &msg);
                    rest = &rest[(end + 1).min(rest.len())..];
                }
                '\u{1}' => {
                    let end = rest.find('\u{2}').unwrap_or(rest.len());
                    let ch: usize = rest[1..end].parse().unwrap_or(0);
                    self.tag(ch);
                    rest = &rest[(end + 1).min(rest.len())..];
                }
                _ => {
                    self.text.push(c);
                    rest = &rest[c.len_utf8()..];
                }
            }
        }
        if self.text.len() > 8000 {
            let cut = self.text.len() - 6000;
            let cut = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(0);
            self.text.drain(..cut);
        }
        self.rows_dirty = true;
    }
    fn report(&mut self, res: &mut Published) {
        if self.rows_dirty {
            self.rows_dirty = false;
            for c in std::iter::once(&self.single).chain(self.chans.iter()) {
                let key = format!("{}", c.number);
                let per_bit = DRATE / c.speed.map_or(50.0, |i| SPEEDS[i]);
                S4481_CHANNELS.put(
                    res,
                    &key,
                    &S4481ChannelRow {
                        channel: c.number as i64,
                        mark_hz: (c.mark_hz * 10.0).round() / 10.0,
                        space_hz: (c.space_hz * 10.0).round() / 10.0,
                        on: c.on,
                        speed_bd: c.speed.map(|i| SPEEDS[i]),
                        framing: if c.locked.is_some() {
                            "start-stop"
                        } else {
                            "sync"
                        },
                        characters: c.chars,
                        framing_errors: c.errors,
                        bits: (c.bits as f64 / per_bit) as i64,
                        snr_db: f64::from((c.snr * 10.0).round() / 10.0),
                        last_text: &c.text,
                    },
                );
            }
        }
        res.put(&S4481Fields {
            note: NOTE_TEXT,
            text: &self.text,
            reception: self.reception,
        });
        let reading: Vec<String> = std::iter::once(&self.single)
            .chain(self.chans.iter())
            .filter(|c| c.on && c.locked.is_some())
            .map(|c| {
                let sp = c.locked.map_or(0.0, |i| SPEEDS[i]);
                if c.number == 0 {
                    format!("single text {sp:.2} Bd")
                } else {
                    format!("ch {} text {sp:.2} Bd", c.number)
                }
            })
            .collect();
        self.status.lock_state = if !reading.is_empty() {
            LockState::Locked
        } else if self.reception != "none" {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        self.status.summary = if reading.is_empty() {
            format!("4481: {}", self.reception)
        } else {
            format!("4481: {}", reading.join(", "))
        };
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.text.clear();
        self.last_tag = None;
        for c in std::iter::once(&mut self.single).chain(self.chans.iter_mut()) {
            c.chars = 0;
            c.errors = 0;
            c.bits = 0;
            c.text.clear();
        }
        self.rows_dirty = true;
    }
}
