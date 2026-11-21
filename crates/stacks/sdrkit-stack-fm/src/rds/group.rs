//! RDS group decoding (IEC 62106 group definitions), with the guard against miscorrection: a wrong character must never be shown.
//!
//! The decoder takes [`Group`]s from the block layer, decides which of their blocks it uses, and keeps
//! the decoded fields: PI, PTY, TP, TA, M/S, PS, AF, ECC, language, PIN, RadioText, ODA, clock time, PTYN, EON, RadioText+ and eRT.

use super::block::{Block, Group};
use super::tmc::Tmc;
use sdrkit_proto_bcdata::tags::{parse_rtplus, TagSet};
use sdrkit_util_geo::time::MJD_UNIX_EPOCH;
use sdrkit_util_text::ebu::{rds_text_into, RdsCharset};
use std::collections::VecDeque;
/// AID of RadioText+.
pub const AID_RTPLUS: u16 = 0x4BD7;
/// AID of enhanced RadioText.
pub const AID_ERT: u16 = 0x6552;
/// AIDs of RDS-TMC.
pub const AID_TMC: [u16; 2] = [0xCD46, 0xCD47];

/// A programme item number (type 1A/1B, block D).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pin {
    /// Day of the month, 1 to 31.
    pub day: u8,
    /// Hour.
    pub hour: u8,
    /// Minute.
    pub minute: u8,
}

/// Clock time and date (type 4A).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClockTime {
    /// UTC seconds since 1970-01-01.
    pub utc_s: i64,
    /// The local time offset in minutes.
    pub offset_min: i32,
    /// The host's wall-clock time of reception, in seconds since 1970-01-01.
    pub received_at_s: f64,
}

/// An open data application (type 3A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Oda {
    /// The application identifier.
    pub aid: u16,
    /// The group type that carries it, as type * 2 + version (0 = A, 1 = B).
    pub group: u8,
}

/// Another network's entry (type 14A).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EonEntry {
    /// The other network's PI.
    pub pi: u16,
    segs: [Option<[u8; 2]>; 4],
    /// Its PS, once all four segments are present; empty before.
    pub name: String,
    /// Its traffic-programme flag (14A block B).
    pub tp: bool,
    /// Its traffic-announcement flag (variant 13).
    pub ta: bool,
    /// Its programme type (variant 13).
    pub pty: Option<u8>,
    /// Its programme item number (variant 14).
    pub pin: Option<Pin>,
    /// Its alternative frequencies in MHz (variant 4, method A), ascending.
    pub af_mhz: Vec<f64>,
}

#[derive(Clone, Debug)]
struct Seen {
    b: u16,
    c: Option<u16>,
    d: u16,
    raw: (u16, Option<u16>, u16),
    corrected: bool,
}

/// Everything decoded from the groups, and the guard's state.
#[derive(Clone, Debug)]
pub struct GroupDecoder {
    charset: RdsCharset,
    /// Accepted groups since reset.
    pub groups: u64,
    /// PI; 0 until a group has been accepted.
    pub pi: u16,
    pi_candidate: u16,
    /// Programme type code; `None` until the first accepted group.
    pub pty: Option<u8>,
    /// Traffic programme.
    pub tp: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music (true) or speech.
    pub music: bool,
    // PS
    strict: [Option<[u8; 2]>; 4],
    latest: [Option<[u8; 2]>; 4],
    type0_since_publish: u32,
    ps_bytes: Option<[u8; 8]>,
    /// The published PS (8 characters, trailing spaces kept), or empty.
    pub ps: String,
    /// The name being collected (`_` for a missing segment) until the first publish; then the published name.
    pub ps_partial: String,
    /// AF list in MHz, ascending, at most 32.
    af_codes: Vec<u8>,
    /// Extended country code.
    pub ecc: Option<u8>,
    /// Language code.
    pub language: Option<u16>,
    /// Programme item number.
    pub pin: Option<Pin>,
    // RT
    rt_buf: [u8; 64],
    rt_seen: [bool; 32],
    rt_ab: Option<bool>,
    rt_b_version: bool,
    /// RadioText as shown (cut at the carriage return, trailing spaces removed).
    pub rt: String,
    /// Every segment up to the carriage return (or all of them) has arrived.
    pub rt_complete: bool,
    /// Open data applications.
    pub oda: Vec<Oda>,
    /// Clock time.
    pub clock: Option<ClockTime>,
    // PTYN
    ptyn_segs: [Option<[u8; 4]>; 2],
    ptyn_ab: Option<bool>,
    /// Programme type name, once both segments are present.
    pub ptyn: String,
    /// Other networks.
    pub eon: Vec<EonEntry>,
    /// The RadioText+ tags and the item bits.
    pub tags: TagSet,
    // eRT
    ert_buf: [u8; 128],
    ert_seen: [bool; 32],
    ert_utf8: bool,
    /// Enhanced RadioText (UTF-8, trimmed).
    pub ert: String,
    history: VecDeque<Seen>,
    /// RDS-TMC.
    pub tmc: Tmc,
    /// Set when the PS was published by the last group (for events).
    pub ps_changed: bool,
    /// Set when the RadioText became complete with the last group.
    pub rt_completed: bool,
}

