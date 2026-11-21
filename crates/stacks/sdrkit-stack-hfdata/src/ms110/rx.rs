//! The MIL-STD-188-110 serial-tone receiver: the 24 kHz channel through the serial-tone front (2 samples a symbol), a search for
//! the known head of a preamble segment, the rate and interleaver read from D1 and D2, the decision-feedback equaliser trained
//! on the preamble and the known symbols, the deinterleaver, the K = 7 Viterbi decoder, the end-of-message pattern.

use super::signal::{
    count_of, data_scrambler, fetch_order, known_at, segment_symbols, set_tribit, Rate,
    CHANNEL_SYMBOL, HEAD, PATTERNS, SEGMENT, SEGMENT_HEAD, SYNC_SCRAMBLER,
};
use crate::serial::board::{formats, outputs, Board, Layout};
use crate::serial::dfe::{Dfe, LAG};
use crate::serial::front::{cis, SerialFront};
use crate::serial::soft::{llr8, max_log};
use crate::serial::{psk8, PatternWatch, EOM};
use crate::typed::{Ms110Event, Ms110MessageEvent, MS110_EVENT, MS110_MESSAGE};
use sdrkit_algo_fec::conv::{codes::HFDL_K7, Viterbi};
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, ConfigError, LockState, Values};
use sdrkit_graph::{no_values, Format, Outbox, Port, RunContext};
use std::f64::consts::TAU;
use std::sync::Arc;

/// Samples a second after the front.
pub const RATE: f64 = 4800.0;
/// The search threshold.
const THRESHOLD: f32 = 0.55;
/// Positions after the first one above the threshold among which the best is taken.
const BEST_OF: u64 = 30;
/// Bad cycles in a row that end a transmission.
const LOST_CYCLES: usize = 25;

/// The results of the stack.
pub static LAYOUT: Layout = Layout {
    events: |d| d.event(&MS110_EVENT).event(&MS110_MESSAGE),
    event: |time, hz, text| MS110_EVENT.event(time, hz, &Ms110Event { text }),
    message: |time, hz, text, bytes| {
        MS110_MESSAGE.event(time, hz, &Ms110MessageEvent { text, bytes })
    },
};

/// What no public document confirms.
pub const NOTE_TEXT: &str = "The waveform follows MIL-STD-188-110C section 5.3.2. Not confirmed by a public document: the \
STANAG 5066 D_PDU layout and its two CRCs that the D_PDU table reads, and the matched filter (the standard specifies the \
spectrum). The 0.0 s interleaver and the 2400 bit/s voice setting are not decoded. The acquisition range is +-130 Hz.";

/// A transmission being received.
struct Session {
    p0: u64,
    w: f64,
    rate: Rate,
    long: bool,
    dfe: Dfe,
    k: usize,
    data_start: usize,
    pre: Vec<(u8, u8)>,
    order: Option<Vec<u32>>,
    soft: Vec<f32>,
    sent: usize,
    sets: Vec<Complex32>,
    points: Vec<Complex32>,
    viterbi: Viterbi,
    decoded: Vec<u8>,
    llrs: Vec<i8>,
    eom: PatternWatch,
    bad_run: usize,
    cycle_err: f32,
    weak_sets: usize,
}

enum State {
    Search,
    Session(Box<Session>),
}

/// The receiver.
pub struct Ms110Decoder {
    front: SerialFront,
    buf: Vec<Complex32>,
    base: u64,
    scratch: Vec<Complex32>,
    state: State,
    search_at: u64,
    board: Board,
    head: Vec<Complex32>,
    scr: [u8; 160],
}

impl Default for Ms110Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// What a session step ends with.
enum Next {
    Wait,
    Ended,
}

