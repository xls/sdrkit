//! The Link 11 CLEW receiver (structure only): DFTs at the tones over windows referred to absolute time; a preamble of the
//! Doppler and sync tones alone; the onset of the data tones; the frame rate and alignment that keep each tone's phase inside a
//! frame; the phase steps of the 15 data tones frame by frame against the Doppler tone's; the constellation offset from their
//! fourth power.

use super::{FrameRate, Link11RxSetting, FRAME_RATE};
use crate::typed::{
    Link11Event, Link11Fields, Link11MessageEvent, Link11MessageRow, LINK11_EVENT, LINK11_MESSAGE,
    LINK11_MESSAGES,
};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Clock, Complex32, ConfigError, LockState, ResultsDecl, Schema, SettingError, SettingId,
    SettingSet, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kind, Outbox, Port, RateFrom, RunContext};
use std::collections::VecDeque;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

const RATE: f64 = 24_000.0;
/// The Doppler tone, Hz.
pub const DOPPLER_HZ: f64 = 605.0;
/// The data and sync tone, Hz.
pub const SYNC_HZ: f64 = 2915.0;
/// The 15 data tones, Hz: 935 + 110 k (k = 0 to 13) and the sync tone.
pub fn data_tones() -> [f64; 15] {
    let mut t = [SYNC_HZ; 15];
    for (k, v) in t.iter_mut().take(14).enumerate() {
        *v = 935.0 + 110.0 * k as f64;
    }
    t
}
/// The noise tones, Hz (between the data tones).
pub const NOISE_HZ: [f64; 4] = [1430.0, 1650.0, 1870.0, 2090.0];
/// The window over which tones 110 Hz apart are orthogonal, samples.
pub const WIN: usize = 218;
/// The hop of the preamble search, samples.
const HOP: u64 = 40;
/// Samples of a frame at each rate (fast, slow).
pub const FRAME_LEN: [u64; 2] = [320, 528];
/// The names of the rates.
pub const RATE_NAMES: [&str; 2] = ["fast (13.33 ms)", "slow (22 ms)"];

/// What no public document confirms.
pub const NOTE_TEXT: &str = "MIL-STD-188-203-1A and STANAG 5511 are not public, so nothing here is confirmed by a public \
document: the tone plan, the frame rates, the preamble of the Doppler and sync tones, the tone levels and the keying at the \
frame boundaries come from public summaries. No bits, words or addresses are read (their mapping and code are not public).";

/// The declaration of the results.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(Link11Fields::SHAPE.fields)
        .with_table(&LINK11_MESSAGES)
        .event(&LINK11_EVENT)
        .event(&LINK11_MESSAGE)
}

#[derive(Clone, Debug)]
struct Msg {
    start: f64,
    rate: usize,
    preamble: i64,
    data: i64,
    duration: f64,
    doppler: f64,
    offset: f64,
    snr: f64,
}

enum State {
    /// Looking for the preamble: preamble hops in a row, hops with the sync tone, the Doppler tone's phase products.
    Search {
        run: usize,
        sync: usize,
        first: u64,
        dopp: Complex32,
        last: Option<Complex32>,
    },
    /// In the preamble: waiting for the data tones.
    Preamble {
        first: u64,
        doppler_hz: f64,
        pre_noise: f32,
        /// The products of the Doppler tone's DFTs hop to hop, and the last DFT, for the residual offset.
        dopp: Complex32,
        last: Option<Complex32>,
    },
    /// The frames are read from `start`, `len` samples each.
    Frames(Box<Frames>),
}

struct Frames {
    first: u64,
    start: u64,
    rate: usize,
    doppler_hz: f64,
    frame: u64,
    prev: [Complex32; 16],
    e0: f32,
    low: usize,
    steps: Vec<[f64; 15]>,
    es: f64,
    noise: f64,
    dd: Complex32,
}

