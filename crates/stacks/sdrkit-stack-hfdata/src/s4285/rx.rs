//! The STANAG 4285 receiver: the serial-tone front (2 samples a symbol), a search for the 80 sync symbols confirmed one frame
//! later, the decision-feedback equaliser trained on the sync and the known symbols of each frame, and one deinterleaver and
//! Viterbi decoder for each coded profile; the first whose bits show the start-of-message pattern is the one read.

use super::signal::{
    is_known, lanes, row_of, scrambler, sync_bits, Profile, ScramblerStart, FRAME, PROFILES, ROWS,
    SOM, SYNC_LEN,
};
use super::{ProfileChoice, S4285RxSetting, StartChoice, PROFILE, SCRAMBLER_START};
use crate::serial::board::{formats, outputs, Board, Layout};
use crate::serial::dfe::{Dfe, LAG};
use crate::serial::front::{cis, SerialFront};
use crate::serial::soft::{llr8, max_log};
use crate::serial::{psk8, PatternWatch, EOM};
use crate::typed::{S4285Event, S4285MessageEvent, S4285_EVENT, S4285_MESSAGE};
use sdrkit_algo_fec::conv::{codes::HFDL_K7, Viterbi};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Complex32, ConfigError, LockState, Schema, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use sdrkit_util_bits::interleave::DelayLanes;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Samples a second after the front.
pub const RATE: f64 = 4800.0;
const THRESHOLD: f32 = 0.6;
const CONFIRM: f32 = 0.5;
const BEST_OF: u64 = 140;
const FRAME_SAMPLES: u64 = 2 * FRAME as u64;
const MISSED: f32 = 0.35;
const MISSED_FRAMES: usize = 8;

/// The results of the stack.
pub static LAYOUT: Layout = Layout {
    events: |d| d.event(&S4285_EVENT).event(&S4285_MESSAGE),
    event: |time, hz, text| S4285_EVENT.event(time, hz, &S4285Event { text }),
    message: |time, hz, text, bytes| {
        S4285_MESSAGE.event(time, hz, &S4285MessageEvent { text, bytes })
    },
};

/// What no public document confirms.
pub const NOTE_TEXT: &str = "STANAG 4285 is sold, so no part of this stack is confirmed by a public document: the frame, \
the sync and scrambler generators (the scrambler's start state is a setting), the mapping, the interleaver increments, the \
puncturing, the start-of-message pattern 03873C3C and the end pattern, and the STANAG 5066 D_PDU layout with its two CRCs.";

/// One coded profile being decoded.
struct Track {
    p: Profile,
    lanes: DelayLanes<f32>,
    sent: Vec<f32>,
    coded: Vec<f32>,
    llrs: Vec<i8>,
    viterbi: Viterbi,
    decoded: Vec<u8>,
    som: PatternWatch,
}

impl Track {
    fn new(p: Profile) -> Track {
        Track {
            p,
            lanes: lanes(p.j, true, 0.0f32),
            sent: Vec::with_capacity(ROWS),
            coded: Vec::new(),
            llrs: Vec::new(),
            viterbi: Viterbi::new(HFDL_K7, 64).expect("the K = 7 code"),
            decoded: Vec::new(),
            som: PatternWatch::new(SOM),
        }
    }

    /// Takes one soft value as sent; decodes the bits of each whole cycle into `decoded`.
    fn push(&mut self, v: f32) {
        self.sent.push(v);
        let per = if self.p.punctured() { 24 } else { ROWS };
        if self.sent.len() < per {
            return;
        }
        let mut out = [0.0f32; ROWS];
        let mut it = self.sent.iter();
        for (r, o) in out.iter_mut().enumerate() {
            let x = if self.p.sends_row(r) {
                it.next().copied().unwrap_or(0.0)
            } else {
                0.0
            };
            *o = self.lanes.push(x);
        }
        self.sent.clear();
        for i in 0..ROWS {
            self.coded.push(out[row_of(i)]);
        }
        let g = 2 * self.p.rep;
        let whole = self.coded.len() / g * g;
        self.llrs.clear();
        for grp in self.coded[..whole].chunks(g) {
            let (mut a, mut b) = (0.0f32, 0.0f32);
            for pr in grp.chunks(2) {
                a += pr[0];
                b += pr[1];
            }
            self.llrs.push(llr8(a, 2.0));
            self.llrs.push(llr8(b, 2.0));
        }
        self.coded.drain(..whole);
        self.viterbi.push(&self.llrs, &mut self.decoded);
    }
}

