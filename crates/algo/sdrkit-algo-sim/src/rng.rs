//! The one seeded random number generator of sdrkit: xoshiro256** (Blackman and Vigna, 2018) seeded through SplitMix64, with
//! uniform, integer, bit, exponential and Gaussian draws.
//!
//! Every simulation in sdrkit draws its random values from here, so the same seed produces the same signal on every platform and in every
//! build. It is not for cryptography.
//!
//! - [`SplitMix64`] turns any 64-bit seed (including 0) into well-mixed words; it fills the xoshiro state and derives the seeds of
//!   independent streams.
//! - [`Rng`] is xoshiro256** (period 2^256 - 1). [`Rng::stream`] returns a generator for one named part of a simulation (the noise,
//!   each fading tap, the phase noise), so adding a part never changes the numbers another part draws.
//! - Gaussian values come from the ziggurat method ([`crate::gauss`]): about one 64-bit draw and one multiply per value.
//!
//! ```
//! use sdrkit_algo_sim::rng::Rng;
//! let mut a = Rng::new(7);
//! let mut b = Rng::new(7);
//! assert_eq!(a.next_u64(), b.next_u64());
//! let x = a.normal();
//! assert!(x.is_finite());
//! ```

use crate::gauss::{self, Ziggurat};
use crate::{Param, SimError};
use sdrkit_core::Complex32;

/// SplitMix64 (Steele, Lea and Flood, 2014, as published by Vigna with xoshiro): a 64-bit counter passed through a mixing
/// function. Used to seed [`Rng`] and to derive stream seeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator starting from `seed`.
    pub const fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The mixing function alone (a bijection of 64-bit words).
    #[inline]
    pub const fn mix(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next word.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        Self::mix(self.state)
    }
}

/// A fast seeded generator: xoshiro256** with its state filled by [`SplitMix64`].
///
/// The same seed produces the same sequence on every platform. Draws:
///
/// | Method | Returns |
/// |---|---|
/// | [`next_u64`](Self::next_u64), [`next_u32`](Self::next_u32) | raw bits |
/// | [`uniform`](Self::uniform) / [`uniform_f32`](Self::uniform_f32) | `[0, 1)` |
/// | [`uniform_open`](Self::uniform_open) | `(0, 1]` (safe for `ln`) |
/// | [`range`](Self::range) | `[lo, hi)` |
/// | [`below`](Self::below) | an integer in `0..n`, without bias |
/// | [`bit`](Self::bit), [`fill_bits`](Self::fill_bits), [`fill_bytes`](Self::fill_bytes) | data for a test frame |
/// | [`exponential`](Self::exponential) | mean 1 |
/// | [`normal`](Self::normal), [`normal_f32`](Self::normal_f32), [`fill_normal`](Self::fill_normal) | mean 0, variance 1 |
/// | [`complex_normal`](Self::complex_normal) | circular complex Gaussian of a given power |
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
    zig: &'static Ziggurat,
}

impl Rng {
    /// A generator seeded with `seed` (any value, including 0): the four state words are the first four outputs of
    /// `SplitMix64::new(seed)`, as the xoshiro authors recommend.
    pub fn new(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        Rng {
            s: [sm.next(), sm.next(), sm.next(), sm.next()],
            zig: gauss::tables(),
        }
    }

    /// A generator with this exact state (for checks against published sequences). The all-zero state is refused: xoshiro would
    /// output zeros forever.
    pub fn from_state(s: [u64; 4]) -> Result<Self, SimError> {
        if s == [0; 4] {
            return Param::State.degenerate();
        }
        Ok(Rng {
            s,
            zig: gauss::tables(),
        })
    }

    /// The generator of stream `stream` of a simulation seeded with `seed`: each part of a simulation (noise, each fading path,
    /// phase noise, impulses) uses its own stream, so the parts are independent of each other and adding one part does not
    /// change what another draws. The stream seed is `SplitMix64::mix(seed') ^ SplitMix64::mix(stream + golden)`, where `seed'` is
    /// the first output of `SplitMix64::new(seed)`.
    pub fn stream(seed: u64, stream: u64) -> Self {
        let base = SplitMix64::new(seed).next();
        Rng::new(base ^ SplitMix64::mix(stream.wrapping_add(0x9E37_79B9_7F4A_7C15)))
    }

    /// The state words (for saving and restoring a simulation).
    pub fn state(&self) -> [u64; 4] {
        self.s
    }

    /// The next 64 random bits.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// The next 32 random bits (the upper half of a 64-bit draw, the better half).
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in `[0, 1)`, with 53 random bits.
    #[inline]
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in `(0, 1]` (never 0, so `ln` of it is finite).
    #[inline]
    pub fn uniform_open(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in `[0, 1)` as `f32`, with 24 random bits.
    #[inline]
    pub fn uniform_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Uniform in `[lo, hi)`.
    #[inline]
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    /// A uniform integer in `0..n` without bias (Lemire's multiply-and-reject). `n = 0` returns 0.
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let threshold = n.wrapping_neg() % n;
        loop {
            let m = self.next_u64() as u128 * n as u128;
            if (m as u64) >= threshold {
                return (m >> 64) as u64;
            }
        }
    }

