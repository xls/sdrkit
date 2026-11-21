//! The high-rate serial-tone receiver: the 24 kHz channel through the matched filter (10 samples a symbol), a search for the
//! 103-symbol block by the products of neighbouring known symbols, timing, frequency and the D values from the block, channel
//! estimates of 16 taps from each 31-symbol probe, an MMSE decision-feedback equaliser designed every 16 symbols from the channel
//! interpolated between two probes, max-log soft bits, the deinterleaver and the tail-biting Viterbi decoder.

use super::signal::{
    block, block_bits, block_input, from_d, mini_probe, name, places, points, scrambler, symbol,
    tribits, Rate, BARKER, BLOCK, FRAME, PROBE, PROBE_LEN, SUPERFRAME, SYNC, SYNC_LEN,
};
use super::{S4539RxSetting, EQUALISER};
use crate::serial::board::{formats, outputs, Board, Layout};
use crate::serial::front::{cis, SerialFront, RATE_IN};
use crate::serial::soft::max_log;
use crate::serial::{psk8, PatternWatch, EOM};
use crate::typed::{S4539Event, S4539MessageEvent, S4539_EVENT, S4539_MESSAGE};
use sdrkit_algo_fec::conv::{codes::HFDL_K7, Viterbi};
use sdrkit_algo_modem::mmse_dfe::MmseDfe;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Complex32, ConfigError, LockState, Schema, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::ops::{Index, IndexMut};
use std::sync::Arc;

/// Samples a symbol.
const SPS: u64 = 10;
/// Symbols of a superframe.
const SF: usize = BLOCK + SUPERFRAME * (FRAME + PROBE_LEN) - PROBE_LEN;
/// The first lag of a channel estimate.
const L0: i32 = -6;
/// Taps of a channel estimate.
const TAPS: usize = 16;
/// Feed-forward taps.
const NF: usize = 21;
/// The symbol distance of the products of the coarse frequency estimate.
const COARSE_LAG: usize = 4;
/// Symbols a session keeps behind the newest one: a frame and two probes, more than the equaliser, the channel estimates and
/// the D values of a reinserted block look back.
const KEEP: usize = FRAME + 2 * PROBE_LEN + 64;

/// The symbols of a session from symbol `first` on, indexed by their number from the session start; older ones are dropped.
struct Tail {
    first: usize,
    v: Vec<Complex32>,
}

impl Tail {
    fn new(v: Vec<Complex32>) -> Tail {
        Tail { first: 0, v }
    }

    /// The number of the next symbol (the symbols seen so far).
    fn count(&self) -> usize {
        self.first + self.v.len()
    }

    fn push(&mut self, x: Complex32) {
        self.v.push(x);
    }

    fn get(&self, k: usize) -> Option<&Complex32> {
        k.checked_sub(self.first).and_then(|i| self.v.get(i))
    }

    /// Drops the symbols more than [`KEEP`] behind the newest, once twice that many are held.
    fn forget(&mut self) {
        if self.v.len() >= 2 * KEEP {
            let n = self.v.len() - KEEP;
            self.v.drain(..n);
            self.first += n;
        }
    }
}

impl Index<usize> for Tail {
    type Output = Complex32;
    fn index(&self, k: usize) -> &Complex32 {
        &self.v[k - self.first]
    }
}

impl IndexMut<usize> for Tail {
    fn index_mut(&mut self, k: usize) -> &mut Complex32 {
        &mut self.v[k - self.first]
    }
}

/// The results of the stack.
pub static LAYOUT: Layout = Layout {
    events: |d| d.event(&S4539_EVENT).event(&S4539_MESSAGE),
    event: |time, hz, text| S4539_EVENT.event(time, hz, &S4539Event { text }),
    message: |time, hz, text, bytes| {
        S4539_MESSAGE.event(time, hz, &S4539MessageEvent { text, bytes })
    },
};

/// What no public document confirms.
pub const NOTE_TEXT: &str = "The waveform follows MIL-STD-188-110C Appendix C. Not confirmed by a public document: the STANAG \
5066 D_PDU layout and its two CRCs that the D_PDU table reads. The scrambler follows Figure C-5 as drawn; no recording has \
confirmed its first values. Acquisition from the mini-probes alone is not done. The acquisition range is +-96 Hz.";

