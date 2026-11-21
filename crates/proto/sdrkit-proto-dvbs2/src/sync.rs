//! Frame hunting and following: the DVB-S2 PLHEADER hunt by differential correlation (of one header, or at a low Es/N0
//! of the headers of several frames of one length), the spectrum sense from the SOF, the
//! PLS code by the correlation of the whole header, the carrier of QPSK and 8PSK from the fourth or eighth power and of the
//! constellations with rings from two headers and the data between them, frames followed by their lengths with
//! descrambling, pilot removal and phase refinement (by decisions, or for the constellations with rings by the pilots
//! where a frame has them); and the DVB-S hypotheses (four constellation variants, every code rate and puncturing phase)
//! with the QPSK carrier loop.

use super::modcod::{ModCod, Modulation};
use super::pl::{self, constellation, header_symbols, pl_scrambling, HEADER};
use crate::inner::{inner_decoder, PunctureRate};
use sdrkit_algo_dsp::fold::fold_complex;
use sdrkit_algo_fec::conv::Depuncturer;
use sdrkit_algo_modem::freq::PowerLine;
use sdrkit_core::Complex32;
use sdrkit_graph::SlicePool;
use sdrkit_proto_mpegts::outer::{sync_search, Dispersal, OuterDecoder};
use std::f64::consts::PI;
use std::sync::Arc;

/// One PLFRAME's data, carrier corrected, descrambled, pilots removed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct S2Frame {
    /// MODCOD (0: dummy).
    pub modcod: u8,
    /// Short frame.
    pub short: bool,
    /// Pilots.
    pub pilots: bool,
    /// The data symbols, from the sync stage's pool.
    pub syms: Arc<[Complex32]>,
    /// Es/N0 of the header, dB.
    pub esn0_db: f64,
    /// MER of the decided data, dB.
    pub mer_db: f64,
    /// Frames since the lock.
    pub index: u64,
    /// The first frame after a (re)lock.
    pub restart: bool,
}

impl sdrkit_graph::Item for S2Frame {
    const KIND: &'static str = "dvbs2.plframe";
}

/// Soft bits of a DVB-S stream (the punctured serial order), positive meaning 0.
#[derive(Clone, Debug, PartialEq)]
pub struct S1Soft {
    /// The soft bits.
    pub soft: Vec<i8>,
    /// Code rate.
    pub rate: PunctureRate,
    /// Puncturing phase of the first bit (on a restart).
    pub phase: usize,
    /// A new stream starts here.
    pub restart: bool,
}

impl sdrkit_graph::Item for S1Soft {
    const KIND: &'static str = "dvbs.soft";
}

/// Which standards the hunt tries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    /// Both.
    Auto,
    /// DVB-S only.
    S,
    /// DVB-S2 only.
    S2,
}

sdrkit_core::choice_enum! {
    /// The standard of the signal received.
    pub enum Received {
        /// DVB-S.
        Dvbs = ("dvbs", "DVB-S"),
        /// DVB-S2.
        Dvbs2 = ("dvbs2", "DVB-S2"),
    }
}

/// What the sync stage reports.
#[derive(Clone, Debug, Default)]
pub struct SyncReport {
    /// The standard received, if known.
    pub standard: Option<Received>,
    /// Locked onto frames or packets.
    pub locked: bool,
    /// The spectrum is inverted.
    pub inverted: bool,
    /// The last MODCOD (S2).
    pub modcod: u8,
    /// Short frame.
    pub short: bool,
    /// Pilots.
    pub pilots: bool,
    /// The MODCOD changed between frames (VCM/ACM).
    pub vcm: bool,
    /// Code rate (S).
    pub rate: Option<PunctureRate>,
    /// Carrier offset from the front end's mixer, cycles per symbol.
    pub freq_cps: f64,
    /// Es/N0, dB.
    pub esn0_db: Option<f64>,
    /// MER, dB.
    pub mer_db: Option<f64>,
    /// PLFRAMEs seen, of which dummy.
    pub frames: u64,
    /// Dummy frames.
    pub dummies: u64,
    /// Hunts that failed.
    pub failed_hunts: u64,
    /// Headers missed.
    pub misses: u64,
    /// Recent symbols for the picture.
    pub constellation: Vec<Complex32>,
    /// MER per frame.
    pub mer_history: Vec<f32>,
    /// Log lines of the stage, carried to the report.
    pub log_lines: Vec<String>,
}

enum Mode {
    Hunt,
    S2 {
        /// Absolute index of the next header.
        next: u64,
        f: f64,
        theta: f64,
        conj: bool,
        misses: u32,
        last_len: usize,
        /// The PLS code expected at the next header: that of the search, then that of the last frame.
        code: u8,
        index: u64,
        /// The amplitude of the symbols, for the constellations with rings.
        amp: f32,
    },
    S1 {
        variant: u8,
        rate: PunctureRate,
        theta: f64,
        nu: f64,
        f: f64,
    },
}

/// The sync stage.
pub struct Sync {
    /// Which standards to try.
    pub standard: Standard,
    /// PL scrambling code.
    pub gold: u32,
    buf: Vec<Complex32>,
    base: u64,
    mode: Mode,
    since_hunt: u64,
    since_restart: u64,
    /// Hunts failed in a row (for the rate dither).
    pub fails_in_row: u32,
    /// Measurements.
    pub report: SyncReport,
    /// Log lines.
    pub log: Vec<String>,
    scr: Vec<u8>,
    scr_len: usize,
    scr_gold: u32,
    cons: Vec<Complex32>,
    cons_of: Option<u8>,
    decide: Decide,
    pool: SlicePool<Complex32>,
    /// Scratch of the hunt: the differential products of the window and their magnitudes.
    diff: Vec<Complex32>,
    mag: Vec<f32>,
    /// Scratch of the hunt: the sum and the difference of the two header sums at every place.
    plus: Vec<Complex32>,
    minus: Vec<Complex32>,
    /// Every PLFRAME length, symbols.
    lengths: Vec<usize>,
    headers: Vec<[Complex32; HEADER]>,
    /// The PLS code of each output of [`Sync::decode_pls`]: by the last signalling bit, then by the sign (32 for
    /// negative) plus the index of the Walsh function.
    pls_map: [[u8; 64]; 2],
    sof_ref: Vec<Complex32>,
    pls_ref: Vec<Complex32>,
    /// Frames out.
    pub frames_out: Vec<S2Frame>,
    /// Soft bits out.
    pub soft_out: Vec<S1Soft>,
    first_modcod: Option<u8>,
    cpos: usize,
    /// Cap of the stored window.
    pub cap: usize,
}

/// Two headers one frame apart that a search found.
struct HeaderPair {
    /// The place and the PLS code of each.
    first: (usize, u8),
    second: (usize, u8),
    /// The spectrum is inverted.
    conj: bool,
    /// The carrier frequency within 1/360 of a cycle per symbol.
    coarse: f64,
}

fn rot(phase: f64) -> Complex32 {
    let (s, c) = phase.sin_cos();
    Complex32::new(c as f32, s as f32)
}

impl Sync {
    /// A sync stage.
    pub fn new(standard: Standard, gold: u32) -> Sync {
        let headers: Vec<[Complex32; HEADER]> = (0..128u8).map(header_symbols).collect();
        let h0 = &headers[0];
        let mut pls_map = [[0u8; 64]; 2];
        for (c, h) in headers.iter().enumerate() {
            // Whether PLS symbol `k` of this code is the negative of that of code 0.
            let negative = |k: usize| (h[26 + k] * h0[26 + k].conj()).re < 0.0;
            let repeat = usize::from(negative(1) != negative(0));
            let mut walsh = 0;
            for bit in 0..5 {
                if negative(2 << bit) != negative(0) {
                    walsh |= 1 << bit;
                }
            }
            pls_map[repeat][usize::from(negative(0)) << 5 | walsh] = c as u8;
        }
        let sof_ref: Vec<Complex32> = (1..26).map(|i| h0[i] * h0[i - 1].conj()).collect();
        let pls_ref: Vec<Complex32> = (0..32)
            .map(|m| h0[27 + 2 * m] * h0[26 + 2 * m].conj())
            .collect();
        let mut lengths: Vec<usize> = (0..128u8).filter_map(Self::frame_len).collect();
        lengths.sort_unstable();
        lengths.dedup();
        Sync {
            standard,
            gold,
            buf: Vec::new(),
            base: 0,
            mode: Mode::Hunt,
            since_hunt: 0,
            since_restart: 0,
            fails_in_row: 0,
            report: SyncReport::default(),
            log: Vec::new(),
            scr: Vec::new(),
            scr_len: 0,
            scr_gold: 0,
            cons: Vec::new(),
            cons_of: None,
            decide: Decide::Scan,
            pool: SlicePool::default(),
            diff: Vec::new(),
            mag: Vec::new(),
            plus: Vec::new(),
            minus: Vec::new(),
            lengths,
            headers,
            pls_map,
            sof_ref,
            pls_ref,
            frames_out: Vec::new(),
            soft_out: Vec::new(),
            first_modcod: None,
            cpos: 0,
            cap: 160_000,
        }
    }

    /// A new symbol stream (the front end restarted).
    pub fn restart(&mut self) {
        self.buf.clear();
        self.base = 0;
        self.mode = Mode::Hunt;
        self.since_hunt = 0;
        self.since_restart = 0;
        self.report.locked = false;
        self.report.standard = None;
        self.first_modcod = None;
        self.report.vcm = false;
    }
    /// The decoder lost the stream: hunt again from the symbols that come.
    pub fn lost(&mut self) {
        if !matches!(self.mode, Mode::Hunt) {
            self.log.push(
                if matches!(self.mode, Mode::S1 { .. }) {
                    "DVB-S: sync lost"
                } else {
                    "DVB-S2: sync lost"
                }
                .into(),
            );
        }
        self.mode = Mode::Hunt;
        self.report.locked = false;
        self.since_hunt = 0;
    }
    /// Frames are followed.
    pub fn following(&self) -> bool {
        matches!(self.mode, Mode::S2 { .. })
    }
    /// Locked to anything.
    pub fn locked(&self) -> bool {
        !matches!(self.mode, Mode::Hunt)
    }

