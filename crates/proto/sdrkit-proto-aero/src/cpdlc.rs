//! FANS-1/A CPDLC, Controller-Pilot Data Link Communications (RTCA DO-258, carried in ARINC 622 envelopes).
//!
//! CPDLC messages travel in four envelopes ([`crate::arinc622::Imi`]): `AT1` (a message), `CR1` (connect request), `CC1`
//! (connect confirm) and `DR1` (disconnect request). The binary message is ASN.1 in unaligned PER (packed encoding rules, ITU-T
//! X.691):
//!
//! ```text
//! SEQUENCE {
//!   header SEQUENCE { msgIdentificationNumber INTEGER (0..63),
//!                     msgReferenceNumber INTEGER (0..63) OPTIONAL,
//!                     timestamp SEQUENCE { hours (0..23), minutes (0..59), seconds (0..59) } OPTIONAL },
//!   element CHOICE { 183 uplink elements } or CHOICE { 129 downlink elements (81 to 128 reserved) },
//!   further elements SEQUENCE SIZE (1..4) OF element OPTIONAL }
//! ```
//!
//! Every element and parameter type is a table ([`types`], [`elements`]) read by the walker of
//! [`sdrkit_util_bits::per_schema`]. [`read`] gives the header and the elements with their typed parameters; [`Message::text`]
//! writes them as indented lines.
//!
//! Standard: RTCA DO-258 / EUROCAE ED-100; ARINC 622; ITU-T X.691.
//!
//! ```
//! use sdrkit_proto_aero::cpdlc::{read, write, Element, Header, Message};
//! use sdrkit_util_bits::per_schema::Value;
//! // Uplink element 20, CLIMB TO AND MAINTAIN, flight level 350 (Altitude alternative 6, coded 350).
//! let m = Message {
//!     header: Header { msg_id: 7, msg_ref: None, timestamp: None },
//!     elements: vec![Element { number: 20, param: Value::Choice(6, Box::new(Value::Int(350))) }],
//! };
//! let mut bin = Vec::new();
//! write(&m, true, &mut bin).unwrap();
//! assert_eq!(bin, [0x03, 0x8A, 0x65, 0x00]);
//! let back = read(&bin, true).unwrap();
//! assert_eq!(back, m);
//! assert!(back.text(true).contains("Flight level: 350"));
//! ```

pub mod elements;
pub mod types;

use crate::arinc622::{self, Imi};
use crate::error::{Error, Item, Result};
use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
use sdrkit_util_bits::per_schema::{print_field, walk, Field, Ty, Value};
use std::fmt::Write as _;

/// The number of uplink message elements (uM0 to uM182).
pub const UPLINK_ELEMENTS: usize = 183;
/// The number of downlink message elements (dM0 to dM128; 81 to 128 are reserved).
pub const DOWNLINK_ELEMENTS: usize = 129;

/// The message header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The message identification number, 0 to 63.
    pub msg_id: u8,
    /// The number of the message this one answers.
    pub msg_ref: Option<u8>,
    /// The time: hours, minutes, seconds.
    pub timestamp: Option<(u8, u8, u8)>,
}

/// One message element: its number and its parameter as read with the element's type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    /// The uM or dM number.
    pub number: u16,
    /// The parameter ([`Value::Null`] for an element without one).
    pub param: Value,
}

/// A decoded CPDLC message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The header.
    pub header: Header,
    /// The elements: one to five.
    pub elements: Vec<Element>,
}

/// The table entry of element `n` of a direction: its text as label, its parameter as type.
pub fn element(uplink: bool, n: u16) -> Option<&'static Field> {
    if uplink {
        elements::UPLINK.get(usize::from(n))
    } else {
        elements::DOWNLINK.get(usize::from(n))
    }
}

/// The text of element `n` of a direction (with its parameters in brackets); `None` for a number out of range or reserved.
pub fn element_text(uplink: bool, n: u16) -> Option<&'static str> {
    element(uplink, n)
        .map(|f| f.label)
        .filter(|t| !t.is_empty())
}

fn message_ty(uplink: bool) -> &'static Ty {
    if uplink {
        &elements::UPLINK_MESSAGE
    } else {
        &elements::DOWNLINK_MESSAGE
    }
}

fn small(v: Option<&Value>) -> u8 {
    v.and_then(Value::int).unwrap_or(0) as u8
}

