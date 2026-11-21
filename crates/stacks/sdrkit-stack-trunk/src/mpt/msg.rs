//! MPT 1327 address codewords: the message structures read into a type mnemonic, called and calling addresses, a channel, a
//! system and what the message means, and their assembly for the test signal.
//!
//! Standard: MPT 1327 section 5.

use super::code::{encode, field, put};

sdrkit_core::choice_enum! {
    /// The type mnemonic of an address codeword (MPT 1327 section 5).
    pub enum MptType {
        /// Go to channel.
        Gtc = ("GTC", "GTC: go to channel"),
        /// Aloha, any single codeword message invited.
        Alh = ("ALH", "ALH: aloha"),
        /// Aloha, except RQD.
        Alhs = ("ALHS", "ALHS: aloha, except RQD"),
        /// Aloha, except RQS.
        Alhd = ("ALHD", "ALHD: aloha, except RQS"),
        /// Aloha, emergency requests only.
        Alhe = ("ALHE", "ALHE: aloha, emergency only"),
        /// Aloha, registration or emergency requests.
        Alhr = ("ALHR", "ALHR: aloha, registration or emergency"),
        /// Aloha, except RQR.
        Alhx = ("ALHX", "ALHX: aloha, except RQR"),
        /// Aloha, fall-back mode.
        Alhf = ("ALHF", "ALHF: aloha, fall-back"),
        /// Acknowledgement.
        Ack = ("ACK", "ACK: acknowledgement"),
        /// Intermediate acknowledgement.
        Acki = ("ACKI", "ACKI: intermediate acknowledgement"),
        /// Call queued.
        Ackq = ("ACKQ", "ACKQ: call queued"),
        /// Message rejected.
        Ackx = ("ACKX", "ACKX: message rejected"),
        /// Called unit unavailable.
        Ackv = ("ACKV", "ACKV: called unit unavailable"),
        /// Emergency call acknowledged.
        Acke = ("ACKE", "ACKE: emergency call acknowledged"),
        /// Try on the given address.
        Ackt = ("ACKT", "ACKT: try on the given address"),
        /// Call-back or negative acknowledgement.
        Ackb = ("ACKB", "ACKB: call-back"),
        /// Availability check.
        Ahy = ("AHY", "AHY: availability check"),
        /// Cancel alert or waiting state.
        Ahyx = ("AHYX", "AHYX: cancel alert"),
        /// Called unit presence check.
        Ahyp = ("AHYP", "AHYP: presence check"),
        /// Status message.
        Ahyq = ("AHYQ", "AHYQ: status message"),
        /// Short data invitation.
        Ahyc = ("AHYC", "AHYC: short data invitation"),
        /// Control channel marker.
        Mark = ("MARK", "MARK: control channel marker"),
        /// Call maintenance.
        Maint = ("MAINT", "MAINT: call maintenance"),
        /// Clear down.
        Clear = ("CLEAR", "CLEAR: clear down"),
        /// Move to another control channel.
        Move = ("MOVE", "MOVE: move to control channel"),
        /// System broadcast.
        Bcast = ("BCAST", "BCAST: broadcast"),
        /// Short data message header.
        Head = ("HEAD", "HEAD: short data header"),
        /// Single address message.
        Sam = ("SAM", "SAM: single address message"),
        /// Standard data call set-up.
        StdData = ("STDDATA", "STDDATA: standard data"),
        /// Standard data acknowledgement.
        StdAck = ("STDACK", "STDACK: standard data acknowledgement"),
        /// A spare function value.
        Spare = ("spare", "Spare"),
        /// A reserved value or category.
        Reserved = ("RSVD", "Reserved"),
    }
}

impl MptType {
    /// The mnemonic.
    pub fn mnemonic(self) -> &'static str {
        <Self as sdrkit_core::Choice>::CHOICES[usize::from(sdrkit_core::Choice::index(self))]
            .key
            .as_ref()
    }
}