    /// Takes symbols at one per symbol.
    pub fn push(&mut self, syms: &[Complex32]) {
        self.buf.extend_from_slice(syms);
        self.since_hunt += syms.len() as u64;
        self.since_restart += syms.len() as u64;
        match self.mode {
            Mode::Hunt => {
                let interval = 65_536u64 << self.fails_in_row.min(2);
                if self.buf.len() >= 49_152
                    && self.since_restart >= 20_000
                    && self.since_hunt >= interval.min(self.buf.len() as u64)
                {
                    self.since_hunt = 0;
                    self.hunt();
                }
            }
            Mode::S2 { .. } => self.follow_s2(),
            Mode::S1 { .. } => self.follow_s1(syms.len()),
        }
        // Bound the window.
        let keep = match self.mode {
            Mode::Hunt => self.cap,
            Mode::S2 { next, .. } => {
                (self.base + self.buf.len() as u64).saturating_sub(next) as usize + 16
            }
            Mode::S1 { .. } => 0,
        };
        if self.buf.len() > keep {
            let drop = self.buf.len() - keep;
            self.buf.drain(..drop);
            self.base += drop as u64;
        }
    }

    fn hunt(&mut self) {
        if self.standard != Standard::S {
            if let Some((pos, code, f, theta, conj)) = self.find_s2() {
                self.fails_in_row = 0;
                self.report.inverted = conj;
                self.report.standard = Some(Received::Dvbs2);
                self.report.locked = true;
                self.mode = Mode::S2 {
                    next: self.base + pos as u64,
                    f,
                    theta,
                    conj,
                    misses: 0,
                    last_len: 0,
                    code,
                    index: 0,
                    amp: 1.0,
                };
                self.follow_s2();
                return;
            }
        }
        if self.standard != Standard::S2 && self.find_s1() {
            self.fails_in_row = 0;
            return;
        }
        if self.since_hunt > 0 {
            // A near miss (see find_s1): the next hunt comes early.
            self.report.failed_hunts += 1;
            return;
        }
        self.fails_in_row += 1;
        self.report.failed_hunts += 1;
        if self.standard == Standard::S {
            self.log
                .push("DVB-S: no packet sync, searching again".into());
        }
    }

    #[inline]
    fn sym(&self, i: usize, conj: bool) -> Complex32 {
        let y = self.buf[i];
        if conj {
            y.conj()
        } else {
            y
        }
    }

    /// The differential header metric at `p` (0 to about 1).
    fn header_metric(&self, p: usize, conj: bool) -> f32 {
        let mut d = [Complex32::new(0.0, 0.0); HEADER];
        let mut mag = [0f32; HEADER];
        for i in 1..HEADER {
            d[i] = self.sym(p + i, conj) * self.sym(p + i - 1, conj).conj();
            mag[i] = d[i].norm_sqr().sqrt();
        }
        self.metric_of(&d, &mag)
    }

    /// The sums over a header of the products `d[i]` of each symbol with the conjugate of the symbol before it (`d[0]`
    /// is not used), each against its pattern: the 25 products of the SOF, then the 32 products inside the pairs of
    /// the PLS code; and after each sum the sum of the magnitudes `mag` of its products. Both sums point along the
    /// carrier's turn per symbol; the second is negative for the codes whose pairs differ from those of code 0.
    #[inline]
    fn header_sums(&self, d: &[Complex32], mag: &[f32]) -> (Complex32, f32, Complex32, f32) {
        let (d, mag) = (&d[..HEADER], &mag[..HEADER]);
        let mut a = Complex32::new(0.0, 0.0);
        let mut na = 0f32;
        for i in 1..26 {
            a += d[i] * self.sof_ref[i - 1].conj();
            na += mag[i];
        }
        let mut b = Complex32::new(0.0, 0.0);
        let mut nb = 0f32;
        for m in 0..32 {
            b += d[27 + 2 * m] * self.pls_ref[m].conj();
            nb += mag[27 + 2 * m];
        }
        (a, na, b, nb)
    }

    /// The header metric from the sums of [`Sync::header_sums`]: the SOF and the PLS code each compared with their own
    /// pattern.
    #[inline]
    fn metric_of(&self, d: &[Complex32], mag: &[f32]) -> f32 {
        let (a, na, b, nb) = self.header_sums(d, mag);
        0.5 * (a.norm_sqr().sqrt() / na.max(1e-12) + b.norm_sqr().sqrt() / nb.max(1e-12))
    }

    /// The PLS code (7 bits) of the header at `p` with the carrier of `f` cycles per symbol taken off: the code whose
    /// header correlates most with the 90 symbols, and the share of their power that the correlation holds (1 for a
    /// clean header at `f`).
    ///
    /// The correlation is over the whole header at once. The products of adjacent symbols do not separate the codes:
    /// some pairs of codes differ in one or two of those products only.
    ///
    /// The 64 PLS symbols are 32 pairs. The second of a pair repeats or negates the first (the last signalling bit), and
    /// the 32 first symbols are a Walsh function or its negative. So the sums and the differences of the pairs each go
    /// through a Hadamard transform of 32 points, and the SOF sum is added to each output with both signs.
    fn decode_pls(&self, p: usize, conj: bool, f: f64) -> (u8, f32) {
        let h0 = &self.headers[0];
        let turn = rot(-2.0 * PI * f);
        let mut w = Complex32::new(1.0, 0.0);
        let mut sof = Complex32::new(0.0, 0.0);
        let mut pw = 0f32;
        let mut pairs = [[Complex32::new(0.0, 0.0); 32]; 2];
        for (k, h) in h0.iter().enumerate() {
            let y = self.sym(p + k, conj);
            pw += y.norm_sqr();
            let u = y * w * h.conj();
            w *= turn;
            if k < 26 {
                sof += u;
            } else if (k - 26) % 2 == 0 {
                pairs[0][(k - 26) / 2] = u;
                pairs[1][(k - 26) / 2] = u;
            } else {
                pairs[0][(k - 26) / 2] += u;
                pairs[1][(k - 26) / 2] -= u;
            }
        }
        let mut best = (0u8, -1f32);
        for (g, map) in pairs.iter_mut().zip(&self.pls_map) {
            hadamard(g);
            for (i, &v) in g.iter().enumerate() {
                for (negative, sum) in [sof + v, sof - v].into_iter().enumerate() {
                    let score = sum.norm_sqr();
                    if score > best.1 {
                        best = (map[negative << 5 | i], score);
                    }
                }
            }
        }
        (best.0, best.1 / (HEADER as f32 * pw.max(1e-12)))
    }

    /// [`Sync::decode_pls`] for a carrier not yet known: the largest correlation over the carriers within
    /// [`HEADER_SPAN`]. Returns the code, the share and the carrier frequency, right within 1/720 of a cycle per symbol.
    fn search_pls(&self, p: usize, conj: bool) -> (u8, f32, f64) {
        let step = 1.0 / (4.0 * HEADER as f64);
        let points = (HEADER_SPAN / step) as i32;
        let mut best = (0u8, -1f32, 0f64);
        for k in -points..=points {
            let f = f64::from(k) * step;
            let (c, share) = self.decode_pls(p, conj, f);
            if share > best.1 {
                best = (c, share, f);
            }
        }
        best
    }

    /// The sum of the symbols of the header at `p` against those of the PLS code `c7`, with the carrier of `f` cycles
    /// per symbol taken off, and the sum of the powers of the symbols.
    fn header_sum(&self, p: usize, c7: u8, conj: bool, f: f64) -> (Complex32, f32) {
        let turn = rot(-2.0 * PI * f);
        let mut w = Complex32::new(1.0, 0.0);
        let mut acc = Complex32::new(0.0, 0.0);
        let mut pw = 0f32;
        for (k, hk) in self.headers[usize::from(c7)].iter().enumerate() {
            let y = self.sym(p + k, conj);
            acc += y * w * hk.conj();
            pw += y.norm_sqr();
            w *= turn;
        }
        (acc, pw)
    }

    fn valid(c7: u8) -> Option<(ModCod, bool, bool)> {
        let mc = c7 >> 2;
        let short = c7 & 2 != 0;
        let pilots = c7 & 1 != 0;
        if mc == 0 {
            // A dummy frame: the caller takes its length.
            return Some((ModCod::new(4)?, short, pilots));
        }
        let m = ModCod::new(mc)?;
        if short && !m.has_short() {
            return None;
        }
        Some((m, short, pilots))
    }

    fn frame_len(c7: u8) -> Option<usize> {
        if c7 >> 2 == 0 {
            return Some(HEADER + 36 * 90);
        }
        let (m, s, p) = Self::valid(c7)?;
        Some(m.frame_symbols(s, p))
    }

    /// The sum of the SOF's differential products at `p` against their pattern, spectrum not inverted. Its angle is the
    /// carrier's turn per symbol. An inverted spectrum negates and conjugates it.
    fn sof_sum(&self, p: usize) -> Complex32 {
        let mut a = Complex32::new(0.0, 0.0);
        for i in 1..26 {
            a += self.buf[p + i] * self.buf[p + i - 1].conj() * self.sof_ref[i - 1].conj();
        }
        a
    }