impl Message {
    fn to_value(&self) -> Value {
        let h = &self.header;
        let ts = h.timestamp.map(|(a, b, c)| {
            Value::Seq(
                [a, b, c]
                    .iter()
                    .map(|&x| Some(Value::Int(i64::from(x))))
                    .collect(),
            )
        });
        let header = Value::Seq(vec![
            Some(Value::Int(i64::from(h.msg_id))),
            h.msg_ref.map(|r| Value::Int(i64::from(r))),
            ts,
        ]);
        let el = |e: &Element| Value::Choice(usize::from(e.number), Box::new(e.param.clone()));
        let first = self.elements.first().map(el).unwrap_or_default();
        let more: Vec<Value> = self.elements.iter().skip(1).map(el).collect();
        Value::Seq(vec![
            Some(header),
            Some(first),
            (!more.is_empty()).then_some(Value::List(more)),
        ])
    }

    fn from_value(v: &Value) -> Option<Message> {
        let h = v.get(0)?;
        let header = Header {
            msg_id: small(h.get(0)),
            msg_ref: h.get(1).map(|r| small(Some(r))),
            timestamp: h
                .get(2)
                .map(|t| (small(t.get(0)), small(t.get(1)), small(t.get(2)))),
        };
        let el = |x: &Value| match x {
            Value::Choice(i, p) => Some(Element {
                number: *i as u16,
                param: (**p).clone(),
            }),
            _ => None,
        };
        let mut elements = vec![el(v.get(1)?)?];
        if let Some(Value::List(more)) = v.get(2) {
            for x in more {
                elements.push(el(x)?);
            }
        }
        Some(Message { header, elements })
    }

    /// Appends the decoded text: the direction, the header, then each element's text and its parameters as "label: value"
    /// lines, one space of indent a level.
    pub fn write_text(&self, uplink: bool, out: &mut String) {
        out.push_str(if uplink {
            "CPDLC Uplink Message:\n"
        } else {
            "CPDLC Downlink Message:\n"
        });
        let h = &self.header;
        let _ = writeln!(out, " Header:\n  Msg ID: {}", h.msg_id);
        if let Some(r) = h.msg_ref {
            let _ = writeln!(out, "  Msg Ref: {r}");
        }
        if let Some((a, b, c)) = h.timestamp {
            let _ = writeln!(out, "  Timestamp: {a:02}:{b:02}:{c:02}");
        }
        out.push_str(" Message data:\n");
        for e in &self.elements {
            let Some(f) = element(uplink, e.number) else {
                continue;
            };
            if f.label.is_empty() {
                let _ = writeln!(out, "  (reserved {})", e.number);
                continue;
            }
            let _ = writeln!(out, "  {}", f.label);
            print_field(&Field::new("", f.ty), &e.param, 3, out);
        }
    }

    /// The decoded text ([`Message::write_text`]).
    pub fn text(&self, uplink: bool) -> String {
        let mut s = String::new();
        self.write_text(uplink, &mut s);
        s
    }

    /// The text of the first element, unfilled.
    pub fn summary(&self, uplink: bool) -> &'static str {
        self.elements
            .first()
            .and_then(|e| element_text(uplink, e.number))
            .unwrap_or("")
    }
}

/// Reads a binary CPDLC message (`uplink` is the direction). An error when the reader fails, or when more than the padding
/// to a whole octet (fewer than 8 bits, all zero) is left over.
pub fn read(data: &[u8], uplink: bool) -> Result<Message> {
    let mut p = PerReader::new(data);
    let mut v = Value::Null;
    walk(&mut p, message_ty(uplink), &mut v);
    if p.failed() {
        return Err(Error::Short);
    }
    let left = p.left();
    if left >= 8 {
        return Err(Error::BadField(Item::CpdlcHeaderOrElement));
    }
    let mut pad = 0;
    p.bits(&mut pad, left as u32);
    if pad != 0 {
        return Err(Error::BadField(Item::CpdlcHeaderOrElement));
    }
    Message::from_value(&v).ok_or(Error::Short)
}

/// Appends the binary form of `m` to `out`, padded to whole octets. An error when a value does not fit its type, an element
/// number is out of range, or the message has no element or more than five.
pub fn write(m: &Message, uplink: bool, out: &mut Vec<u8>) -> Result<()> {
    let count = if uplink {
        UPLINK_ELEMENTS
    } else {
        DOWNLINK_ELEMENTS
    };
    if m.elements.is_empty()
        || m.elements.len() > 5
        || m.elements.iter().any(|e| usize::from(e.number) >= count)
    {
        return Err(Error::BadField(Item::CpdlcHeaderOrElement));
    }
    let mut v = m.to_value();
    let mut bin = Vec::new();
    let mut w = PerWriter::new(&mut bin);
    walk(&mut w, message_ty(uplink), &mut v);
    if w.failed() {
        return Err(Error::BadField(Item::CpdlcHeaderOrElement));
    }
    w.finish();
    out.extend_from_slice(&bin);
    Ok(())
}