impl Default for GroupDecoder {
    fn default() -> Self {
        Self::new(RdsCharset::Auto)
    }
}

/// RDS text bytes as a new string.
fn decoded(bytes: &[u8], charset: RdsCharset) -> String {
    let mut s = String::new();
    rds_text_into(bytes, charset, &mut s);
    s
}

fn bytes2(w: u16) -> [u8; 2] {
    [(w >> 8) as u8, w as u8]
}

impl GroupDecoder {
    /// An empty decoder reading text with `charset`.
    pub fn new(charset: RdsCharset) -> Self {
        GroupDecoder {
            charset,
            groups: 0,
            pi: 0,
            pi_candidate: 0,
            pty: None,
            tp: false,
            ta: false,
            music: true,
            strict: [None; 4],
            latest: [None; 4],
            type0_since_publish: 0,
            ps_bytes: None,
            ps: String::new(),
            ps_partial: String::new(),
            af_codes: Vec::new(),
            ecc: None,
            language: None,
            pin: None,
            rt_buf: [b' '; 64],
            rt_seen: [false; 32],
            rt_ab: None,
            rt_b_version: false,
            rt: String::new(),
            rt_complete: false,
            oda: Vec::new(),
            clock: None,
            ptyn_segs: [None; 2],
            ptyn_ab: None,
            ptyn: String::new(),
            eon: Vec::new(),
            tags: TagSet::default(),
            ert_buf: [0; 128],
            ert_seen: [false; 32],
            ert_utf8: true,
            ert: String::new(),
            history: VecDeque::with_capacity(64),
            tmc: Tmc::default(),
            ps_changed: false,
            rt_completed: false,
        }
    }

    /// Clears everything (a retune or reset), keeping the charset.
    pub fn reset(&mut self) {
        *self = Self::new(self.charset);
    }

    /// Changes how text is read; the shown texts are decoded again.
    pub fn set_charset(&mut self, charset: RdsCharset) {
        self.charset = charset;
        if let Some(b) = self.ps_bytes {
            self.ps = decoded(&b, charset);
        }
        self.refresh_partial();
        self.rebuild_rt();
        self.rebuild_ptyn();
    }

    /// The alternative frequency codes, in order (frequency = (875 + code) / 10 MHz).
    pub fn af_codes(&self) -> &[u8] {
        &self.af_codes
    }

    /// The AF list in MHz.
    pub fn af_mhz(&self) -> Vec<f64> {
        self.af_codes
            .iter()
            .map(|&c| (875 + c as u32) as f64 / 10.0)
            .collect()
    }

    /// Takes one group from the block layer. Returns whether it was accepted.
    pub fn push(&mut self, g: &Group, wall_clock_s: f64) -> bool {
        self.ps_changed = false;
        self.rt_completed = false;
        let [a, b, c, d] = g.blocks;
        let (Some(b), Some(d)) = (b, d) else {
            return false;
        };
        let version_b = b.data & 0x0800 != 0;
        // Block 3 must match the version: C′ in version B, C in version A.
        let c = c.filter(|c| c.c_prime == version_b);
        let any_corr = b.corrected || d.corrected || c.is_some_and(|c| c.corrected);
        let confirmed = self.confirmed(&b, c.as_ref(), &d, any_corr);
        self.remember(&b, c.as_ref(), &d, any_corr);
        let mut c = c;
        if any_corr && !confirmed {
            if b.corrected || d.corrected {
                return false;
            }
            c = None;
        }
        // PI: from a good A, from a good C′, or from a corrected A in a confirmed group.
        if let Some(a) = a {
            if !a.corrected || confirmed {
                self.pi_candidate = a.data;
            }
        } else if let Some(cp) = c.filter(|c| c.c_prime && !c.corrected) {
            self.pi_candidate = cp.data;
        }
        if self.pi_candidate != 0 {
            self.pi = self.pi_candidate;
        }
        self.groups += 1;
        let bw = b.data;
        self.tp = bw & 0x0400 != 0;
        self.pty = Some(((bw >> 5) & 0x1F) as u8);
        let clean = !any_corr && c.is_some();
        self.decode(
            bw,
            c.map(|c| c.data),
            d.data,
            version_b,
            clean,
            wall_clock_s,
        );
        true
    }

