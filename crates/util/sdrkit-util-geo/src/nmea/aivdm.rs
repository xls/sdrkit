//! AIS messages in NMEA sentences: `!AIVDM` (heard from others) and `!AIVDO` (own ship), with the six-bit ASCII payload
//! armour. A message longer than one sentence is split into fragments of at most [`MAX_PAYLOAD_CHARS`] characters. Bits are
//! passed and returned one per byte (0 or 1), or packed most significant bit first.
//!
//! Standard: IEC 61162-1; ITU-R M.1371; IEC 61993-2.

use super::{Builder, Sentence, MAX_LEN};
use crate::{Error, Text};
use crate::{Quantity, SyntaxFault};

/// The most payload characters per sentence, so that `!AIVDM,n,n,s,c,` + payload + `,f*hh` + CR LF fits in 82 characters.
pub const MAX_PAYLOAD_CHARS: usize = 60;
/// The most fragments of one message.
pub const MAX_FRAGMENTS: usize = 9;

/// The armoured character of a 6-bit value (0 to 63).
pub fn armour_char(v: u8) -> Result<char, Error> {
    match v {
        0..=39 => Ok((v + 48) as char),
        40..=63 => Ok((v + 56) as char),
        _ => Err(Error::OutOfRange(Quantity::SixBitValue)),
    }
}

/// The 6-bit value of an armoured character.
pub fn dearmour_char(c: u8) -> Result<u8, Error> {
    match c {
        b'0'..=b'W' => Ok(c - 48),
        b'`'..=b'w' => Ok(c - 56),
        _ => Err(Error::Syntax(SyntaxFault::AisPayloadChar)),
    }
}

/// Armours bits (one per byte, 0 or 1), appending the payload to `out`, and returns the fill bits (0 to 5). Allocation-free when
/// `out` has room.
///
/// ```
/// use sdrkit_util_geo::nmea::aivdm;
/// let mut s = String::new();
/// let fill = aivdm::armour(&[0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1], &mut s).unwrap();
/// assert_eq!((s.as_str(), fill), ("1w", 0));
/// ```
pub fn armour(bits: &[u8], out: &mut String) -> Result<u8, Error> {
    if let Some(at) = bits.iter().position(|&b| b > 1) {
        return Err(Error::BadChar { at });
    }
    for chunk in bits.chunks(6) {
        let mut v = 0u8;
        for i in 0..6 {
            v = (v << 1) | chunk.get(i).copied().unwrap_or(0);
        }
        out.push(armour_char(v)?);
    }
    Ok(((6 - bits.len() % 6) % 6) as u8)
}

/// Armours the first `bit_count` bits of `bytes` (most significant bit first) into `out`; returns the fill bits.
pub fn armour_packed(bytes: &[u8], bit_count: usize, out: &mut String) -> Result<u8, Error> {
    if bit_count > bytes.len() * 8 {
        return Err(Error::OutOfRange(Quantity::BitCount));
    }
    let bit = |i: usize| (bytes[i / 8] >> (7 - i % 8)) & 1;
    let mut i = 0;
    while i < bit_count {
        let mut v = 0u8;
        for k in 0..6 {
            v = (v << 1) | if i + k < bit_count { bit(i + k) } else { 0 };
        }
        out.push(armour_char(v)?);
        i += 6;
    }
    Ok(((6 - bit_count % 6) % 6) as u8)
}

/// Removes the armour, appending the bits (one per byte) to `out` without the `fill` bits. Allocation-free when `out` has room.
///
/// ```
/// use sdrkit_util_geo::nmea::aivdm;
/// let mut bits = Vec::new();
/// aivdm::dearmour("177KQJ5000G?tO`K>RA1wUbN0TKH", 0, &mut bits).unwrap();
/// // Message type 1, MMSI 477553000 (the example of the gpsd AIVDM/AIVDO protocol document).
/// let field = |a: usize, n: usize| bits[a..a + n].iter().fold(0u32, |v, &b| v << 1 | b as u32);
/// assert_eq!((field(0, 6), field(8, 30)), (1, 477_553_000));
/// ```
pub fn dearmour(payload: &str, fill: u8, out: &mut Vec<u8>) -> Result<(), Error> {
    let total = check_fill(payload, fill)?;
    let start = out.len();
    for (at, c) in payload.bytes().enumerate() {
        let v = dearmour_char(c).map_err(|_| Error::BadChar { at })?;
        for i in (0..6).rev() {
            out.push((v >> i) & 1);
        }
    }
    out.truncate(start + total);
    Ok(())
}

