//! Tests of the non-binary LDPC codes and their sum-product decoder: check-node arithmetic worked out by hand in GF(4), the
//! repeat-accumulate construction against a direct accumulator, decoding under a seeded noise model, puncturing, input errors
//! and allocation. (The counting allocator needs `unsafe impl GlobalAlloc`.)
#![allow(unsafe_code)]

use sdrkit_algo_fec::gf::Gf;
use sdrkit_algo_fec::nbldpc::{NbCode, NbDecoder, NbStop, RepeatAccumulate};
use sdrkit_algo_fec::FecError;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs() -> usize {
    ALLOCS.with(|c| c.get())
}

/// SplitMix64, a small seeded generator.
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
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    /// A standard normal value by the Box-Muller method.
    fn gauss(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// The arbitrary repeat-accumulate test code over GF(2^m): `repeats`, a Fisher-Yates permutation and nonzero weights, both
/// drawn from SplitMix64 with `seed`.
struct RaShape {
    repeats: Vec<usize>,
    permutation: Vec<usize>,
    weights: Vec<u16>,
}

fn ra_shape(m: u8, repeats: Vec<usize>, seed: u64) -> RaShape {
    let e: usize = repeats.iter().sum();
    let mut rng = Rng(seed);
    let mut permutation: Vec<usize> = (0..e).collect();
    for i in (1..e).rev() {
        let j = rng.below(i + 1);
        permutation.swap(i, j);
    }
    let q = 1usize << m;
    let weights = (0..e).map(|_| 1 + rng.below(q - 1) as u16).collect();
    RaShape {
        repeats,
        permutation,
        weights,
    }
}

/// The (65, 15) shape over GF(64): five information symbols repeated 4 times and ten repeated 3 times, 50 accumulator outputs.
fn shape_65_15() -> RaShape {
    let mut repeats = vec![4; 5];
    repeats.extend([3; 10]);
    ra_shape(6, repeats, 2016)
}

fn ra_code(m: u8, poly: u32, s: &RaShape, combine: usize) -> NbCode {
    NbCode::repeat_accumulate(
        m,
        poly,
        &RepeatAccumulate {
            repeats: &s.repeats,
            permutation: &s.permutation,
            weights: &s.weights,
            combine,
        },
    )
    .unwrap()
}

/// The code word computed straight from the repeat-accumulate description, independent of the parity-check matrix.
fn accumulate(f: &Gf, s: &RaShape, combine: usize, info: &[u16]) -> Vec<u16> {
    let u: Vec<u16> = s
        .repeats
        .iter()
        .zip(info)
        .flat_map(|(&r, &x)| std::iter::repeat_n(x, r))
        .collect();
    let mut word = info.to_vec();
    let mut y = 0u16;
    for t in 0..u.len() / combine {
        for j in t * combine..(t + 1) * combine {
            y ^= f.mul(s.weights[j], u[s.permutation[j]]);
        }
        word.push(y);
    }
    word
}

/// Likelihood rows for `word` from coherent orthogonal signalling: tone `a` has the output `r_a = amp [a = x] + w` with `w` a standard
/// normal value, and `P(a) ∝ exp(amp r_a)`. Returns the rows and the number of symbols whose most likely value is wrong.
fn noisy(word: &[u16], q: usize, amp: f64, rng: &mut Rng) -> (Vec<f32>, usize) {
    let mut probs = Vec::with_capacity(word.len() * q);
    let mut wrong = 0;
    for &x in word {
        let r: Vec<f64> = (0..q)
            .map(|a| amp * ((a == x as usize) as u8 as f64) + rng.gauss())
            .collect();
        let best = (0..q).max_by(|&a, &b| r[a].total_cmp(&r[b])).unwrap();
        wrong += (best != x as usize) as usize;
        let top = r[best];
        probs.extend(r.iter().map(|&v| (amp * (v - top)).exp() as f32));
    }
    (probs, wrong)
}

fn info_of(rng: &mut Rng, k: usize, q: usize) -> Vec<u16> {
    (0..k).map(|_| rng.below(q) as u16).collect()
}

fn close(a: &[f32], b: &[f32]) {
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-5, "{a:?} != {b:?}");
    }
}