/// The block's symbols `k` whose product with symbol `k - d` needs no D value: both within the probes and their neighbours
/// (symbols 0 to 31 and 71 to 102), or both within one D group.
fn pair_set(d: usize) -> Vec<usize> {
    let segs = [(0, 31), (71, BLOCK - 1), (32, 44), (45, 57), (58, 70)];
    segs.iter().flat_map(|&(a, b)| a + d..=b).collect()
}

/// A channel estimate from a probe.
#[derive(Clone, Copy, Debug)]
struct Probe {
    h: [Complex32; TAPS],
    /// The symbol the estimate belongs to.
    at: usize,
    energy: f32,
}

fn estimate<Y: Index<usize, Output = Complex32> + ?Sized>(
    ys: &Y,
    q: usize,
    probe: &[Complex32],
) -> Probe {
    let mut h = [Complex32::default(); TAPS];
    for (t, ht) in h.iter_mut().enumerate() {
        let l = L0 + t as i32;
        let mut acc = Complex32::default();
        for m in (L0 + 15)..=(L0 + 30) {
            acc += ys[q + m as usize] * probe[(m - l) as usize].conj();
        }
        *ht = acc / 16.0;
    }
    Probe {
        h,
        at: q + 16,
        energy: h.iter().map(|v| v.norm_sqr()).sum(),
    }
}

struct Session {
    p: u64,
    w: f64,
    phi: f64,
    rate: Rate,
    frames: usize,
    next: Option<(Rate, usize)>,
    /// Symbols read, numbered from the first block's first symbol.
    ys: Tail,
    /// The symbols sent as far as known (known symbols and decisions), numbered as `ys`.
    a: Tail,
    prev: Option<Probe>,
    soft: Vec<f32>,
    frames_in: usize,
    noise: f32,
    mean_energy: f32,
    good: usize,
    bad_run: usize,
    block_bad: usize,
    block_probes: usize,
    eom: PatternWatch,
    viterbi: Viterbi,
    pts: Vec<Complex32>,
    scr: Vec<u16>,
    places: Option<Vec<u32>>,
    blk: Vec<u8>,
}

enum State {
    Search,
    Session(Box<Session>),
}

enum Next {
    Wait,
    Ended,
}

/// The receiver.
pub struct S4539Decoder {
    front: SerialFront,
    buf: Vec<Complex32>,
    base: u64,
    scratch: Vec<Complex32>,
    state: State,
    search_at: u64,
    board: Board,
    eq: bool,
    pairs: Vec<usize>,
    refs: Vec<Complex32>,
    pairs4: Vec<usize>,
    refs4: Vec<Complex32>,
    probe: Vec<Complex32>,
    sync: Vec<Complex32>,
}

impl Default for S4539Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl S4539Decoder {
    /// A receiver with the equaliser on.
    pub fn new() -> Self {
        // The products of the block's known symbols that need no D value do not depend on the D values.
        let b: Vec<Complex32> = block(Rate::B3200, 1).iter().map(|&t| psk8(t)).collect();
        let pairs = pair_set(1);
        let refs = pairs.iter().map(|&k| b[k] * b[k - 1].conj()).collect();
        let pairs4 = pair_set(COARSE_LAG);
        let refs4 = pairs4
            .iter()
            .map(|&k| b[k] * b[k - COARSE_LAG].conj())
            .collect();
        S4539Decoder {
            front: SerialFront::new(1),
            buf: Vec::new(),
            base: 0,
            scratch: Vec::new(),
            state: State::Search,
            search_at: 0,
            board: Board::new(&LAYOUT),
            eq: true,
            pairs,
            refs,
            pairs4,
            refs4,
            probe: tribits(PROBE).iter().map(|&t| psk8(t)).collect(),
            sync: tribits(SYNC).iter().map(|&t| psk8(t)).collect(),
        }
    }

    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    fn at(&self, n: u64) -> Complex32 {
        self.buf
            .get(n.wrapping_sub(self.base) as usize)
            .copied()
            .unwrap_or_default()
    }

