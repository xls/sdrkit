//! IEEE 802.11 MAC framing (IEEE Std 802.11-2020 clause 9): the frame control field, the MAC header of management, control and
//! data frames, the FCS, the fixed fields of beacons and probe responses, the element walk, the common elements and builders.
//!
//! - FCS: CRC-32/ISO-HDLC (clause 9.2.4.8), sent least significant byte first (`sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC`).
//! - Elements (clause 9.4.2): an element ID, a length and the body. ID 255 is an extension: its first body byte is the element ID
//!   extension.
//! - The meanings that a display shows (security summaries, HT and VHT capability text, channel widths) are not here. This module
//!   returns the bytes and the fields the standard defines.
//!
//! ```
//! use sdrkit_proto_wlan_phy::mac::{self, ElementValue, Frame, MgmtSubtype};
//! let mut f = Vec::new();
//! mac::push_mgmt_header(&mut f, MgmtSubtype::Beacon, mac::BROADCAST, [2, 0, 0, 0, 0, 1], [2, 0, 0, 0, 0, 1], 7);
//! mac::push_beacon_fields(&mut f, 0, 100, 0x0401);
//! mac::push_element(&mut f, mac::element::SSID, b"home").unwrap();
//! mac::push_element(&mut f, mac::element::DS_PARAMETER_SET, &[6]).unwrap();
//! mac::push_fcs(&mut f);
//! let frame = Frame::parse_with_fcs(&f).unwrap();
//! let b = frame.beacon().unwrap();
//! assert_eq!(b.interval_tu, 100);
//! let v: Vec<ElementValue> = mac::elements(b.elements).map(|e| e.value()).collect();
//! assert_eq!(v, [ElementValue::Ssid(b"home"), ElementValue::DsChannel(6)]);
//! ```

use crate::{CheckKind, Error, Malformation, Result};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use sdrkit_util_bits::crc::Trailer;

/// The broadcast address.
pub const BROADCAST: [u8; 6] = [0xFF; 6];

/// The CRC-32 of the frame, as the FCS computes it.
pub fn crc32(data: &[u8]) -> u32 {
    CRC32_ISO_HDLC.checksum(data)
}

/// Whether the last four bytes of `frame` are its FCS (least significant byte first).
pub fn fcs_ok(frame: &[u8]) -> bool {
    frame.len() >= 4 && CRC32_ISO_HDLC.verify(frame, Trailer::Little)
}

/// Appends the FCS.
pub fn push_fcs(frame: &mut Vec<u8>) {
    frame.extend_from_slice(&crc32(frame).to_le_bytes());
}

/// The frame type (clause 9.2.4.1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameType {
    /// Management (0).
    Management,
    /// Control (1).
    Control,
    /// Data (2).
    Data,
    /// Extension (3).
    Extension,
}

/// The management subtypes (table 9-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MgmtSubtype {
    /// Association request (0).
    AssocRequest,
    /// Association response (1).
    AssocResponse,
    /// Reassociation request (2).
    ReassocRequest,
    /// Reassociation response (3).
    ReassocResponse,
    /// Probe request (4).
    ProbeRequest,
    /// Probe response (5).
    ProbeResponse,
    /// Timing advertisement (6).
    TimingAdvertisement,
    /// Beacon (8).
    Beacon,
    /// ATIM (9).
    Atim,
    /// Disassociation (10).
    Disassociation,
    /// Authentication (11).
    Authentication,
    /// Deauthentication (12).
    Deauthentication,
    /// Action (13).
    Action,
    /// Action No Ack (14).
    ActionNoAck,
    /// A reserved subtype (7, 15).
    Reserved(u8),
}

impl MgmtSubtype {
    /// The subtype of a 4-bit code.
    pub fn from_code(c: u8) -> MgmtSubtype {
        match c & 15 {
            0 => MgmtSubtype::AssocRequest,
            1 => MgmtSubtype::AssocResponse,
            2 => MgmtSubtype::ReassocRequest,
            3 => MgmtSubtype::ReassocResponse,
            4 => MgmtSubtype::ProbeRequest,
            5 => MgmtSubtype::ProbeResponse,
            6 => MgmtSubtype::TimingAdvertisement,
            8 => MgmtSubtype::Beacon,
            9 => MgmtSubtype::Atim,
            10 => MgmtSubtype::Disassociation,
            11 => MgmtSubtype::Authentication,
            12 => MgmtSubtype::Deauthentication,
            13 => MgmtSubtype::Action,
            14 => MgmtSubtype::ActionNoAck,
            c => MgmtSubtype::Reserved(c),
        }
    }