/// The receiver.
pub struct Link11Decoder {
    rate_setting: FrameRate,
    buf: Vec<Complex32>,
    base: u64,
    at: u64,
    state: State,
    rows: VecDeque<Msg>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
    text: String,
    messages: i64,
    doppler: Option<f64>,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    status: Status,
}

impl Default for Link11Decoder {
    fn default() -> Self {
        Self::new()
    }
}

fn search() -> State {
    State::Search {
        run: 0,
        sync: 0,
        first: 0,
        dopp: Complex32::default(),
        last: None,
    }
}

impl Link11Decoder {
    /// A receiver that finds the frame rate.
    pub fn new() -> Self {
        Link11Decoder {
            rate_setting: FrameRate::Auto,
            buf: Vec::new(),
            base: 0,
            at: 0,
            state: search(),
            rows: VecDeque::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
            text: String::new(),
            messages: 0,
            doppler: None,
            clock: None,
            utc0: None,
            dial: 0.0,
            status: Status::default(),
        }
    }

    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    /// The DFT at `f` Hz of the `len` samples from absolute sample `start`, referred to absolute time.
    fn dft(&self, start: u64, len: usize, f: f64) -> Complex32 {
        let w = -TAU * f / RATE;
        let p0 = (w * start as f64).rem_euclid(TAU);
        let mut o = Complex32::new(p0.cos() as f32, p0.sin() as f32);
        let st = Complex32::new(w.cos() as f32, w.sin() as f32);
        let i0 = start.saturating_sub(self.base) as usize;
        let mut acc = Complex32::default();
        for v in self.buf.iter().skip(i0).take(len) {
            acc += v * o;
            o *= st;
        }
        acc / len as f32
    }

    fn event(&self, out: &mut Outbox, t: f64, text: &str) {
        let e = LINK11_EVENT.event(
            self.utc0.unwrap_or(0.0) + t,
            self.dial,
            &Link11Event { text },
        );
        out.messages(0).push(e);
    }

    /// One hop of the preamble search at sample `p`: (Doppler energy, mean data energy, sync energy, noise, Doppler DFT).
    fn hop(&self, p: u64, shift: f64) -> (f32, f32, f32, f32, Complex32) {
        let d = self.dft(p, WIN, DOPPLER_HZ + shift);
        let s = self.dft(p, WIN, SYNC_HZ + shift).norm_sqr();
        let tones = data_tones();
        let data: f32 = tones[..14]
            .iter()
            .map(|&f| self.dft(p, WIN, f + shift).norm_sqr())
            .sum::<f32>()
            / 14.0;
        let noise: f32 = NOISE_HZ
            .iter()
            .map(|&f| self.dft(p, WIN, f + shift).norm_sqr())
            .sum::<f32>()
            / 4.0;
        (d.norm_sqr(), data, s, noise.max(1e-20), d)
    }

    /// The likeness of the phases at the start and the end of each of 8 frames of `len` samples from `s`.
    fn hold(&self, s: u64, len: u64, shift: f64) -> f32 {
        let tones = data_tones();
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for i in 0..8 {
            let a = s + i * len + 4;
            let b = s + (i + 1) * len - 4 - WIN as u64;
            for &f in &tones {
                let x1 = self.dft(a, WIN, f + shift);
                let x2 = self.dft(b, WIN, f + shift);
                num += (x1 * x2.conj()).re;
                den += x1.norm() * x2.norm();
            }
        }
        num / den.max(1e-20)
    }

    /// The DFTs of a frame's window: the 15 data tones and the Doppler tone last.
    fn frame_dfts(&self, f: &Frames, i: u64) -> [Complex32; 16] {
        let len = FRAME_LEN[f.rate];
        let win = if f.rate == 0 { WIN } else { 2 * WIN };
        let s = f.start + i * len + (len - win as u64) / 2;
        let mut x = [Complex32::default(); 16];
        for (k, &t) in data_tones().iter().enumerate() {
            x[k] = self.dft(s, win, t + f.doppler_hz);
        }
        x[15] = self.dft(s, win, DOPPLER_HZ + f.doppler_hz);
        x
    }

