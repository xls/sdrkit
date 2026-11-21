//! AX.25 frames: addresses, the control fields in modulo 8 and 128, the PID, XID and FRMR fields, and the TNC2 text form
//! `SOURCE>DEST,PATH:info`. The FCS is not part of a frame here: [`crate::hdlc`] adds and checks it.
//!
//! Standard: AX.25 Link Access Protocol for Amateur Packet Radio 2.2 (1998) clause 3

use crate::{Error, Result};
use crate::{Param, Part};
use std::fmt;

/// The most addresses in a frame: destination, source and eight repeaters (clause 3.12.3).
pub const MAX_ADDRESSES: usize = 10;
/// The most repeaters in a frame.
pub const MAX_REPEATERS: usize = 8;

/// A callsign of one to six characters. [`Callsign::new`] takes upper-case letters and digits, as AX.25 sends them. A received
/// frame keeps lower-case letters as they came, so a frame from a station that breaks the rule still decodes and encodes back to
/// the same bytes.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Callsign {
    chars: [u8; 6],
    len: u8,
}

impl Callsign {
    /// A callsign from text: 1 to 6 upper-case ASCII letters or digits.
    pub fn new(text: &str) -> Result<Callsign> {
        let b = text.as_bytes();
        let ok = |c: &u8| c.is_ascii_uppercase() || c.is_ascii_digit();
        if b.is_empty() || b.len() > 6 || !b.iter().all(ok) {
            return Err(Error::Syntax(Part::Callsign));
        }
        let mut chars = [b' '; 6];
        chars[..b.len()].copy_from_slice(b);
        Ok(Callsign {
            chars,
            len: b.len() as u8,
        })
    }

    /// The callsign as text.
    pub fn as_str(&self) -> &str {
        // Only ASCII letters and digits are ever stored.
        std::str::from_utf8(&self.chars[..self.len as usize]).unwrap_or("")
    }
}

impl fmt::Display for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Callsign {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Callsign {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        Callsign::new(&s).map_err(serde::de::Error::custom)
    }
}

/// One address: a callsign, its SSID, and the top bit of the SSID byte, which is the command/response bit `C` in the destination
/// and source and the has-been-repeated bit `H` in a repeater address (clause 3.12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Address {
    /// The callsign.
    pub call: Callsign,
    /// The secondary station identifier, 0 to 15.
    pub ssid: u8,
    /// `C` (destination and source) or `H` (repeaters).
    pub flag: bool,
    /// The two reserved bits `R R` of the SSID byte as received (0b11 unless a protocol assigns them a meaning).
    pub reserved: u8,
}

impl Address {
    /// An address from text `CALL` or `CALL-SSID`. A final `*` sets the flag (the TNC2 mark of a repeater that has repeated).
    pub fn parse(text: &str) -> Result<Address> {
        let (text, flag) = match text.strip_suffix('*') {
            Some(t) => (t, true),
            None => (text, false),
        };
        let (call, ssid) = match text.split_once('-') {
            Some((c, n)) => {
                let ok = !n.is_empty() && n.len() <= 2 && n.bytes().all(|d| d.is_ascii_digit());
                let v: u8 = if ok { n.parse().unwrap_or(16) } else { 16 };
                if v > 15 {
                    return Err(Error::Syntax(Part::Ssid));
                }
                (c, v)
            }
            None => (text, 0),
        };
        Ok(Address {
            call: Callsign::new(call)?,
            ssid,
            flag,
            reserved: 0b11,
        })
    }

    /// An address from its 7 bytes, and whether its extension bit marks it the last address.
    pub fn from_bytes(b: &[u8]) -> Result<(Address, bool)> {
        if b.len() < 7 {
            return Err(Error::Short);
        }
        let mut chars = [b' '; 6];
        let mut len = 0;
        let mut ended = false;
        for (i, &c) in b[..6].iter().enumerate() {
            if c & 1 != 0 {
                return Err(Error::Syntax(Part::AddressExtension));
            }
            let ch = c >> 1;
            if ch == b' ' {
                ended = true;
            } else if ended || !ch.is_ascii_alphanumeric() {
                return Err(Error::Syntax(Part::AddressChar));
            } else {
                chars[i] = ch;
                len = i + 1;
            }
        }
        if len == 0 {
            return Err(Error::Syntax(Part::EmptyCallsign));
        }
        let s = b[6];
        Ok((
            Address {
                call: Callsign {
                    chars,
                    len: len as u8,
                },
                ssid: (s >> 1) & 0x0F,
                flag: s & 0x80 != 0,
                reserved: (s >> 5) & 0b11,
            },
            s & 1 != 0,
        ))
    }

