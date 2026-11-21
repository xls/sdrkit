//! One DCR channel: the frame after each sync, the RICH, the SACCH units and their messages, the call sign of the
//! synchronous burst, the traffic halves and the call on the channel.

use super::{DcrRowKind, Shared};
use crate::decoder::Ctx;
use crate::items::{CallDetail, CallKind, CallRecord, Encryption, Ident};
use crate::symbols::{carrier_gone, Sym, SymAnswer, SyncAnswer};
use sdrkit_core::Value;
use sdrkit_util_bits::pack::{push_u64, to_u64};

/// Symbols of the frame sync.
pub const SYNC: usize = 10;
/// Symbols of a frame, the sync included.
pub const FRAME: usize = 192;
/// A missed sync is taken at its nominal place at most this many times in a row.
const FLYWHEEL: u32 = 2;
/// Frames in a row that decode nothing before the channel is lost.
const EMPTY_FRAMES: u32 = 3;
/// A call ends after this long without a good frame, s.
pub const CALL_LOST_S: f64 = 1.5;
/// The two frames that confirm a message lie less than this apart, s.
const CONFIRM_S: f64 = 1.0;
/// A call sign is shown at most once in this time, s.
const SIGN_REPEAT_S: f64 = 10.0;
/// A data block or a manufacturer message is shown at most once in this time, s.
const TEXT_REPEAT_S: f64 = 5.0;
/// An idle message is shown at most once in this time, s.
const IDLE_REPEAT_S: f64 = 10.0;
/// A call sign heard before a call is attached to a call that opens within this time, s.
const SIGN_WAITS_S: f64 = 5.0;

/// The message type of an idle message.
pub const TYPE_IDLE: u8 = 0;
/// The message type of a voice communication.
pub const TYPE_VOICE: u8 = 1;
/// The message type of the end of a communication.
pub const TYPE_END: u8 = 30;

/// The radio information channel: 8 bits, the first sent the most significant, with even parity over all 8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rich {
    /// A service channel frame (F = 1); a synchronous burst when false.
    pub service: bool,
    /// The traffic layout M, 0 to 7.
    pub layout: u8,
    /// The D bit (false: direct).
    pub relay: bool,
}

impl Rich {
    /// The RICH of the 8 bits `v`, or `None` when the parity fails.
    pub fn decode(v: u8) -> Option<Rich> {
        v.count_ones().is_multiple_of(2).then_some(Rich {
            service: v & 0x80 != 0,
            layout: (v >> 2) & 7,
            relay: v & 2 != 0,
        })
    }

    /// The 8 bits sent, the parity bit last.
    pub fn byte(&self) -> u8 {
        let v = (u8::from(self.service) << 7) | ((self.layout & 7) << 2) | (u8::from(self.relay) << 1);
        v | (v.count_ones() % 2) as u8
    }
}

/// What a traffic half of a service channel frame holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    /// Two voice frames.
    Voice,
    /// A coded data block of 80 bits.
    Data,
    /// Nothing that is decoded.
    Nothing,
}

/// What half `half` (0 for TCH1) holds in the traffic layout `layout` (MIC report chapter 3, table 3-2).
pub fn half_of(layout: u8, half: usize) -> Half {
    match (layout, half) {
        (1, 0) | (2, 1) | (4, _) => Half::Data,
        (1, 1) | (2, 0) | (3, _) => Half::Voice,
        _ => Half::Nothing,
    }
}

/// A SACCH message: its type, the fields of its first unit and the payload of its further units.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    /// The message type, 0 to 31.
    pub kind: u8,
    /// The call type: 0 normal, 1 privacy scrambled.
    pub call_type: u8,
    /// The user code, 0 to 511.
    pub user_code: u16,
    /// The manufacturer number, 0 to 127.
    pub manufacturer: u8,
    /// The 18 bits of each unit after the first, first sent first.
    pub extra: Vec<u8>,
}

impl Message {
    /// A message of one unit.
    pub fn new(kind: u8, call_type: u8, user_code: u16, manufacturer: u8) -> Message {
        Message {
            kind,
            call_type,
            user_code,
            manufacturer,
            extra: Vec::new(),
        }
    }

