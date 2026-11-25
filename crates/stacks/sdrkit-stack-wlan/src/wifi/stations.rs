//! The client stations of the `wifi` stack: who sent or was sent a frame, which access point each belongs to, and what each
//! asked for in its probe requests. Only frame headers and unprotected management bodies are read.

use sdrkit_proto_wlan_phy::mac::{self as phy, control, Frame, FrameType, MgmtBody, MgmtSubtype};
use std::collections::HashMap;

/// Most stations kept. The station heard longest ago makes room for a new one.
pub const STATION_CAP: usize = 1000;
/// Most probed SSIDs kept for a station.
pub const SSID_CAP: usize = 16;
/// Two frames with the same bytes within this time are one frame heard on two channels of a wide capture, s.
const SAME_FRAME_S: f64 = 200e-6;

/// What the last frames say about a station and its access point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Link {
    /// Heard, with no frame that ties it to an access point.
    #[default]
    Unknown,
    /// Sent probe requests only.
    Probing,
    /// Authentication or an association request was seen.
    Joining,
    /// A successful association response or a data frame to or from the access point was seen.
    Associated,
    /// A deauthentication or disassociation was seen last.
    Left,
}

impl Link {
    /// A short name.
    pub fn name(self) -> &'static str {
        match self {
            Link::Unknown => "unknown",
            Link::Probing => "probing",
            Link::Joining => "joining",
            Link::Associated => "associated",
            Link::Left => "left",
        }
    }
}

/// One client station.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    /// The address.
    pub addr: [u8; 6],
    /// The vendor of the address block; empty for a randomised address or an unknown block.
    pub vendor: &'static str,
    /// The address is locally administered: the station made it up (address randomisation).
    pub randomised: bool,
    /// The BSSID of the access point the last frames tie the station to.
    pub bssid: Option<[u8; 6]>,
    /// What the last frames say.
    pub link: Link,
    /// Signal time of the first frame to or from the station, s.
    pub first_s: f64,
    /// Signal time of the last frame the station sent, s. `None`: only frames sent to it were heard.
    pub last_heard_s: Option<f64>,
    /// The level of the last frame the station sent, dBFS.
    pub level_dbfs: Option<f64>,
    /// Frames the station sent.
    pub sent: u64,
    /// Frames sent to the station.
    pub received: u64,
    /// Probe requests the station sent.
    pub probes: u64,
    /// Data frames to or from the station (null frames not counted).
    pub data: u64,
    /// Null data frames the station sent (power-save signalling).
    pub null_data: u64,
    /// Frames to or from the station with the Protected Frame bit: counted, never decrypted.
    pub protected: u64,
    /// The SSIDs the station asked for by name in probe requests, oldest first.
    pub ssids: Vec<String>,
    /// The newest PHY generation its probe requests declare: 4 HT, 5 VHT, 6 HE, 7 EHT; 0 when not known.
    pub generation: u8,
    /// The highest rate its probe requests list, Mbit/s.
    pub max_rate_mbps: Option<f64>,
    /// The channel the last frame from the station was heard on (0: not known).
    pub channel: u32,
    /// The reason code of the last deauthentication or disassociation.
    pub last_reason: Option<u16>,
    /// The station is in power save (the Power Management bit of its last frame).
    pub power_save: bool,
}

/// What one frame says, for [`Stations::take`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Heard {
    /// Signal time, s.
    pub time_s: f64,
    /// The level of the frame, dBFS.
    pub level_dbfs: f64,
    /// The channel (0: not known).
    pub channel: u32,
}

/// The counts of frames by kind since the last clear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounts {
    /// Probe requests.
    pub probe_requests: u64,
    /// Authentication frames.
    pub authentications: u64,
    /// Association and reassociation requests and responses.
    pub associations: u64,
    /// Deauthentication and disassociation frames.
    pub departures: u64,
    /// Action frames.
    pub actions: u64,
    /// RTS frames.
    pub rts: u64,
    /// CTS frames.
    pub cts: u64,
    /// Ack frames.
    pub acks: u64,
    /// BlockAck and BlockAckReq frames.
    pub block_acks: u64,
    /// Other control frames.
    pub other_control: u64,
    /// Data frames with a body.
    pub data: u64,
    /// Null data frames.
    pub null_data: u64,
    /// Frames with the Protected Frame bit.
    pub protected: u64,
    /// Frames heard a second time on another channel of a wide capture.
    pub repeats: u64,
}

