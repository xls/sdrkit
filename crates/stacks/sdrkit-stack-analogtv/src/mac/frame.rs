//! The MAC line and frame layout, shared by the generator and the receiver. A line is 1296 samples of 20.25 MHz, counted from sample
//! 1291 of the line before, so the data burst is at the start.
//!
//! Standard: ETS 300 250 Part 1.

/// Samples a line.
pub const SAMPLES: usize = 1296;
/// The sample clock.
pub const CLOCK: f64 = 20.25e6;
/// Lines a frame.
pub const LINES: u32 = 625;
/// The line rate.
pub const LINE_HZ: f64 = 15_625.0;

sdrkit_core::choice_enum! {
    /// The burst format.
    pub enum Variant {
        /// D2-MAC: 105 bits at 10.125 Mbit/s on even samples.
        D2 = ("d2", "D2-MAC/packet"),
        /// D-MAC: 206 bits at 20.25 Mbit/s.
        D = ("d", "D-MAC/packet"),
    }
}

impl Variant {
    /// Bits of the burst.
    pub fn burst_bits(self) -> usize {
        match self {
            Variant::D2 => 105,
            Variant::D => 206,
        }
    }
    /// Grid position of the centre of burst bit `i`.
    #[inline]
    pub fn bit_pos(self, i: usize) -> f64 {
        match self {
            Variant::D2 => 3.0 + 2.0 * i as f64 + 0.5,
            Variant::D => i as f64,
        }
    }
    /// Bit periods a line (the energy dispersal steps).
    pub fn periods(self) -> usize {
        match self {
            Variant::D2 => 648,
            Variant::D => 1296,
        }
    }
    /// Subframes.
    pub fn subframes(self) -> usize {
        match self {
            Variant::D2 => 1,
            Variant::D => 2,
        }
    }
    /// The name.
    pub fn name(self) -> &'static str {
        match self {
            Variant::D2 => "D2-MAC/packet",
            Variant::D => "D-MAC/packet",
        }
    }
}

/// Grid position of a sample number (1..1296).
pub const fn grid(sample: usize) -> usize {
    sample + 5
}

/// The clamp period.
pub const CLAMP: (usize, usize) = (grid(211), grid(225) + 1);
/// The colour difference samples.
pub const COLOUR: (usize, usize) = (grid(236), grid(584) + 1);
/// The luminance samples.
pub const LUMA: (usize, usize) = (grid(590), grid(1286) + 1);
/// Line 624 references: grey, white, black.
pub const REF_GREY: (usize, usize) = (grid(210), grid(372));
/// The white level of line 624.
pub const REF_WHITE: (usize, usize) = (grid(372), grid(534));
/// The black level of line 624.
pub const REF_BLACK: (usize, usize) = (grid(534), grid(696));

/// Line sync word W1 (first transmitted bit first); W2 is its inverse, on alternate lines.
pub const W1: [u8; 6] = [0, 0, 1, 0, 1, 1];

/// The frame sync sequence of line 625: clock run-in then the frame sync word, MSB first.
pub const FSS: u128 = (0x5555_5555u128 << 64) | 0x65AE_F315_3F41_C246u128;
/// Bits of the frame sync sequence.
pub const FSS_BITS: usize = 96;

/// The sync word of line `l` (W1 on odd lines).
pub fn sync_word(l: u32) -> [u8; 6] {
    if l % 2 == 1 {
        W1
    } else {
        W1.map(|b| 1 - b)
    }
}

/// The frame sync sequence bit `i` for frame parity `odd` (inverted on odd frames).
pub fn fss_bit(i: usize, odd: bool) -> u8 {
    let b = ((FSS >> (FSS_BITS - 1 - i)) & 1) as u8;
    if odd {
        1 - b
    } else {
        b
    }
}

/// Line 625's special data: UDT (5), SDF (71), five RDF (94 each).
pub const UDT_BITS: usize = 5;
/// SDF length.
pub const SDF_BITS: usize = 71;
/// RDF length.
pub const RDF_BITS: usize = 94;

/// The static data frame (fields as the receiver reports them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sdf {
    /// Channel identification.
    pub chid: u16,
    /// SDF scrambling control.
    pub sdfscr: u8,
    /// MAC vision and sound control (bit 4 aspect: 1 = 4:3; bits 6 to 8 VSAM).
    pub mvscg: u8,
    /// Conditional access frame count.
    pub cafcnt: u32,
}

