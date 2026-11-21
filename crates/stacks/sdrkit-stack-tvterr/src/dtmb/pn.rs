//! DTMB frame headers: PN420 (a 255-chip m-sequence continued periodically), PN595 (595 chips of a 1023-chip m-sequence, not
//! periodic) and PN945 (a 511-chip m-sequence continued periodically), their generators, and the phase schedule of the
//! super-frame.
//!
//! Standard: GB 20600-2006 clause 4.4. The generators and the schedule come from the specification of the DTMB frame decoder;
//! no printed copy of the standard was compared with them.

use sdrkit_core::Complex32;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq;

sdrkit_core::choice_enum! {
    /// The kind of frame header.
    pub enum Header {
        /// 420 symbols, 225 frames per super-frame.
        Pn420 = ("pn420", "PN420"),
        /// 595 symbols, 216 frames per super-frame, not boosted.
        Pn595 = ("pn595", "PN595"),
        /// 945 symbols, 200 frames per super-frame.
        Pn945 = ("pn945", "PN945"),
    }
}

impl Header {
    /// Symbols in the header.
    pub fn len(self) -> usize {
        match self {
            Header::Pn420 => 420,
            Header::Pn595 => 595,
            Header::Pn945 => 945,
        }
    }
    /// Never empty.
    pub fn is_empty(self) -> bool {
        false
    }
    /// The core length and the chips before and after it (for PN595: the whole header as "core").
    pub fn core_pre_post(self) -> (usize, usize, usize) {
        match self {
            Header::Pn420 => (255, 82, 83),
            Header::Pn595 => (595, 0, 0),
            Header::Pn945 => (511, 217, 217),
        }
    }
    /// "PN420" ...
    pub fn name(self) -> &'static str {
        match self {
            Header::Pn420 => "PN420",
            Header::Pn595 => "PN595",
            Header::Pn945 => "PN945",
        }
    }
    /// Frames per super-frame.
    pub fn frames_per_superframe(self) -> usize {
        match self {
            Header::Pn420 => 225,
            Header::Pn595 => 216,
            Header::Pn945 => 200,
        }
    }
    /// Header power over body power.
    pub fn power_ratio(self) -> f32 {
        match self {
            Header::Pn595 => 1.0,
            _ => 2.0,
        }
    }
    /// The amplitude `a` of each component of a header symbol `a (1 + j) c`.
    pub fn amplitude(self) -> f32 {
        (self.power_ratio() / 2.0).sqrt()
    }
    /// The period of the generator's sequence (1023 for PN595, of which 595 chips are used).
    pub fn period(self) -> usize {
        match self {
            Header::Pn420 => 255,
            Header::Pn595 => 1023,
            Header::Pn945 => 511,
        }
    }
    /// The generator of the header's sequence.
    pub fn generator(self) -> Lfsr {
        match self {
            Header::Pn420 => seq::DTMB_PN420,
            Header::Pn595 => seq::DTMB_PN595,
            Header::Pn945 => seq::DTMB_PN945,
        }
    }
    /// The PN phase of frame `f` of the super-frame: 0, +1, -1, +2, -2, ... up to the middle frame, then back. Always 0 for PN595.
    pub fn phase(self, f: usize) -> usize {
        if self == Header::Pn595 {
            return 0;
        }
        let (n, p) = (self.frames_per_superframe(), self.period());
        let t = n / 2;
        let mut i = f % n;
        if i > t {
            i = 2 * t - i;
        }
        match i {
            0 => 0,
            _ if i % 2 == 1 => i.div_ceil(2),
            _ => p - i / 2,
        }
    }
}

/// The first `n` bits of the generator of `kind`.
pub fn bits(kind: Header, n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    kind.generator().fill_bits(&mut v);
    v
}

/// A header's chips and its per-frame phase.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderSeq {
    /// The kind.
    pub kind: Header,
    /// The chip of each core bit (+1 for bit 0, -1 for bit 1); for PN595 the whole header.
    chips: Vec<f32>,
    core: Vec<Complex32>,
    /// The phase stays fixed (phase 0 in every frame).
    pub fixed: bool,
}

impl HeaderSeq {
    /// The sequence of `kind`.
    pub fn new(kind: Header, fixed: bool) -> HeaderSeq {
        let (n, _, _) = kind.core_pre_post();
        let chips: Vec<f32> = bits(kind, n)
            .iter()
            .map(|&b| if b == 0 { 1.0 } else { -1.0 })
            .collect();
        let a = kind.amplitude();
        let core = chips
            .iter()
            .map(|&c| Complex32::new(a * c, a * c))
            .collect();
        HeaderSeq {
            kind,
            chips,
            core,
            fixed,
        }
    }
    /// The core symbols at phase 0 (for PN595 the whole header).
    pub fn core(&self) -> &[Complex32] {
        &self.core
    }
    /// The phase of frame `k` of the super-frame (0 for a fixed phase).
    pub fn phase(&self, k: usize) -> usize {
        if self.fixed {
            0
        } else {
            self.kind.phase(k)
        }
    }
    /// Writes the header chips (+1 or -1) of phase `phase` into `out` (header length): chip `n` is core chip `(n + phase) mod P`.
    pub fn chips(&self, phase: usize, out: &mut [f32]) {
        let p = self.chips.len();
        for (n, o) in out.iter_mut().enumerate().take(self.kind.len()) {
            *o = self.chips[(n + phase) % p];
        }
    }
    /// The header symbols of frame `k`.
    pub fn header(&self, k: usize) -> Vec<Complex32> {
        let ph = self.phase(k);
        let p = self.core.len();
        (0..self.kind.len())
            .map(|n| self.core[(n + ph) % p])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bits: &[u8]) -> String {
        bits.chunks(4)
            .map(|c| {
                let v = c
                    .iter()
                    .enumerate()
                    .fold(0u8, |a, (i, &b)| a | (b << (3 - i)));
                char::from_digit(v as u32, 16)
                    .unwrap_or('?')
                    .to_ascii_uppercase()
            })
            .collect()
    }

