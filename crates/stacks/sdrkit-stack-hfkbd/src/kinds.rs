//! The typed items of this family: decoded characters, Hell pixel columns and FSQ messages.

use crate::fsq::Trigger;
use crate::typed::{FsqMessageEvent, FSQ_MESSAGE};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::{EventDef, Record};
use sdrkit_graph::Item;

/// One decoded character (`text.chars`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextChar {
    /// The byte as sent (bytes 128 to 255 pass through, so UTF-8 text survives).
    pub byte: u8,
    /// 0 to 100 (100 when the stack has no measure).
    pub confidence: u8,
    /// An alternative reading (CW), 0 for none.
    pub alt: u8,
    /// The signal it belongs to (stacks that decode several signals), 0 otherwise.
    pub signal: u32,
}

impl Item for TextChar {
    const KIND: &'static str = "text.chars";
}

/// Hell pixel columns (`picture.columns`): 14 grey pixels per column, `L008`, bottom row first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HellColumn {
    /// The pixels, 0 (no tone) to 255 (full tone), index 0 = the bottom row.
    pub pixels: [u8; 14],
}

impl Item for HellColumn {
    const KIND: &'static str = "picture.columns";
}

/// A parsed FSQ line (`fsq.message`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FsqMessage {
    /// Signal time.
    pub time: f64,
    /// The sender, when the line starts with "CALL:".
    pub from: Option<String>,
    /// The two hex digits equal the CRC-8 of the callsign.
    pub crc_ok: bool,
    /// The addressee (upper case), "allcall" or "cqcqcq".
    pub to: Option<String>,
    /// The trigger.
    pub trigger: Option<Trigger>,
    /// The text after the trigger.
    pub text: String,
    /// Addressed to my call, allcall or cqcqcq (only when my call is set).
    pub to_me: bool,
}

impl Item for FsqMessage {
    const KIND: &'static str = "fsq.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(FSQ_MESSAGE.def())
    }
    fn write_fields(&self, r: &mut Record) {
        FsqMessageEvent {
            source: self.from.as_deref(),
            target: self.to.as_deref(),
            text: &self.text,
            family: Family::Message,
            crc_ok: self.crc_ok,
            trigger: self.trigger,
            to_me: self.to_me,
        }
        .write(r);
    }
}