    /// Advances; returns false when more samples are needed.
    fn step(&mut self, out: &mut Outbox) -> bool {
        match std::mem::replace(&mut self.state, search()) {
            State::Search {
                mut run,
                mut sync,
                mut first,
                mut dopp,
                mut last,
            } => {
                let p = self.at;
                if p + WIN as u64 > self.end() {
                    self.state = State::Search {
                        run,
                        sync,
                        first,
                        dopp,
                        last,
                    };
                    return false;
                }
                let (d, data, s, noise, dx) = self.hop(p, 0.0);
                self.at += HOP;
                if d > 8.0 * noise && data < 0.2 * d {
                    if run == 0 {
                        first = p;
                        dopp = Complex32::default();
                        last = None;
                        sync = 0;
                    }
                    run += 1;
                    if s > 8.0 * noise && data < 0.2 * s {
                        sync += 1;
                    }
                    if let Some(l) = last {
                        dopp += dx * l.conj();
                    }
                    last = Some(dx);
                    if run >= 12 && sync >= 3 {
                        let doppler_hz = f64::from(dopp.arg()) * RATE / (TAU * HOP as f64);
                        self.event(
                            out,
                            first as f64 / RATE,
                            "preamble (Doppler and sync tones)",
                        );
                        self.doppler = Some(doppler_hz);
                        self.state = State::Preamble {
                            first,
                            doppler_hz,
                            pre_noise: 0.0,
                            dopp: Complex32::default(),
                            last: None,
                        };
                        return true;
                    }
                } else {
                    run = 0;
                }
                self.state = State::Search {
                    run,
                    sync,
                    first,
                    dopp,
                    last,
                };
                true
            }
            State::Preamble {
                first,
                mut doppler_hz,
                pre_noise,
                mut dopp,
                last,
            } => {
                let p = self.at;
                let need = p + 258 + 9 * FRAME_LEN[1] + WIN as u64;
                if need > self.end() {
                    self.state = State::Preamble {
                        first,
                        doppler_hz,
                        pre_noise,
                        dopp,
                        last,
                    };
                    return false;
                }
                let (d, data, _, noise, dx) = self.hop(p, doppler_hz);
                self.at += HOP;
                if data <= 0.25 * d {
                    // The noise tones are clean only while the data tones are off.
                    if d < 4.0 * noise {
                        // The tones went away.
                        return true;
                    }
                    let noise = if pre_noise > 0.0 {
                        0.9 * pre_noise + 0.1 * noise
                    } else {
                        noise
                    };
                    if let Some(l) = last {
                        dopp += dx * l.conj();
                    }
                    self.state = State::Preamble {
                        first,
                        doppler_hz,
                        pre_noise: noise,
                        dopp,
                        last: Some(dx),
                    };
                    return true;
                }
                // The residual offset over the whole preamble.
                if dopp != Complex32::default() {
                    doppler_hz += f64::from(dopp.arg()) * RATE / (TAU * HOP as f64);
                    self.doppler = Some(doppler_hz);
                }
                // The data tones start: the rate and the alignment.
                let rates: Vec<usize> = match self.rate_setting {
                    FrameRate::Auto => vec![0, 1],
                    FrameRate::Fast => vec![0],
                    FrameRate::Slow => vec![1],
                };
                let mut best = (0usize, p, f32::MIN);
                let mut per_rate = [0.0f32; 2];
                for &r in &rates {
                    let mut s = p.saturating_sub(40);
                    while s <= p + 258 {
                        let h = self.hold(s, FRAME_LEN[r], doppler_hz);
                        per_rate[r] = per_rate[r].max(h);
                        if h > best.2 {
                            best = (r, s, h);
                        }
                        s += 4;
                    }
                }
                self.event(
                    out,
                    p as f64 / RATE,
                    &format!(
                        "data tones start (phase hold: fast {:.2}, slow {:.2})",
                        per_rate[0], per_rate[1]
                    ),
                );
                let mut f = Frames {
                    first,
                    start: best.1,
                    rate: best.0,
                    doppler_hz,
                    frame: 0,
                    prev: [Complex32::default(); 16],
                    e0: 0.0,
                    low: 0,
                    steps: Vec::new(),
                    es: 0.0,
                    noise: f64::from(pre_noise),
                    dd: Complex32::new(1.0, 0.0),
                };
                f.prev = self.frame_dfts(&f, 0);
                f.e0 = f.prev[..15].iter().map(|v| v.norm_sqr()).sum::<f32>() / 15.0;
                f.frame = 1;
                self.state = State::Frames(Box::new(f));
                true
            }
            State::Frames(mut f) => {
                let len = FRAME_LEN[f.rate];
                if f.start + (f.frame + 1) * len + 8 > self.end() {
                    self.state = State::Frames(f);
                    return false;
                }
                let x = self.frame_dfts(&f, f.frame);
                let e = x[..15].iter().map(|v| v.norm_sqr()).sum::<f32>() / 15.0;
                if e < 0.15 * f.e0 {
                    f.low += 1;
                } else {
                    f.low = 0;
                }
                if f.low >= 2 || f.steps.len() >= 20_000 {
                    // The last frame read before the low ones ends the message.
                    let keep = f.steps.len().saturating_sub(1);
                    f.steps.truncate(keep);
                    self.finish_message(out, &f);
                    self.at = f.start + f.frame * len;
                    return true;
                }
                // The Doppler tone's step, smoothed over the frames: the tone is steady, its noise is not.
                let d = x[15] * f.prev[15].conj();
                f.dd = if f.frame == 1 {
                    d / d.norm().max(1e-20)
                } else {
                    f.dd * 0.7 + d / d.norm().max(1e-20) * 0.3
                };
                let dd = f.dd;
                let mut st = [0.0f64; 15];
                for k in 0..15 {
                    st[k] = f64::from((x[k] * f.prev[k].conj() * dd.conj()).arg());
                }
                if f.low == 0 {
                    f.es += f64::from(e);
                }
                f.steps.push(st);
                f.prev = x;
                f.frame += 1;
                self.state = State::Frames(f);
                true
            }
        }
    }

