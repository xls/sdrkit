//! Interrogations on 1030 MHz: Mode A and Mode C, the intermode all-calls, and Mode S uplink formats with the address recovered
//! from the address/parity field. [`UplinkDecoder`] finds them and keeps the tables.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.1 and 3.1.2

use crate::crc;
use crate::generator::{Burst, Renderer, Rng};
use crate::settings::EXPIRY_S;
use crate::typed::{
    uplink_tables, AddressedRow, InterrogationEvent, InterrogatorRow, UplinkFields, ADDRESSED,
    BY_UF, INTERROGATION, INTERROGATORS,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, EventDef, GenConfig, Generator, Hz, LockState, Record, Results,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write;

sdrkit_core::choice_enum! {
    /// What kind of interrogation.
    pub enum InterrogationKind {
        /// Mode A (P1-P3 8 µs).
        ModeA = ("mode_a", "Mode A"),
        /// Mode C (P1-P3 21 µs).
        ModeC = ("mode_c", "Mode C"),
        /// Mode A/C/S all-call (P4 short after Mode A).
        AllCallA = ("all_call_a", "Mode A/C/S all-call"),
        /// Mode C/S all-call (P4 short after Mode C).
        AllCallC = ("all_call_c", "Mode C/S all-call"),
        /// Mode A/C-only all-call (P4 long).
        AcOnlyAllCall = ("ac_only_all_call", "Mode A/C-only all-call"),
        /// Mode S (P6).
        ModeS = ("mode_s", "Mode S"),
    }
}

/// A decoded interrogation.
#[derive(Clone, Debug, PartialEq)]
pub struct Interrogation {
    /// Signal time of P1, s.
    pub time: f64,
    /// Kind.
    pub kind: InterrogationKind,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// The P2 (side-lobe suppression) pulse was seen.
    pub p2: bool,
    /// Mode S: the uplink format.
    pub uf: Option<u8>,
    /// Mode S: the message (hex).
    pub hex: Option<String>,
    /// Mode S: the address from the parity (0xFFFFFF for an all-call); a damaged message also gives one.
    pub address: Option<u32>,
    /// Mode S to one aircraft: the address is in the table of addressed aircraft (see [`UplinkDecoder`]); no value
    /// before the decoder has taken the message, or without one aircraft as the address.
    pub confirmed: Option<bool>,
    /// Mode S: a description of the fields.
    pub fields: String,
    /// Mode S: the weakest chip decision against the mean decision, 0 to 1 or more.
    pub margin: Option<f32>,
}

impl Item for Interrogation {
    const KIND: &'static str = "uplink.interrogation";
    fn event_def() -> Option<&'static EventDef> {
        Some(INTERROGATION.def())
    }
    fn write_fields(&self, r: &mut Record) {
        let target = self
            .address
            .filter(|_| self.confirmed == Some(true))
            .map(|a| Hex24(a).to_string());
        InterrogationEvent {
            kind: self.kind,
            level_dbfs: f64::from(self.level_dbfs),
            p2: self.p2,
            uf: self.uf.map(i64::from),
            hex: self.hex.as_deref(),
            target: target.as_deref(),
            confirmed: self.confirmed,
            text: (!self.fields.is_empty()).then_some(self.fields.as_str()),
            family: keys::Family::Aircraft,
        }
        .write(r);
    }
}

/// Decodes the fields of an uplink message: (UF, description, address).
pub fn decode_uf(bytes: &[u8]) -> (u8, String, u32) {
    let uf = if bytes[0] >> 6 == 3 {
        24
    } else {
        bytes[0] >> 3
    };
    let f = |a: usize, b: usize| -> u64 {
        let mut v = 0u64;
        for i in a..=b {
            // A field past the end of a short message reads as zeros.
            let b = bytes.get((i - 1) / 8).copied().unwrap_or(0);
            v = v << 1 | ((b >> (7 - (i - 1) % 8)) & 1) as u64;
        }
        v
    };
    let rem = crc::remainder(bytes);
    let addr = crc::uplink_address(rem);
    let s = match uf {
        0 => format!(
            "short air-air surveillance, RL {}, AQ {}, DS {:02X}",
            f(9, 9),
            f(14, 14),
            f(15, 22)
        ),
        4 | 5 | 20 | 21 => {
            let what = match uf {
                4 => "surveillance, altitude request",
                5 => "surveillance, identity request",
                20 => "Comm-A, altitude request",
                _ => "Comm-A, identity request",
            };
            let mut s = format!(
                "{what}, PC {}, RR {}, DI {}, SD {:04X}",
                f(6, 8),
                f(9, 13),
                f(14, 16),
                f(17, 32)
            );
            if uf >= 20 {
                s.push_str(&format!(", MA {:014X}", f(33, 88)));
            }
            s
        }
        11 => {
            let (pr, ic, cl) = (f(6, 9), f(10, 13), f(14, 16));
            let who = match cl {
                0 => format!("II {ic}"),
                1..=4 => format!("SI {}", ic + 16 * (cl - 1)),
                _ => format!("IC {ic} CL {cl}"),
            };
            format!("Mode S only all-call, PR {pr}, {who}")
        }
        16 => format!(
            "long air-air surveillance, RL {}, AQ {}, MU {:014X}",
            f(9, 9),
            f(14, 14),
            f(33, 88)
        ),
        24 => format!("Comm-C ELM, RC {}, NC {}", f(3, 4), f(5, 8)),
        n => format!("UF{n}"),
    };
    (uf, s, addr)
}

