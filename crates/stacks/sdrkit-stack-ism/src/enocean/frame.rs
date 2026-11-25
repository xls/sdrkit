//! EnOcean frames and telegrams: the ERP1 frame (amplitude keying, 8/12 coding), the ERP2 frame (frequency keying, a length
//! byte, CRC-8), and the telegram layer of both (RORG, sender id, status, the readings of the common equipment profiles).
//!
//! Sources: "EnOcean Radio Protocol 1" V1.2 and "EnOcean Radio Protocol 2" V1.3 (EnOcean GmbH, public); "EnOcean Equipment
//! Profiles" 3.1 and the EEP 2.6.8 profile sheets F6-02-01, D5-00-01, A5-02-01 to A5-02-1B, A5-04-01, A5-06-01, A5-06-02 and
//! A5-07-01 (EnOcean Alliance, public). ISO/IEC 14543-3-10 and 14543-3-11 themselves were not read.

use sdrkit_util_bits::checksum::sum8;
use sdrkit_util_bits::crc::catalog::CRC8_SMBUS;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// The longest ERP1 subtelegram, bytes (ERP1 V1.2, section 2).
pub const ERP1_MAX_BYTES: usize = 21;
/// The ERP1 preamble (ERP1 V1.2, section 1.8): 8 bits, a 1 first.
pub const ERP1_PREAMBLE: [u8; 8] = [1, 0, 1, 0, 1, 0, 1, 0];
/// The ERP1 start of frame, variant 2 (ERP1 V1.2, section 1.9 and the frame figure).
pub const ERP1_SOF: [u8; 4] = [1, 0, 0, 1];
/// The ERP2 preamble, 0xAAAA (ERP2 V1.3, section 3.2).
pub const ERP2_PREAMBLE: u16 = 0xAAAA;
/// The ERP2 synchronization word (ERP2 V1.3, section 3.2).
pub const ERP2_SYNC: u16 = 0xA93C;
/// The ERP2 telegrams longer than this carry a header and a CRC (ERP2 V1.3, sections 4.4 and 4.5).
pub const ERP2_SHORT_MAX: usize = 6;

/// The radio protocol of a subtelegram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Radio {
    /// ERP1: 868.3 MHz, amplitude keying.
    Erp1,
    /// ERP2: frequency keying (902.875 MHz, 928.35 MHz).
    Erp2,
}

impl Radio {
    /// "ERP1" or "ERP2".
    pub fn text(self) -> &'static str {
        match self {
            Radio::Erp1 => "ERP1",
            Radio::Erp2 => "ERP2",
        }
    }
}

/// The integrity check of a subtelegram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hash {
    /// The 8-bit sum (ERP1, STATUS bit 7 = 0).
    Sum,
    /// CRC-8 x^8 + x^2 + x + 1 (ERP1 with STATUS bit 7 = 1, and ERP2).
    Crc,
    /// The 6-byte switch form of ERP1: its 4-bit check is not described publicly and is not tested.
    NotTested,
}

impl Hash {
    /// The text shown.
    pub fn text(self) -> &'static str {
        match self {
            Hash::Sum => "checksum",
            Hash::Crc => "CRC-8",
            Hash::NotTested => "not tested (switch form)",
        }
    }
}

/// One subtelegram after its check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Telegram {
    /// The radio protocol.
    pub radio: Radio,
    /// The telegram type (RORG). For an addressed telegram (ADT, 0xA6) the encapsulated type.
    pub rorg: u8,
    /// The payload (DATA, or Data_DL).
    pub data: Vec<u8>,
    /// The sender id.
    pub sender: u64,
    /// The bytes of the sender id: 3, 4 or 6.
    pub sender_len: u8,
    /// The destination id of an addressed telegram.
    pub dest: Option<u32>,
    /// The repeater level: 0 original, 1 or 2 repeated, 15 not to be repeated.
    pub hops: u8,
    /// The ERP1 status byte (none in ERP2). For a switch telegram the status that its CHOICE implies.
    pub status: Option<u8>,
    /// The check that passed.
    pub hash: Hash,
}

