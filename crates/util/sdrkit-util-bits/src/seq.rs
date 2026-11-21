//! Named scramblers, whiteners, PN sequences, Gold codes and m-sequences.
//!
//! Each entry is an [`Lfsr`] (or a function that builds one from a parameter), written in this crate's notation (see
//! [`crate::lfsr`]): the polynomial is the recurrence (exponents are delays), the seed has stage 1 at bit 0, and the output is the
//! feedback unless the entry says the oldest stage. Where a standard writes the reciprocal polynomial, the entry says so.
//!
//! Use an entry with [`crate::lfsr::Additive`] to scramble, or read its bits directly:
//!
//! ```
//! use sdrkit_util_bits::lfsr::Additive;
//! use sdrkit_util_bits::seq;
//! let mut s = Additive::new(seq::PRBS9_DISPERSAL);
//! let mut bits = [0u8; 16];
//! s.apply_bits(&mut bits);
//! assert_eq!(bits, [0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0]);
//! ```

use crate::lfsr::{Lfsr, Multiplicative, Period};
use crate::pack::reverse_bits;
use crate::Param;
use crate::{BitOrder, Error, Result};

// ---------------------------------------------------------------------------------------------------------------------------
// 9-stage sequences
// ---------------------------------------------------------------------------------------------------------------------------

/// Energy dispersal PRBS x^9 + x^5 + 1 from all ones, output the feedback: ETSI EN 300 401 clause 12 (DAB), ETSI ES 201 980
/// clause 7.2.2 (DRM), ETSI EN 300 163 (NICAM, which writes it x^9 + x^4 + 1), ISDB-S bursts. Starts 0000 0111 1011 1110.
pub const PRBS9_DISPERSAL: Lfsr = Lfsr::fibonacci(0x221, 0x1FF);

/// PN9 data whitening of the TI CC1101/CC1200 and Semtech SX127x radios (TI design note DN509): the 9-stage register from all
/// ones, output the oldest stage, 8 bits per byte least significant first. Its bytes start FF E1 1D 9A ED 85 33 24. (The data
/// sheets call it x^9 + x^5 + 1, read from the other end of the register.)
pub const PN9_WHITENING: Lfsr = Lfsr::fibonacci(0x211, 0x1FF).with_output(0x100);

/// The PN9 whitening of IEEE 802.15.4g SUN FSK as decoded today: [`PN9_WHITENING`] with its first 9 bits skipped (so it starts
/// with the feedback), least significant bit first.
pub fn pn9_802154g() -> Lfsr {
    let mut l = PN9_WHITENING;
    l.skip(9);
    l
}

/// The dPMR scrambler (ETSI TS 102 658): the PN9 register started at 0x100 in the standard's labelling (stage 9 set), output
/// the oldest stage, restarted for each frame.
pub const DPMR: Lfsr = Lfsr::fibonacci(0x211, 0x001).with_output(0x100);

/// The NXDN scrambler (NXDN TS 1-A): x^9 + x^4 + 1 in the standard's notation, started at 0xE4 there (0x04E here), output the
/// oldest stage.
pub const NXDN: Lfsr = Lfsr::fibonacci(0x221, 0x04E).with_output(0x100);

/// The DTMB PN945 frame header (GB 20600-2006): `b[n] = b[n-9] ^ b[n-8] ^ b[n-7] ^ b[n-2]` from 1 1 1 1 1 0 1 1 1, period 511.
/// The header is its first 945 bits (bit 1 is chip -1); a later frame starts further into the period.
pub const DTMB_PN945: Lfsr = Lfsr::recurrence(9, 0x87, 0x1DF);

/// The DTMB PN595 frame header (GB 20600-2006): `b[n] = b[n-10] ^ b[n-3]` (x^10 + x^3 + 1) from 1 then nine zeros, period
/// 1023. The header is its first 595 bits (bit 1 is chip -1).
pub const DTMB_PN595: Lfsr = Lfsr::recurrence(10, 0x81, 0x001);

/// The DTMB PN420 frame header (GB 20600-2006): `b[n] = b[n-8] ^ b[n-6] ^ b[n-5] ^ b[n-1]` from 1 0 1 1 0 0 0 0, period 255.
/// The header is its first 420 bits (bit 1 is chip -1); a later frame starts further into the period.
pub const DTMB_PN420: Lfsr = Lfsr::recurrence(8, 0x8D, 0x0D);

// ---------------------------------------------------------------------------------------------------------------------------
// 7-stage sequences
// ---------------------------------------------------------------------------------------------------------------------------

/// The IEEE 802.11 OFDM scrambler x^7 + x^4 + 1 (IEEE 802.11-2020 clause 17.3.5.5) started at `seed` (7 bits; 0 is not allowed
/// by the standard and returns zeros). From all ones it is the 127-bit pilot polarity sequence of equation 17-25.
pub const fn ieee80211(seed: u8) -> Lfsr {
    Lfsr::fibonacci(0x91, seed as u64 & 0x7F)
}

/// The 802.11 scrambler state that continues a received sequence: `outputs` are its first seven output bits (the scrambled zero
/// bits at the start of a SERVICE field), first received first. An error when fewer than 7 are given.
pub fn ieee80211_from_outputs(outputs: &[u8]) -> Result<Lfsr> {
    if outputs.len() < 7 {
        return Err(Error::InputTooShort);
    }
    let s = crate::pack::to_u64(&outputs[..7]);
    Ok(Lfsr::fibonacci(0x91, s))
}

/// The D-STAR data scrambler: x^7 + x^4 + 1 from all ones (JARL D-STAR specification).
pub const DSTAR: Lfsr = Lfsr::fibonacci(0x91, 0x7F);

