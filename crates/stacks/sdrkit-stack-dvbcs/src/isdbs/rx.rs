//! The ISDB-S receiver core: frame sync, the carrier, the header Viterbi, the TMCC and the frame decoder (one trellis over the
//! carrying slots, byte de-interleaving over 8 frames, energy dispersal and RS(204,188)).
//!
//! Standard: ITU-R BO.1408-1.

use super::tx::*;
use sdrkit_algo_fec::conv::{Ends, LabelMetrics, TrellisDecoder};
use sdrkit_algo_fec::rs::Rs;
use sdrkit_core::Complex32;
use sdrkit_proto_dvbs2::inner::{encoder, INNER};
use sdrkit_proto_mpegts::outer::rs as outer_code;
use std::f64::consts::PI;

/// The known coded symbols of W1 (header symbols 12 to 31) and W2 (172 to 191).
pub fn sync_patterns() -> ([f32; 20], [f32; 20]) {
    let mut a = [0f32; 20];
    let mut b = [0f32; 20];
    for (w, out) in [(W1, &mut a), (W2, &mut b)] {
        let mut e = encoder();
        for i in 0..16 {
            let o = e.push_bit(((w >> (15 - i)) & 1) as u8);
            if i >= 6 {
                out[2 * (i - 6)] = if o & 1 == 0 { 1.0 } else { -1.0 };
                out[2 * (i - 6) + 1] = if o & 2 == 0 { 1.0 } else { -1.0 };
            }
        }
    }
    (a, b)
}

/// Scale of the soft metrics into the integer metrics of the trellis decoder.
const METRIC_SCALE: f32 = 256.0;
/// The largest integer metric of one branch.
const METRIC_MAX: f32 = (1 << 23) as f32;

/// A soft trellis over one frame for the decoder of `sdrkit-algo-fec`: per step four branch metrics (by the code bits x and y) and an
/// optional forced input bit.
pub struct FrameTrellis {
    t: sdrkit_algo_fec::conv::Trellis,
    dec: Option<TrellisDecoder>,
    cap: usize,
    /// Four metrics per step, by label `x | y << 1` (the label order of `sdrkit-algo-fec`).
    metrics: Vec<i32>,
    forced: Vec<u8>,
}

impl Default for FrameTrellis {
    fn default() -> Self {
        FrameTrellis {
            // The K = 7 code is a constant, so its trellis always exists.
            t: sdrkit_algo_fec::conv::Trellis::from_code(&INNER)
                .unwrap_or_else(|_| unreachable!("the K = 7 trellis is valid")),
            dec: None,
            cap: 0,
            metrics: Vec::new(),
            forced: Vec::new(),
        }
    }
}

/// A soft metric as an integer metric. A value that is not finite counts as no information.
fn metric(v: f32) -> i32 {
    if v.is_finite() {
        (v * METRIC_SCALE).clamp(-METRIC_MAX, METRIC_MAX) as i32
    } else {
        0
    }
}

impl FrameTrellis {
    /// Starts a new frame.
    pub fn clear(&mut self) {
        self.metrics.clear();
        self.forced.clear();
    }
    /// A step with separable soft values of x and y (positive means 0) and a forced input bit (negative for none).
    pub fn pair(&mut self, sx: f32, sy: f32, forced: i8) {
        // Label x | y << 1: 0 is (0, 0), 1 is (1, 0), 2 is (0, 1), 3 is (1, 1).
        self.metrics.extend_from_slice(&[
            metric(sx + sy),
            metric(-sx + sy),
            metric(sx - sy),
            metric(-sx - sy),
        ]);
        self.forced
            .push(if forced < 0 { 0xFF } else { forced as u8 });
    }
    /// A step with a table of metrics by `(x << 1) | y`.
    pub fn table(&mut self, t: [f32; 4]) {
        self.metrics
            .extend_from_slice(&[metric(t[0]), metric(t[2]), metric(t[1]), metric(t[3])]);
        self.forced.push(0xFF);
    }
    /// Steps.
    pub fn len(&self) -> usize {
        self.forced.len()
    }
    /// No steps.
    pub fn is_empty(&self) -> bool {
        self.forced.is_empty()
    }
    /// Decodes from `start` (a state, or every state when `None`) and writes the input bit and the state before each step.
    pub fn decode(&mut self, start: Option<u32>, bits: &mut Vec<u8>, states: &mut Vec<u8>) {
        let n = self.len();
        bits.clear();
        bits.resize(n, 0);
        states.clear();
        states.resize(n, 0);
        if self.dec.is_none() || self.cap < n {
            self.cap = n.max(4096);
            match TrellisDecoder::new(&self.t, self.cap) {
                Ok(d) => self.dec = Some(d),
                Err(_) => return,
            }
        }
        let Some(dec) = self.dec.as_mut() else { return };
        let ends = match start {
            Some(s) => Ends::Start(s),
            None => Ends::Open,
        };
        let mut m = LabelMetrics {
            labels: 4,
            metrics: &self.metrics,
        };
        if dec
            .decode(&mut m, n, ends, Some(&self.forced), bits)
            .is_err()
        {
            return;
        }
        let mut e = sdrkit_algo_fec::conv::Encoder::with_state(INNER, start.unwrap_or(0));
        for (b, st) in bits.iter().zip(states.iter_mut()) {
            *st = e.state() as u8;
            e.push_bit(*b);
        }
    }
    /// The code bits (x, y) of input `u` from state `s`.
    pub fn outputs(&self, s: u8, u: u8) -> (u8, u8) {
        let mut e = sdrkit_algo_fec::conv::Encoder::with_state(INNER, s as u32);
        let o = e.push_bit(u);
        (o & 1, (o >> 1) & 1)
    }
}

