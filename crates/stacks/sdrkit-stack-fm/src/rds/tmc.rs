//! RDS-TMC (ISO 14819-1): the service announcement in type 3A and the traffic messages in type 8A, single-group and multi-group.
//!
//! Location codes stay as numbers (they need the country's location table). Messages are kept newest first, at most 64; a message is
//! identified by its event, location and direction, and a repeat of it counts up and refreshes its expiry.

use std::collections::VecDeque;

/// The TMC service, from type 3A with AID 0xCD46 or 0xCD47.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TmcService {
    /// Location table number (variant 0).
    pub ltn: Option<u8>,
    /// Alternative-frequency indicator (variant 0).
    pub afi: bool,
    /// Mode: false = basic, true = enhanced (variant 0).
    pub enhanced_mode: bool,
    /// Message geographical scope bits I, N, R, U (variant 0).
    pub scope: u8,
    /// Service identifier (variant 1).
    pub sid: Option<u8>,
}

/// One TMC message.
#[derive(Clone, Debug, PartialEq)]
pub struct TmcMessage {
    /// Event code (11 bits).
    pub event: u16,
    /// Location code (a number in the location table).
    pub location: u16,
    /// Direction bit (false = positive, true = negative).
    pub negative_direction: bool,
    /// Extent (number of steps along the road, 0 to 7, plus any length from the optional content).
    pub extent: u8,
    /// Diversion advised.
    pub diversion: bool,
    /// Duration and persistence code (single-group messages; 0 to 7).
    pub duration: u8,
    /// Further events from the optional content (label 9).
    pub extra_events: Vec<u16>,
    /// The optional content as (label, value) pairs.
    pub fields: Vec<(u8, u32)>,
    /// A multi-group message.
    pub multi_group: bool,
    /// Times received.
    pub repeats: u32,
    /// Wall-clock time of the first and of the last reception, seconds since 1970.
    pub first_s: f64,
    /// Wall-clock time of the last reception.
    pub last_s: f64,
    /// When the message expires, seconds since 1970.
    pub expires_s: f64,
}

/// Bits of each optional-content label (ISO 14819-1, multi-group free format).
const LABEL_BITS: [u32; 16] = [3, 3, 5, 5, 5, 8, 8, 8, 8, 11, 16, 16, 16, 16, 0, 0];

/// How long a message lasts for a duration code, in seconds (a dynamic event: 15 min, 15 min, 30 min, 1 h, 2 h, 3 h, 4 h, the rest
/// of the day, taken as 24 h). A multi-group message without a duration label lasts as code 0.
pub fn persistence_s(duration: u8) -> f64 {
    const MIN: [f64; 8] = [15.0, 15.0, 30.0, 60.0, 120.0, 180.0, 240.0, 1440.0];
    MIN[(duration & 7) as usize] * 60.0
}

#[derive(Clone, Debug, Default)]
struct Pending {
    ci: u8,
    event: u16,
    location: u16,
    negative: bool,
    extent: u8,
    bits: Vec<bool>,
    next_second: bool,
    last_gsi: Option<u8>,
}

/// The TMC decoder.
#[derive(Clone, Debug, Default)]
pub struct Tmc {
    /// The service announcement.
    pub service: TmcService,
    /// Messages, newest first, at most 64.
    pub messages: VecDeque<TmcMessage>,
    /// Tuning-information groups seen (not decoded further).
    pub tuning_groups: u64,
    pending: Option<Pending>,
}

impl Tmc {
    /// The 3A announcement's block C.
    pub fn announcement(&mut self, c: u16) {
        match c >> 14 {
            0 => {
                self.service.ltn = Some(((c >> 6) & 0x3F) as u8);
                self.service.afi = c & 0x20 != 0;
                self.service.enhanced_mode = c & 0x10 != 0;
                self.service.scope = (c & 0x0F) as u8;
            }
            1 => self.service.sid = Some(((c >> 6) & 0x3F) as u8),
            _ => {}
        }
    }