/// GF(4) with x^2 + x + 1: α = 2, α^2 = 3. One check over three symbols, x0 and x1 received, x2 punctured. After one iteration
/// the message to x2 is the distribution of the sum of the other weighted symbols, an XOR convolution worked out by hand.
#[test]
fn gf4_single_check_by_hand() {
    let p0 = [0.7f32, 0.1, 0.1, 0.1];
    let p1 = [0.1f32, 0.7, 0.1, 0.1];
    let uniform = [0.25f32; 4];
    let probs: Vec<f32> = [p0, p1, uniform].concat();
    let mut hard = [0u16; 3];

    // x0 + x1 + x2 = 0, so x2 = x0 + x1. P(x2 = z) = Σ_a p0(a) p1(a ^ z):
    // z = 0: .07 + .07 + .01 + .01 = .16, z = 1: .49 + .01 + .01 + .01 = .52, z = 2: .07 + .01 + .01 + .07 = .16, z = 3: .16.
    let code = NbCode::parse("4 3 2 0x7\n0:1 1:1 2:1\n").unwrap();
    let mut dec = NbDecoder::new(&code);
    let r = dec.decode(&probs, 10, &mut hard).unwrap();
    assert_eq!(r.iterations, 1);
    assert_eq!(hard, [0, 1, 1]);
    close(dec.posterior_of(2), &[0.16, 0.52, 0.16, 0.16]);
    // The message to x0 is the sum of x1 and a uniform x2, which is uniform, so x0 keeps its intrinsic row.
    close(dec.posterior_of(0), &p0);
    close(dec.posterior_of(1), &p1);

    // x0 + α x1 + x2 = 0. α x1 maps 0, 1, 2, 3 to 0, 2, 3, 1, so α x1 has the row [.1, .1, .7, .1].
    // x2 = x0 + α x1: z = 0: .07 + .01 + .07 + .01 = .16, z = 2: .49 + .01 + .01 + .01 = .52, z = 1 and 3: .16.
    let code = NbCode::parse("4 3 2\n0:1 1:2 2:1\n").unwrap();
    let mut dec = NbDecoder::new(&code);
    dec.decode(&probs, 10, &mut hard).unwrap();
    assert_eq!(hard, [0, 1, 2]);
    close(dec.posterior_of(2), &[0.16, 0.16, 0.52, 0.16]);

    // x0 + x1 + α^2 x2 = 0. α^2 x2 = x0 + x1 has the row [.16, .52, .16, .16]. x2 = α (α^2 x2), and α maps 1 to 2,
    // so x2 has the row [.16, .16, .52, .16].
    let code = NbCode::parse("4 3 2\n0:1 1:1 2:3\n").unwrap();
    let mut dec = NbDecoder::new(&code);
    dec.decode(&probs, 10, &mut hard).unwrap();
    assert_eq!(hard, [0, 1, 2]);
    close(dec.posterior_of(2), &[0.16, 0.16, 0.52, 0.16]);
}

/// GF(4) repetition: x0 + α x1 = 0 means x1 = α^2 x0. Both received. The posterior of x1 is its row times the row of x0 mapped
/// by α^2 (0, 1, 2, 3 to 0, 3, 1, 2): p0 = [.4, .3, .2, .1] maps to [.4, .2, .1, .3]. With p1 = [.1, .2, .3, .4], the product
/// is [.04, .04, .03, .12], which normalizes to [4, 4, 3, 12] / 23.
#[test]
fn gf4_repetition_by_hand() {
    let code = NbCode::parse("4 2 1\n0:1 1:2\n").unwrap();
    let probs = [0.4f32, 0.3, 0.2, 0.1, 0.1, 0.2, 0.3, 0.4];
    let mut dec = NbDecoder::new(&code);
    let mut hard = [0u16; 2];
    dec.decode(&probs, 5, &mut hard).unwrap();
    let want: Vec<f32> = [4.0, 4.0, 3.0, 12.0].iter().map(|v| v / 23.0).collect();
    close(dec.posterior_of(1), &want);
    assert_eq!(hard, [1, 3]);
    // x0 = α x1 maps x1 = 0, 1, 2, 3 to 0, 2, 3, 1, so the row of x0 from x1 is [.1, .4, .2, .3], and the posterior is [.04, .12, .04, .03] / .23.
    let want: Vec<f32> = [4.0, 12.0, 4.0, 3.0].iter().map(|v| v / 23.0).collect();
    close(dec.posterior_of(0), &want);
}