    /// The carrier frequency (cycles per symbol, within [`HEADER_SPAN`]) from the known symbols of the headers `at`
    /// (place and PLS code): the frequency at which the squared correlations with the headers sum to the most. It needs
    /// no property of the data, so it serves every constellation. Two headers at an Es/N0 of 9 dB give it within
    /// about 1/3000 of a cycle per symbol.
    fn header_freq(&self, at: &[(usize, u8)], conj: bool) -> f64 {
        let u: Vec<[Complex32; HEADER]> = at
            .iter()
            .map(|&(p, c7)| {
                let h = &self.headers[usize::from(c7)];
                std::array::from_fn(|k| self.sym(p + k, conj) * h[k].conj())
            })
            .collect();
        let score = |f: f64| -> f64 {
            let turn = rot(-2.0 * PI * f);
            u.iter()
                .map(|u| {
                    let (mut acc, mut w) = (Complex32::new(0.0, 0.0), Complex32::new(1.0, 0.0));
                    for &v in u {
                        acc += v * w;
                        w *= turn;
                    }
                    f64::from(acc.norm_sqr())
                })
                .sum()
        };
        // The correlation of a header falls to zero 1/90 of a cycle per symbol from its peak.
        let mut step = 1.0 / (8.0 * HEADER as f64);
        let points = (HEADER_SPAN / step) as i32;
        let mut best = (0.0, f64::MIN);
        for k in -points..=points {
            let f = f64::from(k) * step;
            let v = score(f);
            if v > best.1 {
                best = (f, v);
            }
        }
        for _ in 0..12 {
            step *= 0.5;
            for f in [best.0 - step, best.0 + step] {
                let v = score(f);
                if v > best.1 {
                    best = (f, v);
                }
            }
        }
        best.0
    }

    /// Makes the PL scrambling of `need` symbols and the constellation of `m` ready.
    fn prepare(&mut self, m: &ModCod, need: usize) {
        if self.scr_len < need || self.scr_gold != self.gold {
            pl_scrambling(self.gold, 33282.max(need), &mut self.scr);
            self.scr_len = self.scr.len();
            self.scr_gold = self.gold;
        }
        if self.cons_of != Some(m.id) {
            self.cons = constellation(m);
            self.decide = Decide::of(&self.cons);
            self.cons_of = Some(m.id);
        }
    }

    /// The carrier frequency (cycles per symbol) of the frame between the headers `first` and `second` (place and PLS
    /// code), from `near`, which is right within [`FREQ_DOUBT`] ([`Sync::header_freq`] at the Es/N0 that the
    /// constellations with rings need).
    ///
    /// The phases of the two headers give the frequency but for a whole number of turns over the frame. Each such
    /// frequency within the doubt is tried on the data of the frame, with the phase on the line from one header to the
    /// other: at the right one the symbols lie nearest to the points of the constellation. No loop has to pull in,
    /// and the result does not depend on the constellation.
    fn frame_freq(&mut self, first: (usize, u8), second: (usize, u8), conj: bool, near: f64) -> f64 {
        let Some((m, _, pilots)) = Self::valid(first.1) else {
            return near;
        };
        let (p, q) = (first.0, second.0);
        let span = (q - p) as f64;
        let need = q - p - HEADER;
        self.prepare(&m, need);
        // The phase of a header at its middle, and its amplitude.
        let middle = 0.5 * (HEADER - 1) as f64;
        let phase = |at: usize, c7: u8| {
            let mut acc = Complex32::new(0.0, 0.0);
            for (k, h) in self.headers[usize::from(c7)].iter().enumerate() {
                acc += self.sym(at + k, conj) * h.conj() * rot(-2.0 * PI * near * (k as f64 - middle));
            }
            (f64::from(acc.im).atan2(f64::from(acc.re)), acc.norm() / HEADER as f32)
        };
        let ((a, amp), (b, _)) = (phase(p, first.1), phase(q, second.1));
        let turn = (b - a - 2.0 * PI * near * span).rem_euclid(2.0 * PI);
        let turn = if turn > PI { turn - 2.0 * PI } else { turn };
        let base = near + turn / (2.0 * PI * span);
        let gain = 1.0 / amp_of(amp * amp);
        // The mean squared distance of every `FREQ_STEP`-th data symbol to its nearest point at the frequency `f`.
        let cost = |f: f64| {
            let mut sum = 0f32;
            let mut i = 0;
            while i < need {
                if !(pilots && i % (PILOT_EVERY + PILOT_FIELD) >= PILOT_EVERY) {
                    let y = self.sym(p + HEADER + i, conj) * QUARTER[usize::from(self.scr[i].wrapping_neg() & 3)];
                    let z = y * rot(-(a + 2.0 * PI * f * (i as f64 + HEADER as f64 - middle))) * gain;
                    sum += (z - nearest(&self.cons, z)).norm_sqr();
                }
                i += FREQ_STEP;
            }
            sum
        };
        let turns = (FREQ_DOUBT * span).ceil() as i32;
        let mut best = (base, f32::MAX);
        for k in -turns..=turns {
            let f = base + f64::from(k) / span;
            let c = cost(f);
            if c < best.1 {
                best = (f, c);
            }
        }
        best.0
    }

    /// Searches the window for two PLHEADERs one frame apart. Returns the place of the first and its PLS code, the
    /// carrier frequency (cycles per symbol), the carrier phase at that header and the spectrum sense.
    ///
    /// The header metric does not see the spectrum sense: an inverted spectrum negates every differential product of
    /// the header, and the metric takes magnitudes. The sense is in the sign of the SOF sum, for a carrier within a
    /// quarter of the symbol rate of the centre.
    ///
    /// At a low Es/N0 no single header stands over the noise in the metric. Then the headers of several frames of one
    /// length are searched together ([`Sync::find_folded`]).
    fn find_s2(&mut self) -> Option<(usize, u8, f64, f64, bool)> {
        let n = self.buf.len();
        if n < HEADER + 100 {
            return None;
        }
        // The product of each symbol with the conjugate of the one before, once for every place of a header.
        let (mut diff, mut mag) = (
            std::mem::take(&mut self.diff),
            std::mem::take(&mut self.mag),
        );
        let (mut plus, mut minus) = (std::mem::take(&mut self.plus), std::mem::take(&mut self.minus));
        diff.clear();
        diff.push(Complex32::new(0.0, 0.0));
        diff.extend(self.buf.windows(2).map(|y| y[1] * y[0].conj()));
        mag.clear();
        mag.extend(diff.iter().map(|d| d.norm_sqr().sqrt()));
        plus.clear();
        minus.clear();
        let mut peaks: Vec<(usize, f32)> = Vec::new();
        for p in 0..n - HEADER {
            let (a, na, b, nb) = self.header_sums(&diff[p..], &mag[p..]);
            let m = 0.5 * (a.norm_sqr().sqrt() / na.max(1e-12) + b.norm_sqr().sqrt() / nb.max(1e-12));
            if m > 0.5 {
                peaks.push((p, m));
            }
            plus.push(a + b);
            minus.push(a - b);
        }
        let power = mag.iter().map(|&m| f64::from(m * m)).sum::<f64>() / n as f64;
        (self.diff, self.mag) = (diff, mag);
        // Local maxima, strongest first.
        peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut found = None;
        for &(p, _) in peaks.iter().take(13) {
            let conj = self.sof_sum(p).re < 0.0;
            let (c7, share, coarse) = self.search_pls(p, conj);
            let Some(len) = Self::frame_len(c7).filter(|_| share >= HEADER_SHARE) else {
                continue;
            };
            // Confirm the next header.
            let q = p + len;
            if q + HEADER + 8 >= n {
                continue;
            }
            let second = (0..=16usize).map(|d| q + d - 8).find_map(|qq| {
                if self.header_metric(qq, false) <= 0.5 {
                    return None;
                }
                let (c, share) = self.decode_pls(qq, conj, coarse);
                (share >= HEADER_SHARE && Self::frame_len(c).is_some()).then_some((qq, c))
            });
            if let Some(second) = second {
                found = self.settle((p, c7), second, conj, coarse);
                if found.is_some() {
                    break;
                }
            }
        }
        if found.is_none() {
            if let Some(h) = self.find_folded(&plus, &minus, power) {
                found = self.settle(h.first, h.second, h.conj, h.coarse);
            }
        }
        (self.plus, self.minus) = (plus, minus);
        found
    }

