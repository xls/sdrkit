//! `hf_chat`: short messages, beacons and voice in 2.4 kHz of upper-sideband audio. All signals are complex baseband at 2400
//! samples a second, one sample a chip; the audio frequency is the baseband frequency plus 1500 Hz. Profiles 0 to 4 send
//! chirps of `2^SF` chips whose direction a 16-bit LFSR weaves; profiles 5 to 7 send OFDM of 48 carriers with 12 pilots.
//! Every burst is one or more lattice units of rising and falling sweeps and comb tones, the data, and a closing weave. A
//! block of `K` bits (a header, a slice of the message and a CRC-16) is coded by the profile's own quasi-cyclic LDPC code and
//! interleaved. [`frame`] holds the messages, [`phy`] the waveform, [`rx`] the receiver, [`arq`] the slot grid and the
//! acknowledged session.

pub mod arq;
pub mod frame;
pub mod phy;
pub mod rx;
pub mod stack;

pub use stack::HF_CHAT;

use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::{Arc, Mutex, OnceLock};

/// Baseband samples a second.
pub const RATE: f64 = 2400.0;
/// The audio frequency of baseband 0 Hz.
pub const AUDIO_CENTRE_HZ: f64 = 1500.0;
/// Samples of a lattice unit.
pub const UNIT: usize = 8400;
/// Samples of the closing weave.
pub const CLOSING: usize = 2400;
/// The most blocks of a message.
pub const MAX_BLOCKS: usize = 16;
/// The seed of the interleaver permutations, plus `N`.
pub const PERM_SEED: u64 = 0x4B41_5753_0000;
/// The seed of the OFDM pilots, plus `64 s + i`.
pub const PILOT_SEED: u64 = 0x4F46_444D_0000;

/// How a profile modulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modulation {
    /// Chirps of `2^sf` chips with a cyclic prefix of `guard` chips.
    Chirp {
        /// The spreading factor.
        sf: u8,
        /// Chips of the cyclic prefix.
        guard: usize,
    },
    /// OFDM of 48 carriers: `bits` a data carrier (1 or 2), `symbols` a block.
    Ofdm {
        /// Bits a data carrier.
        bits: usize,
        /// OFDM symbols a block.
        symbols: usize,
    },
}

/// One profile (the protocol's own table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The id, 0 to 7.
    pub id: u8,
    /// The name.
    pub name: &'static str,
    /// Information bits a block.
    pub k: usize,
    /// Coded bits a block.
    pub n: usize,
    /// The modulation.
    pub modulation: Modulation,
    /// Times the symbols of a block are sent.
    pub repeats: usize,
    /// The slot length, s; `None` for push to talk.
    pub slot_s: Option<f64>,
    /// Lattice units before the data.
    pub front: usize,
    /// Lattice units inside the first block.
    pub midambles: usize,
    /// An estimate of the threshold on white noise, dB in 2.4 kHz; it shapes the soft demapper of the chirp profiles.
    pub design_snr_db: f64,
}

const fn chirp(sf: u8, guard: usize) -> Modulation {
    Modulation::Chirp { sf, guard }
}

/// The eight profiles.
pub const PROFILES: [Profile; 8] = [
    Profile { id: 0, name: "Abyss", k: 128, n: 512, modulation: chirp(10, 0), repeats: 3, slot_s: Some(120.0), front: 2, midambles: 8, design_snr_db: -29.6 },
    Profile { id: 1, name: "Deep", k: 256, n: 1024, modulation: chirp(10, 0), repeats: 1, slot_s: Some(60.0), front: 1, midambles: 1, design_snr_db: -25.2 },
    Profile { id: 2, name: "Weak", k: 336, n: 1008, modulation: chirp(9, 0), repeats: 1, slot_s: Some(60.0), front: 1, midambles: 0, design_snr_db: -21.4 },
    Profile { id: 3, name: "Chat", k: 512, n: 1024, modulation: chirp(8, 0), repeats: 1, slot_s: Some(30.0), front: 1, midambles: 0, design_snr_db: -17.1 },
    Profile { id: 4, name: "Mid", k: 510, n: 1020, modulation: chirp(6, 16), repeats: 1, slot_s: Some(15.0), front: 1, midambles: 0, design_snr_db: -13.3 },
    Profile { id: 5, name: "Fast data", k: 360, n: 1440, modulation: Modulation::Ofdm { bits: 1, symbols: 40 }, repeats: 1, slot_s: Some(15.0), front: 1, midambles: 0, design_snr_db: -7.4 },
    Profile { id: 6, name: "Voice-700", k: 600, n: 1728, modulation: Modulation::Ofdm { bits: 2, symbols: 24 }, repeats: 1, slot_s: None, front: 1, midambles: 0, design_snr_db: -2.3 },
    Profile { id: 7, name: "Voice-1200", k: 900, n: 1728, modulation: Modulation::Ofdm { bits: 2, symbols: 24 }, repeats: 1, slot_s: None, front: 1, midambles: 0, design_snr_db: 0.0 },
];