    /// One random bit, 0 or 1.
    #[inline]
    pub fn bit(&mut self) -> u8 {
        (self.next_u64() >> 63) as u8
    }

    /// Fills `out` with random bits (0 or 1, one per byte), 64 from each draw.
    pub fn fill_bits(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(64) {
            let w = self.next_u64();
            for (i, b) in chunk.iter_mut().enumerate() {
                *b = ((w >> (63 - i)) & 1) as u8;
            }
        }
    }

    /// Fills `out` with random bytes, 8 from each draw (little-endian order of the word).
    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let w = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&w[..chunk.len()]);
        }
    }

    /// An exponential value of mean 1 (`-ln U`).
    #[inline]
    pub fn exponential(&mut self) -> f64 {
        -self.uniform_open().ln()
    }

    /// A standard normal value (mean 0, variance 1), by the ziggurat method ([`crate::gauss`]).
    #[inline]
    pub fn normal(&mut self) -> f64 {
        let z = self.zig;
        gauss::sample(z, self)
    }

    /// [`normal`](Self::normal) as `f32`.
    #[inline]
    pub fn normal_f32(&mut self) -> f32 {
        self.normal() as f32
    }

    /// Fills `out` with standard normal values.
    pub fn fill_normal(&mut self, out: &mut [f32]) {
        for v in out {
            *v = self.normal() as f32;
        }
    }

    /// A circular complex Gaussian value of mean power `power` (`E|z|^2 = power`, so each of I and Q has variance `power / 2`).
    #[inline]
    pub fn complex_normal(&mut self, power: f64) -> Complex32 {
        let s = (0.5 * power).sqrt();
        Complex32::new((self.normal() * s) as f32, (self.normal() * s) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_published_outputs() {
        // splitmix64.c (Vigna) from seed 1234567: the first five outputs.
        let mut s = SplitMix64::new(1_234_567);
        let want = [
            6_457_827_717_110_365_317u64,
            3_203_168_211_198_807_973,
            9_817_491_932_198_370_423,
            4_593_380_528_125_082_431,
            16_408_922_859_458_223_821,
        ];
        for w in want {
            assert_eq!(s.next(), w);
        }
    }

    #[test]
    fn xoshiro256starstar_matches_the_published_outputs() {
        // xoshiro256starstar.c (Blackman and Vigna) from the state {1, 2, 3, 4}.
        let mut r = Rng::from_state([1, 2, 3, 4]).unwrap();
        let want = [
            11_520u64,
            0,
            1_509_978_240,
            1_215_971_899_390_074_240,
            1_216_172_134_540_287_360,
            607_988_272_756_665_600,
            16_172_922_978_634_559_625,
            8_476_171_486_693_032_832,
            10_595_114_339_597_558_777,
            2_904_607_092_377_533_576,
        ];
        for w in want {
            assert_eq!(r.next_u64(), w);
        }
        assert!(Rng::from_state([0; 4]).is_err());
    }

    #[test]
    fn seeds_and_streams_are_repeatable_and_distinct() {
        let a: Vec<u64> = (0..4)
            .map(|_| 0)
            .scan(Rng::new(9), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..4)
            .map(|_| 0)
            .scan(Rng::new(9), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(Rng::new(10).next_u64(), a[0]);
        assert_ne!(Rng::stream(9, 0).next_u64(), Rng::stream(9, 1).next_u64());
        assert_eq!(Rng::stream(9, 3).next_u64(), Rng::stream(9, 3).next_u64());
    }

    #[test]
    fn uniform_draws_stay_in_range_and_below_is_unbiased() {
        let mut r = Rng::new(3);
        for _ in 0..10_000 {
            let u = r.uniform();
            assert!((0.0..1.0).contains(&u));
            let o = r.uniform_open();
            assert!(o > 0.0 && o <= 1.0);
            let f = r.uniform_f32();
            assert!((0.0..1.0).contains(&f));
        }
        assert_eq!(r.below(0), 0);
        let mut counts = [0u32; 3];
        for _ in 0..300_000 {
            counts[r.below(3) as usize] += 1;
        }
        for c in counts {
            assert!((c as f64 - 100_000.0).abs() < 1_000.0, "{counts:?}");
        }
        let mut bits = [0u8; 1000];
        r.fill_bits(&mut bits);
        let ones = bits.iter().map(|&b| b as u32).sum::<u32>();
        assert!(bits.iter().all(|&b| b <= 1) && (400..600).contains(&ones));
        let mut bytes = [0u8; 13];
        r.fill_bytes(&mut bytes);
        assert!(bytes.iter().any(|&b| b != 0));
        let mean = (0..100_000).map(|_| r.exponential()).sum::<f64>() / 100_000.0;
        assert!((mean - 1.0).abs() < 0.02, "{mean}");
    }
}
