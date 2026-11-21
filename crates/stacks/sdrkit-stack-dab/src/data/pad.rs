//! Programme associated data: F-PAD, X-PAD content indicators and subfields, the data group length indicator and the MSC data
//! groups carried in X-PAD. It cuts the dynamic label segments out of the X-PAD and returns them to the shared label decoder.
//!
//! Standard: EN 300 401 clause 7.4.

use crate::fec::crc_ok;
use sdrkit_core::media::Packet;
use sdrkit_proto_bcdata::label::{self, Update};
use sdrkit_proto_bcdata::tags::TagSet;

/// The X-PAD subfield lengths of the variable size X-PAD content indicator (EN 300 401 table on X-PAD lengths).
pub const XPAD_LENGTHS: [usize; 8] = [4, 6, 8, 12, 16, 24, 32, 48];

/// The PAD of one audio frame or access unit.
#[derive(Clone, Debug, PartialEq)]
pub struct PadItem {
    /// The two F-PAD bytes.
    pub fpad: [u8; 2],
    /// The X-PAD bytes in logical order (the bytes before the F-PAD, reversed), as shared bytes. For MPEG Layer II this is the
    /// most the X-PAD can be; its real length follows from the content indicators.
    pub xpad: Packet,
    /// The X-PAD length is exactly the length of `xpad` (DAB+).
    pub exact: bool,
}

impl PadItem {
    /// A PAD with its own copy of `xpad`, the X-PAD bytes in logical order.
    pub fn new(fpad: [u8; 2], xpad: &[u8], exact: bool) -> Self {
        PadItem {
            fpad,
            xpad: Packet::new(xpad.into(), 0.0),
            exact,
        }
    }
}

/// The dynamic label state published by the PAD decoder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    /// The label text (clean UTF-8); empty when none.
    pub text: String,
    /// Its toggle bit.
    pub toggle: bool,
    /// Its character set.
    pub charset: u8,
    /// DL Plus tags.
    pub tags: TagSet,
    /// DL Plus item running.
    pub item_running: bool,
    /// DL Plus item toggle.
    pub item_toggle: bool,
    /// Counts labels completed.
    pub count: u64,
}

/// An MSC data group carried in X-PAD, with the X-PAD application type it came in.
#[derive(Clone, Debug, PartialEq)]
pub struct XpadDataGroup {
    /// The application type of its start subfield.
    pub app_type: u8,
    /// The data group bytes (header, session header, data and CRC).
    pub data: Vec<u8>,
}

/// The PAD decoder: one per selected service.
#[derive(Clone, Debug, Default)]
pub struct PadDecoder {
    last_app: u8,
    last_len: usize,
    dl_buf: Vec<u8>,
    dl_active: bool,
    dl: label::Decoder,
    dg_len: Option<usize>,
    dg: Option<(u8, Vec<u8>, usize)>,
    /// The application types that start a data group (12 always; others from FIG 0/13); the next type continues it.
    pub dg_starts: Vec<u8>,
    /// The current label.
    pub label: Label,
    /// Data groups completed.
    pub groups: Vec<XpadDataGroup>,
    /// The label changed in the last call.
    pub label_changed: bool,
    /// Data group CRC failures (length indicators).
    pub bad_indicators: u64,
}

impl PadDecoder {
    /// A decoder.
    pub fn new() -> Self {
        PadDecoder {
            dg_starts: vec![12],
            ..Default::default()
        }
    }

    /// Clears everything (a new service).
    pub fn reset(&mut self) {
        let starts = std::mem::take(&mut self.dg_starts);
        *self = PadDecoder::new();
        if !starts.is_empty() {
            self.dg_starts = starts;
        }
    }

    fn is_start(&self, t: u8) -> bool {
        self.dg_starts.contains(&t)
    }

