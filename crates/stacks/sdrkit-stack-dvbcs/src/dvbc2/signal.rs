//! The DVB-C2 signal: the OFDM numbers, the guard intervals, the pilot and scrambling sequences, the cell order of an L1 block,
//! the carriers of data symbols (pilots, reserved tones, notches, data slices) and the frequency interleaver.
//!
//! Carrier indices are absolute: carrier `k` lies at `k / T_U` from 0 Hz.
//!
//! Standard: ETSI EN 302 769 V1.3.1 clauses 9.2 to 9.7 and 10.

use super::tables::{CONTINUAL, FREQ_PERM, FREQ_TAPS, RESERVED_S0};
use sdrkit_algo_fec::addrgen::AddressRule;
use sdrkit_util_bits::seq::{DVBC2_PILOT_PRIME, DVBT_PILOTS};
use std::sync::OnceLock;

/// FFT size.
pub const FFT: usize = 4096;
/// Carriers of an L1 block, K_L1.
pub const BLOCK: usize = 3408;
/// Data cells of an L1 block, N_L1.
pub const L1_CELLS: usize = 2840;
/// Data symbols of a C2 frame, L_data.
pub const DATA_SYMBOLS: usize = 448;
/// Symbols of the scattered pilot period, D_Y.
pub const D_Y: usize = 4;
/// The amplitude of scattered, continual and edge pilots.
pub const PILOT_AMP: f32 = 7.0 / 3.0;
/// The period of the reserved tone pattern, 8 L1 blocks.
pub const RESERVED_PERIOD: usize = 8 * BLOCK;

sdrkit_core::choice_enum! {
    /// The channel raster (tables 37 and 38).
    pub enum Raster {
        /// 8 MHz: elementary period 7/64 us.
        R8 = ("8", "8 MHz"),
        /// 6 MHz: elementary period 7/48 us.
        R6 = ("6", "6 MHz"),
    }
}

impl Raster {
    /// The sample rate 1 / T: 64/7 MHz or 48/7 MHz.
    pub fn rate(self) -> f64 {
        match self {
            Raster::R8 => 64e6 / 7.0,
            Raster::R6 => 48e6 / 7.0,
        }
    }
    /// The carrier spacing 1 / T_U.
    pub fn spacing(self) -> f64 {
        self.rate() / FFT as f64
    }
    /// The lowest input rate a receiver takes: 8 Msps or 6 Msps.
    pub fn min_rate(self) -> f64 {
        match self {
            Raster::R8 => 8e6,
            Raster::R6 => 6e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The guard interval (GUARD_INTERVAL).
    pub enum Guard {
        /// 1/64 (01): 64 samples, D_X = 12.
        G1_64 = ("1_64", "1/64"),
        /// 1/128 (00): 32 samples, D_X = 24.
        G1_128 = ("1_128", "1/128"),
    }
}

impl Guard {
    /// Guard samples.
    pub fn samples(self) -> usize {
        match self {
            Guard::G1_64 => 64,
            Guard::G1_128 => 32,
        }
    }
    /// Samples per symbol.
    pub fn symbol_len(self) -> usize {
        FFT + self.samples()
    }
    /// The scattered pilot separation D_X, also the unit of the L1 carrier fields.
    pub fn d_x(self) -> usize {
        match self {
            Guard::G1_64 => 12,
            Guard::G1_128 => 24,
        }
    }
    /// The preamble pilot amplitude A_PP.
    pub fn a_pp(self) -> f32 {
        match self {
            Guard::G1_64 => 4.0 / 3.0,
            Guard::G1_128 => 6.0 / 5.0,
        }
    }
    /// The GUARD_INTERVAL code.
    pub fn code(self) -> u8 {
        match self {
            Guard::G1_64 => 1,
            Guard::G1_128 => 0,
        }
    }
    /// The guard of a GUARD_INTERVAL code.
    pub fn from_code(c: u8) -> Option<Guard> {
        match c {
            1 => Some(Guard::G1_64),
            0 => Some(Guard::G1_128),
            _ => None,
        }
    }
    /// The width of the tuning position and notch start fields, and of the offset and notch width fields.
    pub fn field_bits(self) -> (u32, u32) {
        match self {
            Guard::G1_64 => (14, 9),
            Guard::G1_128 => (13, 8),
        }
    }
}

/// The data scrambling sequence w (clause 9.2.1, figure 31; the register of the DVB-T pilot sequence, `DVBT_PILOTS`) and the
/// pilot scrambling sequence w' (clause 9.2.2, `DVBC2_PILOT_PRIME`), both started from all ones.
pub struct Sequences {
    w: [u8; 2047],
    wp: [u8; 1023],
}

impl Sequences {
    fn new() -> Sequences {
        let mut w = [0u8; 2047];
        let mut wp = [0u8; 1023];
        let mut a = DVBT_PILOTS;
        a.fill_bits(&mut w);
        let mut b = DVBC2_PILOT_PRIME;
        b.fill_bits(&mut wp);
        Sequences { w, wp }
    }

    /// w_k, period 2047.
    pub fn w(&self, k: usize) -> u8 {
        self.w[k % 2047]
    }

    /// w'_i, period 1023.
    pub fn w_prime(&self, i: usize) -> u8 {
        self.wp[i % 1023]
    }

    /// w^P_k = w_k xor w'_i with i = (k mod K_L1) / 6, for k a multiple of 6.
    pub fn w_p(&self, k: usize) -> u8 {
        self.w(k) ^ self.w_prime((k % BLOCK) / 6)
    }

    /// Fills `out` with the pilot bits r_k for k = `first`, `first` + 6, ... (`n` values; `first` a multiple of 6): r_k = w^P_k
    /// at an L1 block start, else r_(k-6) xor w^P_k.
    pub fn pilot_bits(&self, first: usize, n: usize, out: &mut Vec<u8>) {
        out.clear();
        let mut k = first - first % BLOCK;
        let mut r = 0u8;
        while out.len() < n {
            let wp = self.w_p(k);
            r = if k.is_multiple_of(BLOCK) { wp } else { r ^ wp };
            if k >= first {
                out.push(r);
            }
            k += 6;
        }
    }
}

/// The scrambling sequences.
pub fn sequences() -> &'static Sequences {
    static S: OnceLock<Sequences> = OnceLock::new();
    S.get_or_init(Sequences::new)
}

struct Tables {
    continual: [bool; BLOCK],
    reserved: Vec<bool>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut continual = [false; BLOCK];
        for &c in &CONTINUAL {
            continual[usize::from(c)] = true;
        }
        let mut reserved = vec![false; RESERVED_PERIOD];
        for &r in &RESERVED_S0 {
            reserved[usize::from(r)] = true;
        }
        Tables { continual, reserved }
    })
}

