//! The ISDB-S transmission structure: modulation and code words, slot units, the TMCC, the frame layout with bursts, the inner
//! code over the whole frame and the outer chain.
//!
//! Standard: ITU-R BO.1408-1.

use sdrkit_algo_fec::conv::{puncture, Encoder};
use sdrkit_algo_fec::rs::{codes::ISDB_S_TMCC_64_48, Rs};
use sdrkit_proto_dvbs2::inner::PunctureRate;
use sdrkit_util_bits::reader::{BitReader, BitWriter};
use sdrkit_util_bits::seq::{DVB_DISPERSAL, PRBS9_DISPERSAL};
use sdrkit_util_bits::BitOrder;

/// The RS(64,48) code of the TMCC (ARIB STD-B20, the RS(255,239) code shortened).
fn tmcc_code() -> Rs {
    // The code is a constant of the shared crate, so it is always valid.
    Rs::new(ISDB_S_TMCC_64_48).unwrap_or_else(|_| unreachable!("RS(64,48) is a valid code"))
}
use sdrkit_core::{Choice, Complex32};

/// Symbols per frame.
pub const FRAME: usize = 39_936;
/// Header symbols.
pub const HEADER: usize = 192;
/// Data symbols between bursts.
pub const RUN: usize = 203;
/// Slots per frame.
pub const SLOTS: usize = 48;
/// Air time of a slot, symbols.
pub const SLOT_SYMS: usize = 812;
/// Frames per super-frame.
pub const SF: usize = 8;
/// Symbol rate.
pub const RATE: f64 = 28.86e6;
/// W1, W2 (W3 = !W2).
pub const W1: u16 = 0x1B95;
/// W2.
pub const W2: u16 = 0xA340;

/// A modulation and code of the TMCC (code words 1 to 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mc {
    /// 1: BPSK 1/2.
    Bpsk12,
    /// 2 to 6: QPSK 1/2 to 7/8.
    Qpsk(PunctureRate),
    /// 7: TC8PSK 2/3.
    Tc8psk,
}