    fn finish_message(&mut self, out: &mut Outbox, f: &Frames) {
        let mut acc = Complex32::default();
        for s in &f.steps {
            for &v in s {
                let (sn, cs) = (4.0 * v).sin_cos();
                acc += Complex32::new(cs as f32, sn as f32);
            }
        }
        // The offset in [-10, 80) degrees: a step on an axis or 45 degrees off it keeps its quarter turn under noise.
        let wrap = 10f64.to_radians();
        let off = (f64::from(acc.arg()) / 4.0 + wrap).rem_euclid(FRAC_PI_2) - wrap;
        let n = f.steps.len();
        let digits: Vec<String> = f
            .steps
            .iter()
            .map(|s| {
                s.iter()
                    .map(|&v| {
                        let q = ((v - off) / FRAC_PI_2).round() as i64;
                        char::from(b'0' + q.rem_euclid(4) as u8)
                    })
                    .collect()
            })
            .collect();
        let len = FRAME_LEN[f.rate];
        let duration = (n as f64 + 1.0) * len as f64 / RATE;
        let preamble = ((f.start.saturating_sub(f.first)) as f64 / len as f64).round() as i64;
        let deg = off.to_degrees().round().rem_euclid(90.0) + 0.0;
        let snr = 10.0
            * (f.es / n.max(1) as f64 / f.noise.max(1e-20))
                .max(1e-6)
                .log10();
        let mut text = format!(
            "Link 11 message: {} frames, {n} data frames, {duration:.2} s, Doppler tone {:+.1} Hz, phase changes at {deg:.0} deg",
            RATE_NAMES[f.rate], f.doppler_hz
        );
        for d in digits.iter().take(8) {
            text.push(' ');
            text.push_str(d);
        }
        if n > 8 {
            text.push_str(" ...");
        }
        let t = f.start as f64 / RATE;
        let e = LINK11_MESSAGE.event(
            self.utc0.unwrap_or(0.0) + t,
            self.dial,
            &Link11MessageEvent {
                text: &text,
                data_frames: n as i64,
                steps: &digits.join(" "),
            },
        );
        out.messages(0).push(e);
        self.text.push_str(&text);
        self.text.push('\n');
        if self.text.len() > 8000 {
            let cut = self.text.len() - 6000;
            let cut = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(0);
            self.text.drain(..cut);
        }
        self.messages += 1;
        self.rows.push_back(Msg {
            start: (t * 1000.0).round() / 1000.0,
            rate: f.rate,
            preamble,
            data: n as i64,
            duration: (duration * 100.0).round() / 100.0,
            doppler: (f.doppler_hz * 10.0).round() / 10.0,
            offset: deg,
            snr: (snr * 10.0).round() / 10.0,
        });
        self.new_rows += 1;
        while self.rows.len() > LINK11_MESSAGES.cap {
            self.rows.pop_front();
        }
    }