/// The stations table.
#[derive(Clone, Debug, Default)]
pub struct Stations {
    map: HashMap<[u8; 6], Station>,
    /// The access points known from their own frames.
    aps: HashMap<[u8; 6], ()>,
    /// The counts.
    pub counts: FrameCounts,
    recent: [(f64, u32, usize); 8],
    at: usize,
}

/// The CRC-32 of a frame, as the mark of its content.
fn mark(bytes: &[u8]) -> u32 {
    phy::crc32(bytes)
}

fn vendor(a: &[u8; 6]) -> &'static str {
    if phy::is_local(a) {
        ""
    } else {
        sdrkit_util_ids::oui::vendor_of_address(a).unwrap_or("")
    }
}

impl Stations {
    /// The stations, in no order.
    pub fn iter(&self) -> impl Iterator<Item = &Station> {
        self.map.values()
    }

    /// The station with this address.
    pub fn get(&self, addr: &[u8; 6]) -> Option<&Station> {
        self.map.get(addr)
    }

    /// Stations kept.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// No station is kept.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Stations tied to the access point `bssid`.
    pub fn of_bss(&self, bssid: &[u8; 6]) -> usize {
        self.map
            .values()
            .filter(|s| s.link == Link::Associated && s.bssid.as_ref() == Some(bssid))
            .count()
    }

    /// Forgets every station and count.
    pub fn clear(&mut self) {
        *self = Stations::default();
    }

    /// Notes that `bssid` is an access point (a beacon or probe response came from it). It stops being a station.
    pub fn access_point(&mut self, bssid: [u8; 6]) {
        if self.aps.len() < 4 * STATION_CAP {
            self.aps.insert(bssid, ());
        }
        self.map.remove(&bssid);
    }

    /// The station `a`, made when new. `None` for a group address and for an access point.
    fn station(&mut self, a: [u8; 6], now: f64) -> Option<&mut Station> {
        if phy::is_group(&a) || self.aps.contains_key(&a) {
            return None;
        }
        if !self.map.contains_key(&a) && self.map.len() >= STATION_CAP {
            let oldest = self
                .map
                .values()
                .min_by(|x, y| {
                    let k = |s: &Station| s.last_heard_s.unwrap_or(s.first_s);
                    k(x).total_cmp(&k(y))
                })
                .map(|s| s.addr);
            if let Some(o) = oldest {
                self.map.remove(&o);
            }
        }
        Some(self.map.entry(a).or_insert_with(|| Station {
            addr: a,
            vendor: vendor(&a),
            randomised: phy::is_local(&a),
            first_s: now,
            ..Station::default()
        }))
    }

    /// Whether these bytes were taken within the last 200 µs (the same frame on another channel); remembers them.
    fn repeated(&mut self, bytes: &[u8], now: f64) -> bool {
        let m = (now, mark(bytes), bytes.len());
        let hit = self
            .recent
            .iter()
            .any(|r| r.1 == m.1 && r.2 == m.2 && r.2 != 0 && (r.0 - now).abs() <= SAME_FRAME_S);
        if !hit {
            self.recent[self.at] = m;
            self.at = (self.at + 1) % self.recent.len();
        }
        hit
    }

    /// Takes a frame with a good FCS (`bytes` without the FCS).
    pub fn take(&mut self, bytes: &[u8], heard: Heard) {
        let Ok(f) = Frame::parse(bytes) else {
            return;
        };
        if self.repeated(bytes, heard.time_s) {
            self.counts.repeats += 1;
            return;
        }
        let p = f.parties();
        if f.fc.protected {
            self.counts.protected += 1;
        }
        match f.fc.ftype {
            FrameType::Management => self.management(&f, &p, heard),
            FrameType::Data => self.data(&f, &p, heard),
            FrameType::Control => self.control(&f, &p, heard),
            FrameType::Extension => {}
        }
        // The side that is the access point of the frame is not a station.
        if let (true, Some(b)) = (p.from_ap, p.bssid) {
            if f.fc.ftype == FrameType::Data || matches!(f.fc.mgmt_subtype(), Some(MgmtSubtype::Beacon | MgmtSubtype::ProbeResponse)) {
                self.access_point(b);
            }
        }
    }