    /// The 4-bit code.
    pub fn code(self) -> u8 {
        match self {
            MgmtSubtype::AssocRequest => 0,
            MgmtSubtype::AssocResponse => 1,
            MgmtSubtype::ReassocRequest => 2,
            MgmtSubtype::ReassocResponse => 3,
            MgmtSubtype::ProbeRequest => 4,
            MgmtSubtype::ProbeResponse => 5,
            MgmtSubtype::TimingAdvertisement => 6,
            MgmtSubtype::Beacon => 8,
            MgmtSubtype::Atim => 9,
            MgmtSubtype::Disassociation => 10,
            MgmtSubtype::Authentication => 11,
            MgmtSubtype::Deauthentication => 12,
            MgmtSubtype::Action => 13,
            MgmtSubtype::ActionNoAck => 14,
            MgmtSubtype::Reserved(c) => c & 15,
        }
    }

    /// The name in table 9-1.
    pub fn name(self) -> &'static str {
        match self {
            MgmtSubtype::AssocRequest => "Association Request",
            MgmtSubtype::AssocResponse => "Association Response",
            MgmtSubtype::ReassocRequest => "Reassociation Request",
            MgmtSubtype::ReassocResponse => "Reassociation Response",
            MgmtSubtype::ProbeRequest => "Probe Request",
            MgmtSubtype::ProbeResponse => "Probe Response",
            MgmtSubtype::TimingAdvertisement => "Timing Advertisement",
            MgmtSubtype::Beacon => "Beacon",
            MgmtSubtype::Atim => "ATIM",
            MgmtSubtype::Disassociation => "Disassociation",
            MgmtSubtype::Authentication => "Authentication",
            MgmtSubtype::Deauthentication => "Deauthentication",
            MgmtSubtype::Action => "Action",
            MgmtSubtype::ActionNoAck => "Action No Ack",
            MgmtSubtype::Reserved(_) => "Reserved",
        }
    }
}

/// The frame control field (clause 9.2.4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameControl {
    /// Protocol version (0).
    pub version: u8,
    /// Type.
    pub ftype: FrameType,
    /// Subtype (4 bits).
    pub subtype: u8,
    /// To DS.
    pub to_ds: bool,
    /// From DS.
    pub from_ds: bool,
    /// More Fragments.
    pub more_fragments: bool,
    /// Retry.
    pub retry: bool,
    /// Power Management.
    pub power_mgmt: bool,
    /// More Data.
    pub more_data: bool,
    /// Protected Frame.
    pub protected: bool,
    /// +HTC (Order).
    pub order: bool,
}

impl FrameControl {
    /// Reads the two bytes (least significant first).
    pub fn from_bytes(b: [u8; 2]) -> FrameControl {
        let f = b[1];
        FrameControl {
            version: b[0] & 3,
            ftype: match (b[0] >> 2) & 3 {
                0 => FrameType::Management,
                1 => FrameType::Control,
                2 => FrameType::Data,
                _ => FrameType::Extension,
            },
            subtype: b[0] >> 4,
            to_ds: f & 0x01 != 0,
            from_ds: f & 0x02 != 0,
            more_fragments: f & 0x04 != 0,
            retry: f & 0x08 != 0,
            power_mgmt: f & 0x10 != 0,
            more_data: f & 0x20 != 0,
            protected: f & 0x40 != 0,
            order: f & 0x80 != 0,
        }
    }

    /// The two bytes.
    pub fn bytes(&self) -> [u8; 2] {
        let t = match self.ftype {
            FrameType::Management => 0,
            FrameType::Control => 1,
            FrameType::Data => 2,
            FrameType::Extension => 3,
        };
        let flags = [
            self.to_ds,
            self.from_ds,
            self.more_fragments,
            self.retry,
            self.power_mgmt,
            self.more_data,
            self.protected,
            self.order,
        ]
        .iter()
        .enumerate()
        .fold(0u8, |a, (i, &v)| a | (v as u8) << i);
        [
            (self.version & 3) | t << 2 | (self.subtype & 15) << 4,
            flags,
        ]
    }