/// Encodes an uplink message (7 or 14 bytes with the AP field) addressed to `address` (0xFFFFFF for an all-call).
pub fn encode_uf(fields: &[u8], address: u32) -> Vec<u8> {
    let mut v = fields.to_vec();
    let n = v.len() - 3;
    let p = crc::parity(&v[..n]) ^ crc::uplink_overlay(address & 0xFF_FFFF);
    v[n] = (p >> 16) as u8;
    v[n + 1] = (p >> 8) as u8;
    v[n + 2] = p as u8;
    v
}

#[derive(Clone, Copy, Debug)]
struct Pulse {
    start: f64,
    end: f64,
    power: f32,
}

/// A message whose weakest chip decision is under this share of its mean decision does not count toward listing an
/// address. Over the test signal at 20 dB SNR, 8 % of intact messages and 99 % of those with a wrong address are under.
const MIN_MARGIN: f32 = 0.2;

/// Clear messages to an unlisted address, within the expiry time, that list it.
const CONFIRM_COUNT: u64 = 2;

/// The interrogation decoder and its tables.
pub struct UplinkDecoder {
    fs: f64,
    us: f64,
    epoch: f64,
    delay: f64,
    base: u64,
    z: Vec<Complex32>,
    total: u64,
    next: u64,
    blocks: VecDeque<f32>,
    /// Scratch for the noise quantile (no allocation while decoding).
    sorted: Vec<f32>,
    blk_sum: f64,
    blk_n: usize,
    noise: Option<f32>,
    in_pulse: Option<(f64, f64, f64, u64, f32)>,
    low_run: u64,
    pulses: VecDeque<Pulse>,
    expiry_s: f64,
    counts: [u64; 6],
    uf_counts: [u64; 32],
    addressed: ObjectTable<Hex24, Addressed>,
    /// Addresses heard in clear messages but not yet [`CONFIRM_COUNT`] times.
    pending: ObjectTable<Hex24, ()>,
    interrogators: ObjectTable<String, Interrogator>,
    published: Published,
    last: f64,
    out: Vec<Interrogation>,
    summary: String,
}

impl Default for UplinkDecoder {
    fn default() -> Self {
        UplinkDecoder {
            fs: 8e6,
            us: 8.0,
            epoch: 0.0,
            delay: 0.0,
            base: 0,
            z: Vec::new(),
            total: 0,
            next: 0,
            blocks: VecDeque::new(),
            sorted: Vec::with_capacity(64),
            blk_sum: 0.0,
            blk_n: 0,
            noise: None,
            in_pulse: None,
            low_run: 0,
            pulses: VecDeque::new(),
            expiry_s: 60.0,
            counts: [0; 6],
            uf_counts: [0; 32],
            addressed: ObjectTable::new(4096),
            pending: ObjectTable::new(4096),
            interrogators: ObjectTable::new(256),
            published: Published::new(UplinkFields::SHAPE.fields).with_tables(&uplink_tables()),
            last: f64::MIN,
            out: Vec::new(),
            summary: String::new(),
        }
    }
}

impl UplinkDecoder {
    fn reset_stream(&mut self, fs: f64, delay: f64) {
        if self.fs > 0.0 {
            self.epoch += self.total as f64 / self.fs;
        }
        self.fs = fs;
        self.us = fs * 1e-6;
        self.delay = delay;
        self.base = 0;
        self.z.clear();
        self.total = 0;
        self.next = 0;
        self.blocks.clear();
        self.noise = None;
        self.in_pulse = None;
        self.pulses.clear();
    }

    fn zs(&self, a: f64, b: f64) -> Complex32 {
        let i0 = (a.ceil().max(self.base as f64) as u64 - self.base) as usize;
        let i1 = ((b.floor() as u64).saturating_sub(self.base) as usize)
            .min(self.z.len().saturating_sub(1));
        let mut s = Complex32::new(0.0, 0.0);
        if i1 < i0 {
            let i = (a.round() as u64).saturating_sub(self.base) as usize;
            return self.z.get(i).copied().unwrap_or_default();
        }
        for v in &self.z[i0..=i1] {
            s += v;
        }
        s
    }

