//! Test helpers: a seeded random generator, Gaussian noise and BPSK LLRs (positive means 0).
#![allow(dead_code)]

/// xoshiro256** seeded by splitmix64.
pub struct Rng([u64; 4]);

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut s = [0u64; 4];
        for v in s.iter_mut() {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *v = x ^ (x >> 31);
        }
        Rng(s)
    }
    pub fn next(&mut self) -> u64 {
        let s = &mut self.0;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }
    pub fn bit(&mut self) -> u8 {
        (self.next() >> 63) as u8
    }
    pub fn bits(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.bit()).collect()
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// Uniform in (0, 1].
    pub fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }
    pub fn gauss(&mut self) -> f64 {
        let u1 = self.uniform();
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// BPSK (0 -> +1) through AWGN at `ebn0_db` for a code of rate `rate`; returns f32 LLRs `2 y / sigma^2`.
pub fn bpsk_llr(rng: &mut Rng, bits: &[u8], rate: f64, ebn0_db: f64) -> Vec<f32> {
    let esn0 = 10f64.powf(ebn0_db / 10.0) * rate;
    let sigma2 = 1.0 / (2.0 * esn0);
    let sigma = sigma2.sqrt();
    bits.iter()
        .map(|&b| {
            let x = if b == 0 { 1.0 } else { -1.0 };
            let y = x + sigma * rng.gauss();
            (2.0 * y / sigma2) as f32
        })
        .collect()
}

/// The same, quantised to i8 with the crate's quantizer (target mean |L| `target`).
pub fn bpsk_llr8(rng: &mut Rng, bits: &[u8], rate: f64, ebn0_db: f64, target: f32) -> Vec<i8> {
    let f = bpsk_llr(rng, bits, rate, ebn0_db);
    let q = sdrkit_algo_fec::Quantizer {
        target,
        ..Default::default()
    };
    let mut out = vec![0i8; f.len()];
    q.quantize_auto(&f, &mut out).unwrap();
    out
}

/// Hard bits to full-strength i8 LLRs.
pub fn hard_llr(bits: &[u8], mag: i8) -> Vec<i8> {
    bits.iter()
        .map(|&b| if b == 0 { mag } else { -mag })
        .collect()
}

pub fn errors(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

/// Parses a string of 0/1 characters (spaces ignored).
pub fn bits_of(s: &str) -> Vec<u8> {
    s.bytes()
        .filter(|c| *c == b'0' || *c == b'1')
        .map(|c| c - b'0')
        .collect()
}