/// One repeated data frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rdf {
    /// TDM component id.
    pub tdmcid: u8,
    /// Frame count.
    pub fcnt: u8,
}

impl Sdf {
    /// 57 data bits (fields LSB first, concatenated).
    pub fn bits(&self) -> u128 {
        let mut v: u128 = 0;
        let mut pos = 0;
        let mut put = |x: u64, n: usize| {
            for i in 0..n {
                if (x >> i) & 1 == 1 {
                    v |= 1u128 << (56 - pos);
                }
                pos += 1;
            }
        };
        put(self.chid as u64, 16);
        put(self.sdfscr as u64, 8);
        put(self.mvscg as u64, 8);
        put(self.cafcnt as u64, 20);
        put(0, 5);
        v
    }
    /// From 57 data bits.
    pub fn from_bits(v: u128) -> Sdf {
        let mut pos = 0;
        let mut get = |n: usize| -> u64 {
            let mut x = 0u64;
            for i in 0..n {
                if (v >> (56 - pos)) & 1 == 1 {
                    x |= 1 << i;
                }
                pos += 1;
            }
            x
        };
        Sdf {
            chid: get(16) as u16,
            sdfscr: get(8) as u8,
            mvscg: get(8) as u8,
            cafcnt: get(20) as u32,
        }
    }
    /// The aspect bit says 4:3 (bit 4 of MVSCG, bits counted from 1).
    pub fn narrow(&self) -> bool {
        (self.mvscg >> 3) & 1 == 1
    }
    /// VSAM bit 6 = 0: scrambled.
    pub fn scrambled(&self) -> bool {
        (self.mvscg >> 5) & 1 == 0
    }
    /// VSAM bit 7: single-cut line rotation (else double-cut component rotation).
    pub fn single_cut(&self) -> bool {
        (self.mvscg >> 6) & 1 == 1
    }
    /// VSAM bit 8: controlled access.
    pub fn controlled(&self) -> bool {
        (self.mvscg >> 7) & 1 == 1
    }
    /// The text of the access flags.
    pub fn access_text(&self) -> String {
        let access = if self.controlled() {
            "controlled access"
        } else {
            "free access"
        };
        let scr = if !self.scrambled() {
            "unscrambled"
        } else if self.single_cut() {
            "single-cut line rotation"
        } else {
            "double-cut component rotation"
        };
        format!("{access}, {scr}")
    }
    /// MVSCG for the flags.
    pub fn mvscg_of(narrow: bool, scrambled: bool, single_cut: bool, controlled: bool) -> u8 {
        ((narrow as u8) << 3)
            | ((!scrambled as u8) << 5)
            | ((single_cut as u8) << 6)
            | ((controlled as u8) << 7)
    }
}

impl Rdf {
    /// 80 data bits.
    pub fn bits(&self) -> u128 {
        let mut v: u128 = 0;
        for i in 0..8 {
            if (self.tdmcid >> i) & 1 == 1 {
                v |= 1u128 << (79 - i);
            }
            if (self.fcnt >> i) & 1 == 1 {
                v |= 1u128 << (79 - 8 - i);
            }
        }
        v
    }
    /// From 80 data bits.
    pub fn from_bits(v: u128) -> Rdf {
        let mut r = Rdf::default();
        for i in 0..8 {
            if (v >> (79 - i)) & 1 == 1 {
                r.tdmcid |= 1 << i;
            }
            if (v >> (79 - 8 - i)) & 1 == 1 {
                r.fcnt |= 1 << i;
            }
        }
        r
    }
}

/// The 546 bits of line 625's special data.
pub fn special_data(sdf: &Sdf, rdf: &Rdf) -> Vec<u8> {
    let mut out = vec![0u8; UDT_BITS];
    let s = super::codes::sdf_code().encode(sdf.bits());
    for i in (0..SDF_BITS).rev() {
        out.push(((s >> i) & 1) as u8);
    }
    for _ in 0..5 {
        let r = super::codes::rdf_code().encode(rdf.bits());
        for i in (0..RDF_BITS).rev() {
            out.push(((r >> i) & 1) as u8);
        }
    }
    out
}