    fn confirmed(&self, b: &Block, c: Option<&Block>, d: &Block, any_corr: bool) -> bool {
        let raw = (b.raw, c.map(|c| c.raw), d.raw);
        let c_corr = c.is_some_and(|c| c.corrected);
        let seen = self.history.iter().any(|h| {
            h.b == b.data
                && h.d == d.data
                && match (h.c, c) {
                    (Some(x), Some(y)) => x == y.data,
                    _ => true,
                }
                && (!c_corr || h.c.is_some())
                && (!(h.corrected && any_corr) || h.raw != raw)
        });
        // An extra guard: a corrected group that contradicts a clean copy still in the history (same B and C, another D; or same B
        // and D, another C) is not confirmed. Two damaged copies can be miscorrected to the same wrong word.
        let contradicted = any_corr
            && self.history.iter().any(|h| {
                !h.corrected
                    && h.b == b.data
                    && ((h.d != d.data
                        && (h.c.is_none() || c.is_none() || h.c == c.map(|c| c.data)))
                        || (c_corr && h.d == d.data && h.c.is_some() && h.c != c.map(|c| c.data)))
            });
        seen && !contradicted
    }

    fn remember(&mut self, b: &Block, c: Option<&Block>, d: &Block, corrected: bool) {
        if self.history.len() == 64 {
            self.history.pop_front();
        }
        self.history.push_back(Seen {
            b: b.data,
            c: c.map(|c| c.data),
            d: d.data,
            raw: (b.raw, c.map(|c| c.raw), d.raw),
            corrected,
        });
    }

    fn decode(&mut self, b: u16, c: Option<u16>, d: u16, version_b: bool, clean: bool, wall: f64) {
        let gtype = (b >> 12) as u8;
        let code = gtype * 2 + version_b as u8;
        // Groups an ODA has claimed.
        if let Some(o) = self.oda.iter().find(|o| o.group == code).copied() {
            if !matches!(code, 0..=8 | 20 | 28..=31) {
                match o.aid {
                    AID_RTPLUS => self.rtplus_group(b, c, d),
                    AID_ERT => self.ert_group(b, c, d),
                    a if AID_TMC.contains(&a) => self.tmc_group(b, c, d, wall),
                    _ => {}
                }
                return;
            }
        }
        match (gtype, version_b) {
            (0, _) => self.group0(b, c, d, version_b),
            (1, _) => self.group1(c, d, version_b),
            (2, _) => self.group2(b, c, d, version_b),
            (3, false) => self.group3a(b, c, d),
            (4, false) if clean => self.group4a(b, c, d, wall),
            (10, false) => self.group10a(b, c, d),
            (14, false) => self.group14a(b, c, d),
            (8, false) => self.tmc_group(b, c, d, wall),
            _ => {}
        }
    }

    fn group0(&mut self, b: u16, c: Option<u16>, d: u16, version_b: bool) {
        self.ta = b & 0x10 != 0;
        self.music = b & 0x08 != 0;
        let seg = (b & 3) as usize;
        let chars = bytes2(d);
        self.type0_since_publish += 1;
        self.latest[seg] = Some(chars);
        if self.strict[seg].is_some_and(|s| s != chars) {
            self.strict = [None; 4];
        }
        self.strict[seg] = Some(chars);
        if self.strict.iter().all(Option::is_some) {
            let name = self.strict;
            self.publish_ps(name);
            self.strict = [None; 4];
        } else if self.latest.iter().all(Option::is_some) && self.type0_since_publish >= 40 {
            let name = self.latest;
            self.publish_ps(name);
        }
        self.refresh_partial();
        if !version_b {
            if let Some(c) = c {
                for code in bytes2(c) {
                    if (1..=204).contains(&code)
                        && !self.af_codes.contains(&code)
                        && self.af_codes.len() < 32
                    {
                        self.af_codes.push(code);
                        self.af_codes.sort_unstable();
                    }
                }
            }
        }
    }

    fn publish_ps(&mut self, segs: [Option<[u8; 2]>; 4]) {
        let mut bytes = [b' '; 8];
        for (i, s) in segs.iter().enumerate() {
            let s = s.unwrap_or(*b"  ");
            bytes[2 * i] = s[0];
            bytes[2 * i + 1] = s[1];
        }
        self.type0_since_publish = 0;
        let changed = self.ps_bytes != Some(bytes);
        self.ps_bytes = Some(bytes);
        self.ps.clear();
        rds_text_into(&bytes, self.charset, &mut self.ps);
        self.ps_changed = changed;
    }