/// GF(64) with x^6 + x + 1 (the JT65 field; the QRA paper slides do not name a polynomial).
#[test]
fn gf64_field() {
    let f = Gf::new(6, 0x43).unwrap();
    assert_eq!(f.exp(6), 0b11);
    assert_eq!(f.exp(63), 1);
    let code = ra_code(6, 0x43, &shape_65_15(), 1);
    assert_eq!(code.field(), &f);
    assert_eq!(code.q(), 64);
    // Known symbols a and b, and x2 punctured in α^5 x0 + α^40 x1 + α^17 x2 = 0, so x2 = α^-17 (α^5 a + α^40 b).
    let (a, b) = (37u16, 12u16);
    let text = format!("64 3 2\n0:{} 1:{} 2:{}\n", f.exp(5), f.exp(40), f.exp(17));
    let code = NbCode::parse(&text).unwrap();
    let mut probs = vec![0f32; 3 * 64];
    probs[a as usize] = 1.0;
    probs[64 + b as usize] = 1.0;
    probs[128..].fill(1.0);
    let mut dec = NbDecoder::new(&code);
    let mut hard = [0u16; 3];
    dec.decode(&probs, 3, &mut hard).unwrap();
    let want = f
        .div(f.mul(f.exp(5), a) ^ f.mul(f.exp(40), b), f.exp(17))
        .unwrap();
    assert_eq!(hard, [a, b, want]);
    assert!(dec.posterior_of(2)[want as usize] > 0.999);
}

#[test]
fn repeat_accumulate_matches_the_accumulator() {
    let f = Gf::new(6, 0x43).unwrap();
    let s = shape_65_15();
    let code = ra_code(6, 0x43, &s, 1);
    assert_eq!((code.n(), code.k(), code.checks()), (65, 15, 50));
    // Each check holds one repeated value, y[t] and y[t - 1] (not for t = 0).
    assert_eq!(code.edges(), 50 + 50 + 49);
    let mut rng = Rng(7);
    let mut word = vec![0u16; 65];
    for _ in 0..50 {
        let info = info_of(&mut rng, 15, 64);
        code.encode(&info, &mut word).unwrap();
        assert_eq!(word, accumulate(&f, &s, 1, &info));
        assert_eq!(code.unsatisfied(&word).unwrap(), 0);
    }
    word[20] ^= 1;
    assert!(code.unsatisfied(&word).unwrap() > 0);

    // Two values per accumulator step, with a GF(16) field.
    let f16 = Gf::new(4, 0x13).unwrap();
    let s = ra_shape(4, vec![3, 4, 2, 3, 4, 4], 99);
    let code = ra_code(4, 0x13, &s, 2);
    assert_eq!((code.n(), code.k()), (6 + 10, 6));
    let mut word = vec![0u16; 16];
    for _ in 0..50 {
        let info = info_of(&mut rng, 6, 16);
        code.encode(&info, &mut word).unwrap();
        assert_eq!(word, accumulate(&f16, &s, 2, &info));
    }
}

#[test]
fn text_form_round_trip() {
    let code = ra_code(6, 0x43, &shape_65_15(), 1);
    let text = code.to_text();
    let again = NbCode::parse(&format!("# The test code.\n{text}\n# end\n")).unwrap();
    assert_eq!(again.n(), 65);
    for c in 0..code.checks() {
        assert!(code.check(c).eq(again.check(c)));
    }
    let info: Vec<u16> = (0..15).map(|i| ((i * 5 + 3) % 64) as u16).collect();
    let (mut a, mut b) = (vec![0u16; 65], vec![0u16; 65]);
    code.encode(&info, &mut a).unwrap();
    again.encode(&info, &mut b).unwrap();
    assert_eq!(a, b);

    // Information symbols at the end, values in hexadecimal.
    let code = NbCode::parse("4 3 2 0x7 # GF(4)\ninfo 1 2\n0:0x1 1:0x2 2:0x3\n").unwrap();
    let mut w = [0u16; 3];
    code.encode(&[1, 1], &mut w).unwrap();
    // x0 = α + α^2 = 2 ^ 3 = 1.
    assert_eq!(w, [1, 1, 1]);
    assert_eq!(code.info_positions(), &[1, 2]);
}