impl Mc {
    /// From the 4-bit code word.
    pub fn from_word(w: u8) -> Option<Mc> {
        Some(match w {
            1 => Mc::Bpsk12,
            2 => Mc::Qpsk(PunctureRate::R12),
            3 => Mc::Qpsk(PunctureRate::R23),
            4 => Mc::Qpsk(PunctureRate::R34),
            5 => Mc::Qpsk(PunctureRate::R56),
            6 => Mc::Qpsk(PunctureRate::R78),
            7 => Mc::Tc8psk,
            _ => return None,
        })
    }
    /// The code word.
    pub fn word(self) -> u8 {
        match self {
            Mc::Bpsk12 => 1,
            Mc::Qpsk(r) => 2 + PunctureRate::ALL.iter().position(|&x| x == r).unwrap_or(0) as u8,
            Mc::Tc8psk => 7,
        }
    }
    /// Slots of a unit: (assigned, carrying).
    pub fn unit(self) -> (usize, usize) {
        match self {
            Mc::Tc8psk => (1, 1),
            Mc::Qpsk(PunctureRate::R12) => (2, 1),
            Mc::Qpsk(PunctureRate::R23) => (3, 2),
            Mc::Qpsk(PunctureRate::R34) => (4, 3),
            Mc::Qpsk(PunctureRate::R56) => (6, 5),
            Mc::Qpsk(PunctureRate::R78) => (8, 7),
            Mc::Bpsk12 => (4, 1),
        }
    }
    /// The modulation and code as a choice.
    pub fn choice(self) -> McChoice {
        match self {
            Mc::Bpsk12 => McChoice::Bpsk12,
            Mc::Qpsk(PunctureRate::R12) => McChoice::Qpsk12,
            Mc::Qpsk(PunctureRate::R23) => McChoice::Qpsk23,
            Mc::Qpsk(PunctureRate::R34) => McChoice::Qpsk34,
            Mc::Qpsk(PunctureRate::R56) => McChoice::Qpsk56,
            Mc::Qpsk(PunctureRate::R78) => McChoice::Qpsk78,
            Mc::Tc8psk => McChoice::Tc8psk,
        }
    }
    /// "TC8PSK 2/3".
    pub fn name(self) -> &'static str {
        self.choice().label()
    }
    /// Net TS rate if all 48 slots used it, bit/s.
    pub fn full_rate(self) -> f64 {
        let (a, c) = self.unit();
        let packets_per_frame = SLOTS as f64 * c as f64 / a as f64;
        packets_per_frame * 188.0 * 8.0 * RATE / FRAME as f64
    }
    fn order(self) -> usize {
        match self {
            Mc::Tc8psk => 0,
            Mc::Qpsk(PunctureRate::R78) => 1,
            Mc::Qpsk(PunctureRate::R56) => 2,
            Mc::Qpsk(PunctureRate::R34) => 3,
            Mc::Qpsk(PunctureRate::R23) => 4,
            Mc::Qpsk(PunctureRate::R12) => 5,
            Mc::Bpsk12 => 6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// A modulation and code of the TMCC, as a choice.
    pub enum McChoice {
        /// BPSK 1/2.
        Bpsk12 = ("bpsk_1_2", "BPSK 1/2"),
        /// QPSK 1/2.
        Qpsk12 = ("qpsk_1_2", "QPSK 1/2"),
        /// QPSK 2/3.
        Qpsk23 = ("qpsk_2_3", "QPSK 2/3"),
        /// QPSK 3/4.
        Qpsk34 = ("qpsk_3_4", "QPSK 3/4"),
        /// QPSK 5/6.
        Qpsk56 = ("qpsk_5_6", "QPSK 5/6"),
        /// QPSK 7/8.
        Qpsk78 = ("qpsk_7_8", "QPSK 7/8"),
        /// TC8PSK 2/3.
        Tc8psk = ("tc8psk_2_3", "TC8PSK 2/3"),
    }
}

/// Why a layout of slots is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// More than four modulations.
    #[error("{got} modulations: at most four")]
    TooManyGroups {
        /// The modulations given.
        got: usize,
    },
    /// The slots do not add up to 48.
    #[error("{got} slots: the layout must fill exactly 48")]
    SlotCount {
        /// The slots given.
        got: usize,
    },
    /// A group is not a whole number of units of its modulation.
    #[error("{mc:?} in {slots} slots: not a whole number of units")]
    NotWholeUnits {
        /// The modulation and code.
        mc: Mc,
        /// The slots of the group.
        slots: u8,
    },
}

/// The TMCC of a super-frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Tmcc {
    /// Change counter (5 bits).
    pub change: u8,
    /// Up to 4 groups: modulation and slot count (dummies included), in efficiency order.
    pub groups: Vec<(Mc, u8)>,
    /// Relative TS number of each slot (dummies included).
    pub ts_of_slot: [u8; SLOTS],
    /// TS id of each relative number (0xFFFF unused).
    pub ts_ids: [u16; 8],
    /// Emergency warning flag.
    pub ews: bool,
    /// Uplink control.
    pub uplink: u8,
}

/// Checks a layout of slots: at most four modulations that fill 48 slots in whole units.
///
/// # Errors
///
/// The [`LayoutError`] of the first rule that the layout breaks.
pub fn check_layout(groups: &[(Mc, u8)]) -> Result<(), LayoutError> {
    if groups.len() > 4 {
        return Err(LayoutError::TooManyGroups { got: groups.len() });
    }
    let total: usize = groups.iter().map(|g| g.1 as usize).sum();
    if total != SLOTS {
        return Err(LayoutError::SlotCount { got: total });
    }
    for &(mc, slots) in groups {
        if !(slots as usize).is_multiple_of(mc.unit().0) {
            return Err(LayoutError::NotWholeUnits { mc, slots });
        }
    }
    Ok(())
}