impl Telegram {
    /// The sender id as hex ("0180A1B2").
    pub fn sender_text(&self) -> String {
        let mut s = String::new();
        let n = usize::from(self.sender_len.clamp(1, 8));
        let _ = write!(s, "{:0w$X}", self.sender, w = 2 * n);
        s
    }

    /// The repeater level as text.
    pub fn hops_text(&self) -> &'static str {
        match self.hops {
            0 => "original",
            1 => "repeated once",
            2 => "repeated twice",
            15 => "original, not to be repeated",
            _ => "reserved repeater level",
        }
    }
}

/// The ERP1 bits of `bytes` (a subtelegram with its HASH): the preamble, the start of frame (variant 2), then each byte as 12
/// bits: b7 b6 b5 !b5 b4 b3 b2 !b2 b1 b0 and the sync bits 0 1, or 1 0 (the end of frame) after the last byte. Logical bits: on
/// air a 1 is low power and a 0 high power.
pub fn erp1_bits(bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&ERP1_PREAMBLE);
    out.extend_from_slice(&ERP1_SOF);
    for (i, &b) in bytes.iter().enumerate() {
        let bit = |k: u32| (b >> k) & 1;
        out.extend_from_slice(&[
            bit(7),
            bit(6),
            bit(5),
            bit(5) ^ 1,
            bit(4),
            bit(3),
            bit(2),
            bit(2) ^ 1,
            bit(1),
            bit(0),
        ]);
        if i + 1 == bytes.len() {
            out.extend_from_slice(&[1, 0]);
        } else {
            out.extend_from_slice(&[0, 1]);
        }
    }
}

/// The start of the ERP1 data in logical `bits`: just after the last four preamble bits and the start of frame. Variant 2
/// (1 0 0 1) first; then variant 1 (a code violation of 1.5-bit runs) as an NRZ slicer that rounds 1.5 up reads it (1 1 0 0 1).
pub fn erp1_data_start(bits: &[u8]) -> Option<usize> {
    const V2: [u8; 8] = [1, 0, 1, 0, 1, 0, 0, 1];
    const V1: [u8; 9] = [1, 0, 1, 0, 1, 1, 0, 0, 1];
    let find = |p: &[u8]| bits.windows(p.len()).position(|w| w == p).map(|i| i + p.len());
    find(&V2).or_else(|| find(&V1))
}

/// The bytes of ERP1 logical `bits` from `start` (the first data bit), to the end of frame. `None` when an inverse bit or a
/// sync pair is wrong, the bits end first, or the frame is longer than [`ERP1_MAX_BYTES`].
pub fn erp1_bytes(bits: &[u8], start: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = start;
    loop {
        let g = bits.get(at..at + 12)?;
        if g[3] != g[2] ^ 1 || g[7] != g[6] ^ 1 {
            return None;
        }
        let b = (g[0] << 7)
            | (g[1] << 6)
            | (g[2] << 5)
            | (g[4] << 4)
            | (g[5] << 3)
            | (g[6] << 2)
            | (g[8] << 1)
            | g[9];
        out.push(b);
        if out.len() > ERP1_MAX_BYTES {
            return None;
        }
        at += 12;
        match (g[10], g[11]) {
            (0, 1) => {}
            (1, 0) => return Some(out),
            _ => return None,
        }
    }
}