    /// Searches the window for the headers of frames of one length, at an Es/N0 at which no single header shows.
    /// `plus` and `minus` hold, for every place of a header, the sum and the difference of its two sums of
    /// [`Sync::header_sums`]; `power` is the mean squared magnitude of the differential products.
    ///
    /// The sums of all headers of a carrier point the same way, whatever the carrier phase. So for each PLFRAME length
    /// the sums at places one length apart are added, over as many frames as the window holds, and the largest total
    /// over the lengths and the places is taken if it stands [`FOLD_LEVEL`] times over the total of noise. Its sign is
    /// the spectrum sense. Two of its headers must then hold the same PLS code of that length at the same carrier
    /// frequency.
    fn find_folded(
        &self,
        plus: &[Complex32],
        minus: &[Complex32],
        power: f64,
    ) -> Option<HeaderPair> {
        let places = plus.len();
        // The largest total: its level over noise, the frame length, the place and the total.
        let mut best = (0f64, 0usize, 0usize, Complex32::new(0.0, 0.0));
        let mut acc: Vec<Complex32> = Vec::new();
        for &len in &self.lengths {
            let frames = places / len;
            if frames < 2 {
                continue;
            }
            let noise = 57.0 * frames as f64 * power.max(1e-30);
            for sums in [plus, minus] {
                let p = fold_complex(sums, len, &mut acc);
                let level = f64::from(acc[p].norm_sqr()) / noise;
                if level > best.0 {
                    best = (level, len, p, acc[p]);
                }
            }
        }
        let (level, len, p, total) = best;
        if level < FOLD_LEVEL {
            return None;
        }
        let conj = total.re < 0.0;
        // The headers of the first frames that hold a code of this length.
        let step = 1.0 / (4.0 * HEADER as f64);
        let seen: Vec<(usize, u8, f64)> = (0..(places / len).min(FOLD_HEADERS))
            .filter_map(|k| {
                let at = p + k * len;
                let (c7, share, f) = self.search_pls(at, conj);
                (share >= HEADER_SHARE && Self::frame_len(c7) == Some(len)).then_some((at, c7, f))
            })
            .collect();
        for (i, &(at, c7, f)) in seen.iter().enumerate() {
            let agrees = |o: &&(usize, u8, f64)| o.1 == c7 && (o.2 - f).abs() < 2.5 * step;
            if let Some(&(_, _, g)) = seen[i + 1..].iter().find(agrees) {
                return Some(HeaderPair {
                    first: (at, c7),
                    second: (at + len, c7),
                    conj,
                    coarse: 0.5 * (f + g),
                });
            }
        }
        None
    }

    /// The carrier of the frame between the headers `first` and `second` (place and PLS code) for the sense `conj`,
    /// from `coarse`, the carrier frequency within 1/360 of a cycle per symbol. Returns the place and the PLS code of
    /// the first header of the window from which frames of that length follow each other up to `first`, the carrier
    /// frequency (cycles per symbol), the carrier phase at that header and the spectrum sense.
    fn settle(
        &mut self,
        first: (usize, u8),
        second: (usize, u8),
        conj: bool,
        coarse: f64,
    ) -> Option<(usize, u8, f64, f64, bool)> {
        let n = self.buf.len();
        let (m, _, _) = Self::valid(first.1)?;
        let p = first.0;
        if second.0 + HEADER > n {
            return None;
        }
        // The carrier: of QPSK and 8PSK from the line of the fourth or eighth power of the data near the carrier
        // of the headers. The other constellations have no such line (their outer rings cancel in it): from
        // the two headers and the data between them.
        let f = match m.modulation {
            Modulation::Qpsk | Modulation::Psk8 => {
                let power = if m.modulation == Modulation::Psk8 { 8 } else { 4 };
                let a = p + HEADER;
                let b = (a + 32768).min(n);
                // The carrier of the headers is taken off first, and the line is searched only as far from it as the
                // headers leave open: at a low Es/N0 a peak of the noise elsewhere is stronger than the line.
                let seg: Vec<Complex32> = (a..b)
                    .map(|i| self.sym(i, conj) * rot(-2.0 * PI * coarse * (i - a) as f64))
                    .collect();
                power_freq_within(&seg, power, 0.5 / HEADER as f64).map(|d| coarse + d)
            }
            _ => None,
        };
        let f = match f {
            Some(f) => f,
            None => {
                let near = self.header_freq(&[first, second], conj);
                self.frame_freq(first, second, conj, near)
            }
        };
        // The frames of this length that lie before the header in the window are followed too: back to the first
        // of them whose header shows at this carrier.
        let (mut p, mut c7) = first;
        let len = second.0 - first.0;
        while p >= len {
            let (c, share) = self.decode_pls(p - len, conj, f);
            if share < HEADER_SHARE || Self::frame_len(c) != Some(len) {
                break;
            }
            (p, c7) = (p - len, c);
        }
        let (m, s, pi) = Self::valid(c7)?;
        // Phase at the header.
        let h = &self.headers[c7 as usize];
        let mut acc = Complex32::new(0.0, 0.0);
        for (k, hk) in h.iter().enumerate() {
            acc += self.sym(p + k, conj) * hk.conj() * rot(-2.0 * PI * f * k as f64);
        }
        let theta = (acc.im as f64).atan2(acc.re as f64);
        self.log.push(if c7 >> 2 == 0 {
            "DVB-S2, dummy frames".to_string()
        } else {
            format!(
                "DVB-S2, {}{}{}",
                m.name(),
                if s { " short" } else { "" },
                if pi { " pilots" } else { "" }
            )
        });
        Some((p, c7, f, theta, conj))
    }

    fn follow_s2(&mut self) {
        loop {
            let Mode::S2 {
                next,
                f,
                theta,
                conj,
                misses,
                last_len,
                code,
                index,
                amp,
            } = self.mode
            else {
                return;
            };
            let end = self.base + self.buf.len() as u64;
            // Wait for the whole frame (longest) plus the search margin.
            if next + (HEADER + 16) as u64 > end || next < self.base {
                return;
            }
            let p0 = (next - self.base) as usize;
            // A frame is most often as long as the one before it: until that many symbols are here, no search.
            if p0 + last_len + HEADER + 8 > self.buf.len() {
                return;
            }
            // The expected code at the expected place. A header that is 1 to 8 symbols off its place holds no more
            // than 0.11 of its power against its own code, so [`FOLLOW_SHARE`] tells the place.
            let (mut p, mut c7) = (p0, code);
            let (mut acc, mut pw) = self.header_sum(p, c7, conj, f);
            let mut found = acc.norm_sqr() >= FOLLOW_SHARE * HEADER as f32 * pw;
            if !found {
                // The frame is of another kind, or the symbol clock slipped: the place within 8 symbols and the
                // PLS code that hold the largest share.
                let lo = p0.saturating_sub(8);
                let hi = (p0 + 8).min(self.buf.len().saturating_sub(HEADER + 1));
                let mut best = (p0, 0u8, -1f32);
                for q in lo..=hi {
                    let (c, share) = self.decode_pls(q, conj, f);
                    if share > best.2 {
                        best = (q, c, share);
                    }
                }
                if best.2 >= FOLLOW_SHARE && Self::frame_len(best.1).is_some() {
                    (p, c7) = (best.0, best.1);
                    (acc, pw) = self.header_sum(p, c7, conj, f);
                    found = true;
                }
            }
            let Some(len) = Self::frame_len(c7) else {
                self.lost();
                return;
            };
            if p + len + HEADER + 8 > self.buf.len() {
                return;
            }
            // Coherent phase of this header against the prediction. Without a header over [`FOLLOW_SHARE`], the
            // expected one counts from [`HEADER_SHARE`] on.
            let sig = (acc / HEADER as f32).norm_sqr();
            let noise = (pw / HEADER as f32 - sig).max(1e-9);
            let seen = found || sig >= HEADER_SHARE * pw / HEADER as f32;
            let misses = if seen {
                0
            } else {
                self.report.misses += 1;
                if misses + 1 >= 3 {
                    self.log
                        .push("DVB-S2: headers lost, searching again".into());
                    self.lost();
                    return;
                }
                misses + 1
            };
            // A frame without a header that shows is taken as one more of the last kind, at the place and the carrier
            // phase expected.
            let (meas, esn0) = if seen {
                (
                    (acc.im as f64).atan2(acc.re as f64),
                    sdrkit_algo_dsp::meter::power_to_db((sig / noise) as f64),
                )
            } else {
                (theta, self.report.esn0_db.unwrap_or(0.0))
            };
            let mut f = f;
            if index > 0 && last_len > 0 {
                let mut d = meas - theta;
                while d > PI {
                    d -= 2.0 * PI;
                }
                while d < -PI {
                    d += 2.0 * PI;
                }
                f += 0.5 * d / (2.0 * PI * last_len as f64);
            }
            let theta0 = meas;
            // Data.
            let dummy = c7 >> 2 == 0;
            let Some((m, short, pilots)) = Self::valid(c7) else {
                self.lost();
                return;
            };
            let need = len - HEADER;
            self.prepare(&m, need);
            let mut frame = S2Frame {
                modcod: c7 >> 2,
                short,
                pilots,
                esn0_db: esn0,
                index,
                restart: index == 0,
                ..Default::default()
            };
            let mut ph = 0f64;
            // The amplitude of the header, as a mean over the frames.
            let mut amp = if index == 0 {
                amp_of(sig)
            } else if seen {
                amp + HEADER_AMP_RATE * (amp_of(sig) - amp)
            } else {
                amp
            };
            let mut f_next = f;
            if !dummy {
                let data = DataRun {
                    y: &self.buf[p + HEADER..p + len],
                    scr: &self.scr[..need],
                    conj,
                    theta: theta0 + 2.0 * PI * f * HEADER as f64,
                    freq: f,
                    pilots,
                };
                let (decide, cons) = (self.decide, &self.cons[..]);
                let (mut sp, mut ep) = (0f64, 0f64);
                frame.syms = self.pool.exact(data.symbols(), |out| {
                    (ph, sp, ep) = match decide {
                        Decide::Qpsk => data.correct(out, nearest_qpsk),
                        Decide::Psk8 => data.correct(out, nearest_8psk),
                        Decide::Scan if pilots => data.correct_by_pilots(out, cons, &mut amp),
                        Decide::Scan => data.correct_rings(out, cons, amp),
                    };
                });
                // The mean frequency of the frame, as the loop followed it.
                f_next = f + ph / (2.0 * PI * need as f64);
                frame.mer_db = sdrkit_algo_dsp::meter::power_to_db(sp / ep.max(1e-12));
            }
            // The expected phase of the next header.
            let theta_next = theta0 + 2.0 * PI * f * len as f64 + ph;
            let next_abs = self.base + (p + len) as u64;
            self.report.frames += 1;
            if dummy {
                self.report.dummies += 1;
            } else {
                match self.first_modcod {
                    None => self.first_modcod = Some(c7 >> 2),
                    Some(mc) if mc != c7 >> 2 => self.report.vcm = true,
                    _ => {}
                }
                self.report.modcod = c7 >> 2;
                self.report.short = short;
                self.report.pilots = pilots;
                self.report.esn0_db = Some(esn0);
                self.report.mer_db = Some(frame.mer_db);
                self.report.mer_history.push(frame.mer_db as f32);
                if self.report.mer_history.len() > 240 {
                    self.report.mer_history.remove(0);
                }
                for z in frame.syms.iter().step_by(4).take(512) {
                    if self.report.constellation.len() < 2048 {
                        self.report.constellation.push(*z);
                    } else {
                        self.report.constellation[self.cpos] = *z;
                        self.cpos = (self.cpos + 1) % 2048;
                    }
                }
                self.frames_out.push(frame);
            }
            self.report.freq_cps = f;
            let mut th = theta_next;
            while th > PI {
                th -= 2.0 * PI;
            }
            while th < -PI {
                th += 2.0 * PI;
            }
            self.mode = Mode::S2 {
                next: next_abs,
                f: f_next,
                theta: th,
                conj,
                misses,
                last_len: len,
                code: c7,
                index: index + 1,
                amp,
            };
        }
    }