impl Tmcc {
    /// Packs the 384 bits into 48 bytes.
    pub fn pack(&self) -> [u8; 48] {
        let mut bytes = Vec::with_capacity(48);
        let mut w = BitWriter::new(&mut bytes);
        w.put(self.change as u64, 5);
        for i in 0..4 {
            match self.groups.get(i) {
                Some((m, n)) => {
                    w.put(m.word() as u64, 4);
                    w.put(*n as u64, 6);
                }
                None => {
                    w.put(15, 4);
                    w.put(0, 6);
                }
            }
        }
        for &t in &self.ts_of_slot {
            w.put(t as u64, 3);
        }
        for &id in &self.ts_ids {
            w.put(id as u64, 16);
        }
        w.put(self.ews as u64, 1);
        w.put(self.uplink as u64, 4);
        w.put(0, 30);
        w.put(0, 32);
        let mut out = [0u8; 48];
        out.copy_from_slice(&bytes[..48]);
        out
    }
    /// Unpacks; `None` when the fields are not consistent.
    pub fn unpack(b: &[u8; 48]) -> Option<Tmcc> {
        let mut r = BitReader::new(b);
        let change = r.u(5) as u8;
        let mut groups = Vec::new();
        let mut total = 0usize;
        for _ in 0..4 {
            let word = r.u(4) as u8;
            let n = r.u(6) as u8;
            if word == 15 {
                continue;
            }
            let m = Mc::from_word(word)?;
            if !(n as usize).is_multiple_of(m.unit().0) {
                return None;
            }
            total += n as usize;
            groups.push((m, n));
        }
        if total > SLOTS {
            return None;
        }
        let mut ts_of_slot = [0u8; SLOTS];
        for t in ts_of_slot.iter_mut() {
            *t = r.u(3) as u8;
        }
        let mut ts_ids = [0u16; 8];
        for id in ts_ids.iter_mut() {
            *id = r.u(16) as u16;
        }
        let ews = r.u(1) == 1;
        let uplink = r.u(4) as u8;
        Some(Tmcc {
            change,
            groups,
            ts_of_slot,
            ts_ids,
            ews,
            uplink,
        })
    }
    /// The modulation of each slot and whether it carries a packet (dummies follow the carrying slots of a unit).
    pub fn slots(&self) -> Vec<(Mc, bool)> {
        let mut v = Vec::with_capacity(SLOTS);
        for &(m, n) in &self.groups {
            let (a, c) = m.unit();
            for k in 0..n as usize {
                v.push((m, k % a < c));
            }
        }
        v
    }
    /// The groups in efficiency order.
    pub fn sorted(mut groups: Vec<(Mc, u8)>) -> Vec<(Mc, u8)> {
        groups.sort_by_key(|g| g.0.order());
        groups
    }
}

/// RS(64,48) code words of a TMCC, energy-dispersed, as 8 blocks of 64 bits (one per frame).
pub fn tmcc_blocks(t: &Tmcc) -> [[u8; 64]; SF] {
    let mut cw = [0u8; 64];
    let packed = t.pack();
    cw[..48].copy_from_slice(&packed);
    // The data is 48 bytes and the code RS(64,48), so the lengths always match.
    let _ = tmcc_code().encode(&packed, &mut cw[48..]);
    let mut p = DVB_DISPERSAL;
    let mut out = [[0u8; 64]; SF];
    for (k, b) in cw.iter().enumerate() {
        for i in 0..8 {
            let bit = ((b >> (7 - i)) & 1) ^ p.bit();
            out[k / 8][(k % 8) * 8 + i] = bit;
        }
    }
    out
}

/// The TMCC from the 8 blocks (bits): RS decoding and unpacking. Returns the TMCC and the bytes corrected.
pub fn tmcc_decode(blocks: &[[u8; 64]; SF]) -> Option<(Tmcc, usize)> {
    let mut p = DVB_DISPERSAL;
    let mut cw = [0u8; 64];
    for k in 0..64 {
        let mut v = 0u8;
        for i in 0..8 {
            v = (v << 1) | (blocks[k / 8][(k % 8) * 8 + i] ^ p.bit());
        }
        cw[k] = v;
    }
    let n = tmcc_code().decode(&mut cw, &[]).ok()?.corrected as usize;
    let mut b = [0u8; 48];
    b.copy_from_slice(&cw[..48]);
    Tmcc::unpack(&b).map(|t| (t, n))
}

