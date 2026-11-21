//! The 64-bit xorshift generator with the shift triple (13, 7, 17), for tables and test data that come from a seed.
//!
//! Standard: G. Marsaglia, "Xorshift RNGs", Journal of Statistical Software 8 (14), 2003.

/// A 64-bit xorshift generator. One step is `s ^= s << 13`, `s ^= s >> 7`, `s ^= s << 17`.
///
/// A state of 0 stays 0. Use [`Xorshift64::with_zero_seed`] when a protocol replaces a seed of 0.
///
/// ```
/// use sdrkit_util_bits::xorshift::Xorshift64;
/// let mut g = Xorshift64::new(1);
/// assert_eq!(g.below(10), 2);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    /// The start state used in place of a seed of 0 (the 64-bit golden ratio).
    pub const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

    /// Makes a generator with the state `seed`, as it is.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Makes a generator with the state `seed`. If `seed` is 0, the state is `zero_state`.
    pub const fn with_zero_seed(seed: u64, zero_state: u64) -> Self {
        Self {
            state: if seed == 0 { zero_state } else { seed },
        }
    }

    /// Returns the current state.
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// Does one step and returns the new state.
    #[inline]
    pub fn step(&mut self) -> u64 {
        let mut s = self.state;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.state = s;
        s
    }

    /// Returns a value in `0 .. n` as `(step() >> 11) mod n`. If `n` is 0, it returns 0 and does no step.
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        (self.step() >> 11) % n
    }
}

impl Iterator for Xorshift64 {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        Some(self.step())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // From seed 0 (state 0x9E3779B97F4A7C15), the first three steps.
    #[test]
    fn known_steps_from_the_golden_state() {
        let mut g = Xorshift64::with_zero_seed(0, Xorshift64::GOLDEN);
        assert_eq!(g.step(), 0xDC1B_77AE_0BF3_4DAD);
        assert_eq!(g.step(), 0x64F0_EEB9_026E_6076);
        assert_eq!(g.step(), 0x7B07_CE91_E590_6136);
    }

    // From seed 1, five calls of below(10).
    #[test]
    fn known_below_values_from_seed_one() {
        let mut g = Xorshift64::new(1);
        let v: Vec<u64> = (0..5).map(|_| g.below(10)).collect();
        assert_eq!(v, [2, 2, 0, 6, 0]);
    }

    // Marsaglia 2003: a state of 0 stays 0, and a nonzero seed is used as it is.
    #[test]
    fn zero_state_and_plain_seed() {
        let mut z = Xorshift64::new(0);
        assert_eq!(z.step(), 0);
        assert_eq!(Xorshift64::with_zero_seed(5, 9).state(), 5);
        assert_eq!(Xorshift64::new(7).below(0), 0);
    }
}
