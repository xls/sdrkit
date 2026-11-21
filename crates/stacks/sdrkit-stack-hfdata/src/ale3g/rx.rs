//! The 3G-ALE burst receiver: the 24 kHz channel through the serial-tone matched filter (10 samples a symbol); detectors of the
//! TLC/AGC sequence and of the BW0, BW1 and BW3 preambles; timing, frequency and the paths of the channel from the known
//! sequence; Walsh symbols despread and combined over the paths; deinterleavers, the Viterbi decoders, the PDUs, the packets and
//! their datagrams.

use super::pdu::{self, Datagram, Reading};
use super::signal::{
    bw0_interleaver, bw1_interleaver, bw2_pn, bw2_tribit, bw3_interleaver, tribits, walsh_chip,
    BW0_PRE, BW1_PRE, BW2_FRAMES, BW3_PRE, CODE_BW1, CODE_BW2, CODE_K7, PN01, PN3, PN4, TLC,
};
use super::{Ale3gRxSetting, BW2_TAPS};
use crate::serial::front::{cis, SerialFront, RATE_IN};
use crate::serial::psk8;
use crate::serial::soft::llr8;
use crate::typed::{
    Ale3gDatagramEvent, Ale3gFields, Ale3gPduEvent, Ale3gPduRow, ALE3G_DATAGRAM, ALE3G_PDU,
    ALE3G_PDUS,
};
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_modem::rake::{self, Path};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, Complex32, ConfigError, LockState, ResultsDecl, Schema, SettingError, SettingId,
    SettingSet, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kind, Outbox, Port, RateFrom, RunContext};
use sdrkit_util_bits::interleave::Permutation;
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

const SPS: u64 = 10;
const STEP: u64 = 5;
const SEG: usize = 16;
/// The coherent segment of the detectors, symbols.
const DET_SEG: usize = 48;
const THRESHOLD: f32 = 0.12;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "The waveforms and PDUs follow MIL-STD-188-141B Appendix C. Not confirmed by a public document: the \
BW2 PN generator (its drawn feedback taps give a period of 21845, not the 65535 its caption names; the taps are a setting), and \
the dwell and slot timing and the thresholds of the stack. BW3 packets of 256 and 512 bytes are not decoded: the printed \
interleaver parameters give no permutation.";

/// The declaration of the results.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(Ale3gFields::SHAPE.fields)
        .with_table(&ALE3G_PDUS)
        .event(&ALE3G_PDU)
        .event(&ALE3G_DATAGRAM)
}

/// A row of the PDU log.
#[derive(Clone, Debug)]
struct Row {
    utc: f64,
    burst: &'static str,
    kind: String,
    good: bool,
    words: String,
    snr: f64,
    offset: f64,
    data: String,
}

/// A known sequence and its detector.
struct Seq {
    pts: Vec<Complex32>,
}

impl Seq {
    fn new(t: &[u8]) -> Seq {
        Seq {
            pts: t.iter().map(|&x| psk8(x)).collect(),
        }
    }
}

/// The most packets of a BW2 burst.
const BW2_MAX: usize = 24;

/// A BW2 burst being read.
struct Bw2Run {
    f: Fit,
    ft: u32,
    mp: Path,
    pn: Vec<u8>,
    g_prev: Option<Complex32>,
    w: f64,
    /// The phase taken out at symbol `k`.
    phi: f64,
    noise: f32,
    good: usize,
    packets: usize,
    k: u64,
}

/// What a burst's known sequence gave: timing, frequency, paths, noise.
#[derive(Clone, Debug)]
struct Fit {
    p: u64,
    w: f64,
    paths: Vec<Path>,
    noise: f32,
    snr: f64,
}

/// The receiver.
pub struct Ale3gDecoder {
    front: SerialFront,
    buf: Vec<Complex32>,
    base: u64,
    scratch: Vec<Complex32>,
    at: u64,
    /// The turns of the five frequency hypotheses over a detector segment.
    hyp: Vec<[Complex32; DET_SEG]>,
    tlc: Seq,
    pre0: Seq,
    pre1: Seq,
    pre3: Seq,
    pn01: Vec<u8>,
    pn3: Vec<u8>,
    pn4: Vec<u8>,
    taps: u16,
    bw2_pre: Vec<Vec<Complex32>>,
    il0: Permutation,
    il1: Permutation,
    il3: [Option<Permutation>; 2],
    bw2_run: Option<Bw2Run>,
    failed3: [Option<Vec<f32>>; 2],
    hdl: Datagram,
    ldl: Datagram,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    rows: VecDeque<Row>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
    text: String,
    bursts: i64,
    pdus: i64,
    snr: Option<f64>,
    offset: Option<f64>,
    status: Status,
}