    fn find_s1(&mut self) -> bool {
        let n = self.buf.len();
        let w0 = n.saturating_sub(49_152);
        let w: Vec<Complex32> = self.buf[w0..].to_vec();
        let f = power_freq(&w[..w.len().min(16384)], 4);
        let mut theta = 0.0f64;
        let mut nu = 2.0 * PI * f;
        let mut z = Vec::with_capacity(w.len());
        let (mut kp, mut ki) = (0.05, 1e-3);
        for (i, &y) in w.iter().enumerate() {
            if i == 4096 {
                (kp, ki) = (0.02, 2e-4);
            }
            let v = y * rot(-theta);
            let d = Complex32::new(v.re.signum(), v.im.signum());
            let e = (v * d.conj()).im as f64 / 2.0;
            nu += ki * e;
            theta += nu + kp * e;
            z.push(v);
        }
        let settle = 8192;
        let zz = &z[settle..];
        // Score every hypothesis on a short decode.
        let mut scores: Vec<(f64, u8, PunctureRate, usize)> = Vec::new();
        for variant in 0..4u8 {
            let soft = soft_bits(zz, variant, 6000);
            let total: f64 = soft.iter().map(|&s| (s as f64).abs()).sum();
            for rate in PunctureRate::ALL {
                for phase in 0..rate.coded() {
                    let mut d = Depuncturer::with_phase(rate.puncture(), rate.pattern_index(phase));
                    let mut mother = Vec::with_capacity(2 * soft.len() + 1);
                    if rate.starts_with_y(phase) {
                        mother.push(0);
                    }
                    d.push(&soft, &mut mother);
                    let mut v = inner_decoder();
                    let mut bits = Vec::with_capacity(mother.len() / 2);
                    v.push(&mother, &mut bits);
                    let m = v.mean_gain() as f64 * v.steps() as f64 / total.max(1.0);
                    scores.push((m, variant, rate, phase));
                }
            }
        }
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut near = false;
        for &(_, variant, rate, phase) in scores.iter().take(40) {
            let soft = soft_bits(zz, variant, usize::MAX);
            let mut d = Depuncturer::with_phase(rate.puncture(), rate.pattern_index(phase));
            let mut mother = Vec::with_capacity(2 * soft.len() + 1);
            if rate.starts_with_y(phase) {
                mother.push(0);
            }
            d.push(&soft, &mut mother);
            let mut v = inner_decoder();
            let mut bits = Vec::with_capacity(mother.len() / 2);
            v.push(&mother, &mut bits);
            v.flush(&mut bits);
            // The sync bytes alone can be fooled (a quarter turn with another puncturing phase reproduces them with the rest
            // wrong): Reed-Solomon must agree too.
            let hit = sync_search(&bits, &super::sync_rule(16, true));
            if hit.is_some() {
                near = true;
            }
            if hit.is_some_and(|h| rs_agrees(&bits, h.start, h.inverted)) {
                let inverted = variant >= 2;
                self.report.standard = Some(Received::Dvbs);
                self.report.inverted = inverted;
                self.report.rate = Some(rate);
                self.report.locked = true;
                self.report.freq_cps = nu / (2.0 * PI);
                self.log.push(format!(
                    "DVB-S, QPSK {}{}",
                    rate.name(),
                    if inverted { ", spectrum inverted" } else { "" }
                ));
                self.soft_out.push(S1Soft {
                    soft,
                    rate,
                    phase,
                    restart: true,
                });
                // Es/N0 from the decided symbols.
                self.measure_s1(zz, variant);
                self.mode = Mode::S1 {
                    variant,
                    rate,
                    theta,
                    nu,
                    f,
                };
                self.buf.clear();
                self.base += n as u64;
                return true;
            }
        }
        if near {
            // Packets were seen but did not decode: look again soon.
            self.since_hunt = (65_536u64 << self.fails_in_row.min(2)).saturating_sub(16_384);
            self.fails_in_row = 0;
        }
        false
    }

    fn measure_s1(&mut self, z: &[Complex32], variant: u8) {
        let (mut sp, mut ep) = (0f64, 0f64);
        for &v in z.iter().rev().take(8192) {
            let v = apply_variant(v, variant);
            let d = Complex32::new(v.re.signum(), v.im.signum()) * std::f32::consts::FRAC_1_SQRT_2;
            sp += d.norm_sqr() as f64;
            ep += (v - d).norm_sqr() as f64;
            if self.report.constellation.len() < 2048 {
                self.report.constellation.push(v);
            } else {
                self.report.constellation[self.cpos] = v;
                self.cpos = (self.cpos + 1) % 2048;
            }
        }
        let m = sdrkit_algo_dsp::meter::power_to_db(sp / ep.max(1e-12));
        self.report.esn0_db = Some(m);
        self.report.mer_db = Some(m);
    }

    fn follow_s1(&mut self, _n: usize) {
        let Mode::S1 {
            variant,
            rate,
            mut theta,
            mut nu,
            f,
        } = self.mode
        else {
            return;
        };
        let (kp, ki) = (0.01, 5e-5);
        let mut z = Vec::with_capacity(self.buf.len());
        for &y in &self.buf {
            let v = y * rot(-theta);
            let d = Complex32::new(v.re.signum(), v.im.signum());
            let e = (v * d.conj()).im as f64 / 2.0;
            nu += ki * e;
            theta += nu + kp * e;
            if theta.abs() > 1e6 || !theta.is_finite() {
                theta = 0.0;
            }
            z.push(v);
        }
        if theta > PI || theta < -PI {
            theta = theta.rem_euclid(2.0 * PI);
        }
        let soft = soft_bits(&z, variant, usize::MAX);
        if !soft.is_empty() {
            self.soft_out.push(S1Soft {
                soft,
                rate,
                phase: 0,
                restart: false,
            });
        }
        if !z.is_empty() {
            self.measure_s1_running(&z, variant);
        }
        self.report.freq_cps = nu / (2.0 * PI);
        self.base += self.buf.len() as u64;
        self.buf.clear();
        self.mode = Mode::S1 {
            variant,
            rate,
            theta,
            nu,
            f,
        };
    }

    fn measure_s1_running(&mut self, z: &[Complex32], variant: u8) {
        let (mut sp, mut ep) = (0f64, 0f64);
        for &v in z {
            let v = apply_variant(v, variant);
            let d = Complex32::new(v.re.signum(), v.im.signum()) * std::f32::consts::FRAC_1_SQRT_2;
            sp += d.norm_sqr() as f64;
            ep += (v - d).norm_sqr() as f64;
        }
        let m = sdrkit_algo_dsp::meter::power_to_db(sp / ep.max(1e-12));
        let old = self.report.mer_db.unwrap_or(m);
        let v = old + 0.2 * (m - old);
        self.report.esn0_db = Some(v);
        self.report.mer_db = Some(v);
        for &v in z.iter().step_by(4).take(256) {
            let v = apply_variant(v, variant);
            if self.report.constellation.len() < 2048 {
                self.report.constellation.push(v);
            } else {
                self.report.constellation[self.cpos] = v;
                self.cpos = (self.cpos + 1) % 2048;
            }
        }
    }
}

