//! The reader-to-tag commands of EPC Gen2 (GS1, EPC Radio-Frequency Identity Protocols Generation-2 UHF RFID, version 2.0.1,
//! section 6.3.2.12, Tables 6.28 to 6.51): Query, QueryRep, QueryAdjust, ACK, NAK, Select, Req_RN, Read, Write, Kill, Lock and
//! Access, with the CRC-5 of Annex F.1 and the CRC-16 of Annex F.2 and the extensible bit vectors of Annex A. Bits are sent
//! most significant first. A command is decoded only when its length matches its fields and its CRC holds; the cover-coded
//! data of Write and the password halves of Kill and Access are never kept.

use crate::typed::Gen2Command;
use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC5_EPC_C1G2};

/// The most blocks of an extensible bit vector that are read.
pub const EBV_MAX_BLOCKS: usize = 3;

/// The bits of `v`, `n` of them, most significant first.
pub fn bits_of(v: u64, n: usize) -> Vec<u8> {
    (0..n).rev().map(|k| ((v >> k) & 1) as u8).collect()
}

/// The value of `b`, most significant bit first.
pub fn value_of(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, &x| (a << 1) | u64::from(x))
}

/// The extensible bit vector of `v` (Annex A): blocks of 8 bits, the first bit 1 when another block follows.
pub fn ebv(v: u64) -> Vec<u8> {
    let mut groups = vec![v & 0x7F];
    let mut rest = v >> 7;
    while rest > 0 {
        groups.push(rest & 0x7F);
        rest >>= 7;
    }
    let mut out = Vec::new();
    for (i, g) in groups.iter().rev().enumerate() {
        out.push(u8::from(i + 1 < groups.len()));
        out.extend(bits_of(*g, 7));
    }
    out
}

/// The value and the length in bits of the extensible bit vector at the start of `b`; `None` when it runs past `b` or past
/// [`EBV_MAX_BLOCKS`].
pub fn read_ebv(b: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for k in 0..EBV_MAX_BLOCKS {
        let block = b.get(8 * k..8 * k + 8)?;
        v = (v << 7) | value_of(&block[1..]);
        if block[0] == 0 {
            return Some((v, 8 * (k + 1)));
        }
    }
    None
}

/// Appends the CRC-5 of `b` (Annex F.1).
pub fn with_crc5(mut b: Vec<u8>) -> Vec<u8> {
    let c = CRC5_EPC_C1G2.bits(&b);
    b.extend(bits_of(u64::from(c), 5));
    b
}

/// Appends the CRC-16 of `b` (Annex F.2: the ones complement of the register).
pub fn with_crc16(mut b: Vec<u8>) -> Vec<u8> {
    let c = CRC16_GENIBUS.bits(&b);
    b.extend(bits_of(u64::from(c), 16));
    b
}

/// The fields of a decoded command. The data of Write and the password halves of Kill and Access are left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    /// The command.
    pub command: Option<Gen2Command>,
    /// The session, 0 to 3.
    pub session: Option<i64>,
    /// Q of a Query.
    pub q: Option<i64>,
    /// The target of a Query ("A", "B") or of a Select ("S0" to "S3", "SL", "RFU").
    pub target: Option<&'static str>,
    /// The Sel field of a Query.
    pub sel: Option<&'static str>,
    /// The tag data coding of a Query.
    pub coding: Option<&'static str>,
    /// The pilot tone of a Query.
    pub pilot: Option<bool>,
    /// The divide ratio of a Query, "8" or "64/3".
    pub dr: Option<&'static str>,
    /// The UpDn of a QueryAdjust.
    pub updn: Option<&'static str>,
    /// The RN16 or handle, 4 hex digits.
    pub rn16: Option<String>,
    /// The memory bank.
    pub bank: Option<&'static str>,
    /// The pointer: a bit for Select, a word for Read and Write.
    pub pointer: Option<i64>,
    /// The words of a Read.
    pub word_count: Option<i64>,
    /// The mask of a Select, as bits.
    pub mask: Option<String>,
    /// The Truncate bit of a Select.
    pub truncate: Option<bool>,
    /// The action of a Select.
    pub action: Option<i64>,
    /// The mask and action payload of a Lock, 5 hex digits.
    pub lock_payload: Option<String>,
}

/// Why a frame is not a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The length or the code fits no command.
    Layout,
    /// The CRC fails.
    Crc,
}