    /// The management subtype of a management frame.
    pub fn mgmt_subtype(&self) -> Option<MgmtSubtype> {
        (self.ftype == FrameType::Management).then(|| MgmtSubtype::from_code(self.subtype))
    }
}

/// A parsed MAC frame (FCS removed), borrowing its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Frame control.
    pub fc: FrameControl,
    /// Duration/ID.
    pub duration: u16,
    /// Address 1 (the receiver).
    pub addr1: [u8; 6],
    /// Address 2 (the transmitter), when the frame has one.
    pub addr2: Option<[u8; 6]>,
    /// Address 3, when the frame has one.
    pub addr3: Option<[u8; 6]>,
    /// Sequence control: (sequence number, fragment number), when the frame has one.
    pub seq: Option<(u16, u8)>,
    /// Address 4 (data frames with To DS and From DS).
    pub addr4: Option<[u8; 6]>,
    /// QoS Control (QoS data frames).
    pub qos: Option<u16>,
    /// HT Control, when +HTC is set on a management or QoS data frame.
    pub ht_control: Option<u32>,
    /// The frame body.
    pub body: &'a [u8],
}

fn addr(b: &[u8], at: usize) -> Result<[u8; 6]> {
    let s = b.get(at..at + 6).ok_or(Error::InputTooShort)?;
    let mut a = [0u8; 6];
    a.copy_from_slice(s);
    Ok(a)
}

fn u16le(b: &[u8], at: usize) -> Result<u16> {
    let s = b.get(at..at + 2).ok_or(Error::InputTooShort)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

impl<'a> Frame<'a> {
    /// Parses a frame without its FCS (clause 9.2.3, 9.3). Errors for a protocol version other than 0 or a header longer than
    /// the frame.
    pub fn parse(f: &'a [u8]) -> Result<Frame<'a>> {
        if f.len() < 10 {
            return Err(Error::InputTooShort);
        }
        let fc = FrameControl::from_bytes([f[0], f[1]]);
        if fc.version != 0 {
            return Err(Error::Malformed(Malformation::ProtocolVersion));
        }
        let mut fr = Frame {
            fc,
            duration: u16le(f, 2)?,
            addr1: addr(f, 4)?,
            addr2: None,
            addr3: None,
            seq: None,
            addr4: None,
            qos: None,
            ht_control: None,
            body: &[],
        };
        let mut at = 10;
        match fc.ftype {
            FrameType::Control => {
                // CTS (12) and Ack (13) carry only the receiver address. The others carry a transmitter address too.
                if !matches!(fc.subtype, 12 | 13) {
                    fr.addr2 = Some(addr(f, 10)?);
                    at = 16;
                }
            }
            FrameType::Management | FrameType::Data => {
                fr.addr2 = Some(addr(f, 10)?);
                fr.addr3 = Some(addr(f, 16)?);
                let sc = u16le(f, 22)?;
                fr.seq = Some((sc >> 4, (sc & 15) as u8));
                at = 24;
                if fc.ftype == FrameType::Data {
                    if fc.to_ds && fc.from_ds {
                        fr.addr4 = Some(addr(f, 24)?);
                        at = 30;
                    }
                    if fc.subtype & 8 != 0 {
                        fr.qos = Some(u16le(f, at)?);
                        at += 2;
                    }
                }
                let htc = fc.order && (fc.ftype == FrameType::Management || fr.qos.is_some());
                if htc {
                    let s = f.get(at..at + 4).ok_or(Error::InputTooShort)?;
                    fr.ht_control = Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
                    at += 4;
                }
            }
            FrameType::Extension => {}
        }
        fr.body = f.get(at..).ok_or(Error::InputTooShort)?;
        Ok(fr)
    }

    /// Parses a frame that ends with its FCS. Errors for a bad FCS.
    pub fn parse_with_fcs(f: &'a [u8]) -> Result<Frame<'a>> {
        if !fcs_ok(f) {
            return Err(Error::Check(CheckKind::Fcs));
        }
        Frame::parse(&f[..f.len() - 4])
    }

    /// The BSSID: address 3 of a management frame, and of a data frame by its DS bits (table 9-30). `None` between two
    /// distribution systems and for control frames.
    pub fn bssid(&self) -> Option<[u8; 6]> {
        match self.fc.ftype {
            FrameType::Management => self.addr3,
            FrameType::Data => match (self.fc.to_ds, self.fc.from_ds) {
                (false, false) => self.addr3,
                (true, false) => Some(self.addr1),
                (false, true) => self.addr2,
                (true, true) => None,
            },
            _ => None,
        }
    }

    /// The fixed fields and elements of a beacon or probe response.
    pub fn beacon(&self) -> Option<Beacon<'a>> {
        match self.fc.mgmt_subtype()? {
            MgmtSubtype::Beacon | MgmtSubtype::ProbeResponse => Beacon::parse(self.body).ok(),
            _ => None,
        }
    }

    /// The elements of a probe request.
    pub fn probe_request(&self) -> Option<&'a [u8]> {
        (self.fc.mgmt_subtype()? == MgmtSubtype::ProbeRequest).then_some(self.body)
    }
}

/// The body of a beacon or probe response (clause 9.3.3.2, 9.3.3.10): fixed fields, then elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beacon<'a> {
    /// Timestamp, µs.
    pub timestamp: u64,
    /// Beacon interval, time units of 1024 µs.
    pub interval_tu: u16,
    /// Capability information (clause 9.4.1.4).
    pub capability: u16,
    /// The elements.
    pub elements: &'a [u8],
}

