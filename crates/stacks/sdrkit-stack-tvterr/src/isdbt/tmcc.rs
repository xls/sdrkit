//! ISDB-T TMCC: the 204 bits of a frame, the shortened (184,102) difference-set cyclic code that protects them, and its soft
//! decoder.
//!
//! Standard: ARIB STD-B31 clause 3.15.

use sdrkit_algo_fec::cyclic::{CyclicEncoder, MajorityLogic};
use std::sync::OnceLock;

/// The TMCC synchronising word of even frames (B1..B16). Odd frames send its inverse.
pub const SYNC_W0: u16 = 0b0011_0101_1110_1110;

/// The shared (273,191) difference-set cyclic code shortened to (184,102). ARIB STD-B31 clause 3.15.6.9.
fn code() -> &'static CyclicEncoder {
    static C: OnceLock<CyclicEncoder> = OnceLock::new();
    C.get_or_init(|| {
        MajorityLogic::isdb_t_tmcc()
            .map(|m| m.encoder().clone())
            .unwrap_or_else(|_| unreachable!("the TMCC code is a constant of sdrkit-algo-fec"))
    })
}

/// Parity of the 102 information bits: `info[0]` is B20. Returns the 82 parity bits B122..B203.
pub fn parity(info: &[bool; 102]) -> [bool; 82] {
    let mut word = [0u8; 184];
    for (w, &b) in word.iter_mut().zip(info) {
        *w = b as u8;
    }
    let _ = code().encode(&mut word);
    let mut out = [false; 82];
    for (o, &w) in out.iter_mut().zip(&word[102..]) {
        *o = w != 0;
    }
    out
}

/// Whether 184 bits (B20..B203) are a code word.
pub fn is_codeword(bits: &[bool]) -> bool {
    if bits.len() < 184 {
        return false;
    }
    let mut word = [0u8; 184];
    for (w, &b) in word.iter_mut().zip(bits) {
        *w = b as u8;
    }
    code().is_codeword(&word)
}

/// The check bits a code word needs for the information bits of `word` (184 bits, one per byte) XOR the check bits it has: zero
/// for a code word. The code is linear, so the syndrome of a sum of words is the sum of their syndromes.
fn syndrome(word: &[u8; 184]) -> u128 {
    let mut check = [0u8; 82];
    let _ = code().check_bits(&word[..102], &mut check);
    let mut s = 0u128;
    for (k, (&c, &w)) in check.iter().zip(&word[102..]).enumerate() {
        s |= ((c ^ w) as u128 & 1) << k;
    }
    s
}

/// Soft decoding of B20..B203: the sign of `soft[k]` is the bit (negative means 1), its magnitude the reliability. Tries the
/// hard decision, then every pattern of up to four flips among the 16 least reliable bits (the one with the least reliability
/// flipped wins). Returns the corrected bits, or `None` when no pattern produces a code word or a value is not finite.
pub fn decode_soft(soft: &[f32]) -> Option<[bool; 184]> {
    if soft.len() < 184 || soft[..184].iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut hard = [false; 184];
    let mut word = [0u8; 184];
    for k in 0..184 {
        hard[k] = soft[k] < 0.0;
        word[k] = hard[k] as u8;
    }
    let s0 = syndrome(&word);
    if s0 == 0 {
        return Some(hard);
    }
    let mut idx: [usize; 184] = std::array::from_fn(|i| i);
    idx.sort_by(|&a, &b| soft[a].abs().total_cmp(&soft[b].abs()));
    let weak = &idx[..16];
    // The syndrome of a single flipped bit.
    let single: [u128; 16] = std::array::from_fn(|i| {
        let mut e = [0u8; 184];
        e[weak[i]] = 1;
        syndrome(&e)
    });
    let n = weak.len();
    let mut best: Option<(f32, [usize; 4], usize)> = None;
    let mut consider = |set: &[usize]| {
        let r = set.iter().fold(s0, |a, &i| a ^ single[i]);
        if r == 0 {
            let cost: f32 = set.iter().map(|&i| soft[weak[i]].abs()).sum();
            if best.as_ref().is_none_or(|b| cost < b.0) {
                let mut v = [0usize; 4];
                v[..set.len()].copy_from_slice(set);
                best = Some((cost, v, set.len()));
            }
        }
    };
    for a in 0..n {
        consider(&[a]);
        for b in a + 1..n {
            consider(&[a, b]);
            for c in b + 1..n {
                consider(&[a, b, c]);
                for d in c + 1..n {
                    consider(&[a, b, c, d]);
                }
            }
        }
    }
    let (_, set, len) = best?;
    for &i in &set[..len] {
        hard[weak[i]] = !hard[weak[i]];
    }
    Some(hard)
}