struct Session {
    p0: u64,
    w: f64,
    start: u64,
    k: usize,
    dfe: Dfe,
    missed: usize,
    tracks: Vec<Track>,
    locked: Option<usize>,
    eom: PatternWatch,
}

enum State {
    Search,
    Session(Box<Session>),
}

/// The receiver.
pub struct S4285Decoder {
    front: SerialFront,
    buf: Vec<Complex32>,
    base: u64,
    scratch: Vec<Complex32>,
    state: State,
    search_at: u64,
    board: Board,
    sync: [Complex32; SYNC_LEN],
    sync_bits: [u8; SYNC_LEN],
    scr: [u8; FRAME - SYNC_LEN],
    profile: ProfileChoice,
    start: StartChoice,
}

impl Default for S4285Decoder {
    fn default() -> Self {
        Self::new()
    }
}

enum Next {
    Wait,
    Ended,
}

fn start_of(c: StartChoice) -> ScramblerStart {
    match c {
        StartChoice::AllOnes => ScramblerStart::AllOnes,
        StartChoice::One => ScramblerStart::One,
    }
}

impl S4285Decoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        let sync_bits = sync_bits();
        let mut sync = [Complex32::default(); SYNC_LEN];
        for (s, &b) in sync.iter_mut().zip(sync_bits.iter()) {
            *s = psk8(b * 4);
        }
        S4285Decoder {
            front: SerialFront::new(5),
            buf: Vec::new(),
            base: 0,
            scratch: Vec::new(),
            state: State::Search,
            search_at: 0,
            board: Board::new(&LAYOUT),
            sync,
            sync_bits,
            scr: scrambler(ScramblerStart::AllOnes),
            profile: ProfileChoice::Auto,
            start: StartChoice::AllOnes,
        }
    }

    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    fn at(&self, n: u64) -> Complex32 {
        self.buf
            .get(n.saturating_sub(self.base) as usize)
            .copied()
            .unwrap_or_default()
    }

    /// The sync correlation at sample `p` and its block phase product.
    fn metric(&self, p: u64) -> (f32, Complex32) {
        let mut e = 0.0f32;
        let mut s = 0.0f32;
        let mut prev = Complex32::default();
        let mut dp = Complex32::default();
        for i in 0..10 {
            let mut acc = Complex32::default();
            for j in 0..8 {
                let k = 8 * i + j;
                let y = self.at(p + 2 * k as u64);
                e += y.norm_sqr();
                acc += y * self.sync[k].conj();
            }
            s += acc.norm();
            if i > 0 {
                dp += acc * prev.conj();
            }
            prev = acc;
        }
        (s / (e * SYNC_LEN as f32).sqrt().max(1e-12), dp)
    }

    fn profiles(&self) -> Vec<Profile> {
        match self.profile.profile() {
            Some(p) => vec![p],
            None => PROFILES.iter().copied().filter(|p| p.coded).collect(),
        }
    }

    fn search(&mut self, out: &mut Outbox) -> bool {
        let span = 2 * SYNC_LEN as u64;
        let need = BEST_OF + FRAME_SAMPLES + span + LAG as u64 + 4;
        self.search_at = self.search_at.max(self.base);
        while self.search_at + span <= self.end() {
            let p = self.search_at;
            if self.metric(p).0 < THRESHOLD {
                self.search_at += 1;
                continue;
            }
            if p + need > self.end() {
                return false;
            }
            let mut best = (p, 0.0f32);
            for q in p..p + BEST_OF {
                let m = self.metric(q).0;
                if m > best.1 {
                    best = (q, m);
                }
            }
            if self.metric(best.0 + FRAME_SAMPLES).0 < CONFIRM {
                self.search_at = best.0 + 1;
                continue;
            }
            let s = self.lock(best.0);
            self.board
                .event(out, best.0 as f64 / RATE, "4285 frame sync");
            self.board.waveform = match s.locked {
                Some(i) => s.tracks[i].p.name(),
                None => "searching for the rate".into(),
            };
            self.state = State::Session(Box::new(s));
            return true;
        }
        false
    }

    fn sample(&self, n: u64, p0: u64, w: f64) -> Complex32 {
        self.at(n) * cis(-w * (n as f64 - p0 as f64))
    }

    fn pair(&self, s: &Session, k: usize) -> [Complex32; 2] {
        let n = s.start + 2 * k as u64 + LAG as u64;
        [self.sample(n - 1, s.p0, s.w), self.sample(n, s.p0, s.w)]
    }

    fn lock(&mut self, p0: u64) -> Session {
        let (_, dp) = self.metric(p0);
        let w = f64::from(dp.arg()) / 16.0;
        let profiles = self.profiles();
        let plain = profiles.len() == 1 && !profiles[0].coded;
        let pts: &[u8] = if profiles.len() == 1 {
            profiles[0].labels()
        } else {
            &[0, 1, 2, 3, 4, 5, 6, 7]
        };
        let mut s = Session {
            p0,
            w,
            start: p0,
            k: 0,
            dfe: Dfe::new(pts, 0.01),
            missed: 0,
            tracks: profiles
                .iter()
                .filter(|p| p.coded)
                .map(|&p| Track::new(p))
                .collect(),
            locked: None,
            eom: PatternWatch::new(EOM),
        };
        for _ in 0..6 {
            for k in 0..SYNC_LEN {
                let pr = self.pair(&s, k);
                s.dfe.step(pr, 0, Some(self.sync_bits[k] * 4));
            }
        }
        s.k = SYNC_LEN;
        if plain {
            self.board.start_message();
            self.board.waveform = profiles[0].name();
        }
        s
    }

    fn run(&mut self, s: &mut Session, out: &mut Outbox) -> Next {
        loop {
            if s.k == FRAME {
                // The next frame: its start refined by one sample, its sync judged.
                let q = s.start + FRAME_SAMPLES;
                if q + 2 * SYNC_LEN as u64 + LAG as u64 + 2 >= self.end() {
                    return Next::Wait;
                }
                let m: Vec<f32> = (0..3).map(|d| self.metric(q + d - 1).0).collect();
                let mut best = 1;
                if m[0] > m[1] * 1.05 && m[0] >= m[2] {
                    best = 0;
                } else if m[2] > m[1] * 1.05 {
                    best = 2;
                }
                s.start = q + best - 1;
                s.k = 0;
                self.board.snr_db = Some(f64::from(s.dfe.snr_db()));
                self.board.offset_hz =
                    Some(s.w * RATE / TAU + s.dfe.loop_freq() * crate::serial::BAUD / TAU);
                if m[best as usize] < MISSED {
                    s.missed += 1;
                    if s.missed > MISSED_FRAMES {
                        let t = s.start as f64 / RATE;
                        self.board.event(out, t, "4285: sync lost");
                        self.close(s, out, t);
                        return Next::Ended;
                    }
                } else {
                    s.missed = 0;
                }
            }
            let n = s.start + 2 * s.k as u64 + LAG as u64;
            if n >= self.end() {
                return Next::Wait;
            }
            let pr = self.pair(s, s.k);
            let t = (s.start + 2 * s.k as u64) as f64 / RATE;
            let k = s.k;
            s.k += 1;
            if k < SYNC_LEN {
                s.dfe.step(pr, 0, Some(self.sync_bits[k] * 4));
                continue;
            }
            let j = k - SYNC_LEN;
            let scr = self.scr[j];
            if is_known(j) {
                s.dfe.step(pr, scr, Some(0));
                continue;
            }
            let y = s.dfe.step(pr, scr, None);
            if self.symbol(s, out, t, y) {
                return Next::Ended;
            }
        }
    }

    /// A data symbol's soft bits to every profile; true when the message ended.
    fn symbol(&mut self, s: &mut Session, out: &mut Outbox, t: f64, y: Complex32) -> bool {
        let sigma2 = s.dfe.sigma2();
        let mut soft = [[0.0f32; 3]; 4];
        for (bits, sb) in soft.iter_mut().enumerate().skip(1) {
            let labels: &[u8] = match bits {
                3 => &super::signal::MAP3,
                2 => &super::signal::MAP2,
                _ => &super::signal::MAP1,
            };
            let pts: Vec<Complex32> = labels.iter().map(|&n| psk8(n)).collect();
            max_log(y, &pts, bits, sigma2, sb);
        }
        if s.tracks.is_empty() {
            // An uncoded profile: its hard bits at once.
            let bits = self.profile.profile().map_or(1, |p| p.bits);
            for &v in &soft[bits][..bits] {
                self.board.bit(out, t, u8::from(v < 0.0));
            }
            return false;
        }
        for ti in 0..s.tracks.len() {
            let bits = s.tracks[ti].p.bits;
            for &v in &soft[bits][..bits] {
                s.tracks[ti].push(v);
            }
            let dec = std::mem::take(&mut s.tracks[ti].decoded);
            for &b in &dec {
                if s.locked == Some(ti) {
                    self.board.bit(out, t, b);
                    if s.eom.push(b) {
                        self.board.end_message(out, t, 4);
                        self.board.event(out, t, "4285: end of message");
                        s.locked = None;
                        s.eom.reset();
                        for tr in &mut s.tracks {
                            tr.som.reset();
                        }
                        break;
                    }
                } else if s.locked.is_none() && s.tracks[ti].som.push(b) {
                    s.locked = Some(ti);
                    let name = s.tracks[ti].p.name();
                    self.board.waveform = name.clone();
                    self.board
                        .event(out, t, &format!("4285: {name} (start of message found)"));
                    self.board.start_message();
                    s.eom.reset();
                    if s.tracks.len() > 1 {
                        s.dfe = s.dfe.with_points(s.tracks[ti].p.labels());
                    }
                }
            }
            let mut dec = dec;
            dec.clear();
            s.tracks[ti].decoded = dec;
        }
        false
    }

    fn close(&mut self, s: &Session, out: &mut Outbox, t: f64) {
        if !self.board.message().is_empty() {
            self.board.end_message(out, t, 0);
        }
        self.search_at = s.start;
    }

    fn trim(&mut self) {
        let keep = match &self.state {
            State::Search => self.search_at,
            State::Session(s) => s.start,
        }
        .saturating_sub(64);
        if keep > self.base + 48_000 {
            let n = ((keep - self.base) as usize).min(self.buf.len());
            self.buf.drain(..n);
            self.base += n as u64;
        }
    }

    fn reset(&mut self) {
        self.front.reset();
        self.base += self.buf.len() as u64;
        self.buf.clear();
        self.search_at = self.base;
        self.state = State::Search;
    }
}