    /// The block metric at sample `p` (block symbol 0), and whether the block's four quarters hold like power.
    fn metric(&self, p: u64) -> (f32, Complex32, bool) {
        let mut acc = Complex32::default();
        let mut mag = 0.0f32;
        for (&k, r) in self.pairs.iter().zip(&self.refs) {
            let a = self.at(p + SPS * k as u64);
            let b = self.at(p + SPS * (k as u64 - 1));
            let pr = a * b.conj();
            mag += pr.norm();
            acc += pr * r.conj();
        }
        let m = acc.norm() / mag.max(1e-20);
        let mut q = [0.0f32; 4];
        for k in 0..BLOCK - 3 {
            q[k * 4 / (BLOCK - 3)] += self.at(p + SPS * k as u64).norm_sqr();
        }
        let (lo, hi) = q
            .iter()
            .fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
        (m, acc, hi <= 2.0 * lo)
    }

    fn search(&mut self, out: &mut Outbox) -> bool {
        let span = SPS * BLOCK as u64;
        let best_of = SPS * 110;
        let need = best_of + span + SPS * 4 + 16;
        let thr = if self.eq { 0.35 } else { 0.5 };
        self.search_at = self.search_at.max(self.base + SPS * (SYNC_LEN as u64 + 2));
        while self.search_at + span + 16 <= self.end() {
            let p = self.search_at;
            let (m, _, even) = self.metric(p);
            if m < thr || !even {
                self.search_at += 1;
                continue;
            }
            if p + need > self.end() {
                return false;
            }
            let mut best = (p, m);
            for q in p..p + best_of {
                let (m, _, even) = self.metric(q);
                if even && m > best.1 {
                    best = (q, m);
                }
            }
            match self.lock(best.0) {
                Some((s, joined, f_hz, snr)) => {
                    let what = if joined {
                        "joined at a reinserted preamble"
                    } else {
                        "transmission start"
                    };
                    let text = format!(
                        "{what}: {}, frequency error {f_hz:+.1} Hz, SNR {snr:.0} dB",
                        name(s.rate, s.frames)
                    );
                    self.board.event(out, best.0 as f64 / RATE_IN, &text);
                    self.board.waveform = name(s.rate, s.frames);
                    self.board.start_message();
                    self.state = State::Session(Box::new(s));
                    return true;
                }
                None => self.search_at = best.0 + SPS,
            }
        }
        false
    }

    /// The block's symbols from sample `p`, turned back by `w` radians a sample.
    fn block_symbols(&self, p: u64, w: f64) -> Vec<Complex32> {
        (0..BLOCK)
            .map(|k| self.at(p + SPS * k as u64) * cis(-w * (SPS * k as u64) as f64))
            .collect()
    }

    /// The energy of the two probes' coherent sums at sample `p` (one tap), for the timing.
    fn probe_power(&self, p: u64, w: f64) -> f32 {
        let ys = self.block_symbols(p, w);
        let mut a = Complex32::default();
        let mut b = Complex32::default();
        for (m, s) in self.probe.iter().enumerate() {
            a += ys[m] * s.conj();
            b -= ys[72 + m] * s.conj();
        }
        a.norm_sqr() + b.norm_sqr()
    }

    /// The coarse frequency of the block at `p`, radians a sample: the products of the D-free known symbols 4 apart,
    /// unambiguous within +-300 Hz; a multipath bias of up to 45 degrees moves them less than 75 Hz.
    fn coarse(&self, p: u64) -> f64 {
        let mut acc = Complex32::default();
        for (&k, r) in self.pairs4.iter().zip(&self.refs4) {
            let a = self.at(p + SPS * k as u64);
            let b = self.at(p + SPS * (k - COARSE_LAG) as u64);
            acc += a * b.conj() * r.conj();
        }
        f64::from(acc.arg()) / (COARSE_LAG as f64 * SPS as f64)
    }