    /// Updates the station that sent a frame: last heard, level, channel, power save.
    fn sender(&mut self, f: &Frame<'_>, a: [u8; 6], h: Heard) -> Option<&mut Station> {
        let (ps, prot) = (f.fc.power_mgmt, f.fc.protected);
        let s = self.station(a, h.time_s)?;
        s.sent += 1;
        s.last_heard_s = Some(h.time_s);
        s.level_dbfs = Some(h.level_dbfs);
        s.channel = h.channel;
        s.power_save = ps;
        s.protected += u64::from(prot);
        Some(s)
    }

    /// Updates the station a frame is for.
    fn target(&mut self, f: &Frame<'_>, a: [u8; 6], h: Heard) -> Option<&mut Station> {
        let prot = f.fc.protected;
        let s = self.station(a, h.time_s)?;
        s.received += 1;
        s.protected += u64::from(prot);
        Some(s)
    }

    fn management(&mut self, f: &Frame<'_>, p: &phy::Parties, h: Heard) {
        let Some(st) = f.fc.mgmt_subtype() else {
            return;
        };
        let Some(ta) = p.transmitter else {
            return;
        };
        let bssid = p.bssid.filter(|b| !phy::is_group(b));
        let body = f.mgmt();
        match st {
            MgmtSubtype::Beacon | MgmtSubtype::ProbeResponse | MgmtSubtype::TimingAdvertisement => {
                // Sent by an access point (or an IBSS member); a probe response is for a station.
                if st == MgmtSubtype::ProbeResponse && p.from_ap {
                    self.target(f, p.receiver, h);
                }
            }
            MgmtSubtype::ProbeRequest => {
                self.counts.probe_requests += 1;
                let Some(s) = self.sender(f, ta, h) else {
                    return;
                };
                s.probes += 1;
                if s.link == Link::Unknown {
                    s.link = Link::Probing;
                }
                if let Some(MgmtBody::ProbeRequest(r)) = body {
                    s.generation = s.generation.max(r.generation());
                    if let Some(k) = r.max_rate_kbps() {
                        let m = f64::from(k) / 1000.0;
                        s.max_rate_mbps = Some(s.max_rate_mbps.map_or(m, |x| x.max(m)));
                    }
                    if let Some(name) = r.ssid() {
                        let name = String::from_utf8_lossy(name).into_owned();
                        if !s.ssids.contains(&name) {
                            if s.ssids.len() >= SSID_CAP {
                                s.ssids.remove(0);
                            }
                            s.ssids.push(name);
                        }
                    }
                }
            }
            MgmtSubtype::Authentication => {
                self.counts.authentications += 1;
                if p.from_ap {
                    self.target(f, p.receiver, h);
                } else if let Some(s) = self.sender(f, ta, h) {
                    s.bssid = bssid.or(s.bssid);
                    if s.link != Link::Associated {
                        s.link = Link::Joining;
                    }
                }
            }
            MgmtSubtype::AssocRequest | MgmtSubtype::ReassocRequest => {
                self.counts.associations += 1;
                if let Some(s) = self.sender(f, ta, h) {
                    s.bssid = bssid.or(s.bssid);
                    s.link = Link::Joining;
                }
            }
            MgmtSubtype::AssocResponse | MgmtSubtype::ReassocResponse => {
                self.counts.associations += 1;
                let ok = matches!(body, Some(MgmtBody::AssocResponse { status: 0, .. }));
                if let Some(s) = self.target(f, p.receiver, h) {
                    if ok {
                        s.bssid = bssid.or(s.bssid);
                        s.link = Link::Associated;
                    }
                }
            }
            MgmtSubtype::Deauthentication | MgmtSubtype::Disassociation => {
                self.counts.departures += 1;
                let reason = match body {
                    Some(MgmtBody::Deauthentication { reason } | MgmtBody::Disassociation { reason }) => Some(reason),
                    _ => None,
                };
                let s = if p.from_ap {
                    self.target(f, p.receiver, h)
                } else {
                    self.sender(f, ta, h)
                };
                if let Some(s) = s {
                    s.bssid = bssid.or(s.bssid);
                    s.link = Link::Left;
                    s.last_reason = reason;
                }
            }
            MgmtSubtype::Action | MgmtSubtype::ActionNoAck => {
                self.counts.actions += 1;
                if p.from_ap {
                    self.target(f, p.receiver, h);
                } else {
                    self.sender(f, ta, h);
                }
            }
            _ => {}
        }
    }