fn rn(b: &[u8]) -> Option<String> {
    Some(format!("{:04X}", value_of(b)))
}

fn select_bank(v: u64) -> &'static str {
    ["FileType", "EPC", "TID", "File_0"][v as usize & 3]
}

fn access_bank(v: u64) -> &'static str {
    ["Reserved", "EPC", "TID", "User"][v as usize & 3]
}

fn crc16(b: &[u8]) -> Result<(), Refused> {
    if CRC16_GENIBUS.verify_bits(b) {
        Ok(())
    } else {
        Err(Refused::Crc)
    }
}

/// The command of the bits `b` of a frame; `trcal` says whether the frame began with a preamble (a TRcal symbol).
///
/// # Errors
///
/// [`Refused::Layout`] when the code and the length fit no command (a Query without a preamble too), [`Refused::Crc`] when
/// its CRC fails.
pub fn decode(b: &[u8], trcal: bool) -> Result<Decoded, Refused> {
    let n = b.len();
    let mut d = Decoded::default();
    let code = |k: usize| b.get(..k).map(value_of);
    if trcal {
        if code(4) != Some(0b1000) || n != 22 {
            return Err(Refused::Layout);
        }
        if !CRC5_EPC_C1G2.verify_bits(b) {
            return Err(Refused::Crc);
        }
        d.command = Some(Gen2Command::Query);
        d.dr = Some(if b[4] == 0 { "8" } else { "64/3" });
        d.coding = Some(["FM0", "Miller 2", "Miller 4", "Miller 8"][value_of(&b[5..7]) as usize]);
        d.pilot = Some(b[7] == 1);
        d.sel = Some(["all", "all", "~SL", "SL"][value_of(&b[8..10]) as usize]);
        d.session = Some(value_of(&b[10..12]) as i64);
        d.target = Some(if b[12] == 0 { "A" } else { "B" });
        d.q = Some(value_of(&b[13..17]) as i64);
        return Ok(d);
    }
    match (code(2), n) {
        (Some(0b00), 4) => {
            d.command = Some(Gen2Command::QueryRep);
            d.session = Some(value_of(&b[2..4]) as i64);
            return Ok(d);
        }
        (Some(0b01), 18) => {
            d.command = Some(Gen2Command::Ack);
            d.rn16 = rn(&b[2..18]);
            return Ok(d);
        }
        _ => {}
    }
    match code(4) {
        Some(0b1001) if n == 9 => {
            d.command = Some(Gen2Command::QueryAdjust);
            d.session = Some(value_of(&b[4..6]) as i64);
            d.updn = Some(match value_of(&b[6..9]) {
                0b110 => "+1",
                0b011 => "-1",
                0b000 => "unchanged",
                _ => "reserved",
            });
            return Ok(d);
        }
        Some(0b1010) if n >= 45 => {
            let (ptr, len) = read_ebv(&b[12..]).ok_or(Refused::Layout)?;
            let at = 12 + len;
            let mask_len = value_of(b.get(at..at + 8).ok_or(Refused::Layout)?) as usize;
            if n != at + 8 + mask_len + 1 + 16 {
                return Err(Refused::Layout);
            }
            crc16(b)?;
            let t = value_of(&b[4..7]) as usize;
            d.command = Some(Gen2Command::Select);
            d.target = Some(["S0", "S1", "S2", "S3", "SL", "RFU", "RFU", "RFU"][t]);
            d.action = Some(value_of(&b[7..10]) as i64);
            d.bank = Some(select_bank(value_of(&b[10..12])));
            d.pointer = Some(ptr as i64);
            let m = &b[at + 8..at + 8 + mask_len];
            d.mask = Some(m.iter().take(96).map(|&x| char::from(b'0' + x)).collect());
            d.truncate = Some(b[at + 8 + mask_len] == 1);
            return Ok(d);
        }
        Some(0b1000) => return Err(Refused::Layout),
        _ => {}
    }
    let c = code(8).ok_or(Refused::Layout)?;
    match (c, n) {
        (0b1100_0000, 8) => d.command = Some(Gen2Command::Nak),
        (0b1100_0001, 40) => {
            crc16(b)?;
            d.command = Some(Gen2Command::ReqRn);
            d.rn16 = rn(&b[8..24]);
        }
        (0b1100_0010, _) | (0b1100_0011, _) => {
            let (ptr, len) =
                read_ebv(b.get(10..).ok_or(Refused::Layout)?).ok_or(Refused::Layout)?;
            let read = c == 0b1100_0010;
            let body = if read { 8 } else { 16 };
            let at = 10 + len;
            if n != at + body + 16 + 16 {
                return Err(Refused::Layout);
            }
            crc16(b)?;
            d.bank = Some(access_bank(value_of(&b[8..10])));
            d.pointer = Some(ptr as i64);
            if read {
                d.command = Some(Gen2Command::Read);
                d.word_count = Some(value_of(&b[at..at + 8]) as i64);
            } else {
                d.command = Some(Gen2Command::Write);
            }
            d.rn16 = rn(&b[at + body..at + body + 16]);
        }
        (0b1100_0100, 59) => {
            crc16(b)?;
            d.command = Some(Gen2Command::Kill);
            d.rn16 = rn(&b[27..43]);
        }
        (0b1100_0101, 60) => {
            crc16(b)?;
            d.command = Some(Gen2Command::Lock);
            d.lock_payload = Some(format!("{:05X}", value_of(&b[8..28])));
            d.rn16 = rn(&b[28..44]);
        }
        (0b1100_0110, 56) => {
            crc16(b)?;
            d.command = Some(Gen2Command::Access);
            d.rn16 = rn(&b[24..40]);
        }
        _ => return Err(Refused::Layout),
    }
    Ok(d)
}