/// Decodes line 625's special data: the SDF (if it passes its check), the RDF and the blocks that failed.
pub fn decode_special(bits: &[u8]) -> (Option<Sdf>, Option<Rdf>, u32) {
    let word = |a: usize, n: usize| -> u128 {
        let mut v = 0u128;
        for i in 0..n {
            v = (v << 1) | bits.get(a + i).copied().unwrap_or(0) as u128;
        }
        v
    };
    let mut bad = 0;
    let sdf_code = super::codes::sdf_code();
    let rdf_code = super::codes::rdf_code();
    let sdf = match sdf_code.decode(word(UDT_BITS, SDF_BITS)) {
        Some((w, _)) => Some(Sdf::from_bits(sdf_code.info(w))),
        None => {
            bad += 1;
            None
        }
    };
    let mut rdf = None;
    for k in 0..5 {
        match rdf_code.decode(word(UDT_BITS + SDF_BITS + k * RDF_BITS, RDF_BITS)) {
            Some((w, _)) => {
                if rdf.is_none() {
                    rdf = Some(Rdf::from_bits(rdf_code.info(w)));
                }
            }
            None => bad += 1,
        }
    }
    (sdf, rdf, bad)
}

/// E'U = 0.733 (B - Y), E'V = 0.927 (R - Y).
pub const MAC_KU: f64 = 0.733;
/// See [`MAC_KU`].
pub const MAC_KV: f64 = 0.927;

/// The FM sensitivity, Hz per volt.
pub const HZ_PER_V: f64 = 13.5e6;

/// The pre-emphasis H(f) = A (1 + jf/f1) / (1 + jf/f2), A = 1/√2, f1 = 0.84 MHz, f2 = 1.5 MHz, by the bilinear transform at `rate`;
/// `inverse` selects the de-emphasis.
#[derive(Clone, Copy, Debug)]
pub struct MacEmphasis {
    b0: f64,
    b1: f64,
    a1: f64,
    x1: f64,
    y1: f64,
}

impl MacEmphasis {
    /// For `rate`.
    pub fn new(rate: f64, inverse: bool) -> Self {
        let k = 2.0 * rate;
        let (w1, w2) = (
            std::f64::consts::TAU * 0.84e6,
            std::f64::consts::TAU * 1.5e6,
        );
        let (w1, w2) = (k * (w1 / k).tan(), k * (w2 / k).tan());
        let a = std::f64::consts::FRAC_1_SQRT_2;
        // A (1 + s / w1) / (1 + s / w2)
        let (n0, n1) = (a * (1.0 + k / w1), a * (1.0 - k / w1));
        let (d0, d1) = (1.0 + k / w2, 1.0 - k / w2);
        let (n0, n1, d0, d1) = if inverse {
            (d0, d1, n0, n1)
        } else {
            (n0, n1, d0, d1)
        };
        MacEmphasis {
            b0: n0 / d0,
            b1: n1 / d0,
            a1: d1 / d0,
            x1: 0.0,
            y1: 0.0,
        }
    }
    /// One sample.
    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_625_round_trips() {
        let sdf = Sdf {
            chid: 0xBEEF,
            sdfscr: 0,
            mvscg: Sdf::mvscg_of(false, true, true, true),
            cafcnt: 12345,
        };
        let rdf = Rdf {
            tdmcid: 5,
            fcnt: 77,
        };
        let bits = special_data(&sdf, &rdf);
        assert_eq!(bits.len(), 546);
        let (s, r, bad) = decode_special(&bits);
        assert_eq!(bad, 0);
        let s = s.unwrap();
        assert_eq!(s, sdf);
        assert_eq!(r.unwrap(), rdf);
        assert!(!s.narrow() && s.scrambled() && s.controlled() && s.single_cut());
        assert_eq!(
            s.access_text(),
            "controlled access, single-cut line rotation"
        );
        // An error in each block is corrected.
        let mut b2 = bits.clone();
        b2[10] ^= 1;
        for k in 0..5 {
            b2[UDT_BITS + SDF_BITS + k * RDF_BITS + 3] ^= 1;
        }
        let (s2, r2, bad2) = decode_special(&b2);
        assert_eq!((s2, r2, bad2), (Some(sdf), Some(rdf), 0));
    }

    #[test]
    fn emphasis_inverts() {
        let mut p = MacEmphasis::new(40.5e6, false);
        let mut d = MacEmphasis::new(40.5e6, true);
        for i in 0..1000 {
            let x = (i as f64 * 0.05).sin();
            let y = d.process(p.process(x));
            if i > 100 {
                assert!((x - y).abs() < 1e-9);
            }
        }
    }
}