/// One decoded address codeword.
#[derive(Clone, Debug, PartialEq)]
pub struct MptMessage {
    /// Signal time of the codeword's end, s.
    pub time: f64,
    /// The type.
    pub kind: MptType,
    /// The called address text ("-" when absent).
    pub called: String,
    /// The calling address text ("-" when absent).
    pub caller: String,
    /// The channel named (-1 when none).
    pub channel: i32,
    /// The system named (MARK, BCAST, else -1).
    pub system: i32,
    /// What the message means, in words.
    pub meaning: String,
    /// The codeword (64 bits).
    pub codeword: u64,
    /// The GTC is a data call (D = 1).
    pub data: bool,
    /// A broadcast that announces a control channel.
    pub announces_control: bool,
}

impl sdrkit_graph::Item for MptMessage {
    const KIND: &'static str = "mpt.message";
}

/// The address text of MPT 1327 Table 4-1: the special idents by mnemonic, short-form idents, "{PFIX}/{IDENT}", or "ident {N}"
/// for an IDENT without its own prefix.
pub fn address_text(pfix: Option<u32>, ident: u32) -> String {
    let special = match ident {
        8191 => Some("ALLI"),
        8190 => Some("TSCI"),
        8189 => Some("IPFIXI"),
        8188 => Some("SDMI"),
        8187 => Some("DIVERTI"),
        8186 => Some("INCI"),
        8185 => Some("REGI"),
        8103 => Some("DNI"),
        8102 => Some("PABXI"),
        8101 => Some("PSTNGI"),
        0 => Some("DUMMYI"),
        _ => None,
    };
    if let Some(s) = special {
        return s.into();
    }
    if (8121..=8135).contains(&ident) {
        return format!("short-form ident {}", ident - 8120);
    }
    match pfix {
        Some(p) => format!("{p}/{ident}"),
        None => format!("ident {ident}"),
    }
}

const ALOHA: [&str; 8] = [
    "any single codeword message invited",
    "invited, except RQD",
    "invited, except RQS",
    "emergency requests only",
    "registration or emergency requests",
    "invited, except RQR",
    "fall-back mode",
    "reserved",
];
const ALOHA_T: [MptType; 8] = [
    MptType::Alh,
    MptType::Alhs,
    MptType::Alhd,
    MptType::Alhe,
    MptType::Alhr,
    MptType::Alhx,
    MptType::Alhf,
    MptType::Reserved,
];
const ACK: [&str; 8] = [
    "acknowledgement",
    "intermediate acknowledgement",
    "call queued",
    "message rejected",
    "called unit unavailable",
    "emergency call acknowledged",
    "try on the given address",
    "call-back or negative acknowledgement",
];
const ACK_T: [MptType; 8] = [
    MptType::Ack,
    MptType::Acki,
    MptType::Ackq,
    MptType::Ackx,
    MptType::Ackv,
    MptType::Acke,
    MptType::Ackt,
    MptType::Ackb,
];
const AHOY: [&str; 8] = [
    "availability check",
    "spare (customisation)",
    "cancel alert / waiting state",
    "reserved",
    "reserved",
    "called unit presence check",
    "status message",
    "short data invitation",
];
const AHOY_T: [MptType; 8] = [
    MptType::Ahy,
    MptType::Spare,
    MptType::Ahyx,
    MptType::Reserved,
    MptType::Reserved,
    MptType::Ahyp,
    MptType::Ahyq,
    MptType::Ahyc,
];
const MISC_T: [MptType; 8] = [
    MptType::Mark,
    MptType::Maint,
    MptType::Clear,
    MptType::Move,
    MptType::Bcast,
    MptType::Reserved,
    MptType::Reserved,
    MptType::Reserved,
];
const OPER: [&str; 8] = [
    "pressel on",
    "pressel off",
    "periodic message",
    "disconnect",
    "customised",
    "reserved",
    "clear down units not labelled by this address",
    "disable user transmission",
];