/// The telegram of ERP1 subtelegram `b` (RORG to HASH), checked. A 6-byte subtelegram is the switch form: a 4-bit CHOICE,
/// one data byte, the 4-byte id and a 4-bit check (the layout is read in this order; the check is not tested). Longer ones are
/// RORG, DATA, the id, STATUS and HASH; STATUS bit 7 picks the CRC or the sum.
pub fn erp1_telegram(b: &[u8]) -> Option<Telegram> {
    let n = b.len();
    if n == 6 {
        let choice = b[0] >> 4;
        let status = match choice {
            0x5 => 0x20,
            0x6 => 0x30,
            _ => return None,
        };
        let mut v: u64 = 0;
        for &x in b {
            v = (v << 8) | u64::from(x);
        }
        let data = ((v >> 36) & 0xFF) as u8;
        let sender = (v >> 4) & 0xFFFF_FFFF;
        return Some(Telegram {
            radio: Radio::Erp1,
            rorg: 0xF6,
            data: vec![data],
            sender,
            sender_len: 4,
            dest: None,
            hops: 0,
            status: Some(status),
            hash: Hash::NotTested,
        });
    }
    if n < 7 {
        return None;
    }
    let status = b[n - 2];
    let body = &b[..n - 1];
    let hash = if status & 0x80 != 0 {
        if CRC8_SMBUS.checksum(body) != b[n - 1] {
            return None;
        }
        Hash::Crc
    } else {
        if sum8(body) != b[n - 1] {
            return None;
        }
        Hash::Sum
    };
    let sender = b[n - 6..n - 2]
        .iter()
        .fold(0u64, |v, &x| (v << 8) | u64::from(x));
    let (rorg, data, dest) = if b[0] == 0xA6 {
        if n < 12 {
            return None;
        }
        let d = b[n - 10..n - 6]
            .iter()
            .fold(0u32, |v, &x| (v << 8) | u32::from(x));
        (b[1], b[2..n - 10].to_vec(), Some(d))
    } else {
        (b[0], b[1..n - 6].to_vec(), None)
    };
    Some(Telegram {
        radio: Radio::Erp1,
        rorg,
        data,
        sender,
        sender_len: 4,
        dest,
        hops: status & 0x0F,
        status: Some(status),
        hash,
    })
}

/// The ERP1 subtelegram bytes of `t`: RORG (or 0xA6 and RORG with a destination), DATA, the id, `status` and the HASH that
/// `status` bit 7 picks.
pub fn erp1_encode(rorg: u8, data: &[u8], dest: Option<u32>, sender: u32, status: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(data.len() + 12);
    if dest.is_some() {
        b.push(0xA6);
    }
    b.push(rorg);
    b.extend_from_slice(data);
    if let Some(d) = dest {
        b.extend_from_slice(&d.to_be_bytes());
    }
    b.extend_from_slice(&sender.to_be_bytes());
    b.push(status);
    let h = if status & 0x80 != 0 {
        CRC8_SMBUS.checksum(&b)
    } else {
        sum8(&b)
    };
    b.push(h);
    b
}

/// The RORG of an ERP2 telegram type (header bits 0 to 3) and extended type (ERP2 V1.3, figures 11 and 13).
fn erp2_rorg(kind: u8, ext: Option<u8>) -> Option<u8> {
    Some(match kind {
        0x0 => 0xF6,
        0x1 => 0xD5,
        0x2 => 0xA5,
        0x3 => 0xD0,
        0x4 => 0xD2,
        0x5 => 0xD4,
        0x6 => 0xD1,
        0x7 => 0x30,
        0x8 => 0x31,
        0x9 => 0x35,
        0xA => 0xB3,
        0xB => 0xA8,
        0xF => match ext? {
            0x00 => 0xC5,
            0x01 => 0xC6,
            0x02 => 0xC7,
            0x03 => 0x40,
            0x04 => 0x32,
            0x05 => 0xB0,
            0x06 => 0xB1,
            0x07 => 0xB2,
            e => e,
        },
        _ => return None,
    })
}

/// The ERP2 telegram type of `rorg` (header bits 0 to 3) and the extended type, for the test signal.
fn erp2_kind(rorg: u8) -> (u8, Option<u8>) {
    match rorg {
        0xF6 => (0x0, None),
        0xD5 => (0x1, None),
        0xA5 => (0x2, None),
        0xD0 => (0x3, None),
        0xD2 => (0x4, None),
        0xD4 => (0x5, None),
        0xD1 => (0x6, None),
        0x30 => (0x7, None),
        0x31 => (0x8, None),
        0x35 => (0x9, None),
        0xB3 => (0xA, None),
        0xA8 => (0xB, None),
        r => (0xF, Some(r)),
    }
}

