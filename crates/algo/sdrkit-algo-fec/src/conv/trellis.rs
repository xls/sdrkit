//! A general trellis decoder for trellis-coded modulation and other trellises whose branch metrics the caller supplies: ATSC
//! 8-VSB, the DMR and P25 rate 3/4 trellis, the P25 rate 1/2 trellis, ISDB-S TC8PSK, and 2- or 4-state ISI trellises for
//! equalisers.
//!
//! A [`Trellis`] lists, for every state and every input symbol, the next state and the branch's label. A [`BranchMetrics`] source
//! supplies a metric per label for each step (larger is better), and the decoder returns the input symbol of every step of the
//! best path. Parallel transitions (an uncoded bit, as in ATSC and TC8PSK) are handled in the metric source.

use super::ConvCode;
use crate::error::{Decoded, FecError, Param, Result};

/// A trellis: `states` states, `inputs` input symbols per step, and for each `(state, input)` the next state and the branch label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trellis {
    states: usize,
    inputs: usize,
    labels: usize,
    next: Vec<u16>,
    label: Vec<u16>,
}

impl Trellis {
    /// A trellis from its tables: `next[s * inputs + x]` is the state after input `x` in state `s`, `label[s * inputs + x]` the
    /// branch label (below `labels`). An error when the sizes do not match, a value is out of range, or a state has no way in.
    pub fn new(
        states: usize,
        inputs: usize,
        labels: usize,
        next: Vec<u16>,
        label: Vec<u16>,
    ) -> Result<Self> {
        Param::States.check(states, 1, 4096)?;
        Param::Inputs.check(inputs, 1, 256)?;
        Param::Labels.check(labels, 1, 65_536)?;
        if next.len() != states * inputs {
            return Param::Table.range(
                (states * inputs) as i64,
                (states * inputs) as i64,
                next.len(),
            );
        }
        if label.len() != states * inputs {
            return Param::Table.range(
                (states * inputs) as i64,
                (states * inputs) as i64,
                label.len(),
            );
        }
        if let Some(&s) = next.iter().find(|&&s| s as usize >= states) {
            return Param::States.range(0, states as i64 - 1, s);
        }
        if let Some(&l) = label.iter().find(|&&l| l as usize >= labels) {
            return Param::Labels.range(0, labels as i64 - 1, l);
        }
        let mut reach = vec![false; states];
        for &s in &next {
            reach[s as usize] = true;
        }
        if reach.iter().any(|&r| !r) {
            return Param::States.degenerate();
        }
        Ok(Trellis {
            states,
            inputs,
            labels,
            next,
            label,
        })
    }

    /// The trellis of a convolutional code: one input bit, label = the outputs (output `i` in bit `i`, inversion applied). With
    /// branch metrics per label this decodes TC8PSK (ARIB STD-B20: the K = 7 code with an uncoded bit) or any code whose metrics
    /// do not come from LLRs. `K` up to 13.
    pub fn from_code(code: &ConvCode) -> Result<Self> {
        let k = code.k();
        if k > 13 {
            return Param::ConstraintLength.range(1, 13, k);
        }
        let s = code.states() as usize;
        let mut next = Vec::with_capacity(2 * s);
        let mut label = Vec::with_capacity(2 * s);
        for st in 0..s as u32 {
            for b in 0..2u32 {
                let reg = (b << (k - 1)) | st;
                next.push((reg >> 1) as u16);
                label.push(code.label(reg) as u16);
            }
        }
        Trellis::new(s, 2, 1 << code.n(), next, label)
    }

    /// The ATSC 8-VSB trellis code (ATSC A/53 Part 2, clause 6.4 and figure 6.x "8-VSB trellis encoder"): the coded bit `X1`
    /// enters a 4-state rate 1/2 encoder giving `Z1 = X1` and `Z0` from its two delay cells. State `(a << 1) | b`: `Z0 = a`, the
    /// next state is `((X1 ^ b) << 1) | a`. Label `(Z1 << 1) | Z0` selects the level pair `{2·label − 7, 2·label + 1}`, `Z2` (the
    /// precoded `X2`) choosing between them. Input symbol = `X1`. Used 12 times interleaved, one per symbol phase.
    pub fn atsc_8vsb() -> Self {
        let mut next = Vec::with_capacity(8);
        let mut label = Vec::with_capacity(8);
        for s in 0..4u16 {
            let (a, b) = (s >> 1, s & 1);
            for x1 in 0..2u16 {
                next.push(((x1 ^ b) << 1) | a);
                label.push((x1 << 1) | a);
            }
        }
        Trellis {
            states: 4,
            inputs: 2,
            labels: 4,
            next,
            label,
        }
    }