    /// Reads the block at `p`; `None` when it is not one. The channel comes from the two probes (16 taps each), the frequency
    /// from their turn, the D values from the best fit of the 31 valid combinations over the symbols between them.
    fn lock(&self, p: u64) -> Option<(Session, bool, f64, f64)> {
        // Timing: the best one-tap fit of the probes over +-5 samples, at the coarse frequency.
        let wc = self.coarse(p);
        let mut best = (p, 0.0f32);
        for d in 0..=10u64 {
            let q = p + d - 5;
            let e = self.probe_power(q, wc);
            if e > best.1 {
                best = (q, e);
            }
        }
        let p = best.0;
        let neg: Vec<Complex32> = self.probe.iter().map(|s| -s).collect();
        let wc = self.coarse(p);
        // Fine: the probes repeat after 16 symbols; their products 16 symbols apart, after the coarse turn is taken out, give the
        // rest without ambiguity within +-75 Hz (the products of neighbouring symbols are biased by multipath).
        let mut d16 = Complex32::default();
        for k0 in [0u64, 72] {
            for k in 6..=14u64 {
                let a = self.at(p + SPS * (k0 + k));
                let b = self.at(p + SPS * (k0 + k + 16));
                d16 += b * a.conj();
            }
        }
        let rest = (f64::from(d16.arg()) - 16.0 * SPS as f64 * wc + std::f64::consts::PI)
            .rem_euclid(TAU)
            - std::f64::consts::PI;
        let mut w = wc + rest / (16.0 * SPS as f64);
        let mut ys = self.block_symbols(p, w);
        for _ in 0..3 {
            let a = estimate(&ys, 0, &self.probe);
            let b = estimate(&ys, 72, &neg);
            let mut num = Complex32::default();
            for (x, y) in b.h.iter().zip(&a.h) {
                num += x * y.conj();
            }
            w += f64::from(num.arg()) / ((b.at - a.at) as f64 * SPS as f64);
            ys = self.block_symbols(p, w);
        }
        let ha = estimate(&ys, 0, &self.probe);
        let hb = estimate(&ys, 72, &neg);
        let mut num = Complex32::default();
        for (x, y) in ha.h.iter().zip(&hb.h) {
            num += x * y.conj();
        }
        let rho = num.norm() / (ha.energy * hb.energy).sqrt().max(1e-20);
        if self.eq && rho < 0.6 {
            return None;
        }
        // The D values: the valid combination whose block the channel fits best.
        let mut fit = (f32::MAX, Rate::B3200, 1usize);
        for r in Rate::ALL {
            for f in super::signal::FRAMES {
                if !super::signal::valid(r, f) {
                    continue;
                }
                let a: Vec<Complex32> = block(r, f).iter().map(|&t| psk8(t)).collect();
                let mut err = 0.0f32;
                for (k, yk) in ys.iter().enumerate().take(97).skip(9) {
                    let x = (k as f32 - ha.at as f32) / (hb.at - ha.at) as f32;
                    let mut pred = Complex32::default();
                    for i in 0..TAPS {
                        let q = k as i32 - (L0 + i as i32);
                        pred += (ha.h[i] * (1.0 - x) + hb.h[i] * x) * a[q as usize];
                    }
                    err += (yk - pred).norm_sqr();
                }
                if err < fit.0 {
                    fit = (err, r, f);
                }
            }
        }
        let (err, rate, frames) = fit;
        let noise = (err / 88.0).max(1e-12);
        let snr = 10.0 * f64::from(0.5 * (ha.energy + hb.energy) / noise).log10();
        if snr < if self.eq { -8.0 } else { 0.0 } {
            return None;
        }
        // A start when the sync sequence comes before the block: its fit with the first probe's channel.
        let mut cs = Complex32::default();
        let (mut ey, mut ep) = (0.0f32, 0.0f32);
        for k in 10..SYNC_LEN {
            let back = (SYNC_LEN - k) as u64 * SPS;
            let y = self.at(p - back) * cis(w * back as f64);
            let mut pred = Complex32::default();
            for i in 0..TAPS {
                let q = k as i32 - (L0 + i as i32);
                if (0..SYNC_LEN as i32).contains(&q) {
                    pred += ha.h[i] * self.sync[q as usize];
                }
            }
            cs += y * pred.conj();
            ey += y.norm_sqr();
            ep += pred.norm_sqr();
        }
        let sync_c = cs.norm() / (ey * ep).sqrt().max(1e-20);
        let joined = sync_c < if self.eq { 0.35 } else { 0.5 };
        let blk = block(rate, frames);
        let mut a: Vec<Complex32> = blk.iter().map(|&t| psk8(t)).collect();
        a.reserve(2 * KEEP + FRAME);
        let phi = (w * (SPS * BLOCK as u64) as f64).rem_euclid(TAU);
        let mut ys_all = ys;
        ys_all.reserve(2 * KEEP + FRAME);
        let s = Session {
            p,
            w,
            phi,
            rate,
            frames,
            next: None,
            ys: Tail::new(ys_all),
            a: Tail::new(a),
            prev: Some(hb),
            soft: Vec::with_capacity(block_bits(rate, frames)),
            frames_in: 0,
            noise,
            mean_energy: 0.5 * (ha.energy + hb.energy),
            good: 0,
            bad_run: 0,
            block_bad: 0,
            block_probes: 0,
            eom: PatternWatch::new(EOM),
            viterbi: Viterbi::new(HFDL_K7, block_input(rate, frames) + 8).ok()?,
            pts: points(rate),
            scr: scrambler(rate.scrambler_bits()),
            places: places(rate, frames),
            blk,
        };
        let f_hz = w * RATE_IN / TAU;
        Some((s, joined, f_hz, snr))
    }