    /// Feeds samples, appending decoded interrogations.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<Interrogation>) {
        for &s in iq {
            self.z.push(s);
            let p = s.norm_sqr();
            self.blk_sum += p as f64;
            self.blk_n += 1;
            if self.blk_n == 256 {
                if self.blocks.len() == 64 {
                    self.blocks.pop_front();
                }
                self.blocks.push_back((self.blk_sum / 256.0) as f32);
                if self.blocks.len() >= 8 {
                    self.sorted.clear();
                    self.sorted.extend(self.blocks.iter().copied());
                    self.sorted.sort_by(f32::total_cmp);
                    self.noise = Some((self.sorted[self.sorted.len() / 4] / 0.93).max(1e-9));
                }
                self.blk_sum = 0.0;
                self.blk_n = 0;
            }
            let n = self.total;
            self.total += 1;
            let Some(noise) = self.noise else { continue };
            let th = 10.0 * noise;
            let x = n as f64;
            match self.in_pulse.as_mut() {
                None => {
                    if p > th {
                        self.in_pulse = Some((x, x, p as f64, 1, p));
                        self.low_run = 0;
                    }
                }
                Some(ip) => {
                    ip.4 = ip.4.max(p);
                    // Hysteresis: the pulse goes on while above half the threshold and above the ringing of a strong pulse.
                    if p > (0.5 * th).max(0.05 * ip.4) {
                        ip.1 = x;
                        ip.2 += p as f64;
                        ip.3 += 1;
                        self.low_run = 0;
                    } else {
                        self.low_run += 1;
                        // A gap of more than 0.35 µs ends the pulse (a phase reversal dips for less).
                        if self.low_run as f64 > 0.35 * self.us {
                            let (a, b, sum, cnt, _) = *ip;
                            self.in_pulse = None;
                            if self.pulses.len() == 64 {
                                self.pulses.pop_front();
                            }
                            self.pulses.push_back(Pulse {
                                start: a,
                                end: b + 1.0,
                                power: (sum / cnt as f64) as f32,
                            });
                            self.decide(out);
                        }
                    }
                }
            }
        }
        self.decide(out);
        let keep = self
            .pulses
            .front()
            .map_or(self.total, |p| p.start as u64)
            .saturating_sub((40.0 * self.us) as u64);
        if keep > self.base + 65536 {
            let d = (keep - self.base) as usize;
            self.z.drain(..d);
            self.base = keep;
        }
    }

    fn time_of(&self, x: f64) -> f64 {
        self.epoch + x / self.fs - self.delay
    }

    /// Decides on the pulses older than 40 µs (so a P6, or a P3 21 µs later and its P4, have arrived).
    fn decide(&mut self, out: &mut Vec<Interrogation>) {
        let us = self.us;
        let now = self.total as f64;
        while let Some(&p1) = self.pulses.front() {
            if now - p1.start < 40.0 * us {
                break;
            }
            self.pulses.pop_front();
            let w1 = (p1.end - p1.start) / us;
            if !(0.5..=1.2).contains(&w1) {
                continue;
            }
            let near = |t: f64, tol: f64, ps: &VecDeque<Pulse>| {
                ps.iter()
                    .position(|p| (p.start - (p1.start + t * us)).abs() <= tol * us)
            };
            let p2 = near(2.0, 0.25, &self.pulses);
            // Mode S: P6 at 3.5 µs, long.
            if let Some(i6) = near(3.5, 0.3, &self.pulses) {
                let p6 = self.pulses[i6];
                let len = (p6.end - p6.start) / us;
                if len > 14.0 {
                    if let Some(r) = self.mode_s(p1, p6, p2.is_some()) {
                        for _ in 0..=i6 {
                            self.pulses.pop_front();
                        }
                        out.push(r);
                    }
                    continue;
                }
            }
            for (t, kind) in [
                (8.0, InterrogationKind::ModeA),
                (21.0, InterrogationKind::ModeC),
            ] {
                if let Some(i3) = near(t, 0.25, &self.pulses) {
                    let p3 = self.pulses[i3];
                    let w3 = (p3.end - p3.start) / us;
                    if !(0.5..=1.2).contains(&w3) {
                        continue;
                    }
                    let mut k = kind;
                    let mut last = i3;
                    if let Some(i4) = self
                        .pulses
                        .iter()
                        .position(|p| (p.start - (p3.start + 2.0 * us)).abs() <= 0.25 * us)
                    {
                        let w4 = (self.pulses[i4].end - self.pulses[i4].start) / us;
                        k = if w4 > 1.2 {
                            InterrogationKind::AcOnlyAllCall
                        } else if kind == InterrogationKind::ModeA {
                            InterrogationKind::AllCallA
                        } else {
                            InterrogationKind::AllCallC
                        };
                        last = i4;
                    }
                    for _ in 0..=last {
                        self.pulses.pop_front();
                    }
                    out.push(Interrogation {
                        time: self.time_of(p1.start),
                        kind: k,
                        level_dbfs: 10.0 * p1.power.max(1e-12).log10(),
                        p2: p2.is_some(),
                        uf: None,
                        hex: None,
                        address: None,
                        confirmed: None,
                        fields: String::new(),
                        margin: None,
                    });
                    break;
                }
            }
        }
    }

    fn mode_s(&self, p1: Pulse, p6: Pulse, p2: bool) -> Option<Interrogation> {
        let us = self.us;
        // The phase change across sample t: (the product of the two sides, its cosine).
        let turn = |t: f64| -> (f32, f32) {
            let a = self.zs(t - 0.2 * us, t - 0.05 * us);
            let b = self.zs(t + 0.05 * us, t + 0.2 * us);
            let d = (b * a.conj()).re;
            (d, d / (a.norm() * b.norm()).max(f32::MIN_POSITIVE))
        };
        // The sync phase reversal 1.25 µs into P6: the earliest turn of more than 120° in the search, at its deepest
        // point. P6 has no reversal before its sync. The edge of a weak P6 can be found up to 0.5 µs late, which brings
        // the reversal of data bit 1, 0.5 µs after the sync, near the expected place; the search starts 0.6 µs early.
        let c = (p6.start + 1.25 * us).round();
        let (lo, hi) = ((-0.6 * us).floor() as i64, (0.3 * us).ceil() as i64);
        let at = |k: i64| c + k as f64;
        let mut k = (lo..=hi).find(|&k| turn(at(k)).1 < -0.5)?;
        while k < hi && turn(at(k + 1)).0 < turn(at(k)).0 {
            k += 1;
        }
        let ts = at(k);
        let chip = |k: i64| {
            let a = ts + 0.5 * us + k as f64 * 0.25 * us;
            self.zs(a + 0.06 * us, a + 0.19 * us)
        };
        // The first bit of the format gives the length (UF 0 to 15 have 56 bits, UF 16 to 31 have 112); the measured
        // end of P6 can fall short in noise.
        let bits = if (chip(0) * chip(-1).conj()).re < 0.0 {
            112
        } else {
            56
        };
        let mut bytes = vec![0u8; bits / 8];
        let mut prev = chip(-1);
        let (mut weakest, mut sum) = (f32::MAX, 0.0f32);
        for k in 0..bits {
            let c = chip(k as i64);
            let d = (c * prev.conj()).re;
            if d < 0.0 {
                bytes[k / 8] |= 0x80 >> (k % 8);
            }
            weakest = weakest.min(d.abs());
            sum += d.abs();
            prev = c;
        }
        let margin = weakest * bits as f32 / sum.max(f32::MIN_POSITIVE);
        let (uf, fields, addr) = decode_uf(&bytes);
        let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
        Some(Interrogation {
            time: self.time_of(p1.start),
            kind: InterrogationKind::ModeS,
            level_dbfs: 10.0 * p6.power.max(1e-12).log10(),
            p2,
            uf: Some(uf),
            hex: Some(hex),
            address: Some(addr),
            confirmed: None,
            fields,
            margin: Some(margin),
        })
    }

    /// Counts `r` and enters it in the tables.
    ///
    /// A Mode S message enters the tables only in an assigned format (UF 0, 4, 5, 11, 16, 20, 21, 24). UF11 overlays
    /// its parity with an address of all ones, so a UF11 with another remainder is damaged and is left out. The other
    /// formats overlay the parity with the aircraft address (all ones for a broadcast, which has no row): any message,
    /// damaged or not, gives an address. Such an address becomes a row only when [`CONFIRM_COUNT`] clear messages (see
    /// [`MIN_MARGIN`]) give it within the expiry time; a listed address is then updated by every message.
    ///
    /// Returns whether the address of a message to one aircraft is listed after `r`: false for a damaged UF11, an
    /// unassigned format or an address not yet confirmed; no value for a message without one aircraft as the address.
    fn take(&mut self, r: &Interrogation) -> Option<bool> {
        self.last = r.time;
        self.counts[usize::from(r.kind.index())] += 1;
        let (Some(uf), Some(a)) = (r.uf, r.address) else {
            return None;
        };
        if !matches!(uf, 0 | 4 | 5 | 11 | 16 | 20 | 21 | 24) || (uf == 11 && a != 0xFF_FFFF) {
            return Some(false);
        }
        self.uf_counts[usize::from(uf)] += 1;
        if uf == 11 {
            if let Some(who) = r.fields.split(", ").nth(2) {
                let _ = self
                    .interrogators
                    .upsert(who, Stamp::at(r.time), || Interrogator {
                        code: who.to_string(),
                    });
            }
            return None;
        }
        if a == 0xFF_FFFF {
            return None;
        }
        let key = Hex24(a);
        if !self.addressed.contains(&key) && !self.confirms(key, r) {
            return Some(false);
        }
        if let Ok(up) = self
            .addressed
            .upsert(&key, Stamp::at(r.time), || Addressed {
                address: a,
                ..Default::default()
            })
        {
            up.row.last.clone_from(&r.fields);
            *up.row.formats.entry(uf).or_default() += 1;
        }
        Some(self.addressed.contains(&key))
    }

    /// Notes a clear message `r` to the unlisted address `key`; true when it is the [`CONFIRM_COUNT`]th within the
    /// expiry time.
    fn confirms(&mut self, key: Hex24, r: &Interrogation) -> bool {
        if r.margin.is_none_or(|m| m < MIN_MARGIN) {
            return false;
        }
        if self
            .pending
            .seen(&key)
            .is_some_and(|s| r.time - s.first_s > self.expiry_s)
        {
            self.pending.remove(&key);
        }
        let heard = match self.pending.upsert(&key, Stamp::at(r.time), || ()) {
            Ok(_) => self.pending.seen(&key).map_or(0, |s| s.count),
            Err(_) => 0,
        };
        if heard < CONFIRM_COUNT {
            return false;
        }
        self.pending.remove(&key);
        true
    }

    fn publish(&mut self, now: f64) {
        self.addressed.expire(now, self.expiry_s);
        self.pending.expire(now, self.expiry_s);
        self.interrogators.expire(now, self.expiry_s);
        let total: u64 = self.counts.iter().sum();
        let lock_state = if now - self.last <= 5.0 {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = self.noise.map(|x| 10.0 * x.log10());
        self.summary.clear();
        let _ = write!(
            self.summary,
            "{total} interrogations, {} addressed aircraft",
            self.addressed.len()
        );
        let p = &mut self.published;
        let summary = &self.summary;
        p.results_mut().update_status(|s: &mut Status| {
            let changed =
                s.lock_state != lock_state || s.level_dbfs != level || s.summary != *summary;
            if changed {
                s.lock_state = lock_state;
                s.level_dbfs = level;
                s.summary.clone_from(summary);
            }
            changed
        });
        let of = |k: InterrogationKind| self.counts[usize::from(k.index())] as i64;
        p.put(&UplinkFields {
            interrogations: total as i64,
            mode_a: of(InterrogationKind::ModeA),
            mode_c: of(InterrogationKind::ModeC),
            mode_s: of(InterrogationKind::ModeS),
            all_call_a: of(InterrogationKind::AllCallA),
            all_call_c: of(InterrogationKind::AllCallC),
            ac_only_all_call: of(InterrogationKind::AcOnlyAllCall),
        });
        sdrkit_util_objects::shaped::rows(&self.addressed, &ADDRESSED, now, p);
        sdrkit_util_objects::shaped::rows(&self.interrogators, &INTERROGATORS, now, p);
        crate::typed::count_rows(p, &BY_UF, &self.uf_counts);
    }
}

