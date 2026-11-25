//! The three LE PHYs and the bits on the air of a packet on each (Bluetooth Core Specification v5.4 Vol 6 Part B clauses 2.1,
//! 2.2 and 3.3): the uncoded packet of the LE 1M and LE 2M PHYs, and the coded packet of the LE Coded PHY (preamble, FEC block 1
//! with the access address, CI and TERM1 at S=8, FEC block 2 with the PDU, CRC and TERM2 at S=2 or S=8).
//!
//! A [`Link`] is what a receiver listens for: an access address and the CRC preset that goes with it (the advertising pair,
//! a connection's pair from CONNECT_IND, a periodic train's pair from SyncInfo, a BIS's pair from BIGInfo).

use super::{crc24, preamble, ADV_ACCESS_ADDRESS, ADV_CRC_INIT, MAX_PAYLOAD};
use crate::{Error, Param, Result};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::{pack, seq, BitOrder};

/// An LE PHY (Vol 6 Part A clause 1, Part B clause 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phy {
    /// LE 1M: 1 Msym/s, uncoded.
    Le1M,
    /// LE 2M: 2 Msym/s, uncoded.
    Le2M,
    /// LE Coded: 1 Msym/s, FEC block 2 at S=2 or S=8.
    LeCoded,
}

impl Phy {
    /// The PHY of a 3-bit PHY code of AuxPtr, SyncInfo, BIGInfo and LL_PHY_UPDATE_IND bit fields read as a number (0 LE 1M,
    /// 1 LE 2M, 2 LE Coded; Vol 6 Part B clause 2.3.4.5). `None` for a reserved code.
    pub fn from_code(code: u8) -> Option<Phy> {
        match code {
            0 => Some(Phy::Le1M),
            1 => Some(Phy::Le2M),
            2 => Some(Phy::LeCoded),
            _ => None,
        }
    }

    /// The PHY of a one-bit-per-PHY mask (bit 0 LE 1M, bit 1 LE 2M, bit 2 LE Coded; LL_PHY_UPDATE_IND, BIGInfo PHY is a code).
    /// The lowest set bit wins; `None` for no bit.
    pub fn from_mask(mask: u8) -> Option<Phy> {
        [Phy::Le1M, Phy::Le2M, Phy::LeCoded]
            .into_iter()
            .enumerate()
            .find(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, p)| p)
    }

    /// The symbol rate, symbols per second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            Phy::Le2M => 2e6,
            Phy::Le1M | Phy::LeCoded => 1e6,
        }
    }

    /// The nominal peak deviation (modulation index 0.5), Hz.
    pub fn deviation_hz(self) -> f64 {
        0.25 * self.symbol_rate()
    }

    /// The name: "LE 1M", "LE 2M" or "LE Coded".
    pub fn name(self) -> &'static str {
        match self {
            Phy::Le1M => "LE 1M",
            Phy::Le2M => "LE 2M",
            Phy::LeCoded => "LE Coded",
        }
    }
}

/// The coding of FEC block 2 on the LE Coded PHY (Vol 6 Part B clause 2.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Coding {
    /// S=2: 500 kbit/s.
    S2,
    /// S=8: 125 kbit/s.
    S8,
}

impl Coding {
    /// The CI value (table 2.2): 0b00 S=8, 0b01 S=2. `None` for a reserved value.
    pub fn from_ci(ci: u8) -> Option<Coding> {
        match ci {
            0 => Some(Coding::S8),
            1 => Some(Coding::S2),
            _ => None,
        }
    }

    /// The CI value.
    pub fn ci(self) -> u8 {
        match self {
            Coding::S8 => 0,
            Coding::S2 => 1,
        }
    }

    /// Symbols per coded bit (the pattern mapper's P: 1 for S=2, 4 for S=8).
    pub fn pattern(self) -> usize {
        match self {
            Coding::S2 => 1,
            Coding::S8 => 4,
        }
    }

    /// Symbols per information bit (S).
    pub fn spread(self) -> usize {
        2 * self.pattern()
    }

