//! The message layer of the Type II control channel: a queue of words, read as messages of one to three words. A lost word
//! leaves a marker; no message is assembled across it, and a word right after it may be the tail of a lost message. No public
//! document confirms any command code or field.

use super::code::Osw;
use super::plan::Channels;
use std::collections::VecDeque;

/// The words queued before a message is read: three of a message and two idles inside it.
pub const READ_AT: usize = 5;

/// The idle command.
pub const IDLE: u16 = 0x2F8;

/// A talkgroup address with its option bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Talkgroup {
    /// The talkgroup: the address with the option bits cleared.
    pub id: u16,
    /// The option bits.
    pub options: u8,
}

impl Talkgroup {
    /// The talkgroup and options of a group address.
    pub fn of(address: u16) -> Talkgroup {
        Talkgroup {
            id: address & 0xFFF0,
            options: (address & 0xF) as u8,
        }
    }
    /// Encrypted (option bit 3).
    pub fn encrypted(&self) -> bool {
        self.options & 8 != 0
    }
    fn o(&self) -> u8 {
        self.options & 7
    }
    /// An emergency call.
    pub fn emergency(&self) -> bool {
        matches!(self.o(), 2 | 4 | 5)
    }
    /// A patched talkgroup.
    pub fn patch(&self) -> bool {
        matches!(self.o(), 3 | 4)
    }
    /// A multiselect talkgroup.
    pub fn multiselect(&self) -> bool {
        matches!(self.o(), 5 | 7)
    }
}

/// A voice channel grant or update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrantMsg {
    /// The talkgroup (group calls) or the called unit.
    pub target: u16,
    /// The option bits of a talkgroup.
    pub options: u8,
    /// The calling unit (0 when the message has none).
    pub source: u16,
    /// The channel number.
    pub channel: u16,
    /// A group call.
    pub group: bool,
    /// A digital call; `None` for a call update, which takes it from the talkgroup's last grant.
    pub digital: Option<bool>,
    /// Encrypted.
    pub encrypted: bool,
    /// An emergency call.
    pub emergency: bool,
    /// A telephone interconnect call.
    pub interconnect: bool,
    /// A call update (not a new grant).
    pub update: bool,
}

/// A site broadcast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiteMsg {
    /// The system.
    pub system: u16,
    /// The site, from 1.
    pub site: u8,
    /// The band code.
    pub band: u8,
    /// The feature bits.
    pub features: u8,
    /// The control channel number.
    pub channel: u16,
    /// An adjacent site (false: this site).
    pub adjacent: bool,
}

/// What a message means.
#[derive(Clone, Debug, PartialEq)]
pub enum Meaning {
    /// A grant or call update.
    Grant(GrantMsg),
    /// A control channel: the channel and the system or data.
    ControlChannel {
        /// The channel number.
        channel: u16,
        /// The system, or the data of a one-word form.
        system: u16,
        /// Adjacent (true), alternate (false) or this site's (`None`).
        adjacent: Option<bool>,
    },
    /// A site broadcast.
    Site(SiteMsg),
    /// The system ID beacon.
    SystemId(u16),
    /// System status: opcode and data; opcode 1 says Type II or Type I.
    Status {
        /// The opcode.
        opcode: u8,
        /// The data.
        data: u16,
    },
    /// A talkgroup patched (or multiselected) with a member.
    Patch {
        /// The talkgroup.
        talkgroup: u16,
        /// The member talkgroup.
        member: u16,
        /// Multiselect.
        multiselect: bool,
    },
    /// A patch cancelled.
    PatchCancel(u16),
    /// An idle word (not logged).
    Idle,
    /// Any other message, with its text.
    Other,
    /// A word not understood.
    Unknown,
}

/// One message: what it means, its text, and the words it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// The meaning.
    pub meaning: Meaning,
    /// The text.
    pub text: String,
    /// The first word.
    pub first: Osw,
    /// The words it took.
    pub words: u8,
}

/// One queued entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// A word, and whether a lost word came right before it.
    Word(Osw, bool),
    /// A lost word.
    Lost,
}