    /// The probe symbols (as points) that end at superframe position `pos`.
    fn probe_points(&self, s: &Session, frame: usize) -> Vec<Complex32> {
        if frame == SUPERFRAME {
            self.probe.clone()
        } else {
            mini_probe(s.rate, s.frames, frame)
                .iter()
                .map(|&t| psk8(t))
                .collect()
        }
    }

    fn run(&mut self, s: &mut Session, out: &mut Outbox) -> Next {
        loop {
            let k = s.ys.count();
            let n = s.p + SPS * k as u64;
            if n >= self.end() {
                return Next::Wait;
            }
            s.ys.push(self.at(n) * cis(-s.phi));
            s.phi = (s.phi + s.w * SPS as f64).rem_euclid(TAU);
            let pos = k % SF;
            let t = n as f64 / RATE_IN;
            if pos < BLOCK {
                // A reinserted block: "+" probe (the probe after frame 72), the D groups, the "-" probe.
                let v = s.blk[pos];
                s.a.push(psk8(v));
                if pos == PROBE_LEN - 1 {
                    let pr = estimate(&s.ys, k + 1 - PROBE_LEN, &self.probe);
                    if let Some(r) = self.frame_done(s, out, t, pr) {
                        return r;
                    }
                }
                if pos == 71 {
                    self.read_block(s, out, t, k - 71);
                }
                if pos == BLOCK - 1 {
                    let neg: Vec<Complex32> = self.probe.iter().map(|x| -x).collect();
                    s.prev = Some(estimate(&s.ys, k + 1 - PROBE_LEN, &neg));
                    if let Some(nx) = s.next.take() {
                        self.change(s, nx);
                    }
                }
                continue;
            }
            let r = pos - BLOCK;
            let f = r / (FRAME + PROBE_LEN) + 1;
            let i = r % (FRAME + PROBE_LEN);
            if i < FRAME {
                s.a.push(Complex32::default());
                continue;
            }
            let pts = self.probe_points(s, f);
            s.a.push(pts[i - FRAME]);
            if i == FRAME + PROBE_LEN - 1 {
                let pr = estimate(&s.ys, k + 1 - PROBE_LEN, &pts);
                if let Some(r) = self.frame_done(s, out, t, pr) {
                    return r;
                }
            }
        }
    }

    /// Reads the D values of a reinserted block whose first symbol is `k0`; a new rate takes effect after it.
    fn read_block(&mut self, s: &mut Session, out: &mut Outbox, t: f64, k0: usize) {
        if k0 == 0 {
            return;
        }
        let Some(pa) = s.prev else { return };
        let main =
            pa.h.iter()
                .copied()
                .max_by(|a, b| a.norm_sqr().total_cmp(&b.norm_sqr()))
                .unwrap_or_default();
        let lag =
            pa.h.iter()
                .position(|v| *v == main)
                .map_or(0, |i| i as i32 + L0);
        let mut d = [0u8; 3];
        for (g, dv) in d.iter_mut().enumerate() {
            let mut c = Complex32::default();
            for (j, &b) in BARKER.iter().enumerate() {
                let k = (k0 + 32 + 13 * g + j) as i32 + lag;
                if let Some(y) = s.ys.get(k as usize) {
                    c += y * psk8(b).conj();
                }
            }
            let turns = ((c / main).arg() as f64 / FRAC_PI_2).round() as i64;
            *dv = (turns.rem_euclid(4) * 2) as u8;
        }
        if let Some((r, f)) = from_d(d) {
            if (r, f) != (s.rate, s.frames) {
                s.next = Some((r, f));
                self.board
                    .event(out, t, "the rate changed at a reinserted preamble");
            }
        }
    }

