//! Measures the frame error rate of the DVB-S2 FEC frame decoder against the LDPC check rule, in white noise:
//! `cargo run --release -p sdrkit-proto-dvbs2 --example ldpc_rule`. Every rule decodes the same frames through
//! [`FrameDecoder::decode`]: its demapper, its 8-bit LLRs, its iteration cap, then BCH. A frame is lost when BCH fails or a
//! byte of the BBFRAME is wrong.
//!
//! For each code it prints, per rule, the Es/N0 at a frame error rate of 0.1 and 0.01, the lost frames and the mean
//! iterations at 0.3 dB above the 0.01 point of the rule in use, and the gain of the best rule with an offset and of the
//! best rule without one over the rule in use. A rule that loses more frames than the rule in use at that point does not
//! count as best.
//!
//! Environment: `S2_RULE_CODES` a list of MODCOD numbers with `n` (normal) or `s` (short), such as `4n,4s,13s` (default
//! the QPSK rates and a few higher-order MODCODs in both sizes); `S2_RULE_FRAMES` the frames of a point (default 300 normal,
//! 2000 short); `S2_RULE_STEP` the step of the waterfall in dB (default 0.05); `S2_RULE_SEED` a number mixed into every
//! seed; `S2_RULE_THREADS` the threads (default half of the processors).

use sdrkit_algo_fec::ldpc::MinSum;
use sdrkit_core::Complex32;
use sdrkit_dev_test::fer::{self, Point, Trial};
use sdrkit_proto_dvbs2::fec::{ldpc_rule, FrameDecoder, S2Codec};
use sdrkit_proto_dvbs2::modcod::ModCod;
use std::sync::atomic::{AtomicUsize, Ordering};

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// One MODCOD and frame size: the coder and one receiver per rule.
struct S2Trial {
    m: ModCod,
    short: bool,
    enc: S2Codec,
    decs: Vec<FrameDecoder>,
    db: f64,
    seed: u64,
    sent: Vec<u8>,
    word: Vec<u8>,
    coded: Vec<u8>,
    inter: Vec<u8>,
    syms: Vec<Complex32>,
    out: Vec<u8>,
    iters: Vec<u64>,
}

impl S2Trial {
    fn new(m: ModCod, short: bool, rules: &[MinSum]) -> S2Trial {
        let enc = S2Codec::new(&m, short).expect("a code of this size");
        let decs = rules
            .iter()
            .map(|&r| {
                let mut d = FrameDecoder::default();
                d.codec(&m, short).expect("a code of this size").set_rule(r);
                d
            })
            .collect();
        let n = enc.code.nldpc;
        S2Trial {
            m,
            short,
            sent: vec![0; enc.code.kbch / 8],
            word: vec![0; enc.code.kldpc],
            coded: vec![0; n],
            inter: vec![0; n],
            syms: Vec::with_capacity(n),
            out: Vec::new(),
            iters: vec![0; rules.len()],
            enc,
            decs,
            db: 0.0,
            seed: 0,
        }
    }
}

impl Trial for S2Trial {
    fn start(&mut self, db: f64, seed: u64) {
        self.db = db;
        self.seed = seed;
        self.iters.fill(0);
    }

    fn send(&mut self, f: u64) {
        let mut s = mix(self.seed ^ mix(f));
        for b in &mut self.sent {
            s = mix(s);
            *b = s as u8;
        }
        for (k, b) in self.sent.iter().enumerate() {
            for i in 0..8 {
                self.word[8 * k + i] = (b >> (7 - i)) & 1;
            }
        }
        let c = &self.enc;
        c.bch.encode(&mut self.word).expect("a BCH word");
        c.ldpc.encode(&self.word, &mut self.coded).expect("an LDPC word");
        let bits = match &c.interleave {
            Some(p) => {
                p.apply(&self.coded, &mut self.inter).expect("an interleaver of the frame");
                &self.inter
            }
            None => &self.coded,
        };
        self.syms.clear();
        self.syms.extend(
            bits.chunks(c.cons.bits())
                .map(|l| c.points[l.iter().fold(0usize, |a, &v| (a << 1) | v as usize)]),
        );
        fer::noise(&mut self.syms, &mut [], self.db as f32, mix(s));
    }

    fn lost(&mut self, i: usize) -> bool {
        let nv = 10f32.powf(-(self.db as f32) / 10.0);
        let r = self.decs[i]
            .decode(&self.m, self.short, &self.syms, nv, &mut self.out)
            .expect("a frame of this size");
        self.iters[i] += u64::from(r.iterations);
        !(r.bch_ok && self.out == self.sent)
    }
}

/// What one code gave.
struct Outcome {
    name: String,
    rules: Vec<MinSum>,
    present: usize,
    thr: Vec<[Option<f64>; 2]>,
    floor: Point,
    floor_iters: Vec<f64>,
}