    fn refresh_partial(&mut self) {
        if self.ps_bytes.is_some() {
            self.ps_partial.clone_from(&self.ps);
            return;
        }
        if self.latest.iter().all(Option::is_none) {
            self.ps_partial.clear();
            return;
        }
        // Decode the segments that are present together, so a GB2312 pair is read as one; mark the missing ones.
        let mut out = std::mem::take(&mut self.ps_partial);
        out.clear();
        let (mut run, mut n) = ([0u8; 8], 0);
        for s in &self.latest {
            match s {
                Some(s) => {
                    run[n..n + 2].copy_from_slice(s);
                    n += 2;
                }
                None => {
                    rds_text_into(&run[..n], self.charset, &mut out);
                    n = 0;
                    out.push_str("__");
                }
            }
        }
        rds_text_into(&run[..n], self.charset, &mut out);
        self.ps_partial = out;
    }

    fn group1(&mut self, c: Option<u16>, d: u16, version_b: bool) {
        if !version_b {
            if let Some(c) = c {
                match (c >> 12) & 7 {
                    0 => self.ecc = Some(c as u8),
                    3 => self.language = Some(c & 0x0FFF),
                    _ => {}
                }
            }
        }
        let day = (d >> 11) as u8;
        if day != 0 {
            self.pin = Some(Pin {
                day,
                hour: ((d >> 6) & 0x1F) as u8,
                minute: (d & 0x3F) as u8,
            });
        }
    }

    fn group2(&mut self, b: u16, c: Option<u16>, d: u16, version_b: bool) {
        let ab = b & 0x10 != 0;
        let seg = (b & 0x0F) as usize;
        if self.rt_ab.is_some_and(|x| x != ab)
            || (self.rt_ab.is_some() && self.rt_b_version != version_b)
        {
            self.rt_buf = [b' '; 64];
            self.rt_seen = [false; 32];
            self.tags.clear();
            self.rt.clear();
            self.rt_complete = false;
        }
        self.rt_ab = Some(ab);
        self.rt_b_version = version_b;
        if version_b {
            self.rt_buf[seg * 2..seg * 2 + 2].copy_from_slice(&bytes2(d));
            self.rt_seen[seg] = true;
        } else {
            let Some(c) = c else { return };
            self.rt_buf[seg * 4..seg * 4 + 2].copy_from_slice(&bytes2(c));
            self.rt_buf[seg * 4 + 2..seg * 4 + 4].copy_from_slice(&bytes2(d));
            self.rt_seen[seg] = true;
        }
        let was = self.rt_complete;
        self.rebuild_rt();
        self.rt_completed = self.rt_complete && !was;
        self.refresh_rtplus();
    }

    /// Bytes per RT segment and the number of segments, for the current version.
    fn rt_geometry(&self) -> (usize, usize) {
        if self.rt_b_version {
            (2, 16)
        } else {
            (4, 16)
        }
    }

    fn rt_len(&self) -> usize {
        let (per, n) = self.rt_geometry();
        per * n
    }

    fn rebuild_rt(&mut self) {
        let len = self.rt_len();
        let buf = &self.rt_buf[..len];
        let cr = buf.iter().position(|&b| b == 0x0D);
        let text = &buf[..cr.unwrap_or(len)];
        self.rt.clear();
        rds_text_into(text, self.charset, &mut self.rt);
        let kept = self.rt.trim_end_matches(' ').len();
        self.rt.truncate(kept);
        let (per, n) = self.rt_geometry();
        let last = cr.map_or(n - 1, |p| p / per);
        self.rt_complete = self.rt_ab.is_some() && (0..=last).all(|s| self.rt_seen[s]);
    }

    fn group3a(&mut self, b: u16, c: Option<u16>, d: u16) {
        let Some(c) = c else { return };
        let group = (b & 0x1F) as u8;
        let aid = d;
        if let Some(o) = self.oda.iter_mut().find(|o| o.aid == aid) {
            o.group = group;
        } else if self.oda.len() < 16 {
            self.oda.push(Oda { aid, group });
        }
        if AID_TMC.contains(&aid) {
            self.tmc.announcement(c);
        }
        if aid == AID_ERT {
            let utf8 = c & 1 == 1;
            if utf8 != self.ert_utf8 {
                self.ert_utf8 = utf8;
                self.rebuild_ert();
            }
        }
    }