    fn change(&mut self, s: &mut Session, (rate, frames): (Rate, usize)) {
        s.rate = rate;
        s.frames = frames;
        s.blk = block(rate, frames);
        s.pts = points(rate);
        s.scr = scrambler(rate.scrambler_bits());
        s.places = places(rate, frames);
        s.soft.clear();
        s.frames_in = 0;
        if let Ok(v) = Viterbi::new(HFDL_K7, block_input(rate, frames) + 8) {
            s.viterbi = v;
        }
        self.board.waveform = name(rate, frames);
    }

    /// The probe after a frame is in: the frame is equalised, and a block that is complete is decoded.
    fn frame_done(&mut self, s: &mut Session, out: &mut Outbox, t: f64, pb: Probe) -> Option<Next> {
        let Some(pa) = s.prev else {
            s.prev = Some(pb);
            return None;
        };
        // The probe's quality: energy against the probes seen, and its likeness to the last one.
        let mut num = Complex32::default();
        for (x, y) in pb.h.iter().zip(&pa.h) {
            num += x * y.conj();
        }
        let rho = num.norm() / (pa.energy * pb.energy).sqrt().max(1e-20);
        let good = pb.energy > 0.1 * s.mean_energy.max(4.0 * s.noise) && rho > 0.3;
        if good {
            s.mean_energy = if s.good == 0 {
                pb.energy
            } else {
                0.9 * s.mean_energy + 0.1 * pb.energy
            };
        }
        s.block_probes += 1;
        if good {
            s.good += 1;
            s.bad_run = 0;
            if rho > 0.5 {
                let turn = f64::from(num.arg()) / ((pb.at - pa.at) as f64 * SPS as f64);
                s.w += 0.15 * turn;
            }
        } else {
            s.bad_run += 1;
            s.block_bad += 1;
            let limit = if s.good == 0 {
                3
            } else if s.good < 4 {
                6
            } else {
                24
            };
            if s.bad_run >= limit {
                let msg = if s.good == 0 {
                    "false start dropped"
                } else {
                    "signal lost"
                };
                self.board.event(out, t, msg);
                self.close(s, out, t);
                return Some(Next::Ended);
            }
        }
        let first = pa.at + PROBE_LEN - 16;
        self.equalise(s, first, &pa, &pb);
        s.ys.forget();
        s.a.forget();
        s.prev = Some(pb);
        s.frames_in += 1;
        if s.frames_in == s.frames {
            s.frames_in = 0;
            let deliver = s.block_bad * 2 <= s.block_probes;
            s.block_bad = 0;
            s.block_probes = 0;
            if deliver && self.decode_block(s, out, t) {
                return Some(Next::Ended);
            }
            s.soft.clear();
        }
        None
    }