    /// A trellis whose state is the last input symbol (the DMR / P25 rate 3/4 trellis with 8 states and inputs, the P25 rate 1/2
    /// trellis with 4): `table[s][x]` is the constellation point sent for input `x` in state `s`, and the next state is `x`.
    pub fn last_input(table: &[&[u8]]) -> Result<Self> {
        let s = table.len();
        if s == 0 || table.iter().any(|r| r.len() != s) {
            return Param::Table.degenerate();
        }
        let labels = table
            .iter()
            .flat_map(|r| r.iter())
            .map(|&v| v as usize + 1)
            .max()
            .unwrap_or(1);
        let mut next = Vec::with_capacity(s * s);
        let mut label = Vec::with_capacity(s * s);
        for row in table {
            for (x, &l) in row.iter().enumerate() {
                next.push(x as u16);
                label.push(l as u16);
            }
        }
        Trellis::new(s, s, labels, next, label)
    }

    /// Number of states.
    pub fn states(&self) -> usize {
        self.states
    }

    /// Input symbols per step.
    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// Number of distinct labels.
    pub fn labels(&self) -> usize {
        self.labels
    }

    /// The next state after input `x` in state `s`.
    pub fn next(&self, s: usize, x: usize) -> usize {
        self.next[s * self.inputs + x] as usize
    }

    /// The label of the branch for input `x` in state `s`.
    pub fn label(&self, s: usize, x: usize) -> usize {
        self.label[s * self.inputs + x] as usize
    }

    /// Encodes input symbols from state `start`, writing each branch label to `labels`; returns the end state.
    pub fn encode(&self, start: usize, inputs: &[u8], labels: &mut [u16]) -> Result<usize> {
        if labels.len() < inputs.len() {
            return Err(FecError::OutputTooShort { need: inputs.len() });
        }
        if start >= self.states {
            return Param::StartState.range(0, self.states as i64 - 1, start);
        }
        let mut s = start;
        for (&x, l) in inputs.iter().zip(labels.iter_mut()) {
            if x as usize >= self.inputs {
                return Param::Inputs.range(0, self.inputs as i64 - 1, x);
            }
            *l = self.label(s, x as usize) as u16;
            s = self.next(s, x as usize);
        }
        Ok(s)
    }
}

/// A source of branch metrics: for step `step`, fill `m` (one value per label) with the metric of each label. Larger is better.
/// Values should stay within ±2^24 so that sums over a frame cannot overflow the comparison.
pub trait BranchMetrics {
    /// Number of labels (the length of `m`).
    fn labels(&self) -> usize;
    /// Writes the metric of every label at step `step`.
    fn fill(&mut self, step: usize, m: &mut [i32]);
}

/// Branch metrics held in a slice: `metrics[step * labels + label]`.
#[derive(Clone, Copy, Debug)]
pub struct LabelMetrics<'a> {
    /// Labels per step.
    pub labels: usize,
    /// The metrics, `labels` per step.
    pub metrics: &'a [i32],
}

impl BranchMetrics for LabelMetrics<'_> {
    fn labels(&self) -> usize {
        self.labels
    }
    fn fill(&mut self, step: usize, m: &mut [i32]) {
        m.copy_from_slice(&self.metrics[step * self.labels..(step + 1) * self.labels]);
    }
}

/// A Viterbi decoder for a [`Trellis`] with caller-supplied branch metrics, sized at construction for `max_steps`.
///
/// ```
/// use sdrkit_algo_fec::conv::{LabelMetrics, Trellis, TrellisDecoder};
/// use sdrkit_algo_fec::conv::Ends;
/// // ATSC 8-VSB: decode a short run of noisy levels.
/// let t = Trellis::atsc_8vsb();
/// let x1 = [1u8, 0, 0, 1, 1, 1, 0, 1, 0, 0];
/// let mut lab = [0u16; 10];
/// t.encode(0, &x1, &mut lab).unwrap();
/// // Level of each label with Z2 = 0, plus a little noise; metric = -(distance^2) to the nearer of the label's two levels.
/// let rx: Vec<f32> = lab.iter().enumerate().map(|(i, &l)| (2 * l as i32 - 7) as f32 + 0.6 * ((i % 3) as f32 - 1.0)).collect();
/// let mut m = Vec::new();
/// for &r in &rx {
///     for l in 0..4 {
///         let d0 = r - (2 * l - 7) as f32;
///         let d1 = d0 - 8.0;
///         m.push(-((d0 * d0).min(d1 * d1) * 64.0) as i32);
///     }
/// }
/// let mut dec = TrellisDecoder::new(&t, 64).unwrap();
/// let mut out = [0u8; 10];
/// dec.decode(&mut LabelMetrics { labels: 4, metrics: &m }, 10, Ends::Start(0), None, &mut out).unwrap();
/// assert_eq!(out, x1);
/// ```
#[derive(Clone, Debug)]
pub struct TrellisDecoder {
    t: Trellis,
    max_steps: usize,
    /// Predecessors of each state: (previous state, input), `pred_start[s]..pred_start[s + 1]`.
    pred: Vec<(u16, u8)>,
    pred_start: Vec<u32>,
    m: Vec<i32>,
    nx: Vec<i32>,
    bm: Vec<i32>,
    /// The index (into the state's predecessor list) of the survivor, per step and state.
    choice: Vec<u8>,
}