impl Default for Ale3gDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Max-log soft bits of a 16-ary Walsh metric: bit `b` (first the most significant) of the row index.
fn walsh_soft(m: &[f32; 16], scale: f32, out: &mut [f32; 4]) {
    for (b, o) in out.iter_mut().enumerate() {
        let (mut m0, mut m1) = (f32::MIN, f32::MIN);
        for (n, &v) in m.iter().enumerate() {
            if (n >> (3 - b)) & 1 == 0 {
                m0 = m0.max(v);
            } else {
                m1 = m1.max(v);
            }
        }
        *o = (m0 - m1) * scale;
    }
}

impl Ale3gDecoder {
    /// A receiver with the drawn BW2 taps.
    pub fn new() -> Self {
        let tlc = tribits(TLC);
        let mut d = Ale3gDecoder {
            front: SerialFront::new(1),
            buf: Vec::new(),
            base: 0,
            scratch: Vec::new(),
            at: 0,
            hyp: [-80.0f64, -40.0, 0.0, 40.0, 80.0]
                .iter()
                .map(|&f| {
                    let mut r = [Complex32::default(); DET_SEG];
                    for (j, v) in r.iter_mut().enumerate() {
                        *v = cis(-TAU * f * j as f64 / 2400.0);
                    }
                    r
                })
                .collect(),
            tlc: Seq::new(&tlc[..240]),
            pre0: Seq::new(&tribits(BW0_PRE)),
            pre1: Seq::new(&tribits(BW1_PRE)[..288]),
            pre3: Seq::new(&tribits(BW3_PRE)[..288]),
            pn01: tribits(PN01),
            pn3: tribits(PN3),
            pn4: tribits(PN4),
            taps: super::signal::BW2_TAPS_DRAWN,
            bw2_pre: Vec::new(),
            il0: bw0_interleaver(),
            il1: bw1_interleaver(),
            il3: [bw3_interleaver(64), bw3_interleaver(128)],
            bw2_run: None,
            failed3: [None, None],
            hdl: Datagram::default(),
            ldl: Datagram::default(),
            clock: None,
            utc0: None,
            dial: 0.0,
            rows: VecDeque::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
            text: String::new(),
            bursts: 0,
            pdus: 0,
            snr: None,
            offset: None,
            status: Status::default(),
        };
        d.make_bw2();
        d
    }

    fn make_bw2(&mut self) {
        self.bw2_pre = (0..128)
            .map(|ft| bw2_pn(ft, self.taps, 64).iter().map(|&t| psk8(t)).collect())
            .collect();
    }

    fn end(&self) -> u64 {
        self.base + self.buf.len() as u64
    }

    fn y(&self, n: u64) -> Complex32 {
        self.buf
            .get(n.wrapping_sub(self.base) as usize)
            .copied()
            .unwrap_or_default()
    }

    /// The segmented detector metric of `s` at sample `p`, and whether its four quarters hold like power.
    fn metric(&self, s: &Seq, p: u64) -> (f32, bool) {
        let n = s.pts.len();
        let mut e = 0.0f32;
        let mut q = [0.0f32; 4];
        let mut z = [Complex32::default(); 384];
        for (k, pt) in s.pts.iter().enumerate() {
            let v = self.y(p + SPS * k as u64);
            z[k] = v * pt.conj();
            let pw = v.norm_sqr();
            e += pw;
            q[k * 4 / n] += pw;
        }
        let mut best = 0.0f32;
        for rot in &self.hyp {
            let mut sum = 0.0f32;
            for g in 0..n / DET_SEG {
                let mut c = Complex32::default();
                for (j, r) in rot.iter().enumerate() {
                    c += z[g * DET_SEG + j] * r;
                }
                sum += c.norm_sqr();
            }
            best = best.max(sum);
        }
        let (lo, hi) = q
            .iter()
            .fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
        (best / (DET_SEG as f32 * e).max(1e-20), hi <= 2.0 * lo)
    }

    /// The best position within `span` samples from `p` for `s`.
    fn best(&self, s: &Seq, p: u64, span: u64) -> (u64, f32) {
        let mut best = (p, 0.0f32);
        let mut q = p;
        while q < p + span {
            let (m, _) = self.metric(s, q);
            if m > best.1 {
                best = (q, m);
            }
            q += STEP;
        }
        best
    }