/// Data symbols between two pilot fields: 16 slots.
const PILOT_EVERY: usize = 1440;
/// Symbols of a pilot field.
const PILOT_FIELD: usize = 36;
/// The gain of the decision-directed phase refinement, radians per radian of error and symbol.
const DATA_GAIN: f32 = 0.01;
/// The gain of the phase refinement on a pilot symbol.
const PILOT_GAIN: f32 = 0.05;
/// Symbols that share one step of the phase refinement.
const PHASE_GROUP: usize = 4;
/// The carrier offsets that the search on the headers covers, cycles per symbol each way.
const HEADER_SPAN: f64 = 0.1;
/// The least share of the power of a header that its known symbols hold at the carrier followed. A header 9 dB under
/// the noise holds more; at a wrong carrier frequency the share is near 1/90. (In the wrong spectrum sense the 64 PLS
/// symbols still fit another code, so the share does not tell the sense: the SOF does.)
const HEADER_SHARE: f32 = 0.1;
/// The least share of the power of a header that its known symbols hold for a header to be taken at any place near
/// the expected one with any PLS code. Noise passes it once in 30 000 frames; at [`HEADER_SHARE`], which serves for
/// the one expected place and code, a frame of noise in five would pass. A header at an Es/N0 of -2.35 dB holds 0.37.
const FOLLOW_SHARE: f32 = 0.2;
/// The least level of the total of the header sums of several frames over the total of noise, as a ratio of powers.
/// Noise alone passes it in about one search in 1000.
const FOLD_LEVEL: f64 = 20.0;
/// The headers at most that the search over several frames decodes.
const FOLD_HEADERS: usize = 8;
/// Symbols of one step of the carrier loop of the constellations with rings.
const RING_BLOCK: usize = 32;
/// The share of the difference to the amplitude of a header that moves the amplitude.
const HEADER_AMP_RATE: f32 = 0.125;
/// The shares of the phase error of a block that move the phase and (per block length) the frequency of the carrier
/// loop of the constellations with rings. The loop is narrow: the hunt and each frame give it the frequency, and at the
/// lowest Es/N0 of these constellations one decision in four is wrong, so a wide loop slips to the next point of a ring.
const RING_PHASE: f32 = 0.05;
const RING_FREQ: f32 = 0.001;
/// The share of the difference to the amplitude of a pilot field that moves the amplitude.
const PILOT_AMP_RATE: f32 = 0.1;
/// The doubt of the carrier frequency that the search on two headers leaves, cycles per symbol each way.
const FREQ_DOUBT: f64 = 6e-4;
/// One data symbol in this many scores a carrier frequency in [`Sync::frame_freq`].
const FREQ_STEP: usize = 1;

/// The Hadamard transform of `x` in place: output `i` is the sum of the inputs `k` with the sign of the Walsh function
/// `i`, negative where `i & k` has an odd number of bits.
fn hadamard(x: &mut [Complex32; 32]) {
    let mut half = 1;
    while half < 32 {
        for block in x.chunks_exact_mut(2 * half) {
            let (a, b) = block.split_at_mut(half);
            for (u, v) in a.iter_mut().zip(b) {
                (*u, *v) = (*u + *v, *u - *v);
            }
        }
        half *= 2;
    }
}

/// The amplitude of symbols of the power `sig`, within a range that keeps the loop's start sound.
fn amp_of(sig: f32) -> f32 {
    sig.sqrt().clamp(0.1, 2.0)
}

/// A turn by 0 to 3 quarter turns.
const QUARTER: [Complex32; 4] = [
    Complex32::new(1.0, 0.0),
    Complex32::new(0.0, 1.0),
    Complex32::new(-1.0, 0.0),
    Complex32::new(0.0, -1.0),
];

/// How the nearest point of a constellation is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decide {
    /// The four points (±1 ± j) / √2: by the signs.
    Qpsk,
    /// Eight points of magnitude 1 at the multiples of 45°: by the signs and the larger part.
    Psk8,
    /// Any other set: every point is tried.
    Scan,
}

impl Decide {
    /// The rule for the points `c`.
    fn of(c: &[Complex32]) -> Decide {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let diagonal = |z: &Complex32| near(z.re.abs(), h) && near(z.im.abs(), h);
        let axis = |z: &Complex32| {
            (near(z.re.abs(), 1.0) && near(z.im, 0.0)) || (near(z.im.abs(), 1.0) && near(z.re, 0.0))
        };
        if c.len() == 4 && c.iter().all(diagonal) {
            Decide::Qpsk
        } else if c.len() == 8 && c.iter().all(|z| diagonal(z) || axis(z)) {
            Decide::Psk8
        } else {
            Decide::Scan
        }
    }
}

/// The nearest point of the QPSK set to `z`.
#[inline]
fn nearest_qpsk(z: Complex32) -> Complex32 {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(h.copysign(z.re), h.copysign(z.im))
}

/// The nearest point of the 8PSK set to `z`. The parts are chosen without a branch: which of the three cases holds is as
/// random as the data.
#[inline]
fn nearest_8psk(z: Complex32) -> Complex32 {
    // tan 67.5°: beyond this ratio of the parts the point on the axis is nearer than the diagonal one.
    const T: f32 = 2.414_213_6;
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let (a, b) = (z.re.abs(), z.im.abs());
    let re = if a > T * b { 1.0 } else { h };
    let re = if b > T * a { 0.0 } else { re };
    let im = if b > T * a { 1.0 } else { h };
    let im = if a > T * b { 0.0 } else { im };
    Complex32::new(re.copysign(z.re), im.copysign(z.im))
}

/// The nearest point of `cons` to `z`.
#[inline]
fn nearest(cons: &[Complex32], z: Complex32) -> Complex32 {
    let mut best = (Complex32::new(1.0, 0.0), f32::MAX);
    for &c in cons {
        let d = (z - c).norm_sqr();
        if d < best.1 {
            best = (c, d);
        }
    }
    best.0
}

/// The symbols of one PLFRAME after its header, with what turns them into data symbols.
struct DataRun<'a> {
    /// The symbols after the header.
    y: &'a [Complex32],
    /// The PL scrambling of each symbol, quarter turns.
    scr: &'a [u8],
    /// The spectrum is inverted.
    conj: bool,
    /// The carrier phase at the first symbol, radians.
    theta: f64,
    /// The carrier frequency, cycles per symbol.
    freq: f64,
    /// Pilot fields are present.
    pilots: bool,
}

impl DataRun<'_> {
    /// The number of data symbols.
    fn symbols(&self) -> usize {
        let n = self.y.len().min(self.scr.len());
        if self.pilots {
            let period = PILOT_EVERY + PILOT_FIELD;
            n / period * PILOT_EVERY + (n % period).min(PILOT_EVERY)
        } else {
            n
        }
    }

    /// Takes the carrier and the scrambling off, follows the phase by the decisions of `nearest` and by the pilots, and
    /// writes the data symbols to `out` ([`DataRun::symbols`] long). Every point of `nearest` has magnitude 1 (QPSK,
    /// 8PSK). Returns the phase the refinement added (radians), the energy of the decided points and the energy of the
    /// errors.
    ///
    /// The carrier is a phasor. It is set from the phase at the start of each run of 16 slots and of each pilot field, and
    /// brought back to magnitude 1 every few symbols, so its rounding does not add up. The refinement moves it once per
    /// [`PHASE_GROUP`] symbols by the sum of their errors: the symbols of a group do not wait for each other, and at these
    /// gains the phase is the one a step per symbol gives.
    fn correct(
        &self,
        out: &mut [Complex32],
        nearest: impl Fn(Complex32) -> Complex32,
    ) -> (f64, f64, f64) {
        /// Groups between two corrections of the phasor's magnitude.
        const TRIM: usize = 8;
        let n = self.y.len().min(self.scr.len());
        // The turn of the carrier over 0 to `PHASE_GROUP` symbols.
        let mut turns = [Complex32::new(1.0, 0.0); PHASE_GROUP + 1];
        let turn = rot(-2.0 * PI * self.freq);
        for i in 1..=PHASE_GROUP {
            turns[i] = turns[i - 1] * turn;
        }
        let flip = if self.conj { -1.0 } else { 1.0 };
        let (mut ph, mut sp, mut ep) = (0f64, 0f64, 0f64);
        let (mut j, mut o) = (0, 0);
        // The symbol `i` of the frame with the scrambling taken off.
        let plain = |i: usize| {
            let y = self.y[i];
            Complex32::new(y.re, flip * y.im) * QUARTER[usize::from(self.scr[i].wrapping_neg() & 3)]
        };
        // The phasor `w` after `symbols` more symbols and a refinement of `d` radians, to the first order of `d`.
        let advance = |w: Complex32, symbols: usize, d: f32| {
            let t = w * turns[symbols];
            Complex32::new(t.re + t.im * d, t.im - t.re * d)
        };
        // The phasor `w` brought near magnitude 1.
        let trim = |w: Complex32| w * (1.5 - 0.5 * w.norm_sqr());
        while j < n {
            let run = PILOT_EVERY.min(n - j).min(out.len() - o);
            let mut w = rot(-(self.theta + 2.0 * PI * self.freq * j as f64 + ph));
            let (mut dph, mut s, mut e) = (0f32, 0f32, 0f32);
            for (c, part) in out[o..o + run].chunks_mut(PHASE_GROUP).enumerate() {
                if c % TRIM == 0 {
                    w = trim(w);
                }
                let mut step = 0f32;
                for (i, slot) in part.iter_mut().enumerate() {
                    let z = plain(j + c * PHASE_GROUP + i) * (w * turns[i]);
                    let point = nearest(z);
                    let turned = z.im * point.re - z.re * point.im;
                    step += DATA_GAIN * turned;
                    s += 1.0;
                    e += (z - point).norm_sqr();
                    *slot = z;
                }
                dph += step;
                w = advance(w, part.len(), step);
            }
            ph += f64::from(dph);
            sp += f64::from(s);
            ep += f64::from(e);
            j += run;
            o += run;
            if !self.pilots || run < PILOT_EVERY {
                if run == 0 {
                    break;
                }
                continue;
            }
            let field = PILOT_FIELD.min(n - j);
            let mut w = rot(-(self.theta + 2.0 * PI * self.freq * j as f64 + ph));
            let mut dph = 0f32;
            let mut i = 0;
            while i < field {
                let part = PHASE_GROUP.min(field - i);
                let mut step = 0f32;
                for k in 0..part {
                    step += PILOT_GAIN * (plain(j + i + k) * (w * turns[k]) * pl::PILOT.conj()).im;
                }
                dph += step;
                w = advance(w, part, step);
                i += part;
            }
            ph += f64::from(dph);
            j += field;
        }
        (ph, sp, ep)
    }
}