/// A layer's transmission parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerParams {
    /// 0 DQPSK, 1 QPSK, 2 16QAM, 3 64QAM, 7 unused.
    pub modulation: u8,
    /// 0 1/2, 1 2/3, 2 3/4, 3 5/6, 4 7/8.
    pub code_rate: u8,
    /// Interleaving length code 0..3.
    pub interleave: u8,
    /// Segments (0 for an unused layer).
    pub segments: u8,
}

impl LayerParams {
    /// An unused layer.
    pub const UNUSED: LayerParams = LayerParams {
        modulation: 7,
        code_rate: 7,
        interleave: 7,
        segments: 0,
    };
    /// Whether the layer is used.
    pub fn used(&self) -> bool {
        self.segments > 0 && self.modulation != 7
    }
    /// Bits per cell.
    pub fn bits(&self) -> u32 {
        match self.modulation {
            0 | 1 => 2,
            2 => 4,
            3 => 6,
            _ => 0,
        }
    }
    /// Code rate as a fraction.
    pub fn rate(&self) -> (u32, u32) {
        match self.code_rate {
            0 => (1, 2),
            1 => (2, 3),
            2 => (3, 4),
            3 => (5, 6),
            4 => (7, 8),
            _ => (0, 1),
        }
    }
    /// "QPSK".
    pub fn modulation_name(&self) -> &'static str {
        ["DQPSK", "QPSK", "16QAM", "64QAM", "?", "?", "?", "-"][self.modulation.min(7) as usize]
    }
    /// "2/3".
    pub fn rate_name(&self) -> &'static str {
        ["1/2", "2/3", "3/4", "5/6", "7/8", "?", "?", "-"][self.code_rate.min(7) as usize]
    }
}

/// One configuration (current or next).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Partial reception (one-seg) in layer A.
    pub partial: bool,
    /// Layers A, B, C.
    pub layers: [LayerParams; 3],
}

impl Config {
    /// Segments of the used layers.
    pub fn total(&self) -> u32 {
        self.layers
            .iter()
            .filter(|l| l.used())
            .map(|l| l.segments as u32)
            .sum()
    }

    /// Checks the rules of a configuration: 13 segments in all (ISDB-T) or 1 or 3 (ISDB-TSB), differential layers before
    /// coherent ones, a partial-reception layer A of one segment, interleave codes 0 to 3.
    pub fn valid(&self) -> bool {
        if !matches!(self.total(), 1 | 3 | 13) {
            return false;
        }
        let mut coherent_seen = false;
        // The partial-reception segment is a group of its own and outside the order rule.
        let skip = usize::from(self.partial);
        for l in self.layers.iter().skip(skip).filter(|l| l.used()) {
            if l.modulation == 0 && coherent_seen {
                return false;
            }
            if l.modulation != 0 {
                coherent_seen = true;
            }
        }
        if self
            .layers
            .iter()
            .filter(|l| l.used())
            .any(|l| l.code_rate > 4 || l.interleave > 3 || l.modulation > 3)
        {
            return false;
        }
        !(self.partial && self.layers[0].segments != 1)
    }