impl Ms110Decoder {
    /// A receiver.
    pub fn new() -> Self {
        let syms = segment_symbols(0, 0, 0);
        let head = (0..HEAD)
            .map(|k| psk8(super::signal::segment_tribit(&syms, k)))
            .collect();
        Ms110Decoder {
            front: SerialFront::new(5),
            buf: Vec::new(),
            base: 0,
            scratch: Vec::new(),
            state: State::Search,
            search_at: 0,
            board: Board::new(&LAYOUT),
            head,
            scr: data_scrambler(),
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

    /// The block correlations of the segment head at sample `p`, and the energy of its samples.
    fn blocks(&self, p: u64, b: &mut [Complex32; 36]) -> f32 {
        let i0 = (p - self.base) as usize;
        let mut e = 0.0f32;
        for (i, bi) in b.iter_mut().enumerate() {
            let mut acc = Complex32::default();
            for j in 0..8 {
                let k = 8 * i + j;
                let y = self.buf[i0 + 2 * k];
                e += y.norm_sqr();
                acc += y * self.head[k].conj();
            }
            *bi = acc;
        }
        e
    }

    fn metric(&self, p: u64) -> f32 {
        let mut b = [Complex32::default(); 36];
        let e = self.blocks(p, &mut b);
        let s: f32 = b.iter().map(|v| v.norm()).sum();
        s / (e * HEAD as f32).sqrt().max(1e-12)
    }

    /// Searches; true when a session started.
    fn search(&mut self, out: &mut Outbox) -> bool {
        let span = 2 * HEAD as u64;
        let need = BEST_OF + 2 * SEGMENT as u64 + LAG as u64 + 4;
        self.search_at = self.search_at.max(self.base);
        while self.search_at + span <= self.end() {
            let p = self.search_at;
            if self.metric(p) < THRESHOLD {
                self.search_at += 1;
                continue;
            }
            if p + need > self.end() {
                return false;
            }
            let mut best = (p, 0.0f32);
            for q in p..p + BEST_OF {
                let m = self.metric(q);
                if m > best.1 {
                    best = (q, m);
                }
            }
            match self.lock(best.0) {
                Some(s) => {
                    let t = best.0 as f64 / RATE;
                    self.board.waveform = s.rate.name(s.long);
                    self.board
                        .event(out, t, &format!("110 sync: {}", s.rate.name(s.long)));
                    self.board.start_message();
                    self.state = State::Session(Box::new(s));
                    return true;
                }
                None => self.search_at = best.0 + 1,
            }
        }
        false
    }

    fn sample(&self, n: u64, p0: u64, w: f64) -> Complex32 {
        self.at(n) * cis(-w * (n as f64 - p0 as f64))
    }

    fn pair(&self, p0: u64, w: f64, k: usize) -> [Complex32; 2] {
        let n = p0 + 2 * k as u64 + LAG as u64;
        [self.sample(n - 1, p0, w), self.sample(n, p0, w)]
    }

    /// Reads a preamble segment at `p0`: the equaliser trained, D1, D2 and the count read. `None` when they name no rate.
    fn lock(&self, p0: u64) -> Option<Session> {
        let mut b = [Complex32::default(); 36];
        self.blocks(p0, &mut b);
        let mut acc = Complex32::default();
        for i in 0..35 {
            acc += b[i + 1] * b[i].conj();
        }
        let w = f64::from(acc.arg()) / 16.0;
        let mut eq = Dfe::new(&[0, 1, 2, 3, 4, 5, 6, 7], 0.005);
        let pat = |c: u8, k: usize| PATTERNS[usize::from(c)][k % 8];
        for _ in 0..4 {
            for k in 0..HEAD {
                let c = SEGMENT_HEAD[k / CHANNEL_SYMBOL];
                eq.step(self.pair(p0, w, k), SYNC_SCRAMBLER[k % 32], Some(pat(c, k)));
            }
        }
        let mut read = [0u8; 6];
        for (m, r) in read.iter_mut().enumerate() {
            let mut score = [0.0f32; 8];
            for j in 0..CHANNEL_SYMBOL {
                let k = HEAD + m * CHANNEL_SYMBOL + j;
                let y = eq.step(self.pair(p0, w, k), SYNC_SCRAMBLER[k % 32], None);
                for (c, s) in score.iter_mut().enumerate() {
                    *s += (y * psk8(pat(c as u8, k)).conj()).re;
                }
            }
            *r = (0..8u8)
                .max_by(|&a, &b| score[usize::from(a)].total_cmp(&score[usize::from(b)]))
                .unwrap_or(0);
        }
        let (d1, d2) = (read[0], read[1]);
        let (rate, long) = Rate::from_d(d1, d2)?;
        let count = count_of([read[2], read[3], read[4]])?;
        if d1 < 4 || count > 23 || (!long && count > 2) {
            return None;
        }
        let mut pre = Vec::with_capacity((usize::from(count) + 1) * SEGMENT);
        for c in (0..=count).rev() {
            let syms = segment_symbols(d1, d2, c);
            pre.extend((0..SEGMENT).map(|k| {
                (
                    PATTERNS[usize::from(syms[k / CHANNEL_SYMBOL])][k % 8],
                    SYNC_SCRAMBLER[k % 32],
                )
            }));
        }
        let mut dfe = eq.with_points(rate.labels());
        for _ in 0..2 {
            for (k, &(p, s)) in pre.iter().enumerate().take(SEGMENT) {
                dfe.step(self.pair(p0, w, k), s, Some(p));
            }
        }
        let order = fetch_order(rate, long);
        let mut viterbi = Viterbi::new(HFDL_K7, 64).ok()?;
        viterbi.reset_to(0);
        let points = rate.labels().iter().map(|&n| psk8(n)).collect();
        Some(Session {
            p0,
            w,
            rate,
            long,
            dfe,
            k: SEGMENT,
            data_start: pre.len(),
            pre,
            soft: vec![0.0; rate.block_bits(long)],
            order,
            sent: 0,
            sets: Vec::with_capacity(32),
            points,
            viterbi,
            decoded: Vec::new(),
            llrs: Vec::new(),
            eom: PatternWatch::new(EOM),
            bad_run: 0,
            cycle_err: 0.0,
            weak_sets: 0,
        })
    }

    /// Runs the session over the samples in; returns whether it ended.
    fn run(&mut self, s: &mut Session, out: &mut Outbox) -> Next {
        loop {
            let n = s.p0 + 2 * s.k as u64 + LAG as u64;
            if n >= self.end() {
                return Next::Wait;
            }
            let pair = self.pair(s.p0, s.w, s.k);
            let t = (s.p0 + 2 * s.k as u64) as f64 / RATE;
            if s.k < s.data_start {
                let (p, scr) = s.pre[s.k];
                s.dfe.step(pair, scr, Some(p));
                s.k += 1;
                continue;
            }
            let d = s.k - s.data_start;
            let scr = self.scr[d % 160];
            let nsym = s.rate.block_symbols(s.long);
            let pos = d % nsym;
            s.k += 1;
            if s.rate == Rate::B75 {
                let y = s.dfe.step(pair, scr, None);
                s.sets.push(y);
                if s.sets.len() == 32 {
                    if self.set_done(s, pos) {
                        self.board
                            .event(out, t, "signal lost (the sets no longer fit)");
                        self.close(s, out, t);
                        return Next::Ended;
                    }
                    if s.sent == s.soft.len() && self.block_done(s, out, t) {
                        return Next::Ended;
                    }
                }
                continue;
            }
            let (u, kn) = s.rate.cycle();
            match known_at(s.rate, s.long, pos) {
                Some(kv) => {
                    let y = s.dfe.step(pair, scr, Some(kv));
                    s.cycle_err += (psk8(kv) - y).norm_sqr();
                    if pos % (u + kn) == u + kn - 1 {
                        if s.cycle_err / kn as f32 > 0.9 {
                            s.bad_run += 1;
                        } else {
                            s.bad_run = 0;
                        }
                        s.cycle_err = 0.0;
                        if s.bad_run > LOST_CYCLES {
                            self.board
                                .event(out, t, "signal lost (probes no longer fit)");
                            self.close(s, out, t);
                            return Next::Ended;
                        }
                    }
                }
                None => {
                    let y = s.dfe.step(pair, scr, None);
                    let bits = s.rate.bits();
                    let mut v = [0.0f32; 3];
                    max_log(y, &s.points, bits, s.dfe.sigma2(), &mut v);
                    if s.order.is_none() {
                        for &x in &v[..bits] {
                            if self.bit(s, out, t, u8::from(x < 0.0)) {
                                return Next::Ended;
                            }
                        }
                        if pos == nsym - 1 {
                            self.board.blocks += 1;
                            self.measure(s);
                        }
                        continue;
                    }
                    for &x in &v[..bits] {
                        if let Some(o) = &s.order {
                            let i = o[s.sent] as usize;
                            s.soft[i] = x;
                        }
                        s.sent += 1;
                    }
                    if s.sent == s.soft.len() && self.block_done(s, out, t) {
                        return Next::Ended;
                    }
                }
            }
        }
    }

    /// A 75 bit/s set is in: its two soft bits. True when the sets have been weak too long.
    fn set_done(&mut self, s: &mut Session, pos: usize) -> bool {
        let exc = pos / 32 == s.rate.block_symbols(s.long) / 32 - 1;
        let mut e = [0.0f32; 4];
        let p: f32 = s.sets.iter().map(|y| y.norm_sqr()).sum();
        for (set, ei) in e.iter_mut().enumerate() {
            let mut acc = Complex32::default();
            for (j, y) in s.sets.iter().enumerate() {
                acc += *y * psk8(set_tribit(set, exc, j)).conj();
            }
            *ei = acc.norm_sqr();
        }
        s.sets.clear();
        let max = |a: usize, b: usize| e[a].max(e[b]);
        let scale = 1.0 / (32.0 * s.dfe.sigma2());
        // Sets 0 and 1 send a first bit 0; sets 0 and 3 a second bit 0.
        let v = [
            (max(0, 1) - max(3, 2)) * scale,
            (max(0, 3) - max(1, 2)) * scale,
        ];
        if let Some(o) = &s.order {
            for x in v {
                s.soft[o[s.sent] as usize] = x;
                s.sent += 1;
            }
        }
        let best = e.iter().copied().fold(0.0f32, f32::max);
        if best < 0.2 * 32.0 * p {
            s.weak_sets += 1;
        } else {
            s.weak_sets = 0;
        }
        s.weak_sets > 45
    }

    /// An interleaver block is in: deinterleaved, the repeats added, decoded. True when the message ended.
    fn block_done(&mut self, s: &mut Session, out: &mut Outbox, t: f64) -> bool {
        self.board.blocks += 1;
        self.measure(s);
        let rep = s.rate.repetition();
        s.llrs.clear();
        for g in s.soft.chunks(2 * rep) {
            let (mut a, mut b) = (0.0f32, 0.0f32);
            for p in g.chunks(2) {
                a += p[0];
                b += p[1];
            }
            s.llrs.push(llr8(a, 2.0));
            s.llrs.push(llr8(b, 2.0));
        }
        s.sent = 0;
        s.decoded.clear();
        s.viterbi.push(&s.llrs, &mut s.decoded);
        let bits = std::mem::take(&mut s.decoded);
        let mut ended = false;
        for &b in &bits {
            if self.bit(s, out, t, b) {
                ended = true;
                break;
            }
        }
        s.decoded = bits;
        ended
    }

    /// The SNR and the frequency error of the session, at the end of a block.
    fn measure(&mut self, s: &Session) {
        self.board.snr_db = Some(f64::from(s.dfe.snr_db()));
        self.board.offset_hz =
            Some(s.w * RATE / TAU + s.dfe.loop_freq() * crate::serial::BAUD / TAU);
    }

    /// A decoded bit; true when it completes the end-of-message pattern (the message is then reported).
    fn bit(&mut self, s: &mut Session, out: &mut Outbox, t: f64, b: u8) -> bool {
        self.board.bit(out, t, b);
        if s.eom.push(b) {
            self.board.end_message(out, t, 4);
            self.board.event(out, t, "end of message");
            self.search_at = s.p0 + 2 * s.k as u64;
            return true;
        }
        false
    }

    /// Ends a transmission without its end-of-message pattern.
    fn close(&mut self, s: &Session, out: &mut Outbox, t: f64) {
        if !self.board.message().is_empty() {
            self.board.end_message(out, t, 0);
            self.board.event(out, t, "transmission ended");
        }
        self.search_at = s.p0 + 2 * s.k as u64;
    }

    fn trim(&mut self) {
        let keep = match &self.state {
            State::Search => self.search_at,
            State::Session(s) => s.p0 + 2 * s.k as u64,
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

impl sdrkit_graph::Decoder for Ms110Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.ms110"
    }
    fn outputs(&self) -> Vec<Port> {
        outputs()
    }
    fn values(&self) -> Values {
        no_values()
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
            let t = (s.p0 + 2 * s.k as u64) as f64 / RATE;
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
                    .set_state(st, format_args!("110: searching, {n} messages"));
            }
            State::Session(s) => {
                let st = if locked {
                    LockState::Locked
                } else {
                    LockState::Tracking
                };
                let name = s.rate.name(s.long);
                self.board.set_state(st, format_args!("110: {name}"));
            }
        }
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
    }
}