    /// The 7 bytes of this address. `last` sets the extension bit.
    pub fn to_bytes(&self, last: bool) -> [u8; 7] {
        let mut o = [b' ' << 1; 7];
        for (d, &c) in o.iter_mut().zip(&self.call.chars[..self.call.len as usize]) {
            *d = c << 1;
        }
        o[6] = (u8::from(self.flag) << 7)
            | ((self.reserved & 3) << 5)
            | ((self.ssid & 0x0F) << 1)
            | u8::from(last);
        o
    }
}

impl fmt::Display for Address {
    /// `CALL` or `CALL-SSID` (the flag is not shown; [`Frame`]'s TNC2 text marks repeaters).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ssid == 0 {
            write!(f, "{}", self.call)
        } else {
            write!(f, "{}-{}", self.call, self.ssid)
        }
    }
}

/// The sequence-number range of a connection (clause 4.2): modulo 8 has 1-byte I and S control fields, modulo 128 (after SABME)
/// has 2-byte ones. A receiver learns it from the connection, not from the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Modulo {
    /// Sequence numbers 0 to 7.
    #[default]
    Eight,
    /// Sequence numbers 0 to 127.
    OneTwentyEight,
}

/// The supervisory frames (clause 4.3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Supervisory {
    /// Receive ready.
    Rr,
    /// Receive not ready.
    Rnr,
    /// Reject.
    Rej,
    /// Selective reject.
    Srej,
}

/// The unnumbered frames (clause 4.3.3), each with its control field with the P/F bit clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unnumbered {
    /// Set asynchronous balanced mode, extended (modulo 128): 0x6F.
    Sabme,
    /// Set asynchronous balanced mode: 0x2F.
    Sabm,
    /// Disconnect: 0x43.
    Disc,
    /// Disconnected mode: 0x0F.
    Dm,
    /// Unnumbered acknowledge: 0x63.
    Ua,
    /// Frame reject: 0x87.
    Frmr,
    /// Unnumbered information: 0x03.
    Ui,
    /// Exchange identification: 0xAF.
    Xid,
    /// Test: 0xE3.
    Test,
    /// Any other unnumbered control field (P/F bit clear).
    Other(u8),
}

impl Unnumbered {
    fn code(self) -> u8 {
        match self {
            Unnumbered::Sabme => 0x6F,
            Unnumbered::Sabm => 0x2F,
            Unnumbered::Disc => 0x43,
            Unnumbered::Dm => 0x0F,
            Unnumbered::Ua => 0x63,
            Unnumbered::Frmr => 0x87,
            Unnumbered::Ui => 0x03,
            Unnumbered::Xid => 0xAF,
            Unnumbered::Test => 0xE3,
            Unnumbered::Other(c) => c & !0x10,
        }
    }

    fn from_code(c: u8) -> Unnumbered {
        match c & !0x10 {
            0x6F => Unnumbered::Sabme,
            0x2F => Unnumbered::Sabm,
            0x43 => Unnumbered::Disc,
            0x0F => Unnumbered::Dm,
            0x63 => Unnumbered::Ua,
            0x87 => Unnumbered::Frmr,
            0x03 => Unnumbered::Ui,
            0xAF => Unnumbered::Xid,
            0xE3 => Unnumbered::Test,
            o => Unnumbered::Other(o),
        }
    }
}

/// A control field (clause 4.2, figures 4.1 and 4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    /// An information frame.
    I {
        /// Send sequence number N(S).
        ns: u8,
        /// Receive sequence number N(R).
        nr: u8,
        /// The poll bit.
        poll: bool,
    },
    /// A supervisory frame.
    S {
        /// Which one.
        kind: Supervisory,
        /// Receive sequence number N(R).
        nr: u8,
        /// The poll/final bit.
        pf: bool,
    },
    /// An unnumbered frame.
    U {
        /// Which one.
        kind: Unnumbered,
        /// The poll/final bit.
        pf: bool,
    },
}