impl Profile {
    /// The profile of `id`, if there is one.
    pub fn of(id: u8) -> Option<&'static Profile> {
        PROFILES.get(usize::from(id))
    }

    /// True for the chirp profiles.
    pub fn is_chirp(&self) -> bool {
        matches!(self.modulation, Modulation::Chirp { .. })
    }

    /// Data symbols of one copy of a block: `ceil(N / SF)` for chirps, 40 or 24 for OFDM.
    pub fn symbols(&self) -> usize {
        match self.modulation {
            Modulation::Chirp { sf, .. } => self.n.div_ceil(usize::from(sf)),
            Modulation::Ofdm { symbols, .. } => symbols,
        }
    }

    /// Samples of one symbol: `2^SF` plus the guard, or 60.
    pub fn symbol_len(&self) -> usize {
        match self.modulation {
            Modulation::Chirp { sf, guard } => (1 << sf) + guard,
            Modulation::Ofdm { .. } => phy::OFDM_SYMBOL,
        }
    }

    /// Symbols of a block, the repeats counted.
    pub fn block_symbols(&self) -> usize {
        self.symbols() * self.repeats
    }

    /// Samples of the data of a block.
    pub fn block_samples(&self) -> usize {
        self.block_symbols() * self.symbol_len()
    }

    /// Seconds of the data of a block.
    pub fn block_seconds(&self) -> f64 {
        self.block_samples() as f64 / RATE
    }

    /// Lattice units of a burst.
    pub fn units(&self) -> usize {
        self.front + self.midambles
    }

    /// Message bits a block carries: `K - 24`.
    pub fn usable(&self) -> usize {
        self.k - 24
    }

    /// Zero bits that fill the last chirp symbol of a copy.
    pub fn fill_bits(&self) -> usize {
        match self.modulation {
            Modulation::Chirp { sf, .. } => self.symbols() * usize::from(sf) - self.n,
            Modulation::Ofdm { .. } => 0,
        }
    }

    /// Samples of a burst of `blocks` blocks.
    pub fn burst_samples(&self, blocks: usize) -> usize {
        self.units() * UNIT + blocks * self.block_samples() + CLOSING
    }

    /// Blocks a slot carries: 1 for profiles 0 and 1; for the others `floor((slot - 9) / data seconds)`, at least 1. For
    /// push to talk profiles, one block per 10 s.
    pub fn blocks_per_slot(&self) -> usize {
        if self.id <= 1 {
            return 1;
        }
        let slot = self.slot_s.unwrap_or(10.0);
        (((slot - 1.5 - 3.0 - 3.5 - 1.0) / self.block_seconds() + 1e-9).floor() as usize).max(1)
    }
}

/// The generator of `hf_chat`: xorshift64 from `seed` (a seed of 0 starts from the golden ratio) after 8 steps whose outputs
/// are thrown away.
pub fn generator(seed: u64) -> Xorshift64 {
    let mut g = Xorshift64::with_zero_seed(seed, Xorshift64::GOLDEN);
    for _ in 0..8 {
        g.step();
    }
    g
}

/// The interleaver permutation of length `n`: interleaved bit `i` is code word bit `p[i]`. A Fisher-Yates shuffle from the
/// generator of [`PERM_SEED`] + `n`, `j = below(i + 1)` for `i` from `n - 1` down to 1.
pub fn permutation(n: usize) -> Arc<Vec<u16>> {
    static CACHE: OnceLock<Mutex<Vec<Arc<Vec<u16>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut held = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = held.iter().find(|p| p.len() == n) {
        return p.clone();
    }
    let mut p: Vec<u16> = (0..n as u16).collect();
    let mut g = generator(PERM_SEED + n as u64);
    for i in (1..n).rev() {
        let j = g.below(i as u64 + 1) as usize;
        p.swap(i, j);
    }
    let p = Arc::new(p);
    held.push(p.clone());
    p
}

const TABLES: &str = include_str!("ldpc_tables.txt");

/// The LDPC codes of the eight profiles, from the base matrices of `ldpc_tables.txt`.
pub fn codes() -> &'static [LdpcCode] {
    static C: OnceLock<Vec<LdpcCode>> = OnceLock::new();
    C.get_or_init(|| {
        let mut out = Vec::new();
        let mut lines = TABLES.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).peekable();
        while let Some(head) = lines.next() {
            let f: Vec<usize> = head.split_whitespace().skip(1).map(|v| v.parse().expect("a number")).collect();
            let (id, n, k, z) = (f[0], f[1], f[2], f[3]);
            let mut rows: Vec<Vec<(u16, u16)>> = Vec::new();
            while lines.peek().is_some_and(|l| !l.starts_with("code")) {
                let row = lines.next().unwrap_or_default();
                rows.push(
                    row.split_whitespace()
                        .map(|v| v.parse::<i32>().expect("a shift"))
                        .enumerate()
                        .filter(|(_, s)| *s >= 0)
                        .map(|(c, s)| (c as u16, s as u16))
                        .collect(),
                );
            }
            let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
            let code = LdpcCode::from_qc(&format!("hf_chat profile {id}"), z, n / z, k, &refs).expect("an hf_chat code");
            assert!(code.can_encode(), "the code of profile {id} has an encoder");
            out.push(code);
        }
        out
    })
}

