//! The Datawell HXV real-time format: 64-bit vectors protected by the BCH(63,51) code of `sdrkit_algo_fec`, vector synchronisation
//! with slips and repairs, and the block structure with the system file and the directional spectrum.
//!
//! Standard: Datawell DWR-MkIII manual, section 5.7.

use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::pack::{sign_magnitude, to_sign_magnitude};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The sync word of a block's first vector.
pub const SYNC: u16 = 0x7FFF;

/// The vector code: BCH(63,51), generator octal 12471, corrects 2 bits.
fn code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(SmallCode::bch_63_51)
}

/// The 63 covered bits of a vector (bit 27 from the first sent removed), most significant (first sent) first.
fn covered(v: u64) -> u64 {
    let hi = v >> 37; // bits 0..26 (27 bits)
    let lo = v & ((1u64 << 36) - 1); // bits 28..63 (36 bits)
    (hi << 36) | lo
}

/// A vector from 63 covered bits and the uncovered bit 27.
fn uncovered(c: u64, b27: u64) -> u64 {
    let hi = c >> 36;
    let lo = c & ((1u64 << 36) - 1);
    (hi << 37) | ((b27 & 1) << 36) | lo
}

/// Encodes the 52 information bits of a vector (cyclic, heave, north, west; the parity field ignored): fills in the parity.
pub fn encode(v: u64) -> u64 {
    let c = code().encode((covered(v) >> 12) as u128) as u64;
    uncovered(c, (v >> 36) & 1)
}

/// Checks a vector: `Some((corrected vector, bits repaired))` for 0 to 2 errors in the covered bits, `None` otherwise.
pub fn check(v: u64) -> Option<(u64, u32)> {
    let (c, w) = code().decode(covered(v) as u128)?;
    Some((uncovered(c as u64, (v >> 36) & 1), w as u32))
}

/// A decoded vector (`wavebuoy.vector`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    /// Its index in the stream (one per 64 bits since the lock).
    pub index: u64,
    pub cyclic: u16,
    /// Displacements, cm.
    pub heave: i32,
    pub north: i32,
    pub west: i32,
    pub parity: u16,
    pub repaired: u32,
    /// WR-SG: west is AAA, heave and north are two vertical samples.
    pub wrsg: bool,
    /// Signal time of its last bit, seconds.
    pub time: f64,
}

/// A 12-bit sign-magnitude field (bit 11 set: negative) at the low end of `v`.
fn sm12(v: u64) -> i32 {
    sign_magnitude(v & 0xFFF, 12) as i32
}

/// Reads the fields of a vector.
pub fn fields(v: u64, repaired: u32) -> Vector {
    Vector {
        index: 0,
        cyclic: (v >> 48) as u16,
        heave: sm12(v >> 36),
        north: sm12(v >> 24),
        west: sm12(v >> 12),
        parity: (v & 0xFFF) as u16,
        repaired,
        wrsg: (v >> 12) & 0xFFF == 0xAAA,
        time: 0.0,
    }
}

/// Builds a vector from its fields (12-bit sign-magnitude displacements), with its parity.
pub fn build(cyclic: u16, heave: i32, north: i32, west_raw: u16) -> u64 {
    let v = ((cyclic as u64) << 48)
        | ((sm(heave) as u64) << 36)
        | ((sm(north) as u64) << 24)
        | ((west_raw as u64 & 0xFFF) << 12);
    encode(v)
}

/// The 12-bit sign-magnitude form of a displacement (saturates at ±2047).
pub fn sm(x: i32) -> u16 {
    to_sign_magnitude(x as i64, 12) as u16
}

/// The vector synchroniser: bits in, vectors out.
#[derive(Clone, Debug, Default)]
pub struct VectorSync {
    /// The newest 128 bits, the newest in the lowest bit.
    hist: u128,
    nbits: u64,
    /// Locked at this position (bit count mod 64) and polarity (true: inverted).
    pub locked: Option<(u64, bool)>,
    /// Per position (bit count mod 64) and polarity: the bit count of the last clean vector there and how many in a row.
    cand: Vec<(u64, u32)>,
    next_end: u64,
    lost_run: u32,
    slip_until: u64,
    last_heave: Option<i32>,
    index: u64,
    pub vectors: u64,
    pub clean: u64,
    pub repaired: u64,
    pub lost: u64,
}