/// Capability information bit: ESS.
pub const CAP_ESS: u16 = 0x0001;
/// Capability information bit: IBSS.
pub const CAP_IBSS: u16 = 0x0002;
/// Capability information bit: Privacy.
pub const CAP_PRIVACY: u16 = 0x0010;

impl<'a> Beacon<'a> {
    /// Reads the fixed fields. Errors for a body shorter than 12 bytes.
    pub fn parse(body: &'a [u8]) -> Result<Beacon<'a>> {
        if body.len() < 12 {
            return Err(Error::InputTooShort);
        }
        let mut ts = [0u8; 8];
        ts.copy_from_slice(&body[..8]);
        Ok(Beacon {
            timestamp: u64::from_le_bytes(ts),
            interval_tu: u16::from_le_bytes([body[8], body[9]]),
            capability: u16::from_le_bytes([body[10], body[11]]),
            elements: &body[12..],
        })
    }
}

/// Element IDs (table 9-92).
pub mod element {
    /// SSID.
    pub const SSID: u8 = 0;
    /// Supported Rates and BSS Membership Selectors.
    pub const SUPPORTED_RATES: u8 = 1;
    /// DSSS Parameter Set (the current channel).
    pub const DS_PARAMETER_SET: u8 = 3;
    /// TIM.
    pub const TIM: u8 = 5;
    /// Country.
    pub const COUNTRY: u8 = 7;
    /// HT Capabilities.
    pub const HT_CAPABILITIES: u8 = 45;
    /// RSN.
    pub const RSN: u8 = 48;
    /// Extended Supported Rates and BSS Membership Selectors.
    pub const EXTENDED_RATES: u8 = 50;
    /// HT Operation.
    pub const HT_OPERATION: u8 = 61;
    /// VHT Capabilities.
    pub const VHT_CAPABILITIES: u8 = 191;
    /// VHT Operation.
    pub const VHT_OPERATION: u8 = 192;
    /// S1G Capabilities.
    pub const S1G_CAPABILITIES: u8 = 217;
    /// Vendor Specific.
    pub const VENDOR_SPECIFIC: u8 = 221;
    /// S1G Operation.
    pub const S1G_OPERATION: u8 = 232;
    /// Element ID Extension: the first body byte is the extension ID.
    pub const EXTENSION: u8 = 255;
}

/// One element: its ID, the extension ID for ID 255, and the body (after the extension ID).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element<'a> {
    /// Element ID.
    pub id: u8,
    /// Element ID extension (ID 255 only).
    pub ext: Option<u8>,
    /// The body.
    pub data: &'a [u8],
}

/// A cipher or AKM suite: OUI and type (clause 9.4.2.24.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Suite {
    /// The OUI (00-0F-AC for the suites of the standard).
    pub oui: [u8; 3],
    /// The suite type.
    pub kind: u8,
}

/// The OUI of the suites the standard defines.
pub const IEEE_OUI: [u8; 3] = [0x00, 0x0F, 0xAC];