/// The telegram of ERP2 `data_pl` (the bytes that the length byte counts, the CRC last), checked. Only telegrams longer than
/// [`ERP2_SHORT_MAX`] bytes have a CRC; shorter ones have no check and give `None`.
pub fn erp2_telegram(d: &[u8]) -> Option<Telegram> {
    let n = d.len();
    if n <= ERP2_SHORT_MAX || CRC8_SMBUS.checksum(&d[..n - 1]) != d[n - 1] {
        return None;
    }
    let h = d[0];
    let mut at = 1;
    let mut hops = 0;
    let mut optional = 0;
    if h & 0x10 != 0 {
        let e = *d.get(at)?;
        hops = e >> 4;
        optional = usize::from(e & 0x0F);
        at += 1;
    }
    let ext = if h & 0x0F == 0x0F {
        at += 1;
        Some(*d.get(at - 1)?)
    } else {
        None
    };
    let rorg = erp2_rorg(h & 0x0F, ext)?;
    let (id_len, has_dest) = match h >> 5 {
        0 => (3, false),
        1 => (4, false),
        2 => (4, true),
        3 => (6, false),
        _ => return None,
    };
    let sender = d
        .get(at..at + id_len)?
        .iter()
        .fold(0u64, |v, &x| (v << 8) | u64::from(x));
    at += id_len;
    let dest = if has_dest {
        let v = d
            .get(at..at + 4)?
            .iter()
            .fold(0u32, |v, &x| (v << 8) | u32::from(x));
        at += 4;
        Some(v)
    } else {
        None
    };
    let end = (n - 1).checked_sub(optional)?;
    if end < at {
        return None;
    }
    Some(Telegram {
        radio: Radio::Erp2,
        rorg,
        data: d[at..end].to_vec(),
        sender,
        sender_len: id_len as u8,
        dest,
        hops,
        status: None,
        hash: Hash::Crc,
    })
}

/// The ERP2 Data_PL of a telegram with a 32-bit sender id, no extended header, `rorg`, `data` and an optional destination,
/// with its CRC; for the test signal.
pub fn erp2_encode(rorg: u8, data: &[u8], dest: Option<u32>, sender: u32) -> Vec<u8> {
    let (kind, ext) = erp2_kind(rorg);
    let ac = if dest.is_some() { 2 } else { 1 };
    let mut d = vec![(ac << 5) | kind];
    d.extend(ext);
    d.extend_from_slice(&sender.to_be_bytes());
    if let Some(x) = dest {
        d.extend_from_slice(&x.to_be_bytes());
    }
    d.extend_from_slice(data);
    d.push(CRC8_SMBUS.checksum(&d));
    d
}

/// The ERP2 bits of `data_pl`: the preamble, the synchronization word, the length byte and the bytes, MSB first.
pub fn erp2_bits(data_pl: &[u8], out: &mut Vec<u8>) {
    let mut push = |v: u32, n: u32| {
        for k in (0..n).rev() {
            out.push(((v >> k) & 1) as u8);
        }
    };
    push(u32::from(ERP2_PREAMBLE), 16);
    push(u32::from(ERP2_SYNC), 16);
    push(data_pl.len() as u32, 8);
    for &b in data_pl {
        push(u32::from(b), 8);
    }
}

/// The ERP2 Data_PL in `bits` (1 the upper tone) after the last four preamble bits and the synchronization word. `None`
/// without the word, or when the bits end before the length byte says.
pub fn erp2_data(bits: &[u8]) -> Option<Vec<u8>> {
    let mut pat = [0u8; 20];
    for (k, p) in pat.iter_mut().enumerate() {
        let v = (u32::from(ERP2_PREAMBLE & 0xF) << 16) | u32::from(ERP2_SYNC);
        *p = ((v >> (19 - k)) & 1) as u8;
    }
    let start = bits.windows(20).position(|w| w == pat)? + 20;
    let byte = |i: usize| -> Option<u8> {
        let g = bits.get(start + 8 * i..start + 8 * i + 8)?;
        Some(g.iter().fold(0u8, |v, &b| (v << 1) | b))
    };
    let len = usize::from(byte(0)?);
    if len == 0 {
        return None;
    }
    (1..=len).map(byte).collect()
}