    /// Decodes one frame's PAD. Completed data groups are appended to `self.groups`.
    pub fn push(&mut self, pad: &PadItem) {
        self.label_changed = false;
        let f0 = pad.fpad[0];
        let f1 = pad.fpad[1];
        if f0 >> 6 != 0 {
            return;
        }
        let ind = (f0 >> 4) & 3;
        let ci = f1 & 0x02 != 0;
        let x = pad.xpad.bytes();
        // The subfields (application type, start, length): at most the four that the content indicators can list.
        let mut subs = [(0u8, 0usize, 0usize); 4];
        let mut count = 0;
        match ind {
            1 => {
                if x.len() < 4 {
                    return;
                }
                subs[0] = if ci {
                    (x[0] & 31, 1, 3)
                } else {
                    (self.continuation(), 0, 4)
                };
                count = 1;
            }
            2 => {
                let limit = x.len();
                if ci {
                    let mut i = 0;
                    while i < 4 && i < limit {
                        let b = x[i];
                        i += 1;
                        let t = b & 31;
                        if t == 0 {
                            break;
                        }
                        subs[count] = (t, 0, XPAD_LENGTHS[(b >> 5) as usize]);
                        count += 1;
                    }
                    // The subfields follow the indicators, as much of each as the X-PAD holds.
                    let mut at = i;
                    for (k, sub) in subs[..count].iter_mut().enumerate() {
                        if at >= limit {
                            count = k;
                            break;
                        }
                        let l = sub.2.min(limit - at);
                        (sub.1, sub.2) = (at, l);
                        at += l;
                    }
                } else {
                    let len = if pad.exact {
                        limit
                    } else {
                        self.last_len.min(limit)
                    };
                    if len > 0 {
                        subs[0] = (self.continuation(), 0, len);
                        count = 1;
                    }
                }
            }
            _ => return,
        }
        for &(t, at, len) in &subs[..count] {
            let bytes = &x[at..at + len];
            self.last_app = t;
            self.last_len = len;
            self.subfield(t, bytes);
        }
    }

    fn continuation(&self) -> u8 {
        match self.last_app {
            2 => 3,
            t if self.is_start(t) => t + 1,
            t => t,
        }
    }

    fn subfield(&mut self, t: u8, b: &[u8]) {
        match t {
            1 => {
                if b.len() >= 4 && crc_ok(&b[..4]) {
                    self.dg_len = Some((((b[0] as usize) << 8) | b[1] as usize) & 0x3FFF);
                } else {
                    self.bad_indicators += 1;
                    self.dg_len = None;
                    self.dg = None;
                }
            }
            2 => {
                self.dl_buf.clear();
                self.dl_buf.extend_from_slice(b);
                self.dl_active = true;
                self.dl_try();
            }
            3 => {
                if self.dl_active {
                    self.dl_buf.extend_from_slice(b);
                    self.dl_try();
                }
            }
            t if self.is_start(t) => {
                if let Some(len) = self.dg_len.take() {
                    let mut v = Vec::with_capacity(len);
                    v.extend_from_slice(&b[..b.len().min(len)]);
                    self.dg = Some((t, v, len));
                    self.dg_check();
                }
            }
            t if t > 0 && self.is_start(t - 1) => {
                if let Some((st, v, len)) = self.dg.as_mut() {
                    if *st + 1 == t {
                        let room = len.saturating_sub(v.len());
                        v.extend_from_slice(&b[..b.len().min(room)]);
                    }
                }
                self.dg_check();
            }
            _ => {}
        }
    }

    fn dg_check(&mut self) {
        let Some((t, v, target)) = self.dg.as_mut() else {
            return;
        };
        if v.len() >= *target && *target > 0 {
            let data = std::mem::take(v);
            let app = *t;
            self.dg = None;
            self.groups.push(XpadDataGroup {
                app_type: app,
                data,
            });
            if self.groups.len() > 64 {
                self.groups.remove(0);
            }
        }
    }

    /// Tries to complete a DL segment from the collected bytes, and returns it to the label decoder.
    fn dl_try(&mut self) {
        let b = &self.dl_buf;
        if b.len() < 2 {
            return;
        }
        let Some(n) = label::segment_len(b) else {
            // A command this receiver does not know: it ends where the CRC checks.
            if (4..=20).any(|n| b.len() >= n && crc_ok(&b[..n])) || b.len() >= 20 {
                self.dl_active = false;
            }
            return;
        };
        if b.len() < n {
            return;
        }
        self.dl_active = false;
        let dl_plus = b[0] & 0x1F == 0x10 | label::COMMAND_DL_PLUS;
        let mut update = self.dl.push(&b[..n]);
        if update.is_err() && dl_plus {
            // The length field of a DL Plus command did not check: the command ends where the CRC checks.
            if let Some(m) = (4..=20).find(|&m| m <= b.len() && crc_ok(&b[..m])) {
                let mut seg = b[..m].to_vec();
                seg[1] = (seg[1] & 0xF0) | ((m - 5) as u8 & 15);
                update = self.dl.push(&seg);
            }
        }
        if matches!(update, Ok(Update::Label | Update::Tags | Update::Cleared)) {
            self.publish_label();
        }
    }