/// The Bluetooth LE whitening register for RF channel `channel` (0 to 39) (Bluetooth Core Specification Vol 6 Part B clause 3.2):
/// x^7 + x^4 + 1, position 0 set to 1 and positions 1 to 6 holding the channel index, most significant bit in position 1. Apply
/// it least significant bit of each byte first.
pub const fn ble_whitening(channel: u8) -> Lfsr {
    let seed = 1 | (reverse_bits(channel as u64 & 0x3F, 6) << 1);
    Lfsr::galois(0x91, seed)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Self-synchronising scramblers
// ---------------------------------------------------------------------------------------------------------------------------

/// The 802.11b (DSSS) self-synchronising scrambler 1 + z^-4 + z^-7 (IEEE 802.11-2020 clause 16.2.4), also used by ASTM F3411
/// Remote ID over 802.11b.
pub const fn ieee80211b(state: u8) -> Multiplicative {
    Multiplicative::fibonacci(0x91, state as u64)
}

/// The G3RUH 9600 bit/s packet scrambler x^17 + x^12 + 1 (J. Miller, G3RUH modem, 1988), also used by many CubeSats.
pub const fn g3ruh(state: u32) -> Multiplicative {
    Multiplicative::fibonacci(0x21001, state as u64)
}

/// The Eurobalise telegram scrambler (ERA SUBSET-036 clause 4.3.2): a 32-stage Galois self-synchronising register with
/// H(x) = x^31 + x^30 + x^29 + x^27 + x^25 + 1 below the top stage. `state` is the start value the telegram's B bits give.
pub const fn eurobalise(state: u32) -> Multiplicative {
    Multiplicative::galois(
        (1 << 32) | (1 << 31) | (1 << 30) | (1 << 29) | (1 << 27) | (1 << 25) | 1,
        state as u64,
    )
}

// ---------------------------------------------------------------------------------------------------------------------------
// 15-stage sequences
// ---------------------------------------------------------------------------------------------------------------------------

/// DVB energy dispersal 1 + x^14 + x^15 loaded with 100101010000000 (stages 1 to 15), output the feedback (ETSI EN 300 421
/// clause 4.4.1, EN 300 429, EN 300 744; the DVB-S2 baseband scrambler of EN 302 307-1 clause 5.2.2; ARIB STD-B31 for ISDB-T).
/// Its bytes start 0x03, 0xF6. The register is reloaded every 8 transport packets.
pub const DVB_DISPERSAL: Lfsr = Lfsr::fibonacci(0xC001, 0x00A9);

/// The VDL Mode 2 scrambler x^15 + x + 1 (ICAO Doc 9776), register 0x6959 at the first header bit in the document's labelling
/// (0x4D4B here); HFDL (ARINC 635) uses the same register and seed every 120 symbols.
pub const VDL2: Lfsr = Lfsr::fibonacci(0x8003, 0x4D4B);

/// The D2-MAC/packet energy dispersal x^15 + x + 1 from all ones (ETSI EN 300 250).
pub const MAC_PRBS15: Lfsr = Lfsr::fibonacci(0x8003, 0x7FFF);

/// The Inmarsat Aeronautical scrambler 1 + x + x^15 loaded with 110100101011001 (stages 1 to 15), restarted every frame (Inmarsat
/// Aeronautical System Definition Manual).
pub const AERO: Lfsr = Lfsr::fibonacci(0x8003, 0b100_1101_0100_1011);

/// The FEC block scrambler of the Inmarsat BGAN forward link (ETSI TS 102 744-2-1 clause 5.3.7): 1 + x + x^15 loaded with
/// 110 1001 0101 1001 (6959h, stage 15 of the standard as the most significant bit), output the feedback, restarted at each FEC
/// block. The same register as [`VDL2`] and [`AERO`]; its first 32 bits are 131BC425.
pub const BGAN: Lfsr = Lfsr::fibonacci(0x8003, 0x4D4B);

/// The S-VISSR frame scrambler x^15 + x^14 + 1 seeded 011001110011111 (JMA S-VISSR format; the frame bytes use bits 10 000 on,
/// odd bytes complemented).
pub const SVISSR: Lfsr = Lfsr::fibonacci(0xC001, 0b011_0011_1001_1111);

// ---------------------------------------------------------------------------------------------------------------------------
// 8-stage sequences and their byte tables
// ---------------------------------------------------------------------------------------------------------------------------

/// The CCSDS pseudo-randomiser (CCSDS 131.0-B clause 10, h(x) = x^8 + x^7 + x^5 + x^3 + 1 in CCSDS notation), all ones at the
/// start of each frame, output the oldest stage, most significant bit first: FF 48 0E C0 9A ...
pub const CCSDS_RANDOMISER: Lfsr = Lfsr::fibonacci(0x12B, 0xFF).with_output(0x80);

/// The CCSDS randomiser as a 255-byte table, XORed a byte at a time.
pub static CCSDS: Period<255> = Period::of(CCSDS_RANDOMISER, BitOrder::Msb);

/// The LoRa payload whitening (x^8 + x^6 + x^5 + x^4 + 1 from 0xFF): byte `i` is the register after `i` steps, so the table
/// starts FF FE FC F8 F0 E1 C2 85 (as in the published LoRa decoders, e.g. Tapparel et al. 2020).
pub static LORA_WHITENING: Period<255> = {
    let mut t = [0u8; 255];
    let mut r = Lfsr::fibonacci(0x171, 0xFF);
    let mut i = 0;
    while i < 255 {
        t[i] = r.state() as u8;
        r = r.step().0;
        i += 1;
    }
    Period::from_table(t)
};

/// The Inmarsat-C frame scrambler (Inmarsat-C SDM): 8 stages, a(n+8) = a(n) + a(n+2) + a(n+3) + a(n+4), started at 0x80 in the
/// manual's labelling, output the oldest stage; one bit per 4-byte group says whether the group is complemented. (The manual's
/// "x^7 + x^5 + x^4 + x^3" does not match this register.)
pub const INMARSAT_C: Lfsr = Lfsr::fibonacci(0x171, 0x01).with_output(0x80);

// ---------------------------------------------------------------------------------------------------------------------------
// 11- and 12-stage sequences
// ---------------------------------------------------------------------------------------------------------------------------

/// The HD Radio scrambler x^11 + x^9 + 1 from all ones, output the feedback, restarted every transfer frame (NRSC-5).
pub const HD_RADIO: Lfsr = Lfsr::fibonacci(0xA01, 0x7FF);

/// The DVB-T and ISDB-T pilot sequence w_k (ETSI EN 300 744 clause 4.5.2, written x^11 + x^2 + 1 there; ARIB
/// STD-B31): all ones at k = 0, output the oldest stage, so w_0 .. w_10 are 1.
pub const DVBT_PILOTS: Lfsr = Lfsr::fibonacci(0xA01, 0x7FF).with_output(0x400);

/// The DVB-C2 pilot scrambling sequence w'_i (ETSI EN 302 769 clause 9.2.2, figure 32): X^10 + X^3 + 1 from all ones, feedback
/// stage 10 XOR stage 7, output stage 10, so w'_i = w'_(i-10) XOR w'_(i-7) and w'_0 .. w'_9 are 1. Period 1023.
pub const DVBC2_PILOT_PRIME: Lfsr = Lfsr::fibonacci(0x481, 0x3FF).with_output(0x200);

/// The China Digital Radio scrambler x^12 + x^11 + x^8 + x^6 + 1 started 100000000000 (stage 12 set), restarted every logical frame (GY/T 268.1-2013).
pub const CDR: Lfsr = Lfsr::fibonacci(0x1941, 0x800);

// ---------------------------------------------------------------------------------------------------------------------------
// ATSC
// ---------------------------------------------------------------------------------------------------------------------------

/// The ATSC PN511 of the field sync (ATSC A/53 Part 2 clause 5.3.2.2): x^9 + x^7 + x^6 + x^4 + x^3 + x + 1, Galois, preloaded
/// 010000000. Starts 0000 0001 0111.
pub const ATSC_PN511: Lfsr = Lfsr::galois(0x2DB, 0x002);

/// The ATSC PN63 of the field sync: x^6 + x + 1, Galois, with the preload of A/53 Part 2 clause 5.3.2.2
/// (0x38 in this register).
pub const ATSC_PN63: Lfsr = Lfsr::galois(0x43, 0x38);

/// The ATSC data randomiser (ATSC A/53 Part 2 clause 5.3.1.1): a 16-stage Galois register x^16 + x^13 + x^12 + x^11 + x^7 + x^6 +
/// x^3 + x + 1 preloaded 0xF180 at each field, one byte per step taken from stages 1, 3, 4, 7, 11, 12, 13 and 14. Fills `out` from
/// the start of a field. Its bytes start C0 6D 3F 99 38 6A 29 52.
pub fn atsc_randomiser(out: &mut [u8]) {
    const TAKE: [u32; 8] = [0, 2, 3, 6, 10, 11, 12, 13];
    let mut l = Lfsr::galois(0x1_38CB, 0xF180);
    for o in out {
        let s = l.state();
        let mut b = 0u8;
        for (k, &bit) in TAKE.iter().enumerate() {
            b |= (((s >> bit) & 1) as u8) << k;
        }
        *o = b;
        l.bit();
    }
}

/// The ATSC 3.0 bootstrap PN sequence (ATSC A/321 clause 5.2.2): x^16 + x^15 + x^14 + x + 1, loaded with `seed` in the
/// standard's register order (stage r0 at bit 0), output the last stage.
pub const fn atsc3_bootstrap(seed: u16) -> Lfsr {
    Lfsr::fibonacci(0x1_C003, reverse_bits(seed as u64, 16)).with_output(0x8000)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Long registers
// ---------------------------------------------------------------------------------------------------------------------------

/// The TETRA scrambling sequence (ETSI EN 300 392-2 clause 8.2.5): the 32-stage register of the CRC-32 polynomial, with
/// p(k) = e(1 - k) for k = -29 .. 0 (e(1) .. e(30) the extended colour code: MCC, MNC, colour code, e(1) its most significant
/// bit) and p(-31) = p(-30) = 1. The output starts at p(1), the first new bit; `extended_colour` is 0 for the BSCH.
pub const fn tetra(extended_colour: u32) -> Lfsr {
    let e = extended_colour as u64 & 0x3FFF_FFFF;
    // Stage j + 1 holds p(-j): stage 1 = e(1), stage 30 = e(30), stages 31 and 32 = 1.
    let seed = reverse_bits(e, 30) | (0b11 << 30);
    Lfsr::fibonacci(0x1_04C1_1DB7, seed)
}

/// The SARSAT second-generation beacon spreading sequences (C/S T.018 clause 2.2.3): x^23 + x^18 + 1 (in T.018's notation), the
/// I arm loaded with 1 followed by 22 zeros and the Q arm with 0011 1111 1000 0011 0101 100, the register contents being the
/// first 23 chips. The first 64 I chips are 8000 0108 4212 84A1 in hex.
pub const fn sarsat_t018(q_arm: bool) -> Lfsr {
    // The 23 chips, first chip in the top bit.
    let seed = if q_arm { 0x1F_C1AC } else { 0x40_0000 };
    Lfsr::fibonacci((1 << 23) | (1 << 5) | 1, seed).with_output(1 << 22)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Gold codes and m-sequences
// ---------------------------------------------------------------------------------------------------------------------------

/// The length-31 Gold sequence c(n) of 3GPP TS 36.211 clause 7.2 and TS 38.211 clause 5.2.1 (Nc = 1600) for `c_init`, written
/// into `out` (0 or 1 each). x1 is started 1, 0, 0 ..., x2 with the bits of `c_init`. Runs a 32-bit word per step after the
/// 1600-step offset, which is jumped in log time.
pub fn gold_3gpp(c_init: u32, out: &mut [u8]) {
    // x(n + 31) = x(n + 3) + x(n) and x(n + 31) = x(n + 3) + x(n + 2) + x(n + 1) + x(n).
    let mut x1 = Lfsr::recurrence(31, 0b1001, 1);
    let mut x2 = Lfsr::recurrence(31, 0b1111, (c_init & 0x7FFF_FFFF) as u64);
    x1.skip(1600);
    x2.skip(1600);
    // The registers hold x(n) at stage 31 (bit 30) down to x(n + 30) at stage 1; read them as words with x(n) at bit 0.
    let mut a = reverse_bits(x1.state(), 31) as u32;
    let mut b = reverse_bits(x2.state(), 31) as u32;
    for chunk in out.chunks_mut(28) {
        // Bits 0..30 of a and b are x(n) .. x(n + 30); the next 28 values come from the recurrence at once.
        let c = a ^ b;
        for (i, o) in chunk.iter_mut().enumerate() {
            *o = ((c >> i) & 1) as u8;
        }
        let na = (a ^ (a >> 3)) & 0x0FFF_FFFF;
        let nb = (b ^ (b >> 1) ^ (b >> 2) ^ (b >> 3)) & 0x0FFF_FFFF;
        a = (a >> 28) | (na << 3);
        b = (b >> 28) | (nb << 3);
    }
}

const DVBS2_M: usize = (1 << 18) - 1;

/// The x and y m-sequences of the DVB-S2 physical-layer scrambling (ETSI EN 302 307-1 clause 5.5.4), packed 64 per word, built
/// at compile time: x(i + 18) = x(i + 7) + x(i) from x(0) = 1, y(i + 18) = y(i + 10) + y(i + 7) + y(i + 5) + y(i) from all ones.
#[allow(long_running_const_eval)]
static DVBS2_XY: ([u64; 4096], [u64; 4096]) = {
    let mut x = [0u64; 4096];
    let mut y = [0u64; 4096];
    let mut rx = Lfsr::recurrence(18, (1 << 7) | 1, 1);
    let mut ry = Lfsr::recurrence(18, (1 << 10) | (1 << 7) | (1 << 5) | 1, 0x3FFFF);
    let mut i = 0;
    while i < DVBS2_M {
        let (nx, bx) = rx.step();
        let (ny, by) = ry.step();
        rx = nx;
        ry = ny;
        x[i / 64] |= (bx as u64) << (i % 64);
        y[i / 64] |= (by as u64) << (i % 64);
        i += 1;
    }
    (x, y)
};

#[inline]
fn dvbs2_bit(t: &[u64; 4096], i: usize) -> u8 {
    ((t[i / 64] >> (i % 64)) & 1) as u8
}

/// The DVB-S2 physical-layer scrambling sequence R_n(i) (ETSI EN 302 307-1 clause 5.5.4) of Gold code `n` (0 to 262 141), from
/// symbol `start`, written into `out`: each value 0 to 3 is the number of quarter turns. No table is built per call.
pub fn gold_dvbs2(n: u32, start: usize, out: &mut [u8]) -> Result<()> {
    if n as usize >= DVBS2_M - 1 {
        return Err(Error::BadParameter(Param::GoldCode));
    }
    let (x, y) = &DVBS2_XY;
    let z = |i: usize| dvbs2_bit(x, (i + n as usize) % DVBS2_M) ^ dvbs2_bit(y, i % DVBS2_M);
    for (k, o) in out.iter_mut().enumerate() {
        let i = (start + k) % DVBS2_M;
        *o = 2 * z((i + 131_072) % DVBS2_M) + z(i);
    }
    Ok(())
}

/// GPS C/A code G2 phase selector taps for PRN 1 to 32 (IS-GPS-200 table 3-Ia).
pub const GPS_CA_TAPS: [(u8, u8); 32] = [
    (2, 6),
    (3, 7),
    (4, 8),
    (5, 9),
    (1, 9),
    (2, 10),
    (1, 8),
    (2, 9),
    (3, 10),
    (2, 3),
    (3, 4),
    (5, 6),
    (6, 7),
    (7, 8),
    (8, 9),
    (9, 10),
    (1, 4),
    (2, 5),
    (3, 6),
    (4, 7),
    (5, 8),
    (6, 9),
    (1, 3),
    (4, 6),
    (5, 7),
    (6, 8),
    (7, 9),
    (8, 10),
    (1, 6),
    (2, 7),
    (3, 8),
    (4, 9),
];

/// Writes the 1023-chip GPS C/A code of PRN 1 to 32 into `out` (IS-GPS-200 clause 3.2.1.3): G1 = 1 + x^3 + x^10 and G2 = 1 + x^2 +
/// x^3 + x^6 + x^8 + x^9 + x^10 from all ones, the chip being G1's last stage XOR the two G2 stages of the PRN's phase selector.
pub fn gps_ca(prn: u8, out: &mut [u8; 1023]) -> Result<()> {
    let (a, b) = *GPS_CA_TAPS
        .get((prn as usize).wrapping_sub(1))
        .ok_or(Error::BadParameter(Param::GpsPrn))?;
    let mut g1 = Lfsr::fibonacci(0x409, 0x3FF).with_output(1 << 9);
    let mut g2 = Lfsr::fibonacci(0x74D, 0x3FF).with_output((1 << (a - 1)) | (1 << (b - 1)));
    for o in out.iter_mut() {
        *o = g1.bit() ^ g2.bit();
    }
    Ok(())
}

/// G2 delays in chips of GPS PRN 1 to 32 (IS-GPS-200 table 3-Ia, "Code Delay Chips" column).
pub const GPS_CA_DELAYS: [u16; 32] = [
    5, 6, 7, 8, 17, 18, 139, 140, 141, 251, 252, 254, 255, 256, 257, 258, 469, 470, 471, 472, 473,
    474, 509, 512, 513, 514, 515, 516, 859, 860, 861, 862,
];

/// G2 delays in chips of SBAS PRN 120 to 158 (RTCA DO-229 appendix A table A-1, as the GPS PRN assignment list repeats it).
pub const SBAS_CA_DELAYS: [u16; 39] = [
    145, 175, 52, 21, 237, 235, 886, 657, 634, 762, 355, 1012, 176, 603, 130, 359, 595, 68, 386,
    797, 456, 499, 883, 307, 127, 211, 121, 118, 163, 628, 853, 484, 289, 811, 202, 1021, 463, 568,
    904,
];

/// G2 delays in chips of QZSS L1 C/A PRN 193 to 202 (IS-QZSS-PNT table 3.2.2-1).
pub const QZSS_CA_DELAYS: [u16; 10] = [339, 208, 711, 189, 263, 537, 663, 942, 173, 900];

/// Writes the 1023-chip C/A-family code whose G2 sequence is delayed by `delay` chips into `out` (IS-GPS-200 clause 3.2.1.3):
/// chip t is G1(t) XOR G2(t − delay), with G1 and G2 as in [`gps_ca`]. SBAS and QZSS L1 C/A codes are defined this way.
/// Returns an error when `delay` is 1023 or more.
///
/// ```
/// use sdrkit_util_bits::seq::{ca_by_delay, SBAS_CA_DELAYS};
/// let mut c = [0u8; 1023];
/// ca_by_delay(SBAS_CA_DELAYS[0], &mut c)?; // PRN 120
/// assert_eq!(&c[..10], &[0, 1, 1, 0, 1, 1, 1, 0, 0, 1]); // 0671 octal
/// # Ok::<(), sdrkit_util_bits::Error>(())
/// ```
pub fn ca_by_delay(delay: u16, out: &mut [u8; 1023]) -> Result<()> {
    if delay >= 1023 {
        return Err(Error::BadParameter(Param::G2Delay));
    }
    let mut g1 = Lfsr::fibonacci(0x409, 0x3FF).with_output(1 << 9);
    let mut g2 = Lfsr::fibonacci(0x74D, 0x3FF).with_output(1 << 9);
    // Chip 0 takes G2 output 1023 − delay: run G2 that far first.
    for _ in 0..(1023 - delay) % 1023 {
        g2.bit();
    }
    for o in out.iter_mut() {
        *o = g1.bit() ^ g2.bit();
    }
    Ok(())
}

/// The G2 delay of an L1 C/A-family PRN: GPS 1 to 32, SBAS 120 to 158 and QZSS 193 to 202. `None` for any other number.
pub fn l1_ca_delay(prn: u16) -> Option<u16> {
    match prn {
        1..=32 => Some(GPS_CA_DELAYS[prn as usize - 1]),
        120..=158 => Some(SBAS_CA_DELAYS[prn as usize - 120]),
        193..=202 => Some(QZSS_CA_DELAYS[prn as usize - 193]),
        _ => None,
    }
}

/// Writes the L1 C/A code of a GPS (1 to 32), SBAS (120 to 158) or QZSS (193 to 202) PRN into `out`. Returns an error for
/// any other number.
pub fn l1_ca(prn: u16, out: &mut [u8; 1023]) -> Result<()> {
    let d = l1_ca_delay(prn).ok_or(Error::BadParameter(Param::L1CaPrn))?;
    ca_by_delay(d, out)
}

/// A Zadoff-Chu sequence of root `u` and length `N`: x(n) = exp(−jπ u n (n + 1) / N) for odd N, and exp(−jπ u n² / N) for
/// even N. LTE and NR use it for the PSS, PRACH and reference signals, NB-IoT for the NPSS and NSSS.
/// Standard: 3GPP TS 36.211 clause 5.5.1.1 and clause 6.11.1.1.
///
/// The phase is kept as an exact whole number k with x(n) = exp(−jπ k / N), so long sequences lose no precision.
///
/// ```
/// use sdrkit_util_bits::seq::ZadoffChu;
/// let zc = ZadoffChu::new(25, 63)?; // the LTE PSS root of N_ID2 = 0
/// assert_eq!(zc.phase_index(1), 50);
/// let [re, im] = zc.value(0);
/// assert_eq!((re, im), (1.0, 0.0));
/// # Ok::<(), sdrkit_util_bits::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZadoffChu {
    root: u32,
    len: u32,
}

impl ZadoffChu {
    /// The sequence of root `root` and length `len`. Returns an error when `root` is not from 1 to `len` − 1 or has a common
    /// factor with `len` (the sequence then loses its constant-amplitude zero-autocorrelation property).
    pub fn new(root: u32, len: u32) -> Result<Self> {
        let gcd = |mut a: u32, mut b: u32| {
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        };
        if root == 0 || root >= len || gcd(root, len) != 1 {
            return Err(Error::BadParameter(Param::ZadoffChuRoot));
        }
        Ok(ZadoffChu { root, len })
    }

    /// The root u.
    pub fn root(&self) -> u32 {
        self.root
    }

    /// The length N.
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Always false: a sequence has at least two values.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The phase index k of value `n` (taken modulo N), from 0 to 2N − 1: x(n) = exp(−jπ k / N).
    pub fn phase_index(&self, n: u64) -> u64 {
        let (u, len) = (self.root as u128, self.len as u128);
        let n = n as u128 % len;
        let q = if len % 2 == 1 { n * (n + 1) } else { n * n };
        (u * q % (2 * len)) as u64
    }

    /// The phase of value `n` in radians, from −2π to 0.
    pub fn phase(&self, n: u64) -> f64 {
        -std::f64::consts::PI * self.phase_index(n) as f64 / self.len as f64
    }

    /// Value `n` as (real, imaginary).
    pub fn value(&self, n: u64) -> [f64; 2] {
        let k = self.phase_index(n);
        if k == 0 {
            return [1.0, 0.0];
        }
        let (s, c) = self.phase(n).sin_cos();
        [c, s]
    }

    /// Writes values `start`, `start + 1` ... into `out` as (real, imaginary) pairs. Never allocates.
    pub fn fill(&self, start: u64, out: &mut [[f32; 2]]) {
        for (i, o) in out.iter_mut().enumerate() {
            let [re, im] = self.value(start + i as u64);
            *o = [re as f32, im as f32];
        }
    }
}

/// BeiDou B1I G2 phase selector taps for ranging codes 1 to 37 (BDS-SIS-ICD-B1I table 4-1).
pub const BDS_B1I_TAPS: [(u8, u8); 37] = [
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 8),
    (1, 9),
    (1, 10),
    (1, 11),
    (2, 7),
    (3, 4),
    (3, 5),
    (3, 6),
    (3, 8),
    (3, 9),
    (3, 10),
    (3, 11),
    (4, 5),
    (4, 6),
    (4, 8),
    (4, 9),
    (4, 10),
    (4, 11),
    (5, 6),
    (5, 8),
    (5, 9),
    (5, 10),
    (5, 11),
    (6, 8),
    (6, 9),
    (6, 10),
    (6, 11),
    (8, 9),
    (8, 10),
    (8, 11),
    (9, 10),
    (9, 11),
    (10, 11),
];

/// Writes the 2046-chip BeiDou B1I ranging code 1 to 37 into `out` (BDS-SIS-ICD-B1I clause 4.1.2): G1 = 1 + x + x^7 + x^8 + x^9 +
/// x^10 + x^11 and G2 = 1 + x + x^2 + x^3 + x^4 + x^5 + x^8 + x^9 + x^11, both loaded 01010101010, the 2047-chip Gold code
/// truncated by one chip.
pub fn bds_b1i(code: u8, out: &mut [u8; 2046]) -> Result<()> {
    let (a, b) = *BDS_B1I_TAPS
        .get((code as usize).wrapping_sub(1))
        .ok_or(Error::BadParameter(Param::BeidouCode))?;
    let mut g1 = Lfsr::fibonacci(0xF83, 0x2AA).with_output(1 << 10);
    let mut g2 = Lfsr::fibonacci(0xB3F, 0x2AA).with_output((1 << (a - 1)) | (1 << (b - 1)));
    for o in out.iter_mut() {
        *o = g1.bit() ^ g2.bit();
    }
    Ok(())
}

/// The GLONASS L1OF/L2OF ranging code (GLONASS ICD edition 5.1 clause 3.3.1.2): 511 chips of 1 + x^5 + x^9 from all ones, taken
/// from the 7th stage. Starts 111111100.
pub const GLONASS_RANGING: Lfsr = Lfsr::fibonacci(0x221, 0x1FF).with_output(1 << 6);

/// LTE SSS m-sequence s~: x(i + 5) = x(i + 2) + x(i), x(0..4) = 0 0 0 0 1 (3GPP TS 36.211 clause 6.11.2.1).
pub const LTE_SSS_S: Lfsr = Lfsr::recurrence(5, 0b00101, 0b10000);
/// LTE SSS m-sequence c~: x(i + 5) = x(i + 3) + x(i).
pub const LTE_SSS_C: Lfsr = Lfsr::recurrence(5, 0b01001, 0b10000);
/// LTE SSS m-sequence z~: x(i + 5) = x(i + 4) + x(i + 2) + x(i + 1) + x(i).
pub const LTE_SSS_Z: Lfsr = Lfsr::recurrence(5, 0b10111, 0b10000);
/// NR PSS m-sequence: x(i + 7) = x(i + 4) + x(i), [x(6) .. x(0)] = 1110110 (3GPP TS 38.211 clause 7.4.2.2.1).
pub const NR_PSS: Lfsr = Lfsr::recurrence(7, 0b0010001, 0b1110110);
/// NR SSS m-sequence x0: x0(i + 7) = x0(i + 4) + x0(i), [x(6) .. x(0)] = 0000001 (3GPP TS 38.211 clause 7.4.2.3.1).
pub const NR_SSS_X0: Lfsr = Lfsr::recurrence(7, 0b0010001, 0b0000001);
/// NR SSS m-sequence x1: x1(i + 7) = x1(i + 1) + x1(i).
pub const NR_SSS_X1: Lfsr = Lfsr::recurrence(7, 0b0000011, 0b0000001);

// ---------------------------------------------------------------------------------------------------------------------------
// Fixed tables
// ---------------------------------------------------------------------------------------------------------------------------

/// The M17 randomiser (M17 specification, "Randomizer"): 46 bytes XORed onto the 368 type-4 bits, most significant bit first.
pub static M17_RANDOMISER: Period<46> = Period::from_table([
    0xD6, 0xB5, 0xE2, 0x30, 0x82, 0xFF, 0x84, 0x62, 0xBA, 0x4E, 0x96, 0x90, 0xD8, 0x98, 0xDD, 0x5D,
    0x0C, 0xC8, 0x52, 0x43, 0x91, 0x1D, 0xF8, 0x6E, 0x68, 0x2F, 0x35, 0xDA, 0x14, 0xEA, 0xCD, 0x76,
    0x19, 0x8D, 0xD5, 0x80, 0xD1, 0x33, 0x87, 0x13, 0x57, 0x18, 0x2D, 0x29, 0x78, 0xC3,
]);

/// The Vaisala RS41 frame whitening: 64 bytes XORed onto every frame byte, repeating (Vaisala RS41 frame format, as published by
/// the open radiosonde decoders).
pub static RS41_WHITENING: Period<64> = Period::from_table([
    0x96, 0x83, 0x3E, 0x51, 0xB1, 0x49, 0x08, 0x98, 0x32, 0x05, 0x59, 0x0E, 0xF9, 0x44, 0xC6, 0x26,
    0x21, 0x60, 0xC2, 0xEA, 0x79, 0x5D, 0x6D, 0xA1, 0x54, 0x69, 0x47, 0x0C, 0xDC, 0xE8, 0x5C, 0xF1,
    0xF7, 0x76, 0x82, 0x7F, 0x07, 0x99, 0xA2, 0x2C, 0x93, 0x7C, 0x30, 0x63, 0xF5, 0x10, 0x2E, 0x61,
    0xD0, 0xBC, 0xB4, 0xB6, 0x06, 0xAA, 0xF4, 0x23, 0x78, 0x6E, 0x3B, 0xAE, 0xBF, 0x7B, 0x4C, 0xC1,
]);

/// The FT4 77-bit message scrambling vector "rvec" (Franke, Somerville and Taylor, QEX 2020, "The FT4 and FT8 Communication
/// Protocols").
pub const FT4_RVEC: [u8; 77] = [
    0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 0, 0,
    1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1,
];

/// The DVB-S2 PLS scrambling sequence, 64 bits, first sent most significant (ETSI EN 302 307-1 clause 5.5.2.4).
pub const DVBS2_PLS_SCRAMBLE: u64 = 0x719D_83C9_5342_2DFA;

/// The eCzas (Polish time signal) 5-byte frame scrambling mask.
pub const ECZAS_SCRAMBLE: [u8; 5] = [0x0A, 0x47, 0x55, 0x4D, 0x2B];

/// Loran-C master phase codes for intervals A and B, +1 or -1 per pulse (USCG Loran-C Signal Specification, COMDTPUB
/// P16562.6).
pub const LORAN_MASTER: [[i8; 9]; 2] = [
    [1, 1, -1, -1, 1, -1, 1, -1, 1],
    [1, -1, -1, 1, 1, 1, 1, 1, -1],
];

/// Loran-C secondary phase codes for intervals A and B.
pub const LORAN_SECONDARY: [[i8; 8]; 2] =
    [[1, 1, 1, 1, 1, -1, -1, 1], [1, -1, 1, -1, 1, 1, -1, -1]];

// ---------------------------------------------------------------------------------------------------------------------------
// Lookup by name
// ---------------------------------------------------------------------------------------------------------------------------

/// Every fixed additive scrambler register above, by a stable key (the lower-case name of its constant), for code that picks one
/// at run time (a setting, a file). Each register is the constant of that name, with its seed: use it with
/// [`crate::lfsr::Additive`]. Registers built from a parameter (802.11, BLE, TETRA ...) and the sequences that are not scramblers
/// (pilots, sync codes) are not listed.
pub const SCRAMBLERS: &[(&str, Lfsr)] = &[
    ("prbs9_dispersal", PRBS9_DISPERSAL),
    ("pn9_whitening", PN9_WHITENING),
    ("dpmr", DPMR),
    ("nxdn", NXDN),
    ("dstar", DSTAR),
    ("dvb_dispersal", DVB_DISPERSAL),
    ("vdl2", VDL2),
    ("mac_prbs15", MAC_PRBS15),
    ("aero", AERO),
    ("bgan", BGAN),
    ("svissr", SVISSR),
    ("ccsds_randomiser", CCSDS_RANDOMISER),
    ("hd_radio", HD_RADIO),
    ("cdr", CDR),
];

/// The scrambler register whose key is `key` (see [`SCRAMBLERS`]).
///
/// ```
/// use sdrkit_util_bits::seq;
/// assert_eq!(seq::scrambler("dvb_dispersal"), Some(seq::DVB_DISPERSAL));
/// assert_eq!(seq::scrambler("DVB_DISPERSAL"), None);
/// ```
pub fn scrambler(key: &str) -> Option<Lfsr> {
    SCRAMBLERS.iter().find(|(k, _)| *k == key).map(|&(_, l)| l)
}

/// The keys [`ByteSequence::named`] accepts: registers read 8 bits per byte (`dvb_dispersal` and `prbs9_dispersal` most significant
/// bit first, `pn9_whitening` least significant bit first) and the byte tables `ccsds` ([`CCSDS`]), `lora_whitening`,
/// `m17_randomiser` and `rs41_whitening`.
pub const BYTE_SEQUENCES: &[&str] = &[
    "dvb_dispersal",
    "prbs9_dispersal",
    "pn9_whitening",
    "ccsds",
    "lora_whitening",
    "m17_randomiser",
    "rs41_whitening",
];

/// Where a [`ByteSequence`] takes its bytes from.
#[derive(Clone, Debug)]
enum ByteSource {
    /// A register read 8 bits per byte.
    Register(crate::lfsr::Additive, BitOrder),
    /// A table that repeats, and the index of the next byte.
    Table(&'static [u8], usize),
}

/// A sequence of bytes XORed onto data a byte at a time (whiteners and randomisers of byte-oriented links), with its position kept
/// between calls. Never allocates.
///
/// ```
/// use sdrkit_util_bits::seq::ByteSequence;
/// // CCSDS 131.0-B clause 10: the randomiser starts FF 48 0E C0 9A.
/// let mut s = ByteSequence::named("ccsds").unwrap();
/// let mut d = [0u8; 5];
/// s.apply(&mut d);
/// assert_eq!(d, [0xFF, 0x48, 0x0E, 0xC0, 0x9A]);
/// ```
#[derive(Clone, Debug)]
pub struct ByteSequence {
    src: ByteSource,
}

impl ByteSequence {
    /// The sequence whose key is `key` (see [`BYTE_SEQUENCES`]).
    pub fn named(key: &str) -> Option<ByteSequence> {
        use crate::lfsr::Additive;
        Some(match key {
            "dvb_dispersal" => ByteSequence::register(DVB_DISPERSAL, BitOrder::Msb),
            "prbs9_dispersal" => ByteSequence::register(PRBS9_DISPERSAL, BitOrder::Msb),
            "pn9_whitening" => ByteSequence {
                src: ByteSource::Register(Additive::new(PN9_WHITENING), BitOrder::Lsb),
            },
            "ccsds" => ByteSequence::table(CCSDS.bytes()),
            "lora_whitening" => ByteSequence::table(LORA_WHITENING.bytes()),
            "m17_randomiser" => ByteSequence::table(M17_RANDOMISER.bytes()),
            "rs41_whitening" => ByteSequence::table(RS41_WHITENING.bytes()),
            _ => return None,
        })
    }

    /// A register read 8 bits per byte in `order`, from its seed.
    pub fn register(lfsr: Lfsr, order: BitOrder) -> ByteSequence {
        ByteSequence {
            src: ByteSource::Register(crate::lfsr::Additive::new(lfsr), order),
        }
    }

    /// A table that repeats (an empty table XORs nothing).
    pub fn table(table: &'static [u8]) -> ByteSequence {
        ByteSequence {
            src: ByteSource::Table(table, 0),
        }
    }

    /// Starts the sequence again.
    pub fn reset(&mut self) {
        match &mut self.src {
            ByteSource::Register(a, _) => a.reset(),
            ByteSource::Table(_, at) => *at = 0,
        }
    }

    /// XORs `data` with the next bytes of the sequence.
    pub fn apply(&mut self, data: &mut [u8]) {
        match &mut self.src {
            ByteSource::Register(a, order) => a.apply_bytes(data, *order),
            ByteSource::Table(t, at) => {
                if t.is_empty() {
                    return;
                }
                for d in data {
                    *d ^= t[*at];
                    *at += 1;
                    if *at == t.len() {
                        *at = 0;
                    }
                }
            }
        }
    }

    /// Moves the sequence on by `n` bytes without using them.
    pub fn advance(&mut self, n: usize) {
        match &mut self.src {
            ByteSource::Register(a, order) => {
                let mut scratch = [0u8; 1];
                for _ in 0..n {
                    a.apply_bytes(&mut scratch, *order);
                }
            }
            ByteSource::Table(t, at) => {
                if !t.is_empty() {
                    *at = (*at + n % t.len()) % t.len();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::to_u64;

    fn bits(mut l: Lfsr, n: usize) -> Vec<u8> {
        let mut v = vec![0u8; n];
        l.fill_bits(&mut v);
        v
    }

    fn bytes(mut l: Lfsr, n: usize, order: BitOrder) -> Vec<u8> {
        let mut v = vec![0u8; n];
        l.fill_bytes(&mut v, order);
        v
    }

    #[test]
    fn energy_dispersal_starts_as_published() {
        // EN 300 401 / ES 201 980: the first 16 bits 0000 0111 1011 1110.
        assert_eq!(
            bits(PRBS9_DISPERSAL, 16),
            [0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0]
        );
        assert_eq!(PRBS9_DISPERSAL.period(1000), Some(511));
        // EN 300 421 figure 2: 0000 0011 1111 0110 ...
        assert_eq!(bytes(DVB_DISPERSAL, 2, BitOrder::Msb), [0x03, 0xF6]);
        assert_eq!(DVB_DISPERSAL.period(40000), Some(32767));
    }

    #[test]
    fn byte_sequences_by_name_give_their_published_first_bytes() {
        // (key, first bytes as the standard prints them)
        let want: [(&str, &[u8]); 7] = [
            ("dvb_dispersal", &[0x03, 0xF6]),           // EN 300 421 figure 2
            ("prbs9_dispersal", &[0x07, 0xBE]),         // EN 300 401 clause 12: 0000 0111 1011 1110
            ("pn9_whitening", &[0xFF, 0xE1, 0x1D]),     // TI DN509
            ("ccsds", &[0xFF, 0x48, 0x0E, 0xC0, 0x9A]), // CCSDS 131.0-B clause 10
            ("lora_whitening", &[0xFF, 0xFE, 0xFC, 0xF8]),
            ("m17_randomiser", &[0xD6, 0xB5, 0xE2, 0x30]), // M17 specification
            ("rs41_whitening", &[0x96, 0x83, 0x3E, 0x51]),
        ];
        assert_eq!(want.len(), BYTE_SEQUENCES.len());
        for (key, first) in want {
            let mut s = ByteSequence::named(key).unwrap();
            let mut d = vec![0u8; first.len()];
            s.apply(&mut d);
            assert_eq!(d, first, "{key}");
            // Cut anywhere, with skips, the sequence runs on as one.
            let mut whole = vec![0u8; 300];
            s.reset();
            s.apply(&mut whole);
            s.reset();
            let mut parts = vec![0u8; 300];
            s.apply(&mut parts[..7]);
            s.advance(100);
            s.apply(&mut parts[107..]);
            assert_eq!(parts[..7], whole[..7], "{key}");
            assert_eq!(parts[107..], whole[107..], "{key}");
        }
        assert!(ByteSequence::named("custom").is_none());
        let mut empty = ByteSequence::table(&[]);
        let mut d = [1u8; 3];
        empty.apply(&mut d);
        empty.advance(5);
        assert_eq!(d, [1; 3]);
    }

    #[test]
    fn scramblers_by_name_are_the_constants() {
        let mut seen = std::collections::BTreeSet::new();
        for &(k, l) in SCRAMBLERS {
            assert!(
                k.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{k}"
            );
            assert!(seen.insert(k), "{k} twice");
            assert_eq!(scrambler(k), Some(l));
        }
        assert_eq!(scrambler("ccsds_randomiser"), Some(CCSDS_RANDOMISER));
        assert_eq!(scrambler("prbs9_dispersal"), Some(PRBS9_DISPERSAL));
        assert_eq!(scrambler("custom"), None);
    }

    #[test]
    fn pn9_whitening_matches_dn509() {
        assert_eq!(
            bytes(PN9_WHITENING, 8, BitOrder::Lsb),
            [0xFF, 0xE1, 0x1D, 0x9A, 0xED, 0x85, 0x33, 0x24]
        );
        // The 802.15.4g form is the same sequence 9 bits on.
        assert_eq!(bits(pn9_802154g(), 50), bits(PN9_WHITENING, 59)[9..]);
    }

    #[test]
    fn right_shift_registers_as_their_documents_draw_them() {
        // Registers drawn shifting right, output at bit 0, feedback into the top stage: dPMR, NXDN and CC1101 PN9.
        fn right(seed: u16, tap: u32, n: usize) -> Vec<u8> {
            let mut s = seed;
            (0..n)
                .map(|_| {
                    let o = (s & 1) as u8;
                    let fb = (s ^ (s >> tap)) & 1;
                    s = (s >> 1) | (fb << 8);
                    o
                })
                .collect()
        }
        assert_eq!(bits(DPMR, 600), right(0x100, 5, 600));
        assert_eq!(bits(NXDN, 600), right(0xE4, 4, 600));
        assert_eq!(bits(PN9_WHITENING, 600), right(0x1FF, 5, 600));
    }

    #[test]
    fn dtmb_headers_equal_the_worked_vectors() {
        // The headers at phase 0, most significant bit first; the last digit is padded with zeros.
        fn hex(l: Lfsr, n: usize) -> String {
            bits(l, n)
                .chunks(4)
                .map(|c| {
                    let v = c
                        .iter()
                        .enumerate()
                        .fold(0u32, |a, (i, &b)| a | (u32::from(b) << (3 - i)));
                    char::from_digit(v, 16).unwrap_or('?').to_ascii_uppercase()
                })
                .collect()
        }
        assert_eq!(
            hex(DTMB_PN420, 420),
            "B0A5E9FEA1CF0D9A3DC7407C4A22D5C8C938109BCCEFCB2B6906358AA60BAFB7614BD3FD439E1B347B8E80F89445AB91927021379"
        );
        assert_eq!(
            hex(DTMB_PN595, 595),
            "80249A6BE63E477E1C07FE389D95DEA3D2A0BFD55E8748CB59EB19F954CCA7D3846C8A6F75733BB9D4E83DB86252CD116974C581497DE31BB0F272C437F38D4A1096FAE2E43ED517B39F0"
        );
        assert_eq!(
            hex(DTMB_PN945, 945),
            "FB946DFF3259AD7E9A7C9CDC081A82531A292661E8F1233A432F2D8055BABDE0EBA16959060BE1BD4B9EDAA88E44D953B15C5DA03FB3F1884F38EEFAC28708B1F728DBFE64B35AFD34F939B8103504A634524CC3D1E24674865E5B00AB757BC1D742D2B20C17C37A973DB5511C89B2A762B8BB407F678"
        );
        // The first eight bits of a frame whose header starts `phase` bits into the period.
        let first = |l: Lfsr, phase: usize| {
            bits(l, phase + 8)[phase..]
                .iter()
                .fold(0u8, |a, &b| (a << 1) | b)
        };
        for (phase, want) in [
            (0, 0xB0),
            (1, 0x61),
            (254, 0xD8),
            (2, 0xC2),
            (56, 0x9A),
            (199, 0x83),
        ] {
            assert_eq!(first(DTMB_PN420, phase), want, "PN420 phase {phase}");
        }
        for (phase, want) in [
            (0, 0xFB),
            (1, 0xF7),
            (510, 0x7D),
            (2, 0xEE),
            (50, 0xB5),
            (461, 0x1D),
        ] {
            assert_eq!(first(DTMB_PN945, phase), want, "PN945 phase {phase}");
        }
        assert_eq!(DTMB_PN420.period(1000), Some(255));
        assert_eq!(DTMB_PN945.period(1000), Some(511));
        assert_eq!(DTMB_PN595.period(2000), Some(1023));
    }

    #[test]
    fn ieee80211_sequence_and_recovery() {
        // IEEE 802.11-2020 clause 17.3.5.5: the 127-bit sequence from all ones.
        let want = "0000111011110010110010010000001000100110001011101011011000001100110101001110011110110100001010101111101001010001101110001111111";
        let got: String = bits(ieee80211(0x7F), 127)
            .iter()
            .map(|b| (b'0' + b) as char)
            .collect();
        assert_eq!(got, want);
        let mut l = ieee80211(0x5D);
        let first = bits(l, 7);
        l.skip(7);
        assert_eq!(ieee80211_from_outputs(&first).unwrap(), l);
        assert!(ieee80211_from_outputs(&first[..6]).is_err());
        assert_eq!(bits(DSTAR, 127), bits(ieee80211(0x7F), 127));
    }

    #[test]
    fn ble_whitening_follows_the_figure() {
        // Bluetooth Core Vol 6 Part B figure 3.5: positions 0..6, position 0 = 1, positions 1..6 = channel (MSB first); each
        // step outputs position 6, which goes into position 0 and is XORed into position 4.
        for ch in [0u8, 1, 17, 37, 39] {
            let mut p = [0u8; 7];
            p[0] = 1;
            for (i, x) in p.iter_mut().enumerate().skip(1) {
                *x = (ch >> (6 - i)) & 1;
            }
            let mut want = Vec::new();
            for _ in 0..100 {
                let out = p[6];
                p = [out, p[0], p[1], p[2], p[3] ^ out, p[4], p[5]];
                want.push(out);
            }
            assert_eq!(bits(ble_whitening(ch), 100), want, "channel {ch}");
        }
    }

    #[test]
    fn fifteen_stage_sequences_as_their_documents_draw_them() {
        fn left(seed: u16, a: u32, b: u32, n: usize) -> Vec<u8> {
            let mut s = seed;
            (0..n)
                .map(|_| {
                    let f = ((s >> a) ^ (s >> b)) & 1;
                    s = ((s << 1) | f) & 0x7FFF;
                    f as u8
                })
                .collect()
        }
        assert_eq!(bits(VDL2, 500), left(0x4D4B, 14, 0, 500));
        assert_eq!(bits(MAC_PRBS15, 500), left(0x7FFF, 14, 0, 500));
        assert_eq!(bits(SVISSR, 500), left(0b011_0011_1001_1111, 14, 13, 500));
        // The Aero register: stages 1..15 loaded 1 1 0 1 0 0 1 0 1 0 1 1 0 0 1, new bit = stage 1 XOR stage 15.
        let init = [1u8, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1];
        let mut s = init;
        let want: Vec<u8> = (0..500)
            .map(|_| {
                let b = s[0] ^ s[14];
                s.rotate_right(1);
                s[0] = b;
                b
            })
            .collect();
        assert_eq!(bits(AERO, 500), want);
    }

    #[test]
    fn ccsds_and_lora_tables() {
        // CCSDS 131.0-B annex: FF 48 0E C0 9A 0D 70 BC ...
        assert_eq!(
            &CCSDS.bytes()[..8],
            &[0xFF, 0x48, 0x0E, 0xC0, 0x9A, 0x0D, 0x70, 0xBC]
        );
        assert_eq!(
            &LORA_WHITENING.bytes()[..8],
            &[0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE1, 0xC2, 0x85]
        );
        let mut d = [0x5Au8; 300];
        CCSDS.apply(&mut d, 0);
        CCSDS.apply(&mut d, 0);
        assert_eq!(d, [0x5A; 300]);
    }

    #[test]
    fn inmarsat_c_register() {
        // a(n + 8) = a(n) + a(n + 2) + a(n + 3) + a(n + 4), the first 8 values being the start register read from bit 0.
        let mut a: Vec<u8> = (0..8).map(|i| ((0x80u32 >> i) & 1) as u8).collect();
        for n in 0..152 {
            let v = a[n] ^ a[n + 2] ^ a[n + 3] ^ a[n + 4];
            a.push(v);
        }
        assert_eq!(bits(INMARSAT_C, 160), a);
    }

    #[test]
    fn hd_cdr_and_dvbt_pilots() {
        assert_eq!(HD_RADIO.period(5000), Some(2047));
        // EN 300 744: w_k from all ones; w_k = w_(k-11) XOR w_(k-9).
        let mut w = vec![1u8; 11];
        for k in 11..300 {
            let v = w[k - 11] ^ w[k - 9];
            w.push(v);
        }
        assert_eq!(bits(DVBT_PILOTS, 300), w);
        // The HD Radio sequence is the same register with the feedback as output: w shifted by 11.
        assert_eq!(bits(HD_RADIO, 289), w[11..]);
        assert_eq!(CDR.period(5000), Some(4095));
    }

    #[test]
    fn dvbc2_pilot_prime() {
        // EN 302 769 figure 32: w'_i from all ones; w'_i = w'_(i-10) XOR w'_(i-7).
        let mut w = vec![1u8; 10];
        for i in 10..1100 {
            let v = w[i - 10] ^ w[i - 7];
            w.push(v);
        }
        assert_eq!(bits(DVBC2_PILOT_PRIME, 1100), w);
        assert_eq!(DVBC2_PILOT_PRIME.period(5000), Some(1023));
        let first: String = w[..32].iter().map(|b| char::from(b'0' + b)).collect();
        assert_eq!(first, "11111111110000000111000011111101");
    }

    #[test]
    fn atsc_sequences() {
        // ATSC A/53 Part 2: PN511 begins 0000 0001 0111; randomiser bytes C0 6D 3F 99 38 6A 29 52.
        assert_eq!(
            &bits(ATSC_PN511, 12)[..],
            &[0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1]
        );
        assert_eq!(ATSC_PN511.period(1000), Some(511));
        assert_eq!(ATSC_PN63.period(100), Some(63));
        let mut r = [0u8; 8];
        atsc_randomiser(&mut r);
        assert_eq!(r, [0xC0, 0x6D, 0x3F, 0x99, 0x38, 0x6A, 0x29, 0x52]);
        // A/321: right-shifting register r0..r15, feedback r0 ^ r1 ^ r2 ^ r15 into r15, output r0.
        let seed = 0x019Du16;
        let mut s = seed;
        let want: Vec<u8> = (0..300)
            .map(|_| {
                let o = (s & 1) as u8;
                let fb = (s ^ (s >> 1) ^ (s >> 2) ^ (s >> 15)) & 1;
                s = (s >> 1) | (fb << 15);
                o
            })
            .collect();
        assert_eq!(bits(atsc3_bootstrap(seed), 300), want);
    }

    #[test]
    fn tetra_follows_clause_8_2_5() {
        // p(k) = sum_{j=1..32} c_j p(k - j) with the CRC-32 polynomial's coefficients; p(-31) = p(-30) = 1; p(k) = e(1 - k).
        const C: [u32; 14] = [1, 2, 4, 5, 7, 8, 10, 11, 12, 16, 22, 23, 26, 32];
        for colour in [0u32, 1, 0x2AAA_AAAA, 0x3FFF_FFFF, 0x1234_5678] {
            let mut p = std::collections::HashMap::new();
            p.insert(-31i32, 1u8);
            p.insert(-30, 1);
            for k in -29..=0 {
                let e_index = 1 - k; // 1..=30
                p.insert(k, ((colour >> (30 - e_index)) & 1) as u8);
            }
            let mut want = Vec::new();
            for k in 1..=200 {
                let v = C.iter().fold(0u8, |a, &j| a ^ p[&(k - j as i32)]);
                p.insert(k, v);
                want.push(v);
            }
            assert_eq!(bits(tetra(colour), 200), want, "colour {colour:#x}");
        }
    }

    #[test]
    fn sarsat_t018_first_chips() {
        let hex = |l: Lfsr| {
            let mut s = String::new();
            crate::pack::bits_to_hex(&bits(l, 64), &mut s);
            s
        };
        assert_eq!(hex(sarsat_t018(false)), "80000108421284A1");
        assert_eq!(hex(sarsat_t018(true)), "3F8358BAD030F231");
    }

    #[test]
    fn gold_3gpp_matches_the_definition() {
        for c_init in [0u32, 1, 301, 0x123_4567, 0x7FFF_FFFF] {
            let n = 300;
            let mut x1 = vec![0u8; n + 1600 + 31];
            let mut x2 = vec![0u8; n + 1600 + 31];
            x1[0] = 1;
            for (i, x) in x2.iter_mut().enumerate().take(31) {
                *x = ((c_init >> i) & 1) as u8;
            }
            for i in 0..n + 1600 {
                x1[i + 31] = x1[i + 3] ^ x1[i];
                x2[i + 31] = x2[i + 3] ^ x2[i + 2] ^ x2[i + 1] ^ x2[i];
            }
            let want: Vec<u8> = (0..n).map(|i| x1[i + 1600] ^ x2[i + 1600]).collect();
            let mut got = vec![0u8; n];
            gold_3gpp(c_init, &mut got);
            assert_eq!(got, want, "c_init {c_init}");
        }
    }

    #[test]
    fn dvbs2_gold_matches_the_definition() {
        let m = DVBS2_M;
        let mut x = vec![0u8; m + 18];
        let mut y = vec![1u8; m + 18];
        x[0] = 1;
        for v in x.iter_mut().take(18).skip(1) {
            *v = 0;
        }
        for i in 0..m {
            x[i + 18] = x[i + 7] ^ x[i];
            y[i + 18] = y[i + 10] ^ y[i + 7] ^ y[i + 5] ^ y[i];
        }
        for n in [0u32, 1, 12345, 262_140] {
            let z = |i: usize| x[(i + n as usize) % m] ^ y[i];
            let want: Vec<u8> = (0..500).map(|i| 2 * z((i + 131_072) % m) + z(i)).collect();
            let mut got = vec![0u8; 500];
            gold_dvbs2(n, 0, &mut got).unwrap();
            assert_eq!(got, want, "n {n}");
            let mut late = vec![0u8; 10];
            gold_dvbs2(n, 490, &mut late).unwrap();
            assert_eq!(late, want[490..]);
        }
        assert!(gold_dvbs2(262_142, 0, &mut [0u8; 1]).is_err());
    }

    /// First ten chips in octal of SBAS PRN 120 to 158 (RTCA DO-229 appendix A table A-1). The same values, and the G2 delays,
    /// are in the L1 C/A PRN code assignments of GPS.gov (January 2026 edition).
    const SBAS_FIRST10: [(u16, u16); 39] = [
        (120, 0o0671),
        (121, 0o0536),
        (122, 0o1510),
        (123, 0o1545),
        (124, 0o0160),
        (125, 0o0701),
        (126, 0o0013),
        (127, 0o1060),
        (128, 0o0245),
        (129, 0o0527),
        (130, 0o1436),
        (131, 0o1226),
        (132, 0o1257),
        (133, 0o0046),
        (134, 0o1071),
        (135, 0o0561),
        (136, 0o1037),
        (137, 0o0770),
        (138, 0o1327),
        (139, 0o1472),
        (140, 0o0124),
        (141, 0o0366),
        (142, 0o0133),
        (143, 0o0465),
        (144, 0o0717),
        (145, 0o0217),
        (146, 0o1742),
        (147, 0o1422),
        (148, 0o1442),
        (149, 0o0523),
        (150, 0o0736),
        (151, 0o1635),
        (152, 0o0136),
        (153, 0o0273),
        (154, 0o1026),
        (155, 0o0003),
        (156, 0o1670),
        (157, 0o0624),
        (158, 0o0235),
    ];

    /// First ten chips in octal of QZSS PRN 193 to 202 (IS-QZSS-PNT table 3.2.2-1). The same values, and the G2 delays, are in
    /// the L1 C/A PRN code assignments of GPS.gov (January 2026 edition).
    const QZSS_FIRST10: [(u16, u16); 10] = [
        (193, 0o0727),
        (194, 0o0170),
        (195, 0o0030),
        (196, 0o0472),
        (197, 0o1237),
        (198, 0o0414),
        (199, 0o1050),
        (200, 0o1630),
        (201, 0o0571),
        (202, 0o0732),
    ];

    #[test]
    fn ca_by_delay_matches_the_tap_pairs_and_the_printed_first_chips() {
        let (mut a, mut b) = ([0u8; 1023], [0u8; 1023]);
        for prn in 1..=32u8 {
            gps_ca(prn, &mut a).unwrap();
            ca_by_delay(GPS_CA_DELAYS[prn as usize - 1], &mut b).unwrap();
            assert_eq!(a, b, "PRN {prn}");
            l1_ca(prn as u16, &mut b).unwrap();
            assert_eq!(a, b, "PRN {prn}");
        }
        // First ten chips in octal: SBAS from RTCA DO-229 table A-1, QZSS from IS-QZSS-PNT table 3.2.2-1.
        for (prn, oct) in SBAS_FIRST10 {
            l1_ca(prn, &mut b).unwrap();
            assert_eq!(to_u64(&b[..10]) as u16, oct, "PRN {prn}");
            assert_eq!(b.iter().filter(|&&x| x == 1).count(), 512, "PRN {prn}");
        }
        for (prn, oct) in QZSS_FIRST10 {
            l1_ca(prn, &mut b).unwrap();
            assert_eq!(to_u64(&b[..10]) as u16, oct, "PRN {prn}");
        }
        assert!(ca_by_delay(1023, &mut b).is_err());
        for prn in [0u16, 33, 119, 159, 192, 203] {
            assert!(l1_ca(prn, &mut b).is_err() && l1_ca_delay(prn).is_none());
        }
    }

    #[test]
    fn zadoff_chu_follows_its_defining_equation() {
        // TS 36.211 equation 6.11.1.1-1: d_u(n) = exp(-j pi u n (n + 1) / 63), roots 25, 29 and 34.
        for u in [25u32, 29, 34] {
            let zc = ZadoffChu::new(u, 63).unwrap();
            for n in 0..63u64 {
                let want = -std::f64::consts::PI * (u as f64) * (n * (n + 1)) as f64 / 63.0;
                let [re, im] = zc.value(n);
                assert!((re - want.cos()).abs() < 1e-12 && (im - want.sin()).abs() < 1e-12);
            }
        }
        // Even length: exp(-j pi u n^2 / N).
        let zc = ZadoffChu::new(3, 10).unwrap();
        assert_eq!(zc.phase_index(4), 3 * 16 % 20);
        // Constant amplitude, zero cyclic autocorrelation at every shift (prime and odd lengths).
        for (u, len) in [(5u32, 11u32), (129, 1499), (147, 601), (1, 839)] {
            let zc = ZadoffChu::new(u, len).unwrap();
            let x: Vec<[f64; 2]> = (0..len as u64).map(|n| zc.value(n)).collect();
            for shift in [1usize, 2, len as usize / 2] {
                let (mut re, mut im) = (0.0, 0.0);
                for n in 0..len as usize {
                    let (a, b) = (x[n], x[(n + shift) % len as usize]);
                    re += a[0] * b[0] + a[1] * b[1];
                    im += a[1] * b[0] - a[0] * b[1];
                }
                assert!(
                    re.hypot(im) < 1e-6 * len as f64,
                    "u {u} N {len} shift {shift}"
                );
            }
        }
        // Large lengths keep an exact phase.
        let zc = ZadoffChu::new(4_000_000_001, 4_000_000_003).unwrap();
        assert!(zc.phase_index(3_999_999_999) < 8_000_000_006);
        let mut out = [[0f32; 2]; 3];
        ZadoffChu::new(25, 63).unwrap().fill(62, &mut out);
        assert_eq!(out[1], [1.0, 0.0]);
        for (u, len) in [(0u32, 63u32), (63, 63), (3, 63), (2, 0), (1, 1)] {
            assert!(ZadoffChu::new(u, len).is_err(), "u {u} N {len}");
        }
    }

    #[test]
    fn gps_ca_first_ten_chips() {
        // IS-GPS-200 table 3-Ia, first 10 chips in octal.
        const OCT: [u16; 32] = [
            0o1440, 0o1620, 0o1710, 0o1744, 0o1133, 0o1455, 0o1131, 0o1454, 0o1626, 0o1504, 0o1642,
            0o1750, 0o1764, 0o1772, 0o1775, 0o1776, 0o1156, 0o1467, 0o1633, 0o1715, 0o1746, 0o1763,
            0o1063, 0o1706, 0o1743, 0o1761, 0o1770, 0o1774, 0o1127, 0o1453, 0o1625, 0o1712,
        ];
        let mut c = [0u8; 1023];
        for prn in 1..=32u8 {
            gps_ca(prn, &mut c).unwrap();
            assert_eq!(to_u64(&c[..10]) as u16, OCT[prn as usize - 1], "PRN {prn}");
            assert_eq!(c.iter().filter(|&&b| b == 1).count(), 512);
        }
        assert!(gps_ca(0, &mut c).is_err() && gps_ca(33, &mut c).is_err());
    }

    #[test]
    fn beidou_and_glonass_codes() {
        let pm = |c: &[u8]| -> Vec<i32> { c.iter().map(|&b| 1 - 2 * b as i32).collect() };
        let mut a = [0u8; 2046];
        let mut b = [0u8; 2046];
        bds_b1i(1, &mut a).unwrap();
        bds_b1i(2, &mut b).unwrap();
        // Gold codes of length 2047: cross-correlation in {-65, -1, 63} over the full period; the 2046-chip codes stay close.
        let (pa, pb) = (pm(&a), pm(&b));
        let peak = (0..2046)
            .map(|s| {
                (0..2046)
                    .map(|i| pa[i] * pb[(i + s) % 2046])
                    .sum::<i32>()
                    .abs()
            })
            .max()
            .unwrap();
        assert!(peak < 200, "{peak}");
        assert!(bds_b1i(38, &mut a).is_err());
        // G1 and G2 structure: the ICD registers, stage 1 newest, loaded 01010101010.
        let mut g1 = [0u8, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0];
        let mut g2 = g1;
        let mut want = Vec::new();
        for _ in 0..2046 {
            want.push(g1[10] ^ g2[0] ^ g2[2]);
            let f1 = g1[0] ^ g1[6] ^ g1[7] ^ g1[8] ^ g1[9] ^ g1[10];
            let f2 = g2[0] ^ g2[1] ^ g2[2] ^ g2[3] ^ g2[4] ^ g2[7] ^ g2[8] ^ g2[10];
            g1.rotate_right(1);
            g1[0] = f1;
            g2.rotate_right(1);
            g2[0] = f2;
        }
        bds_b1i(1, &mut a).unwrap();
        assert_eq!(a.to_vec(), want);
        // GLONASS: starts 111111100, period 511.
        assert_eq!(bits(GLONASS_RANGING, 9), [1, 1, 1, 1, 1, 1, 1, 0, 0]);
        assert_eq!(GLONASS_RANGING.period(1000), Some(511));
    }

    #[test]
    fn three_gpp_m_sequences() {
        for (l, k, n) in [
            (LTE_SSS_S, vec![2usize, 0], 5usize),
            (LTE_SSS_C, vec![3, 0], 5),
            (LTE_SSS_Z, vec![4, 2, 1, 0], 5),
            (NR_PSS, vec![4, 0], 7),
            (NR_SSS_X0, vec![4, 0], 7),
            (NR_SSS_X1, vec![1, 0], 7),
        ] {
            let first = bits(l, n);
            let mut x = first.clone();
            for i in 0..100 {
                let v = k.iter().fold(0u8, |a, &j| a ^ x[i + j]);
                x.push(v);
            }
            assert_eq!(bits(l, n + 100), x);
            assert_eq!(l.period(200), Some((1 << n) - 1));
        }
        assert_eq!(bits(NR_PSS, 7), [0, 1, 1, 0, 1, 1, 1]);
        assert_eq!(bits(LTE_SSS_S, 5), [0, 0, 0, 0, 1]);
    }

    #[test]
    fn self_synchronising_named() {
        let data: Vec<u8> = (0..200).map(|i| (i % 3 == 0) as u8).collect();
        for (mut tx, mut rx) in [
            (ieee80211b(0x1B), ieee80211b(0)),
            (g3ruh(0), g3ruh(0x1_2345)),
            (eurobalise(0xDEAD_BEEF), eurobalise(0)),
        ] {
            let mut l = data.clone();
            tx.scramble(&mut l);
            rx.descramble(&mut l);
            assert_eq!(&l[40..], &data[40..]);
        }
    }

    #[test]
    fn fixed_tables() {
        assert_eq!(DVBS2_PLS_SCRAMBLE >> 60, 0b0111);
        assert_eq!(M17_RANDOMISER.bytes()[0], 0xD6);
        assert_eq!(FT4_RVEC.len(), 77);
        assert_eq!(RS41_WHITENING.bytes().len(), 64);
        assert_eq!(LORAN_MASTER[0].iter().map(|&x| x as i32).sum::<i32>(), 1);
        assert_eq!(ECZAS_SCRAMBLE.len(), 5);
    }
}