impl TrellisDecoder {
    /// A decoder for `t`, for frames up to `max_steps` steps. An error when a state has more than 256 ways in.
    pub fn new(t: &Trellis, max_steps: usize) -> Result<Self> {
        let s = t.states;
        let mut lists: Vec<Vec<(u16, u8)>> = vec![Vec::new(); s];
        for st in 0..s {
            for x in 0..t.inputs {
                lists[t.next(st, x)].push((st as u16, x as u8));
            }
        }
        if lists.iter().any(|l| l.len() > 256) {
            return Param::Inputs.range(0, 256, lists.iter().map(|l| l.len()).max().unwrap_or(0));
        }
        let mut pred = Vec::new();
        let mut pred_start = vec![0u32];
        for l in &lists {
            pred.extend_from_slice(l);
            pred_start.push(pred.len() as u32);
        }
        Ok(TrellisDecoder {
            t: t.clone(),
            max_steps,
            pred,
            pred_start,
            m: vec![0; s],
            nx: vec![0; s],
            bm: vec![0; t.labels],
            choice: vec![0; max_steps * s],
        })
    }

    /// The trellis.
    pub fn trellis(&self) -> &Trellis {
        &self.t
    }

    /// Decodes `steps` steps, writing each step's input symbol to `out`.
    ///
    /// `ends`: [`super::Ends::Start`] fixes the start state, [`super::Ends::Zero`] fixes start and end to state 0 (all steps are
    /// written; tails depend on the trellis), [`super::Ends::Open`] leaves both free; [`super::Ends::TailBiting`] is not supported
    /// here. `forced`, when given, holds one value per step: an input symbol the encoder is known to have used at that step (sync
    /// or known bits), or `0xFF` for a free step. `metric` in the result is the winning path metric relative to the best start.
    pub fn decode(
        &mut self,
        bm: &mut impl BranchMetrics,
        steps: usize,
        ends: super::Ends,
        forced: Option<&[u8]>,
        out: &mut [u8],
    ) -> Result<Decoded> {
        use super::Ends;
        if steps > self.max_steps {
            return Err(FecError::TooLong {
                max: self.max_steps,
            });
        }
        if out.len() < steps {
            return Err(FecError::OutputTooShort { need: steps });
        }
        if bm.labels() != self.t.labels {
            return Param::Labels.conflict(Param::Table);
        }
        if let Some(f) = forced {
            if f.len() < steps {
                return Err(FecError::InputLength {
                    expected: steps,
                    got: f.len(),
                });
            }
        }
        let s = self.t.states;
        const NEG: i32 = i32::MIN / 4;
        let (start, end) = match ends {
            Ends::Zero => (Some(0usize), Some(0usize)),
            Ends::Start(st) => {
                if st as usize >= s {
                    return Param::StartState.range(0, s as i64 - 1, st);
                }
                (Some(st as usize), None)
            }
            Ends::Open => (None, None),
            Ends::TailBiting => return Param::TailBiting.unsupported(),
        };
        match start {
            Some(st) => {
                self.m.fill(NEG);
                self.m[st] = 0;
            }
            None => self.m.fill(0),
        }
        for step in 0..steps {
            bm.fill(step, &mut self.bm);
            let f = forced.map(|f| f[step]).unwrap_or(0xFF);
            let ch = &mut self.choice[step * s..(step + 1) * s];
            for ns in 0..s {
                let lo = self.pred_start[ns] as usize;
                let hi = self.pred_start[ns + 1] as usize;
                let mut best = NEG;
                let mut bi = 0u8;
                for (i, &(ps, x)) in self.pred[lo..hi].iter().enumerate() {
                    if f != 0xFF && x != f {
                        continue;
                    }
                    let lab = self.t.label(ps as usize, x as usize);
                    let v = self.m[ps as usize].saturating_add(self.bm[lab]);
                    if v > best {
                        best = v;
                        bi = i as u8;
                    }
                }
                self.nx[ns] = best;
                ch[ns] = bi;
            }
            // Keep the metrics near zero (they are not modular here: forced steps leave states at NEG).
            let top = self.nx.iter().copied().max().unwrap_or(0);
            for v in self.nx.iter_mut() {
                *v = if *v <= NEG { NEG } else { *v - top };
            }
            std::mem::swap(&mut self.m, &mut self.nx);
        }
        let mut state = match end {
            Some(e) => e,
            None => (0..s).max_by_key(|&i| self.m[i]).unwrap_or(0),
        };
        let metric = self.m[state];
        if metric <= NEG {
            return Err(FecError::Uncorrectable);
        }
        for step in (0..steps).rev() {
            let i = self.choice[step * s + state] as usize;
            let (ps, x) = self.pred[self.pred_start[state] as usize + i];
            out[step] = x;
            state = ps as usize;
        }
        Ok(Decoded {
            corrected: 0,
            metric,
            iterations: 1,
        })
    }
}