    fn publish_label(&mut self) {
        let tags = self.dl.tags();
        self.label = Label {
            text: self.dl.label().to_string(),
            toggle: self.dl.toggle(),
            charset: self.dl.charset().code(),
            tags: tags.clone(),
            item_running: tags.item.running,
            item_toggle: tags.item.toggle,
            count: self.dl.count(),
        };
        self.label_changed = true;
    }
}

/// Writes PAD for the generator and tests: content indicators and subfields into an X-PAD in logical order.
pub mod write {
    use crate::fec::crc_of;

    /// The smallest X-PAD length index that holds `n` bytes.
    pub fn length_index(n: usize) -> Option<u8> {
        super::XPAD_LENGTHS
            .iter()
            .position(|&l| l >= n)
            .map(|i| i as u8)
    }

    /// A dynamic label segment: prefix, characters and CRC.
    pub fn dl_segment(
        toggle: bool,
        first: bool,
        last: bool,
        number: u8,
        charset: u8,
        chars: &[u8],
    ) -> Vec<u8> {
        let mut v = vec![
            ((toggle as u8) << 7)
                | ((first as u8) << 6)
                | ((last as u8) << 5)
                | ((chars.len() as u8 - 1) & 15),
            if first {
                charset << 4
            } else {
                (number & 7) << 4
            },
        ];
        v.extend_from_slice(chars);
        let c = crc_of(&v);
        v.extend_from_slice(&c.to_be_bytes());
        v
    }

    /// A dynamic label command (1: clear).
    pub fn dl_clear(toggle: bool) -> Vec<u8> {
        let mut v = vec![((toggle as u8) << 7) | 0x60 | 0x10 | 1, 0];
        let c = crc_of(&v);
        v.extend_from_slice(&c.to_be_bytes());
        v
    }

    /// A DL Plus tags command: (content type, start, length − 1) tags.
    pub fn dl_plus(
        toggle: bool,
        link: bool,
        item_toggle: bool,
        item_running: bool,
        tags: &[(u8, u8, u8)],
    ) -> Vec<u8> {
        let mut body = vec![
            ((item_toggle as u8) << 3)
                | ((item_running as u8) << 2)
                | ((tags.len().max(1) as u8 - 1) & 3),
        ];
        for &(ct, s, l) in tags {
            body.extend_from_slice(&[ct & 0x7F, s & 0x7F, l & 0x7F]);
        }
        let mut v = vec![
            ((toggle as u8) << 7) | 0x60 | 0x10 | 2,
            ((link as u8) << 7) | ((body.len() as u8 - 1) & 15),
        ];
        v.extend_from_slice(&body);
        let c = crc_of(&v);
        v.extend_from_slice(&c.to_be_bytes());
        v
    }

    /// The data group length indicator subfield (4 bytes).
    pub fn length_indicator(len: usize) -> Vec<u8> {
        let mut v = vec![((len >> 8) & 0x3F) as u8, len as u8];
        let c = crc_of(&v);
        v.extend_from_slice(&c.to_be_bytes());
        v
    }

    /// A variable size X-PAD with content indicators for `subs` (application type, bytes; each padded to its length class).
    pub fn variable_xpad(subs: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut ci = Vec::new();
        let mut data = Vec::new();
        for (t, b) in subs.iter().take(4) {
            let li = length_index(b.len()).unwrap_or(7);
            ci.push((li << 5) | (t & 31));
            let mut bb = b.clone();
            bb.resize(super::XPAD_LENGTHS[li as usize], 0);
            data.extend(bb);
        }
        if subs.len() < 4 {
            ci.push(0);
        }
        ci.extend(data);
        ci
    }

    /// The DAB+ data stream element holding a PAD (X-PAD reversed, then F-PAD).
    pub fn dse(xpad: &[u8], fpad: [u8; 2]) -> Vec<u8> {
        let mut body: Vec<u8> = xpad.iter().rev().copied().collect();
        body.extend_from_slice(&fpad);
        let mut v = vec![0x80];
        if body.len() >= 255 {
            v.push(255);
            v.push((body.len() - 255) as u8);
        } else {
            v.push(body.len() as u8);
        }
        v.extend(body);
        v
    }