impl VectorSync {
    /// Forgets the lock (a restart of the demodulator); the counters stay.
    pub fn reset(&mut self) {
        *self = VectorSync {
            vectors: self.vectors,
            clean: self.clean,
            repaired: self.repaired,
            lost: self.lost,
            ..Default::default()
        };
    }

    fn cand_slot(&mut self, pos: u64, pol: bool) -> &mut (u64, u32) {
        if self.cand.len() != 128 {
            self.cand = vec![(0, 0); 128];
        }
        &mut self.cand[(pos as usize) * 2 + pol as usize]
    }

    /// The 64 bits ending `back` bits before the newest, in polarity `inv`.
    fn word(&self, back: u32, inv: bool) -> u64 {
        let w = (self.hist >> back) as u64;
        if inv {
            !w
        } else {
            w
        }
    }

    /// The polarity of a clean code word: the one whose heave and north are smaller.
    fn polarity(v: u64) -> bool {
        let a = fields(v, 0);
        let b = fields(!v, 0);
        a.heave.abs() + a.north.abs() > b.heave.abs() + b.north.abs()
    }

    /// Flips the polarity (a cyclic 8000 seen outside a block means the tones are the other way round).
    pub fn flip(&mut self) {
        if let Some((p, inv)) = self.locked {
            self.locked = Some((p, !inv));
        }
    }

    /// Locked, and inverted.
    pub fn state(&self) -> (bool, bool) {
        match self.locked {
            Some((_, inv)) => (true, inv),
            None => (false, false),
        }
    }

    /// Pushes one bit (1 = the higher tone); returns a vector when one is accepted, or `Err(index)` when one is lost.
    pub fn push(&mut self, bit: u8, time: f64) -> Option<Result<Vector, u64>> {
        self.hist = (self.hist << 1) | (bit & 1) as u128;
        self.nbits += 1;
        if self.nbits < 64 {
            return None;
        }
        let pos = self.nbits % 64;
        match self.locked {
            None => {
                for inv in [false, true] {
                    let w = self.word(0, inv);
                    if let Some((v, 0)) = check(w) {
                        let pol = inv ^ Self::polarity(v);
                        let nb = self.nbits;
                        let slot = self.cand_slot(pos, pol);
                        let c = if slot.1 > 0 && slot.0 + 64 == nb {
                            slot.1 + 1
                        } else {
                            1
                        };
                        *slot = (nb, c);
                        if c >= 3 {
                            self.locked = Some((pos, pol));
                            self.next_end = self.nbits + 64;
                            self.lost_run = 0;
                            self.index = 0;
                            self.last_heave = None;
                            let v = self.word(0, pol);
                            return Some(Ok(self.accept(v, 0, time)));
                        }
                        break;
                    }
                }
                None
            }
            Some((p, inv)) => {
                if self.nbits <= self.slip_until && pos != p {
                    // For 12 bits after a lost vector, a clean vector at another phase moves the phase.
                    if let Some((v, 0)) = check(self.word(0, inv)) {
                        self.locked = Some((pos, inv));
                        self.next_end = self.nbits + 64;
                        self.slip_until = 0;
                        return Some(Ok(self.accept(v, 0, time)));
                    }
                }
                if self.nbits != self.next_end {
                    return None;
                }
                self.next_end += 64;
                let w = self.word(0, inv);
                match check(w) {
                    Some((v, 0)) => Some(Ok(self.accept(v, 0, time))),
                    other => {
                        // A clean vector ending 1 to 12 bits earlier: the phase slipped.
                        if self.nbits >= 64 + 12 {
                            for k in 1..=12u32 {
                                if let Some((v, 0)) = check(self.word(k, inv)) {
                                    self.locked = Some(((self.nbits - k as u64) % 64, inv));
                                    self.next_end = self.nbits - k as u64 + 64;
                                    return Some(Ok(self.accept(v, 0, time)));
                                }
                            }
                        }
                        if let Some((v, r)) = other {
                            let f = fields(v, r);
                            if self.last_heave.is_none_or(|h| (f.heave - h).abs() <= 200) {
                                return Some(Ok(self.accept(v, r, time)));
                            }
                        }
                        self.slip_until = self.nbits + 12;
                        self.lost += 1;
                        self.lost_run += 1;
                        let idx = self.index;
                        self.index += 1;
                        if self.lost_run > 8 {
                            self.locked = None;
                            self.cand.clear();
                        }
                        Some(Err(idx))
                    }
                }
            }
        }
    }