    /// The configuration a receiver of a band of `ns` segments decodes: this one when its layers fill exactly `ns` segments; for a
    /// band of one segment inside a wider signal, layer A alone when it is the one-segment partial-reception layer. `None` when
    /// the configuration is not valid or does not fit the band.
    pub fn within(&self, ns: usize) -> Option<Config> {
        if !self.valid() {
            return None;
        }
        if self.total() == ns as u32 {
            return Some(*self);
        }
        let a = self.layers[0];
        (ns == 1 && self.partial && a.used() && a.segments == 1).then_some(Config {
            partial: true,
            layers: [a, LayerParams::UNUSED, LayerParams::UNUSED],
        })
    }
}

/// The TMCC information.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tmcc {
    /// 0 ISDB-T, 1 ISDB-TSB.
    pub system: u8,
    /// Parameter switching countdown (15 = normal).
    pub countdown: u8,
    /// Emergency alarm start flag.
    pub emergency: bool,
    /// The current configuration.
    pub current: Config,
    /// The next configuration.
    pub next: Config,
}

fn put(bits: &mut Vec<bool>, v: u32, n: usize) {
    for i in (0..n).rev() {
        bits.push((v >> i) & 1 != 0);
    }
}

fn get(bits: &[bool], at: &mut usize, n: usize) -> u32 {
    let mut v = 0;
    for _ in 0..n {
        v = (v << 1) | bits[*at] as u32;
        *at += 1;
    }
    v
}