impl Control {
    /// A UI frame's control field with the P/F bit clear (0x03), as APRS sends.
    pub const UI: Control = Control::U {
        kind: Unnumbered::Ui,
        pf: false,
    };

    /// Reads a control field from the start of `b`: the field and its length in bytes.
    pub fn parse(b: &[u8], modulo: Modulo) -> Result<(Control, usize)> {
        let c = *b.first().ok_or(Error::Short)?;
        if c & 3 == 3 {
            return Ok((
                Control::U {
                    kind: Unnumbered::from_code(c),
                    pf: c & 0x10 != 0,
                },
                1,
            ));
        }
        match modulo {
            Modulo::Eight => {
                let nr = c >> 5;
                let pf = c & 0x10 != 0;
                if c & 1 == 0 {
                    Ok((
                        Control::I {
                            ns: (c >> 1) & 7,
                            nr,
                            poll: pf,
                        },
                        1,
                    ))
                } else {
                    Ok((
                        Control::S {
                            kind: s_kind(c),
                            nr,
                            pf,
                        },
                        1,
                    ))
                }
            }
            Modulo::OneTwentyEight => {
                let d = *b.get(1).ok_or(Error::Short)?;
                let nr = d >> 1;
                let pf = d & 1 != 0;
                if c & 1 == 0 {
                    Ok((
                        Control::I {
                            ns: c >> 1,
                            nr,
                            poll: pf,
                        },
                        2,
                    ))
                } else {
                    if c & 0xF0 != 0 {
                        return Err(Error::Syntax(Part::SControl128));
                    }
                    Ok((
                        Control::S {
                            kind: s_kind(c),
                            nr,
                            pf,
                        },
                        2,
                    ))
                }
            }
        }
    }

    /// Writes the control field into `out` (1 or 2 bytes) and returns the length. An error when a sequence number does not fit
    /// the modulo.
    pub fn write(&self, modulo: Modulo, out: &mut [u8; 2]) -> Result<usize> {
        let max = match modulo {
            Modulo::Eight => 7,
            Modulo::OneTwentyEight => 127,
        };
        match *self {
            Control::U { kind, pf } => {
                out[0] = kind.code() | (u8::from(pf) << 4);
                Ok(1)
            }
            Control::I { ns, nr, poll } => {
                if ns > max || nr > max {
                    return Err(Error::BadParameter(Param::SequenceNumber));
                }
                match modulo {
                    Modulo::Eight => {
                        out[0] = (nr << 5) | (u8::from(poll) << 4) | (ns << 1);
                        Ok(1)
                    }
                    Modulo::OneTwentyEight => {
                        out[0] = ns << 1;
                        out[1] = (nr << 1) | u8::from(poll);
                        Ok(2)
                    }
                }
            }
            Control::S { kind, nr, pf } => {
                if nr > max {
                    return Err(Error::BadParameter(Param::SequenceNumber));
                }
                let ss = match kind {
                    Supervisory::Rr => 0,
                    Supervisory::Rnr => 1,
                    Supervisory::Rej => 2,
                    Supervisory::Srej => 3,
                } << 2;
                match modulo {
                    Modulo::Eight => {
                        out[0] = (nr << 5) | (u8::from(pf) << 4) | ss | 1;
                        Ok(1)
                    }
                    Modulo::OneTwentyEight => {
                        out[0] = ss | 1;
                        out[1] = (nr << 1) | u8::from(pf);
                        Ok(2)
                    }
                }
            }
        }
    }

    /// Whether frames of this type carry a PID (I and UI frames, clause 3.4).
    pub fn has_pid(&self) -> bool {
        matches!(
            self,
            Control::I { .. }
                | Control::U {
                    kind: Unnumbered::Ui,
                    ..
                }
        )
    }