    fn accept(&mut self, v: u64, repaired: u32, time: f64) -> Vector {
        let mut f = fields(v, repaired);
        f.index = self.index;
        f.time = time;
        self.index += 1;
        self.vectors += 1;
        if repaired == 0 {
            self.clean += 1;
        } else {
            self.repaired += 1;
        }
        self.lost_run = 0;
        self.last_heave = Some(f.heave);
        f
    }
}

/// One point of the directional spectrum (manual table 5.7.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpecPoint {
    pub n: u8,
    pub hz: f64,
    pub dir_deg: f64,
    pub spread_deg: f64,
    pub m2: f64,
    pub n2: f64,
    pub rpsd: f64,
    pub k: f64,
    /// RPSD x PSDmax once system word 3 is known (m^2/Hz).
    pub psd: Option<f64>,
}

/// Reads spectrum entry `n` from its four words.
pub fn spec_point(w: [u16; 4]) -> SpecPoint {
    let (w0, w1, w2, w3) = (w[0] as u32, w[1] as u32, w[2] as u32, w[3] as u32);
    let n = ((w0 >> 8) & 0x3F) as u8;
    let hz = if n < 16 {
        0.025 + 0.005 * n as f64
    } else {
        0.11 + 0.01 * (n as f64 - 16.0)
    };
    SpecPoint {
        n,
        hz,
        dir_deg: (w0 & 0xFF) as f64 * 360.0 / 256.0,
        spread_deg: 0.4476 * ((w2 >> 8) as f64 + ((w0 >> 14) & 3) as f64 / 4.0),
        m2: ((w2 & 0xFF) as f64 + ((w1 >> 14) & 3) as f64 / 4.0 - 128.0) / 128.0,
        n2: ((w3 >> 8) as f64 + ((w1 >> 12) & 3) as f64 / 4.0 - 128.0) / 128.0,
        rpsd: (-((w1 & 0xFFF) as f64) / 200.0).exp(),
        k: (w3 & 0xFF) as f64 / 100.0,
        psd: None,
    }
}

/// The system file values (manual table 5.7.4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemValues {
    pub tn: Option<u8>,
    pub gps_ok: Option<bool>,
    pub gps_fix: Option<bool>,
    pub hrms_m: Option<f64>,
    pub hs_m: Option<f64>,
    pub fz_hz: Option<f64>,
    pub tz_s: Option<f64>,
    pub psd_max: Option<f64>,
    pub tr_c: Option<f64>,
    pub tw_c: Option<f64>,
    pub weeks_left: Option<u32>,
    pub battery: Option<u8>,
    pub av0: Option<f64>,
    pub ax0: Option<f64>,
    pub ay0: Option<f64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub orientation_deg: Option<f64>,
    pub inclination_deg: Option<f64>,
}

/// An accelerometer offset: 12-bit sign-magnitude, 1/800 m/s^2 steps.
fn acc(v: u16) -> f64 {
    sm12(v as u64) as f64 / 800.0
}