/// The waterfall of one code: a coarse search (down in 0.5 dB steps, then up in 0.1 dB steps) for the first point where
/// the best rule loses at most 60 %, then points
/// `step` dB apart from 0.15 dB below it until every rule loses under 0.25 % or the point is 0.4 dB above the 0.01
/// crossing of the rule in use; then one point of `frames` frames 0.3 dB above that crossing.
fn measure(m: ModCod, short: bool, frames: u64, step: f64, seed: u64) -> Outcome {
    let rules = fer::rules();
    let present_rule = ldpc_rule(&m, short).expect("a code");
    let present = rules.iter().position(|&r| r == present_rule).expect("the rule in use is compared");
    let r = rules.len();
    let mut t = S2Trial::new(m, short, &rules);
    let best = |t: &mut S2Trial, db: f64| {
        let p = fer::point(t, r, db, seed ^ 7, 40, Some(10));
        (0..r).map(|i| p.fer(i)).fold(1.0, f64::min)
    };
    let mut db = m.qef_esn0(short) - 0.8;
    while best(&mut t, db) <= 0.6 && db > m.qef_esn0(short) - 4.0 {
        db -= 0.5;
    }
    while db < m.qef_esn0(short) + 2.0 && best(&mut t, db) > 0.6 {
        db += 0.1;
    }
    let first = db - 0.15;
    let mut points: Vec<Point> = Vec::new();
    for i in 0..80u64 {
        let at = first + step * i as f64;
        let p = fer::point(&mut t, r, at, seed ^ mix(i + 1), frames, Some(50));
        let all_low = (0..r).all(|d| p.fer(d) < 0.0025);
        points.push(p);
        let past = fer::threshold(&points, present, 0.01).is_some_and(|x| at > x + 0.4);
        if all_low || past {
            break;
        }
    }
    let thr: Vec<[Option<f64>; 2]> = (0..r)
        .map(|i| [fer::threshold(&points, i, 0.1), fer::threshold(&points, i, 0.01)])
        .collect();
    let floor_db = thr[present][1].unwrap_or(first + step * points.len() as f64) + 0.3;
    let floor = fer::point(&mut t, r, floor_db, seed ^ 0xF100, frames, None);
    let floor_iters = t.iters.iter().map(|&n| n as f64 / frames as f64).collect();
    let name = format!("{}{}", m.name(), if short { " short" } else { " normal" });
    Outcome { name, rules, present, thr, floor, floor_iters }
}

fn rule_name(r: MinSum) -> String {
    format!("{}/{}", r.scale_q4, r.offset)
}

fn text(o: &Outcome) -> String {
    use std::fmt::Write;
    let mut t = String::new();
    let f = |x: Option<f64>| x.map_or("   -  ".into(), |v| format!("{v:6.2}"));
    let _ = writeln!(t, "{} (rule in use {}), {} frames at the last point", o.name, rule_name(o.rules[o.present]), o.floor.frames);
    for (i, &r) in o.rules.iter().enumerate() {
        let _ = writeln!(
            t,
            "  {:>5}{} FER 0.1 {}  0.01 {}  lost {:5} at {:.2} dB  iterations {:5.2}",
            rule_name(r),
            if i == o.present { "*" } else { " " },
            f(o.thr[i][0]),
            f(o.thr[i][1]),
            o.floor.lost[i],
            o.floor.db,
            o.floor_iters[i]
        );
    }
    let ok = |i: usize| o.floor.lost[i] <= o.floor.lost[o.present];
    for offset in [0u8, 1] {
        let best = (0..o.rules.len())
            .filter(|&i| o.rules[i].offset == offset && ok(i))
            .filter_map(|i| Some((i, o.thr[i][1]?)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((b, _)) = best {
            let gain = |k: usize| match (o.thr[o.present][k], o.thr[b][k]) {
                (Some(p), Some(x)) => format!("{:+.2}", p - x),
                _ => "  -  ".into(),
            };
            let _ = writeln!(
                t,
                "  best offset {offset}: {} gain {} dB at 0.1, {} dB at 0.01",
                rule_name(o.rules[b]),
                gain(0),
                gain(1)
            );
        }
    }
    t
}

fn main() {
    let env = |name: &str| std::env::var(name).ok();
    let codes: Vec<(ModCod, bool)> = match env("S2_RULE_CODES") {
        Some(list) => list
            .split(',')
            .map(|c| {
                let c = c.trim();
                let (id, size) = c.split_at(c.len() - 1);
                (ModCod::new(id.parse().expect("a MODCOD number")).expect("a MODCOD"), size == "s")
            })
            .collect(),
        None => [false, true]
            .iter()
            .flat_map(|&short| {
                (1..=11u8)
                    .chain([12, 13, 19, 25])
                    .filter_map(ModCod::new)
                    .filter(move |m| !short || m.has_short())
                    .map(move |m| (m, short))
            })
            .collect(),
    };
    let frames = |short: bool| {
        env("S2_RULE_FRAMES")
            .and_then(|f| f.parse().ok())
            .unwrap_or(if short { 2000 } else { 300 })
    };
    let step: f64 = env("S2_RULE_STEP").and_then(|s| s.parse().ok()).unwrap_or(0.05);
    let seed: u64 = env("S2_RULE_SEED").and_then(|s| s.parse().ok()).unwrap_or(0);
    let threads: usize = env("S2_RULE_THREADS")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get() / 2).max(1));
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(m, short)) = codes.get(i) else {
                    break;
                };
                let o = measure(m, short, frames(short), step, mix(seed) ^ mix(2 * m.id as u64 + short as u64));
                print!("{}", text(&o));
            });
        }
    });
}