    fn reset(&mut self) {
        self.base += self.buf.len() as u64;
        self.buf.clear();
        self.at = self.base;
        self.state = search();
    }
}

impl sdrkit_graph::Decoder for Link11Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.link11"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        Link11RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(
            Link11RxSetting::FrameRate,
            Value::Choice(self.rate_setting.index()),
        )])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Link11RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Link11RxSetting::FrameRate => self.rate_setting = FRAME_RATE.read(id, value)?,
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
            self.utc0 = Some(now - self.end() as f64 / RATE);
        }
        self.buf.extend(x.iter().map(|v| {
            if v.re.is_finite() && v.im.is_finite() {
                *v
            } else {
                Complex32::default()
            }
        }));
        while self.step(out) {}
        // Keep what the frames and the alignment search may still read.
        let keep = match &self.state {
            State::Frames(f) => f.start + f.frame.saturating_sub(1) * FRAME_LEN[f.rate],
            _ => self.at.saturating_sub(2 * FRAME_LEN[1]),
        };
        if keep > self.base + 96_000 {
            let n = ((keep - self.base) as usize).min(self.buf.len());
            self.buf.drain(..n);
            self.base += n as u64;
        }
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let State::Frames(f) = std::mem::replace(&mut self.state, search()) {
            if !f.steps.is_empty() {
                self.finish_message(out, &f);
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            res.clear_rows(&LINK11_MESSAGES);
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        for (k, m) in self.rows.iter().skip(start).enumerate() {
            let key = format!("{}", self.serial + k as u64);
            LINK11_MESSAGES.put(
                res,
                &key,
                &Link11MessageRow {
                    start_s: m.start,
                    rate: if m.rate == 0 { "fast" } else { "slow" },
                    preamble_frames: m.preamble,
                    data_frames: m.data,
                    duration_s: m.duration,
                    doppler_hz: m.doppler,
                    offset_deg: m.offset,
                    snr_db: m.snr,
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        res.put(&Link11Fields {
            note: NOTE_TEXT,
            text: &self.text,
            messages_total: self.messages,
            offset_hz: self.doppler.filter(|v| v.is_finite()).map(Hertz),
        });
        let (state, what) = match &self.state {
            State::Search { .. } => (LockState::Searching, "waiting"),
            State::Preamble { .. } => (LockState::Tracking, "preamble"),
            State::Frames(_) => (LockState::Tracking, "frames"),
        };
        self.status.lock_state = if self.messages > 0 {
            LockState::Locked
        } else {
            state
        };
        self.status.summary = format!("Link 11: {what}, {} messages", self.messages);
        self.status.offset_hz = self.doppler.filter(|v| v.is_finite());
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.new_rows = 0;
        self.text.clear();
        self.messages = 0;
        self.cleared = true;
    }
}