/// Header bits of frame `f` (0 to 7): W1, the TMCC block, W2 (frame 0) or W3.
pub fn header_bits(f: usize, block: &[u8; 64]) -> [u8; 96] {
    let mut h = [0u8; 96];
    for i in 0..16 {
        h[i] = ((W1 >> (15 - i)) & 1) as u8;
        let w = if f == 0 { W2 } else { !W2 };
        h[80 + i] = ((w >> (15 - i)) & 1) as u8;
    }
    h[16..80].copy_from_slice(block);
    h
}

/// BPSK of a bit.
#[inline]
pub fn bpsk(b: u8) -> Complex32 {
    Complex32::new(if b == 0 { 1.0 } else { -1.0 }, 0.0)
}

/// The TC8PSK point of label (W, Y, X).
#[inline]
pub fn psk8(w: u8, y: u8, x: u8) -> Complex32 {
    let idx = [0u32, 1, 3, 2][((y << 1) | x) as usize] + 4 * w as u32;
    let a = idx as f32 * std::f32::consts::FRAC_PI_4;
    Complex32::new(a.cos(), a.sin())
}

/// QPSK of (P1 on I, P0 on Q): (0, 0) at 45 degrees.
#[inline]
pub fn qpsk(i: u8, q: u8) -> Complex32 {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(if i == 0 { s } else { -s }, if q == 0 { s } else { -s })
}

/// The burst sequence: PN9 (1 + x^5 + x^9) from all ones in every frame, 4 symbols after each run.
pub fn burst_bits() -> Vec<u8> {
    let mut r = PRBS9_DISPERSAL;
    let mut b = vec![0u8; 192 * 4];
    r.fill_bits(&mut b);
    b
}

/// The PRBS bytes of a super-frame (48 slots of 204 bytes, 8 frames), from the load at its start.
pub fn sf_prbs() -> Vec<u8> {
    let mut p = DVB_DISPERSAL;
    let mut b = vec![0u8; SF * SLOTS * 204];
    p.fill_bytes(&mut b, BitOrder::Msb);
    b
}

/// Air position of data symbol `d` in a frame.
#[inline]
pub fn data_pos(d: usize) -> usize {
    HEADER + d + 4 * (d / RUN)
}

