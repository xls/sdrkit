//! The min-sum check rule of the family codes, the hf_chat codes and the slotted_voice code: frame error rate against rule
//! (scale and offset) on BPSK in white noise, each code with the iteration cap, the LLR quantiser and the good-frame test of
//! its receivers. Every rule of a point decodes the same frames. Each line gives, per rule, the Eb/N0 at a frame error rate
//! of 0.1 and 0.01, the lost frames of each rule in ten times the frames 0.3 dB above the crossing of the rule in use, and
//! the best rule with its gain. Run with
//! `cargo test --release -p sdrkit-stack-custom --test ldpc_scale -- --ignored --nocapture`; `LDPC_SCALE_FRAMES` sets the
//! frames a point (default 1000).

mod common;

use common::Noise;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_dev_test::fer::{self, Trial};
use sdrkit_stack_custom::family::ldpc;
use sdrkit_stack_custom::hf_chat::{self, PROFILES};
use sdrkit_stack_custom::slotted_voice::{phy, v4};

/// file_transfer and arq_packet: clipped at 30, full scale 127.
fn clip30(l: &[f32]) -> Vec<i8> {
    l.iter().map(|v| (v.clamp(-30.0, 30.0) * (127.0 / 30.0)).round() as i8).collect()
}

/// picture_burst and qam_video: the mean magnitude scaled to 12.
fn mean12(l: &[f32]) -> Vec<i8> {
    let mean = l.iter().map(|v| v.abs()).sum::<f32>() / l.len() as f32;
    l.iter().map(|v| (v * 12.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect()
}

/// hf_chat: clipped at 30, 4 steps a unit.
fn hf(l: &[f32]) -> Vec<i8> {
    l.iter().map(|v| (v.clamp(-30.0, 30.0) * 4.0).round() as i8).collect()
}

/// An LLR quantiser of a receiver.
type Quantise = fn(&[f32]) -> Vec<i8>;
/// The encoder of a code: information bits in, code word out.
type Encode<'a> = Box<dyn Fn(&[u8]) -> Vec<u8> + Sync + 'a>;

/// How the bits of a code word reach the decoder.
#[derive(Clone, Copy)]
enum Shape {
    /// Every bit is sent; the word is good when the decoder ends with every check holding.
    Full(Quantise),
    /// A slotted_voice word of `k` information bits and `n` sent bits: the information bits, then the parity bits in turn
    /// (repeated parity adds up), the mean magnitude scaled to 12, the shortened bits known; good as [`phy::decode_word`]
    /// says.
    Slot { k: usize, n: usize },
}

/// One code as its receivers decode it.
struct Job<'a> {
    name: String,
    code: &'a LdpcCode,
    encode: Encode<'a>,
    iters: u32,
    shape: Shape,
    /// The rule the receivers use.
    used: MinSum,
}

impl Job<'_> {
    /// Information bits and sent bits of a word.
    fn size(&self) -> (usize, usize) {
        match self.shape {
            Shape::Full(_) => (self.code.k(), self.code.n()),
            Shape::Slot { k, n } => (k, n),
        }
    }
}

/// The BPSK capacity limit for rate `r`, dB Eb/N0 (rounded down).
fn limit_db(r: f64) -> f64 {
    [(0.25, -0.8), (0.34, -0.5), (0.5, 0.1), (0.53, 0.2), (0.67, 1.0), (0.75, 1.6), (0.84, 2.3), (0.9, 3.1)]
        .iter()
        .find(|(x, _)| r <= *x)
        .map_or(3.1, |p| p.1)
}

/// One code with a decoder per rule: random information bits, BPSK in white noise at an Eb/N0, the receiver's quantiser.
struct Run<'a> {
    job: &'a Job<'a>,
    decs: Vec<LdpcDecoder>,
    noise: Noise,
    sigma2: f64,
    info: Vec<u8>,
    cw: Vec<u8>,
    q: Vec<i8>,
    out: Vec<u8>,
}

