//! The owned, fully read ACARS message.

use super::assemble::{Head, Joined, Quality, MAX_JOINED};
use super::{printable_into, sublabel, Block, End, ACK, NAK};
use crate::adsc::{self, Adsc};
use crate::arinc622::{self, Imi};
use crate::cpdlc::{self, Cpdlc};
use crate::formats::{self, FormatReading};
use crate::labels;
use crate::readings::{self, MediaAdvisory, Oooi};
use crate::textpos::{self, TextPosition};
use sdrkit_util_geo::Position;

/// The technical acknowledgement of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tak {
    /// NAK: nothing is acknowledged.
    Nak,
    /// The block id acknowledged.
    Block(u8),
    /// ACK, or another character.
    Other(u8),
}

impl Tak {
    /// The acknowledgement of a TAK character.
    pub fn of(c: u8) -> Tak {
        match c {
            NAK => Tak::Nak,
            ACK => Tak::Other(ACK),
            c if c.is_ascii_alphanumeric() => Tak::Block(c),
            c => Tak::Other(c),
        }
    }
}

/// What the message's text was read as.
#[derive(Clone, Debug, PartialEq)]
pub enum Reading {
    /// Nothing was read.
    None,
    /// An OOOI report of a Q label.
    Oooi(Oooi),
    /// A media advisory (label SA).
    MediaAdvisory(MediaAdvisory),
    /// A position written as text.
    Position(TextPosition),
    /// An ADS-C message.
    Adsc(Adsc),
    /// A FANS-1/A CPDLC message.
    Cpdlc(Cpdlc),
    /// A plain-text airline format ([`formats`]).
    Format(FormatReading),
}

/// An ACARS message: one block, or several joined, with its fixed-format reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// The mode character.
    pub mode: char,
    /// The registration (the padding dots removed).
    pub registration: String,
    /// The technical acknowledgement.
    pub tak: Tak,
    /// The label (`_d` form).
    pub label: String,
    /// The label's meaning ([`labels::meaning`]).
    pub label_meaning: Option<&'static str>,
    /// The block id of the first block.
    pub block_id: char,
    /// Aircraft to ground.
    pub downlink: bool,
    /// Downlink message sequence number (`M01`).
    pub msn: String,
    /// Downlink flight id.
    pub flight: String,
    /// H1 sublabel.
    pub sublabel: Option<String>,
    /// H1 message function identifier.
    pub mfi: Option<String>,
    /// The printable text (control characters as `.`, line breaks kept).
    pub text: String,
    /// Blocks joined.
    pub blocks: u32,
    /// A block is missing.
    pub incomplete: bool,
    /// The text reached the cap.
    pub cut: bool,
    /// Bits repaired, and whether every BCS was good.
    pub quality: Quality,
    /// The fixed-format reading.
    pub reading: Reading,
}

impl Message {
    /// The message of one block (no joining).
    pub fn from_block(b: &Block, quality: Quality) -> Message {
        let mut m = Self::make(&Head::of(b, 0), b.app_text(), quality);
        m.incomplete = b.end == End::Etb;
        m
    }

    /// The message of a joined result of [`super::assemble::Assembler`].
    pub fn from_joined<X>(j: &Joined<X>) -> Message {
        let mut m = Self::make(&j.body.head, &j.body.text, j.body.quality);
        m.blocks = j.parts;
        m.incomplete = !j.complete;
        m.cut |= j.cut;
        m
    }

    fn make(h: &Head, text: &str, quality: Quality) -> Message {
        let label = h.label_str().to_owned();
        let downlink = h.downlink();
        let sub = (label == "H1").then(|| sublabel(text, downlink)).flatten();
        let mut printable = String::with_capacity(text.len().min(MAX_JOINED));
        let cut = printable_into(text.as_bytes(), MAX_JOINED, &mut printable);
        let registration = h.registration().to_owned();
        let prefix = if downlink && !h.msn().is_empty() {
            format!("{}{}{:<6}", h.msn(), h.sequence as char, h.flight())
        } else {
            String::new()
        };
        let reading = read_text(&label, &prefix, text, downlink, &registration);
        Message {
            mode: h.mode as char,
            label_meaning: labels::meaning(&label),
            label,
            tak: Tak::of(h.tak),
            block_id: h.block_id as char,
            downlink,
            msn: h.msn().to_owned(),
            flight: h.flight().to_owned(),
            sublabel: sub.map(|s| s.sublabel.to_owned()),
            mfi: sub.and_then(|s| s.mfi).map(str::to_owned),
            text: printable,
            blocks: 1,
            incomplete: false,
            cut,
            quality,
            registration,
            reading,
        }
    }