/// The L1 block cell carried by carrier offset `kk` (0 to K_L1 - 1) of a preamble symbol: `q = kk - ceil(kk / 6)`, or `None`
/// for a pilot.
pub fn l1_cell(kk: usize) -> Option<usize> {
    (!kk.is_multiple_of(6)).then(|| kk - kk.div_ceil(6))
}

/// The frequency interleaver addresses H(q) for `cells` cells: H0 for even symbols, H1 for odd ones. The interleaved cell
/// `q` carries input cell `H(q)`.
pub fn freq_addresses(odd: bool, cells: usize, out: &mut Vec<u32>) {
    let rule = AddressRule {
        taps: &FREQ_TAPS,
        perm: &FREQ_PERM[usize::from(odd)],
    };
    if rule.addresses(cells, out).is_err() {
        out.clear();
    }
}

/// What a carrier of a data symbol carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Nothing: outside the system or inside a notch.
    Off,
    /// A scattered, continual or edge pilot.
    Pilot,
    /// A reserved tone.
    Reserved,
    /// A data cell.
    Data,
}

/// The carriers of a C2 system that data symbols use: its edges, its notches and whether reserved tones are on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The guard interval.
    pub guard: Guard,
    /// K_min, the lowest carrier (an edge pilot).
    pub k_min: usize,
    /// K_max, the highest carrier (an edge pilot).
    pub k_max: usize,
    /// The notches, (K_N,min, K_N,max) inclusive.
    pub notches: Vec<(usize, usize)>,
    /// Reserved tones are on.
    pub reserved: bool,
}

impl Layout {
    /// The class of carrier `k` in data symbol `l` (counted from the first data symbol of the frame).
    pub fn class(&self, k: usize, l: usize) -> Cell {
        if k < self.k_min || k > self.k_max {
            return Cell::Off;
        }
        let mut edge = k == self.k_min || k == self.k_max;
        for &(a, b) in &self.notches {
            if (a..=b).contains(&k) {
                return Cell::Off;
            }
            edge |= k + 1 == a || k == b + 1;
        }
        let dx = self.guard.d_x();
        let phase = dx * (l % D_Y);
        let t = tables();
        if edge || k % (dx * D_Y) == phase || t.continual[k % BLOCK] {
            return Cell::Pilot;
        }
        if self.reserved && t.reserved[(k + RESERVED_PERIOD - phase) % RESERVED_PERIOD] {
            return Cell::Reserved;
        }
        Cell::Data
    }

    /// The value of the pilot on carrier `k` given its bit r_k.
    pub fn pilot_value(r: u8) -> f32 {
        if r == 0 {
            PILOT_AMP
        } else {
            -PILOT_AMP
        }
    }
}

/// Whether carrier `k` of data symbol `l` is a reserved tone position of the pattern (without regard to pilots or notches).
pub fn reserved_position(k: usize, l: usize, guard: Guard) -> bool {
    let phase = guard.d_x() * (l % D_Y);
    tables().reserved[(k + RESERVED_PERIOD - phase) % RESERVED_PERIOD]
}

/// Whether `kk` (0 to K_L1 - 1) is a continual pilot offset.
pub fn continual(kk: usize) -> bool {
    tables().continual[kk % BLOCK]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_start_as_the_standard_says() {
        let s = sequences();
        let w: String = (0..32).map(|k| char::from(b'0' + s.w(k))).collect();
        assert_eq!(w, "11111111111000000000110000000111");
        let wp: String = (0..32).map(|i| char::from(b'0' + s.w_prime(i))).collect();
        assert_eq!(wp, "11111111110000000111000011111101");
        assert_eq!((0..2047).map(|k| u32::from(s.w(k))).sum::<u32>(), 1024);
        assert_eq!((0..1023).map(|i| u32::from(s.w_prime(i))).sum::<u32>(), 512);
    }
}