    /// A type 8A group (blocks B, C, D; C must be usable).
    pub fn group(&mut self, b: u16, c: u16, d: u16, wall_s: f64) {
        self.expire(wall_s);
        if b & 0x10 != 0 {
            self.tuning_groups += 1;
            return;
        }
        if b & 0x08 != 0 {
            // Single group.
            let m = TmcMessage {
                event: c & 0x07FF,
                location: d,
                negative_direction: c & 0x4000 != 0,
                extent: ((c >> 11) & 7) as u8,
                diversion: c & 0x8000 != 0,
                duration: (b & 7) as u8,
                extra_events: Vec::new(),
                fields: Vec::new(),
                multi_group: false,
                repeats: 1,
                first_s: wall_s,
                last_s: wall_s,
                expires_s: wall_s + persistence_s((b & 7) as u8),
            };
            self.store(m);
            return;
        }
        let ci = (b & 7) as u8;
        if c & 0x8000 != 0 {
            // First group of a multi-group message.
            self.pending = Some(Pending {
                ci,
                event: c & 0x07FF,
                location: d,
                negative: c & 0x4000 != 0,
                extent: ((c >> 11) & 7) as u8,
                bits: Vec::with_capacity(112),
                next_second: true,
                last_gsi: None,
            });
            return;
        }
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        let second = c & 0x4000 != 0;
        let gsi = ((c >> 12) & 3) as u8;
        if p.ci != ci || second != p.next_second || p.last_gsi.is_some_and(|g| gsi + 1 != g) {
            self.pending = None;
            return;
        }
        p.next_second = false;
        p.last_gsi = Some(gsi);
        let free = ((c as u32 & 0x0FFF) << 16) | d as u32;
        p.bits.extend((0..28).rev().map(|i| free >> i & 1 == 1));
        if gsi == 0 {
            let p = self.pending.take().expect("pending");
            let fields = parse_free_format(&p.bits);
            let duration = fields.iter().find(|f| f.0 == 0).map_or(0, |f| f.1 as u8);
            let diversion = fields.iter().any(|f| f.0 == 1 && f.1 == 5);
            let m = TmcMessage {
                event: p.event,
                location: p.location,
                negative_direction: p.negative,
                extent: p.extent,
                diversion,
                duration,
                extra_events: fields
                    .iter()
                    .filter(|f| f.0 == 9)
                    .map(|f| f.1 as u16)
                    .collect(),
                fields,
                multi_group: true,
                repeats: 1,
                first_s: wall_s,
                last_s: wall_s,
                expires_s: wall_s + persistence_s(duration),
            };
            self.store(m);
        }
    }

    fn store(&mut self, mut m: TmcMessage) {
        if let Some(i) = self.messages.iter().position(|o| {
            o.event == m.event
                && o.location == m.location
                && o.negative_direction == m.negative_direction
        }) {
            let old = self.messages.remove(i).expect("present");
            m.repeats = old.repeats + 1;
            m.first_s = old.first_s;
        }
        self.messages.push_front(m);
        self.messages.truncate(64);
    }

    fn expire(&mut self, wall_s: f64) {
        self.messages.retain(|m| m.expires_s >= wall_s);
    }
}

/// Splits multi-group optional content into (label, value) pairs; stops at the end or at a label that does not fit.
pub fn parse_free_format(bits: &[bool]) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut i = 0;
    let take = |i: &mut usize, n: u32| -> Option<u32> {
        if *i + n as usize > bits.len() {
            return None;
        }
        let v = bits[*i..*i + n as usize]
            .iter()
            .fold(0u32, |a, &b| (a << 1) | b as u32);
        *i += n as usize;
        Some(v)
    };
    while let Some(label) = take(&mut i, 4) {
        // Trailing zeros fill the last group.
        if label == 0 && bits[i..].iter().all(|b| !b) {
            break;
        }
        if label == 15 {
            break;
        }
        let Some(v) = take(&mut i, LABEL_BITS[label as usize]) else {
            break;
        };
        out.push((label as u8, v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_multi_group_messages() {
        let mut t = Tmc::default();
        t.announcement((12 << 6) | 0x20 | 0x08); // variant 0
        t.announcement((1 << 14) | (5 << 6));
        assert_eq!(
            t.service,
            TmcService {
                ltn: Some(12),
                afi: true,
                enhanced_mode: false,
                scope: 8,
                sid: Some(5)
            }
        );
        // Single group: event 101, location 12345, direction negative, extent 2, duration 3.
        let b = 0x8000 | 0x08 | 3;
        let c = 0x4000 | (2 << 11) | 101;
        t.group(b, c, 12345, 1000.0);
        t.group(b, c, 12345, 1010.0);
        assert_eq!(t.messages.len(), 1);
        let m = &t.messages[0];
        assert_eq!(
            (
                m.event,
                m.location,
                m.negative_direction,
                m.extent,
                m.duration,
                m.repeats
            ),
            (101, 12345, true, 2, 3, 2)
        );
        assert_eq!(m.expires_s, 1010.0 + 3600.0);
        // Multi-group: first group, then one more group with label 9 (extra event 401) and label 0 (duration 2).
        let ci = 5u16;
        t.group(0x8000 | ci, 0x8000 | (1 << 11) | 700, 222, 1020.0);
        // Free format: 1001 + 11 bits of 401, 0000 + 3 bits of 2 = 22 bits, then zeros; 28 bits: C low 12, D 16.
        let bits: u32 = (((9u32 << 11) | 401) << 7 | 2) << 6;
        let (c2, d2) = (0x4000 | ((bits >> 16) & 0x0FFF) as u16, bits as u16); // second group, GSI 0
        t.group(0x8000 | ci, c2, d2, 1021.0);
        let m = &t.messages[0];
        assert!(m.multi_group);
        assert_eq!(
            (m.event, m.location, m.extra_events.clone(), m.duration),
            (700, 222, vec![401], 2)
        );
        // Expiry.
        t.group(0x8000 | 0x10, 0, 0, 1022.0 + 86_400.0);
        assert!(t.messages.is_empty());
        assert_eq!(t.tuning_groups, 1);
    }
}