impl DataRun<'_> {
    /// [`DataRun::correct`] for a constellation `cons` with rings (16APSK, 32APSK), and the same returns. Its points
    /// lie closer in phase than those of 8PSK, and the hunt gives its carrier from the headers alone, so the loop is of
    /// the second order: every [`RING_BLOCK`] symbols, the phase error of the block against its decisions moves the
    /// phase and the frequency by the shares of `gear`. A pilot field is a block of known symbols. The ring radii need
    /// the amplitude: the symbols are divided by `amp`, which follows the decisions too.
    fn correct_rings(
        &self,
        out: &mut [Complex32],
        cons: &[Complex32],
        amp: f32,
    ) -> (f64, f64, f64) {
        let n = self.y.len().min(self.scr.len());
        let flip = if self.conj { -1.0 } else { 1.0 };
        let plain = |i: usize| {
            let y = self.y[i];
            Complex32::new(y.re, flip * y.im) * QUARTER[usize::from(self.scr[i].wrapping_neg() & 3)]
        };
        let (mut phi, mut omega) = (self.theta, 2.0 * PI * self.freq);
        let (mut sp, mut ep) = (0f64, 0f64);
        let (mut j, mut o) = (0, 0);
        // Ends a block of `len` symbols with the correlation `acc` of the symbols with their points.
        let step = |phi: &mut f64, omega: &mut f64, acc: Complex32, len: usize| {
            let err = if acc.norm_sqr() > 0.0 {
                acc.im.atan2(acc.re)
            } else {
                0.0
            };
            *phi += *omega * len as f64 + f64::from(RING_PHASE * err);
            *omega += f64::from(RING_FREQ * err) / len as f64;
        };
        while j < n && o < out.len() {
            let run = PILOT_EVERY.min(n - j).min(out.len() - o);
            for part in out[o..o + run].chunks_mut(RING_BLOCK) {
                let turn = rot(-omega);
                let mut w = rot(-phi) / amp;
                let mut acc = Complex32::new(0.0, 0.0);
                let (mut s, mut e) = (0f32, 0f32);
                for (i, slot) in part.iter_mut().enumerate() {
                    let z = plain(j + i) * w;
                    w *= turn;
                    let point = nearest(cons, z);
                    acc += z * point.conj();
                    s += point.norm_sqr();
                    e += (z - point).norm_sqr();
                    *slot = z;
                }
                j += part.len();
                step(&mut phi, &mut omega, acc, part.len());
                sp += f64::from(s);
                ep += f64::from(e);
            }
            o += run;
            if !self.pilots || run < PILOT_EVERY {
                continue;
            }
            let field = PILOT_FIELD.min(n - j);
            if field == 0 {
                break;
            }
            let turn = rot(-omega);
            let mut w = rot(-phi);
            let mut acc = Complex32::new(0.0, 0.0);
            for i in 0..field {
                acc += plain(j + i) * w * pl::PILOT.conj();
                w *= turn;
            }
            step(&mut phi, &mut omega, acc, field);
            j += field;
        }
        let ph = phi - (self.theta + 2.0 * PI * self.freq * j as f64);
        (ph, sp, ep)
    }
}

impl DataRun<'_> {
    /// [`DataRun::correct`] for a constellation `cons` with rings in a frame with pilots, and the same returns. The
    /// phase comes from the known symbols alone: the header before the frame and each pilot field give the phase at
    /// their middle, and the data between two of them get the line through both. After the last field the phase of
    /// that field holds. No decision moves the carrier, so a wrong decision turns nothing. The symbols are divided by
    /// `amp`, which follows the amplitude of the pilot fields.
    fn correct_by_pilots(&self, out: &mut [Complex32], cons: &[Complex32], amp: &mut f32) -> (f64, f64, f64) {
        let n = self.y.len().min(self.scr.len());
        let flip = if self.conj { -1.0 } else { 1.0 };
        let plain = |i: usize| {
            let y = self.y[i];
            Complex32::new(y.re, flip * y.im) * QUARTER[usize::from(self.scr[i].wrapping_neg() & 3)]
        };
        let omega = 2.0 * PI * self.freq;
        let (mut sp, mut ep) = (0f64, 0f64);
        let (mut j, mut o) = (0, 0);
        // The phase over the prediction at the last known symbols, and their middle (the header's lies before the data).
        let (mut before, mut at) = (0f64, -0.5 * HEADER as f64);
        while j < n && o < out.len() {
            let run = PILOT_EVERY.min(n - j).min(out.len() - o);
            let field = if run == PILOT_EVERY {
                PILOT_FIELD.min(n - j - run)
            } else {
                0
            };
            // The pilot field after the run, against the prediction with the phase before the run.
            let mut slope = 0f64;
            let mut after = before;
            if field > 0 {
                let first = j + run;
                let turn = rot(-omega);
                let mut w = rot(-(self.theta + omega * first as f64 + before));
                let mut acc = Complex32::new(0.0, 0.0);
                for i in 0..field {
                    acc += plain(first + i) * w * pl::PILOT.conj();
                    w *= turn;
                }
                let middle = first as f64 + 0.5 * field as f64;
                after = before + f64::from(acc.im.atan2(acc.re));
                slope = (after - before) / (middle - at);
                *amp = (*amp + PILOT_AMP_RATE * (acc.norm() / field as f32 - *amp)).clamp(0.1, 2.0);
                at = middle;
            }
            // The phase of symbol `i` of the frame: the line from the known symbols before the run to those after.
            let start = if field > 0 {
                // `at` is now the middle of the field after the run; the line ends there with `after`.
                after - slope * (at - j as f64)
            } else {
                before
            };
            let turn = rot(-(omega + slope));
            let mut w = rot(-(self.theta + omega * j as f64 + start)) / *amp;
            let (mut s, mut e) = (0f32, 0f32);
            for (c, part) in out[o..o + run].chunks_mut(RING_BLOCK).enumerate() {
                if c % 8 == 0 {
                    // The rounding of the turns does not add up.
                    let k = (j + c * RING_BLOCK) as f64;
                    w = rot(-(self.theta + omega * k + start + slope * (k - j as f64))) / *amp;
                }
                for (i, slot) in part.iter_mut().enumerate() {
                    let z = plain(j + c * RING_BLOCK + i) * w;
                    w *= turn;
                    let point = nearest(cons, z);
                    s += point.norm_sqr();
                    e += (z - point).norm_sqr();
                    *slot = z;
                }
            }
            sp += f64::from(s);
            ep += f64::from(e);
            j += run + field;
            o += run;
            before = after;
        }
        // The phase at the end of the frame over the prediction: that of the last field, as far as the frame is long.
        let ph = if at > 0.0 { before * n as f64 / at } else { 0.0 };
        (ph, sp, ep)
    }
}

fn rs_agrees(bits: &[u8], start: usize, inv: bool) -> bool {
    let mut dec = OuterDecoder::new(Dispersal::DvbGroups);
    let n = (bits.len() - start) / 8;
    let bytes: Vec<u8> = (0..n)
        .map(|k| {
            let v = bits[start + 8 * k..start + 8 * k + 8]
                .iter()
                .fold(0u8, |a, &b| (a << 1) | b);
            if inv {
                !v
            } else {
                v
            }
        })
        .collect();
    dec.push(&bytes, &mut |_| {});
    let good = dec.stats.clean + dec.stats.corrected;
    good >= 2
}

#[inline]
fn apply_variant(v: Complex32, variant: u8) -> Complex32 {
    let v = if variant >= 2 { v.conj() } else { v };
    if variant & 1 == 1 {
        Complex32::new(-v.im, v.re)
    } else {
        v
    }
}

/// Soft bits (I then Q of each symbol) of at most `max` bits.
fn soft_bits(z: &[Complex32], variant: u8, max: usize) -> Vec<i8> {
    let mut out = Vec::with_capacity((2 * z.len()).min(max));
    for &v in z {
        if out.len() >= max {
            break;
        }
        let v = apply_variant(v, variant);
        out.push((v.re * 64.0).clamp(-127.0, 127.0) as i8);
        out.push((v.im * 64.0).clamp(-127.0, 127.0) as i8);
    }
    out
}

/// The carrier frequency (cycles per symbol) of a PSK-like signal from the strongest line of its `power`-th power
/// (`sdrkit_algo_modem::freq::PowerLine`, FFT of 1024 to 65536 points). Returns 0 when `x` is too short or `power` is out of range.
pub fn power_freq(x: &[Complex32], power: u32) -> f64 {
    let n = x.len().next_power_of_two().clamp(1024, 1 << 16);
    PowerLine::new(power, n)
        .and_then(|mut p| p.estimate(&x[..x.len().min(n)]))
        .map_or(0.0, |l| l.freq)
}