/// What an envelope's body gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// No message body (a connect confirm or disconnect without a message).
    Empty,
    /// The body decoded completely.
    Decoded(Message),
    /// The body did not decode.
    Undecoded,
}

/// A CPDLC message found in an ACARS text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpdlc {
    /// The ground facility.
    pub ground: String,
    /// `AT1`, `CR1`, `CC1` or `DR1`.
    pub imi: Imi,
    /// The aircraft registration (leading dots and trailing dots and spaces removed).
    pub aircraft: String,
    /// The envelope CRC is good.
    pub crc_ok: bool,
    /// Read with the uplink set (ground to air).
    pub uplink: bool,
    /// The body.
    pub body: Body,
}

impl Cpdlc {
    /// The name of what the envelope carries.
    pub fn kind(&self) -> &'static str {
        match self.imi {
            Imi::Cr1 => "CPDLC connect request",
            Imi::Cc1 => "CPDLC connect confirm",
            Imi::Dr1 => "CPDLC disconnect request",
            _ => "CPDLC message",
        }
    }

    /// The body decoded completely.
    pub fn parsed(&self) -> bool {
        matches!(self.body, Body::Decoded(_))
    }

    /// The decoded message, when there is one.
    pub fn message(&self) -> Option<&Message> {
        match &self.body {
            Body::Decoded(m) => Some(m),
            _ => None,
        }
    }

    /// The one-line description: "FANS-1/A CPDLC message (AT1), ground facility G, aircraft R", and ", CRC FAILED" when the
    /// check fails on a body that is not empty.
    pub fn description(&self) -> String {
        let mut s = format!(
            "FANS-1/A {} ({}), ground facility {}, aircraft {}",
            self.kind(),
            self.imi.as_str(),
            self.ground,
            self.aircraft
        );
        if !self.crc_ok && self.body != Body::Empty {
            s.push_str(", CRC FAILED");
        }
        s
    }

    /// The decoded text without the description: the message lines, "(no message body)" or "(the message body did not
    /// decode)".
    pub fn text(&self) -> String {
        match &self.body {
            Body::Empty => "(no message body)".into(),
            Body::Undecoded => "(the message body did not decode)".into(),
            Body::Decoded(m) => m.text(self.uplink),
        }
    }

    /// The text of the first element, empty when the body did not decode.
    pub fn summary(&self) -> &'static str {
        self.message().map(|m| m.summary(self.uplink)).unwrap_or("")
    }
}

/// Finds and reads the first CPDLC envelope (`.AT1`, `.CR1`, `.CC1`, `.DR1`) in an ACARS application text. `downlink` is the
/// block's direction: a downlink is read with the downlink set, an uplink with the uplink set.
pub fn decode(text: &str, downlink: bool, registration: &str) -> Option<Cpdlc> {
    let e = arinc622::find_cpdlc(text, registration)?;
    let uplink = !downlink;
    let mut bin = vec![0u8; e.hex.len() / 2];
    let body = match e.binary(&mut bin) {
        Ok([]) => Body::Empty,
        Ok(d) => match read(d, uplink) {
            Ok(m) => Body::Decoded(m),
            Err(_) => Body::Undecoded,
        },
        Err(_) if e.hex.len() < 4 => Body::Empty,
        Err(_) => Body::Undecoded,
    };
    Some(Cpdlc {
        ground: e.ground.to_owned(),
        imi: e.imi,
        aircraft: e
            .address
            .trim_start_matches('.')
            .trim_end_matches(['.', ' '])
            .to_owned(),
        crc_ok: e.crc_ok,
        uplink,
        body,
    })
}

/// Appends an `AT1` envelope of `m` to `out` (`/GROUND.AT1.ADDRESS` with hex and CRC). An error as for [`write`] and
/// [`arinc622::build`].
pub fn build(ground: &str, aircraft: &str, m: &Message, uplink: bool, out: &mut String) -> Result<()> {
    let mut bin = Vec::new();
    write(m, uplink, &mut bin)?;
    out.push('/');
    arinc622::build(ground, Imi::At1, aircraft, &bin, out)
}