    /// The bits of a header (1 for chip -1).
    fn header_bits(kind: Header, k: usize, fixed: bool) -> Vec<u8> {
        HeaderSeq::new(kind, fixed)
            .header(k)
            .iter()
            .map(|s| (s.re < 0.0) as u8)
            .collect()
    }

    #[test]
    fn headers_equal_the_worked_vectors() {
        assert_eq!(
            hex(&header_bits(Header::Pn420, 0, false)),
            "B0A5E9FEA1CF0D9A3DC7407C4A22D5C8C938109BCCEFCB2B6906358AA60BAFB7614BD3FD439E1B347B8E80F89445AB91927021379"
        );
        assert_eq!(
            hex(&header_bits(Header::Pn595, 0, false)),
            "80249A6BE63E477E1C07FE389D95DEA3D2A0BFD55E8748CB59EB19F954CCA7D3846C8A6F75733BB9D4E83DB86252CD116974C581497DE31BB0F272C437F38D4A1096FAE2E43ED517B39F0"
        );
        assert_eq!(
            hex(&header_bits(Header::Pn945, 0, false)),
            "FB946DFF3259AD7E9A7C9CDC081A82531A292661E8F1233A432F2D8055BABDE0EBA16959060BE1BD4B9EDAA88E44D953B15C5DA03FB3F1884F38EEFAC28708B1F728DBFE64B35AFD34F939B8103504A634524CC3D1E24674865E5B00AB757BC1D742D2B20C17C37A973DB5511C89B2A762B8BB407F678"
        );
        // First header symbols: chip -1 gives -a (1 + j).
        assert_eq!(
            HeaderSeq::new(Header::Pn420, false).header(0)[0],
            Complex32::new(-1.0, -1.0)
        );
        let s = HeaderSeq::new(Header::Pn595, false).header(0)[0];
        let a = std::f32::consts::FRAC_1_SQRT_2;
        assert!((s.re + a).abs() < 1e-6 && (s.im + a).abs() < 1e-6);
    }

    #[test]
    fn phase_schedule_and_first_chips_per_frame() {
        let first = |kind, k| hex(&header_bits(kind, k, false)[..8]);
        for (k, ph, h) in [
            (0, 0, "B0"),
            (1, 1, "61"),
            (2, 254, "D8"),
            (3, 2, "C2"),
            (111, 56, "9A"),
            (112, 199, "83"),
            (113, 56, "9A"),
            (224, 0, "B0"),
            (225, 0, "B0"),
        ] {
            assert_eq!(
                (Header::Pn420.phase(k), first(Header::Pn420, k)),
                (ph, h.into()),
                "PN420 frame {k}"
            );
        }
        for (k, ph, h) in [
            (0, 0, "FB"),
            (1, 1, "F7"),
            (2, 510, "7D"),
            (3, 2, "EE"),
            (99, 50, "B5"),
            (100, 461, "1D"),
            (101, 50, "B5"),
            (199, 1, "F7"),
            (200, 0, "FB"),
        ] {
            assert_eq!(
                (Header::Pn945.phase(k), first(Header::Pn945, k)),
                (ph, h.into()),
                "PN945 frame {k}"
            );
        }
        for (k, ph) in [(4, 253), (5, 3), (6, 252), (12, 249)] {
            assert_eq!(Header::Pn420.phase(k), ph);
        }
        for (k, ph) in [(4, 509), (10, 506), (11, 6), (12, 505)] {
            assert_eq!(Header::Pn945.phase(k), ph);
        }
        // The first half of a super-frame has distinct phases; a fixed header and PN595 stay at 0.
        for kind in [Header::Pn420, Header::Pn945] {
            let half = kind.frames_per_superframe() / 2;
            let set: std::collections::HashSet<usize> = (0..=half).map(|k| kind.phase(k)).collect();
            assert_eq!(set.len(), half + 1);
            assert_eq!(HeaderSeq::new(kind, true).phase(7), 0);
            assert_eq!((kind.len() + 3780) * kind.frames_per_superframe(), 945_000);
        }
        assert_eq!(
            (595 + 3780) * Header::Pn595.frames_per_superframe(),
            945_000
        );
        assert_eq!(Header::Pn595.phase(17), 0);
    }

    #[test]
    fn cores_are_m_sequences_with_two_level_autocorrelation() {
        for kind in [Header::Pn420, Header::Pn945] {
            let core = bits(kind, kind.period());
            let n = core.len();
            let c = |i: usize| if core[i] == 0 { 1 } else { -1 };
            assert_eq!((0..n).map(c).sum::<i32>(), -1, "an m-sequence sums to -1");
            for lag in 1..n {
                let s: i32 = (0..n).map(|i| c(i) * c((i + lag) % n)).sum();
                assert_eq!(s, -1, "two-level autocorrelation at lag {lag}");
            }
            // The header is the core continued periodically.
            let h = bits(kind, kind.len());
            assert!((0..kind.len()).all(|i| h[i] == core[i % n]));
        }
        // PN595 comes from a register of period 1023.
        let long = bits(Header::Pn595, 2046);
        assert_eq!(&long[..1023], &long[1023..]);
        assert_eq!(Header::Pn595.generator().period(2000), Some(1023));
        assert_eq!(&long[..10], &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let h = HeaderSeq::new(Header::Pn420, false).header(3);
        assert_eq!(h.len(), 420);
        assert_eq!(&h[..82], &h[255..337]);
        assert_eq!(&h[337..], &h[82..165]);
    }
}