    /// The frame type's name: `I`, `RR`, `RNR`, `REJ`, `SREJ`, `SABME`, `SABM`, `DISC`, `DM`, `UA`, `FRMR`, `UI`, `XID`, `TEST`
    /// or `U`.
    pub fn name(&self) -> &'static str {
        match self {
            Control::I { .. } => "I",
            Control::S { kind, .. } => match kind {
                Supervisory::Rr => "RR",
                Supervisory::Rnr => "RNR",
                Supervisory::Rej => "REJ",
                Supervisory::Srej => "SREJ",
            },
            Control::U { kind, .. } => match kind {
                Unnumbered::Sabme => "SABME",
                Unnumbered::Sabm => "SABM",
                Unnumbered::Disc => "DISC",
                Unnumbered::Dm => "DM",
                Unnumbered::Ua => "UA",
                Unnumbered::Frmr => "FRMR",
                Unnumbered::Ui => "UI",
                Unnumbered::Xid => "XID",
                Unnumbered::Test => "TEST",
                Unnumbered::Other(_) => "U",
            },
        }
    }
}

fn s_kind(c: u8) -> Supervisory {
    match (c >> 2) & 3 {
        0 => Supervisory::Rr,
        1 => Supervisory::Rnr,
        2 => Supervisory::Rej,
        _ => Supervisory::Srej,
    }
}

/// The meaning of a PID byte (clause 3.4, figure 3.2): `None` for values the standard leaves undefined.
pub fn pid_name(pid: u8) -> Option<&'static str> {
    Some(match pid {
        0x01 => "ISO 8208/CCITT X.25 PLP",
        0x06 => "compressed TCP/IP (RFC 1144)",
        0x07 => "uncompressed TCP/IP (RFC 1144)",
        0x08 => "segmentation fragment",
        0xC3 => "TEXNET datagram protocol",
        0xC4 => "link quality protocol",
        0xCA => "Appletalk",
        0xCB => "Appletalk ARP",
        0xCC => "ARPA Internet Protocol",
        0xCD => "ARPA Address Resolution",
        0xCE => "FlexNet",
        0xCF => "NET/ROM",
        0xF0 => "no layer 3",
        0xFF => "escape (next byte has more)",
        p if p & 0x30 == 0x10 || p & 0x30 == 0x20 => "AX.25 layer 3",
        _ => return None,
    })
}

/// The PID of "no layer 3 protocol", as APRS uses (0xF0).
pub const PID_NONE: u8 = 0xF0;

/// Whether a frame is a command or a response (clause 6.1.2): version 2 sets the `C` bits of destination and source to opposite
/// values. Two equal bits are the version 1 form, which says nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandResponse {
    /// Destination `C` = 1, source `C` = 0.
    Command,
    /// Destination `C` = 0, source `C` = 1.
    Response,
    /// Both bits equal (AX.25 version 1 or earlier).
    Version1,
}

/// An AX.25 frame without its FCS. See the [module documentation](self).
///
/// ```
/// use sdrkit_proto_link::ax25::{Control, Frame, Unnumbered};
/// let f = Frame::from_tnc2("N0CALL-9>APRS,WIDE1-1*,WIDE2-1:!4903.50N/07201.75W-Test").unwrap();
/// assert_eq!(f.control, Control::U { kind: Unnumbered::Ui, pf: false });
/// let mut bytes = Vec::new();
/// f.encode(&mut bytes).unwrap();
/// let g = Frame::parse(&bytes).unwrap();
/// assert_eq!(g.to_string(), "N0CALL-9>APRS,WIDE1-1*,WIDE2-1:!4903.50N/07201.75W-Test");
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    addrs: [Address; MAX_ADDRESSES],
    count: u8,
    /// The control field.
    pub control: Control,
    /// The modulo the control field was read or is written with.
    pub modulo: Modulo,
    /// The PID (I and UI frames).
    pub pid: Option<u8>,
    /// The information field (empty for frames that have none).
    pub info: &'a [u8],
}