/// Reads an address codeword (bit 1 = 1). Returns `None` for a data codeword (bit 1 = 0).
pub fn decode(cw: u64, time: f64) -> Option<MptMessage> {
    if cw >> 63 == 0 {
        return None;
    }
    let pfix = field(cw, 2, 8);
    let ident1 = field(cw, 9, 21);
    let called_std = address_text(Some(pfix), ident1);
    let mut m = MptMessage {
        time,
        kind: MptType::Reserved,
        called: called_std.clone(),
        caller: "-".into(),
        channel: -1,
        system: -1,
        meaning: String::new(),
        codeword: cw,
        data: false,
        announces_control: false,
    };
    if field(cw, 22, 22) == 0 {
        // Go to channel (5.4).
        let d = field(cw, 23, 23) == 1;
        let chan = field(cw, 24, 33);
        let ident2 = field(cw, 34, 46);
        m.kind = MptType::Gtc;
        m.caller = address_text(None, ident2);
        m.channel = chan as i32;
        m.data = d;
        m.meaning = if d {
            format!("go to channel {chan}, data (audio muted)")
        } else {
            format!("go to channel {chan}, speech")
        };
        return Some(m);
    }
    let cat = field(cw, 23, 25);
    match cat {
        0 => {
            let typ = field(cw, 26, 27);
            let func = field(cw, 28, 30) as usize;
            match typ {
                0 => {
                    let chan4 = field(cw, 31, 34);
                    let wt = field(cw, 35, 37);
                    let mm = field(cw, 40, 44);
                    let n = field(cw, 45, 48);
                    m.kind = ALOHA_T[func];
                    m.meaning = format!(
                        "{}, control channel (low 4 bits) {chan4}, wait {wt}, address bits {mm}, aloha number {n}",
                        ALOHA[func]
                    );
                }
                1 => {
                    let ident2 = field(cw, 31, 43);
                    let qual = field(cw, 44, 44);
                    m.kind = ACK_T[func];
                    m.caller = address_text(None, ident2);
                    m.meaning = format!("{}, qualifier {qual}", ACK[func]);
                }
                2 => {
                    let ident2 = field(cw, 31, 43);
                    m.kind = AHOY_T[func];
                    m.caller = address_text(None, ident2);
                    m.meaning = match func {
                        0 => {
                            let d = field(cw, 44, 44) == 1;
                            let point = field(cw, 45, 45) == 1;
                            let check = field(cw, 46, 46) == 1;
                            let e = field(cw, 47, 47) == 1;
                            let ad = field(cw, 48, 48) == 1;
                            let mut s = format!(
                                "availability check of the {} unit, {}",
                                if point { "calling" } else { "called" },
                                if d { "data" } else { "speech" }
                            );
                            if check {
                                s.push_str(", readiness checked");
                            }
                            if e {
                                s.push_str(", emergency");
                            }
                            if ad {
                                s.push_str(", data codeword follows");
                            }
                            s
                        }
                        6 => format!("status message {}", field(cw, 44, 48)),
                        7 => format!(
                            "short data invitation, descriptor {}, {} slots",
                            field(cw, 46, 48),
                            field(cw, 44, 45)
                        ),
                        f => AHOY[f].into(),
                    };
                }
                _ => {
                    m.kind = MISC_T[func];
                    match func {
                        0 => {
                            let chan4 = field(cw, 2, 5);
                            let sys = field(cw, 7, 21);
                            m.called = "-".into();
                            m.system = sys as i32;
                            m.meaning = format!(
                                "control channel marker, system {sys}, control channel (low 4 bits) {chan4}"
                            );
                        }
                        1 => {
                            let chan = field(cw, 31, 40);
                            let oper = field(cw, 41, 43) as usize;
                            m.channel = chan as i32;
                            m.meaning =
                                format!("call maintenance on channel {chan}: {}", OPER[oper]);
                        }
                        2 => {
                            let chan = field(cw, 2, 11);
                            let cont = field(cw, 12, 21);
                            m.called = "-".into();
                            m.channel = chan as i32;
                            m.meaning = format!("clear down from channel {chan}");
                            if cont != 0 {
                                m.meaning
                                    .push_str(&format!(", move to control channel {cont}"));
                            }
                        }
                        3 => {
                            let cont = field(cw, 31, 40);
                            let q = field(cw, 41, 45);
                            m.channel = cont as i32;
                            m.meaning =
                                format!("move to control channel {cont} (address qualifier {q})");
                        }
                        4 => {
                            let sysdef = field(cw, 2, 6);
                            let sys = field(cw, 7, 21);
                            m.called = "-".into();
                            m.system = sys as i32;
                            let chan = field(cw, 31, 40);
                            let site = field(cw, 45, 48);
                            m.meaning = match sysdef {
                                0 | 1 | 4 | 5 => {
                                    m.channel = chan as i32;
                                    m.announces_control = sysdef == 0;
                                    let what = match sysdef {
                                        0 => "control channel announced",
                                        1 => "control channel withdrawn",
                                        4 => "adjacent site control channel",
                                        _ => "vote now advice",
                                    };
                                    let mut s =
                                        format!("{what}, system {sys}, control channel {chan}");
                                    if (sysdef == 4 || sysdef == 5) && site != 0 {
                                        s.push_str(&format!(", site {site}"));
                                    }
                                    s
                                }
                                2 => {
                                    let per = field(cw, 31, 31);
                                    let ival = field(cw, 32, 36);
                                    let pon = field(cw, 37, 37);
                                    let id = field(cw, 38, 38);
                                    let mut s = if per == 0 {
                                        format!(
                                            "call maintenance parameters, system {sys}: periodic messages every {ival} s"
                                        )
                                    } else {
                                        format!("call maintenance parameters, system {sys}: no periodic messages")
                                    };
                                    s.push_str(&format!(
                                        ", pressel on {}, MAINT address {}",
                                        if pon == 0 { "sent" } else { "not sent" },
                                        if id == 0 { "group" } else { "individual" }
                                    ));
                                    s
                                }
                                3 => format!("registration parameters, system {sys}"),
                                _ => format!(
                                    "broadcast type {sysdef}, system {sys} (reserved or spare)"
                                ),
                            };
                        }
                        _ => {
                            m.meaning = "reserved".into();
                        }
                    }
                }
            }
        }
        1 => {
            if field(cw, 26, 26) == 1 {
                let len = field(cw, 27, 28);
                let pfix2 = field(cw, 29, 35);
                let ident2 = field(cw, 36, 48);
                m.kind = MptType::Head;
                m.caller = address_text(Some(pfix2), ident2);
                let n = len + 1;
                m.meaning = if n == 1 {
                    "short data message header, 1 data codeword follows".into()
                } else {
                    format!("short data message header, {n} data codewords follow")
                };
            } else {
                m.kind = MptType::Sam;
                m.meaning = "single address message".into();
            }
        }
        2 => {
            m.kind = MptType::StdData;
            m.meaning = "standard data call set-up".into();
        }
        5 => {
            m.kind = MptType::StdAck;
            m.meaning = "standard data acknowledgement or selective acknowledgement".into();
        }
        c => {
            m.kind = MptType::Reserved;
            m.meaning = format!("category {c} (reserved or spare)");
        }
    }
    Some(m)
}

