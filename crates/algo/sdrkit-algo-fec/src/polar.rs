//! 5G NR polar codes (3GPP TS 38.212 clauses 5.3.1 and 5.4.1): code construction from the reliability sequence, input bit
//! interleaving, encoding, sub-block interleaving and rate matching by repetition, puncturing or shortening, and their inverse;
//! decoded by successive-cancellation list decoding with the CRC choosing among the list.

use crate::error::{Decoded, FecError, Param, Result};
use crate::tables_3gpp::{NR_POLAR_IL_MAX, NR_POLAR_Q, NR_POLAR_SUBBLOCK};

/// The parameters of one NR polar code instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolarParams {
    /// Information bits including the CRC (`K`).
    pub k: usize,
    /// Bits after rate matching (`E`).
    pub e: usize,
    /// `n_max`: 9 for the downlink (PBCH, PDCCH), 10 for the uplink.
    pub n_max: u32,
    /// Input bit interleaving (`I_IL`: on for the downlink).
    pub interleave: bool,
}

impl PolarParams {
    /// The NR PBCH code: `K = 56`, `E = 864`, `n_max = 9`, interleaved (38.212 clause 7.1.4).
    pub const PBCH: PolarParams = PolarParams {
        k: 56,
        e: 864,
        n_max: 9,
        interleave: true,
    };

    /// The downlink control code for `k` bits (DCI plus CRC) and `e` rate-matched bits (38.212 clause 7.3.3).
    pub const fn downlink(k: usize, e: usize) -> PolarParams {
        PolarParams {
            k,
            e,
            n_max: 9,
            interleave: true,
        }
    }
}

const BIG: f32 = 1.0e4;

/// An NR polar encoder and SCL decoder for one parameter set, sized at construction.
///
/// ```
/// use sdrkit_algo_fec::polar::{Polar, PolarParams};
/// let mut p = Polar::new(PolarParams::PBCH, 8).unwrap();
/// let bits: Vec<u8> = (0..56).map(|i| (i * 5 % 3 == 0) as u8).collect();
/// let mut e = [0u8; 864];
/// p.encode(&bits, &mut e).unwrap();
/// let llr: Vec<f32> = e.iter().map(|&b| if b == 0 { 1.5 } else { -1.5 }).collect();
/// let mut out = [0u8; 56];
/// p.decode(&llr, |c| c == &bits[..], &mut out).unwrap();
/// assert_eq!(&out[..], &bits[..]);
/// ```
#[derive(Clone, Debug)]
pub struct Polar {
    params: PolarParams,
    n: usize,
    logn: usize,
    list: usize,
    /// Frozen flags by u index.
    frozen: Vec<bool>,
    /// Information positions (ascending).
    info: Vec<u16>,
    /// Input interleaver Π: `c'_k = c_{Π(k)}`.
    pi: Vec<u16>,
    /// Sub-block interleaver J: `y_n = d_{J(n)}`.
    j: Vec<u16>,
    // Arena: 2L paths.
    alpha: Vec<f32>,
    beta: Vec<u8>,
    u: Vec<u8>,
    pm: Vec<f32>,
    active: Vec<bool>,
    // Scratch.
    y: Vec<f32>,
    cand: Vec<(f32, u16, u8)>,
    cbits: Vec<u8>,
    cprime: Vec<u8>,
    work: Vec<u8>,
}