    /// The F-PAD bytes: X-PAD indicator (0 none, 1 short, 2 variable) and the CI flag.
    pub fn fpad(ind: u8, ci: bool) -> [u8; 2] {
        [(ind & 3) << 4, (ci as u8) << 1]
    }
}

#[cfg(test)]
mod tests {
    use super::write::*;
    use super::*;

    fn feed(d: &mut PadDecoder, subs: &[(u8, Vec<u8>)]) {
        let x = variable_xpad(subs);
        d.push(&PadItem::new(fpad(2, true), &x, true));
    }

    #[test]
    fn a_two_segment_label_appears_after_the_last_segment() {
        let mut d = PadDecoder::new();
        let s0 = dl_segment(false, true, false, 0, 15, "Beyoncé - ".as_bytes());
        let s1 = dl_segment(false, false, true, 1, 0, b"Halo");
        // The first segment in an 8-byte start and continuations.
        feed(&mut d, &[(2, s0[..8].to_vec())]);
        d.push(&PadItem::new(fpad(2, false), &s0[8..], true));
        assert_eq!(d.label.text, "");
        feed(&mut d, &[(2, s1.clone())]);
        assert_eq!(d.label.text, "Beyoncé - Halo");
        // A bad CRC on a new label leaves the old one.
        let mut bad = dl_segment(true, true, true, 0, 0, b"Next song");
        let n = bad.len();
        bad[n - 1] ^= 1;
        feed(&mut d, &[(2, bad)]);
        assert_eq!(d.label.text, "Beyoncé - Halo");
        // DL Plus: Artist "Beyoncé", Title "Halo", running.
        feed(
            &mut d,
            &[(
                2,
                dl_plus(false, false, false, true, &[(4, 0, 6), (1, 10, 3)]),
            )],
        );
        // The tags come in content type order: Title (1), then Artist (4).
        assert_eq!(d.label.tags.entries().count(), 2);
        assert_eq!(d.label.tags.entries().next().unwrap().1, "Halo");
        assert_eq!(d.label.tags.entries().nth(1).unwrap().1, "Beyoncé");
        assert!(d.label.item_running);
        feed(
            &mut d,
            &[(2, dl_segment(true, true, true, 0, 0, b"Next song"))],
        );
        assert_eq!(d.label.text, "Next song");
        assert!((d.label.tags.entries().count() == 0));
        feed(
            &mut d,
            &[(
                2,
                dl_plus(true, true, false, false, &[(1, 0, 8), (0, 0, 0)]),
            )],
        );
        assert_eq!(d.label.tags.entries().count(), 1);
        assert_eq!(d.label.tags.entries().next().unwrap().1, "Next song");
        assert!(!d.label.item_running);
        feed(
            &mut d,
            &[(2, dl_plus(true, true, false, false, &[(4, 5, 40)]))],
        );
        assert!((d.label.tags.entries().count() == 0));
        let mut badp = dl_plus(true, true, false, false, &[(1, 0, 8)]);
        let n = badp.len();
        badp[n - 2] ^= 0x40;
        feed(&mut d, &[(2, badp)]);
        assert!((d.label.tags.entries().count() == 0));
        feed(&mut d, &[(2, dl_clear(true))]);
        assert_eq!(d.label.text, "");
    }

    #[test]
    fn mp2_xpad_without_indicators_continues() {
        let mut d = PadDecoder::new();
        let seg = dl_segment(false, true, true, 0, 0, b"Mp2 Radi");
        assert_eq!(seg.len(), 12);
        // One 12-byte subfield with its indicator: the MP2 X-PAD holds the indicator, the end marker... the decoder reads only
        // what the indicators list.
        let mut x = vec![(3 << 5) | 2, 0];
        x.extend_from_slice(&seg);
        x.resize(40, 0x55);
        d.push(&PadItem::new(fpad(2, true), &x, false));
        assert_eq!(d.label.text, "Mp2 Radi");
        // Split over two frames: an 8-byte subfield with an indicator, then 8 bytes without.
        let mut d = PadDecoder::new();
        let mut x = vec![(2 << 5) | 2, 0];
        x.extend_from_slice(&seg[..8]);
        x.resize(40, 0x55);
        d.push(&PadItem::new(fpad(2, true), &x, false));
        assert_eq!(d.label.text, "");
        let mut x = seg[8..].to_vec();
        x.resize(40, 0x55);
        d.push(&PadItem::new(fpad(2, false), &x, false));
        assert_eq!(d.label.text, "Mp2 Radi");
    }
}