    /// The name: "S=2" or "S=8".
    pub fn name(self) -> &'static str {
        match self {
            Coding::S2 => "S=2",
            Coding::S8 => "S=8",
        }
    }
}

/// The physical channel a link belongs to: it sets which PDU format the receiver expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkKind {
    /// The primary and secondary advertising physical channels: advertising physical channel PDUs.
    Advertising,
    /// A periodic advertising train: advertising physical channel PDUs (AUX_SYNC_IND, AUX_CHAIN_IND).
    Periodic,
    /// A connection (the data physical channel): data physical channel PDUs.
    Data,
    /// A broadcast isochronous stream or its control subevent: isochronous physical channel PDUs.
    Isochronous,
}

/// An access address, its CRC preset and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Link {
    /// The access address.
    pub access_address: u32,
    /// The CRC preset (24 bits).
    pub crc_init: u32,
    /// The physical channel kind.
    pub kind: LinkKind,
}

impl Link {
    /// The advertising link: 0x8E89BED6 with the preset 0x555555.
    pub const ADVERTISING: Link = Link {
        access_address: ADV_ACCESS_ADDRESS,
        crc_init: ADV_CRC_INIT,
        kind: LinkKind::Advertising,
    };

    /// A link of `kind`.
    pub const fn new(access_address: u32, crc_init: u32, kind: LinkKind) -> Link {
        Link {
            access_address,
            crc_init: crc_init & 0xFF_FFFF,
            kind,
        }
    }
}

fn check(pdu: &[u8], channel: u8) -> Result<()> {
    if pdu.len() < 2 {
        return Err(Error::InputTooShort);
    }
    if pdu.len() > 2 + MAX_PAYLOAD {
        return Err(Error::TooLong);
    }
    if channel > 39 {
        return Err(Error::BadParameter(Param::BleChannelIndex));
    }
    Ok(())
}

/// Appends the whitened PDU and CRC bits of a packet on `channel`, least significant bit first.
fn pdu_crc_bits(pdu: &[u8], link: &Link, channel: u8, out: &mut Vec<u8>) {
    let crc = crc24(pdu, link.crc_init);
    let start = out.len();
    pack::unpack(pdu, BitOrder::Lsb, out);
    pack::push_u64_lsb(out, crc as u64, 24);
    Additive::new(seq::ble_whitening(channel)).apply_bits(&mut out[start..]);
}

/// An uncoded PHY.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uncoded {
    /// LE 1M.
    Le1M,
    /// LE 2M.
    Le2M,
}

impl Uncoded {
    /// The PHY.
    pub fn phy(self) -> Phy {
        match self {
            Uncoded::Le1M => Phy::Le1M,
            Uncoded::Le2M => Phy::Le2M,
        }
    }
}

/// Appends the bits on the air of a packet on an uncoded PHY (clause 2.1): the preamble (8 bits on LE 1M, 16 on LE 2M), the
/// access address, then the PDU and its CRC with `link`'s preset, whitened for `channel`. Errors for a PDU shorter than its
/// header or longer than 2 + [`MAX_PAYLOAD`] bytes, or a channel above 39.
pub fn packet_bits(
    pdu: &[u8],
    link: &Link,
    channel: u8,
    phy: Uncoded,
    out: &mut Vec<u8>,
) -> Result<()> {
    check(pdu, channel)?;
    let pre = preamble(link.access_address) as u64;
    match phy {
        Uncoded::Le1M => pack::push_u64_lsb(out, pre, 8),
        Uncoded::Le2M => pack::push_u64_lsb(out, pre | pre << 8, 16),
    }
    pack::push_u64_lsb(out, link.access_address as u64, 32);
    pdu_crc_bits(pdu, link, channel, out);
    Ok(())
}

/// The symbol pattern of the LE Coded preamble, in transmission order (clause 2.2.1): repeated 10 times.
pub const CODED_PREAMBLE: [u8; 8] = [0, 0, 1, 1, 1, 1, 0, 0];
/// Symbols of the LE Coded preamble.
pub const CODED_PREAMBLE_SYMBOLS: usize = 80;
/// Symbols of FEC block 1: (32 + 2 + 3) bits, rate 1/2, P = 4.
pub const CODED_BLOCK1_SYMBOLS: usize = 37 * 8;