    /// Fine timing, frequency, paths and noise of known sequence `s` near `p`; `max_paths` paths at most.
    fn fit(&self, s: &Seq, p: u64, max_paths: usize) -> Fit {
        let n = s.pts.len();
        let seg_sums = |p: u64, w: f64, seg: usize| -> Vec<Complex32> {
            s.pts
                .chunks(seg)
                .enumerate()
                .map(|(g, ch)| {
                    let mut c = Complex32::default();
                    for (j, pt) in ch.iter().enumerate() {
                        let k = (g * seg + j) as u64;
                        c += self.y(p + SPS * k) * cis(-w * (SPS * k) as f64) * pt.conj();
                    }
                    c
                })
                .collect()
        };
        // Timing over +-3 samples and five frequency hypotheses 40 Hz apart: the largest segment power.
        let mut tp = (p, 0.0f64, 0.0f32);
        for h in -2..=2i32 {
            let wh = TAU * 40.0 * f64::from(h) / RATE_IN;
            for d in 0..=6u64 {
                let q = (p + d).saturating_sub(3);
                let e: f32 = seg_sums(q, wh, SEG).iter().map(|c| c.norm_sqr()).sum();
                if e > tp.2 {
                    tp = (q, wh, e);
                }
            }
        }
        let p = tp.0;
        let c = seg_sums(p, tp.1, SEG);
        let mut acc = Complex32::default();
        for i in 1..c.len() {
            acc += c[i] * c[i - 1].conj();
        }
        let mut w = tp.1 + f64::from(acc.arg()) / (SEG as f64 * SPS as f64);
        for parts in [2usize, 2] {
            let len = n / parts;
            let h = seg_sums(p, w, len);
            if h.len() >= 2 {
                w += f64::from((h[1] * h[0].conj()).arg()) / (len as f64 * SPS as f64);
            }
        }
        // The channel at half-symbol lags over +-6 symbols.
        let mut prof = Vec::with_capacity(25);
        for i in 0..25i64 {
            let lag = -60 + 5 * i;
            let mut g = Complex32::default();
            for (k, pt) in s.pts.iter().enumerate() {
                let m = (p as i64 + lag + (SPS * k as u64) as i64).max(0) as u64;
                g += self.y(m) * cis(-w * (SPS * k as u64) as f64) * pt.conj();
            }
            prof.push(g / n as f32);
        }
        let paths = rake::select(&prof, -60, 5, max_paths, 0.2, 10);
        let gain: f32 = paths.iter().map(|q| q.gain.norm_sqr()).sum();
        let mut e = 0.0f32;
        for k in 0..n {
            e += self.y(p + SPS * k as u64).norm_sqr();
        }
        let noise = (e / n as f32 - gain).max(1e-6 * gain.max(1e-12));
        Fit {
            p,
            w,
            paths,
            noise,
            snr: 10.0 * f64::from(gain / noise).log10(),
        }
    }

    /// The despread chip `k` of a burst (`p` the start of its known sequence) on path `q`.
    fn chip(&self, f: &Fit, q: &Path, k: u64) -> Complex32 {
        let n = (f.p as i64 + q.lag as i64 + (SPS * k) as i64).max(0) as u64;
        self.y(n) * cis(-f.w * (SPS * k) as f64)
    }

    /// The soft bits of `count` 16-ary Walsh symbols of `len` chips from chip `k0`, on the PN `pn` from chip offset `pn0`,
    /// with the paths of `f` (their gains updated by the decisions).
    fn walsh_symbols(
        &self,
        f: &mut Fit,
        k0: u64,
        count: usize,
        len: usize,
        pn: &[u8],
        pn0: usize,
    ) -> Vec<f32> {
        let mut out = Vec::with_capacity(4 * count);
        let gain: f32 = f
            .paths
            .iter()
            .map(|q| q.gain.norm_sqr())
            .sum::<f32>()
            .max(1e-12);
        let sigma2 = (f.noise / gain).max(1e-3);
        let mut wv = vec![[Complex32::default(); 16]; f.paths.len()];
        for i in 0..count {
            for (pi, q) in f.paths.iter().enumerate() {
                let mut x = [Complex32::default(); 16];
                for j in 0..len {
                    let k = k0 + (len * i + j) as u64;
                    let v = self.chip(f, q, k) * psk8(pn[(pn0 + len * i + j) % pn.len()]).conj();
                    x[j % 16] += v;
                }
                // The 16 Walsh correlations of the folded chips.
                for (n, wn) in wv[pi].iter_mut().enumerate() {
                    let mut s = Complex32::default();
                    for (j, xj) in x.iter().enumerate() {
                        if walsh_chip(n, j) == 0 {
                            s += xj;
                        } else {
                            s -= xj;
                        }
                    }
                    *wn = s;
                }
            }
            let mut m = [0.0f32; 16];
            for (n, mv) in m.iter_mut().enumerate() {
                let mut z = Complex32::default();
                for (pi, q) in f.paths.iter().enumerate() {
                    z += q.gain.conj() * wv[pi][n];
                }
                *mv = z.re / gain;
            }
            let best = (0..16).max_by(|&a, &b| m[a].total_cmp(&m[b])).unwrap_or(0);
            for (pi, q) in f.paths.iter_mut().enumerate() {
                q.gain = q.gain * 0.5 + wv[pi][best] / len as f32 * 0.5;
            }
            let mut s = [0.0f32; 4];
            walsh_soft(&m, 1.0 / (len as f32 * sigma2), &mut s);
            out.extend_from_slice(&s);
        }
        out
    }