/// [`power_freq`] for a carrier within `span` cycles per symbol of zero: the strongest line of that range only. `None`
/// when `x` is too short or an argument is out of range.
fn power_freq_within(x: &[Complex32], power: u32, span: f64) -> Option<f64> {
    let n = x.len().next_power_of_two().clamp(1024, 1 << 16);
    let mut line = PowerLine::new(power, n).ok()?;
    line.set_span(span).ok()?;
    line.estimate(&x[..x.len().min(n)]).ok().map(|l| l.freq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pl::code7;

    fn noise(seed: &mut u64) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }

    /// The decisions by sign and by sector are the nearest points of the QPSK and 8PSK sets of DVB-S2.
    #[test]
    fn fast_decisions_are_the_nearest_points() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for (id, rule) in [(4u8, Decide::Qpsk), (13, Decide::Psk8), (18, Decide::Scan)] {
            let cons = constellation(&ModCod::new(id).unwrap());
            assert_eq!(Decide::of(&cons), rule);
            for _ in 0..20_000 {
                let z = Complex32::new(1.5 * noise(&mut seed), 1.5 * noise(&mut seed));
                let want = nearest(&cons, z);
                let got = match rule {
                    Decide::Qpsk => nearest_qpsk(z),
                    Decide::Psk8 => nearest_8psk(z),
                    Decide::Scan => want,
                };
                // Two points at the same distance within rounding are both right.
                let (far, least) = ((z - got).norm_sqr(), (z - want).norm_sqr());
                assert!((far - least).abs() < 1e-5, "{id} at {z}: {got} for {want}");
                assert!(cons.iter().any(|c| (c - got).norm() < 1e-4));
            }
        }
    }

    /// A frame with a carrier offset, scrambling and pilots comes out as its points, in both spectrum senses, and the
    /// refinement follows a small error of the frequency.
    #[test]
    fn data_run_takes_carrier_and_scrambling_off() {
        let mut seed = 0x0123_4567_89ab_cdefu64;
        let mut scr = Vec::new();
        pl_scrambling(0, 33282, &mut scr);
        for (id, pilots) in [(4u8, false), (13, true), (18, true), (18, false), (26, true), (24, false)] {
            let cons = constellation(&ModCod::new(id).unwrap());
            let slots = 21_600 / 90;
            let n = slots * 90
                + if pilots {
                    (slots - 1) / 16 * PILOT_FIELD
                } else {
                    0
                };
            for conj in [false, true] {
                let (theta, freq, slip) = (0.7, 0.0123, 2e-6);
                let mut want = Vec::new();
                let mut y = Vec::with_capacity(n);
                for i in 0..n {
                    let pilot = pilots && i % (PILOT_EVERY + PILOT_FIELD) >= PILOT_EVERY;
                    let point = if pilot {
                        pl::PILOT
                    } else {
                        let k = ((noise(&mut seed) + 1.0) * 0.5 * cons.len() as f32) as usize;
                        cons[k.min(cons.len() - 1)]
                    };
                    if !pilot {
                        want.push(point);
                    }
                    let v = point
                        * QUARTER[usize::from(scr[i])]
                        * rot(theta + 2.0 * PI * (freq + slip) * i as f64);
                    y.push(if conj { v.conj() } else { v });
                }
                let run = DataRun {
                    y: &y,
                    scr: &scr[..n],
                    conj,
                    theta,
                    freq,
                    pilots,
                };
                assert_eq!(run.symbols(), want.len());
                let mut out = vec![Complex32::default(); want.len()];
                let rule = Decide::of(&cons);
                let mut amp = 1.0;
                let (ph, sp, ep) = match rule {
                    Decide::Qpsk => run.correct(&mut out, nearest_qpsk),
                    Decide::Psk8 => run.correct(&mut out, nearest_8psk),
                    Decide::Scan if pilots => run.correct_by_pilots(&mut out, &cons, &mut amp),
                    Decide::Scan => run.correct_rings(&mut out, &cons, amp),
                };
                assert!((amp - 1.0).abs() < 0.01, "MODCOD {id}: amplitude {amp}");
                let worst = out
                    .iter()
                    .zip(&want)
                    .map(|(a, b)| (a - b).norm())
                    .fold(0f32, f32::max);
                assert!(worst < 0.05, "MODCOD {id} conj {conj}: {worst}");
                // The phase the loop added is the slip of the frequency over the frame.
                let slipped = 2.0 * PI * slip * n as f64;
                assert!((ph - slipped).abs() < 0.03, "{ph} for {slipped}");
                assert!(sdrkit_algo_dsp::meter::power_to_db(sp / ep.max(1e-12)) > 30.0);
            }
        }
    }

    /// A number of unit variance, near enough to normal.
    fn gauss(seed: &mut u64) -> f32 {
        (0..12).map(|_| noise(seed)).sum::<f32>() * 0.5
    }

    /// `x` at a carrier of `freq` cycles per symbol and phase `theta`, with noise at `esn0_db`, in the spectrum sense
    /// `conj`.
    fn on_air(x: &[Complex32], freq: f64, theta: f64, esn0_db: f32, conj: bool, seed: &mut u64) -> Vec<Complex32> {
        let sigma = (10f32.powf(-esn0_db / 10.0) / 2.0).sqrt();
        x.iter()
            .enumerate()
            .map(|(i, &v)| {
                let n = Complex32::new(gauss(seed), gauss(seed)) * sigma;
                let y = v * rot(theta + 2.0 * PI * freq * i as f64) + n;
                if conj {
                    y.conj()
                } else {
                    y
                }
            })
            .collect()
    }

    /// Every PLS code is decoded from its header at Es/N0 0 dB, at a carrier offset, in both spectrum senses; the search
    /// finds the code and the offset; and the SOF sum has the sign of the sense.
    #[test]
    fn pls_codes_decode_at_a_carrier_offset_in_both_senses() {
        let mut seed = 0x1357_9bdf_0246_8aceu64;
        let mut s = Sync::new(Standard::S2, 0);
        let freq = 0.0131;
        for c in 0..128u8 {
            for conj in [false, true] {
                s.buf = on_air(&header_symbols(c), freq, 1.1, 0.0, conj, &mut seed);
                let (got, share) = s.decode_pls(0, conj, freq);
                assert_eq!(got, c, "code {c}, inverted {conj}");
                assert!(share > 0.25, "code {c}: share {share}");
                assert_eq!(s.sof_sum(0).re < 0.0, conj, "code {c}");
                let (got, _, f) = s.search_pls(0, conj);
                assert_eq!(got, c, "code {c}, inverted {conj}, searched");
                assert!((f - freq).abs() < 1.0 / 360.0, "code {c}: {f}");
            }
        }
    }

    /// The carrier frequency of a frame of each constellation with rings comes from its two headers and its data, within
    /// 2 parts in a million of a cycle per symbol, from a start that is wrong by several turns over the frame; in both
    /// spectrum senses, at an Es/N0 half a decibel over the figure of the code rate.
    #[test]
    fn frame_frequency_from_two_headers() {
        let mut seed = 0x0f1e_2d3c_4b5a_6978u64;
        let mut scr = Vec::new();
        pl_scrambling(0, 33282, &mut scr);
        for (id, short, pilots) in [(18u8, false, false), (18, true, true), (24, false, false), (27, true, false)] {
            let m = ModCod::new(id).unwrap();
            let cons = constellation(&m);
            let code = code7(id, short, pilots);
            let len = m.frame_symbols(short, pilots);
            let mut x = Vec::with_capacity(2 * len);
            for _ in 0..2 {
                x.extend_from_slice(&header_symbols(code));
                for i in 0..len - HEADER {
                    let pilot = pilots && i % (PILOT_EVERY + PILOT_FIELD) >= PILOT_EVERY;
                    let point = if pilot {
                        pl::PILOT
                    } else {
                        let k = ((noise(&mut seed) + 1.0) * 0.5 * cons.len() as f32) as usize;
                        cons[k.min(cons.len() - 1)]
                    };
                    x.push(point * QUARTER[usize::from(scr[i])]);
                }
            }
            for (conj, wrong) in [(false, 4.1e-4), (true, -2.6e-4)] {
                let freq = -0.0072;
                let mut s = Sync::new(Standard::S2, 0);
                s.buf = on_air(&x, freq, 2.0, m.qef_esn0(short) as f32 + 0.5, conj, &mut seed);
                let near = s.header_freq(&[(0, code), (len, code)], conj);
                assert!((near - freq).abs() < FREQ_DOUBT, "MODCOD {id}: {near} from the headers");
                let f = s.frame_freq((0, code), (len, code), conj, freq + wrong);
                assert!((f - freq).abs() < 2e-6, "MODCOD {id}, inverted {conj}: {f}");
            }
        }
    }

    /// Frames of QPSK at an Es/N0 of -2 dB, where two headers in a row seldom pass the header metric: the search over
    /// several frames finds a header, the spectrum sense and the carrier frequency; normal and short frames, with and
    /// without pilots.
    #[test]
    fn headers_under_the_noise_are_found_over_several_frames() {
        let mut seed = 0x2468_ace0_1357_9bdfu64;
        let mut scr = Vec::new();
        pl_scrambling(0, 33282, &mut scr);
        for (id, short, pilots, conj) in [(1u8, false, false, false), (1, false, true, true), (2, true, false, true)] {
            let m = ModCod::new(id).unwrap();
            let cons = constellation(&m);
            let code = code7(id, short, pilots);
            let len = m.frame_symbols(short, pilots);
            let lead = 1234;
            let mut point = || {
                let k = ((noise(&mut seed) + 1.0) * 0.5 * cons.len() as f32) as usize;
                cons[k.min(cons.len() - 1)]
            };
            let mut x: Vec<Complex32> = (0..lead).map(|_| point()).collect();
            while x.len() + len <= 150_000 {
                x.extend_from_slice(&header_symbols(code));
                for i in 0..len - HEADER {
                    let pilot = pilots && i % (PILOT_EVERY + PILOT_FIELD) >= PILOT_EVERY;
                    let v = if pilot { pl::PILOT } else { point() };
                    x.push(v * QUARTER[usize::from(scr[i])]);
                }
            }
            // Near the edge of the carrier offsets that the search covers.
            let freq = if conj { -0.09 } else { 0.09 };
            let mut s = Sync::new(Standard::S2, 0);
            s.buf = on_air(&x, freq, 0.7, -2.0, conj, &mut seed);
            let (p, c7, f, _, sense) = s.find_s2().expect("the headers are found");
            assert_eq!(c7, code, "MODCOD {id}");
            assert_eq!((p - lead) % len, 0, "MODCOD {id}: header at {p}");
            assert_eq!(sense, conj, "MODCOD {id}");
            assert!((f - freq).abs() < 2e-5, "MODCOD {id}: {f}");
        }
    }
}
