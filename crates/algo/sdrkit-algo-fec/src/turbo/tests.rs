//! The vector kernel against a scalar reference decoder: equal outputs on every kind of code, equal frame error counts, and
//! the time per trellis step of both.

use super::*;
use crate::turbo_codes::{sh_code, sh_map, sl_code, SH_PATTERNS};
use std::time::Instant;

const OLD_NEG: i16 = -16_000;

/// The scalar reference decoder: one state and one branch at a time.
struct Old {
    code: TurboCode,
    next: Vec<u8>,
    out: Vec<u8>,
    chan: Vec<i16>,
    ls: Vec<i16>,
    ls2: Vec<i16>,
    la: Vec<i16>,
    le: Vec<i16>,
    la2: Vec<i16>,
    le2: Vec<i16>,
    alpha: Vec<[i16; MAX_STATES]>,
    hard: Vec<u8>,
}

impl Old {
    fn new(code: TurboCode) -> Self {
        let r = code.rsc;
        let ns = r.states();
        let mut next = vec![0u8; 2 * ns];
        let mut out = vec![0u8; 2 * ns];
        for s in 0..ns {
            for u in 0..2u8 {
                let (p, n) = r.step(s, u);
                next[2 * s + u as usize] = n as u8;
                out[2 * s + u as usize] = p;
            }
        }
        let per = code.periods();
        Old {
            next,
            out,
            chan: vec![0; code.streams() * per],
            ls: vec![0; per],
            ls2: vec![0; per],
            la: vec![0; per],
            le: vec![0; per],
            la2: vec![0; per],
            le2: vec![0; per],
            alpha: vec![[0; MAX_STATES]; per + 1],
            hard: vec![0; code.k],
            code,
        }
    }