/// Removes the armour into `out` packed most significant bit first; returns the number of bits. `out` must hold them.
pub fn dearmour_packed(payload: &str, fill: u8, out: &mut [u8]) -> Result<usize, Error> {
    let total = check_fill(payload, fill)?;
    if out.len() * 8 < total {
        return Err(Error::TooLong);
    }
    out[..total.div_ceil(8)].fill(0);
    let mut n = 0;
    for (at, c) in payload.bytes().enumerate() {
        let v = dearmour_char(c).map_err(|_| Error::BadChar { at })?;
        for i in (0..6).rev() {
            if n < total {
                out[n / 8] |= ((v >> i) & 1) << (7 - n % 8);
                n += 1;
            }
        }
    }
    Ok(total)
}

fn check_fill(payload: &str, fill: u8) -> Result<usize, Error> {
    if fill > 5 {
        return Err(Error::OutOfRange(Quantity::FillBits));
    }
    (payload.len() * 6)
        .checked_sub(fill as usize)
        .ok_or(Error::OutOfRange(Quantity::FillBitsPastPayload))
}

/// The fields of one `VDM` or `VDO` sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vdm<'a> {
    /// Whether it is `VDO` (own ship) rather than `VDM`.
    pub own_ship: bool,
    /// Number of fragments of the message, 1 to 9.
    pub count: u8,
    /// This fragment's number, 1 to `count`.
    pub number: u8,
    /// Sequential message id 0 to 9 (multi-fragment messages).
    pub sequence_id: Option<u8>,
    /// AIS channel: `A`, `B` (or `1`, `2`, as some receivers write it).
    pub channel: Option<char>,
    /// The armoured payload.
    pub payload: &'a str,
    /// Fill bits, 0 to 5.
    pub fill: u8,
}

impl<'a> Vdm<'a> {
    /// Reads the fields of a parsed `!xxVDM` / `!xxVDO` sentence and checks them.
    pub fn from_sentence(s: &Sentence<'a>) -> Result<Vdm<'a>, Error> {
        let own_ship = match s.formatter() {
            "VDM" => false,
            "VDO" => true,
            _ => return Err(Error::Syntax(SyntaxFault::NotVdm)),
        };
        let mut f = s.fields();
        let mut next = || f.next().ok_or(Error::Syntax(SyntaxFault::VdmFieldCount));
        let num = |t: &str, what: SyntaxFault| t.parse::<u8>().map_err(|_| Error::Syntax(what));
        let count = num(next()?, SyntaxFault::FragmentCount)?;
        let number = num(next()?, SyntaxFault::FragmentNumber)?;
        let seq = next()?;
        let chan = next()?;
        let payload = next()?;
        let fill = num(next()?, SyntaxFault::FillBits)?;
        if !(1..=MAX_FRAGMENTS as u8).contains(&count) || number == 0 || number > count {
            return Err(Error::OutOfRange(Quantity::FragmentNumbers));
        }
        let sequence_id = if seq.is_empty() {
            None
        } else {
            Some(num(seq, SyntaxFault::SequentialMessageId)?).filter(|v| *v <= 9)
        };
        if !seq.is_empty() && sequence_id.is_none() {
            return Err(Error::OutOfRange(Quantity::SequentialMessageId));
        }
        let channel = match chan.as_bytes() {
            [] => None,
            [c @ (b'A' | b'B' | b'1' | b'2')] => Some(*c as char),
            _ => return Err(Error::Syntax(SyntaxFault::Channel)),
        };
        if fill > 5 {
            return Err(Error::OutOfRange(Quantity::FillBits));
        }
        if let Some(at) = payload.bytes().position(|c| dearmour_char(c).is_err()) {
            return Err(Error::BadChar { at });
        }
        Ok(Vdm {
            own_ship,
            count,
            number,
            sequence_id,
            channel,
            payload,
            fill,
        })
    }
}

/// The sentences of one message, built one at a time without allocation. Made by [`fragments`].
#[derive(Clone, Debug)]
pub struct Fragments<'a> {
    address: Text<8>,
    channel: Option<char>,
    sequence_id: u8,
    payload: &'a str,
    fill: u8,
    count: usize,
    next: usize,
}