impl Trial for Run<'_> {
    fn start(&mut self, ebn0_db: f64, seed: u64) {
        let (k, n) = self.job.size();
        self.sigma2 = n as f64 / (2.0 * k as f64 * 10f64.powf(ebn0_db / 10.0));
        self.noise = Noise::new(seed);
    }

    fn send(&mut self, _f: u64) {
        let (k, n) = self.job.size();
        self.info = (0..k).map(|_| u8::from(self.noise.sample(1.0).re < 0.0)).collect();
        self.cw = (self.job.encode)(&self.info);
        let (noise, sigma2) = (&mut self.noise, self.sigma2);
        let mut llr = |b: u8| {
            let y = if b == 0 { 1.0 } else { -1.0 } + f64::from(noise.sample(2.0 * sigma2).re);
            (2.0 * y / sigma2) as f32
        };
        self.q = match self.job.shape {
            Shape::Full(quantise) => quantise(&self.cw.iter().map(|&b| llr(b)).collect::<Vec<_>>()),
            Shape::Slot { .. } => {
                let at = |j: usize| if j < k { j } else { phy::K + (j - k) % phy::K };
                let mut acc = [0f32; phy::N];
                for j in 0..n {
                    acc[at(j)] += llr(self.cw[at(j)]);
                }
                let mean = acc.iter().map(|v| v.abs()).sum::<f32>() / n as f32;
                acc.iter().map(|&v| phy::quantise(v, 12.0 / mean.max(1e-12))).collect()
            }
        };
    }

    fn lost(&mut self, i: usize) -> bool {
        match self.job.shape {
            Shape::Full(_) => self.decs[i].decode(&self.q, self.job.iters, &mut self.out).is_err() || self.out != self.cw,
            Shape::Slot { k, .. } => {
                let l: [i8; phy::N] = self.q.as_slice().try_into().expect("640 LLRs");
                let (good, out) = phy::decode_word(&mut self.decs[i], &l, k, self.job.iters);
                !good || out[..k] != self.info[..]
            }
        }
    }
}

/// The result line of one code, as [`fer::compare`] writes it, searched from the capacity limit of its rate.
fn measure(job: &Job, frames: u64) -> String {
    let rules = fer::rules();
    let mut run = Run {
        job,
        decs: rules.iter().map(|&r| LdpcDecoder::new(job.code, r)).collect(),
        noise: Noise::new(0),
        sigma2: 1.0,
        info: Vec::new(),
        cw: Vec::new(),
        q: Vec::new(),
        out: vec![0; job.code.n()],
    };
    let (k, n) = job.size();
    let from = limit_db(k as f64 / n as f64);
    fer::compare(&mut run, &format!("{:<28} it {:2}", job.name, job.iters), job.used, from, frames, true)
}

fn run(jobs: &[Job]) {
    let frames = fer::frames(1000);
    fer::header("Eb/N0", frames);
    for l in fer::parallel(jobs, |j| measure(j, frames)) {
        println!("{l}");
    }
}

#[test]
#[ignore = "measurement: minutes in release"]
fn family_frame_error_rate_against_rule() {
    let codes: Vec<_> = ldpc::tables().iter().map(|m| ldpc::code(m.n, m.rate).unwrap()).collect();
    let mut jobs = Vec::new();
    for c in &codes {
        let (a, d) = ldpc::RATES[c.base.rate];
        // The quantisers of the receivers of each length: 7680 both, 2304 clipped, 1920 mean-scaled.
        let quants: &[(&str, Quantise)] = match c.base.n {
            7680 => &[("clip", clip30), ("mean", mean12)],
            2304 => &[("clip", clip30)],
            _ => &[("mean", mean12)],
        };
        for &(qn, q) in quants {
            jobs.push(Job {
                name: format!("family {} {a}/{d} {qn}", c.base.n),
                code: &c.code,
                encode: Box::new(move |info| {
                    let mut cw = vec![0u8; c.base.n];
                    c.encode(info, &mut cw);
                    cw
                }),
                iters: 50,
                shape: Shape::Full(q),
                used: ldpc::rule(c.base.n, c.base.rate),
            });
        }
    }
    run(&jobs);
}

#[test]
#[ignore = "measurement: minutes in release"]
fn hf_chat_frame_error_rate_against_rule() {
    let jobs: Vec<Job> = PROFILES
        .iter()
        .map(|p| Job {
            name: format!("hf_chat {} {}/{}", p.id, p.k, p.n),
            code: hf_chat::code(p),
            encode: Box::new(move |info| hf_chat::encode(p, info)),
            iters: if p.is_chirp() { 12 } else { 30 },
            shape: Shape::Full(hf),
            used: hf_chat::rule(p),
        })
        .collect();
    run(&jobs);
}

/// The word shapes of the slotted_voice receivers: the control word (every bit, 50 iterations), the version 3 words of 480
/// bits (40 iterations) and three version 4 slots (40 iterations).
#[test]
#[ignore = "measurement: minutes in release"]
fn slotted_voice_frame_error_rate_against_rule() {
    use v4::Profile;
    let mut shapes = vec![("control".to_string(), phy::K, phy::N, 50), ("v3 320".into(), 320, 480, 40), ("v3 240".into(), 240, 480, 40)];
    for p in [Profile::V80, Profile::V48, Profile::V32R] {
        shapes.push((format!("v4 {p:?}"), p.k(), p.n(), 40));
    }
    let jobs: Vec<Job> = shapes
        .into_iter()
        .map(|(name, k, n, iters)| Job {
            name: format!("slotted_voice {name} {k}/{n}"),
            code: phy::code(),
            encode: Box::new(|info| {
                let mut cw = [0u8; phy::N];
                phy::encode(info, &mut cw);
                cw.to_vec()
            }),
            iters,
            shape: Shape::Slot { k, n },
            used: phy::RULE,
        })
        .collect();
    run(&jobs);
}