    /// The units of the message, 26 information bits each: the lead flag, the units still to come (Wr), the type and 18
    /// payload bits. The extra bits fill units of 18 bits, the last one padded with zeros; at most 4 units.
    pub fn units(&self) -> Vec<Vec<u8>> {
        let more = self.extra.len().div_ceil(18).min(3);
        let mut out = Vec::with_capacity(more + 1);
        for i in 0..=more {
            let mut u = Vec::with_capacity(26);
            u.push(u8::from(i == 0));
            push_u64(&mut u, (more - i) as u64, 2);
            push_u64(&mut u, u64::from(self.kind & 31), 5);
            if i == 0 {
                push_u64(&mut u, u64::from(self.call_type & 3), 2);
                push_u64(&mut u, u64::from(self.user_code & 511), 9);
                push_u64(&mut u, u64::from(self.manufacturer & 127), 7);
            } else {
                let at = (i - 1) * 18;
                u.extend((0..18).map(|k| self.extra.get(at + k).copied().unwrap_or(0)));
            }
            out.push(u);
        }
        out
    }

    /// The extra bits as hex digits, the first bit the most significant, the last byte filled with zeros.
    pub fn extra_hex(&self) -> String {
        hex_of_bits(&self.extra)
    }
}

/// Bits as upper-case hex digits, the first bit the most significant, the last byte filled with zeros.
pub fn hex_of_bits(bits: &[u8]) -> String {
    bits.chunks(8)
        .map(|c| {
            let b = c.iter().enumerate().fold(0u8, |a, (i, &x)| a | ((x & 1) << (7 - i)));
            format!("{b:02X}")
        })
        .collect()
}

/// The SACCH units of one message as they arrive: at most 4 units with the same type, the first with the lead flag.
#[derive(Clone, Debug, Default)]
pub struct Units {
    /// The message so far and the Wr that the next unit must carry.
    open: Option<(Message, u8)>,
}

impl Units {
    /// Takes the 26 information bits of one unit. Returns the message that it completes.
    pub fn push(&mut self, u: &[u8]) -> Option<Message> {
        let lead = u[0] == 1;
        let wr = to_u64(&u[1..3]) as u8;
        let kind = to_u64(&u[3..8]) as u8;
        let payload = &u[8..26];
        if lead {
            let m = Message::new(
                kind,
                to_u64(&payload[0..2]) as u8,
                to_u64(&payload[2..11]) as u16,
                to_u64(&payload[11..18]) as u8,
            );
            if wr == 0 {
                self.open = None;
                return Some(m);
            }
            self.open = Some((m, wr - 1));
            return None;
        }
        match self.open.take() {
            Some((mut m, next)) if m.kind == kind && next == wr => {
                m.extra.extend_from_slice(payload);
                if wr == 0 {
                    Some(m)
                } else {
                    self.open = Some((m, wr - 1));
                    None
                }
            }
            _ => None,
        }
    }

    /// Drops the message in assembly.
    pub fn clear(&mut self) {
        self.open = None;
    }
}

/// The call sign of the 80 bits of a PICH: 9 BCD digits, the first the most significant. `None` when a digit is above 9.
pub fn call_sign(pich: &[u8]) -> Option<String> {
    pich[..36]
        .chunks(4)
        .map(|d| char::from_digit(to_u64(d) as u32, 10))
        .collect()
}

/// The 80 bits of a PICH that carries the call sign `digits` (9 decimal digits).
pub fn pich_bits(digits: &str) -> Vec<u8> {
    let mut b = Vec::with_capacity(80);
    for c in digits.chars().take(9) {
        push_u64(&mut b, u64::from(c.to_digit(10).unwrap_or(0)), 4);
    }
    b.resize(80, 0);
    b
}

/// The name of a call type.
pub fn call_type_name(t: u8) -> &'static str {
    match t {
        0 => "normal",
        1 => "privacy scrambled",
        _ => "reserved",
    }
}

/// The text of a user code: "user code 0 (any)" for 0.
fn user_code_text(uc: u16) -> String {
    if uc == 0 {
        "user code 0 (any)".into()
    } else {
        format!("user code {uc}")
    }
}

/// The text of a manufacturer number: "manufacturer 0 (common)" for 0.
fn manufacturer_text(m: u8) -> String {
    if m == 0 {
        "manufacturer 0 (common)".into()
    } else {
        format!("manufacturer {m}")
    }
}

/// The call on a channel.
#[derive(Clone, Debug)]
struct OpenCall {
    message: Message,
    start_s: f64,
    frames: u32,
    voice: u32,
    bits: u64,
    fixed: u64,
    call_sign: Option<String>,
}