/// The code of profile `p`.
pub fn code(p: &Profile) -> &'static LdpcCode {
    &codes()[usize::from(p.id)]
}

/// The code word of the `K` information bits `info` of profile `p`: the information bits, then the parity.
pub fn encode(p: &Profile, info: &[u8]) -> Vec<u8> {
    let mut cw = vec![0u8; p.n];
    code(p).encode(&info[..p.k], &mut cw).expect("K information bits");
    cw
}

/// Interleaves the code word `cw`: bit `i` of the result is `cw[p[i]]`.
pub fn interleave(cw: &[u8]) -> Vec<u8> {
    permutation(cw.len()).iter().map(|&i| cw[usize::from(i)]).collect()
}

/// The min-sum check rule of the code of profile `p`: scale 15/16 below rate 0.29, 14/16 below 0.47, else 13/16
/// (measured in `tests/ldpc_scale.rs`).
pub fn rule(p: &Profile) -> MinSum {
    let scale_q4 = if 100 * p.k < 29 * p.n {
        15
    } else if 100 * p.k < 47 * p.n {
        14
    } else {
        13
    };
    MinSum::with_scale(scale_q4)
}

/// A layered min-sum decoder of the code of profile `p`, with the check rule of [`rule`].
pub fn decoder(p: &Profile) -> LdpcDecoder {
    LdpcDecoder::new(code(p), rule(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fnv(p: &[u16]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &v in p {
            for b in u32::from(v).to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    }

    #[test]
    fn generator_and_interleaver_check_values() {
        assert_eq!(generator(1).step(), 0x7d3e_032e_9a79_08ff);
        assert_eq!(generator(0).step(), 0xddaa_4e85_b0d6_e28b);
        for (n, first, hash) in [
            (512, [161, 92, 177, 130, 445, 473, 323, 256, 310, 276, 57, 407, 104, 496, 110, 147], 0x0fa0_5751_4a56_4f55u64),
            (1008, [500, 589, 680, 40, 289, 67, 992, 907, 617, 45, 483, 653, 507, 773, 857, 287], 0x5e90_a832_7626_d189),
            (1020, [494, 123, 558, 480, 817, 213, 513, 954, 341, 925, 991, 343, 192, 360, 63, 371], 0x489e_0786_e8ef_5a75),
            (1024, [54, 629, 976, 36, 1016, 378, 334, 459, 919, 568, 27, 804, 358, 483, 716, 739], 0x0f41_dab2_8c01_4921),
            (1440, [98, 907, 205, 286, 934, 73, 1015, 257, 1210, 299, 725, 473, 1326, 82, 208, 849], 0xa814_92c0_9641_a279),
            (1728, [247, 1453, 700, 910, 1597, 390, 484, 611, 819, 439, 91, 948, 83, 772, 805, 1273], 0xdaf0_57c2_2471_7d6d),
        ] {
            let p = permutation(n);
            assert_eq!(p[..16], first, "{n}");
            assert_eq!(fnv(&p), hash, "{n}");
        }
    }

    #[test]
    fn profile_timing() {
        let secs = [66.56, 43.95, 23.89, 13.65, 5.67, 1.00, 0.60, 0.60];
        let bursts = [246_144, 124_672, 68_144, 43_568, 24_400, 13_200, 12_240, 12_240];
        for (p, (&s, &b)) in PROFILES.iter().zip(secs.iter().zip(&bursts)) {
            assert!((p.block_seconds() - s).abs() < 0.006, "{}", p.id);
            assert_eq!(p.burst_samples(1), b, "{}", p.id);
            assert_eq!(code(p).n(), p.n);
            assert_eq!(code(p).k(), p.k);
        }
        let per: Vec<usize> = PROFILES[..6].iter().map(|p| p.blocks_per_slot()).collect();
        assert_eq!(per, [1, 1, 2, 1, 1, 6]);
        assert_eq!((PROFILES[6].usable(), PROFILES[7].usable()), (576, 876));
        assert_eq!(PROFILES[0].fill_bits(), 8);
        assert_eq!(PROFILES[1].fill_bits(), 6);
        // Every slotted profile with its blocks fits its slot: on-air time + 3 s <= slot + 0.5 s.
        for p in &PROFILES[..6] {
            let air = 1.5 + p.burst_samples(p.blocks_per_slot()) as f64 / RATE;
            assert!(air + 3.0 <= p.slot_s.unwrap() + 0.5, "{}", p.id);
        }
    }
}