/// A 24-bit address as a table key. Its text is six hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
pub struct Hex24(pub u32);

impl std::fmt::Display for Hex24 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06X}", self.0 & 0xFF_FFFF)
    }
}

/// An aircraft that interrogations address.
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Addressed {
    address: u32,
    last: String,
    formats: BTreeMap<u8, u64>,
}

impl Shaped for Addressed {
    type Row = AddressedRow<'static>;
    /// The row of [`ADDRESSED`].
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut fm = String::new();
        for (u, n) in &self.formats {
            if !fm.is_empty() {
                fm.push(' ');
            }
            let _ = write!(fm, "UF{u}x{n}");
        }
        let row = AddressedRow {
            icao: &Hex24(self.address).to_string(),
            interrogations: columns::heard(seen),
            formats: &fm,
            last: &self.last,
            last_heard: columns::last_heard(seen).map(Time),
        };
        out.put(&row);
    }
}

/// An interrogator heard in all-calls.
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Interrogator {
    code: String,
}

impl Shaped for Interrogator {
    type Row = InterrogatorRow<'static>;
    /// The row of [`INTERROGATORS`].
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let row = InterrogatorRow {
            code: &self.code,
            all_calls: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        };
        out.put(&row);
    }
}

sdrkit_core::commands! {
    /// The commands of [`UplinkDecoder`].
    pub enum UplinkCommand {
        /// Forgets the interrogations heard.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

impl Component for UplinkDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.uplink_decoder",
            vec![Port::iq("in")],
            vec![Port::typed::<Interrogation>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(UplinkDecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 7.9e6 {
            return Err(ConfigError::RateTooLow {
                need: 8e6,
                got: f.rate,
            });
        }
        self.reset_stream(f.rate, 0.0);
        Ok(vec![Format::new(Kind::typed::<Interrogation>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut out = std::mem::take(&mut self.out);
        out.clear();
        self.feed(iq, &mut out);
        for r in &mut out {
            r.confirmed = self.take(r);
        }
        let o = io.items_out::<Interrogation>(0);
        for r in &out {
            o.push(Timed {
                time: r.time,
                value: r.clone(),
            });
        }
        self.out = out;
        if io.tick() {
            let now = self.time_of(self.total as f64);
            self.publish(now);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match UplinkDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            UplinkDecoderSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(UplinkDecoderSetting::ExpiryS, Value::F64(self.expiry_s))])
    }
    fn commands(&self) -> &Commands {
        UplinkCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match UplinkCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            UplinkCommand::Clear => {
                self.counts = [0; 6];
                self.uf_counts = [0; 32];
                self.addressed.clear();
                self.pending.clear();
                self.interrogators.clear();
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

sdrkit_core::settings! {
    /// The settings of [`UplinkDecoder`].
    pub enum UplinkDecoderSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
    }
}

// ---------------------------------------------------------------- the generator

/// A burst of an interrogation: P1, P2 and P6 with its phase reversals.
pub fn mode_s_burst(t: f64, msg: &[u8], amp: f32, freq: f64, phase: f64) -> Burst {
    let us = 1e-6;
    let mut edges = vec![
        (t, 1.0f32),
        (t + 0.8 * us, -1.0),
        (t + 2.0 * us, 1.0),
        (t + 2.8 * us, -1.0),
    ];
    let t6 = t + 3.5 * us;
    let ts = t6 + 1.25 * us;
    let mut sign = 1.0f32;
    edges.push((t6, 1.0));
    edges.push((ts, -2.0 * sign));
    sign = -sign;
    let bits = msg.len() * 8;
    for k in 0..bits {
        if msg[k / 8] >> (7 - k % 8) & 1 == 1 {
            edges.push((ts + 0.5 * us + k as f64 * 0.25 * us, -2.0 * sign));
            sign = -sign;
        }
    }
    let end = ts + 0.5 * us + bits as f64 * 0.25 * us + 0.5 * us;
    edges.push((end, -sign));
    Burst {
        start: t,
        edges,
        amp,
        freq,
        phase,
        end,
    }
}

/// A Mode A/C interrogation (with P2), optionally with a P4 (`Some(long)`).
pub fn mode_ac_burst(
    t: f64,
    mode_c: bool,
    p4: Option<bool>,
    amp: f32,
    freq: f64,
    phase: f64,
) -> Burst {
    let us = 1e-6;
    let t3 = if mode_c { 21.0 } else { 8.0 };
    let mut p = vec![(0.0, 0.8 * us), (2.0 * us, 0.8 * us), (t3 * us, 0.8 * us)];
    if let Some(long) = p4 {
        p.push(((t3 + 2.0) * us, if long { 1.6 } else { 0.8 } * us));
    }
    Burst::from_pulses(t, &p, amp, freq, phase)
}

/// The least amplitude of an interrogation of the test signal.
const AMP_MIN: f64 = 0.05;
/// The amplitude of an interrogation is uniform from [`AMP_MIN`] to this much above it.
const AMP_SPAN: f64 = 0.2;

/// What the 1030 MHz test signal declares: chips of 0.25 µs in about 4 MHz, at the mean power of an interrogation while it
/// is on, so the framework's SNR is that of a mean interrogation.
pub fn signal_info() -> SignalInfo {
    let hi = AMP_MIN + AMP_SPAN;
    SignalInfo {
        bandwidth_hz: 4e6,
        symbol_rate_hz: Some(4e6),
        bit_rate_hz: Some(4e6),
        power: (hi.powi(3) - AMP_MIN.powi(3)) / (3.0 * AMP_SPAN),
    }
}

/// The 1030 MHz test signal: a few interrogators interrogating a few aircraft with every kind of interrogation.
pub struct UplinkGenerator {
    r: Renderer,
    rng: Rng,
    next_t: f64,
    queue: VecDeque<Burst>,
    targets: Vec<u32>,
    /// Every Mode S interrogation sent: (time, message).
    pub log: Vec<(f64, Vec<u8>)>,
    /// Keep the log.
    pub keep_log: bool,
}

impl UplinkGenerator {
    /// A generator at `rate` (8 Msps or more).
    pub fn new(rate: Hz, seed: u64) -> UplinkGenerator {
        let mut rng = Rng::new(seed);
        let targets = (0..5)
            .map(|_| 0x400000 + rng.below(0x6FFFFF) as u32)
            .collect();
        UplinkGenerator {
            r: Renderer::new(rate, (0.4 * rate).min(6e6)),
            rng,
            next_t: 0.001,
            queue: VecDeque::new(),
            targets,
            log: Vec::new(),
            keep_log: false,
        }
    }
    /// From a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> UplinkGenerator {
        UplinkGenerator::new(cfg.rate_hz, cfg.seed)
    }
    /// The aircraft it interrogates.
    pub fn targets(&self) -> &[u32] {
        &self.targets
    }
    fn make(&mut self) -> Burst {
        let t = self.next_t;
        self.next_t += 0.0006 + self.rng.uniform() * 0.0008;
        let amp = (AMP_MIN + AMP_SPAN * self.rng.uniform()) as f32;
        let freq = self.rng.range(-20e3, 20e3);
        let ph = self.rng.range(0.0, std::f64::consts::TAU);
        let kind = self.rng.below(10);
        match kind {
            0 => mode_ac_burst(t, false, None, amp, freq, ph),
            1 => mode_ac_burst(t, true, None, amp, freq, ph),
            2 => mode_ac_burst(t, false, Some(false), amp, freq, ph),
            3 => mode_ac_burst(t, true, Some(true), amp, freq, ph),
            _ => {
                let a = self.targets[self.rng.below(self.targets.len() as u64) as usize];
                let uf = [0u8, 4, 5, 11, 16, 20, 21][self.rng.below(7) as usize];
                let long = matches!(uf, 16 | 20 | 21);
                let mut f = vec![0u8; if long { 14 } else { 7 }];
                f[0] = uf << 3;
                match uf {
                    4 | 5 | 20 | 21 => {
                        f[0] |= 0; // PC 0
                        f[1] = (self.rng.below(32) as u8) << 3; // RR
                        f[2] = self.rng.below(256) as u8;
                    }
                    11 => {
                        let ic = 1 + self.rng.below(14) as u8;
                        f[0] |= 0; // PR 0
                        f[1] = ic << 3;
                    }
                    _ => {}
                }
                if long {
                    for b in f.iter_mut().take(11).skip(4) {
                        *b = self.rng.below(256) as u8;
                    }
                }
                let msg = encode_uf(&f, if uf == 11 { 0xFF_FFFF } else { a });
                if self.keep_log {
                    self.log.push((t, msg.clone()));
                }
                mode_s_burst(t, &msg, amp, freq, ph)
            }
        }
    }
}

impl Generator for UplinkGenerator {
    fn rate_hz(&self) -> Hz {
        self.r.rate()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = self.r.time_of(self.r.now_index() + out.len() as u64) + 2e-6;
        while self.next_t < t_end {
            let b = self.make();
            self.queue.push_back(b);
        }
        while let Some(b) = self.queue.pop_front() {
            self.r.add(b);
        }
        self.r.render(out);
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uplink_fields() {
        let mut f = vec![0u8; 7];
        f[0] = 4 << 3;
        let m = encode_uf(&f, 0x4840D6);
        let (uf, s, a) = decode_uf(&m);
        assert_eq!((uf, a), (4, 0x4840D6));
        assert!(s.contains("altitude request"));
        let mut f = vec![0u8; 7];
        f[0] = 11 << 3;
        f[1] = 5 << 3;
        let (uf, s, a) = decode_uf(&encode_uf(&f, 0xFFFFFF));
        assert_eq!((uf, a), (11, 0xFFFFFF));
        assert!(s.contains("II 5"), "{s}");
    }

    /// The target and the flag of the event of `r`.
    fn event_of(r: &Interrogation) -> (Value, Value) {
        let decl = INTERROGATION.def().record_decl().unwrap().clone();
        let mut rec = Record::new(decl.clone());
        r.write_fields(&mut rec);
        let get = |k: &str| rec.value(decl.id(k).unwrap()).cloned().unwrap();
        (get("target"), get("confirmed"))
    }

    /// A message to `address` in UF4 at `time` with a clear decision margin.
    fn surveillance(time: f64, address: u32) -> Interrogation {
        let mut f = vec![0u8; 7];
        f[0] = 4 << 3;
        let msg = encode_uf(&f, address);
        let (uf, fields, a) = decode_uf(&msg);
        Interrogation {
            time,
            kind: InterrogationKind::ModeS,
            level_dbfs: -20.0,
            p2: false,
            uf: Some(uf),
            hex: Some(msg.iter().map(|b| format!("{b:02X}")).collect()),
            address: Some(a),
            confirmed: None,
            fields,
            margin: Some(1.0),
        }
    }

    #[test]
    fn an_event_names_its_address_only_once_it_is_confirmed() {
        let mut d = UplinkDecoder::default();
        let mut r = surveillance(1.0, 0x4840D6);
        r.confirmed = d.take(&r);
        assert_eq!(event_of(&r), (Value::None, Value::Bool(false)));
        let mut r = surveillance(2.0, 0x4840D6);
        r.confirmed = d.take(&r);
        assert_eq!(
            event_of(&r),
            (Value::Text("4840D6".into()), Value::Bool(true))
        );
        // A damaged UF11 gives an address other than all ones: no target.
        let mut f = vec![0u8; 7];
        f[0] = 11 << 3;
        let msg = encode_uf(&f, 0x123456);
        let (uf, fields, a) = decode_uf(&msg);
        let mut r = Interrogation {
            uf: Some(uf),
            address: Some(a),
            fields,
            ..surveillance(3.0, 0)
        };
        r.confirmed = d.take(&r);
        assert_eq!(event_of(&r), (Value::None, Value::Bool(false)));
        // An all-call has no single address.
        let msg = encode_uf(&f, 0xFF_FFFF);
        let (uf, fields, a) = decode_uf(&msg);
        let mut r = Interrogation {
            uf: Some(uf),
            address: Some(a),
            fields,
            ..surveillance(4.0, 0)
        };
        r.confirmed = d.take(&r);
        assert_eq!(event_of(&r), (Value::None, Value::None));
    }
}

#[cfg(test)]
mod signal_tests {
    use super::*;

    #[test]
    fn one_interrogation_decodes() {
        let mut f = vec![0u8; 7];
        f[0] = 4 << 3;
        f[1] = 0x28;
        let msg = encode_uf(&f, 0x4840D6);
        let mut r = Renderer::new(8e6, 3.2e6);
        r.add(mode_s_burst(0.0002, &msg, 0.2, 5e3, 0.4));
        let mut x = vec![Complex32::new(0.0, 0.0); 8000];
        r.render(&mut x);
        // Noise 40 dB below a pulse of amplitude 0.1 in 2 MHz.
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(4e-6, 1).unwrap();
        awgn.process(&mut x);
        let mut d = UplinkDecoder::default();
        d.reset_stream(8e6, 0.0);
        let mut out = Vec::new();
        // Noise first, so the floor is known.
        let mut n = vec![Complex32::new(0.0, 0.0); 4000];
        sdrkit_algo_sim::noise::Awgn::with_power(4e-6, 2)
            .unwrap()
            .process(&mut n);
        d.feed(&n, &mut out);
        d.feed(&x, &mut out);
        d.feed(&n, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(
            out[0].hex.as_deref().unwrap(),
            msg.iter().map(|b| format!("{b:02X}")).collect::<String>()
        );
        assert_eq!(out[0].address, Some(0x4840D6));
    }

    /// The decoder over `seconds` of the test signal of `seed` with noise at `snr_db`: the addresses it sent, the
    /// addresses listed, the interrogator codes listed and the addresses of the events that are confirmed.
    fn listed(seed: u64, snr_db: f64, seconds: f64) -> Listed {
        use sdrkit_components::sim::{impaired, with_generator_settings, NoiseKind};
        let rate = 10e6;
        let schema = with_generator_settings(&crate::settings::UplinkSetting::schema()).unwrap();
        let mut v = Values::defaults(&schema);
        let noise = Value::Choice(Choice::index(NoiseKind::Snr));
        v.set(schema.id("gen_noise").unwrap(), &noise).unwrap();
        v.set(schema.id("gen_snr_db").unwrap(), &Value::F64(snr_db))
            .unwrap();
        v.set(schema.id("gen_seed").unwrap(), &Value::I64(seed as i64))
            .unwrap();
        let g = UplinkGenerator::new(rate, seed);
        let sent = g.targets().to_vec();
        let mut g = impaired(Box::new(g), &v).unwrap();
        let mut d = UplinkDecoder::default();
        d.reset_stream(rate, 0.0);
        let mut x = vec![Complex32::new(0.0, 0.0); 1 << 16];
        let mut out = Vec::new();
        let mut confirmed = Vec::new();
        for _ in 0..(seconds * rate / x.len() as f64) as usize {
            g.generate(&mut x);
            out.clear();
            d.feed(&x, &mut out);
            for r in &mut out {
                r.confirmed = d.take(r);
                if r.confirmed == Some(true) {
                    confirmed.extend(r.address);
                }
            }
        }
        let addressed = d.addressed.iter().map(|r| r.key.0).collect();
        let codes = d.interrogators.iter().map(|r| r.key.clone()).collect();
        (sent, addressed, codes, confirmed)
    }

    /// What [`listed`] gives.
    type Listed = (Vec<u32>, Vec<u32>, Vec<String>, Vec<u32>);

    /// The test signal at 20 dB SNR, six seeds of 3 s: every address listed, and every address of a confirmed event,
    /// was sent, and all 30 sent are listed.
    #[test]
    fn noise_lists_only_addresses_that_were_sent() {
        let (mut wrong, mut found, mut total) = (Vec::new(), 0, 0);
        for seed in 1..=6 {
            let (sent, addressed, codes, confirmed) = listed(seed, 20.0, 3.0);
            wrong.extend(
                addressed
                    .iter()
                    .chain(&confirmed)
                    .filter(|a| !sent.contains(a))
                    .map(|&a| (seed, Hex24(a))),
            );
            found += sent.iter().filter(|a| addressed.contains(a)).count();
            total += sent.len();
            // The test signal sends II codes 1 to 14 in UF11.
            assert!(
                codes
                    .iter()
                    .all(|c| (1..=14).any(|i| *c == format!("II {i}"))),
                "seed {seed}: {codes:?}"
            );
        }
        assert!(wrong.is_empty(), "listed but not sent: {wrong:?}");
        assert_eq!(found, total, "{found} of {total} sent addresses listed");
    }
}