    fn group4a(&mut self, b: u16, c: Option<u16>, d: u16, wall: f64) {
        let Some(c) = c else { return };
        let mjd = (((b & 3) as i64) << 15) | (c >> 1) as i64;
        let hour = (((c & 1) << 4) | (d >> 12)) as i64;
        let minute = ((d >> 6) & 0x3F) as i64;
        let half_hours = (d & 0x1F) as i32;
        let offset = if d & 0x20 != 0 {
            -half_hours
        } else {
            half_hours
        };
        if hour >= 24 || minute >= 60 || !(50_000..80_000).contains(&mjd) || offset.abs() > 30 {
            return;
        }
        let utc_s = (mjd - MJD_UNIX_EPOCH) * 86_400 + hour * 3600 + minute * 60;
        self.clock = Some(ClockTime {
            utc_s,
            offset_min: offset * 30,
            received_at_s: wall,
        });
    }

    fn group10a(&mut self, b: u16, c: Option<u16>, d: u16) {
        let Some(c) = c else { return };
        let ab = b & 0x10 != 0;
        if self.ptyn_ab.is_some_and(|x| x != ab) {
            self.ptyn_segs = [None; 2];
            self.ptyn.clear();
        }
        self.ptyn_ab = Some(ab);
        let [c0, c1] = bytes2(c);
        let [d0, d1] = bytes2(d);
        self.ptyn_segs[(b & 1) as usize] = Some([c0, c1, d0, d1]);
        self.rebuild_ptyn();
    }

    fn rebuild_ptyn(&mut self) {
        if let [Some(a), Some(b)] = self.ptyn_segs {
            let mut bytes = [0u8; 8];
            bytes[..4].copy_from_slice(&a);
            bytes[4..].copy_from_slice(&b);
            self.ptyn.clear();
            rds_text_into(&bytes, self.charset, &mut self.ptyn);
        }
    }

    fn tmc_group(&mut self, b: u16, c: Option<u16>, d: u16, wall: f64) {
        if let Some(c) = c {
            self.tmc.group(b, c, d, wall);
        }
    }

    fn group14a(&mut self, b: u16, c: Option<u16>, d: u16) {
        let variant = (b & 0x0F) as usize;
        let on_pi = d;
        if !matches!(variant, 0..=4 | 13 | 14) || on_pi == 0 || on_pi == self.pi {
            return;
        }
        let Some(c) = c else { return };
        let idx = match self.eon.iter().position(|e| e.pi == on_pi) {
            Some(i) => i,
            None if self.eon.len() < 16 => {
                self.eon.push(EonEntry {
                    pi: on_pi,
                    ..Default::default()
                });
                self.eon.len() - 1
            }
            None => return,
        };
        let charset = self.charset;
        let e = &mut self.eon[idx];
        e.tp = b & 0x10 != 0;
        match variant {
            4 => {
                for code in bytes2(c) {
                    let mhz = (875 + code as u32) as f64 / 10.0;
                    if (1..=204).contains(&code) && !e.af_mhz.contains(&mhz) && e.af_mhz.len() < 25
                    {
                        e.af_mhz.push(mhz);
                        e.af_mhz.sort_by(f64::total_cmp);
                    }
                }
                return;
            }
            13 => {
                e.pty = Some((c >> 11) as u8);
                e.ta = c & 1 != 0;
                return;
            }
            14 => {
                let day = (c >> 11) as u8;
                if day != 0 {
                    e.pin = Some(Pin {
                        day,
                        hour: ((c >> 6) & 0x1F) as u8,
                        minute: (c & 0x3F) as u8,
                    });
                }
                return;
            }
            _ => {}
        }
        e.segs[variant] = Some(bytes2(c));
        if e.segs.iter().all(Option::is_some) {
            let bytes: Vec<u8> = e.segs.iter().flatten().flatten().copied().collect();
            e.name = decoded(&bytes, charset);
        }
    }

    fn rtplus_group(&mut self, b: u16, c: Option<u16>, d: u16) {
        let Some(c) = c else { return };
        let (item, tags) = parse_rtplus(b, c, d);
        let (per, _) = self.rt_geometry();
        let (seen, charset) = (&self.rt_seen, self.charset);
        self.tags.merge_received(
            item,
            &tags,
            &self.rt_buf[..self.rt_len()],
            |i| seen[i / per],
            |bytes, out| rds_text_into(bytes, charset, out),
        );
    }

    /// Takes the text of each RT+ tag again after new RadioText segments arrived.
    fn refresh_rtplus(&mut self) {
        let (per, _) = self.rt_geometry();
        let (seen, charset) = (&self.rt_seen, self.charset);
        self.tags.refresh(
            &self.rt_buf[..self.rt_len()],
            |i| seen[i / per],
            |bytes, out| rds_text_into(bytes, charset, out),
        );
    }