impl sdrkit_graph::Decoder for S4285Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.s4285"
    }
    fn outputs(&self) -> Vec<Port> {
        outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        S4285RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (S4285RxSetting::Profile, Value::Choice(self.profile.index())),
            (
                S4285RxSetting::ScramblerStart,
                Value::Choice(self.start.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match S4285RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S4285RxSetting::Profile => self.profile = PROFILE.read(id, value)?,
            S4285RxSetting::ScramblerStart => {
                self.start = SCRAMBLER_START.read(id, value)?;
                self.scr = scrambler(start_of(self.start));
            }
        }
        self.state = State::Search;
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(LAYOUT.declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.board.attach(&ctx.clock);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        crate::serial::check_rate(input)?;
        self.board.configure(input);
        self.reset();
        Ok(formats())
    }
    fn retune(&mut self, input: &Format) {
        self.board.configure(input);
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.board.saw(self.end() as f64 / RATE);
        self.scratch.clear();
        self.front.process(x, &mut self.scratch);
        self.buf.extend_from_slice(&self.scratch);
        loop {
            match std::mem::replace(&mut self.state, State::Search) {
                State::Search => {
                    if !self.search(out) {
                        break;
                    }
                }
                State::Session(mut s) => match self.run(&mut s, out) {
                    Next::Wait => {
                        self.state = State::Session(s);
                        break;
                    }
                    Next::Ended => {}
                },
            }
        }
        self.trim();
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let State::Session(s) = std::mem::replace(&mut self.state, State::Search) {
            let t = s.start as f64 / RATE;
            self.close(&s, out, t);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let locked = self.board.messages > 0;
        match &self.state {
            State::Search => {
                let st = if locked {
                    LockState::Locked
                } else {
                    LockState::Searching
                };
                let n = self.board.messages;
                self.board
                    .set_state(st, format_args!("4285: searching, {n} messages"));
            }
            State::Session(s) => {
                let st = if locked || s.locked.is_some() {
                    LockState::Locked
                } else {
                    LockState::Tracking
                };
                let w = match s.locked {
                    Some(i) => s.tracks[i].p.name(),
                    None if s.tracks.is_empty() => self.board.waveform.clone(),
                    None => "searching for the rate".into(),
                };
                self.board.set_state(st, format_args!("4285: {w}"));
            }
        }
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
    }
}