impl Tmcc {
    /// The 102 information bits (B20..B121).
    pub fn info_bits(&self) -> [bool; 102] {
        let mut b = Vec::with_capacity(102);
        put(&mut b, self.system as u32, 2);
        put(&mut b, self.countdown as u32, 4);
        put(&mut b, self.emergency as u32, 1);
        for c in [&self.current, &self.next] {
            put(&mut b, c.partial as u32, 1);
            for l in &c.layers {
                put(&mut b, l.modulation as u32, 3);
                put(&mut b, l.code_rate as u32, 3);
                put(&mut b, l.interleave as u32, 3);
                put(&mut b, if l.used() { l.segments as u32 } else { 15 }, 4);
            }
        }
        put(&mut b, 0b111, 3); // phase-shift correction (none)
        put(&mut b, 0xFFF, 12); // reserved
        let mut out = [false; 102];
        out.copy_from_slice(&b);
        out
    }
    /// Reads the information bits back.
    pub fn from_info(b: &[bool]) -> Tmcc {
        let mut at = 0;
        let system = get(b, &mut at, 2) as u8;
        let countdown = get(b, &mut at, 4) as u8;
        let emergency = get(b, &mut at, 1) != 0;
        let mut cfg = [Config::default(); 2];
        for c in cfg.iter_mut() {
            c.partial = get(b, &mut at, 1) != 0;
            for l in c.layers.iter_mut() {
                l.modulation = get(b, &mut at, 3) as u8;
                l.code_rate = get(b, &mut at, 3) as u8;
                l.interleave = get(b, &mut at, 3) as u8;
                let s = get(b, &mut at, 4) as u8;
                l.segments = if s == 15 { 0 } else { s };
                if l.segments == 0 {
                    *l = LayerParams::UNUSED;
                }
            }
        }
        Tmcc {
            system,
            countdown,
            emergency,
            current: cfg[0],
            next: cfg[1],
        }
    }
    /// The 204 TMCC bits of a frame (B0..B203): the differential start bit (B0), the sync word (inverted on odd frames), the segment type, the
    /// information and the parity.
    pub fn frame_bits(&self, odd: bool, differential: bool) -> [bool; 204] {
        let mut out = [false; 204];
        let w = if odd { !SYNC_W0 } else { SYNC_W0 };
        for i in 0..16 {
            out[1 + i] = (w >> (15 - i)) & 1 != 0;
        }
        for i in 0..3 {
            out[17 + i] = differential;
        }
        let info = self.info_bits();
        out[20..122].copy_from_slice(&info);
        let p = parity(&info);
        out[122..204].copy_from_slice(&p);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tmcc() -> Tmcc {
        let a = LayerParams {
            modulation: 1,
            code_rate: 1,
            interleave: 2,
            segments: 1,
        };
        let b = LayerParams {
            modulation: 3,
            code_rate: 2,
            interleave: 1,
            segments: 12,
        };
        let cfg = Config {
            partial: true,
            layers: [a, b, LayerParams::UNUSED],
        };
        Tmcc {
            system: 0,
            countdown: 15,
            emergency: false,
            current: cfg,
            next: cfg,
        }
    }

    #[test]
    fn tmcc_packs_corrects_and_refuses() {
        let t = test_tmcc();
        assert!(t.current.valid());
        let even = t.frame_bits(false, false);
        let odd = t.frame_bits(true, false);
        assert!(is_codeword(&even[20..]));
        assert_eq!(&even[20..], &odd[20..], "the same 184 bits in both frames");
        for i in 0..16 {
            assert_ne!(even[1 + i], odd[1 + i], "odd frames invert the sync word");
        }
        assert_eq!(Tmcc::from_info(&even[20..122]), t);
        let dq = t.frame_bits(false, true);
        assert!(dq[17] && dq[18] && dq[19]);
        // 200 random trials of 1-4 errors on low-reliability bits.
        let mut rng = sdrkit_algo_sim::rng::Rng::new(0x1234_5678);
        let mut rnd = || rng.next_u64();
        let mut fixed = 0;
        for trial in 0..200 {
            let mut soft: Vec<f32> = even[20..]
                .iter()
                .map(|&b| if b { -1.0 } else { 1.0 })
                .collect();
            let nerr = 1 + trial % 4;
            let mut pos = Vec::new();
            while pos.len() < nerr {
                let p = (rnd() % 184) as usize;
                if !pos.contains(&p) {
                    pos.push(p);
                }
            }
            for &p in &pos {
                soft[p] = -soft[p] * 0.2;
            }
            // Other bits a little less sure in places, but above the flipped ones.
            for k in 0..184 {
                if !pos.contains(&k) {
                    soft[k] *= 0.5 + 0.5 * ((rnd() % 100) as f32 / 100.0);
                }
            }
            if let Some(w) = decode_soft(&soft) {
                if w[..] == even[20..] {
                    fixed += 1;
                }
            }
        }
        assert_eq!(fixed, 200);
        // 17 errors spread evenly are refused (never another valid word).
        let mut soft: Vec<f32> = even[20..]
            .iter()
            .map(|&b| if b { -1.0 } else { 1.0 })
            .collect();
        for k in 0..17 {
            let p = k * 10 + 3;
            soft[p] = -soft[p];
        }
        assert!(decode_soft(&soft).is_none());
    }

    #[test]
    fn a_dqpsk_layer_after_a_coherent_one_is_refused() {
        let a = LayerParams {
            modulation: 1,
            code_rate: 0,
            interleave: 0,
            segments: 1,
        };
        let b = LayerParams {
            modulation: 0,
            code_rate: 0,
            interleave: 0,
            segments: 3,
        };
        let c = LayerParams {
            modulation: 3,
            code_rate: 0,
            interleave: 0,
            segments: 9,
        };
        // The partial-reception segment is a group of its own: DQPSK can follow it.
        assert!(
            Config {
                partial: true,
                layers: [a, b, c]
            }
            .valid(),
            "DQPSK after the partial segment"
        );
        let a3 = LayerParams { segments: 1, ..a };
        assert!(
            !Config {
                partial: false,
                layers: [a3, b, c]
            }
            .valid(),
            "DQPSK after a coherent layer"
        );
        let a = LayerParams { modulation: 0, ..a };
        assert!(Config {
            partial: true,
            layers: [a, b, c]
        }
        .valid());
        let q16 = LayerParams {
            modulation: 2,
            code_rate: 0,
            interleave: 0,
            segments: 4,
        };
        let dq = LayerParams {
            modulation: 0,
            code_rate: 0,
            interleave: 0,
            segments: 9,
        };
        assert!(
            !Config {
                partial: false,
                layers: [q16, dq, LayerParams::UNUSED]
            }
            .valid(),
            "DQPSK after 16QAM"
        );
    }
}