/// The convolutional FEC encoder of clause 3.3.1 as the figure draws it: a three-stage register `d` (`d[0]` the newest input),
/// a0 from G0(x) = 1 + x + x^2 + x^3 sent first, a1 from G1(x) = 1 + x^2 + x^3 sent second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FecEncoder {
    d: [u8; 3],
}

impl FecEncoder {
    /// Appends a0 and a1 for each input bit.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<u8>) {
        for &b in bits {
            let b = b & 1;
            out.push(b ^ self.d[0] ^ self.d[1] ^ self.d[2]);
            out.push(b ^ self.d[1] ^ self.d[2]);
            self.d = [b, self.d[0], self.d[1]];
        }
    }

    /// The register is all zeros (after a termination sequence).
    pub fn is_zero(&self) -> bool {
        self.d == [0; 3]
    }
}

/// Appends the pattern mapper's output for coded bits (clause 3.3.2, table 3.1): P = 1 passes each bit, P = 4 sends 0 as 0011
/// and 1 as 1100.
pub fn pattern_map(coded: &[u8], coding: Coding, out: &mut Vec<u8>) {
    for &c in coded {
        match coding {
            Coding::S2 => out.push(c & 1),
            Coding::S8 if c & 1 == 0 => out.extend_from_slice(&[0, 0, 1, 1]),
            Coding::S8 => out.extend_from_slice(&[1, 1, 0, 0]),
        }
    }
}

/// Appends the coded symbols of FEC block 1 for `access_address` and `coding` (the access address and CI least significant bit
/// first, then TERM1), from an encoder at zero. Returns the encoder for FEC block 2.
pub fn coded_block1(access_address: u32, coding: Coding, out: &mut Vec<u8>) -> FecEncoder {
    let mut bits = Vec::with_capacity(37);
    pack::push_u64_lsb(&mut bits, access_address as u64, 32);
    pack::push_u64_lsb(&mut bits, coding.ci() as u64, 2);
    bits.extend_from_slice(&[0, 0, 0]);
    let mut enc = FecEncoder::default();
    let mut coded = Vec::with_capacity(74);
    enc.encode(&bits, &mut coded);
    pattern_map(&coded, Coding::S8, out);
    enc
}

/// Appends the symbols on the air of a packet on the LE Coded PHY (clause 2.2): the 80-symbol preamble, FEC block 1 at S=8 and
/// FEC block 2 (the whitened PDU and CRC, then TERM2) at `coding`. Errors as [`packet_bits`] does.
pub fn coded_symbols(
    pdu: &[u8],
    link: &Link,
    channel: u8,
    coding: Coding,
    out: &mut Vec<u8>,
) -> Result<()> {
    check(pdu, channel)?;
    for _ in 0..10 {
        out.extend_from_slice(&CODED_PREAMBLE);
    }
    let mut enc = coded_block1(link.access_address, coding, out);
    let mut bits = Vec::with_capacity(8 * (pdu.len() + 3) + 3);
    pdu_crc_bits(pdu, link, channel, &mut bits);
    bits.extend_from_slice(&[0, 0, 0]);
    let mut coded = Vec::with_capacity(2 * bits.len());
    enc.encode(&bits, &mut coded);
    pattern_map(&coded, coding, out);
    Ok(())
}

/// The duration of a packet on the air, s: the preamble to the last CRC (or TERM2) symbol.
pub fn airtime_s(payload_len: usize, phy: Phy, coding: Coding) -> f64 {
    let n = payload_len + 2;
    match phy {
        Phy::Le1M => (8 + 32 + 8 * (n + 3)) as f64 * 1e-6,
        Phy::Le2M => (16 + 32 + 8 * (n + 3)) as f64 * 0.5e-6,
        Phy::LeCoded => {
            (CODED_PREAMBLE_SYMBOLS + CODED_BLOCK1_SYMBOLS + (8 * (n + 3) + 3) * coding.spread())
                as f64
                * 1e-6
        }
    }
}