/// What a common element says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementValue<'a> {
    /// SSID bytes, 0 to 32. It is empty for a hidden network or a wildcard probe.
    Ssid(&'a [u8]),
    /// Supported or extended rates: each byte is a rate in 500 kbit/s with bit 7 meaning basic, or a membership selector.
    Rates(&'a [u8]),
    /// The current channel of the DSSS Parameter Set.
    DsChannel(u8),
    /// An RSN element body ([`Rsn::parse`] reads it).
    Rsn(&'a [u8]),
    /// A vendor specific element.
    Vendor {
        /// The OUI (or the first three bytes of a longer organisation identifier).
        oui: [u8; 3],
        /// The byte after the OUI (the vendor's type), when present.
        kind: Option<u8>,
        /// The bytes after the type.
        data: &'a [u8],
    },
    /// Any other element, or a known element whose length is wrong.
    Other(Element<'a>),
}

impl<'a> Element<'a> {
    /// Reads the element as one of the common types.
    pub fn value(&self) -> ElementValue<'a> {
        let d = self.data;
        match (self.id, self.ext) {
            (element::SSID, _) if d.len() <= 32 => ElementValue::Ssid(d),
            (element::SUPPORTED_RATES | element::EXTENDED_RATES, _) => ElementValue::Rates(d),
            (element::DS_PARAMETER_SET, _) if d.len() == 1 => ElementValue::DsChannel(d[0]),
            (element::RSN, _) => ElementValue::Rsn(d),
            (element::VENDOR_SPECIFIC, _) if d.len() >= 3 => ElementValue::Vendor {
                oui: [d[0], d[1], d[2]],
                kind: d.get(3).copied(),
                data: d.get(4..).unwrap_or(&[]),
            },
            _ => ElementValue::Other(*self),
        }
    }

    /// The vendor of a vendor specific element's OUI (`sdrkit_util_ids::oui`).
    pub fn vendor_name(&self) -> Option<&'static str> {
        match self.value() {
            ElementValue::Vendor { oui, .. } => sdrkit_util_ids::oui::vendor(oui),
            _ => None,
        }
    }
}

/// The elements of a frame body, in order (see [`elements`]).
#[derive(Clone, Debug)]
pub struct Elements<'a> {
    rest: &'a [u8],
    truncated: bool,
}

impl<'a> Elements<'a> {
    /// Whether the walk stopped at an element running past the end of the body (seen once the iterator has ended).
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = Element<'a>;

    fn next(&mut self) -> Option<Element<'a>> {
        if self.rest.len() < 2 {
            self.truncated |= !self.rest.is_empty();
            self.rest = &[];
            return None;
        }
        let id = self.rest[0];
        let len = self.rest[1] as usize;
        if 2 + len > self.rest.len() {
            self.truncated = true;
            self.rest = &[];
            return None;
        }
        let body = &self.rest[2..2 + len];
        self.rest = &self.rest[2 + len..];
        if id == element::EXTENSION {
            if let Some((&ext, data)) = body.split_first() {
                return Some(Element {
                    id,
                    ext: Some(ext),
                    data,
                });
            }
        }
        Some(Element {
            id,
            ext: None,
            data: body,
        })
    }
}

/// Walks the elements of `body`. A truncated element ends the walk and is never read past the end.
pub fn elements(body: &[u8]) -> Elements<'_> {
    Elements {
        rest: body,
        truncated: false,
    }
}

/// The first element with ID `id`.
pub fn find(body: &[u8], id: u8) -> Option<Element<'_>> {
    elements(body).find(|e| e.id == id)
}

/// Every vendor specific element with this OUI and type: the bytes after the type.
pub fn vendor_elements(body: &[u8], oui: [u8; 3], kind: u8) -> impl Iterator<Item = &[u8]> {
    elements(body).filter_map(move |e| match e.value() {
        ElementValue::Vendor {
            oui: o,
            kind: Some(k),
            data,
        } if o == oui && k == kind => Some(data),
        _ => None,
    })
}

/// The fields of an RSN element (clause 9.4.2.24). Lists are read on demand from the borrowed body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsn<'a> {
    /// Version (1).
    pub version: u16,
    /// The group data cipher suite.
    pub group: Option<Suite>,
    /// The pairwise cipher suites (4 bytes each).
    pub pairwise: &'a [u8],
    /// The AKM suites (4 bytes each).
    pub akm: &'a [u8],
    /// RSN capabilities.
    pub capabilities: Option<u16>,
}

fn suites(b: &[u8]) -> impl Iterator<Item = Suite> + '_ {
    b.as_chunks::<4>().0.iter().map(|c| Suite {
        oui: [c[0], c[1], c[2]],
        kind: c[3],
    })
}