impl<'a> Frame<'a> {
    /// Parses frame bytes (FCS removed) of a modulo-8 link, which is what every connectionless station (APRS, beacons, CubeSats)
    /// uses.
    pub fn parse(bytes: &'a [u8]) -> Result<Frame<'a>> {
        Frame::parse_with(bytes, Modulo::Eight)
    }

    /// Parses frame bytes (FCS removed) with the given modulo.
    pub fn parse_with(bytes: &'a [u8], modulo: Modulo) -> Result<Frame<'a>> {
        let mut addrs = [Address::default(); MAX_ADDRESSES];
        let mut count = 0usize;
        let mut at = 0;
        loop {
            if count == MAX_ADDRESSES {
                return Err(Error::Syntax(Part::TooManyAddresses));
            }
            let (a, last) = Address::from_bytes(bytes.get(at..).ok_or(Error::Short)?)?;
            addrs[count] = a;
            count += 1;
            at += 7;
            if last {
                break;
            }
        }
        if count < 2 {
            return Err(Error::Syntax(Part::OneAddress));
        }
        let (control, n) = Control::parse(&bytes[at..], modulo)?;
        at += n;
        let pid = if control.has_pid() {
            let p = *bytes.get(at).ok_or(Error::Short)?;
            at += 1;
            Some(p)
        } else {
            None
        };
        Ok(Frame {
            addrs,
            count: count as u8,
            control,
            modulo,
            pid,
            info: &bytes[at..],
        })
    }

    /// A frame from its parts. An error with more than 8 repeaters, or a PID given (or missing) against the frame type.
    pub fn new(
        destination: Address,
        source: Address,
        repeaters: &[Address],
        control: Control,
        pid: Option<u8>,
        info: &'a [u8],
    ) -> Result<Frame<'a>> {
        if repeaters.len() > MAX_REPEATERS {
            return Err(Error::TooLong);
        }
        if control.has_pid() != pid.is_some() {
            return Err(Error::BadParameter(Param::Pid));
        }
        let mut addrs = [Address::default(); MAX_ADDRESSES];
        addrs[0] = destination;
        addrs[1] = source;
        addrs[2..2 + repeaters.len()].copy_from_slice(repeaters);
        Ok(Frame {
            addrs,
            count: 2 + repeaters.len() as u8,
            control,
            modulo: Modulo::Eight,
            pid,
            info,
        })
    }

    /// A UI command frame with PID 0xF0 (no layer 3), as APRS and beacons send: destination `C` = 1, source `C` = 0
    /// (clause 6.1.2).
    pub fn ui(
        destination: Address,
        source: Address,
        repeaters: &[Address],
        info: &'a [u8],
    ) -> Result<Frame<'a>> {
        let mut f = Frame::new(
            destination,
            source,
            repeaters,
            Control::UI,
            Some(PID_NONE),
            info,
        )?;
        f.addrs[0].flag = true;
        f.addrs[1].flag = false;
        Ok(f)
    }

    /// A UI command frame from TNC2 text `SOURCE>DEST,REPEATER*,REPEATER:info`. A `*` marks the last repeater that has
    /// repeated: it and every repeater before it get the `H` bit. An error when an address is not an AX.25 address (APRS-IS
    /// constructs such as `qAR` or `TCPIP*` cannot go on the air. Read those with [`Tnc2`]).
    pub fn from_tnc2(text: &'a str) -> Result<Frame<'a>> {
        let t = Tnc2::parse(text)?;
        let source = Address::parse(t.source)?;
        let destination = Address::parse(t.destination)?;
        let mut reps = [Address::default(); MAX_REPEATERS];
        let mut n = 0;
        let mut last_used = None;
        for p in t.path() {
            if n == MAX_REPEATERS {
                return Err(Error::TooLong);
            }
            let a = Address::parse(p)?;
            if a.flag {
                last_used = Some(n);
            }
            reps[n] = a;
            n += 1;
        }
        for (i, r) in reps[..n].iter_mut().enumerate() {
            r.flag = last_used.is_some_and(|l| i <= l);
        }
        Frame::ui(destination, source, &reps[..n], t.info.as_bytes())
    }

    /// The destination address.
    pub fn destination(&self) -> &Address {
        &self.addrs[0]
    }

    /// The source address.
    pub fn source(&self) -> &Address {
        &self.addrs[1]
    }

    /// The repeater addresses (0 to 8).
    pub fn repeaters(&self) -> &[Address] {
        &self.addrs[2..self.count as usize]
    }

    /// Mutable access to the repeater addresses (to set `H` bits when repeating).
    pub fn repeaters_mut(&mut self) -> &mut [Address] {
        &mut self.addrs[2..self.count as usize]
    }

    /// Command or response, from the `C` bits.
    pub fn command_response(&self) -> CommandResponse {
        match (self.addrs[0].flag, self.addrs[1].flag) {
            (true, false) => CommandResponse::Command,
            (false, true) => CommandResponse::Response,
            _ => CommandResponse::Version1,
        }
    }

    /// The encoded length in bytes (without the FCS).
    pub fn len(&self) -> usize {
        let c = match (self.control, self.modulo) {
            (Control::U { .. }, _) | (_, Modulo::Eight) => 1,
            _ => 2,
        };
        7 * self.count as usize + c + usize::from(self.pid.is_some()) + self.info.len()
    }

    /// Always false: a frame has at least two addresses and a control field.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Appends the frame's bytes (without the FCS) to `out`. An error when a sequence number does not fit the modulo.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        let mut c = [0u8; 2];
        let cl = self.control.write(self.modulo, &mut c)?;
        let n = self.count as usize;
        for (i, a) in self.addrs[..n].iter().enumerate() {
            out.extend_from_slice(&a.to_bytes(i + 1 == n));
        }
        out.extend_from_slice(&c[..cl]);
        if let Some(p) = self.pid {
            out.push(p);
        }
        out.extend_from_slice(self.info);
        Ok(())
    }