    fn utc(&self, t: f64) -> f64 {
        self.utc0.unwrap_or(0.0) + t
    }

    fn log(
        &mut self,
        out: &mut Outbox,
        t: f64,
        burst: &'static str,
        r: &Reading,
        f: &Fit,
        data: String,
    ) {
        let utc = self.utc(t);
        let offset = f.w * RATE_IN / TAU;
        self.snr = Some(f.snr);
        self.offset = Some(offset);
        if r.good {
            self.pdus += 1;
            let line = format!("{burst} {}: {}", r.kind, r.text);
            self.text.push_str(&line);
            if !data.is_empty() {
                self.text.push(' ');
                self.text.push_str(&data);
            }
            self.text.push('\n');
            if self.text.len() > 8000 {
                let cut = self.text.len() - 6000;
                let cut = (cut..self.text.len())
                    .find(|&i| self.text.is_char_boundary(i))
                    .unwrap_or(0);
                self.text.drain(..cut);
            }
            let e = ALE3G_PDU.event(
                utc,
                self.dial,
                &Ale3gPduEvent {
                    text: &line,
                    burst,
                    r#type: r.kind,
                },
            );
            out.messages(0).push(e);
        }
        self.rows.push_back(Row {
            utc,
            burst,
            kind: r.kind.to_string(),
            good: r.good,
            words: r.text.clone(),
            snr: (f.snr * 10.0).round() / 10.0,
            offset: (offset * 10.0).round() / 10.0,
            data,
        });
        self.new_rows += 1;
        while self.rows.len() > ALE3G_PDUS.cap {
            self.rows.pop_front();
        }
    }

    fn datagram(&mut self, out: &mut Outbox, t: f64, kind: &str, d: &[u8], parts: usize) {
        let line = format!(
            "{kind} datagram complete, {parts} packet{}, {} bytes: {}",
            if parts == 1 { "" } else { "s" },
            d.len(),
            pdu::show_data(d)
        );
        self.text.push_str(&line);
        self.text.push('\n');
        let e = ALE3G_DATAGRAM.event(self.utc(t), self.dial, &Ale3gDatagramEvent { text: &line });
        out.messages(0).push(e);
    }

    /// Searches from `self.at`, or goes on with a BW2 burst; returns false when more samples are needed.
    fn step(&mut self, out: &mut Outbox, finishing: bool) -> bool {
        if let Some(mut run) = self.bw2_run.take() {
            return match self.bw2_packet(out, &mut run, finishing) {
                Some(true) => {
                    self.bw2_run = Some(run);
                    true
                }
                Some(false) => {
                    self.bw2_run = Some(run);
                    false
                }
                None => {
                    self.bw2_end(&run);
                    true
                }
            };
        }
        let p = self.at;
        if p + SPS * 640 > self.end() {
            return false;
        }
        for kind in 0..4 {
            let s = match kind {
                0 => &self.pre0,
                1 => &self.pre1,
                2 => &self.pre3,
                _ => &self.tlc,
            };
            let (m, even) = self.metric(s, p);
            if m < THRESHOLD || !even {
                continue;
            }
            let span = SPS * s.pts.len() as u64 / 2;
            // The symbols after the start of the known sequence that the burst needs.
            let after: u64 = match kind {
                0 => 384 + 832,
                1 => 576 + 2304,
                2 => 640 + 16 * 272,
                _ => 256 + 1280,
            };
            if p + span + SPS * (after + 8) > self.end() && !finishing {
                return false;
            }
            let s = match kind {
                0 => &self.pre0,
                1 => &self.pre1,
                2 => &self.pre3,
                _ => &self.tlc,
            };
            let (q, _) = self.best(s, p, span);
            match kind {
                0 => self.bw0(out, q),
                1 => self.bw1(out, q),
                2 => self.bw3(out, q, finishing),
                _ => self.after_tlc(out, q),
            }
            return true;
        }
        self.at += STEP;
        true
    }