/// Splits an armoured payload into `!AIVDM` (or `!AIVDO`, by `address`) sentences. `sequence_id` (0 to 9) is written only when
/// there is more than one fragment; the caller advances it per multi-fragment message.
///
/// ```
/// use sdrkit_util_geo::nmea::aivdm;
/// let s: Vec<_> = aivdm::fragments("AIVDM", Some('A'), 0, "15RTgt0PAso;90TKcjM8h6g208CQ", 0).unwrap().collect();
/// assert_eq!(s[0].as_ref().unwrap().as_str(), "!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A");
/// ```
pub fn fragments<'a>(
    address: &str,
    channel: Option<char>,
    sequence_id: u8,
    payload: &'a str,
    fill: u8,
) -> Result<Fragments<'a>, Error> {
    if sequence_id > 9 || fill > 5 {
        return Err(Error::OutOfRange(Quantity::SequentialMessageIdOrFillBits));
    }
    if let Some(c) = channel {
        if !matches!(c, 'A' | 'B' | '1' | '2') {
            return Err(Error::Syntax(SyntaxFault::Channel));
        }
    }
    if let Some(at) = payload.bytes().position(|c| dearmour_char(c).is_err()) {
        return Err(Error::BadChar { at });
    }
    let count = payload.len().div_ceil(MAX_PAYLOAD_CHARS).max(1);
    if count > MAX_FRAGMENTS {
        return Err(Error::TooLong);
    }
    Ok(Fragments {
        address: Text::try_from_str(address)?,
        channel,
        sequence_id,
        payload,
        fill,
        count,
        next: 0,
    })
}

impl Iterator for Fragments<'_> {
    type Item = Result<Text<MAX_LEN>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.count {
            return None;
        }
        let i = self.next;
        self.next += 1;
        let part = &self.payload[(i * MAX_PAYLOAD_CHARS).min(self.payload.len())
            ..((i + 1) * MAX_PAYLOAD_CHARS).min(self.payload.len())];
        let build = || -> Result<Text<MAX_LEN>, Error> {
            let mut b = Builder::new('!', &self.address)?;
            b.field_fmt(format_args!("{}", self.count))?
                .field_fmt(format_args!("{}", i + 1))?;
            if self.count > 1 {
                b.field_fmt(format_args!("{}", self.sequence_id))?;
            } else {
                b.field("")?;
            }
            match self.channel {
                Some(c) => b.field_fmt(format_args!("{c}"))?,
                None => b.field("")?,
            };
            b.field(part)?.field_fmt(format_args!(
                "{}",
                if i + 1 == self.count { self.fill } else { 0 }
            ))?;
            b.finish()
        };
        Some(build())
    }
}

/// A complete message from [`Assembler::push`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<'a> {
    /// The message bits, one per byte.
    pub bits: &'a [u8],
    /// The channel of the last fragment.
    pub channel: Option<char>,
    /// Whether it came in `VDO` (own ship).
    pub own_ship: bool,
}

#[derive(Clone, Debug, Default)]
struct Slot {
    payload: String,
    count: u8,
    next: u8,
    channel: Option<char>,
}