/// The name of a telegram type (EEP 3.1, section 3, and ERP2 V1.3, figure 11).
pub fn rorg_name(rorg: u8) -> &'static str {
    match rorg {
        0xF6 => "RPS (switch)",
        0xD5 => "1BS (1 byte)",
        0xA5 => "4BS (4 bytes)",
        0xD2 => "VLD (variable length)",
        0xD1 => "MSC (manufacturer specific)",
        0xA6 => "ADT (addressed)",
        0xC6 => "Smart Ack learn request",
        0xC7 => "Smart Ack learn answer",
        0xA7 => "Smart Ack reclaim",
        0xC5 => "Remote management",
        0x30 => "Secure",
        0x31 => "Secure, encapsulated type",
        0x32 => "Secure, decrypted",
        0x34 => "Security maintenance",
        0x35 => "Secure teach-in",
        0xD0 => "Signal",
        0xD4 => "Universal teach-in",
        0x40 => "Chained data",
        0xA8 => "Acknowledge",
        0xB0..=0xB3 => "Generic profiles",
        _ => "unknown type",
    }
}

/// Whether `rorg` is a secure (encrypted) telegram type.
pub fn is_secure(rorg: u8) -> bool {
    matches!(rorg, 0x30 | 0x31 | 0x33 | 0x34 | 0x35)
}

/// A profile (EEP) learned from a 4BS teach-in telegram with its profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eep {
    /// RORG.
    pub rorg: u8,
    /// FUNC.
    pub func: u8,
    /// TYPE.
    pub kind: u8,
}

impl Eep {
    /// "A5-02-05".
    pub fn text(self) -> String {
        format!("{:02X}-{:02X}-{:02X}", self.rorg, self.func, self.kind)
    }
}

/// What a telegram says.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The reading as one line.
    pub text: String,
    /// The profile that a 4BS teach-in names, to keep for the sender.
    pub learned: Option<Eep>,
    /// The profile the reading follows, when it is certain.
    pub profile: Option<String>,
}

/// The names of the buttons of F6-02-01 (EEP 2.6.8): A I, A 0, B I, B 0.
const ROCKER: [&str; 4] = ["AI", "A0", "BI", "B0"];

/// The reading of `t`. `eep` is the profile that a teach-in of the sender gave before.
pub fn reading(t: &Telegram, eep: Option<Eep>) -> Reading {
    let mut r = Reading {
        text: String::new(),
        learned: None,
        profile: None,
    };
    if is_secure(t.rorg) {
        r.text = format!("encrypted, {} bytes", t.data.len());
        return r;
    }
    match (t.rorg, t.data.as_slice()) {
        (0xF6, [d]) => rps(t, *d, &mut r),
        (0xD5, [d]) => {
            r.profile = Some("D5-00-01".into());
            r.text = if d & 0x08 == 0 {
                "teach-in (1BS)".into()
            } else if d & 1 == 1 {
                "contact closed".into()
            } else {
                "contact open".into()
            };
        }
        (0xA5, [db3, db2, db1, db0]) => four_bs([*db3, *db2, *db1, *db0], eep, &mut r),
        (0xD2, d) => r.text = format!("VLD data {} (profile not sent)", crate::family::hex(d)),
        (0xD1, d) if d.len() >= 2 => {
            let m = (u16::from(d[0]) << 3) | u16::from(d[1] >> 5);
            r.text = format!("manufacturer {m:03X}, {} bytes", d.len());
        }
        (0xD4, _) => r.text = "universal teach-in".into(),
        (rorg, d) => r.text = format!("{}, {} bytes", rorg_name(rorg), d.len()),
    }
    r
}