    fn bw0(&mut self, out: &mut Outbox, q: u64) {
        let pre = Seq {
            pts: self.pre0.pts.clone(),
        };
        let mut f = self.fit(&pre, q, 1);
        let t = f.p as f64 / RATE_IN;
        let soft = self.walsh_symbols(&mut f, 384, 13, 64, &self.pn01.clone(), 0);
        let bits = decode_tail_biting(&CODE_K7, &self.il0, &soft, 26);
        let r = pdu::read_bw0(&bits);
        let end = f.p + SPS * (384 + 832);
        if (!r.good && f.snr < 3.0) || f.snr < -10.0 {
            self.at = q + STEP;
            return;
        }
        self.bursts += 1;
        self.log(out, t, "BW0", &r, &f, String::new());
        self.at = end;
    }

    fn bw1(&mut self, out: &mut Outbox, q: u64) {
        let pre = Seq {
            pts: self.pre1.pts.clone(),
        };
        let mut f = self.fit(&pre, q, 3);
        let t = f.p as f64 / RATE_IN;
        let soft = self.walsh_symbols(&mut f, 576, 36, 64, &self.pn01.clone(), 0);
        let bits = decode_tail_biting(&CODE_BW1, &self.il1, &soft, 48);
        let r = pdu::read_bw1(&bits);
        if (!r.good && f.snr < 3.0) || f.snr < -10.0 {
            self.at = q + STEP;
            return;
        }
        self.bursts += 1;
        self.log(out, t, "BW1", &r, &f, String::new());
        self.at = f.p + SPS * (576 + 2304);
    }

    fn bw3(&mut self, out: &mut Outbox, q: u64, finishing: bool) {
        let pre = Seq {
            pts: self.pre3.pts.clone(),
        };
        let f = self.fit(&pre, q, 3);
        let t = f.p as f64 / RATE_IN;
        if f.snr < -10.0 {
            self.at = q + STEP;
            return;
        }
        let pn3 = self.pn3.clone();
        for (si, n) in [64usize, 128].into_iter().enumerate() {
            let nsym = 2 * n + 16;
            let end = f.p + SPS * (640 + 16 * nsym as u64);
            if end > self.end() && !finishing {
                break;
            }
            let Some(il) = self.il3[si].clone() else {
                continue;
            };
            let mut g = f.clone();
            let soft = self.walsh_symbols(&mut g, 640, nsym, 16, &pn3, 0);
            let mut sent = vec![0.0f32; soft.len()];
            if il.invert(&soft, &mut sent).is_err() {
                continue;
            }
            let mut tries: Vec<(Vec<f32>, Vec<f32>)> = vec![
                (sent.clone(), vec![0.0; sent.len()]),
                (vec![0.0; sent.len()], sent.clone()),
            ];
            if let Some(old) = &self.failed3[si] {
                tries.push((old.clone(), sent.clone()));
                tries.push((sent.clone(), old.clone()));
            }
            for (a, b) in tries {
                let bits = decode_terminated(&CODE_K7, &[&a, &b], 8 * n + 57);
                let p = pdu::read_packet(&bits, n, 8);
                if p.good {
                    self.failed3[si] = None;
                    self.bursts += 1;
                    let r = Reading {
                        kind: "LDL_DATA",
                        good: true,
                        text: format!("{n} bytes, sequence {:05X}", p.seq),
                    };
                    let shown = pdu::show_data(&p.data);
                    self.log(out, t, "BW3", &r, &f, shown);
                    if let Some(d) = self.ldl.push(&p, 10) {
                        let parts = if (p.seq >> 15) & 3 == 3 {
                            1
                        } else {
                            self.ldl.parts().max(1)
                        };
                        self.datagram(out, t, "LDL", &d, parts);
                    }
                    self.at = end;
                    return;
                }
            }
            self.failed3[si] = Some(sent);
        }
        if f.snr >= 3.0 {
            self.bursts += 1;
            let r = Reading {
                kind: "LDL_DATA",
                good: false,
                text: "BW3 burst, CRC bad".into(),
            };
            self.log(out, t, "BW3", &r, &f, String::new());
        }
        self.at = q + SPS * 640;
    }