/// What the decoder found.
#[derive(Clone, Debug, Default)]
pub struct DecodeStats {
    /// TMCC good and bad.
    pub tmcc_good: u64,
    /// Bad.
    pub tmcc_bad: u64,
    /// RS clean, corrected, failed.
    pub rs_clean: u64,
    /// Corrected.
    pub rs_corrected: u64,
    /// Failed.
    pub rs_failed: u64,
    /// Super-frames.
    pub superframes: u64,
}

/// The frame decoder: header, TMCC, data, outer code.
pub struct Decoder {
    trellis: FrameTrellis,
    bits: Vec<u8>,
    states: Vec<u8>,
    blocks: [[u8; 64]; SF],
    have: [bool; SF],
    /// The TMCCs decoded, newest last (with the super-frame they came in).
    pub tmccs: Vec<(u64, Tmcc)>,
    /// The layout in use.
    pub current: Option<Tmcc>,
    sf_index: u64,
    frame_in_sf: Option<usize>,
    slots_tx: Vec<Vec<Option<[u8; 203]>>>,
    sf_layout: Option<Tmcc>,
    prbs: Vec<u8>,
    rs: Rs,
    /// The selected stream (-1: the one with the most slots).
    pub stream: i32,
    /// Counters.
    pub stats: DecodeStats,
    /// Log lines.
    pub log: Vec<String>,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder {
            trellis: FrameTrellis::default(),
            bits: Vec::new(),
            states: Vec::new(),
            blocks: [[0; 64]; SF],
            have: [false; SF],
            tmccs: Vec::new(),
            current: None,
            sf_index: 0,
            frame_in_sf: None,
            slots_tx: vec![vec![None; SLOTS]; SF],
            sf_layout: None,
            prbs: sf_prbs(),
            rs: outer_code(),
            stream: -1,
            stats: DecodeStats::default(),
            log: Vec::new(),
        }
    }
}

impl Decoder {
    /// Forgets the frame position (a new lock).
    pub fn resync(&mut self) {
        self.frame_in_sf = None;
        self.have = [false; SF];
    }
    /// The relative stream decoded.
    pub fn selected(&self) -> Option<u8> {
        let t = self.current.as_ref()?;
        if self.stream >= 0 {
            return Some(self.stream as u8);
        }
        let mut count = [0usize; 8];
        for (i, s) in t.slots().iter().enumerate() {
            if s.1 {
                count[t.ts_of_slot[i] as usize & 7] += 1;
            }
        }
        (0..8).max_by_key(|&k| count[k]).map(|k| k as u8)
    }