    /// The position the message provides (an ADS-C report, a text position, or the position of a plain-text format in a
    /// downlink), with its altitude in feet when known.
    pub fn position(&self) -> Option<(Position, Option<f64>)> {
        match &self.reading {
            Reading::Position(p) => Some((p.position, p.alt_ft)),
            Reading::Adsc(a) => Some((a.position()?, a.report().map(|r| r.alt_ft))),
            Reading::Format(f) if self.downlink => Some((f.position?, f.alt_ft)),
            _ => None,
        }
    }
}

/// The one-line reading of a message's fixed format (an OOOI report, a media advisory, a position, an ADS-C report, a CPDLC
/// envelope or an airline format), its kind ("Position report", "ADS-C", ...) and its category ([`formats::Category`]); empty
/// texts when nothing was read.
pub fn reading_of(m: &Message) -> (String, &'static str, &'static str) {
    use formats::Category;
    match &m.reading {
        Reading::None => (String::new(), "", ""),
        Reading::Oooi(o) => (o.to_string(), "OOOI report", Category::Oooi.name()),
        Reading::MediaAdvisory(a) => (a.to_string(), "Media advisory", Category::Other.name()),
        Reading::Position(p) => {
            let alt = p.alt_ft.map(|a| format!(" {a:.0} ft")).unwrap_or_default();
            let text = format!("position {:.4} {:.4}{alt}", p.position.lat_deg, p.position.lon_deg);
            (text, "Position report", Category::Position.name())
        }
        Reading::Adsc(a) => {
            let text = if !a.crc_ok {
                "ADS-C, envelope check failed".to_string()
            } else if let Some(r) = a.report() {
                format!("ADS-C report {:.4} {:.4} {:.0} ft", r.lat_deg, r.lon_deg, r.alt_ft)
            } else if a.imi == Imi::Dis {
                "ADS-C disconnect".to_string()
            } else {
                "ADS-C".to_string()
            };
            (text, "ADS-C", Category::Position.name())
        }
        Reading::Cpdlc(c) => {
            let s = c.summary();
            let text = if s.is_empty() {
                c.description()
            } else {
                format!("{}: {s}", c.description())
            };
            (text, c.kind(), Category::Other.name())
        }
        Reading::Format(f) => (f.line(), f.kind, f.category.name()),
    }
}

/// Reads the fixed formats of an application text: OOOI reports (Q labels), the media advisory (SA), ADS-C and CPDLC envelopes,
/// and the plain-text airline formats ([`formats`], read with `prefix`, the downlink's message number, sequence character and
/// flight, in front). In a downlink that none of these read, it reads a position written as text.
pub fn read_text(label: &str, prefix: &str, text: &str, downlink: bool, registration: &str) -> Reading {
    if label == "QQ" {
        if let Some(f) = formats::read_qq(text) {
            return Reading::Format(f);
        }
    }
    if label.starts_with('Q') {
        if let Some(o) = readings::oooi(label, text) {
            return Reading::Oooi(o);
        }
    }
    if label == "SA" {
        if let Some(m) = readings::media_advisory(text) {
            return Reading::MediaAdvisory(m);
        }
    }
    if let Some(e) = arinc622::find(text, registration) {
        let r = match e.imi {
            Imi::Ads | Imi::Dis => adsc::decode(text, downlink, registration).map(Reading::Adsc),
            _ => cpdlc::decode(text, downlink, registration).map(Reading::Cpdlc),
        };
        if let Some(r) = r {
            return r;
        }
    }
    if let Some(f) = formats::read_message(label, prefix, text) {
        return Reading::Format(f);
    }
    if downlink {
        if let Some(p) = textpos::read(label, text) {
            return Reading::Position(p);
        }
    }
    Reading::None
}