    /// After a TLC/AGC sequence: a BW2 burst when its PN preamble follows, else a BW4 burst when a word stands out, else the
    /// sequence belongs to another burst.
    fn after_tlc(&mut self, out: &mut Outbox, q: u64) {
        let tlc = Seq {
            pts: self.tlc.pts.clone(),
        };
        let f = self.fit(&tlc, q, 3);
        // BW2: the 64-symbol preamble of each FTcount.
        let mut e = 0.0f32;
        let mut cs: Vec<f32> = Vec::with_capacity(128);
        let main = f.paths.first().copied();
        if let Some(mp) = main {
            for k in 0..64u64 {
                e += self.chip(&f, &mp, 240 + k).norm_sqr();
            }
            for pre in &self.bw2_pre {
                let mut c = Complex32::default();
                for (k, pt) in pre.iter().enumerate() {
                    c += self.chip(&f, &mp, 240 + k as u64) * pt.conj();
                }
                cs.push(c.norm_sqr() / (64.0 * e).max(1e-20));
            }
        }
        let mut order: Vec<usize> = (0..cs.len()).collect();
        order.sort_by(|&a, &b| cs[b].total_cmp(&cs[a]));
        if order.len() >= 2 && cs[order[0]] >= 0.55 && cs[order[0]] > 1.6 * cs[order[1]] {
            self.bw2(out, f, order[0] as u32);
            return;
        }
        // BW4: 20 segments of 64 chips, each word in power over the paths.
        let mut pw = [0.0f32; 4];
        let mut et = 0.0f32;
        for seg in 0..20u64 {
            let mut es = 0.0f32;
            let mut segw = [0.0f32; 4];
            for q2 in &f.paths {
                let mut c = [Complex32::default(); 4];
                for j in 0..64u64 {
                    let k = 256 + 64 * seg + j;
                    let v = self.chip(&f, q2, k) * psk8(self.pn4[(64 * seg + j) as usize]).conj();
                    es += v.norm_sqr();
                    for (word, cw) in c.iter_mut().enumerate() {
                        if walsh_chip(word, (j % 4) as usize) == 0 {
                            *cw += v;
                        } else {
                            *cw -= v;
                        }
                    }
                }
                for (word, s) in segw.iter_mut().enumerate() {
                    *s += c[word].norm_sqr();
                }
            }
            et += es;
            for (a, b) in pw.iter_mut().zip(segw) {
                *a += b;
            }
        }
        let mut ord: Vec<usize> = (0..4).collect();
        ord.sort_by(|&a, &b| pw[b].total_cmp(&pw[a]));
        let best = pw[ord[0]] / (64.0 * et).max(1e-20);
        if best > 0.1 && pw[ord[0]] > 4.0 * pw[ord[1]] && f.snr >= -10.0 {
            self.bursts += 1;
            let w = ord[0];
            let kind = if w == 3 {
                "LDL_ACK / LDL_EOM"
            } else {
                "LDL_ACK"
            };
            let r = Reading {
                kind,
                good: true,
                text: format!("{}{}: {}", w >> 1, w & 1, pdu::bw4_name(w)),
            };
            let t = f.p as f64 / RATE_IN;
            self.log(out, t, "BW4", &r, &f, String::new());
            self.at = f.p + SPS * 1536;
            return;
        }
        // The TLC/AGC sequence of a BW0 or BW1 burst: on to its preamble. A weak one was noise.
        self.at = if f.snr >= -10.0 {
            q + SPS * 240
        } else {
            q + STEP
        };
    }

    fn bw2(&mut self, _out: &mut Outbox, f: Fit, ft: u32) {
        let Some(mp) = f.paths.first().copied() else {
            self.at = f.p + SPS * 240;
            return;
        };
        let w = f.w;
        let noise = f.noise.max(1e-6);
        self.bw2_run = Some(Bw2Run {
            pn: bw2_pn(ft, self.taps, 64 + BW2_MAX * BW2_FRAMES * 48),
            f,
            ft,
            mp,
            g_prev: None,
            phi: w * (SPS * 304) as f64,
            w,
            noise,
            good: 0,
            packets: 0,
            k: 304,
        });
    }

    fn bw2_sample(&self, r: &Bw2Run, k: u64) -> Complex32 {
        let n = (r.f.p as i64 + r.mp.lag as i64 + (SPS * k) as i64).max(0) as u64;
        self.y(n) * cis(-(r.phi + r.w * (SPS as f64) * (k as f64 - r.k as f64)))
    }