    fn decode(
        &mut self,
        iters: u32,
        mut check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let c = &self.code;
        let (k, n, per) = (c.k, c.n, c.periods());
        let st = c.rsc.streams();
        let np = c.rsc.parities;
        let ns = c.rsc.states();
        let chan = &self.chan;
        let s1 = &chan[..per];
        let s2 = &chan[st * per..(st + 1) * per];
        self.ls[..n].copy_from_slice(&s1[..n]);
        for i in 0..n {
            let p = c.pi[i] as usize;
            self.ls[p] = self.ls[p].saturating_add(s2[i]);
        }
        for i in 0..n {
            self.ls2[i] = self.ls[c.pi[i] as usize];
        }
        self.ls[n..].copy_from_slice(&s1[n..]);
        self.ls2[n..].copy_from_slice(&s2[n..]);
        self.la.fill(0);
        self.la2.fill(0);
        self.le2.fill(0);
        let zero = [0i16; 0];
        let par = |c0: usize, j: usize| -> &[i16] {
            if j < np {
                &chan[(c0 * st + 1 + j) * per..(c0 * st + 2 + j) * per]
            } else {
                &zero
            }
        };
        let len0 = n + c.tail(0);
        let len1 = n + c.tail(1);
        let known0 = c.ends[0] != End::Open;
        let known1 = c.ends[1] != End::Open;
        let mut used = 0;
        let mut passed = false;
        for _ in 0..iters.max(1) {
            used += 1;
            for i in 0..n {
                self.la[c.pi[i] as usize] = self.le2[i];
            }
            old_siso(
                &self.next,
                &self.out,
                ns,
                &mut self.alpha,
                OldBranch {
                    sys: &self.ls,
                    p0: par(0, 0),
                    p1: par(0, 1),
                    la: &self.la,
                },
                &mut self.le,
                n,
                len0,
                known0,
                c.scale,
            );
            for i in 0..n {
                self.la2[i] = self.le[c.pi[i] as usize];
            }
            old_siso(
                &self.next,
                &self.out,
                ns,
                &mut self.alpha,
                OldBranch {
                    sys: &self.ls2,
                    p0: par(1, 0),
                    p1: par(1, 1),
                    la: &self.la2,
                },
                &mut self.le2,
                n,
                len1,
                known1,
                c.scale,
            );
            for i in 0..n {
                self.la[c.pi[i] as usize] = self.le2[i];
            }
            for i in 0..k {
                let l = self.ls[i] as i32 + self.le[i] as i32 + self.la[i] as i32;
                self.hard[i] = (l < 0) as u8;
            }
            if check(&self.hard) {
                passed = true;
                break;
            }
        }
        out[..k].copy_from_slice(&self.hard);
        let corrected = (0..k)
            .filter(|&i| self.ls[i] != 0 && ((self.ls[i] < 0) as u8) != self.hard[i])
            .count() as u32;
        if passed || iters == 0 {
            Ok(Decoded {
                corrected,
                metric: 0,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }
}

/// The inputs of [`old_siso`]: systematic, up to two parities (an empty slice for none), a-priori.
struct OldBranch<'a> {
    sys: &'a [i16],
    p0: &'a [i16],
    p1: &'a [i16],
    la: &'a [i16],
}

/// One max-log-MAP pass of the scalar reference.
#[allow(clippy::too_many_arguments)]
fn old_siso(
    next: &[u8],
    out: &[u8],
    ns: usize,
    alpha: &mut [[i16; MAX_STATES]],
    b: OldBranch,
    le: &mut [i16],
    n: usize,
    len: usize,
    known: bool,
    scale: (i32, i32),
) {
    let gam = |t: usize, s: usize, u: usize| -> i16 {
        let su = b.sys[t] + b.la[t];
        let p = out[2 * s + u];
        let mut g = if u == 0 { su } else { -su };
        if let Some(&v) = b.p0.get(t) {
            g += if p & 1 == 0 { v } else { -v };
        }
        if let Some(&v) = b.p1.get(t) {
            g += if p & 2 == 0 { v } else { -v };
        }
        g
    };
    let mut a = [OLD_NEG; MAX_STATES];
    a[0] = 0;
    alpha[0] = a;
    for t in 0..len {
        let mut na = [OLD_NEG; MAX_STATES];
        for s in 0..ns {
            for u in 0..2 {
                let nx = next[2 * s + u] as usize;
                let v = alpha[t][s].saturating_add(gam(t, s, u));
                if v > na[nx] {
                    na[nx] = v;
                }
            }
        }
        let base = na[0];
        for v in na[..ns].iter_mut() {
            *v = v.saturating_sub(base).max(OLD_NEG);
        }
        alpha[t + 1] = na;
    }
    let mut bt = [if known { OLD_NEG } else { 0 }; MAX_STATES];
    bt[0] = 0;
    for t in (0..len).rev() {
        let (mut m0, mut m1) = (i32::MIN, i32::MIN);
        let mut nb = [OLD_NEG; MAX_STATES];
        for s in 0..ns {
            for u in 0..2 {
                let nx = next[2 * s + u] as usize;
                let g = gam(t, s, u);
                if t < n {
                    let v = alpha[t][s] as i32 + g as i32 + bt[nx] as i32;
                    if u == 0 {
                        m0 = m0.max(v);
                    } else {
                        m1 = m1.max(v);
                    }
                }
                let w = bt[nx].saturating_add(g);
                if w > nb[s] {
                    nb[s] = w;
                }
            }
        }
        let base = nb[0];
        for v in nb[..ns].iter_mut() {
            *v = v.saturating_sub(base).max(OLD_NEG);
        }
        bt = nb;
        if t < n {
            // (m0 - m1) is twice the posterior; the extrinsic removes the systematic and a-priori parts.
            let e = ((m0 - m1) / 2 - b.sys[t] as i32 - b.la[t] as i32) * scale.0 / scale.1;
            le[t] = e.clamp(-2000, 2000) as i16;
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn bits(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.next() >> 63) as u8).collect()
    }
    fn gauss(&mut self) -> f64 {
        let u1 = ((self.next() >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
        let u2 = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
    fn permutation(&mut self, n: usize) -> Vec<u32> {
        let mut p: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            p.swap(i, self.below(i + 1));
        }
        p
    }
}

/// Eb/N0 (dB) at which the BPSK-input AWGN capacity equals `rate`.
fn bpsk_limit(rate: f64) -> f64 {
    let cap = |esn0: f64| {
        let s2 = 1.0 / (2.0 * esn0);
        let s = s2.sqrt();
        let (mut acc, mut wsum) = (0.0, 0.0);
        for i in -4000..=4000 {
            let z = i as f64 / 500.0;
            let w = (-z * z / 2.0).exp();
            let y = 1.0 + s * z;
            acc += w * (1.0 + (-2.0 * y / s2).exp()).log2();
            wsum += w;
        }
        1.0 - acc / wsum
    };
    let (mut lo, mut hi) = (-20.0f64, 10.0f64);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if cap(10f64.powf(mid / 10.0)) < rate {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo - 10.0 * rate.log10()
}

/// A code with its transmission map.
struct Case {
    name: String,
    code: TurboCode,
    map: Vec<Slot>,
}

impl Case {
    fn rate(&self) -> f64 {
        self.code.k as f64 / self.map.len() as f64
    }

    /// Trellis steps of both constituents in one iteration.
    fn steps(&self) -> usize {
        2 * self.code.n + self.code.tail(0) + self.code.tail(1)
    }

    /// Random bits sent as BPSK over white noise at `ebn0` dB: the bits and the soft values, scaled to a mean magnitude of 12.
    fn frame(&self, rng: &mut Rng, ebn0: f64) -> (Vec<u8>, Vec<i8>) {
        let code = &self.code;
        let bits = rng.bits(code.k);
        let mut raw = vec![0u8; code.streams() * code.periods()];
        code.encode(&bits, &mut raw).unwrap();
        let mut tx = vec![0u8; self.map.len()];
        code.transmit(&self.map, &raw, &mut tx).unwrap();
        let sigma = (1.0 / (2.0 * self.rate() * 10f64.powf(ebn0 / 10.0))).sqrt();
        let y: Vec<f64> = tx
            .iter()
            .map(|&b| 1.0 - 2.0 * b as f64 + sigma * rng.gauss())
            .collect();
        let gain = 12.0 * y.len() as f64 / y.iter().map(|v| v.abs()).sum::<f64>();
        let llr = y
            .iter()
            .map(|v| (v * gain).round().clamp(-127.0, 127.0) as i8)
            .collect();
        (bits, llr)
    }
}

/// Every slot of `code` once.
fn full_map(code: &TurboCode) -> Vec<Slot> {
    let st = code.rsc.streams();
    let mut m = Vec::new();
    for c in 0..2 {
        for j in 0..st {
            for t in 0..code.n + code.tail(c) {
                m.push(Slot {
                    stream: (c * st + j) as u8,
                    period: t as u32,
                });
            }
        }
    }
    m
}

fn lte(k: usize) -> Case {
    let t = Turbo::new(k).unwrap();
    Case {
        name: format!("LTE K {k}, rate 1/3"),
        code: t.dec.code.clone(),
        map: t.map,
    }
}

fn sh(k: usize, pattern: usize) -> Case {
    let p = &SH_PATTERNS[pattern];
    Case {
        name: format!("3GPP2 K {k}, rate {}/{}", p.rate.0, p.rate.1),
        code: sh_code(k).unwrap(),
        map: sh_map(k, p),
    }
}

/// The 16-state code with a random interleaver of `n`, every data bit sent and `pq` bits of each parity.
fn sl(n: usize, pq: usize, rng: &mut Rng) -> Case {
    let code = sl_code(&rng.permutation(n)).unwrap();
    let mut map: Vec<Slot> = (0..n as u32)
        .map(|t| Slot {
            stream: 0,
            period: t,
        })
        .collect();
    for i in 0..pq {
        let period = (i * n / pq) as u32;
        map.push(Slot { stream: 1, period });
        map.push(Slot { stream: 3, period });
    }
    code.check_map(&map).unwrap();
    Case {
        name: format!(
            "16-state N {n}, rate {:.2}",
            (n - 4) as f64 / map.len() as f64
        ),
        code,
        map,
    }
}

/// A code from `rsc` with a random interleaver and every slot sent.
fn general(name: &str, rsc: Rsc, k: usize, ends: [End; 2], rng: &mut Rng) -> Case {
    let code = TurboCode::new(rsc, k, rng.permutation(k), ends).unwrap();
    Case {
        name: name.into(),
        map: full_map(&code),
        code,
    }
}

/// Decodes `chan` with the reference and with both forms of the kernel and requires equal results, decisions and
/// extrinsics.
fn require_equal(
    case: &Case,
    old: &mut Old,
    new: &mut TurboDecoder,
    chan: &[i16],
    iters: u32,
    bits: &[u8],
) {
    let (k, n) = (case.code.k, case.code.n);
    let mut want = vec![0u8; k];
    let mut got = vec![0u8; k];
    old.chan.copy_from_slice(chan);
    let r0 = old.decode(iters, |b| b == bits, &mut want);
    for plain in [false, true] {
        new.plain = plain;
        new.chan.copy_from_slice(chan);
        let r1 = new.decode(iters, |b| b == bits, &mut got);
        let what = format!("{}, {iters} iterations, plain {plain}", case.name);
        assert_eq!(format!("{r0:?}"), format!("{r1:?}"), "{what}");
        assert_eq!(want, got, "{what}");
        assert!(old.le[..n] == new.le[..n], "first extrinsic: {what}");
        assert!(old.le2[..n] == new.le2[..n], "second extrinsic: {what}");
    }
    new.plain = false;
}

/// The vector form of the target, the array form and the scalar reference give the same iteration counts, decisions and
/// extrinsic values: 4, 8 and 16 states, one and two parities, every kind of end, below and above the waterfall, and with
/// channel values large enough to saturate the state metrics.
#[test]
fn kernel_forms_equal_the_reference() {
    let mut rng = Rng(21);
    let four = Rsc {
        memory: 2,
        feedback: 0b111,
        forward: [0b101, 0b011],
        parities: 2,
    };
    let cases = [
        lte(40),
        lte(1408),
        sh(1146, 0),
        sh(1146, 11),
        sh(12_282, 6),
        sl(436, 422, &mut rng),
        general(
            "4 states, tail and open",
            four,
            200,
            [End::Tail, End::Open],
            &mut rng,
        ),
        general(
            "16 states, two tails",
            RSC_SL,
            300,
            [End::Tail, End::Tail],
            &mut rng,
        ),
        general(
            "8 states, open ends",
            RSC_3GPP2,
            257,
            [End::Open, End::Open],
            &mut rng,
        ),
    ];
    for case in &cases {
        let mut old = Old::new(case.code.clone());
        let mut new = TurboDecoder::new(case.code.clone());
        let limit = bpsk_limit(case.rate());
        for offset in [-1.0, 0.7, 3.0] {
            let (bits, llr) = case.frame(&mut rng, limit + offset);
            new.load(&case.map, &llr).unwrap();
            let chan = new.chan.clone();
            for iters in [0, 1, 8] {
                require_equal(case, &mut old, &mut new, &chan, iters, &bits);
            }
        }
        // Large values: systematic sums up to 8000, parities up to 8000.
        let st = case.code.rsc.streams();
        let per = case.code.periods();
        let bits = rng.bits(case.code.k);
        for round in 0..3 {
            let chan: Vec<i16> = (0..2 * st * per)
                .map(|i| {
                    let top = if (i / per) % st == 0 { 4000 } else { 8000 };
                    match round {
                        0 => rng.below(2 * top + 1) as i16 - top as i16,
                        1 => [top as i16, -(top as i16)][rng.below(2)],
                        _ => top as i16,
                    }
                })
                .collect();
            require_equal(case, &mut old, &mut new, &chan, 3, &bits);
        }
    }
}

/// The codes and rates the stacks use, each with three Eb/N0 offsets (dB above the BPSK limit of the rate) around its
/// waterfall and a frame count.
fn waterfall_cases(rng: &mut Rng) -> Vec<(Case, [f64; 3], usize)> {
    let mut v = vec![
        (lte(1408), [0.6, 1.0, 1.4], 40),
        (lte(6144), [0.5, 0.8, 1.1], 12),
        (sl(3444, 1270, rng), [0.7, 1.0, 1.3], 20),
    ];
    for pattern in [0, 1, 2, 3, 4, 6, 8, 10] {
        v.push((sh(12_282, pattern), [0.7, 0.85, 1.0], 8));
    }
    v
}

/// Frame errors of the reference and of the kernel after at most 8 iterations, over the same seeds: equal at every point.
#[test]
fn frame_errors_equal_the_reference() {
    let mut rng = Rng(5);
    for (case, offsets, frames) in waterfall_cases(&mut rng) {
        let mut old = Old::new(case.code.clone());
        let mut new = TurboDecoder::new(case.code.clone());
        let limit = bpsk_limit(case.rate());
        let k = case.code.k;
        let (mut a, mut b) = (vec![0u8; k], vec![0u8; k]);
        for offset in offsets {
            let (mut bad_old, mut bad_new) = (0, 0);
            for _ in 0..frames {
                let (bits, llr) = case.frame(&mut rng, limit + offset);
                new.load(&case.map, &llr).unwrap();
                old.chan.copy_from_slice(&new.chan);
                bad_old += old.decode(8, |d| d == bits, &mut a).is_err() as usize;
                bad_new += new.decode(8, |d| d == bits, &mut b).is_err() as usize;
                assert_eq!(a, b, "{}", case.name);
            }
            println!(
                "{:28} Eb/N0 {:5.2} dB (limit {offset:+.1}): frame errors old {bad_old:2}, new {bad_new:2} of {frames}",
                case.name,
                limit + offset
            );
            assert_eq!(bad_old, bad_new, "{}", case.name);
        }
    }
}

/// Seconds of the fastest of `runs` calls.
fn fastest(runs: usize, mut f: impl FnMut()) -> f64 {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

/// Prints the time per trellis step and per code word of the reference, the vector form and the array form: one iteration
/// with the stop at high SNR, 8 iterations without a stop below the waterfall. Run with `--release -- --ignored --nocapture`.
#[test]
#[ignore = "timing"]
fn time_per_trellis_step() {
    let mut rng = Rng(9);
    let cases = [
        lte(96),
        lte(1408),
        lte(6144),
        sh(1146, 0),
        sh(12_282, 0),
        sh(12_282, 10),
        sl(3444, 1270, &mut rng),
    ];
    for case in &cases {
        let mut old = Old::new(case.code.clone());
        let mut new = TurboDecoder::new(case.code.clone());
        let limit = bpsk_limit(case.rate());
        let mut out = vec![0u8; case.code.k];
        for (offset, stop) in [(6.0, true), (-1.0, false)] {
            let (bits, llr) = case.frame(&mut rng, limit + offset);
            new.load(&case.map, &llr).unwrap();
            old.chan.copy_from_slice(&new.chan);
            let mut used = 0;
            let t_old = fastest(15, || {
                used = match old.decode(8, |d| stop && d == bits, &mut out) {
                    Ok(r) => r.iterations,
                    Err(_) => 8,
                }
            });
            let steps = (used as usize * case.steps()) as f64;
            let mut line = format!(
                "{:28} {used} iterations: old {:6.1} ns/step {:8.1} us/word",
                case.name,
                t_old * 1e9 / steps,
                t_old * 1e6
            );
            for (form, plain) in [("vector", false), ("array", true)] {
                new.plain = plain;
                let mut now = 0;
                let t = fastest(15, || {
                    now = match new.decode(8, |d| stop && d == bits, &mut out) {
                        Ok(r) => r.iterations,
                        Err(_) => 8,
                    }
                });
                assert_eq!(now, used);
                line += &format!(
                    "; {form} {:5.1} ns/step {:7.1} us/word ({:4.1}x)",
                    t * 1e9 / steps,
                    t * 1e6,
                    t_old / t
                );
            }
            println!("{line}");
        }
    }
}