    /// The XID parameters of an XID frame's information field (clause 4.3.3.7).
    pub fn xid(&self) -> Result<Xid<'a>> {
        Xid::parse(self.info)
    }

    /// The FRMR information of a FRMR frame (clause 4.3.3.9).
    pub fn frmr(&self) -> Result<Frmr> {
        Frmr::parse(self.info, self.modulo)
    }
}

impl fmt::Display for Frame<'_> {
    /// The TNC2 monitor form `SOURCE>DEST,REPEATER*,REPEATER:info`: the last repeater whose `H` bit is set is marked with `*`, and
    /// the information field is shown as text (bytes that are not UTF-8 become U+FFFD).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}>{}", self.source(), self.destination())?;
        let reps = self.repeaters();
        let star = reps.iter().rposition(|r| r.flag);
        for (i, r) in reps.iter().enumerate() {
            write!(f, ",{r}")?;
            if Some(i) == star {
                f.write_str("*")?;
            }
        }
        f.write_str(":")?;
        for chunk in self.info.utf8_chunks() {
            f.write_str(chunk.valid())?;
            if !chunk.invalid().is_empty() {
                f.write_str("\u{FFFD}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("destination", self.destination())
            .field("source", self.source())
            .field("repeaters", &self.repeaters())
            .field("control", &self.control)
            .field("pid", &self.pid)
            .field("info", &self.info)
            .finish()
    }
}

/// The TNC2 text of a packet split into its parts. The addresses can be other than AX.25 addresses, because APRS-IS lines carry
/// `qAR`, `TCPIP*` and callsigns longer than six characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tnc2<'a> {
    /// The source.
    pub source: &'a str,
    /// The destination.
    pub destination: &'a str,
    /// The path after the destination, comma separated (empty when there is none).
    pub path: &'a str,
    /// The information field.
    pub info: &'a str,
}

impl<'a> Tnc2<'a> {
    /// Splits `SOURCE>DEST[,PATH...]:info` (a final CR or LF is not part of the information).
    pub fn parse(text: &'a str) -> Result<Tnc2<'a>> {
        let text = text.trim_end_matches(['\r', '\n']);
        let (header, info) = text
            .split_once(':')
            .ok_or(Error::Syntax(Part::Tnc2Header))?;
        let (source, rest) = header
            .split_once('>')
            .ok_or(Error::Syntax(Part::Tnc2Header))?;
        let (destination, path) = rest.split_once(',').unwrap_or((rest, ""));
        if source.is_empty() || destination.is_empty() {
            return Err(Error::Syntax(Part::Tnc2Header));
        }
        Ok(Tnc2 {
            source,
            destination,
            path,
            info,
        })
    }

    /// The path entries.
    pub fn path(&self) -> impl Iterator<Item = &'a str> {
        self.path.split(',').filter(|p| !p.is_empty())
    }
}

/// The XID information field (clause 4.3.3.7): format indicator 0x82, group identifier 0x80, a 16-bit group length, and then
/// parameters as identifier, length and value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xid<'a> {
    params: &'a [u8],
}