#[test]
fn decodes_clean_noisy_and_punctured_words() {
    let s = shape_65_15();
    let code = ra_code(6, 0x43, &s, 1);
    let mut dec = NbDecoder::new(&code);
    let mut rng = Rng(65);
    let mut word = vec![0u16; 65];
    let mut hard = vec![0u16; 65];

    // Clean input.
    let info = info_of(&mut rng, 15, 64);
    code.encode(&info, &mut word).unwrap();
    let mut probs = vec![0.1f32 / 63.0; 65 * 64];
    for (i, &x) in word.iter().enumerate() {
        probs[i * 64 + x as usize] = 0.9;
    }
    let r = dec.decode(&probs, 50, &mut hard).unwrap();
    assert_eq!(hard, word);
    assert_eq!((r.iterations, r.corrected, r.metric), (1, 0, 0));

    // Clean input with information symbols 13 and 14 punctured.
    for i in [13, 14] {
        probs[i * 64..(i + 1) * 64].fill(1.0);
    }
    dec.decode(&probs, 50, &mut hard).unwrap();
    assert_eq!(hard, word);

    // Noise at several amplitudes, with and without information symbols 13 and 14 punctured. The table goes to the test
    // output. The bounds hold with a margin for this seed.
    let trials = 200;
    let mut table = Vec::new();
    for (amp, punctured) in [
        (1.6, false),
        (2.0, false),
        (2.4, false),
        (3.0, false),
        (2.4, true),
        (3.0, true),
    ] {
        let (mut ok, mut wrong_total, mut iters) = (0, 0, 0);
        for _ in 0..trials {
            let info = info_of(&mut rng, 15, 64);
            code.encode(&info, &mut word).unwrap();
            let (mut probs, wrong) = noisy(&word, 64, amp, &mut rng);
            wrong_total += wrong;
            if punctured {
                for i in [13, 14] {
                    probs[i * 64..(i + 1) * 64].fill(1.0);
                }
            }
            if let Ok(r) = dec.decode(&probs, 50, &mut hard) {
                if hard == word {
                    ok += 1;
                    iters += r.iterations;
                    if !punctured {
                        assert_eq!(r.corrected as usize, wrong);
                    }
                }
            }
        }
        println!(
            "GF(64) (65, 15), amplitude {amp}, punctured {punctured}: {:.1} wrong symbols per word, {ok} of {trials} decoded, {:.1} iterations on average",
            wrong_total as f64 / trials as f64,
            iters as f64 / ok.max(1) as f64
        );
        table.push((amp, punctured, wrong_total, ok));
    }
    // At amplitude 3.0 about one symbol in four is wrong before decoding.
    assert!(table[3].2 > 10 * trials);
    for &(amp, _, _, ok) in &table {
        if amp >= 3.0 {
            assert!(ok >= trials * 95 / 100, "{ok} of {trials} at {amp}");
        }
    }
    // More signal never decodes fewer words by much.
    assert!(table[0].3 <= table[2].3 + 10);
}

/// The natural logarithm of the modified Bessel function I0, by its power series below 12 and its asymptotic form above.
fn ln_i0(x: f64) -> f64 {
    if x < 12.0 {
        let (mut term, mut sum) = (1.0, 1.0);
        for k in 1..60 {
            term *= (x / (2.0 * k as f64)).powi(2);
            sum += term;
        }
        sum.ln()
    } else {
        x - 0.5 * (std::f64::consts::TAU * x).ln() + (1.0 + 1.0 / (8.0 * x)).ln()
    }
}

/// Likelihood rows for `word` from non-coherent orthogonal signalling: tone `a` has the envelope `r_a = |amp [a = x] + w|` with
/// `w` complex, its two parts standard normal, so Es/N0 = amp² / 2. `P(a) ∝ I0(amp r_a)`.
fn envelopes(word: &[u16], q: usize, amp: f64, rng: &mut Rng) -> Vec<f32> {
    let mut probs = Vec::with_capacity(word.len() * q);
    for &x in word {
        let l: Vec<f64> = (0..q)
            .map(|a| {
                let re = amp * ((a == x as usize) as u8 as f64) + rng.gauss();
                ln_i0(amp * re.hypot(rng.gauss()))
            })
            .collect();
        let top = l.iter().copied().fold(f64::MIN, f64::max);
        probs.extend(l.iter().map(|&v| (v - top).exp() as f32));
    }
    probs
}

