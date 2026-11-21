//! Messages, acknowledgements, rejects, bulletins and announcements (APRS 1.0.1 chapter 14), the reply-ack form of APRS 1.1
//! ("{MM}AA"), directed queries, and the telemetry set-up messages.

use super::telemetry::TelemetryMeta;
use super::text;
use crate::{Error, Result};
use crate::{Param, Part};

/// What a message carries.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Content {
    /// A text message.
    Text {
        /// The text.
        text: String,
        /// The message number (1 to 5 characters) when the sender requests an acknowledgement.
        id: Option<String>,
        /// The APRS 1.1 reply-ack: the number of a message from the addressee that this message also acknowledges. It is an
        /// empty string when the sender supports reply-ack but has nothing to acknowledge.
        reply_ack: Option<String>,
    },
    /// An acknowledgement of message `id`.
    Ack {
        /// The message number acknowledged.
        id: String,
        /// A reply-ack carried on the acknowledgement.
        reply_ack: Option<String>,
    },
    /// A reject of message `id`.
    Rej {
        /// The message number rejected.
        id: String,
    },
    /// A bulletin `BLN0` to `BLN9` (`id` is the digit) or a group bulletin `BLNnGROUP`.
    Bulletin {
        /// The bulletin line, `0` to `9`.
        id: char,
        /// The group name after the digit (empty for a general bulletin).
        group: String,
        /// The text.
        text: String,
    },
    /// An announcement `BLNA` to `BLNZ` (`id` is the letter).
    Announcement {
        /// The announcement identifier, `A` to `Z`.
        id: char,
        /// The text.
        text: String,
    },
    /// A National Weather Service bulletin (addressee `NWS-...`, `SKY-...`, `CWA-...`, `BOM-...`).
    Weather {
        /// The text.
        text: String,
    },
    /// A telemetry set-up message (the addressee is the station that sends the telemetry).
    Telemetry(TelemetryMeta),
    /// A directed query `?APRSx` (chapter 15).
    Query {
        /// The query, without the question mark.
        query: String,
    },
}

/// A message to an addressee.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Message {
    /// The addressee (up to nine characters, blanks removed).
    pub addressee: String,
    /// What it carries.
    pub content: Content,
}

/// Splits `text{MM}AA` or `text{MM`. Returns the text, the number and the reply-ack.
fn split_id(body: &str) -> (&str, Option<String>, Option<String>) {
    let Some(i) = body.rfind('{') else {
        return (body, None, None);
    };
    let tail = &body[i + 1..];
    let (no, ack) = match tail.split_once('}') {
        Some((a, b)) => (a, Some(b.to_string())),
        None => (tail, None),
    };
    if no.is_empty() || no.len() > 5 || !no.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return (body, None, None);
    }
    (&body[..i], Some(no.to_string()), ack)
}

/// Reads a message from the information field (it starts with `:`).
pub fn parse(info: &[u8]) -> Result<Message> {
    if info.len() < 11 || info[0] != b':' || info[10] != b':' {
        return Err(Error::Syntax(Part::MessageAddressee));
    }
    let addressee = text(&info[1..10]).trim_end().to_string();
    if addressee.is_empty() {
        return Err(Error::Syntax(Part::MessageAddressee));
    }
    let body = text(&info[11..]);
    let body = body.trim_end_matches(['\r', '\n']);
    let content = if let Some((id, reply_ack)) = ack_or_rej(body, "ack") {
        Content::Ack { id, reply_ack }
    } else if let Some((id, _)) = ack_or_rej(body, "rej") {
        Content::Rej { id }
    } else if let Some(q) = body.strip_prefix('?') {
        Content::Query {
            query: q.trim().to_string(),
        }
    } else if let Some(meta) = TelemetryMeta::parse(body) {
        Content::Telemetry(meta)
    } else if let Some(rest) = addressee.strip_prefix("BLN") {
        let (text_part, _, _) = split_id(body);
        let mut cs = rest.chars();
        match cs.next() {
            Some(c) if c.is_ascii_digit() => Content::Bulletin {
                id: c,
                group: cs.as_str().to_string(),
                text: text_part.to_string(),
            },
            Some(c) if c.is_ascii_uppercase() && rest.len() == 1 => Content::Announcement {
                id: c,
                text: text_part.to_string(),
            },
            _ => text_message(body),
        }
    } else if ["NWS-", "SKY-", "CWA-", "BOM-"]
        .iter()
        .any(|p| addressee.starts_with(p))
    {
        Content::Weather {
            text: split_id(body).0.to_string(),
        }
    } else {
        text_message(body)
    };
    Ok(Message { addressee, content })
}

/// `ackMM`, `ackMM}AA` (or the same with `rej`): the number (1 to 5 letters or digits) and the reply-ack. `None` when the text
/// is not of this form (for example "acknowledged").
fn ack_or_rej(body: &str, word: &str) -> Option<(String, Option<String>)> {
    let head = body.get(..3)?;
    if !head.eq_ignore_ascii_case(word) {
        return None;
    }
    let rest = &body[3..];
    let (id, reply_ack) = match rest.split_once('}') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let ok =
        |s: &str| !s.is_empty() && s.len() <= 5 && s.bytes().all(|c| c.is_ascii_alphanumeric());
    if !ok(id) || reply_ack.is_some_and(|r| !r.is_empty() && !ok(r)) {
        return None;
    }
    Some((id.to_string(), reply_ack.map(str::to_string)))
}

fn text_message(body: &str) -> Content {
    let (t, id, reply_ack) = split_id(body);
    Content::Text {
        text: t.to_string(),
        id,
        reply_ack,
    }
}

/// Appends the information field of a text message: `:ADDRESSEE:text{id`. The addressee is padded to nine characters. An error
/// if the addressee is longer than nine characters, the text holds `|`, `~` or `{`, or is longer than 67 characters, or the
/// number is longer than 5 characters (chapter 14).
pub fn encode_text(addressee: &str, text: &str, id: Option<&str>, out: &mut String) -> Result<()> {
    if addressee.is_empty() || addressee.len() > 9 || !addressee.is_ascii() {
        return Err(Error::BadParameter(Param::Addressee));
    }
    if text.len() > 67 || text.contains(['|', '~', '{']) {
        return Err(Error::BadParameter(Param::MessageText));
    }
    if id.is_some_and(|i| {
        i.is_empty() || i.len() > 5 || !i.bytes().all(|c| c.is_ascii_alphanumeric())
    }) {
        return Err(Error::BadParameter(Param::MessageNumber));
    }
    out.push(':');
    out.push_str(addressee);
    for _ in addressee.len()..9 {
        out.push(' ');
    }
    out.push(':');
    out.push_str(text);
    if let Some(i) = id {
        out.push('{');
        out.push_str(i);
    }
    Ok(())
}

/// Appends the information field of an acknowledgement `:ADDRESSEE:ackid`.
pub fn encode_ack(addressee: &str, id: &str, out: &mut String) -> Result<()> {
    if id.is_empty() || id.len() > 5 {
        return Err(Error::BadParameter(Param::MessageNumber));
    }
    encode_text(addressee, "", None, out)?;
    out.push_str("ack");
    out.push_str(id);
    Ok(())
}