/// XID parameter identifiers (clause 4.3.3.7, figure 4.5).
pub mod xid {
    /// Classes of procedures (half or full duplex).
    pub const CLASSES: u8 = 2;
    /// HDLC optional functions (REJ or SREJ, modulo 8 or 128 ...).
    pub const OPTIONAL_FUNCTIONS: u8 = 3;
    /// I field length, transmit (bits).
    pub const I_LENGTH_TX: u8 = 5;
    /// I field length, receive (bits).
    pub const I_LENGTH_RX: u8 = 6;
    /// Window size, transmit (frames).
    pub const WINDOW_TX: u8 = 7;
    /// Window size, receive (frames).
    pub const WINDOW_RX: u8 = 8;
    /// Acknowledge timer T1 (ms).
    pub const ACK_TIMER: u8 = 9;
    /// Retries N2.
    pub const RETRIES: u8 = 10;
}

impl<'a> Xid<'a> {
    /// Reads the header of an XID information field.
    pub fn parse(info: &'a [u8]) -> Result<Xid<'a>> {
        if info.len() < 4 {
            return Err(Error::Short);
        }
        if info[0] != 0x82 || info[1] != 0x80 {
            return Err(Error::Syntax(Part::Xid));
        }
        let len = u16::from_be_bytes([info[2], info[3]]) as usize;
        let params = info.get(4..4 + len).ok_or(Error::Short)?;
        Ok(Xid { params })
    }

    /// The parameters as (identifier, value bytes). A parameter whose length runs past the group ends the iteration.
    pub fn params(&self) -> impl Iterator<Item = (u8, &'a [u8])> + 'a {
        let mut rest = self.params;
        std::iter::from_fn(move || {
            let (&pi, r) = rest.split_first()?;
            let (&pl, r) = r.split_first()?;
            let v = r.get(..pl as usize)?;
            rest = &r[pl as usize..];
            Some((pi, v))
        })
    }

    /// The value of parameter `pi` as an unsigned big-endian number (values up to 8 bytes).
    pub fn value(&self, pi: u8) -> Option<u64> {
        self.params()
            .find(|(p, v)| *p == pi && v.len() <= 8)
            .map(|(_, v)| v.iter().fold(0u64, |a, &b| (a << 8) | b as u64))
    }
}

/// The information field of a FRMR frame (clause 4.3.3.9, figures 4.6 and 4.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frmr {
    /// The rejected frame's control field (in modulo 8 the one byte, in modulo 128 both bytes with the first in the low byte).
    pub rejected_control: u16,
    /// V(S) of the station sending FRMR.
    pub vs: u8,
    /// V(R) of the station sending FRMR.
    pub vr: u8,
    /// The rejected frame was a response.
    pub response: bool,
    /// W: the control field was invalid or not implemented.
    pub w: bool,
    /// X: the frame had an information field that its type does not allow.
    pub x: bool,
    /// Y: the information field was too long.
    pub y: bool,
    /// Z: N(R) was invalid.
    pub z: bool,
}

impl Frmr {
    /// Reads FRMR information: 3 bytes in modulo 8, 5 in modulo 128.
    pub fn parse(info: &[u8], modulo: Modulo) -> Result<Frmr> {
        match modulo {
            Modulo::Eight => {
                let b = info.get(..3).ok_or(Error::Short)?;
                Ok(Frmr {
                    rejected_control: b[0] as u16,
                    vs: (b[1] >> 1) & 7,
                    response: b[1] & 0x10 != 0,
                    vr: b[1] >> 5,
                    w: b[2] & 1 != 0,
                    x: b[2] & 2 != 0,
                    y: b[2] & 4 != 0,
                    z: b[2] & 8 != 0,
                })
            }
            Modulo::OneTwentyEight => {
                let b = info.get(..5).ok_or(Error::Short)?;
                Ok(Frmr {
                    rejected_control: u16::from_le_bytes([b[0], b[1]]),
                    vs: b[2] >> 1,
                    vr: b[3] >> 1,
                    response: b[3] & 1 != 0,
                    w: b[4] & 1 != 0,
                    x: b[4] & 2 != 0,
                    y: b[4] & 4 != 0,
                    z: b[4] & 8 != 0,
                })
            }
        }
    }
}