    fn data(&mut self, f: &Frame<'_>, p: &phy::Parties, h: Heard) {
        let null = f.is_null_data();
        if null {
            self.counts.null_data += 1;
        } else {
            self.counts.data += 1;
        }
        let Some(ta) = p.transmitter else {
            return;
        };
        let bssid = p.bssid.filter(|b| !phy::is_group(b));
        if p.to_ap {
            // A station to its access point.
            if let Some(s) = self.sender(f, ta, h) {
                s.bssid = bssid.or(s.bssid);
                s.link = Link::Associated;
                if null {
                    s.null_data += 1;
                } else {
                    s.data += 1;
                }
            }
        } else if p.from_ap {
            // An access point to a station. A group address is not a station.
            if let Some(s) = self.target(f, p.receiver, h) {
                s.bssid = bssid.or(s.bssid);
                s.link = Link::Associated;
                s.data += u64::from(!null);
            }
        } else if !f.fc.to_ds && !f.fc.from_ds {
            // Station to station (an IBSS, a direct link): the header names no access point.
            if let Some(s) = self.sender(f, ta, h) {
                s.data += u64::from(!null);
            }
            self.target(f, p.receiver, h);
        }
        // To DS and From DS both set: a link between two access points; neither end is a client station.
    }

    fn control(&mut self, f: &Frame<'_>, p: &phy::Parties, h: Heard) {
        match f.fc.subtype {
            control::RTS => self.counts.rts += 1,
            control::CTS => self.counts.cts += 1,
            control::ACK => self.counts.acks += 1,
            control::BLOCK_ACK | control::BLOCK_ACK_REQ => self.counts.block_acks += 1,
            _ => self.counts.other_control += 1,
        }
        // A control frame does not say which side is the client, except PS-Poll. It updates stations already known.
        if f.fc.subtype == control::PS_POLL {
            if let Some(ta) = p.transmitter {
                if let Some(s) = self.sender(f, ta, h) {
                    s.bssid = p.bssid.or(s.bssid);
                    s.link = Link::Associated;
                }
            }
            return;
        }
        if let Some(ta) = p.transmitter {
            if self.map.contains_key(&ta) {
                self.sender(f, ta, h);
            }
        }
        if self.map.contains_key(&p.receiver) {
            self.target(f, p.receiver, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AP: [u8; 6] = [0x00, 0x1B, 0x63, 1, 2, 3];
    const STA: [u8; 6] = [0xDA, 0xA1, 0x19, 4, 5, 6];
    const STA2: [u8; 6] = [0x00, 0x17, 0xF2, 7, 8, 9];

    fn header(ftype: u8, subtype: u8, flags: u8, a1: [u8; 6], a2: [u8; 6], a3: [u8; 6]) -> Vec<u8> {
        let mut f = vec![ftype << 2 | subtype << 4, flags, 0, 0];
        f.extend_from_slice(&a1);
        f.extend_from_slice(&a2);
        f.extend_from_slice(&a3);
        f.extend_from_slice(&[0x10, 0]);
        f
    }

    fn at(t: f64) -> Heard {
        Heard {
            time_s: t,
            level_dbfs: -40.0,
            channel: 6,
        }
    }

    #[test]
    fn wifi_stations_from_probe_requests() {
        let mut s = Stations::default();
        let mut f = header(0, 4, 0, phy::BROADCAST, STA, phy::BROADCAST);
        phy::push_element(&mut f, phy::element::SSID, b"cafe").unwrap();
        phy::push_element(&mut f, phy::element::SUPPORTED_RATES, &[0x82, 0x84, 0x0C, 0x6C]).unwrap();
        phy::push_element(&mut f, phy::element::HT_CAPABILITIES, &[0; 26]).unwrap();
        s.take(&f, at(1.0));
        // The wildcard SSID adds no name. The same frame 50 µs later on another channel is a repeat.
        let mut w = header(0, 4, 0, phy::BROADCAST, STA, phy::BROADCAST);
        phy::push_element(&mut w, phy::element::SSID, b"").unwrap();
        s.take(&w, at(1.5));
        s.take(&w, at(1.50005));
        let st = s.get(&STA).unwrap();
        assert!(st.randomised);
        assert_eq!(st.vendor, "");
        assert_eq!((st.probes, st.sent, st.link), (2, 2, Link::Probing));
        assert_eq!(st.ssids, ["cafe"]);
        assert_eq!((st.generation, st.max_rate_mbps), (4, Some(54.0)));
        assert_eq!(st.last_heard_s, Some(1.5));
        assert_eq!((s.counts.probe_requests, s.counts.repeats), (2, 1));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn wifi_stations_join_send_data_and_leave() {
        let mut s = Stations::default();
        // Authentication and association request from the station; the response from the access point.
        let mut auth = header(0, 11, 0, AP, STA2, AP);
        auth.extend_from_slice(&[0, 0, 1, 0, 0, 0]);
        s.take(&auth, at(1.0));
        assert_eq!(s.get(&STA2).unwrap().link, Link::Joining);
        let mut resp = header(0, 1, 0, STA2, AP, AP);
        resp.extend_from_slice(&[0x01, 0x04, 0, 0, 0x01, 0xC0]);
        s.take(&resp, at(1.1));
        let st = s.get(&STA2).unwrap();
        assert_eq!((st.link, st.bssid), (Link::Associated, Some(AP)));
        // The response was sent to the station: it does not move its last-heard time or level.
        assert_eq!((st.last_heard_s, st.received), (Some(1.0), 1));
        assert!(!st.randomised && !st.vendor.is_empty(), "{:?}", st.vendor);
        // A protected QoS data frame to the access point, a null frame, then data from the access point to another station.
        let mut d = header(2, 8, 0x41, AP, STA2, [1, 2, 3, 4, 5, 6]);
        d.extend_from_slice(&[0, 0, 9, 9, 9, 9]);
        s.take(&d, at(2.0));
        s.take(&header(2, 4, 0x11, AP, STA2, AP), at(2.1));
        let mut down = header(2, 0, 0x42, STA, AP, [1, 2, 3, 4, 5, 6]);
        down.extend_from_slice(&[7; 20]);
        s.take(&down, at(2.2));
        let st = s.get(&STA2).unwrap();
        assert_eq!((st.data, st.null_data, st.protected, st.power_save), (1, 1, 1, true));
        let other = s.get(&STA).unwrap();
        assert_eq!((other.link, other.bssid, other.last_heard_s), (Link::Associated, Some(AP), None));
        assert_eq!(s.of_bss(&AP), 2);
        // The access point never becomes a station; a group address neither.
        assert!(s.get(&AP).is_none());
        s.take(&header(2, 0, 0x02, phy::BROADCAST, AP, AP), at(2.3));
        assert_eq!(s.len(), 2);
        // An Ack to a known station counts as received; an RTS from an unknown address makes no station.
        let mut ack = vec![0xD4, 0, 0, 0];
        ack.extend_from_slice(&STA2);
        s.take(&ack, at(2.4));
        let mut rts = vec![0xB4, 0, 0, 0];
        rts.extend_from_slice(&AP);
        rts.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        s.take(&rts, at(2.5));
        assert_eq!((s.counts.acks, s.counts.rts, s.len()), (1, 1, 2));
        // Deauthentication from the access point with reason 3.
        let mut de = header(0, 12, 0, STA2, AP, AP);
        de.extend_from_slice(&[3, 0]);
        s.take(&de, at(3.0));
        let st = s.get(&STA2).unwrap();
        assert_eq!((st.link, st.last_reason), (Link::Left, Some(3)));
        assert_eq!(s.of_bss(&AP), 1);
        assert_eq!((s.counts.data, s.counts.null_data, s.counts.protected), (3, 1, 2));
    }
}