/// Interprets the system words received.
pub fn system_values(words: &[Option<u16>; 16]) -> SystemValues {
    let mut s = SystemValues::default();
    if let Some(v) = words[0] {
        s.tn = Some((v & 0xF) as u8);
        s.gps_ok = Some(v & 0x10 != 0);
        s.gps_fix = Some(v & 0x20 != 0);
    }
    if let Some(v) = words[1] {
        s.hrms_m = Some(v as f64 / 400.0);
        s.hs_m = Some(4.0 * v as f64 / 400.0);
    }
    if let Some(v) = words[2] {
        let fz = (v & 0xFF) as f64 / 400.0;
        s.fz_hz = Some(fz);
        s.tz_s = (fz > 0.0).then(|| 1.0 / fz);
    }
    if let Some(v) = words[3] {
        s.psd_max = Some(5000.0 * (-(v as f64) / 200.0).exp());
    }
    if let Some(v) = words[4] {
        s.tr_c = Some((v & 0x3FF) as f64 / 20.0 - 5.0);
    }
    if let Some(v) = words[5] {
        s.tw_c = Some((v & 0x3FF) as f64 / 20.0 - 5.0);
    }
    if let Some(v) = words[6] {
        s.weeks_left = Some(((v >> 4) & 0xFF) as u32);
        s.battery = Some((v & 7) as u8);
    }
    s.av0 = words[7].map(acc);
    s.ax0 = words[8].map(acc);
    s.ay0 = words[9].map(acc);
    let pos = |a: Option<u16>, b: Option<u16>, scale: f64| -> Option<f64> {
        let (a, b) = (a?, b?);
        let raw = ((a as u64 & 0xFFF) << 12) | (b as u64 & 0xFFF);
        Some(sign_magnitude(raw, 24) as f64 / (1u32 << 23) as f64 * scale)
    };
    if words[10..14].iter().all(|w| w.is_some()) {
        s.lat = pos(words[10], words[11], 90.0);
        s.lon = pos(words[12], words[13], 180.0);
    }
    s.orientation_deg = words[14].map(|v| 360.0 * (v & 0xFF) as f64 / 256.0);
    s.inclination_deg = words[15]
        .map(|v| (90.0 / 128.0) * ((v & 0xFF) as f64 - 128.0 + ((v >> 8) & 0xF) as f64 / 16.0));
    s
}

/// Encodes a latitude or longitude as two system words (sign bit 11 of the first, then 11 + 12 bits of value / 2^23 x scale).
pub fn position_words(deg: f64, scale: f64) -> (u16, u16) {
    let v = ((deg.abs() / scale) * (1u32 << 23) as f64).round() as u32;
    let a = ((v >> 12) & 0x7FF) as u16 | if deg < 0.0 { 0x800 } else { 0 };
    (a, (v & 0xFFF) as u16)
}

/// The HXV structure reader: blocks of 18 vectors from the sync word, the system file and the spectrum.
#[derive(Clone, Debug)]
pub struct BlockReader {
    /// The index of the current block's sync vector.
    sync_index: Option<u64>,
    sys_number: Option<u8>,
    group: [Option<u16>; 16],
    pub blocks: u64,
    pub last_block: Option<u8>,
    pub system: [Option<u16>; 16],
    pub spectrum: Vec<Option<SpecPoint>>,
    /// The newest 128 displacement samples (heave, north, west).
    pub recent: std::collections::VecDeque<(i32, i32, i32)>,
    pub last: Option<Vector>,
}

impl Default for BlockReader {
    fn default() -> Self {
        BlockReader {
            sync_index: None,
            sys_number: None,
            group: [None; 16],
            blocks: 0,
            last_block: None,
            system: [None; 16],
            spectrum: vec![None; 64],
            recent: std::collections::VecDeque::new(),
            last: None,
        }
    }
}