    /// Equalises the 256 data symbols from `k0`, between the probes `pa` and `pb`.
    fn equalise(&mut self, s: &mut Session, k0: usize, pa: &Probe, pb: &Probe) {
        let mut num = Complex32::default();
        for (x, y) in pb.h.iter().zip(&pa.h) {
            num += x * y.conj();
        }
        let theta = num.arg();
        let span = (pb.at - pa.at) as f32;
        let bits = s.rate.bits();
        let m = 1usize << bits;
        let mut cand = vec![Complex32::default(); m];
        let mut zs = [Complex32::default(); 16];
        let mut noise_acc = 0.0f32;
        let mut dec_acc = 0.0f32;
        for blk in 0..FRAME / 16 {
            let kc = k0 + blk * 16 + 8;
            let x = (kc - pa.at) as f32 / span;
            let rot_b = cis(-f64::from(theta));
            let rot = cis(f64::from(theta * x));
            let mut h = [Complex32::default(); TAPS];
            for (i, hv) in h.iter_mut().enumerate() {
                *hv = (pa.h[i] * (1.0 - x) + pb.h[i] * rot_b * x) * rot;
            }
            let (hh, lag0, first, nf): (Vec<Complex32>, i32, i32, usize) = if self.eq {
                let max = h.iter().map(|v| v.norm_sqr()).fold(0.0f32, f32::max);
                let fp = h
                    .iter()
                    .position(|v| v.norm_sqr() >= 0.1 * max)
                    .unwrap_or(0) as i32
                    + L0;
                (h.to_vec(), L0, fp - 8, NF)
            } else {
                let i = h
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.norm_sqr().total_cmp(&b.1.norm_sqr()))
                    .map_or(0, |(i, _)| i);
                (vec![h[i]], L0 + i as i32, L0 + i as i32, 1)
            };
            let Ok(d) = MmseDfe::design(&hh, lag0, first, nf, s.noise) else {
                // No design: the symbols are erased.
                s.soft.extend(std::iter::repeat_n(0.0, 16 * bits));
                continue;
            };
            let nb = d.feedback().len();
            let mut past = vec![Complex32::default(); nb];
            let mut win = vec![Complex32::default(); nf];
            let mut derr = 0.0f32;
            for (j, z) in zs.iter_mut().enumerate() {
                let k = k0 + blk * 16 + j;
                for (i, w) in win.iter_mut().enumerate() {
                    let idx = k as i32 + first + i as i32;
                    *w = if idx >= 0 {
                        s.ys.get(idx as usize).copied().unwrap_or_default()
                    } else {
                        Complex32::default()
                    };
                }
                for (i, pv) in past.iter_mut().enumerate() {
                    *pv = k
                        .checked_sub(i + 1)
                        .and_then(|q| s.a.get(q))
                        .copied()
                        .unwrap_or_default();
                }
                *z = d.output(&win, &past);
                let sc = s.scr[k - k0];
                for (l, c) in cand.iter_mut().enumerate() {
                    *c = symbol(s.rate, &s.pts, l, sc);
                }
                let best = cand
                    .iter()
                    .min_by(|a, b| (*z - **a).norm_sqr().total_cmp(&(*z - **b).norm_sqr()))
                    .copied()
                    .unwrap_or_default();
                derr += (*z - best).norm_sqr();
                s.a[k] = best;
            }
            dec_acc += derr;
            let var = d.expected_noise().max(1.5 * derr / 16.0).max(1e-3);
            let mut v = [0.0f32; 8];
            for (j, z) in zs.iter().enumerate() {
                let k = k0 + blk * 16 + j;
                let sc = s.scr[k - k0];
                for (l, c) in cand.iter_mut().enumerate() {
                    *c = symbol(s.rate, &s.pts, l, sc);
                }
                max_log(*z, &cand, bits, var, &mut v);
                s.soft.extend_from_slice(&v[..bits]);
            }
            // The noise from the residual of the channel with the decisions.
            for j in 0..16 {
                let k = k0 + blk * 16 + j;
                let mut pred = Complex32::default();
                for (i, hv) in h.iter().enumerate() {
                    let q = k as i32 - (L0 + i as i32);
                    if let Some(a) = s.a.get(q as usize) {
                        pred += hv * a;
                    }
                }
                noise_acc += (s.ys[k] - pred).norm_sqr();
            }
        }
        let fresh = (noise_acc / FRAME as f32).min(dec_acc / FRAME as f32 * pb.energy);
        if fresh.is_finite() && fresh > 0.0 {
            s.noise = 0.7 * s.noise + 0.3 * fresh;
        }
        self.board.offset_hz = Some(s.w * RATE_IN / TAU);
        let e: f32 = pb.energy;
        self.board.snr_db = Some(10.0 * f64::from((e / s.noise.max(1e-12)).max(1e-6)).log10());
    }

    /// Decodes a complete interleaver block; true when the message ended.
    fn decode_block(&mut self, s: &mut Session, out: &mut Outbox, t: f64) -> bool {
        self.board.blocks += 1;
        let Some(bits) = super::signal::decode_block(&mut s.viterbi, s.places.as_deref(), &s.soft)
        else {
            return false;
        };
        for b in bits {
            self.board.bit(out, t, b);
            if s.eom.push(b) {
                self.board.end_message(out, t, 4);
                self.board.event(out, t, "end of message");
                self.search_at = s.p + SPS * s.ys.count() as u64;
                return true;
            }
        }
        false
    }

    fn close(&mut self, s: &Session, out: &mut Outbox, t: f64) {
        if !self.board.message().is_empty() {
            self.board.end_message(out, t, 0);
        }
        self.search_at = s.p + SPS * s.ys.count() as u64;
    }

    fn trim(&mut self) {
        let keep = match &self.state {
            State::Search => self.search_at,
            State::Session(s) => s.p + SPS * s.ys.count() as u64,
        }
        .saturating_sub(SPS * (SYNC_LEN as u64 + 16));
        if keep > self.base + 240_000 {
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

impl sdrkit_graph::Decoder for S4539Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.s4539"
    }
    fn outputs(&self) -> Vec<Port> {
        outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        S4539RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(S4539RxSetting::Equaliser, Value::Bool(self.eq))])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match S4539RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S4539RxSetting::Equaliser => self.eq = EQUALISER.read(id, value)?,
        }
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
        self.board.saw(self.end() as f64 / RATE_IN);
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
            let t = (s.p + SPS * s.ys.count() as u64) as f64 / RATE_IN;
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
                    .set_state(st, format_args!("4539: searching, {n} messages"));
            }
            State::Session(s) => {
                let st = if locked {
                    LockState::Locked
                } else {
                    LockState::Tracking
                };
                let nm = name(s.rate, s.frames);
                self.board.set_state(st, format_args!("4539: {nm}"));
            }
        }
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::signal::{message_bits, transmission};
    use super::*;
    use crate::serial::gen::SymbolGen;
    use sdrkit_core::Generator;
    use sdrkit_graph::Decoder;

    /// The session, the soft bits and the sample buffer held, as (symbols, decisions, soft bits, samples) capacities.
    fn held(d: &S4539Decoder) -> Option<(u64, [usize; 4], usize)> {
        match &d.state {
            State::Session(s) => Some((
                s.p,
                [
                    s.ys.v.capacity(),
                    s.a.v.capacity(),
                    s.soft.capacity(),
                    d.buf.capacity(),
                ],
                s.ys.count(),
            )),
            State::Search => None,
        }
    }

    #[test]
    fn a_ten_minute_broadcast_holds_bounded_buffers() {
        let (rate, frames) = (Rate::B3200, 72);
        let bytes: Vec<u8> = crate::serial::TEST_TEXT
            .bytes()
            .cycle()
            .take(250_000)
            .collect();
        let mut sym = vec![Complex32::default(); 720];
        sym.extend(transmission(
            rate,
            frames,
            0,
            &message_bits(rate, frames, &bytes, false),
        ));
        assert!(sym.len() > 2400 * 600);
        let mut g = SymbolGen::serial(RATE_IN, sym);
        let mut d = S4539Decoder::new();
        let kinds: Vec<_> = formats().iter().map(|f| f.kind).collect();
        let mut out = Outbox::new(&kinds);
        let mut x = vec![Complex32::default(); RATE_IN as usize];
        let mut seen = Vec::new();
        for sec in 1..=600 {
            g.generate(&mut x);
            d.process(&x, 0, &mut out);
            out.messages(0).clear();
            if [120, 360, 600].contains(&sec) {
                seen.push(held(&d).expect("the session holds"));
            }
        }
        let (p, caps, _) = seen[0];
        for &(q, c, n) in &seen[1..] {
            assert_eq!(q, p, "one session from the start");
            assert_eq!(c, caps, "capacities at 2 minutes {caps:?}, later {c:?}");
            assert!(n > 2400 * 300);
        }
        let most = BLOCK + 2 * KEEP + FRAME;
        assert!(caps[0] <= most && caps[1] <= most, "{caps:?}");
        assert!(d.board.blocks >= 68, "{}", d.board.blocks);
        // The last block read is still right.
        let m = d.board.message();
        let tail = &m[m.len() - 2000..];
        let want = crate::serial::TEST_TEXT.repeat(40);
        assert!(want.as_bytes().windows(2000).any(|w| w == tail));
    }
}