impl<'a> Rsn<'a> {
    /// Reads an RSN element body. Fields after the version are optional and end where the body ends. Errors for a body shorter
    /// than the version or a suite count running past the end.
    pub fn parse(b: &'a [u8]) -> Result<Rsn<'a>> {
        let version = u16le(b, 0)?;
        let mut r = Rsn {
            version,
            group: None,
            pairwise: &[],
            akm: &[],
            capabilities: None,
        };
        let mut at = 2;
        if b.len() >= at + 4 {
            r.group = suites(&b[at..at + 4]).next();
            at += 4;
        }
        let list = |at: &mut usize| -> Result<&'a [u8]> {
            if b.len() < *at + 2 {
                return Ok(&[]);
            }
            let n = u16le(b, *at)? as usize;
            let s = b
                .get(*at + 2..*at + 2 + 4 * n)
                .ok_or(Error::Malformed(Malformation::RsnSuiteCount))?;
            *at += 2 + 4 * n;
            Ok(s)
        };
        r.pairwise = list(&mut at)?;
        r.akm = list(&mut at)?;
        if b.len() >= at + 2 {
            r.capabilities = Some(u16le(b, at)?);
        }
        Ok(r)
    }

    /// The pairwise cipher suites.
    pub fn pairwise_suites(&self) -> impl Iterator<Item = Suite> + 'a {
        suites(self.pairwise)
    }

    /// The AKM suites.
    pub fn akm_suites(&self) -> impl Iterator<Item = Suite> + 'a {
        suites(self.akm)
    }
}

/// Appends a management frame header (clause 9.3.3.1): frame control, duration 0, DA, SA, BSSID and the sequence number.
pub fn push_mgmt_header(
    out: &mut Vec<u8>,
    subtype: MgmtSubtype,
    da: [u8; 6],
    sa: [u8; 6],
    bssid: [u8; 6],
    seq: u16,
) {
    let fc = FrameControl {
        version: 0,
        ftype: FrameType::Management,
        subtype: subtype.code(),
        to_ds: false,
        from_ds: false,
        more_fragments: false,
        retry: false,
        power_mgmt: false,
        more_data: false,
        protected: false,
        order: false,
    };
    out.extend_from_slice(&fc.bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&da);
    out.extend_from_slice(&sa);
    out.extend_from_slice(&bssid);
    out.extend_from_slice(&((seq & 0x0FFF) << 4).to_le_bytes());
}

/// Appends the fixed fields of a beacon or probe response: timestamp, beacon interval and capability information.
pub fn push_beacon_fields(out: &mut Vec<u8>, timestamp: u64, interval_tu: u16, capability: u16) {
    out.extend_from_slice(&timestamp.to_le_bytes());
    out.extend_from_slice(&interval_tu.to_le_bytes());
    out.extend_from_slice(&capability.to_le_bytes());
}

/// Appends an element. Errors for a body over 255 bytes.
pub fn push_element(out: &mut Vec<u8>, id: u8, data: &[u8]) -> Result<()> {
    if data.len() > 255 {
        return Err(Error::TooLong);
    }
    out.push(id);
    out.push(data.len() as u8);
    out.extend_from_slice(data);
    Ok(())
}

/// Appends a vendor specific element: OUI, type and data. Errors for a body over 255 bytes.
pub fn push_vendor(out: &mut Vec<u8>, oui: [u8; 3], kind: u8, data: &[u8]) -> Result<()> {
    if data.len() > 251 {
        return Err(Error::TooLong);
    }
    out.push(element::VENDOR_SPECIFIC);
    out.push(4 + data.len() as u8);
    out.extend_from_slice(&oui);
    out.push(kind);
    out.extend_from_slice(data);
    Ok(())
}

/// A rate byte in 500 kbit/s units as a rate in kbit/s, with whether it is basic. `None` for a BSS membership selector (clause
/// 9.4.2.3: 127 HT PHY, 126 VHT PHY, 123 SAE hash-to-element, 122 HE PHY, 121 EHT PHY).
pub fn rate_kbps(b: u8) -> Option<(u32, bool)> {
    let v = b & 0x7F;
    if b & 0x80 != 0 && matches!(v, 121..=127) {
        return None;
    }
    Some((500 * v as u32, b & 0x80 != 0))
}