/// One channel: the frame layer on its symbols, the messages and the call.
#[derive(Clone, Debug)]
pub struct Link {
    /// The key of the channel's calls in the call log.
    pub key: u32,
    /// The plan label that the rows of the channel start with.
    pub label: Option<String>,
    buf: Vec<Sym>,
    active: bool,
    after: usize,
    misses: u32,
    empty: u32,
    in_step: bool,
    last_rich: Option<Rich>,
    units: Units,
    last_message: Option<(Message, f64)>,
    call: Option<OpenCall>,
    pending_sign: Option<(String, f64)>,
    signs: Vec<(String, f64)>,
    shown: Vec<(String, f64)>,
    idle_shown_s: f64,
    /// Signal time of the last frame that passed a check, s.
    pub last_good_s: f64,
    /// Frames that passed a check.
    pub good_frames: u64,
    soft: Vec<f32>,
}

impl Link {
    /// The channel whose calls have the key `key` and whose rows start with `label`.
    pub fn new(key: u32, label: Option<String>) -> Link {
        Link {
            key,
            label,
            buf: Vec::with_capacity(FRAME + 2),
            active: false,
            after: 0,
            misses: 0,
            empty: 0,
            in_step: false,
            last_rich: None,
            units: Units::default(),
            last_message: None,
            call: None,
            pending_sign: None,
            signs: Vec::new(),
            shown: Vec::new(),
            idle_shown_s: f64::NEG_INFINITY,
            last_good_s: f64::NEG_INFINITY,
            good_frames: 0,
            soft: vec![0.0; 2 * (FRAME - SYNC)],
        }
    }

    /// A call is open on the channel.
    pub fn in_call(&self) -> bool {
        self.call.is_some()
    }

    /// A frame sync was found: `pre` holds its symbols. `tracking`: the symbol layer follows a frame. The sync word can
    /// turn up inside a frame: while the last frame decoded, a sync is taken only within 2 symbols of its nominal place;
    /// otherwise one inside a frame is not taken.
    pub fn sync(&mut self, pre: &[Sym], tracking: bool) -> SyncAnswer {
        let len = self.buf.len();
        let take = if !tracking {
            true
        } else if self.in_step {
            if self.active {
                len <= SYNC + 2 || len + 1 >= FRAME
            } else {
                self.after + 2 >= SYNC
            }
        } else {
            !(self.active && len > SYNC + 2 && len < FRAME - 6)
        };
        if !take {
            return SyncAnswer::Ignore;
        }
        if !tracking {
            self.in_step = false;
        }
        self.buf.clear();
        self.buf.extend_from_slice(pre);
        self.active = true;
        self.after = 0;
        self.misses = 0;
        SyncAnswer::Take
    }

    /// One symbol after a sync. A frame is decoded when its 182 symbols are in. A sync that does not come is taken at its
    /// nominal place twice; the third miss, three frames in a row that decode nothing, or a lost carrier end the channel.
    pub fn symbol(&mut self, s: Sym, sh: &mut Shared, ctx: &mut Ctx) -> SymAnswer {
        self.buf.push(s);
        if !self.active {
            self.after += 1;
            if self.after <= SYNC {
                return SymAnswer::Continue;
            }
            if self.misses >= FLYWHEEL {
                self.lost(sh, ctx);
                return SymAnswer::Stop;
            }
            self.misses += 1;
            self.active = true;
            return SymAnswer::Continue;
        }
        if self.buf.len() < FRAME {
            return SymAnswer::Continue;
        }
        self.active = false;
        self.after = 0;
        if carrier_gone(&self.buf[SYNC..FRAME]) {
            self.lost(sh, ctx);
            return SymAnswer::Stop;
        }
        let good = self.frame(sh, ctx);
        self.buf.clear();
        self.in_step = good;
        if good {
            self.empty = 0;
        } else {
            self.empty += 1;
            if self.empty >= EMPTY_FRAMES {
                self.lost(sh, ctx);
                return SymAnswer::Stop;
            }
        }
        SymAnswer::Continue
    }

    /// The channel is lost: the call ends as signal lost and the frame layer starts again.
    fn lost(&mut self, sh: &mut Shared, ctx: &mut Ctx) {
        self.end_call(false, sh, ctx);
        self.reset();
    }