/// The message queue.
#[derive(Clone, Debug, Default)]
pub struct MessageLayer {
    queue: VecDeque<Entry>,
    lost_before: bool,
}

/// The commands after which an idle inside a 308 message is read and the first word kept.
const INSIDE_308: [u16; 13] = [
    0x30A, 0x30B, 0x30D, 0x310, 0x311, 0x317, 0x318, 0x319, 0x31A, 0x320, 0x322, 0x32E, 0x340,
];

fn tg_text(t: Talkgroup) -> String {
    format!("{:04X}", t.id)
}

impl MessageLayer {
    /// Queues a good word.
    pub fn word(&mut self, w: Osw) {
        self.queue.push_back(Entry::Word(w, self.lost_before));
        self.lost_before = false;
    }

    /// Queues the marker of a lost word.
    pub fn lost(&mut self) {
        if self.queue.back() != Some(&Entry::Lost) {
            self.queue.push_back(Entry::Lost);
        }
        self.lost_before = true;
    }

    /// Forgets the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.lost_before = false;
    }

    /// Reads every message that the queue holds enough words for (all of them with `flush`), in order.
    pub fn read(&mut self, ch: Channels<'_>, flush: bool, mut out: impl FnMut(Message)) {
        while self.queue.len() >= READ_AT || (flush && !self.queue.is_empty()) {
            if let Some(m) = self.step(ch) {
                out(m);
            }
        }
    }

    fn at(&self, i: usize) -> Option<Osw> {
        match self.queue.get(i) {
            Some(Entry::Word(w, _)) => Some(*w),
            _ => None,
        }
    }

    /// Takes `n` entries from the front.
    fn take(&mut self, n: usize) {
        for _ in 0..n {
            self.queue.pop_front();
        }
    }

    fn msg(&mut self, n: usize, first: Osw, meaning: Meaning, text: String) -> Option<Message> {
        self.take(n);
        Some(Message {
            meaning,
            text,
            first,
            words: n as u8,
        })
    }

    /// Reads one message at the front, or drops one entry.
    fn step(&mut self, ch: Channels<'_>) -> Option<Message> {
        let (a, after_lost) = match self.queue.front().copied() {
            Some(Entry::Word(w, l)) => (w, l),
            Some(Entry::Lost) | None => {
                self.take(1);
                return None;
            }
        };
        let b = self.at(1);
        let c = self.at(2);
        if ch.is_tx(a.command) {
            return self.obt(a, b, c, ch);
        }
        match a.command {
            0x308 => return self.form_308(a, b, c, ch),
            0x321 => return self.form_321(a, b, c, ch),
            _ => {}
        }
        let tg = Talkgroup::of(a.address);
        if ch.is_channel(a.command) {
            if a.group {
                if after_lost {
                    return self.msg(
                        1,
                        a,
                        Meaning::Unknown,
                        format!(
                            "channel {:03X} for {:04X}: the tail of a lost message",
                            a.command, a.address
                        ),
                    );
                }
                return self.msg(
                    1,
                    a,
                    Meaning::Grant(GrantMsg {
                        target: tg.id,
                        options: tg.options,
                        source: 0,
                        channel: a.command,
                        group: true,
                        digital: None,
                        encrypted: tg.encrypted(),
                        emergency: tg.emergency(),
                        interconnect: false,
                        update: true,
                    }),
                    format!(
                        "call update: talkgroup {} on channel {:03X}",
                        tg_text(tg),
                        a.command
                    ),
                );
            }
            if a.address & 0xFF00 == 0x1F00 {
                return self.msg(
                    1,
                    a,
                    Meaning::ControlChannel {
                        channel: a.command,
                        system: a.address & 0xFF,
                        adjacent: None,
                    },
                    format!("control channel {:03X}", a.command),
                );
            }
        }
        let (m, text) = match (a.command, a.group) {
            (IDLE, false) => (Meaning::Idle, "idle".to_string()),
            (0x300, true) => (
                Meaning::Other,
                format!("busy, queued: talkgroup {}", tg_text(tg)),
            ),
            (0x303, true) => (
                Meaning::Other,
                format!("emergency busy, queued: talkgroup {}", tg_text(tg)),
            ),
            (0x324, false) => (
                Meaning::Other,
                format!("interconnect rejected: unit {}", a.address),
            ),
            (0x32A, true) => (
                Meaning::Other,
                format!("affiliation request: talkgroup {}", tg_text(tg)),
            ),
            (0x32B, false) => (
                Meaning::SystemId(a.address),
                format!("system ID {:04X}", a.address),
            ),
            (0x32C, false) => (Meaning::Other, format!("roaming: unit {}", a.address)),
            (0x360..=0x39F, _) => (
                Meaning::Other,
                format!("AMSS: site {}", a.command - 0x360 + 1),
            ),
            (0x3A0, true) => (
                Meaning::Other,
                format!(
                    "diagnostic: opcode {}, data {:03X}",
                    a.address >> 12,
                    a.address & 0xFFF
                ),
            ),
            (0x3BF | 0x3C0, _) => {
                let opcode = (a.address >> 13) as u8;
                let data = a.address & 0x1FFF;
                let text = if opcode == 1 {
                    format!(
                        "system status: {}",
                        if data & 0x1000 != 0 {
                            "Type II"
                        } else {
                            "Type I"
                        }
                    )
                } else {
                    format!("system status: opcode {opcode}, data {data:04X}")
                };
                (Meaning::Status { opcode, data }, text)
            }
            _ => (
                Meaning::Unknown,
                format!(
                    "unknown: command {:03X}, address {:04X}",
                    a.command, a.address
                ),
            ),
        };
        self.msg(1, a, m, text)
    }

    /// A 308 message.
    fn form_308(
        &mut self,
        a: Osw,
        b: Option<Osw>,
        c: Option<Osw>,
        ch: Channels<'_>,
    ) -> Option<Message> {
        let Some(b) = b else {
            return self.unknown(a);
        };
        let both_ind = !a.group && !b.group;
        let both_grp = a.group && b.group;
        let ao = a.address & 7;
        if ch.is_channel(b.command) {
            let tg = Talkgroup::of(b.address);
            if !b.group && b.address & 0xFF00 == 0x1F00 {
                return self.msg(
                    2,
                    a,
                    Meaning::ControlChannel {
                        channel: b.command,
                        system: a.address,
                        adjacent: None,
                    },
                    format!(
                        "control channel {:03X}, system {:04X}",
                        b.command, a.address
                    ),
                );
            }
            if b.group && a.address != 0 && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: tg.id,
                        options: tg.options,
                        source: a.address,
                        channel: b.command,
                        group: true,
                        digital: Some(false),
                        encrypted: tg.encrypted(),
                        emergency: tg.emergency(),
                        interconnect: false,
                        update: false,
                    },
                );
            }
            if !b.group && a.address != 0 && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: a.address,
                        options: 0,
                        source: b.address,
                        channel: b.command,
                        group: false,
                        digital: Some(false),
                        encrypted: false,
                        emergency: false,
                        interconnect: false,
                        update: false,
                    },
                );
            }
            if !b.group && a.address == 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: 0,
                        options: 0,
                        source: b.address,
                        channel: b.command,
                        group: false,
                        digital: Some(false),
                        encrypted: false,
                        emergency: false,
                        interconnect: true,
                        update: false,
                    },
                );
            }
        }
        let tgb = Talkgroup::of(b.address);
        let tga = Talkgroup::of(a.address);
        match b.command {
            IDLE => {
                if c.is_some_and(|c| INSIDE_308.contains(&c.command)) {
                    // An idle inside the message: read it, and take A again with the next word.
                    self.queue.remove(1);
                    return None;
                }
                self.msg(2, a, Meaning::Idle, "idle".into())
            }
            0x300 if b.group => self.other(
                2,
                a,
                format!("busy: unit {} for talkgroup {}", a.address, tg_text(tgb)),
            ),
            0x302 if !b.group => self.other(
                2,
                a,
                format!("private call busy: {} to {}", a.address, b.address),
            ),
            0x303 if b.group => {
                self.other(2, a, format!("emergency busy: talkgroup {}", tg_text(tgb)))
            }
            0x308 => {
                if c.is_some_and(|c| c.command == IDLE) {
                    self.msg(1, a, Meaning::Idle, "idle".into())
                } else {
                    self.unknown(a)
                }
            }
            0x30A if both_ind => self.other(
                2,
                a,
                format!(
                    "dynamic regroup: unit {} to the talkgroup of {:04X}",
                    a.address, b.address
                ),
            ),
            0x30B => {
                if c.is_some_and(|c| c.command == IDLE && !c.group) {
                    self.queue.remove(2);
                    return None;
                }
                if let Some(c) = c.filter(|c| {
                    b.group
                        && !c.group
                        && ch.is_channel(c.command)
                        && c.address & 0xFF00 == 0x1F00
                        && b.address & 0xFC00 == 0x2800
                        && b.address & 0x3FF == c.command
                }) {
                    return self.msg(
                        3,
                        a,
                        Meaning::ControlChannel {
                            channel: c.command,
                            system: a.address,
                            adjacent: None,
                        },
                        format!(
                            "control channel {:03X}, system {:04X}",
                            c.command, a.address
                        ),
                    );
                }
                self.extended(a, b)
            }
            0x30D if both_ind => {
                let op = b.address & 0xF;
                let text = match op {
                    0..=7 => format!("status {op} from unit {}", a.address),
                    8 => format!("emergency acknowledged: unit {}", a.address),
                    _ => format!("dynamic regroup: unit {}", a.address),
                };
                self.other(2, a, text)
            }
            0x310 if both_ind => self.other(
                2,
                a,
                format!(
                    "affiliation: unit {} to the talkgroup of {:04X}",
                    a.address, b.address
                ),
            ),
            0x311 if both_ind => self.other(
                2,
                a,
                format!("message {} from unit {}", b.address & 0xF, a.address),
            ),
            0x315 | 0x317 if both_ind => self.other(
                2,
                a,
                format!(
                    "private call ring: {} called by {}{}",
                    a.address,
                    b.address,
                    if b.command == 0x315 {
                        ", encrypted"
                    } else {
                        ""
                    }
                ),
            ),
            0x318 if both_ind => self.other(
                2,
                a,
                format!("ring acknowledged: {} and {}", a.address, b.address),
            ),
            0x319 if both_ind => self.other(
                2,
                a,
                format!("call alert: {} from {}", a.address, b.address),
            ),
            0x31A if both_ind => self.other(
                2,
                a,
                format!("call alert acknowledged: {} and {}", a.address, b.address),
            ),
            0x31B if both_ind => self.other(
                2,
                a,
                format!("roaming: unit {}, system {:04X}", a.address, b.address),
            ),
            0x320 => {
                if c.is_some_and(|c| c.command == IDLE && !c.group) {
                    self.queue.remove(2);
                    return None;
                }
                match c.filter(|c| c.command == 0x30B && c.address & 0xFC00 == 0x6000) {
                    Some(c) => self.site(a, b, c),
                    None => self.unknown(a),
                }
            }
            0x322 if both_grp => {
                let text = format!(
                    "date and time: {:04}-{:02}-{:02} {:02}:{:02}, day of week {}",
                    (a.address >> 9) + 2000,
                    (a.address >> 5) & 0xF,
                    a.address & 0x1F,
                    (b.address >> 8) & 0x1F,
                    b.address & 0xFF,
                    b.address >> 13
                );
                self.other(2, a, text)
            }
            0x32E if both_grp => self.other(
                2,
                a,
                format!(
                    "emergency PTT: unit {} on the talkgroup of {:04X}",
                    a.address, b.address
                ),
            ),
            0x340 if both_grp && matches!(ao, 3 | 4 | 5 | 7) => {
                let member = (b.address & 0xFFF) << 4;
                let multiselect = matches!(ao, 5 | 7);
                self.msg(
                    2,
                    a,
                    Meaning::Patch {
                        talkgroup: tga.id,
                        member,
                        multiselect,
                    },
                    format!(
                        "{}: talkgroup {} includes talkgroup {:04X}",
                        if multiselect { "multiselect" } else { "patch" },
                        tg_text(tga),
                        member
                    ),
                )
            }
            _ => self.unknown(a),
        }
    }

    /// The extended function of 308 then 30B.
    fn extended(&mut self, a: Osw, b: Osw) -> Option<Message> {
        let tga = Talkgroup::of(a.address);
        let v = b.address;
        let ao = a.address & 7;
        if b.group && v & 0xFC00 == 0x2800 {
            return self.msg(
                2,
                a,
                Meaning::ControlChannel {
                    channel: v & 0x3FF,
                    system: a.address,
                    adjacent: None,
                },
                format!(
                    "control channel {:03X}, system {:04X}",
                    v & 0x3FF,
                    a.address
                ),
            );
        }
        if v & 0xFC00 == 0x6000 {
            let adj = b.group;
            return self.msg(
                2,
                a,
                Meaning::ControlChannel {
                    channel: v & 0x3FF,
                    system: a.address,
                    adjacent: Some(adj),
                },
                format!(
                    "{} control channel {:03X}, system {:04X}",
                    if adj { "adjacent" } else { "alternate" },
                    v & 0x3FF,
                    a.address
                ),
            );
        }
        if b.group {
            if v == 0x2021 && matches!(ao, 3 | 4 | 5 | 7) {
                return self.msg(
                    2,
                    a,
                    Meaning::PatchCancel(tga.id),
                    format!("patch cancelled: talkgroup {}", tg_text(tga)),
                );
            }
            return self.other(
                2,
                a,
                format!("extended function {v:04X} for talkgroup {}", tg_text(tga)),
            );
        }
        let text = match v {
            0x261C => format!("deaffiliation: unit {}", a.address),
            0x26E0..=0x26E7 => format!("status {} from unit {}", v & 7, a.address),
            0x26E8 => format!("emergency acknowledged: unit {}", a.address),
            0x26F0..=0x26FF => format!("message {} from unit {}", v & 0xF, a.address),
            _ if v & 0xFC00 == 0x2C00 => format!("denied: unit {}, reason {v:04X}", a.address),
            _ => format!("extended function {v:04X} for unit {}", a.address),
        };
        self.other(2, a, text)
    }

    /// The site broadcast of A, B and C.
    fn site(&mut self, a: Osw, b: Osw, c: Osw) -> Option<Message> {
        let s = SiteMsg {
            system: a.address,
            site: ((b.address >> 10) + 1) as u8,
            band: ((b.address >> 7) & 7) as u8,
            features: (b.address & 0x3F) as u8,
            channel: c.address & 0x3FF,
            adjacent: c.group,
        };
        let text = format!(
            "{}: system {:04X}, site {}, band {}, features {:02X}, control channel {:03X}",
            if s.adjacent {
                "adjacent site"
            } else {
                "this site"
            },
            s.system,
            s.site,
            s.band,
            s.features,
            s.channel
        );
        self.msg(3, a, Meaning::Site(s), text)
    }

    /// A 321 (digital) message.
    fn form_321(
        &mut self,
        a: Osw,
        b: Option<Osw>,
        c: Option<Osw>,
        ch: Channels<'_>,
    ) -> Option<Message> {
        let Some(b) = b else {
            return self.unknown(a);
        };
        if ch.is_channel(b.command) {
            let tg = Talkgroup::of(b.address);
            if a.group && b.group && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: tg.id,
                        options: tg.options,
                        source: a.address,
                        channel: b.command,
                        group: true,
                        digital: Some(true),
                        encrypted: tg.encrypted(),
                        emergency: tg.emergency(),
                        interconnect: false,
                        update: false,
                    },
                );
            }
            if !b.group && a.address != 0 && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: a.address,
                        options: 0,
                        source: b.address,
                        channel: b.command,
                        group: false,
                        digital: Some(true),
                        encrypted: a.group,
                        emergency: false,
                        interconnect: false,
                        update: false,
                    },
                );
            }
        }
        match b.command {
            IDLE => {
                if c.is_some_and(|c| matches!(c.command, 0x317 | 0x318)) {
                    self.queue.remove(1);
                    return None;
                }
                self.msg(2, a, Meaning::Idle, "idle".into())
            }
            0x315 | 0x317 if !a.group && !b.group => self.other(
                2,
                a,
                format!(
                    "digital private call ring: {} called by {}{}",
                    a.address,
                    b.address,
                    if b.command == 0x315 {
                        ", encrypted"
                    } else {
                        ""
                    }
                ),
            ),
            _ => self.unknown(a),
        }
    }

    /// An OBT form: A's command is a transmit channel.
    fn obt(&mut self, a: Osw, b: Option<Osw>, c: Option<Osw>, ch: Channels<'_>) -> Option<Message> {
        let Some(b) = b else {
            return self.unknown(a);
        };
        if b.command == 0x320 && a.group && b.group {
            if let Some(c) = c.filter(|c| c.command == 0x30B && c.address & 0xFC00 == 0x6000) {
                return self.site(a, b, c);
            }
        }
        if b.command == IDLE {
            return self.msg(2, a, Meaning::Idle, "idle".into());
        }
        if ch.is_channel(b.command) && !ch.is_tx(b.command) {
            let tg = Talkgroup::of(b.address);
            if b.group && a.address != 0 && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: tg.id,
                        options: tg.options,
                        source: a.address,
                        channel: b.command,
                        group: true,
                        digital: Some(!a.group),
                        encrypted: tg.encrypted(),
                        emergency: tg.emergency(),
                        interconnect: false,
                        update: false,
                    },
                );
            }
            if !b.group && a.address != 0 && b.address != 0 {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: a.address,
                        options: 0,
                        source: b.address,
                        channel: b.command,
                        group: false,
                        digital: Some(false),
                        encrypted: a.group,
                        emergency: false,
                        interconnect: false,
                        update: false,
                    },
                );
            }
            if !b.group && a.address == 0 && !a.group {
                return self.grant(
                    2,
                    a,
                    GrantMsg {
                        target: 0,
                        options: 0,
                        source: b.address,
                        channel: b.command,
                        group: false,
                        digital: Some(false),
                        encrypted: false,
                        emergency: false,
                        interconnect: true,
                        update: false,
                    },
                );
            }
        }
        self.unknown(a)
    }

    fn grant(&mut self, n: usize, a: Osw, g: GrantMsg) -> Option<Message> {
        let kind = if g.interconnect {
            "interconnect grant".to_string()
        } else if g.group {
            format!(
                "{} group grant",
                if g.digital == Some(true) {
                    "digital"
                } else {
                    "analog"
                }
            )
        } else {
            format!(
                "{} private call grant",
                if g.digital == Some(true) {
                    "digital"
                } else {
                    "analog"
                }
            )
        };
        let mut text = if g.group {
            format!(
                "{kind}: talkgroup {:04X} from {} on channel {:03X}",
                g.target, g.source, g.channel
            )
        } else if g.interconnect {
            format!("{kind}: from {} on channel {:03X}", g.source, g.channel)
        } else {
            format!(
                "{kind}: {} to {} on channel {:03X}",
                g.source, g.target, g.channel
            )
        };
        if g.emergency {
            text.push_str(", emergency");
        }
        if g.encrypted {
            text.push_str(", encrypted");
        }
        self.msg(n, a, Meaning::Grant(g), text)
    }

    fn other(&mut self, n: usize, a: Osw, text: String) -> Option<Message> {
        self.msg(n, a, Meaning::Other, text)
    }

    /// A is not understood; the next word is read again.
    fn unknown(&mut self, a: Osw) -> Option<Message> {
        self.msg(
            1,
            a,
            Meaning::Unknown,
            format!(
                "unknown: command {:03X}, address {:04X}",
                a.command, a.address
            ),
        )
    }
}