impl Polar {
    /// The code for `params` with list size `list` (1 to 32). An error for `K` outside 12..=164 with interleaving (or `K > E`,
    /// `K` above `N`).
    pub fn new(params: PolarParams, list: usize) -> Result<Self> {
        let (k, e) = (params.k, params.e);
        Param::List.check(list, 1, 32)?;
        Param::K.check(k, 1, e as i64)?;
        Param::NMax.check(params.n_max, 5, 10)?;
        if params.interleave && k > 164 {
            return Param::K.range(1, 164, k);
        }
        // N (clause 5.3.1).
        let ceil_log2 = |x: f64| x.log2().ceil() as u32;
        let le = ceil_log2(e as f64);
        let n1 = if (e as f64) <= 9.0 / 8.0 * 2f64.powi(le as i32 - 1)
            && (k as f64) / (e as f64) < 9.0 / 16.0
        {
            le - 1
        } else {
            le
        };
        let n2 = ceil_log2(8.0 * k as f64);
        let logn = n1.min(n2).min(params.n_max).max(5) as usize;
        let n = 1usize << logn;
        if k > n {
            return Param::K.range(1, n as i64, k);
        }
        // Sub-block interleaver (clause 5.4.1.1).
        let j: Vec<u16> = (0..n)
            .map(|i| (NR_POLAR_SUBBLOCK[32 * i / n] as usize * (n / 32) + i % (n / 32)) as u16)
            .collect();
        // Frozen set (clause 5.4.1.1 and 5.3.1.2).
        let mut pre = vec![false; n];
        if e < n {
            if (k as f64) / (e as f64) <= 7.0 / 16.0 {
                for &jj in &j[..n - e] {
                    pre[jj as usize] = true;
                }
                let t = if 4 * e >= 3 * n {
                    (0.75 * n as f64 - e as f64 / 2.0).ceil()
                } else {
                    (9.0 * n as f64 / 16.0 - e as f64 / 4.0).ceil()
                }
                .max(0.0) as usize;
                for p in pre.iter_mut().take(t.min(n)) {
                    *p = true;
                }
            } else {
                for &jj in &j[e..n] {
                    pre[jj as usize] = true;
                }
            }
        }
        let mut frozen = vec![true; n];
        let mut chosen = 0;
        for &q in NR_POLAR_Q.iter().rev() {
            let q = q as usize;
            if q >= n || pre[q] {
                continue;
            }
            if chosen == k {
                break;
            }
            frozen[q] = false;
            chosen += 1;
        }
        if chosen < k {
            return Param::K.conflict(Param::E);
        }
        let info: Vec<u16> = (0..n).filter(|&i| !frozen[i]).map(|i| i as u16).collect();
        // Input interleaver (clause 5.3.1.1).
        let pi: Vec<u16> = if params.interleave {
            NR_POLAR_IL_MAX
                .iter()
                .filter(|&&v| v as usize >= 164 - k)
                .map(|&v| (v as usize - (164 - k)) as u16)
                .collect()
        } else {
            (0..k as u16).collect()
        };
        let slots = 2 * list;
        Ok(Polar {
            params,
            n,
            logn,
            list,
            frozen,
            info,
            pi,
            j,
            alpha: vec![0.0; slots * 2 * n],
            beta: vec![0; slots * 2 * n],
            u: vec![0; slots * n],
            pm: vec![0.0; slots],
            active: vec![false; slots],
            y: vec![0.0; n],
            cand: vec![(0.0, 0, 0); slots * 2],
            cbits: vec![0; k],
            cprime: vec![0; k],
            work: vec![0; n],
        })
    }

    /// The mother code length `N`.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The information positions (indices of `u`), ascending.
    pub fn info_positions(&self) -> &[u16] {
        &self.info
    }