    /// The next packet of a BW2 burst: `Some(true)` read, `Some(false)` waiting for samples, `None` the burst ended.
    fn bw2_packet(&mut self, out: &mut Outbox, r: &mut Bw2Run, finishing: bool) -> Option<bool> {
        let q = r.packets;
        if q >= BW2_MAX {
            return None;
        }
        if r.f.p + SPS * (r.k + 960 + 48) > self.end() {
            return if finishing { None } else { Some(false) };
        }
        let pnk = |r: &Bw2Run, k: u64| r.pn[k as usize - 240];
        if [3, 6, 12].contains(&q) {
            // The burst goes on only when the next frame's known symbols are there.
            let mut c = Complex32::default();
            let mut e = 0.0f32;
            for i in 32..48u64 {
                let v = self.bw2_sample(r, r.k + i);
                c += v * psk8(pnk(r, r.k + i)).conj();
                e += v.norm_sqr();
            }
            if c.norm_sqr() / (16.0 * e).max(1e-20) < 0.3 {
                return None;
            }
        }
        let mut soft = Vec::with_capacity(1920);
        for _ in 0..BW2_FRAMES {
            let k = r.k;
            let mut c = Complex32::default();
            for i in 32..48u64 {
                c += self.bw2_sample(r, k + i) * psk8(pnk(r, k + i)).conj();
            }
            let g = c / 16.0;
            let turn = r.g_prev.map_or(0.0, |gp| f64::from((g * gp.conj()).arg()));
            let mut ne = 0.0f32;
            for i in 32..48u64 {
                ne += (self.bw2_sample(r, k + i) - g * psk8(pnk(r, k + i))).norm_sqr();
            }
            r.noise = 0.8 * r.noise + 0.2 * ne / 16.0;
            let gp = r.g_prev.unwrap_or(g);
            let s = 1.0 / r.noise.max(1e-6);
            for i in 0..32u64 {
                let x = (i as f32 + 8.5) / 48.0;
                let h = gp * (1.0 - x) + g * x;
                let y = self.bw2_sample(r, k + i);
                let mut d = [f32::MAX; 8];
                for (m, dm) in d.iter_mut().enumerate() {
                    *dm = (y - h * psk8(bw2_tribit(m as u8, r.ft) + pnk(r, k + i))).norm_sqr();
                }
                for b in 0..3 {
                    let (mut d0, mut d1) = (f32::MAX, f32::MAX);
                    for (m, &dm) in d.iter().enumerate() {
                        if (m >> (2 - b)) & 1 == 0 {
                            d0 = d0.min(dm);
                        } else {
                            d1 = d1.min(dm);
                        }
                    }
                    soft.push((d1 - d0) * s);
                }
            }
            r.g_prev = Some(g);
            r.phi += r.w * (48 * SPS) as f64;
            r.w += 0.2 * turn / (48.0 * SPS as f64);
            r.k += 48;
        }
        let peak = soft.iter().map(|v| v.abs()).fold(1e-6f32, f32::max);
        let mut llr = vec![0i8; 4 * 1920];
        for (i, v) in soft.iter().enumerate() {
            llr[4 * i + (r.ft % 4) as usize] = llr8(*v, 100.0 / peak);
        }
        let mut bits = vec![0u8; 1913];
        let ok = Viterbi::new(CODE_BW2, 1920)
            .and_then(|mut v| v.decode_only(&llr, Ends::Zero, &mut bits))
            .is_ok();
        r.packets += 1;
        let p = pdu::read_packet(&bits, 233, 0);
        let good = ok && p.good;
        if good {
            r.good += 1;
        }
        let reading = Reading {
            kind: "HDL_DATA",
            good,
            text: format!("packet {}, sequence {:05X}", q + 1, p.seq),
        };
        let shown = if good {
            pdu::show_data(&p.data)
        } else {
            String::new()
        };
        let fx = Fit {
            w: r.w,
            ..r.f.clone()
        };
        let t = r.f.p as f64 / RATE_IN;
        self.log(out, t, "BW2", &reading, &fx, shown);
        if good {
            if let Some(d) = self.hdl.push(&p, 9) {
                let parts = (d.len().div_ceil(233)).max(1);
                self.datagram(out, t, "HDL", &d, parts);
            }
        }
        Some(true)
    }

    fn bw2_end(&mut self, r: &Bw2Run) {
        self.bursts += 1;
        let line = format!(
            "forward transmission {}: {} packets, {} with a good CRC",
            r.ft, r.packets, r.good
        );
        self.text.push_str(&line);
        self.text.push('\n');
        self.at = r.f.p + SPS * r.k;
    }

    fn trim(&mut self) {
        let keep = self.at.saturating_sub(SPS * 400);
        if keep > self.base + 480_000 {
            let n = ((keep - self.base) as usize).min(self.buf.len());
            self.buf.drain(..n);
            self.base += n as u64;
        }
    }