/// Encodes one frame's coded stream into symbols: the header and the packets of the carrying slots (203 bytes each, in slot
/// order) with the layout of `slots`. The encoder state runs on from the previous frame.
pub fn encode_frame(
    enc: &mut Encoder,
    header: &[u8; 96],
    slots: &[(Mc, bool)],
    packets: &[[u8; 203]],
    bursts: &[u8],
    out: &mut Vec<Complex32>,
) {
    out.clear();
    out.resize(FRAME, Complex32::new(0.0, 0.0));
    for (i, &b) in header.iter().enumerate() {
        let o = enc.push_bit(b);
        out[2 * i] = bpsk(o & 1);
        out[2 * i + 1] = bpsk(o >> 1);
    }
    let mut data: Vec<Complex32> = Vec::with_capacity(SLOTS * SLOT_SYMS);
    let mut k = 0;
    let mut s = 0;
    while s < SLOTS {
        let m = slots[s].0;
        // The group: consecutive slots of the same modulation.
        let mut e = s;
        while e < SLOTS && slots[e].0 == m {
            e += 1;
        }
        let carrying = (s..e).filter(|&i| slots[i].1).count();
        let bits: Vec<u8> = packets[k..k + carrying]
            .iter()
            .flat_map(|p| {
                p.iter()
                    .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
            })
            .collect();
        k += carrying;
        match m {
            Mc::Tc8psk => {
                for pair in bits.chunks(2) {
                    let o = enc.push_bit(pair[1]);
                    data.push(psk8(pair[0], o >> 1, o & 1));
                }
            }
            Mc::Qpsk(r) => {
                let mut coded = Vec::with_capacity(2 * bits.len());
                for &b in &bits {
                    let o = enc.push_bit(b);
                    coded.extend_from_slice(&[o & 1, o >> 1]);
                }
                let mut ser = Vec::new();
                puncture(&r.puncture(), 0, &coded, &mut ser);
                for c in ser.chunks(2) {
                    data.push(qpsk(c[0], c[1]));
                }
            }
            Mc::Bpsk12 => {
                for &b in &bits {
                    let o = enc.push_bit(b);
                    data.push(bpsk(o & 1));
                    data.push(bpsk(o >> 1));
                }
            }
        }
        // The carrying slots' symbols fill the group's air time exactly (whole units).
        data.resize(e * SLOT_SYMS, Complex32::new(0.0, 0.0));
        s = e;
    }
    for (d, v) in data.iter().enumerate().take(SLOTS * SLOT_SYMS) {
        out[data_pos(d)] = *v;
    }
    for r in 0..192 {
        for j in 0..4 {
            out[HEADER + r * (RUN + 4) + RUN + j] = bpsk(bursts[4 * r + j]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_units_tmcc_and_maps() {
        assert!((Mc::Tc8psk.full_rate() / 1e6 - 52.2).abs() < 0.06);
        assert!((Mc::Qpsk(PunctureRate::R12).full_rate() / 1e6 - 26.1).abs() < 0.06);
        assert!((Mc::Bpsk12.full_rate() / 1e6 - 13.0).abs() < 0.06);
        assert!((SF as f64 * FRAME as f64 / RATE * 1e3 - 11.07).abs() < 0.1);
        assert_eq!(HEADER + SLOTS * SLOT_SYMS + 192 * 4, FRAME);
        assert_eq!(burst_bits().len(), 768);
        assert!(check_layout(&[(Mc::Qpsk(PunctureRate::R34), 3), (Mc::Tc8psk, 45)]).is_err());
        assert!(check_layout(&[(Mc::Tc8psk, 40)]).is_err());
        let groups = Tmcc::sorted(vec![
            (Mc::Bpsk12, 4),
            (Mc::Tc8psk, 36),
            (Mc::Qpsk(PunctureRate::R34), 8),
        ]);
        assert!(check_layout(&groups).is_ok());
        let mut ts_of_slot = [0u8; SLOTS];
        for (i, t) in ts_of_slot.iter_mut().enumerate() {
            *t = if i < 36 {
                0
            } else if i < 44 {
                1
            } else {
                2
            };
        }
        let mut ids = [0xFFFFu16; 8];
        ids[..3].copy_from_slice(&[0x4010, 0x4011, 0x4012]);
        let t = Tmcc {
            change: 3,
            groups,
            ts_of_slot,
            ts_ids: ids,
            ews: false,
            uplink: 0,
        };
        assert_eq!(Tmcc::unpack(&t.pack()).unwrap(), t);
        let s = t.slots();
        assert_eq!(s[35], (Mc::Tc8psk, true));
        assert_eq!(s[39], (Mc::Qpsk(PunctureRate::R34), false));
        assert_eq!(s[44], (Mc::Bpsk12, true));
        assert_eq!(s[47], (Mc::Bpsk12, false));
        let mut blocks = tmcc_blocks(&t);
        for i in 0..8 {
            blocks[i][3] ^= 1;
        }
        assert_eq!(tmcc_decode(&blocks).unwrap().0, t);
        assert!((psk8(0, 1, 1) - Complex32::new(0.0, 1.0)).norm() < 1e-6);
        assert!((psk8(1, 1, 1) - Complex32::new(0.0, -1.0)).norm() < 1e-6);
        assert!((psk8(1, 0, 0) + psk8(0, 0, 0)).norm() < 1e-6);
        assert_eq!(!W2, 0x5CBF);
    }
}