/// Builds address codewords from their fields (for the test signal and the tests).
pub mod build {
    use super::*;

    fn word(f: &[(u32, u32, u32)]) -> u64 {
        let mut i = 0u64;
        for &(a, b, v) in f {
            put(&mut i, a, b, v);
        }
        encode(i)
    }
    /// GTC (5.4).
    pub fn gtc(pfix: u32, ident1: u32, d: bool, chan: u32, ident2: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 0),
            (23, 23, d as u32),
            (24, 33, chan),
            (34, 46, ident2),
        ])
    }
    /// Aloha (5.5.1).
    #[allow(clippy::too_many_arguments)]
    pub fn aloha(pfix: u32, ident1: u32, func: u32, chan4: u32, wt: u32, m: u32, n: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 1),
            (23, 25, 0),
            (26, 27, 0),
            (28, 30, func),
            (31, 34, chan4),
            (35, 37, wt),
            (40, 44, m),
            (45, 48, n),
        ])
    }
    /// Acknowledgement (5.5.2).
    pub fn ack(pfix: u32, ident1: u32, func: u32, ident2: u32, qual: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 1),
            (26, 27, 1),
            (28, 30, func),
            (31, 43, ident2),
            (44, 44, qual),
        ])
    }
    /// Ahoy (5.5.3.2), with bits 44 to 48 as given.
    pub fn ahoy(pfix: u32, ident1: u32, func: u32, ident2: u32, tail5: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 1),
            (26, 27, 2),
            (28, 30, func),
            (31, 43, ident2),
            (44, 48, tail5),
        ])
    }
    /// Miscellaneous (5.5.4) with function `func` and the given extra fields.
    pub fn misc(func: u32, fields: &[(u32, u32, u32)]) -> u64 {
        let mut f = vec![(1, 1, 1), (22, 22, 1), (26, 27, 3), (28, 30, func)];
        f.extend_from_slice(fields);
        word(&f)
    }
    /// BCAST (5.5.4.5).
    pub fn bcast(sysdef: u32, sys: u32, params: &[(u32, u32, u32)]) -> u64 {
        let mut f = vec![(2, 6, sysdef), (7, 21, sys)];
        f.extend_from_slice(params);
        misc(4, &f)
    }
    /// Short data header (5.6.2).
    pub fn head(pfix: u32, ident1: u32, len: u32, pfix2: u32, ident2: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 1),
            (23, 25, 1),
            (26, 26, 1),
            (27, 28, len),
            (29, 35, pfix2),
            (36, 48, ident2),
        ])
    }
    /// A codeword of category `cat` (SAM with bit 26 = 0 for category 1).
    pub fn category(pfix: u32, ident1: u32, cat: u32) -> u64 {
        word(&[
            (1, 1, 1),
            (2, 8, pfix),
            (9, 21, ident1),
            (22, 22, 1),
            (23, 25, cat),
        ])
    }

    /// The eight test messages, in order.
    pub fn eight_messages() -> Vec<u64> {
        vec![
            gtc(5, 1234, false, 17, 2003),
            aloha(5, 0, 0, 3, 2, 8, 1),
            ack(5, 77, 0, 4321, 0),
            ahoy(9, 100, 0, 0, 0),
            bcast(0, 20001, &[(31, 40, 3)]),
            bcast(4, 20001, &[(31, 40, 9), (45, 48, 2)]),
            gtc(100, 8000, true, 1023, 8100),
            ack(127, 8191, 3, 1, 1),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    fn d(cw: u64) -> MptMessage {
        assert!(crate::mpt::code::valid(cw));
        decode(cw, 0.0).unwrap()
    }

    #[test]
    fn address_texts() {
        assert_eq!(address_text(None, 8191), "ALLI");
        assert_eq!(address_text(Some(5), 8190), "TSCI");
        assert_eq!(address_text(Some(5), 8186), "INCI");
        assert_eq!(address_text(Some(5), 8101), "PSTNGI");
        assert_eq!(address_text(Some(3), 100), "3/100");
        assert_eq!(address_text(Some(3), 0), "DUMMYI");
        assert_eq!(address_text(Some(3), 8125), "short-form ident 5");
        assert_eq!(address_text(Some(5), 1234), "5/1234");
        assert_eq!(address_text(None, 7), "ident 7");
    }

    #[test]
    fn every_structure() {
        let m = d(gtc(5, 1234, true, 801, 77));
        assert_eq!((m.kind.mnemonic(), m.channel), ("GTC", 801));
        assert!(m.meaning.contains("data"));
        for f in 0..8 {
            let m = d(aloha(5, 0, f, 11, 5, 17, 9));
            assert_eq!(m.kind, ALOHA_T[f as usize]);
            for s in [
                "(low 4 bits) 11",
                "wait 5",
                "address bits 17",
                "aloha number 9",
            ] {
                assert!(m.meaning.contains(s), "{}", m.meaning);
            }
            let m = d(ack(5, 1, f, 4321, 1));
            assert_eq!(m.kind, ACK_T[f as usize]);
            assert!(m.meaning.contains("qualifier 1"));
            assert_eq!(m.caller, "ident 4321");
        }
        let m = d(ahoy(5, 1, 0, 8189, 0b11111));
        assert_eq!(m.caller, "IPFIXI");
        for s in [
            "calling",
            "emergency",
            "data codeword follows",
            "readiness checked",
        ] {
            assert!(m.meaning.contains(s), "{}", m.meaning);
        }
        assert_eq!(d(ahoy(5, 1, 2, 3, 0)).kind.mnemonic(), "AHYX");
        let m = d(ahoy(5, 1, 5, 8190, 0));
        assert_eq!((m.kind.mnemonic(), m.caller.as_str()), ("AHYP", "TSCI"));
        assert_eq!(d(ahoy(5, 1, 6, 3, 21)).meaning, "status message 21");
        let m = d(ahoy(5, 1, 7, 8188, 0b10000));
        assert_eq!(m.caller, "SDMI");
        assert!(
            m.meaning.contains("descriptor 0") && m.meaning.contains("2 slots"),
            "{}",
            m.meaning
        );
        let m = d(misc(0, &[(2, 5, 10), (7, 21, 31000)]));
        assert_eq!((m.kind.mnemonic(), m.system), ("MARK", 31000));
        assert!(m.meaning.contains("(low 4 bits) 10"));
        let m = d(misc(
            1,
            &[(2, 8, 5), (9, 21, 10), (31, 40, 77), (41, 43, 3)],
        ));
        assert_eq!((m.kind.mnemonic(), m.channel), ("MAINT", 77));
        assert!(m.meaning.contains("disconnect"));
        let m = d(misc(2, &[(2, 11, 500), (12, 21, 3)]));
        assert_eq!(
            (m.kind.mnemonic(), m.channel, m.called.as_str()),
            ("CLEAR", 500, "-")
        );
        assert!(m.meaning.contains("control channel 3"));
        let m = d(misc(
            3,
            &[(2, 8, 8), (9, 21, 1), (31, 40, 640), (41, 45, 19)],
        ));
        assert_eq!((m.kind.mnemonic(), m.channel), ("MOVE", 640));
        assert!(m.called.starts_with("8/") && m.meaning.contains("qualifier 19"));
        let m = d(bcast(0, 1, &[(31, 40, 311)]));
        assert!(m.announces_control && m.channel == 311);
        assert!(d(bcast(1, 1, &[(31, 40, 311)]))
            .meaning
            .contains("withdrawn"));
        let m = d(bcast(
            2,
            1,
            &[(31, 31, 0), (32, 36, 12), (37, 37, 0), (38, 38, 1)],
        ));
        for s in ["every 12 s", "pressel on sent", "individual"] {
            assert!(m.meaning.contains(s), "{}", m.meaning);
        }
        let m = d(bcast(4, 1, &[(31, 40, 99), (45, 48, 6)]));
        assert!(m.meaning.contains("site 6") && m.channel == 99);
        let m = d(head(5, 8190, 2, 6, 1500));
        assert_eq!(
            (m.kind.mnemonic(), m.called.as_str(), m.caller.as_str()),
            ("HEAD", "TSCI", "6/1500")
        );
        assert!(m.meaning.contains("3 data codewords"));
        assert_eq!(d(category(5, 1, 1)).kind.mnemonic(), "SAM");
        assert_eq!(d(category(5, 1, 2)).kind.mnemonic(), "STDDATA");
        assert_eq!(d(category(5, 1, 5)).kind.mnemonic(), "STDACK");
        assert_eq!(d(category(5, 1, 6)).kind.mnemonic(), "RSVD");
    }

    #[test]
    fn the_eight_messages_read_as_expected() {
        let ms: Vec<MptMessage> = eight_messages().into_iter().map(d).collect();
        assert_eq!(ms[0].meaning, "go to channel 17, speech");
        assert_eq!(ms[0].caller, "ident 2003");
        assert_eq!(ms[1].called, "DUMMYI");
        assert_eq!(
            ms[1].meaning,
            "any single codeword message invited, control channel (low 4 bits) 3, wait 2, address bits 8, aloha number 1"
        );
        assert_eq!(ms[3].caller, "DUMMYI");
        assert_eq!(
            ms[3].meaning,
            "availability check of the called unit, speech"
        );
        assert_eq!(
            ms[4].meaning,
            "control channel announced, system 20001, control channel 3"
        );
        assert_eq!(
            ms[5].meaning,
            "adjacent site control channel, system 20001, control channel 9, site 2"
        );
        assert_eq!(ms[6].meaning, "go to channel 1023, data (audio muted)");
        assert_eq!(
            (ms[7].kind.mnemonic(), ms[7].called.as_str()),
            ("ACKX", "ALLI")
        );
        assert_eq!(ms[7].meaning, "message rejected, qualifier 1");
    }
}