    /// Encodes `bits` (`K`, CRC included) into the `E` rate-matched bits `e`.
    pub fn encode(&mut self, bits: &[u8], e: &mut [u8]) -> Result<()> {
        let (k, ee, n) = (self.params.k, self.params.e, self.n);
        if bits.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: bits.len(),
            });
        }
        if e.len() != ee {
            return Err(FecError::OutputTooShort { need: ee });
        }
        let u = &mut self.work;
        u.fill(0);
        for (i, &p) in self.info.iter().enumerate() {
            u[p as usize] = bits[self.pi[i] as usize] & 1;
        }
        // d = u G_N.
        let mut h = 1;
        while h < n {
            for i in (0..n).step_by(2 * h) {
                for t in i..i + h {
                    u[t] ^= u[t + h];
                }
            }
            h *= 2;
        }
        // Rate matching (clause 5.4.1.2): y_n = d_J(n), then repetition, puncturing or shortening.
        let y = |m: usize| u[self.j[m] as usize];
        if ee >= n {
            for (i, o) in e.iter_mut().enumerate() {
                *o = y(i % n);
            }
        } else if (k as f64) / (ee as f64) <= 7.0 / 16.0 {
            for (i, o) in e.iter_mut().enumerate() {
                *o = y(i + n - ee);
            }
        } else {
            for (i, o) in e.iter_mut().enumerate() {
                *o = y(i);
            }
        }
        Ok(())
    }

    /// Decodes the `E` received LLRs (`f32`, positive means 0) with the list decoder; `check` is called on the candidate
    /// information bits (`K`, in the order given to [`Polar::encode`]) from the most likely path down, and the first that passes
    /// is written to `out`. `metric` is the chosen path's rank in the list. `Err(Uncorrectable)` when no candidate passes.
    pub fn decode(
        &mut self,
        llr: &[f32],
        mut check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let (k, ee, n) = (self.params.k, self.params.e, self.n);
        if llr.len() != ee {
            return Err(FecError::InputLength {
                expected: ee,
                got: llr.len(),
            });
        }
        if out.len() < k {
            return Err(FecError::OutputTooShort { need: k });
        }
        crate::llr::check_finite(llr)?;
        // Rate recovery into y, then d = y through J^-1.
        self.y.fill(0.0);
        if ee >= n {
            for (i, &v) in llr.iter().enumerate() {
                self.y[i % n] += v;
            }
        } else if (k as f64) / (ee as f64) <= 7.0 / 16.0 {
            for (i, &v) in llr.iter().enumerate() {
                self.y[i + n - ee] = v;
            }
        } else {
            self.y[..ee].copy_from_slice(llr);
            for v in &mut self.y[ee..] {
                *v = BIG;
            }
        }
        for slot in 0..2 * self.list {
            self.active[slot] = false;
        }
        self.active[0] = true;
        self.pm[0] = 0.0;
        for m in 0..n {
            self.alpha[self.j[m] as usize] = self.y[m];
        }
        for phi in 0..n {
            for slot in 0..2 * self.list {
                if self.active[slot] {
                    self.calc_alpha(slot, phi);
                }
            }
            if self.frozen[phi] {
                for slot in 0..2 * self.list {
                    if self.active[slot] {
                        let l = self.leaf(slot);
                        if l < 0.0 {
                            self.pm[slot] += -l;
                        }
                        self.u[slot * n + phi] = 0;
                        self.update_beta(slot, phi, 0);
                    }
                }
            } else {
                self.fork(phi);
            }
        }
        // Candidates by metric.
        let mut order = [0usize; 64];
        let mut cnt = 0;
        for slot in 0..2 * self.list {
            if self.active[slot] {
                order[cnt] = slot;
                cnt += 1;
            }
        }
        let order = &mut order[..cnt];
        order.sort_unstable_by(|&a, &b| self.pm[a].total_cmp(&self.pm[b]));
        for (rank, &slot) in order.iter().enumerate() {
            for (i, &p) in self.info.iter().enumerate() {
                self.cprime[i] = self.u[slot * n + p as usize];
            }
            for (i, &v) in self.cprime.iter().enumerate() {
                self.cbits[self.pi[i] as usize] = v;
            }
            if check(&self.cbits) {
                out[..k].copy_from_slice(&self.cbits);
                return Ok(Decoded {
                    corrected: 0,
                    metric: rank as i32,
                    iterations: 1,
                });
            }
        }
        Err(FecError::Uncorrectable)
    }

    #[inline]
    fn layer_off(&self, lambda: usize) -> usize {
        // Layer λ (size N >> λ) starts after the larger layers: 2N - 2 (N >> (λ - 1)).
        2 * self.n - 2 * (self.n >> lambda)
    }

    fn leaf(&self, slot: usize) -> f32 {
        self.alpha[slot * 2 * self.n + self.layer_off(self.logn)]
    }

    /// Computes the LLRs down to leaf `phi` for a path.
    fn calc_alpha(&mut self, slot: usize, phi: usize) {
        let (n, logn) = (self.n, self.logn);
        let start = if phi == 0 {
            1
        } else {
            logn - phi.trailing_zeros() as usize
        };
        let base = slot * 2 * n;
        for lambda in start..=logn {
            let size = n >> lambda;
            let src = base + self.layer_off(lambda - 1);
            let dst = base + self.layer_off(lambda);
            // The node at this layer is a right child when bit (logn - lambda) of phi is set.
            let right = phi >> (logn - lambda) & 1 == 1;
            let bsrc = base + self.layer_off(lambda);
            for t in 0..size {
                let a = self.alpha[src + t];
                let b = self.alpha[src + t + size];
                self.alpha[dst + t] = if right {
                    let v = self.beta[bsrc + t];
                    b + if v == 0 { a } else { -a }
                } else {
                    a.signum() * b.signum() * a.abs().min(b.abs())
                };
            }
        }
    }

    /// Records leaf bit `bit` for a path and combines partial sums up the tree.
    fn update_beta(&mut self, slot: usize, phi: usize, bit: u8) {
        let (n, logn) = (self.n, self.logn);
        let base = slot * 2 * n;
        // The beta store of layer λ holds the left child's partial sums at that layer (size N >> λ).
        let mut lambda = logn;
        let mut node = phi;
        // Current completed node's sums live in `work`-like scratch: use the layer slot directly.
        let mut cur_len = 1;
        let mut cur = [0u8; 1024];
        cur[0] = bit;
        loop {
            if lambda == 0 {
                break;
            }
            let off = base + self.layer_off(lambda);
            if node & 1 == 0 {
                self.beta[off..off + cur_len].copy_from_slice(&cur[..cur_len]);
                break;
            }
            // Right child: parent = (left ^ right, right).
            let mut parent = [0u8; 1024];
            for t in 0..cur_len {
                parent[t] = self.beta[off + t] ^ cur[t];
                parent[t + cur_len] = cur[t];
            }
            cur[..2 * cur_len].copy_from_slice(&parent[..2 * cur_len]);
            cur_len *= 2;
            lambda -= 1;
            node >>= 1;
        }
    }

    /// Splits every path on information bit `phi` and keeps the `L` best.
    fn fork(&mut self, phi: usize) {
        let n = self.n;
        let slots = 2 * self.list;
        let mut c = 0;
        for slot in 0..slots {
            if self.active[slot] {
                let l = self.leaf(slot);
                let (m0, m1) = if l >= 0.0 {
                    (self.pm[slot], self.pm[slot] + l)
                } else {
                    (self.pm[slot] - l, self.pm[slot])
                };
                self.cand[c] = (m0, slot as u16, 0);
                self.cand[c + 1] = (m1, slot as u16, 1);
                c += 2;
            }
        }
        let cands = &mut self.cand[..c];
        cands.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let keep = c.min(self.list);
        // Which children of each parent survive.
        let mut take = [0u8; 64];
        for &(_, s, b) in &cands[..keep] {
            take[s as usize] |= 1 << b;
        }
        let mut pms = [0f32; 128];
        for &(m, s, b) in &cands[..c] {
            if take[s as usize] >> b & 1 == 1 {
                pms[2 * s as usize + b as usize] = m;
            }
        }
        for slot in 0..slots {
            if self.active[slot] && take[slot] == 0 {
                self.active[slot] = false;
            }
        }
        for slot in 0..slots {
            if !self.active[slot] || take[slot] == 0 {
                continue;
            }
            if take[slot] == 3 {
                // Copy into a free slot for bit 1.
                let free = (0..slots)
                    .find(|&f| !self.active[f] && take[f] == 0)
                    .unwrap_or(slot);
                if free != slot {
                    let (a, b) = (slot * 2 * n, free * 2 * n);
                    self.alpha.copy_within(a..a + 2 * n, b);
                    self.beta.copy_within(a..a + 2 * n, b);
                    self.u.copy_within(slot * n..slot * n + n, free * n);
                    self.active[free] = true;
                    self.pm[free] = pms[2 * slot + 1];
                    self.u[free * n + phi] = 1;
                    self.update_beta(free, phi, 1);
                    // Mark so the new slot is not processed again in this loop.
                    take[free] = 0x80;
                }
                self.pm[slot] = pms[2 * slot];
                self.u[slot * n + phi] = 0;
                self.update_beta(slot, phi, 0);
            } else if take[slot] & 0x7F != 0 && take[slot] != 0x80 {
                let b = if take[slot] == 2 { 1 } else { 0 };
                self.pm[slot] = pms[2 * slot + b as usize];
                self.u[slot * n + phi] = b;
                self.update_beta(slot, phi, b);
            }
        }
    }
}