impl BlockReader {
    /// Forgets the block in progress (a polarity flip or a new lock).
    pub fn restart(&mut self) {
        self.sync_index = None;
        self.sys_number = None;
        self.group = [None; 16];
    }
    /// Empties everything (Clear).
    pub fn clear(&mut self) {
        *self = BlockReader::default();
    }
    /// Takes a vector; returns true when the cyclic word says the polarity is wrong (8000 outside a block).
    pub fn take(&mut self, v: &Vector) -> bool {
        if v.cyclic == SYNC {
            self.sync_index = Some(v.index);
            self.sys_number = None;
            self.group = [None; 16];
        } else if v.cyclic == 0x8000 && self.sync_index.is_none() {
            return true;
        }
        if self.recent.len() >= 128 {
            self.recent.pop_front();
        }
        self.recent.push_back((v.heave, v.north, v.west));
        self.last = Some(*v);
        let Some(si) = self.sync_index else {
            return false;
        };
        let pos = v.index - si;
        if pos >= 18 {
            // A wrong 19th vector: wait for the next sync.
            self.sync_index = None;
            return false;
        }
        match pos {
            0 => {}
            1 => {
                let num = (v.cyclic >> 12) as u8;
                self.system[num as usize] = Some(v.cyclic & 0xFFF);
                self.sys_number = Some(num);
                self.blocks += 1;
                self.last_block = Some(num);
            }
            p => {
                let k = (p - 2) as usize;
                self.group[k] = Some(v.cyclic);
                if k % 4 == 3 {
                    let g = k / 4;
                    if let (Some(num), Some(w0), Some(w1), Some(w2), Some(w3)) = (
                        self.sys_number,
                        self.group[4 * g],
                        self.group[4 * g + 1],
                        self.group[4 * g + 2],
                        self.group[4 * g + 3],
                    ) {
                        let pt = spec_point([w0, w1, w2, w3]);
                        if pt.n as usize == 4 * num as usize + g {
                            self.spectrum[pt.n as usize] = Some(pt);
                        }
                    }
                }
            }
        }
        false
    }
    /// A lost vector: its place in the block passes by counting.
    pub fn lost(&mut self, index: u64) {
        if let Some(si) = self.sync_index {
            if index - si >= 17 {
                self.sync_index = None;
            }
        }
    }
    /// The spectrum points received, with PSD once PSDmax is known.
    pub fn points(&self) -> Vec<SpecPoint> {
        let pmax = system_values(&self.system).psd_max;
        self.spectrum
            .iter()
            .flatten()
            .map(|p| SpecPoint {
                psd: pmax.map(|m| m * p.rpsd),
                ..*p
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    const EXAMPLES: [u64; 3] = [
        0x49F4_0060_0381_0AB7,
        0x766C_8090_0980_A134,
        0x7FFF_80E0_0300_1689,
    ];

    #[test]
    fn w1_examples_and_repairs() {
        for &v in &EXAMPLES {
            assert_eq!(check(v), Some((v, 0)));
            assert_eq!(encode(v), v);
        }
        let f = fields(EXAMPLES[2], 0);
        assert_eq!((f.cyclic, f.heave, f.north, f.west), (0x7FFF, -14, 3, 1));
        let f = fields(EXAMPLES[0], 0);
        assert_eq!((f.heave, f.north, f.west), (6, 3, -16));
        let pos: Vec<u32> = (0..64).filter(|&i| i != 27).collect();
        let mut rng = Rng::new(3);
        let mut refused = 0;
        let trials = 400;
        for t in 0..trials {
            let v = EXAMPLES[t % 3];
            let a = pos[rng.below(63) as usize];
            let mut b = pos[rng.below(63) as usize];
            while b == a {
                b = pos[rng.below(63) as usize];
            }
            let mut c = pos[rng.below(63) as usize];
            while c == a || c == b {
                c = pos[rng.below(63) as usize];
            }
            let e1 = v ^ (1u64 << (63 - a));
            assert_eq!(check(e1), Some((v, 1)));
            let e2 = e1 ^ (1u64 << (63 - b));
            assert_eq!(check(e2), Some((v, 2)));
            let e3 = e2 ^ (1u64 << (63 - c));
            match check(e3) {
                None => refused += 1,
                Some((w, _)) => assert_ne!(w, v),
            }
        }
        assert!(refused * 4 > trials, "refused {refused}");
        let sg = fields(build(0x1234, 10, -20, 0xAAA), 0);
        assert!(sg.wrsg && sg.heave == 10 && sg.north == -20);
        assert!(!fields(EXAMPLES[0], 0).wrsg);
    }
}