    /// One frame of carrier-corrected symbols (`w2`: it is the first of a super-frame); packets of the selected stream go to `out`.
    pub fn frame(
        &mut self,
        y: &[Complex32],
        w2: bool,
        k: f32,
        out: &mut impl FnMut([u8; 188], bool),
    ) {
        // Header.
        self.trellis.clear();
        let w = if w2 { W2 } else { !W2 };
        for i in 0..96 {
            let forced = if i < 16 {
                ((W1 >> (15 - i)) & 1) as i8
            } else if i >= 80 {
                ((w >> (95 - i)) & 1) as i8
            } else {
                -1
            };
            self.trellis
                .pair(y[2 * i].re * k, y[2 * i + 1].re * k, forced);
        }
        self.trellis.decode(None, &mut self.bits, &mut self.states);
        let f = if w2 {
            Some(0)
        } else {
            self.frame_in_sf.map(|f| f + 1).filter(|&f| f < SF)
        };
        self.frame_in_sf = f;
        let Some(f) = f else {
            return;
        };
        if f == 0 {
            self.have = [false; SF];
            self.sf_index += 1;
            self.stats.superframes += 1;
            // The layout of this super-frame: the TMCC of two super-frames ago, else the newest.
            let want = self.sf_index.saturating_sub(2);
            self.sf_layout = self
                .tmccs
                .iter()
                .rev()
                .find(|(i, _)| *i <= want)
                .or(self.tmccs.last())
                .map(|t| t.1.clone());
            self.slots_tx = vec![vec![None; SLOTS]; SF];
        }
        self.blocks[f].copy_from_slice(&self.bits[16..80]);
        self.have[f] = true;
        if f == SF - 1 && self.have.iter().all(|&h| h) {
            match tmcc_decode(&self.blocks) {
                Some((t, _)) => {
                    self.stats.tmcc_good += 1;
                    if self.current.as_ref().is_none_or(|c| c.change != t.change) {
                        self.log.push(format!(
                            "TMCC: change {} groups {}",
                            t.change,
                            t.groups
                                .iter()
                                .map(|(m, n)| format!("{} x{n}", m.name()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    self.current = Some(t.clone());
                    self.tmccs.push((self.sf_index, t));
                    if self.tmccs.len() > 4 {
                        self.tmccs.remove(0);
                    }
                }
                None => self.stats.tmcc_bad += 1,
            }
        }
        let Some(layout) = self.sf_layout.clone() else {
            return;
        };
        // Data: one trellis over the carrying slots, from the state the sync word leaves.
        let start = (w & 63) as u32;
        let start = ((start & 1) << 5)
            | ((start >> 1) & 1) << 4
            | ((start >> 2) & 1) << 3
            | ((start >> 3) & 1) << 2
            | ((start >> 4) & 1) << 1
            | ((start >> 5) & 1);
        let slots = layout.slots();
        self.trellis.clear();
        let mut kinds: Vec<(Mc, usize)> = Vec::new(); // per carrying slot: modulation and first step
        let data = |d: usize| y[data_pos(d)];
        let mut s = 0;
        while s < SLOTS {
            let m = slots[s].0;
            let mut e = s;
            while e < SLOTS && slots[e].0 == m {
                e += 1;
            }
            let carrying = (s..e).filter(|&i| slots[i].1).count();
            let base = s * SLOT_SYMS;
            match m {
                Mc::Tc8psk => {
                    for c in 0..carrying {
                        kinds.push((m, self.trellis.len()));
                        for j in 0..SLOT_SYMS {
                            let z = data(base + c * SLOT_SYMS + j);
                            let mut t = [0f32; 4];
                            for (yx, tv) in t.iter_mut().enumerate() {
                                let (xb, yb) = ((yx >> 1) as u8, (yx & 1) as u8);
                                let a = (z * psk8(0, yb, xb).conj()).re;
                                let b = (z * psk8(1, yb, xb).conj()).re;
                                *tv = a.max(b) * k;
                            }
                            self.trellis.table(t);
                        }
                    }
                }
                Mc::Qpsk(r) => {
                    let keep = r.puncture().keep();
                    let mut soft = (0..(e - s) * SLOT_SYMS).flat_map(|j| {
                        let z = data(base + j);
                        [
                            z.re * k * std::f32::consts::SQRT_2,
                            z.im * k * std::f32::consts::SQRT_2,
                        ]
                    });
                    for c in 0..carrying {
                        kinds.push((m, self.trellis.len()));
                        for st in 0..1624 {
                            let j = 2 * ((c * 1624 + st) % (keep.len() / 2));
                            let sx = if keep[j] == 1 {
                                soft.next().unwrap_or(0.0)
                            } else {
                                0.0
                            };
                            let sy = if keep[j + 1] == 1 {
                                soft.next().unwrap_or(0.0)
                            } else {
                                0.0
                            };
                            self.trellis.pair(sx, sy, -1);
                        }
                    }
                }
                Mc::Bpsk12 => {
                    for c in 0..carrying {
                        kinds.push((m, self.trellis.len()));
                        for st in 0..1624 {
                            let d0 = base + 2 * (c * 1624 + st);
                            self.trellis.pair(data(d0).re * k, data(d0 + 1).re * k, -1);
                        }
                    }
                }
            }
            s = e;
        }
        self.trellis
            .decode(Some(start), &mut self.bits, &mut self.states);
        // Packets of the carrying slots.
        let carrying_slots: Vec<usize> = (0..SLOTS).filter(|&i| slots[i].1).collect();
        for (ci, &(m, first)) in kinds.iter().enumerate() {
            let mut p = [0u8; 203];
            let mut bitpos = 0usize;
            let mut put = |b: u8| {
                p[bitpos / 8] |= (b & 1) << (7 - bitpos % 8);
                bitpos += 1;
            };
            match m {
                Mc::Tc8psk => {
                    let slot = carrying_slots[ci];
                    let grp_start = slots.iter().position(|q| q.0 == m).unwrap_or(0);
                    let c_in_group = (grp_start..slot).filter(|&i| slots[i].1).count();
                    for j in 0..SLOT_SYMS {
                        let t = first + j;
                        let u = self.bits[t];
                        let (x, yb) = self.trellis.outputs(self.states[t], u);
                        let z = data(grp_start * SLOT_SYMS + c_in_group * SLOT_SYMS + j);
                        let w0 = (z * psk8(0, yb, x).conj()).re;
                        let w1 = (z * psk8(1, yb, x).conj()).re;
                        put((w1 > w0) as u8);
                        put(u);
                    }
                }
                _ => {
                    for t in first..first + 1624 {
                        put(self.bits[t]);
                    }
                }
            }
            self.slots_tx[f][carrying_slots[ci]] = Some(p);
        }
        if f == SF - 1 {
            self.superframe_done(&layout, out);
        }
    }

    fn superframe_done(&mut self, layout: &Tmcc, out: &mut impl FnMut([u8; 188], bool)) {
        let Some(sel) = self.selected() else {
            return;
        };
        for fr in 0..SF {
            for s in 0..SLOTS {
                if layout.ts_of_slot[s] != sel || self.slots_tx[0][s].is_none() {
                    continue;
                }
                // De-interleave: byte j of frame fr was sent at n = 8 j + fr.
                let mut w = [0u8; 204];
                w[0] = 0x47;
                let mut ok = true;
                for j in 0..203 {
                    let n = 8 * j + fr;
                    match &self.slots_tx[n / 203][s] {
                        Some(p) => {
                            w[1 + j] = p[n % 203] ^ self.prbs[(fr * SLOTS + s) * 204 + 1 + j]
                        }
                        None => ok = false,
                    }
                }
                if !ok {
                    continue;
                }
                let bad = match self.rs.decode(&mut w, &[]) {
                    Ok(d) if d.corrected == 0 => {
                        self.stats.rs_clean += 1;
                        false
                    }
                    Ok(_) => {
                        self.stats.rs_corrected += 1;
                        false
                    }
                    Err(_) => {
                        self.stats.rs_failed += 1;
                        true
                    }
                };
                let mut p = [0u8; 188];
                p.copy_from_slice(&w[..188]);
                p[0] = 0x47;
                if bad {
                    p[1] |= 0x80;
                }
                out(p, bad);
            }
        }
    }
}

/// The carrier frequency (cycles per symbol) from the eighth power.
pub fn coarse_freq(y: &[Complex32]) -> f64 {
    sdrkit_proto_dvbs2::sync::power_freq(y, 8)
}

/// Frame sync metric at `p` (W1 and W2/W3 coded symbols; the sign of the W2 part distinguishes W2 from W3).
pub fn sync_metric(y: &[Complex32], p: usize, a: &[f32; 20], b: &[f32; 20]) -> (f32, bool) {
    let mut s1 = Complex32::new(0.0, 0.0);
    let mut s2 = Complex32::new(0.0, 0.0);
    let mut pw = 0f32;
    for i in 0..20 {
        s1 += y[p + 12 + i] * a[i];
        s2 += y[p + 172 + i] * b[i];
        pw += y[p + 12 + i].norm() + y[p + 172 + i].norm();
    }
    let w2 = (s2 * s1.conj()).re > 0.0;
    ((s1.norm() + s2.norm()) / pw.max(1e-9), w2)
}

/// The phase of a frame's symbols: at W1, then at every burst (squared, the half-turn resolved toward the running phase).
pub fn frame_phases(y: &[Complex32], a: &[f32; 20], b_prev: f64) -> (f64, Vec<f64>) {
    let mut s1 = Complex32::new(0.0, 0.0);
    for i in 0..20 {
        s1 += y[12 + i] * a[i];
    }
    let th = (s1.im as f64).atan2(s1.re as f64);
    let mut out = Vec::with_capacity(192);
    let mut cur = th;
    for r in 0..192 {
        let mut q = Complex32::new(0.0, 0.0);
        for j in 0..4 {
            let v = y[HEADER + r * (RUN + 4) + RUN + j];
            q += v * v;
        }
        let half = (q.im as f64).atan2(q.re as f64) / 2.0;
        let mut best = half;
        for c in [half, half + PI, half - PI] {
            if (wrap(c - cur)).abs() < (wrap(best - cur)).abs() {
                best = c;
            }
        }
        let best = cur + wrap(best - cur);
        out.push(best);
        cur = best;
    }
    let _ = b_prev;
    (th, out)
}

/// Wraps to (-pi, pi].
pub fn wrap(mut a: f64) -> f64 {
    while a > PI {
        a -= 2.0 * PI;
    }
    while a <= -PI {
        a += 2.0 * PI;
    }
    a
}