/// An RPS reading by F6-02-01 when the status says a PTM 2xx module (T21 = 1); the profile is not sent.
fn rps(t: &Telegram, d: u8, r: &mut Reading) {
    let Some(status) = t.status else {
        // ERP2 carries no status: the PTM bits that F6-02-01 needs are not known.
        r.text = format!("switch data {d:02X}");
        return;
    };
    let (t21, nu) = (status & 0x20 != 0, status & 0x10 != 0);
    if !t21 {
        r.text = format!("switch data {d:02X} (PTM 1xx module)");
        return;
    }
    let eb = if d & 0x10 != 0 { "pressed" } else { "released" };
    if nu {
        let first = ROCKER[usize::from(d >> 5) & 3];
        r.text = format!("rocker {first} {eb}");
        if d & 1 == 1 {
            let second = ROCKER[usize::from(d >> 1) & 3];
            let _ = write!(r.text, ", and {second}");
        }
    } else {
        let n = match d >> 5 {
            0 => "no button",
            3 => "3 or 4 buttons",
            _ => "buttons (not valid)",
        };
        r.text = format!("{n} {eb}");
    }
    r.text.push_str(" (as F6-02-01)");
}

/// A 4BS reading: a teach-in, or the data by the learned profile, else the raw bytes.
fn four_bs(db: [u8; 4], eep: Option<Eep>, r: &mut Reading) {
    let [db3, db2, db1, db0] = db;
    if db0 & 0x08 == 0 {
        if db0 & 0x80 != 0 {
            let e = Eep {
                rorg: 0xA5,
                func: db3 >> 2,
                kind: ((db3 & 3) << 5) | (db2 >> 3),
            };
            let m = (u16::from(db2 & 7) << 8) | u16::from(db1);
            r.text = format!("teach-in, profile {}, manufacturer {m:03X}", e.text());
            r.learned = Some(e);
        } else {
            r.text = "teach-in without profile".into();
        }
        return;
    }
    let Some(e) = eep.filter(|e| e.rorg == 0xA5) else {
        r.text = format!(
            "4BS data {} (profile unknown: no teach-in heard)",
            crate::family::hex(&db)
        );
        return;
    };
    let text = match (e.func, e.kind) {
        (0x02, k @ 0x01..=0x0B) => {
            let min = -40.0 + 10.0 * f64::from(k - 1);
            Some(format!("{:.1} °C", min + f64::from(255 - db1) * 40.0 / 255.0))
        }
        (0x02, k @ 0x10..=0x1B) => {
            let min = -60.0 + 10.0 * f64::from(k - 0x10);
            Some(format!("{:.1} °C", min + f64::from(255 - db1) * 80.0 / 255.0))
        }
        (0x04, 0x01) if db2 <= 250 && db1 <= 250 => {
            let mut s = format!("{:.1} % humidity", f64::from(db2) * 100.0 / 250.0);
            if db0 & 0x02 != 0 {
                let _ = write!(s, ", {:.1} °C", f64::from(db1) * 40.0 / 250.0);
            }
            Some(s)
        }
        (0x06, 0x01) => {
            let lx = if db0 & 1 == 1 {
                300.0 + f64::from(db2) * (30_000.0 - 300.0) / 255.0
            } else {
                600.0 + f64::from(db1) * (60_000.0 - 600.0) / 255.0
            };
            Some(format!("{lx:.0} lx, supply {:.2} V", f64::from(db3) * 5.1 / 255.0))
        }
        (0x06, 0x02) => {
            let lx = if db0 & 1 == 1 {
                f64::from(db2) * 510.0 / 255.0
            } else {
                f64::from(db1) * 1020.0 / 255.0
            };
            Some(format!("{lx:.0} lx, supply {:.2} V", f64::from(db3) * 5.1 / 255.0))
        }
        (0x07, 0x01) => {
            let mut s = String::from(if db1 >= 128 { "motion" } else { "no motion" });
            if db0 & 1 == 1 && db3 <= 250 {
                let _ = write!(s, ", supply {:.2} V", f64::from(db3) * 5.0 / 250.0);
            }
            Some(s)
        }
        _ => None,
    };
    match text {
        Some(t) => {
            r.text = t;
            r.profile = Some(e.text());
        }
        None => {
            r.text = format!(
                "4BS data {} (profile {} not decoded)",
                crate::family::hex(&db),
                e.text()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erp1_round_trip_and_the_size_formula() {
        // A 1BS telegram, checksum form: D5 09, id 01 80 A1 B2, status 00.
        let b = erp1_encode(0xD5, &[0x09], None, 0x0180_A1B2, 0x00);
        assert_eq!(b.len(), 8);
        let mut bits = Vec::new();
        erp1_bits(&b, &mut bits);
        assert_eq!(bits.len(), 12 + 12 * b.len());
        let start = erp1_data_start(&bits[1..]).unwrap() + 1;
        assert_eq!(start, 12);
        let got = erp1_bytes(&bits, start).unwrap();
        assert_eq!(got, b);
        let t = erp1_telegram(&got).unwrap();
        assert_eq!((t.rorg, t.sender, t.hash), (0xD5, 0x0180_A1B2, Hash::Sum));
        assert_eq!(reading(&t, None).text, "contact closed");
    }

    #[test]
    fn a_crc_status_an_addressed_telegram_and_a_bad_inverse_bit() {
        let b = erp1_encode(0xA5, &[1, 2, 3, 0x08], Some(0xF1F2_F3F4), 0x1234_5678, 0x80);
        let t = erp1_telegram(&b).unwrap();
        assert_eq!((t.rorg, t.dest, t.hash), (0xA5, Some(0xF1F2_F3F4), Hash::Crc));
        let mut bits = Vec::new();
        erp1_bits(&b, &mut bits);
        bits[12 + 3] ^= 1;
        assert!(erp1_bytes(&bits, 12).is_none());
    }

    #[test]
    fn teach_in_then_temperature() {
        // A5-02-05 teach-in: FUNC 02, TYPE 05, manufacturer 00B, LRN type 1, LRN bit 0.
        let ti = [0x02 << 2, 0x05 << 3, 0x0B, 0x80];
        let t = erp1_telegram(&erp1_encode(0xA5, &ti, None, 1, 0)).unwrap();
        let r = reading(&t, None);
        assert_eq!(r.learned.map(Eep::text).as_deref(), Some("A5-02-05"));
        // 0 to +40 °C, 255 to 0: DB1 = 0 is +40 °C.
        let d = erp1_telegram(&erp1_encode(0xA5, &[0, 0, 0, 0x08], None, 1, 0)).unwrap();
        assert_eq!(reading(&d, r.learned).text, "40.0 °C");
    }

    #[test]
    fn the_switch_form_as_read() {
        // CHOICE 6 (status 0x30), data 0x30, id 0029_1A01, check nibble 0.
        let v: u64 = (0x6 << 44) | (0x30 << 36) | (0x0029_1A01 << 4);
        let b: Vec<u8> = (0..6).rev().map(|k| (v >> (8 * k)) as u8).collect();
        let t = erp1_telegram(&b).unwrap();
        assert_eq!((t.rorg, t.sender, t.hash), (0xF6, 0x0029_1A01, Hash::NotTested));
        assert_eq!(reading(&t, None).text, "rocker A0 pressed (as F6-02-01)");
    }

    #[test]
    fn erp2_round_trip() {
        let d = erp2_encode(0xF6, &[0x30], None, 0x0405_0607);
        let mut bits = Vec::new();
        erp2_bits(&d, &mut bits);
        let got = erp2_data(&bits).unwrap();
        assert_eq!(got, d);
        let t = erp2_telegram(&got).unwrap();
        assert_eq!((t.rorg, t.sender, t.data.as_slice()), (0xF6, 0x0405_0607, &[0x30][..]));
    }
}