/// Joins multi-fragment messages. Messages with different sequential ids may interleave; a fragment out of order discards its
/// partial message. Every buffer is sized when the assembler is made, so pushing allocates nothing.
#[derive(Clone, Debug)]
pub struct Assembler {
    /// One slot per sequential id 0–9, and one for fragments without an id.
    slots: [Slot; 11],
    bits: Vec<u8>,
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler {
    /// An empty assembler.
    pub fn new() -> Assembler {
        let slot = || Slot {
            payload: String::with_capacity(MAX_FRAGMENTS * MAX_PAYLOAD_CHARS),
            ..Slot::default()
        };
        Assembler {
            slots: std::array::from_fn(|_| slot()),
            bits: Vec::with_capacity(MAX_FRAGMENTS * MAX_PAYLOAD_CHARS * 6),
        }
    }

    /// Adds one sentence's fields; returns the message when this completes one.
    pub fn push(&mut self, v: &Vdm<'_>) -> Result<Option<Message<'_>>, Error> {
        if v.count == 1 {
            self.bits.clear();
            dearmour(v.payload, v.fill, &mut self.bits)?;
            return Ok(Some(Message {
                bits: &self.bits,
                channel: v.channel,
                own_ship: v.own_ship,
            }));
        }
        let slot = &mut self.slots[v.sequence_id.map_or(10, usize::from)];
        if v.number == 1 {
            slot.payload.clear();
            slot.count = v.count;
            slot.next = 1;
        }
        if slot.next != v.number
            || slot.count != v.count
            || slot.payload.len() + v.payload.len() > slot.payload.capacity()
        {
            slot.next = 0;
            slot.payload.clear();
            return Err(Error::Syntax(SyntaxFault::FragmentOrder));
        }
        slot.payload.push_str(v.payload);
        slot.channel = v.channel;
        slot.next += 1;
        if v.number < v.count {
            return Ok(None);
        }
        slot.next = 0;
        self.bits.clear();
        let r = dearmour(&slot.payload, v.fill, &mut self.bits);
        slot.payload.clear();
        r?;
        Ok(Some(Message {
            bits: &self.bits,
            channel: v.channel,
            own_ship: v.own_ship,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nmea::parse;

    fn field(bits: &[u8], at: usize, n: usize) -> u64 {
        bits[at..at + n]
            .iter()
            .fold(0u64, |v, &b| v << 1 | b as u64)
    }

    #[test]
    fn alphabet() {
        // The armour table: 0 → '0', 39 → 'W', 40 → '`', 63 → 'w'.
        assert_eq!(
            [0, 39, 40, 63].map(|v| armour_char(v).unwrap()),
            ['0', 'W', '`', 'w']
        );
        for v in 0..64 {
            assert_eq!(dearmour_char(armour_char(v).unwrap() as u8), Ok(v));
        }
        assert!(armour_char(64).is_err());
        assert!(dearmour_char(b'X').is_err());
        assert!(dearmour_char(b'x').is_err());
    }

    #[test]
    fn gpsd_examples() {
        // From the gpsd "AIVDM/AIVDO protocol decoding" document: a type 1 position report of MMSI 477553000, and a two-fragment
        // type 5 static report of MMSI 351759000.
        let s = parse("!AIVDM,1,1,,B,177KQJ5000G?tO`K>RA1wUbN0TKH,0*5C").unwrap();
        let v = Vdm::from_sentence(&s).unwrap();
        let mut a = Assembler::new();
        let m = a.push(&v).unwrap().unwrap();
        assert_eq!(
            (field(m.bits, 0, 6), field(m.bits, 8, 30), m.bits.len()),
            (1, 477_553_000, 168)
        );
        assert_eq!(m.channel, Some('B'));
        let p1 = parse(
            "!AIVDM,2,1,1,A,55?MbV02;H;s<HtKR20EHE:0@T4@Dn2222222216L961O5Gf0NSQEp6ClRp8,0*1C",
        )
        .unwrap();
        let p2 = parse("!AIVDM,2,2,1,A,88888888880,2*25").unwrap();
        assert!(a.push(&Vdm::from_sentence(&p1).unwrap()).unwrap().is_none());
        let m = a.push(&Vdm::from_sentence(&p2).unwrap()).unwrap().unwrap();
        assert_eq!(
            (field(m.bits, 0, 6), field(m.bits, 8, 30), m.bits.len()),
            (5, 351_759_000, 424)
        );
        // Building the sentences again from the bits produces the same text.
        let mut payload = String::new();
        let fill = armour(m.bits, &mut payload).unwrap();
        let again: Vec<_> = fragments("AIVDM", Some('A'), 1, &payload, fill)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(again.len(), 2);
        assert_eq!(
            again[0],
            "!AIVDM,2,1,1,A,55?MbV02;H;s<HtKR20EHE:0@T4@Dn2222222216L961O5Gf0NSQEp6ClRp8,0*1C"
        );
        assert_eq!(again[1], "!AIVDM,2,2,1,A,88888888880,2*25");
    }

    #[test]
    fn packed_forms_agree() {
        let mut bits = Vec::new();
        dearmour("15RTgt0PAso;90TKcjM8h6g208CQ", 0, &mut bits).unwrap();
        let mut packed = [0u8; 21];
        assert_eq!(
            dearmour_packed("15RTgt0PAso;90TKcjM8h6g208CQ", 0, &mut packed),
            Ok(168)
        );
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!((packed[i / 8] >> (7 - i % 8)) & 1, b);
        }
        let mut s = String::new();
        assert_eq!(armour_packed(&packed, 168, &mut s), Ok(0));
        assert_eq!(s, "15RTgt0PAso;90TKcjM8h6g208CQ");
        let mut s = String::new();
        assert_eq!(armour_packed(&[0xff], 7, &mut s), Ok(5));
        assert_eq!(s, "wP");
        assert!(armour_packed(&[0xff], 9, &mut s).is_err());
        assert!(dearmour_packed("ww", 0, &mut [0u8; 1]).is_err());
    }

    #[test]
    fn bad_input() {
        let mut out = Vec::new();
        assert!(dearmour("1x", 0, &mut out).is_err());
        assert!(dearmour("1", 6, &mut out).is_err());
        assert!(dearmour("", 1, &mut out).is_err());
        assert!(armour(&[0, 2], &mut String::new()).is_err());
        assert!(fragments("AIVDM", Some('C'), 0, "1", 0).is_err());
        assert!(fragments("AIVDM", None, 10, "1", 0).is_err());
        assert!(fragments("AIVDM", None, 0, &"1".repeat(MAX_PAYLOAD_CHARS * 10), 0).is_err());
        for bad in [
            "!AIVDM,0,1,,A,1,0*",
            "!AIVDM,2,3,,A,1,0*",
            "!AIVDM,1,1,,C,1,0*",
            "!AIVDM,1,1,,A,1,6*",
            "!AIVDM,1,1,12,A,1,0*",
            "!AIVDM,1,1,,A,1x,0*",
            "!AIVDM,1,1,,A,1*",
            "!GPGGA,1,1,,A,1,0*",
        ] {
            let body = &bad[1..bad.len() - 1];
            let line = format!("{bad}{:02X}", crate::nmea::checksum(body.as_bytes()));
            let s = parse(&line).unwrap();
            assert!(Vdm::from_sentence(&s).is_err(), "{bad}");
        }
        // A second fragment without the first is refused, and the assembler recovers for the next message.
        let mut a = Assembler::new();
        let p2 = parse("!AIVDM,2,2,1,A,88888888880,2*25").unwrap();
        assert!(a.push(&Vdm::from_sentence(&p2).unwrap()).is_err());
        let one = parse("!AIVDM,1,1,,B,177KQJ5000G?tO`K>RA1wUbN0TKH,0*5C").unwrap();
        assert!(a
            .push(&Vdm::from_sentence(&one).unwrap())
            .unwrap()
            .is_some());
    }

    #[test]
    fn long_messages_split_and_rejoin() {
        let bits: Vec<u8> = (0..1008).map(|i| ((i * 7 + i / 3) % 2) as u8).collect();
        let mut payload = String::new();
        let fill = armour(&bits, &mut payload).unwrap();
        assert_eq!((payload.len(), fill), (168, 0));
        let sentences: Vec<_> = fragments("AIVDO", Some('B'), 7, &payload, fill)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(sentences.len(), 3);
        let mut a = Assembler::new();
        let mut got = None;
        for s in &sentences {
            assert!(s.len() <= MAX_LEN);
            let p = parse(s).unwrap();
            if let Some(m) = a.push(&Vdm::from_sentence(&p).unwrap()).unwrap() {
                assert!(m.own_ship);
                got = Some(m.bits.to_vec());
            }
        }
        assert_eq!(got.unwrap(), bits);
        // A message of zero bits is one empty fragment.
        let empty: Vec<_> = fragments("AIVDM", None, 0, "", 0)
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].as_str(), "!AIVDM,1,1,,,,0*67");
    }
}