    /// Forgets the frame in progress, the RICH and the units (a retune or a lost channel). The call, if any, is dropped
    /// without a row.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.active = false;
        self.after = 0;
        self.misses = 0;
        self.empty = 0;
        self.in_step = false;
        self.last_rich = None;
        self.units.clear();
        self.last_message = None;
        self.call = None;
    }

    /// About every 0.25 s: a call without a good frame for 1.5 s ends as signal lost.
    pub fn tick(&mut self, sh: &mut Shared, ctx: &mut Ctx) {
        if self.call.is_some() && (ctx.now - self.last_good_s > CALL_LOST_S || !ctx.has_call(self.key)) {
            self.end_call(false, sh, ctx);
        }
    }

    /// The identity of a row: the plan label of the channel, then `base`.
    fn identity(&self, base: &str) -> String {
        match &self.label {
            Some(l) => format!("{l} {base}"),
            None => base.to_string(),
        }
    }

    fn row(&self, sh: &mut Shared, now: f64, kind: DcrRowKind, base: &str, text: String) {
        let identity = self.identity(base);
        sh.row(now, kind, identity, text, self.label.clone());
    }

    /// Decodes the frame in `buf`. Returns true when the SACCH, the PICH or a data block passed its CRC.
    fn frame(&mut self, sh: &mut Shared, ctx: &mut Ctx) -> bool {
        let now = ctx.now;
        sh.counts.frames += 1;
        let mut syms = [Sym::default(); FRAME - SYNC];
        for (k, s) in self.buf[SYNC..FRAME].iter().enumerate() {
            let mut s = *s;
            if sh.pn[k] != 0 {
                s.v = -s.v;
                s.d = -s.d;
            }
            syms[k] = s;
            let (a, b) = s.soft_dibit();
            self.soft[2 * k] = a;
            self.soft[2 * k + 1] = b;
        }
        // The RICH: one sign bit for each of its 8 symbols. A failed parity is repaired by turning the least reliable bit
        // when its symbol is under 1 level; the last good RICH of the channel stands in otherwise.
        let raw = syms[..8].iter().fold(0u8, |a, s| (a << 1) | u8::from(s.v < 0.0));
        let repaired = Rich::decode(raw).or_else(|| {
            let (i, m) = syms[..8]
                .iter()
                .enumerate()
                .map(|(i, s)| (i, s.v.abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1))?;
            if m < 1.0 {
                Rich::decode(raw ^ (0x80 >> i))
            } else {
                None
            }
        });
        ctx.identity(repaired.is_some());
        let rich = match repaired {
            Some(r) => {
                self.last_rich = Some(r);
                r
            }
            None => {
                sh.counts.rich_bad += 1;
                match self.last_rich {
                    Some(r) => r,
                    None => return false,
                }
            }
        };
        let (mut bits, mut fixed) = (0u64, 0u64);
        let mut good = false;
        let (info, sacch_ok, errs) = sh.sacch.decode(&self.soft[16..76]);
        ctx.block(sacch_ok, 60, errs);
        let mut message = None;
        if sacch_ok {
            sh.counts.sacch_ok += 1;
            sh.counts.checked(60, errs);
            bits += 60;
            fixed += errs as u64;
            good = true;
            self.last_good_s = now;
            message = self.units.push(&info);
        } else {
            sh.counts.sacch_bad += 1;
        }
        if let Some(m) = message {
            self.message(m, sh, ctx);
        }
        let mut voice = 0u32;
        if !rich.service {
            let (p, ok, errs) = sh.facch1.decode(&self.soft[76..220]);
            let sign = if ok { call_sign(&p) } else { None };
            ctx.block(sign.is_some(), 144, errs);
            if let Some(sign) = sign {
                sh.counts.pich_ok += 1;
                sh.counts.checked(144, errs);
                bits += 144;
                fixed += errs as u64;
                good = true;
                self.last_good_s = now;
                self.call_sign(sign, sh, ctx);
            }
        } else {
            for half in 0..2 {
                let at = 76 + half * 144;
                match half_of(rich.layout, half) {
                    Half::Voice => {
                        if sacch_ok {
                            voice += 2;
                        }
                    }
                    Half::Data => {
                        let (d, ok, errs) = sh.facch1.decode(&self.soft[at..at + 144]);
                        ctx.block(ok, 144, errs);
                        if ok {
                            sh.counts.data_ok += 1;
                            sh.counts.checked(144, errs);
                            bits += 144;
                            fixed += errs as u64;
                            good = true;
                            self.last_good_s = now;
                            self.data(half, &d, sh, now);
                        } else {
                            sh.counts.data_bad += 1;
                        }
                    }
                    Half::Nothing => {}
                }
            }
        }
        sh.counts.voice_frames += u64::from(voice);
        if good {
            self.good_frames += 1;
        }
        if let Some(c) = self.call.as_mut() {
            c.frames += 1;
            c.voice += voice;
            c.bits += bits;
            c.fixed += fixed;
            if good {
                ctx.update_call(self.key, |r| {
                    r.voice_frames += voice;
                    r.bits += bits;
                    r.bit_errors += fixed;
                });
            }
        }
        good
    }

    /// A complete SACCH message. A message counts when the same one came in the frame before, less than 1 s earlier; a
    /// voice message of the call in progress and the end of that call count at once.
    fn message(&mut self, m: Message, sh: &mut Shared, ctx: &mut Ctx) {
        let now = ctx.now;
        let confirmed = self
            .last_message
            .as_ref()
            .is_some_and(|(l, t)| *l == m && now - t < CONFIRM_S);
        self.last_message = Some((m.clone(), now));
        let uc = format!("UC {}", m.user_code);
        match m.kind {
            TYPE_VOICE => {
                if self.call.as_ref().is_some_and(|c| c.message == m) {
                    sh.last_user_code = Some(m.user_code);
                } else if confirmed {
                    sh.last_user_code = Some(m.user_code);
                    self.end_call(false, sh, ctx);
                    self.open_call(m, sh, ctx);
                }
            }
            TYPE_END => {
                let same = self.call.as_ref().is_some_and(|c| {
                    c.message.user_code == m.user_code && c.message.manufacturer == m.manufacturer
                });
                if same {
                    self.end_call(true, sh, ctx);
                }
            }
            _ if !confirmed => {}
            TYPE_IDLE => {
                sh.last_user_code = Some(m.user_code);
                if self.call.is_none() && now - self.idle_shown_s >= IDLE_REPEAT_S {
                    self.idle_shown_s = now;
                    self.row(sh, now, DcrRowKind::Idle, &uc, format!("idle, user code {}", m.user_code));
                }
            }
            2..=19 => {
                sh.last_user_code = Some(m.user_code);
                let mut text = format!(
                    "format {}, {}, {}, {} extra bits",
                    m.kind - 1,
                    manufacturer_text(m.manufacturer),
                    user_code_text(m.user_code),
                    m.extra.len()
                );
                if !m.extra.is_empty() {
                    text.push_str(", ");
                    text.push_str(&m.extra_hex());
                }
                if self.fresh(&text, now) {
                    self.row(sh, now, DcrRowKind::ManufacturerMessage, &uc, text);
                }
            }
            _ => {
                sh.last_user_code = Some(m.user_code);
                let text = format!("reserved message type {}, user code {}", m.kind, m.user_code);
                if self.fresh(&text, now) {
                    self.row(sh, now, DcrRowKind::UnknownMessage, &uc, text);
                }
            }
        }
    }

    /// Returns true when `text` was not shown in the last 5 s, and notes it as shown now.
    fn fresh(&mut self, text: &str, now: f64) -> bool {
        self.shown.retain(|(_, t)| now - t < TEXT_REPEAT_S);
        if self.shown.iter().any(|(s, _)| s == text) {
            return false;
        }
        self.shown.push((text.to_string(), now));
        true
    }

    /// A good data block of half `half`.
    fn data(&mut self, half: usize, bits: &[u8], sh: &mut Shared, now: f64) {
        let hex = hex_of_bits(&bits[..80]);
        let name = if half == 0 { "TCH1" } else { "TCH2" };
        if self.fresh(&format!("{name} {hex}"), now) {
            self.row(sh, now, DcrRowKind::Data, name, hex);
        }
    }

    /// A call sign from a PICH: shown once in 10 s, attached to the call in progress or to the next call.
    fn call_sign(&mut self, sign: String, sh: &mut Shared, ctx: &mut Ctx) {
        let now = ctx.now;
        sh.last_call_sign = Some(sign.clone());
        self.signs.retain(|(_, t)| now - t < SIGN_REPEAT_S);
        if !self.signs.iter().any(|(s, _)| *s == sign) {
            self.signs.push((sign.clone(), now));
            self.row(sh, now, DcrRowKind::CallSign, &sign, format!("call sign {sign}"));
        }
        match self.call.as_mut() {
            Some(c) => {
                if c.call_sign.is_none() {
                    c.call_sign = Some(sign.clone());
                    ctx.update_call(self.key, |r| r.src = Ident::from(sign));
                }
            }
            None => self.pending_sign = Some((sign, now)),
        }
    }

    /// Opens the call of the confirmed voice message `m`.
    fn open_call(&mut self, m: Message, sh: &mut Shared, ctx: &mut Ctx) {
        let now = ctx.now;
        let sign = self
            .pending_sign
            .take()
            .filter(|(_, t)| now - t < SIGN_WAITS_S)
            .map(|(s, _)| s);
        let mut text = format!(
            "{}, {}, {}",
            user_code_text(m.user_code),
            manufacturer_text(m.manufacturer),
            call_type_name(m.call_type)
        );
        if let Some(s) = &sign {
            text.push_str(", call sign ");
            text.push_str(s);
        }
        let mut rec = CallRecord {
            kind: CallKind::Voice,
            src: sign.clone().map(Ident::from).unwrap_or_default(),
            dst: Ident::from(format!("UC {}", m.user_code)),
            group: true,
            encryption: if m.call_type == 1 {
                Encryption::encrypted(None, "privacy scrambler", None)
            } else {
                Encryption::clear()
            },
            text: manufacturer_text(m.manufacturer),
            ..Default::default()
        };
        rec.set(
            CallDetail::CallType,
            Value::Text(call_type_name(m.call_type).into()),
        );
        if let Some(l) = &self.label {
            rec.set(CallDetail::Channel, Value::Text(l.clone()));
        }
        ctx.start_call(self.key, rec, false);
        sh.counts.calls += 1;
        let uc = format!("UC {}", m.user_code);
        self.row(sh, now, DcrRowKind::Call, &uc, text);
        self.call = Some(OpenCall {
            message: m,
            start_s: now,
            frames: 0,
            voice: 0,
            bits: 0,
            fixed: 0,
            call_sign: sign,
        });
    }

    /// Ends the open call: by its end-of-communication message (`end_seen`) or because the signal went.
    fn end_call(&mut self, end_seen: bool, sh: &mut Shared, ctx: &mut Ctx) {
        let Some(c) = self.call.take() else {
            return;
        };
        let now = ctx.now;
        ctx.end_call(self.key, end_seen);
        let end = if end_seen { now } else { self.last_good_s.max(c.start_s) };
        let ber = if c.bits > 0 {
            100.0 * c.fixed as f64 / c.bits as f64
        } else {
            0.0
        };
        let reason = if end_seen {
            "end-of-communication message"
        } else {
            "signal lost"
        };
        let text = format!(
            "{:.1} s, {} frames, {} voice frames, {:.1} % bit errors, {reason}",
            end - c.start_s,
            c.frames,
            c.voice,
            ber
        );
        let uc = format!("UC {}", c.message.user_code);
        self.row(sh, now, DcrRowKind::CallEnd, &uc, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rich_parity_covers_all_eight_bits() {
        for (v, service, layout) in [(0x11u8, false, 4u8), (0x8D, true, 3), (0x95, true, 5)] {
            let r = Rich::decode(v).expect("even parity");
            assert_eq!((r.service, r.layout, r.relay), (service, layout, false));
            assert_eq!(r.byte(), v);
        }
        assert_eq!(Rich::decode(0x10), None);
    }

    #[test]
    fn units_of_four_make_one_message_and_a_wrong_unit_drops_it() {
        let mut m = Message::new(9, 0, 7, 5);
        m.extra = (0..54).map(|i| (i % 3 == 0) as u8).collect();
        let units = m.units();
        assert_eq!(units.len(), 4);
        let mut u = Units::default();
        assert_eq!(u.push(&units[0]), None);
        assert_eq!(u.push(&units[1]), None);
        assert_eq!(u.push(&units[2]), None);
        assert_eq!(u.push(&units[3]), Some(m.clone()));
        assert_eq!(u.push(&units[0]), None);
        assert_eq!(u.push(&units[2]), None);
        assert_eq!(u.push(&units[3]), None);
        assert_eq!(u.push(&units[1]), None);
    }

    #[test]
    fn a_call_sign_with_a_digit_above_nine_is_refused() {
        assert_eq!(call_sign(&pich_bits("100000001")).as_deref(), Some("100000001"));
        let mut b = pich_bits("100000001");
        b[0..4].copy_from_slice(&[1, 0, 1, 0]);
        assert_eq!(call_sign(&b), None);
    }
}