    fn ert_group(&mut self, b: u16, c: Option<u16>, d: u16) {
        let Some(c) = c else { return };
        let seg = (b & 0x1F) as usize;
        let [c0, c1] = bytes2(c);
        let [d0, d1] = bytes2(d);
        self.ert_buf[seg * 4..seg * 4 + 4].copy_from_slice(&[c0, c1, d0, d1]);
        self.ert_seen[seg] = true;
        self.rebuild_ert();
    }

    fn rebuild_ert(&mut self) {
        let end = self.ert_buf.iter().position(|&b| b == 0x0D).unwrap_or(128);
        let bytes = &self.ert_buf[..end];
        self.ert = if self.ert_utf8 {
            let v: Vec<u8> = bytes.iter().copied().filter(|&b| b != 0).collect();
            let s = match std::str::from_utf8(&v) {
                Ok(s) => s.to_string(),
                Err(e) if e.error_len().is_none() => {
                    String::from_utf8_lossy(&v[..e.valid_up_to()]).into_owned()
                }
                Err(_) => String::from_utf8_lossy(&v).into_owned(),
            };
            s.trim().to_string()
        } else {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_be_bytes([p[0], p[1]]))
                .filter(|&u| u != 0)
                .collect();
            String::from_utf16_lossy(&units).trim().to_string()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PI: u16 = 0x4A21;

    fn b0(seg: u16) -> u16 {
        0x0400 | (10 << 5) | 0x08 | seg
    }

    fn ps_group(name: &[u8; 8], seg: u16) -> Group {
        let s = seg as usize;
        Group::clean(
            PI,
            b0(seg),
            0xCDCD,
            u16::from_be_bytes([name[2 * s], name[2 * s + 1]]),
        )
    }

    fn feed(dec: &mut GroupDecoder, g: Group) -> bool {
        dec.push(&g, 0.0)
    }

    #[test]
    fn monitor_line() {
        // The D word 5445 is "TE": a name starting with "TE" (with "SDRKIT  " it reads 5344).
        assert_eq!(ps_group(b"TEST    ", 0).hex(), "4A21 0548 CDCD 5445");
        assert_eq!(ps_group(b"SDRKIT  ", 0).hex(), "4A21 0548 CDCD 5344");
        let mut g = ps_group(b"SDRKIT  ", 0);
        g.blocks[2] = None;
        assert_eq!(g.hex(), "4A21 0548 ---- 5344");
    }

    #[test]
    fn ps_is_collected_strictly_and_partial_shows_gaps() {
        let mut d = GroupDecoder::default();
        assert_eq!(d.pi, 0);
        assert_eq!(d.pty, None);
        feed(&mut d, ps_group(b"SDRKIT  ", 0));
        assert_eq!(d.ps_partial, "SD______");
        assert_eq!(d.pi, PI);
        feed(&mut d, ps_group(b"SDRKIT  ", 2));
        assert_eq!(d.ps_partial, "SD__IT__");
        assert_eq!(d.ps, "");
        feed(&mut d, ps_group(b"SDRKIT  ", 1));
        feed(&mut d, ps_group(b"SDRKIT  ", 3));
        assert_eq!(d.ps, "SDRKIT  ");
        assert_eq!(d.pty, Some(10));
        assert!(d.tp && !d.ta && d.music);
        // A new name never mixes with the old one.
        feed(&mut d, ps_group(b"NEWNAME!", 0));
        feed(&mut d, ps_group(b"NEWNAME!", 1));
        assert_eq!(d.ps, "SDRKIT  ");
        feed(&mut d, ps_group(b"NEWNAME!", 2));
        feed(&mut d, ps_group(b"NEWNAME!", 3));
        assert_eq!(d.ps, "NEWNAME!");
    }

    #[test]
    fn alternative_frequencies() {
        let mut d = GroupDecoder::default();
        for _ in 0..2 {
            feed(&mut d, Group::clean(PI, b0(0), (227 << 8) | 30, 0x5455));
            feed(&mut d, Group::clean(PI, b0(1), (45 << 8) | 80, 0x4E45));
        }
        assert_eq!(d.af_mhz(), vec![90.5, 92.0, 95.5]);
    }

    #[test]
    fn types_1a_4a_10a() {
        let mut d = GroupDecoder::default();
        let b1 = 0x1000 | 0x0400 | (10 << 5);
        feed(
            &mut d,
            Group::clean(PI, b1, 0xE2, (23 << 11) | (14 << 6) | 5),
        );
        feed(
            &mut d,
            Group::clean(PI, b1, 0x3027, (23 << 11) | (14 << 6) | 5),
        );
        assert_eq!(d.ecc, Some(0xE2));
        assert_eq!(d.language, Some(0x27));
        assert_eq!(
            d.pin,
            Some(Pin {
                day: 23,
                hour: 14,
                minute: 5
            })
        );
        // 4A: MJD 61336, 12:34 UTC, +6 half hours.
        let mjd: u32 = 61336;
        let (h, m, off) = (12u16, 34u16, 6u16);
        let b4 = 0x4000 | 0x0400 | (10 << 5) | (mjd >> 15) as u16;
        let c4 = (((mjd & 0x7FFF) as u16) << 1) | (h >> 4);
        let d4 = ((h & 0xF) << 12) | (m << 6) | off;
        feed(&mut d, Group::clean(PI, b4, c4, d4));
        let ct = d.clock.unwrap();
        assert_eq!(ct.utc_s, 1_792_758_840);
        assert_eq!(ct.offset_min, 180);
        // 10A: "ROCKPOP ".
        let ba = 0xA000 | 0x0400 | (10 << 5);
        feed(
            &mut d,
            Group::clean(
                PI,
                ba,
                u16::from_be_bytes(*b"RO"),
                u16::from_be_bytes(*b"CK"),
            ),
        );
        assert_eq!(d.ptyn, "");
        feed(
            &mut d,
            Group::clean(
                PI,
                ba | 1,
                u16::from_be_bytes(*b"PO"),
                u16::from_be_bytes(*b"P "),
            ),
        );
        assert_eq!(d.ptyn, "ROCKPOP ");
    }

    fn rt_groups(text: &[u8], ab: bool) -> Vec<Group> {
        let mut t = text.to_vec();
        while !t.len().is_multiple_of(4) {
            t.push(b' ');
        }
        t.chunks(4)
            .enumerate()
            .map(|(i, c)| {
                let b = 0x2000 | 0x0400 | (10 << 5) | ((ab as u16) << 4) | i as u16;
                Group::clean(
                    PI,
                    b,
                    u16::from_be_bytes([c[0], c[1]]),
                    u16::from_be_bytes([c[2], c[3]]),
                )
            })
            .collect()
    }

    #[test]
    fn rt_is_shown_progressively_and_clears_on_ab_change() {
        let mut d = GroupDecoder::default();
        let gs = rt_groups(b"Hello world\r", false);
        feed(&mut d, gs[0]);
        assert_eq!(d.rt, "Hell");
        assert!(!d.rt_complete);
        feed(&mut d, gs[2]);
        assert_eq!(d.rt, "Hell    rld");
        feed(&mut d, gs[1]);
        assert_eq!(d.rt, "Hello world");
        assert!(d.rt_complete);
        let other = rt_groups(b"Bye\r", true);
        feed(&mut d, other[0]);
        assert_eq!(d.rt, "Bye");
        assert!(d.rt_complete);
    }

    #[test]
    fn oda_rtplus_and_ert() {
        let mut d = GroupDecoder::default();
        for g in rt_groups(b"Hello world\r", false) {
            feed(&mut d, g);
        }
        let b3 = 0x3000 | 0x0400 | (10 << 5);
        feed(&mut d, Group::clean(PI, b3 | 22, 0x0000, AID_RTPLUS)); // carried in 11A
        feed(&mut d, Group::clean(PI, b3 | 24, 0x0001, AID_ERT)); // carried in 12A, UTF-8
        assert_eq!(d.oda.len(), 2);
        // 11A: running, type 4 start 0 length 4, type 1 start 6 length 4.
        let b11 = 0xB000 | 0x0400 | (10 << 5) | 0x08; // type 1 bits 5..3 are 0 for Artist (4)
        let c11 = (4u16 << 13) | (4 << 1); // Artist: start 0, length 4; type 2 bit 5 is 0
        let d11 = ((1u16 & 0x1F) << 11) | (6 << 5) | 4;
        feed(&mut d, Group::clean(PI, b11, c11, d11));
        assert!(d.tags.item.running);
        assert_eq!(
            d.tags.entries().collect::<Vec<_>>(),
            vec![(1, "world"), (4, "Hello")]
        );
        // eRT "Ąžuolas ėjo" + CR in 4 segments.
        let mut bytes = "Ąžuolas ėjo".as_bytes().to_vec();
        bytes.push(0x0D);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        assert_eq!(bytes.len(), 16);
        for (i, c) in bytes.chunks(4).enumerate() {
            let b12 = 0xC000 | 0x0400 | (10 << 5) | i as u16;
            feed(
                &mut d,
                Group::clean(
                    PI,
                    b12,
                    u16::from_be_bytes([c[0], c[1]]),
                    u16::from_be_bytes([c[2], c[3]]),
                ),
            );
        }
        assert_eq!(d.ert, "Ąžuolas ėjo");
    }

    #[test]
    fn eon() {
        let mut d = GroupDecoder::default();
        let name = b"LRT KL. ";
        for v in 0..4u16 {
            let b = 0xE000 | 0x0400 | (10 << 5) | v;
            let c = u16::from_be_bytes([name[2 * v as usize], name[2 * v as usize + 1]]);
            feed(&mut d, Group::clean(PI, b, c, 0x4B01));
            if v < 3 {
                assert_eq!(d.eon[0].name, "");
            }
        }
        feed(&mut d, Group::clean(PI, 0xE000, 0x4142, 0x0000)); // ON-PI 0 ignored
        feed(&mut d, Group::clean(PI, 0xE000, 0x4142, PI)); // own PI ignored
        assert_eq!(d.eon.len(), 1);
        assert_eq!(d.eon[0].pi, 0x4B01);
        assert_eq!(d.eon[0].name, "LRT KL. ");
    }

    #[test]
    fn version_b_rt_takes_pi_from_c_prime() {
        let mut d = GroupDecoder::default();
        let text = b"Radio 2B\r ";
        for (i, c) in text.chunks(2).enumerate() {
            let b = 0x2800 | 0x0400 | (10 << 5) | i as u16;
            let mut g = Group::clean(0x4A22, b, 0x4A22, u16::from_be_bytes([c[0], c[1]]));
            g.blocks[0] = None;
            feed(&mut d, g);
        }
        assert_eq!(d.rt, "Radio 2B");
        assert!(d.rt_complete);
        assert_eq!(d.pi, 0x4A22);
    }

    #[test]
    fn gb2312_names() {
        let mut d = GroupDecoder::default();
        let gb = [0xD6, 0xD0, 0xB9, 0xFA, 0xD6, 0xAE, 0xC9, 0xF9];
        for s in 0..4 {
            feed(&mut d, ps_group(&gb, s));
        }
        assert_eq!(d.ps, "中国之声");
        // A scrolling 8-byte name, moving one character (two bytes) per group, segments in the order 0 1 0 2 0 3.
        let mut d = GroupDecoder::default();
        let text: Vec<u8> = [
            0xD6u8, 0xD0, 0xB9, 0xFA, 0xD6, 0xAE, 0xC9, 0xF9, 0xBD, 0xBB, 0xCD, 0xA8, 0xB5, 0xE7,
            0xCC, 0xA8,
        ]
        .to_vec();
        let order = [0u16, 1, 0, 2, 0, 3];
        let mut shown = Vec::new();
        for k in 0..240usize {
            let shift = (k % 8) * 2;
            let window: Vec<u8> = (0..8).map(|i| text[(shift + i) % text.len()]).collect();
            let seg = order[k % order.len()];
            let w: [u8; 8] = window.try_into().unwrap();
            feed(&mut d, ps_group(&w, seg));
            if !d.ps.is_empty() {
                shown.push(d.ps.clone());
            }
        }
        assert!(!shown.is_empty());
        assert!(
            shown.iter().all(|s| !s.contains('?') && !s.contains('_')),
            "{shown:?}"
        );
    }

    #[test]
    fn a_corrected_group_needs_a_repeat_and_raw_words_must_differ() {
        let mut d = GroupDecoder::default();
        let mut g = ps_group(b"SDRKIT  ", 0);
        g.blocks[3] = Some(Block {
            data: 0x5344,
            raw: 0x5345,
            corrected: true,
            voted: false,
            c_prime: false,
        });
        assert!(!feed(&mut d, g));
        // The same corrected words again do not confirm each other.
        assert!(!feed(&mut d, g));
        // A clean copy confirms.
        assert!(feed(&mut d, ps_group(b"SDRKIT  ", 0)));
        assert!(feed(&mut d, g));
        // A corrected C alone: the group is used without C.
        let mut d = GroupDecoder::default();
        let mut g = Group::clean(PI, b0(0), (45 << 8) | 80, 0x5455);
        g.blocks[2] = Some(Block {
            data: (45 << 8) | 80,
            raw: 1,
            corrected: true,
            voted: false,
            c_prime: false,
        });
        assert!(feed(&mut d, g));
        assert!(d.af_mhz().is_empty());
        assert_eq!(d.ps_partial, "TU______");
    }
}