/// The commands as the generator writes them.
pub mod encode {
    use super::{bits_of, ebv, with_crc16, with_crc5};

    /// A Query.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        dr: u64,
        m: u64,
        trext: u64,
        sel: u64,
        session: u64,
        target: u64,
        q: u64,
    ) -> Vec<u8> {
        let mut b = bits_of(0b1000, 4);
        for (v, n) in [
            (dr, 1),
            (m, 2),
            (trext, 1),
            (sel, 2),
            (session, 2),
            (target, 1),
            (q, 4),
        ] {
            b.extend(bits_of(v, n));
        }
        with_crc5(b)
    }

    /// A QueryRep.
    pub fn query_rep(session: u64) -> Vec<u8> {
        let mut b = bits_of(0, 2);
        b.extend(bits_of(session, 2));
        b
    }

    /// A QueryAdjust.
    pub fn query_adjust(session: u64, updn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1001, 4);
        b.extend(bits_of(session, 2));
        b.extend(bits_of(updn, 3));
        b
    }

    /// An ACK.
    pub fn ack(rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b01, 2);
        b.extend(bits_of(rn, 16));
        b
    }

    /// A NAK.
    pub fn nak() -> Vec<u8> {
        bits_of(0b1100_0000, 8)
    }

    /// A Select with the mask `mask` (bits).
    pub fn select(
        target: u64,
        action: u64,
        bank: u64,
        pointer: u64,
        mask: &[u8],
        truncate: u64,
    ) -> Vec<u8> {
        let mut b = bits_of(0b1010, 4);
        b.extend(bits_of(target, 3));
        b.extend(bits_of(action, 3));
        b.extend(bits_of(bank, 2));
        b.extend(ebv(pointer));
        b.extend(bits_of(mask.len() as u64, 8));
        b.extend_from_slice(mask);
        b.extend(bits_of(truncate, 1));
        with_crc16(b)
    }

    /// A Req_RN.
    pub fn req_rn(rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0001, 8);
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }

    /// A Read.
    pub fn read(bank: u64, pointer: u64, count: u64, rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0010, 8);
        b.extend(bits_of(bank, 2));
        b.extend(ebv(pointer));
        b.extend(bits_of(count, 8));
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }

    /// A Write of the cover-coded word `data`.
    pub fn write(bank: u64, pointer: u64, data: u64, rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0011, 8);
        b.extend(bits_of(bank, 2));
        b.extend(ebv(pointer));
        b.extend(bits_of(data, 16));
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }

    /// A Kill with the cover-coded password half `half`.
    pub fn kill(half: u64, rfu: u64, rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0100, 8);
        b.extend(bits_of(half, 16));
        b.extend(bits_of(rfu, 3));
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }

    /// A Lock.
    pub fn lock(payload: u64, rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0101, 8);
        b.extend(bits_of(payload, 20));
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }

    /// An Access with the cover-coded password half `half`.
    pub fn access(half: u64, rn: u64) -> Vec<u8> {
        let mut b = bits_of(0b1100_0110, 8);
        b.extend(bits_of(half, 16));
        b.extend(bits_of(rn, 16));
        with_crc16(b)
    }
}