    fn reset(&mut self) {
        self.front.reset();
        self.base += self.buf.len() as u64;
        self.buf.clear();
        self.at = self.base;
        self.failed3 = [None, None];
        self.bw2_run = None;
    }
}

/// Deinterleaves the soft bits of a tail-biting burst (positive favours 0) and decodes `k` bits.
pub fn decode_tail_biting(
    code: &sdrkit_algo_fec::conv::ConvCode,
    il: &Permutation,
    soft: &[f32],
    k: usize,
) -> Vec<u8> {
    let mut d = vec![0.0f32; soft.len()];
    if il.invert(soft, &mut d).is_err() {
        return vec![0; k];
    }
    let peak = d.iter().map(|v| v.abs()).fold(1e-6f32, f32::max);
    let llr: Vec<i8> = d.iter().map(|&v| llr8(v, 100.0 / peak)).collect();
    let mut out = vec![0u8; k];
    let _ = Viterbi::new(*code, k + 8)
        .and_then(|mut v| v.decode_only(&llr, Ends::TailBiting, &mut out));
    out
}

/// Decodes a terminated stream whose outputs `outs[i]` (one soft value a step each, 0 where not sent) give `k` bits.
fn decode_terminated(
    code: &sdrkit_algo_fec::conv::ConvCode,
    outs: &[&Vec<f32>],
    k: usize,
) -> Vec<u8> {
    let n = code.n();
    let steps = outs.first().map_or(0, |o| o.len());
    let peak = outs
        .iter()
        .flat_map(|o| o.iter())
        .map(|v| v.abs())
        .fold(1e-6f32, f32::max);
    let mut llr = vec![0i8; n * steps];
    for (i, o) in outs.iter().enumerate().take(n) {
        for (s, &v) in o.iter().enumerate() {
            llr[n * s + i] = llr8(v, 100.0 / peak);
        }
    }
    let mut out = vec![0u8; steps.saturating_sub(code.k() as usize - 1).max(k)];
    let _ =
        Viterbi::new(*code, steps + 8).and_then(|mut v| v.decode_only(&llr, Ends::Zero, &mut out));
    out.truncate(k);
    out
}

impl sdrkit_graph::Decoder for Ale3gDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfdata.ale3g"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        Ale3gRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(Ale3gRxSetting::Bw2Taps, Value::I64(i64::from(self.taps)))])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Ale3gRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Ale3gRxSetting::Bw2Taps => {
                self.taps = BW2_TAPS.read(id, value)? as u16;
                self.make_bw2();
            }
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
            self.utc0 = Some(now - self.end() as f64 / RATE_IN);
        }
        self.scratch.clear();
        self.front.process(x, &mut self.scratch);
        self.buf.extend_from_slice(&self.scratch);
        self.at = self.at.max(self.base);
        while self.step(out, false) {}
        self.trim();
    }
    fn finish(&mut self, out: &mut Outbox) {
        // Pad with silence so that a burst at the end is read.
        let pad = vec![Complex32::default(); (SPS * 5000) as usize];
        self.buf.extend_from_slice(&pad);
        while self.step(out, true) {}
    }
    fn report(&mut self, res: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            res.clear_rows(&ALE3G_PDUS);
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        for (k, r) in self.rows.iter().skip(start).enumerate() {
            let key = format!("{}", self.serial + k as u64);
            ALE3G_PDUS.put(
                res,
                &key,
                &Ale3gPduRow {
                    utc: Time(r.utc),
                    burst: r.burst,
                    r#type: &r.kind,
                    crc_good: r.good,
                    words: &r.words,
                    snr_db: r.snr,
                    offset_hz: Hertz(r.offset),
                    data: &r.data,
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        res.put(&Ale3gFields {
            note: NOTE_TEXT,
            text: &self.text,
            bursts_total: self.bursts,
            pdus_good: self.pdus,
            snr_db: self.snr.filter(|v| v.is_finite()),
            offset_hz: self.offset.filter(|v| v.is_finite()).map(Hertz),
        });
        self.status.lock_state = if self.pdus > 0 {
            LockState::Locked
        } else {
            LockState::Searching
        };
        self.status.summary = format!("3G-ALE: {} bursts, {} PDUs good", self.bursts, self.pdus);
        self.status.snr_db = self.snr.filter(|v| v.is_finite()).map(|v| v as f32);
        self.status.offset_hz = self.offset.filter(|v| v.is_finite());
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.new_rows = 0;
        self.text.clear();
        self.bursts = 0;
        self.pdus = 0;
        self.cleared = true;
        self.hdl = Datagram::default();
        self.ldl = Datagram::default();
    }
}