/// A check that the other checks imply (their sum) is accepted and changes no code word: 51 checks of rank 50.
#[test]
fn a_redundant_check_is_accepted() {
    let s = shape_65_15();
    let code = ra_code(6, 0x43, &s, 1);
    // The sum of all 50 accumulator checks: each inner output cancels, the last output and the weighted sources stay.
    let mut sum = [0u16; 65];
    let mut text = String::from("64 65 15 0x43\n");
    for c in 0..code.checks() {
        let line: Vec<String> = code
            .check(c)
            .map(|(sym, h)| {
                sum[sym] ^= h;
                format!("{sym}:{h}")
            })
            .collect();
        text += &line.join(" ");
        text.push('\n');
    }
    let closing: Vec<String> = (0..65)
        .filter(|&i| sum[i] != 0)
        .map(|i| format!("{i}:{}", sum[i]))
        .collect();
    text += &closing.join(" ");
    text.push('\n');
    let closed = NbCode::parse(&text).unwrap();
    assert_eq!((closed.checks(), closed.n(), closed.k()), (51, 65, 15));
    let mut rng = Rng(51);
    let (mut a, mut b) = (vec![0u16; 65], vec![0u16; 65]);
    for _ in 0..200 {
        let info = info_of(&mut rng, 15, 64);
        code.encode(&info, &mut a).unwrap();
        closed.encode(&info, &mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(closed.unsatisfied(&b).unwrap(), 0);
    }
}

/// The (65, 15) code over GF(64) with the stop rule on the extrinsic distributions, as a punctured (63, 13) code (information
/// symbols 13 and 14 not sent): no noise; 12 erased symbols (at least 95 of 100 words); non-coherent 64-ary signalling at
/// Es/N0 4.6 dB (at least 75 % of 200 words) and 6.5 dB (at least 97 %); noise alone (no word).
/// The counts go to the test output.
#[test]
fn confidence_stop_decodes_clean_erased_and_noisy_words() {
    let s = shape_65_15();
    let code = ra_code(6, 0x43, &s, 1);
    let mut dec = NbDecoder::new(&code);
    dec.set_stop(NbStop::Confidence { slack: 0.01 });
    let mut rng = Rng(6513);
    let mut word = vec![0u16; 65];
    let mut hard = vec![0u16; 65];
    let clean = |word: &[u16]| {
        let mut probs = vec![0.01f32 / 63.0; 65 * 64];
        for (i, &x) in word.iter().enumerate() {
            probs[i * 64 + x as usize] = 0.99;
        }
        for i in [13, 14] {
            probs[i * 64..(i + 1) * 64].fill(1.0);
        }
        probs
    };

    // No noise: every word, and the extrinsic rows name the word without the channel rows.
    for _ in 0..30 {
        let info = info_of(&mut rng, 15, 64);
        code.encode(&info, &mut word).unwrap();
        let r = dec.decode(&clean(&word), 100, &mut hard).unwrap();
        assert_eq!(hard, word);
        assert_eq!(r.metric, 0);
        for (row, &x) in dec.extrinsic().as_chunks::<64>().0.iter().zip(&word) {
            assert!(row[x as usize] > 0.99);
            assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
    }

    // Erasures: 12 of the 63 sent symbols carry no information (a row of equal values), the others are clean.
    let mut ok = 0;
    for _ in 0..100 {
        let info = info_of(&mut rng, 15, 64);
        code.encode(&info, &mut word).unwrap();
        let mut probs = clean(&word);
        let mut lost = vec![13, 14];
        while lost.len() < 14 {
            let i = rng.below(65);
            if !lost.contains(&i) {
                probs[i * 64..(i + 1) * 64].fill(1.0);
                lost.push(i);
            }
        }
        if dec.decode(&probs, 100, &mut hard).is_ok() {
            assert_eq!(hard, word, "a confident result is the sent word");
            ok += 1;
        }
    }
    println!("GF(64) (63, 13), 12 erasures: {ok} of 100 decoded");
    assert!(ok >= 95, "{ok} of 100 with 12 erasures");

    // Noise. Es/N0 = amp² / 2: 4.6 dB at amplitude 2.4, 6.5 dB at amplitude 3.0.
    let trials = 200;
    let mut counts = Vec::new();
    for amp in [2.4, 3.0] {
        let (mut ok, mut wrong) = (0, 0);
        for _ in 0..trials {
            let info = info_of(&mut rng, 15, 64);
            code.encode(&info, &mut word).unwrap();
            let mut probs = envelopes(&word, 64, amp, &mut rng);
            for i in [13, 14] {
                probs[i * 64..(i + 1) * 64].fill(1.0);
            }
            if dec.decode(&probs, 100, &mut hard).is_ok() {
                if hard == word {
                    ok += 1;
                } else {
                    wrong += 1;
                }
            }
        }
        println!(
            "GF(64) (63, 13), Es/N0 {:.1} dB: {ok} of {trials} decoded, {wrong} wrong words",
            10.0 * (amp * amp / 2.0).log10()
        );
        counts.push((ok, wrong));
    }
    assert!(counts[0].0 >= trials * 75 / 100, "{:?}", counts[0]);
    assert!(counts[1].0 >= trials * 97 / 100, "{:?}", counts[1]);
    assert!(counts[0].1 + counts[1].1 <= 2, "{counts:?}");

    // Noise alone never passes the rule.
    for _ in 0..100 {
        let probs = envelopes(&[0u16; 65], 64, 0.0, &mut rng);
        assert_eq!(dec.decode(&probs, 100, &mut hard), Err(FecError::Uncorrectable));
    }
}

#[test]
fn decodes_over_gf256() {
    let s = ra_shape(8, vec![3; 8], 256);
    let code = ra_code(8, 0x11D, &s, 1);
    assert_eq!(code.n(), 32);
    let mut dec = NbDecoder::new(&code);
    let mut rng = Rng(8);
    let (mut word, mut hard) = (vec![0u16; 32], vec![0u16; 32]);
    let mut ok = 0;
    for _ in 0..20 {
        let info = info_of(&mut rng, 8, 256);
        code.encode(&info, &mut word).unwrap();
        let (probs, _) = noisy(&word, 256, 3.5, &mut rng);
        if dec.decode(&probs, 30, &mut hard).is_ok() && hard == word {
            ok += 1;
        }
    }
    assert!(ok >= 16, "{ok} of 20");
}

#[test]
fn failure_reports_the_last_iteration() {
    let code = ra_code(6, 0x43, &shape_65_15(), 1);
    let mut dec = NbDecoder::new(&code);
    let mut rng = Rng(3);
    let probs: Vec<f32> = (0..65 * 64).map(|_| rng.uniform() as f32).collect();
    let mut hard = vec![0u16; 65];
    assert_eq!(
        dec.decode(&probs, 3, &mut hard),
        Err(FecError::Uncorrectable)
    );
    assert!(code.unsatisfied(&hard).unwrap() > 0);
    for row in dec.posterior().chunks(64) {
        assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }
}

#[test]
fn bad_input_returns_errors() {
    let code = NbCode::parse("4 3 2\n0:1 1:1 2:1\n").unwrap();
    let mut dec = NbDecoder::new(&code);
    let mut hard = [0u16; 3];
    let good = [1.0f32; 12];
    assert_eq!(
        dec.decode(&good[..11], 5, &mut hard),
        Err(FecError::InputLength {
            expected: 12,
            got: 11
        })
    );
    assert_eq!(
        dec.decode(&good, 5, &mut hard[..2]),
        Err(FecError::OutputTooShort { need: 3 })
    );
    let mut bad = good;
    bad[5] = f32::NAN;
    assert_eq!(
        dec.decode(&bad, 5, &mut hard),
        Err(FecError::NotFinite { at: 5 })
    );
    bad[5] = f32::INFINITY;
    assert_eq!(
        dec.decode(&bad, 5, &mut hard),
        Err(FecError::NotFinite { at: 5 })
    );
    bad[5] = -0.5;
    assert!(matches!(
        dec.decode(&bad, 5, &mut hard),
        Err(FecError::NotAllowed { .. })
    ));
    bad = good;
    bad[4..8].fill(0.0);
    assert!(matches!(
        dec.decode(&bad, 5, &mut hard),
        Err(FecError::Degenerate { .. })
    ));

    assert_eq!(
        code.encode(&[1], &mut hard),
        Err(FecError::InputLength {
            expected: 2,
            got: 1
        })
    );
    assert!(code.encode(&[1, 4], &mut hard).is_err());
    assert_eq!(
        code.encode(&[1, 2], &mut hard[..2]),
        Err(FecError::OutputTooShort { need: 3 })
    );
    assert!(code.unsatisfied(&hard[..2]).is_err());

    for text in [
        "",
        "4 3\n0:1 1:1 2:1\n",
        "5 3 2\n0:1 1:1 2:1\n",
        "8192 3 2\n0:1\n",
        "512 3 2\n0:1 1:1 2:1\n",
        "4 3 2 0x7 9\n0:1 1:1 2:1\n",
        "4 3 2\n",
        "4 3 2\n0:1 1:1 3:1\n",
        "4 3 2\n0:0 1:1 2:1\n",
        "4 3 2\n0:4 1:1 2:1\n",
        "4 3 2\n0:1 0:1 2:1\n",
        "4 3 2\n0-1 1:1 2:1\n",
        "4 3 2\n0:x 1:1 2:1\n",
        "4 3 2\ninfo 0\n0:1 1:1 2:1\n",
        "4 3 2\ninfo 0 0\n0:1 1:1 2:1\n",
        "4 3 2\n0:1 1:1 2:1\ninfo 0 1\n",
        "4 3 2 0x5\n0:1 1:1 2:1\n",
        "4 3 3\n0:1 1:1 2:1\n",
        // The parity symbol x2 is in no check.
        "4 3 2\n0:1 1:1\n",
        // Two checks on x2 alone and x0, x1 alone: the second constrains the information symbols.
        "4 3 2\n2:1\n0:1 1:1\n",
    ] {
        assert!(NbCode::parse(text).is_err(), "{text:?}");
    }
    assert!(NbCode::new(13, 0x201B, 3, &[&[(0, 1)]], &[1]).is_err());

    let s = shape_65_15();
    let ra = |repeats: &[usize], permutation: &[usize], weights: &[u16], combine| {
        NbCode::repeat_accumulate(
            6,
            0x43,
            &RepeatAccumulate {
                repeats,
                permutation,
                weights,
                combine,
            },
        )
    };
    assert!(ra(&s.repeats, &s.permutation, &s.weights, 1).is_ok());
    assert!(ra(&s.repeats, &s.permutation, &s.weights, 3).is_err());
    assert!(ra(&s.repeats, &s.permutation, &s.weights, 0).is_err());
    assert!(ra(&s.repeats, &s.permutation[1..], &s.weights, 1).is_err());
    let mut p = s.permutation.clone();
    p[0] = p[1];
    assert!(ra(&s.repeats, &p, &s.weights, 1).is_err());
    let mut w = s.weights.clone();
    w[3] = 0;
    assert!(ra(&s.repeats, &s.permutation, &w, 1).is_err());
    w[3] = 64;
    assert!(ra(&s.repeats, &s.permutation, &w, 1).is_err());
    let mut r = s.repeats.clone();
    r[0] = 0;
    assert!(ra(&r, &s.permutation, &s.weights, 1).is_err());
}

#[test]
fn decode_and_encode_do_not_allocate() {
    let code = ra_code(6, 0x43, &shape_65_15(), 1);
    let mut dec = NbDecoder::new(&code);
    let mut rng = Rng(11);
    let mut word = vec![0u16; 65];
    let mut hard = vec![0u16; 65];
    let info = info_of(&mut rng, 15, 64);
    code.encode(&info, &mut word).unwrap();
    let (good, _) = noisy(&word, 64, 3.0, &mut rng);
    let bad: Vec<f32> = (0..65 * 64).map(|_| rng.uniform() as f32).collect();
    let _ = dec.decode(&good, 50, &mut hard);
    let before = allocs();
    for _ in 0..100 {
        code.encode(&info, &mut word).unwrap();
        let _ = dec.decode(&good, 50, &mut hard);
        let _ = dec.decode(&bad, 20, &mut hard);
        let _ = code.unsatisfied(&hard);
    }
    assert_eq!(allocs() - before, 0);
}
