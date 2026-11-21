//! FIG syntax (EN 300 401 clauses 5.2.2, 6 and 8, and the FIG 0/22 TII database of earlier editions): every FIG type and
//! extension decoded into a [`Fig`], and a record of each FIG for an FIC analyser ([`FigRecord`]). The lists of a decoded FIG
//! are in a [`FigLists`] that the caller keeps, so decoding allocates nothing once the lists have grown.

use crate::bits::{BitReader, Fields};
use std::fmt;
use std::ops::Deref;

/// The longest data field of a FIG: its 5-bit length field (EN 300 401 clause 5.2.2).
const FIG_MAX: usize = 31;

/// Bytes of a FIG's data field, held in place.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FigBytes {
    len: u8,
    bytes: [u8; FIG_MAX],
}

impl FigBytes {
    /// A copy of `data`. Bytes beyond the 31 that a FIG can carry are cut.
    pub fn new(data: &[u8]) -> Self {
        let n = data.len().min(FIG_MAX);
        let mut bytes = [0; FIG_MAX];
        bytes[..n].copy_from_slice(&data[..n]);
        FigBytes {
            len: n as u8,
            bytes,
        }
    }
}

impl Deref for FigBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl fmt::Debug for FigBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/// A FIG as it came: type, extension and flags, and its data field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FigRecord {
    /// The FIG type (0 … 7).
    pub fig_type: u8,
    /// The extension (type 0: 5 bits; types 1 and 2: 3 bits).
    pub ext: u8,
    data: FigBytes,
}

impl FigRecord {
    /// A record of a FIG of `fig_type` with this data field (first byte the flags byte). A data field longer than a FIG
    /// can carry is cut to 31 bytes.
    pub fn new(fig_type: u8, data: &[u8]) -> Self {
        let ext = match fig_type {
            0 => data.first().map_or(0, |b| b & 31),
            1 | 2 => data.first().map_or(0, |b| b & 7),
            _ => 0,
        };
        FigRecord {
            fig_type,
            ext,
            data: FigBytes::new(data),
        }
    }
    /// The whole data field, including its first (flags) byte.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// C/N (type 0).
    pub fn cn(&self) -> bool {
        self.fig_type == 0 && self.data().first().is_some_and(|b| b & 0x80 != 0)
    }
    /// OE (type 0; type 1 has it in bit 3).
    pub fn oe(&self) -> bool {
        match self.fig_type {
            0 => self.data().first().is_some_and(|b| b & 0x40 != 0),
            1 => self.data().first().is_some_and(|b| b & 0x08 != 0),
            _ => false,
        }
    }
    /// P/D (type 0).
    pub fn pd(&self) -> bool {
        self.fig_type == 0 && self.data().first().is_some_and(|b| b & 0x20 != 0)
    }
    /// "0/1", "1/0" …
    pub fn name(&self) -> String {
        format!("{}/{}", self.fig_type, self.ext)
    }
}

/// A sub-channel's organisation (FIG 0/1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubChannelOrg {
    /// Sub-channel id.
    pub id: u8,
    /// Start address in CU.
    pub start: u16,
    /// The protection: short form (UEP index; `None` for table switch 1) or long form.
    pub form: SubChForm,
}

/// Short or long form of FIG 0/1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubChForm {
    /// UEP table index (table switch 0), or `None` when the table switch is 1 (undefined).
    Short(Option<u8>),
    /// EEP: option (0 = A, 1 = B), level (1 … 4), size in CU.
    Long {
        /// 0 = A, 1 = B.
        option: u8,
        /// 1 … 4.
        level: u8,
        /// Size in CU.
        size: u16,
    },
}

/// A service component (FIG 0/2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentOrg {
    /// Transport mechanism: 0 stream audio, 1 stream data, 3 packet data.
    pub tmid: u8,
    /// ASCTy (TMId 0) or DSCTy (TMId 1).
    pub ty: u8,
    /// Sub-channel (TMId 0 and 1).
    pub subch: Option<u8>,
    /// SCId (TMId 3).
    pub scid: Option<u16>,
    /// Primary component.
    pub primary: bool,
    /// Conditional access.
    pub ca: bool,
}

/// A service (FIG 0/2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceOrg {
    /// SId (16 or 32 bits).
    pub sid: u32,
    /// Local flag.
    pub local: bool,
    /// CAId.
    pub caid: u8,
    /// Components.
    pub components: Vec<ComponentOrg>,
}

/// A packet mode component (FIG 0/3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketOrg {
    /// SCId.
    pub scid: u16,
    /// CAOrg present.
    pub ca_org: Option<u16>,
    /// Data groups used.
    pub data_groups: bool,
    /// DSCTy.
    pub dscty: u8,
    /// Sub-channel.
    pub subch: u8,
    /// Packet address.
    pub address: u16,
}

/// One linkage set (FIG 0/6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkSet {
    /// Linkage set number.
    pub lsn: u16,
    /// Active.
    pub active: bool,
    /// Hard link.
    pub hard: bool,
    /// International.
    pub international: bool,
    /// Identifier list qualifier (0 DAB SId, 1 RDS PI, 3 DRM/AMSS).
    pub idlq: u8,
    /// The identifiers (with their ECC for an international list), first the key service.
    pub ids: Vec<(Option<u8>, u32)>,
}

/// A component global definition (FIG 0/8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDef {
    /// SId.
    pub sid: u32,
    /// SCIdS.
    pub scids: u8,
    /// Sub-channel (short form).
    pub subch: Option<u8>,
    /// SCId (long form).
    pub scid: Option<u16>,
}

/// User applications of a component (FIG 0/13).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserApps {
    /// SId.
    pub sid: u32,
    /// SCIdS.
    pub scids: u8,
    /// (user application type, its data).
    pub apps: Vec<(u16, FigBytes)>,
}

/// A programme type entry (FIG 0/17).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtyEntry {
    /// SId.
    pub sid: u16,
    /// S/D: the type describes the current programme.
    pub dynamic: bool,
    /// P/S (secondary).
    pub secondary: bool,
    /// Language, when given.
    pub language: Option<u8>,
    /// The programme type.
    pub pty: u8,
    /// The complementary code, when given.
    pub cc: Option<u8>,
}

/// Announcement support (FIG 0/18).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AsuEntry {
    /// SId.
    pub sid: u16,
    /// ASu flags.
    pub asu: u16,
    /// Cluster ids.
    pub clusters: Vec<u8>,
}

/// Announcement switching (FIG 0/19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AswEntry {
    /// Cluster id.
    pub cluster: u8,
    /// ASw flags.
    pub asw: u16,
    /// New flag.
    pub new: bool,
    /// Region (when the region flag is set).
    pub region: Option<u8>,
    /// The sub-channel carrying it.
    pub subch: u8,
}

/// A frequency list (FIG 0/21).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreqList {
    /// The id field (EId, PI, DRM/AMSS id).
    pub id: u32,
    /// Range and modulation.
    pub rm: u8,
    /// Continuity flag.
    pub continuity: bool,
    /// Frequencies in kHz (with the control field for DAB).
    pub freqs_khz: Vec<(u32, u8)>,
}

/// The position of a main identifier of the TII database (FIG 0/22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiiMain {
    /// MainId.
    pub main: u8,
    /// Degrees.
    pub lat: f64,
    /// Degrees.
    pub lon: f64,
}

/// Sub-identifiers of the TII database, placed relative to their main identifier (FIG 0/22).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TiiSubs {
    /// MainId.
    pub main: u8,
    /// (SubId, delay µs, latitude offset degrees, longitude offset degrees).
    pub subs: Vec<(u8, u16, f64, f64)>,
}

/// Other-ensemble services (FIG 0/24).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OeService {
    /// SId.
    pub sid: u32,
    /// CAId.
    pub caid: u8,
    /// The ensembles that carry it.
    pub eids: Vec<u16>,
}

/// What a label belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelOf {
    /// The ensemble.
    Ensemble(u16),
    /// A programme service.
    Service(u32),
    /// A service component.
    Component {
        /// SId.
        sid: u32,
        /// SCIdS.
        scids: u8,
    },
    /// A data service.
    DataService(u32),
    /// An X-PAD user application.
    XpadApp {
        /// SId.
        sid: u32,
        /// SCIdS.
        scids: u8,
        /// X-PAD application type.
        app: u8,
    },
    /// A region (FIG 1/3).
    Region(u8),
    /// Something else (extension number).
    Other(u8),
}

/// A decoded FIG. Its lists are in the [`FigLists`] it was decoded into; the segment of an extended label is in its record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fig<'a> {
    /// 0/0.
    Ensemble {
        /// EId.
        eid: u16,
        /// Change flags.
        change: u8,
        /// Alarm flag.
        alarm: bool,
        /// CIF count 0 … 4999.
        cif_count: u16,
        /// Occurrence change, when the change flags are set.
        occurrence: Option<u8>,
    },
    /// 0/1.
    SubChannels(&'a [SubChannelOrg]),
    /// 0/2.
    Services(&'a [ServiceOrg]),
    /// 0/3.
    Packets(&'a [PacketOrg]),
    /// 0/5: (short form sub-channel or long form SCId, language).
    Languages(&'a [(Option<u8>, Option<u16>, u8)]),
    /// 0/6.
    Linking(&'a [LinkSet]),
    /// 0/7.
    Configuration {
        /// Number of services.
        services: u8,
        /// Reconfiguration count.
        count: u16,
    },
    /// 0/8.
    GlobalDefinitions(&'a [GlobalDef]),
    /// 0/9.
    Country {
        /// Local time offset in half hours.
        lto: i8,
        /// LTO unique.
        lto_unique: bool,
        /// Extended country code.
        ecc: u8,
        /// International table id.
        table: u8,
    },
    /// 0/10.
    DateTime {
        /// Modified Julian date.
        mjd: u32,
        /// Leap second indicator.
        lsi: bool,
        /// Hours (UTC).
        hours: u8,
        /// Minutes.
        minutes: u8,
        /// Seconds (long form).
        seconds: Option<u8>,
        /// Milliseconds (long form).
        ms: Option<u16>,
    },
    /// 0/13.
    UserApps(&'a [UserApps]),
    /// 0/14: (sub-channel, FEC scheme).
    Fec(&'a [(u8, u8)]),
    /// 0/17.
    ProgrammeTypes(&'a [PtyEntry]),
    /// 0/18.
    AnnouncementSupport(&'a [AsuEntry]),
    /// 0/19.
    AnnouncementSwitching(&'a [AswEntry]),
    /// 0/21.
    Frequencies(&'a [FreqList]),
    /// 0/22.
    Tii {
        /// The main identifiers.
        mains: &'a [TiiMain],
        /// The sub-identifiers.
        subs: &'a [TiiSubs],
    },
    /// 0/24.
    OeServices(&'a [OeService]),
    /// 0/25: (SId, ASu, EIds).
    OeAnnouncementSupport(&'a [(u16, u16, Vec<u16>)]),
    /// 0/26, recorded only.
    OeAnnouncementSwitching,
    /// 1/x.
    Label {
        /// What it names.
        of: LabelOf,
        /// The character set.
        charset: u8,
        /// The 16 label bytes.
        text: [u8; 16],
        /// The character flag field.
        flags: u16,
    },
    /// 2/x: one segment of an extended label.
    ExtLabel {
        /// What it names.
        of: LabelOf,
        /// Toggle flag.
        toggle: bool,
        /// Segment index.
        segment: u8,
        /// The reserved bit after the segment index (when set, segment 0 has no character flag field).
        rfu: bool,
        /// The segment bytes.
        data: &'a [u8],
    },
    /// A FIG type or extension not decoded.
    Unknown,
}

/// Why a FIG did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncated;

fn sid(r: &mut BitReader, pd: bool) -> Option<u32> {
    r.read(if pd { 32 } else { 16 })
}

/// A list whose elements stay, with their own storage, from one fill to the next.
#[derive(Clone, Debug, Default)]
struct Pool<T> {
    items: Vec<T>,
    used: usize,
}

impl<T: Default> Pool<T> {
    fn clear(&mut self) {
        self.used = 0;
    }
    /// Adds an element and returns it as an earlier fill left it: the caller sets every field.
    fn add(&mut self) -> &mut T {
        if self.used == self.items.len() {
            self.items.push(T::default());
        }
        self.used += 1;
        &mut self.items[self.used - 1]
    }
    fn filled(&self) -> &[T] {
        &self.items[..self.used]
    }
}

/// The lists of decoded FIGs. [`decode`] clears and refills the lists of the FIG it decodes, so a caller that keeps one
/// `FigLists` allocates nothing once each list has held its longest FIG.
#[derive(Clone, Debug, Default)]
pub struct FigLists {
    subchannels: Vec<SubChannelOrg>,
    services: Pool<ServiceOrg>,
    packets: Vec<PacketOrg>,
    languages: Vec<(Option<u8>, Option<u16>, u8)>,
    links: Pool<LinkSet>,
    globals: Vec<GlobalDef>,
    user_apps: Pool<UserApps>,
    fec: Vec<(u8, u8)>,
    ptys: Vec<PtyEntry>,
    asu: Pool<AsuEntry>,
    asw: Vec<AswEntry>,
    freqs: Pool<FreqList>,
    tii_mains: Vec<TiiMain>,
    tii_subs: Pool<TiiSubs>,
    oe_services: Pool<OeService>,
    oe_asu: Pool<(u16, u16, Vec<u16>)>,
}

/// Decodes a FIG record. The lists of the FIG are written to `lists`.
pub fn decode<'a>(rec: &'a FigRecord, lists: &'a mut FigLists) -> Result<Fig<'a>, Truncated> {
    let d = rec.data();
    if d.is_empty() {
        return Err(Truncated);
    }
    let body = &d[1..];
    let pd = rec.pd();
    let t = Truncated;
    let mut r = BitReader::new(body);
    Ok(match (rec.fig_type, rec.ext) {
        (0, 0) => {
            let eid = r.read(16).ok_or(t)? as u16;
            let change = r.read(2).ok_or(t)? as u8;
            let alarm = r.flag().ok_or(t)?;
            let hi = r.read(5).ok_or(t)? as u16;
            let lo = r.read(8).ok_or(t)? as u16;
            let occurrence = if change != 0 {
                r.read(8).map(|v| v as u8)
            } else {
                None
            };
            Fig::Ensemble {
                eid,
                change,
                alarm,
                cif_count: (hi * 250 + lo) % 5000,
                occurrence,
            }
        }
        (0, 1) => {
            let v = &mut lists.subchannels;
            v.clear();
            while r.left() >= 24 {
                let id = r.read(6).ok_or(t)? as u8;
                let start = r.read(10).ok_or(t)? as u16;
                let long = r.flag().ok_or(t)?;
                let form = if long {
                    let option = r.read(3).ok_or(t)? as u8;
                    let level = r.read(2).ok_or(t)? as u8 + 1;
                    let size = r.read(10).ok_or(t)? as u16;
                    SubChForm::Long {
                        option,
                        level,
                        size,
                    }
                } else {
                    let switch = r.flag().ok_or(t)?;
                    let index = r.read(6).ok_or(t)? as u8;
                    SubChForm::Short(if switch { None } else { Some(index) })
                };
                v.push(SubChannelOrg { id, start, form });
            }
            if r.left() >= 8 {
                return Err(t);
            }
            Fig::SubChannels(v)
        }
        (0, 2) => {
            let v = &mut lists.services;
            v.clear();
            while r.left() > 0 {
                let s = v.add();
                s.sid = sid(&mut r, pd).ok_or(t)?;
                s.local = r.flag().ok_or(t)?;
                s.caid = r.read(3).ok_or(t)? as u8;
                let n = r.read(4).ok_or(t)? as usize;
                s.components.clear();
                for _ in 0..n {
                    let tmid = r.read(2).ok_or(t)? as u8;
                    let (ty, subch, scid) = match tmid {
                        3 => (0, None, Some(r.read(12).ok_or(t)? as u16)),
                        _ => {
                            let ty = r.read(6).ok_or(t)? as u8;
                            (ty, Some(r.read(6).ok_or(t)? as u8), None)
                        }
                    };
                    let primary = r.flag().ok_or(t)?;
                    let ca = r.flag().ok_or(t)?;
                    s.components.push(ComponentOrg {
                        tmid,
                        ty,
                        subch,
                        scid,
                        primary,
                        ca,
                    });
                }
            }
            Fig::Services(v.filled())
        }
        (0, 3) => {
            let v = &mut lists.packets;
            v.clear();
            while r.left() >= 40 {
                let scid = r.read(12).ok_or(t)? as u16;
                r.pass(3).ok_or(t)?;
                let caorg_flag = r.flag().ok_or(t)?;
                let data_groups = r.flag().ok_or(t)?;
                r.pass(1).ok_or(t)?;
                let dscty = r.read(6).ok_or(t)? as u8;
                let subch = r.read(6).ok_or(t)? as u8;
                let address = r.read(10).ok_or(t)? as u16;
                let ca_org = if caorg_flag {
                    Some(r.read(16).ok_or(t)? as u16)
                } else {
                    None
                };
                v.push(PacketOrg {
                    scid,
                    ca_org,
                    data_groups,
                    dscty,
                    subch,
                    address,
                });
            }
            Fig::Packets(v)
        }
        (0, 5) => {
            let v = &mut lists.languages;
            v.clear();
            while r.left() >= 16 {
                let long = r.flag().ok_or(t)?;
                if long {
                    r.pass(3).ok_or(t)?;
                    let scid = r.read(12).ok_or(t)? as u16;
                    let lang = r.read(8).ok_or(t)? as u8;
                    v.push((None, Some(scid), lang));
                } else {
                    r.pass(1).ok_or(t)?;
                    let sc = r.read(6).ok_or(t)? as u8;
                    let lang = r.read(8).ok_or(t)? as u8;
                    v.push((Some(sc), None, lang));
                }
            }
            Fig::Languages(v)
        }
        (0, 6) => {
            let v = &mut lists.links;
            v.clear();
            while r.left() >= 16 {
                let list = r.flag().ok_or(t)?;
                let active = r.flag().ok_or(t)?;
                let hard = r.flag().ok_or(t)?;
                let international = r.flag().ok_or(t)?;
                let lsn = r.read(12).ok_or(t)? as u16;
                let l = v.add();
                let ids = &mut l.ids;
                ids.clear();
                let mut idlq = 0;
                if list {
                    r.pass(1).ok_or(t)?;
                    idlq = r.read(2).ok_or(t)? as u8;
                    r.pass(1).ok_or(t)?;
                    let n = r.read(4).ok_or(t)? as usize;
                    for _ in 0..n {
                        if pd {
                            ids.push((None, r.read(32).ok_or(t)?));
                        } else if international {
                            let ecc = r.read(8).ok_or(t)? as u8;
                            ids.push((Some(ecc), r.read(16).ok_or(t)?));
                        } else {
                            ids.push((None, r.read(16).ok_or(t)?));
                        }
                    }
                }
                (l.lsn, l.active, l.hard, l.international, l.idlq) =
                    (lsn, active, hard, international, idlq);
            }
            Fig::Linking(v.filled())
        }
        (0, 7) => {
            let services = r.read(6).ok_or(t)? as u8;
            let count = r.read(10).ok_or(t)? as u16;
            Fig::Configuration { services, count }
        }
        (0, 8) => {
            let v = &mut lists.globals;
            v.clear();
            while r.left() > 0 {
                let s = sid(&mut r, pd).ok_or(t)?;
                let ext = r.flag().ok_or(t)?;
                r.pass(3).ok_or(t)?;
                let scids = r.read(4).ok_or(t)? as u8;
                let long = r.flag().ok_or(t)?;
                let (subch, scid) = if long {
                    r.pass(3).ok_or(t)?;
                    (None, Some(r.read(12).ok_or(t)? as u16))
                } else {
                    r.pass(1).ok_or(t)?;
                    (Some(r.read(6).ok_or(t)? as u8), None)
                };
                if ext {
                    r.pass(8).ok_or(t)?;
                }
                v.push(GlobalDef {
                    sid: s,
                    scids,
                    subch,
                    scid,
                });
            }
            Fig::GlobalDefinitions(v)
        }
        (0, 9) => {
            let _ext = r.flag().ok_or(t)?;
            let lto_unique = r.flag().ok_or(t)?;
            let neg = r.flag().ok_or(t)?;
            let half = r.read(5).ok_or(t)? as i8;
            let ecc = r.read(8).ok_or(t)? as u8;
            let table = r.read(8).ok_or(t)? as u8;
            Fig::Country {
                lto: if neg { -half } else { half },
                lto_unique,
                ecc,
                table,
            }
        }
        (0, 10) => {
            r.pass(1).ok_or(t)?;
            let mjd = r.read(17).ok_or(t)?;
            let lsi = r.flag().ok_or(t)?;
            r.pass(1).ok_or(t)?;
            let long = r.flag().ok_or(t)?;
            let hours = r.read(5).ok_or(t)? as u8;
            let minutes = r.read(6).ok_or(t)? as u8;
            let (seconds, ms) = if long {
                (
                    Some(r.read(6).ok_or(t)? as u8),
                    Some(r.read(10).ok_or(t)? as u16),
                )
            } else {
                (None, None)
            };
            Fig::DateTime {
                mjd,
                lsi,
                hours,
                minutes,
                seconds,
                ms,
            }
        }
        (0, 13) => {
            let v = &mut lists.user_apps;
            v.clear();
            while r.left() > 0 {
                let u = v.add();
                u.sid = sid(&mut r, pd).ok_or(t)?;
                u.scids = r.read(4).ok_or(t)? as u8;
                let n = r.read(4).ok_or(t)? as usize;
                u.apps.clear();
                for _ in 0..n {
                    let ty = r.read(11).ok_or(t)? as u16;
                    let len = r.read(5).ok_or(t)? as usize;
                    let data = FigBytes::new(r.take_bytes(len).ok_or(t)?);
                    u.apps.push((ty, data));
                }
            }
            Fig::UserApps(v.filled())
        }
        (0, 14) => {
            let v = &mut lists.fec;
            v.clear();
            while r.left() >= 8 {
                let sc = r.read(6).ok_or(t)? as u8;
                let fec = r.read(2).ok_or(t)? as u8;
                v.push((sc, fec));
            }
            Fig::Fec(v)
        }
        (0, 17) => {
            let v = &mut lists.ptys;
            v.clear();
            while r.left() >= 32 {
                let s = r.read(16).ok_or(t)? as u16;
                let dynamic = r.flag().ok_or(t)?;
                let secondary = r.flag().ok_or(t)?;
                let l = r.flag().ok_or(t)?;
                let cc = r.flag().ok_or(t)?;
                r.pass(4).ok_or(t)?;
                let language = if l {
                    Some(r.read(8).ok_or(t)? as u8)
                } else {
                    None
                };
                r.pass(3).ok_or(t)?;
                let pty = r.read(5).ok_or(t)? as u8;
                let cc = if cc {
                    Some(r.read(8).ok_or(t)? as u8)
                } else {
                    None
                };
                v.push(PtyEntry {
                    sid: s,
                    dynamic,
                    secondary,
                    language,
                    pty,
                    cc,
                });
            }
            Fig::ProgrammeTypes(v)
        }
        (0, 18) => {
            let v = &mut lists.asu;
            v.clear();
            while r.left() >= 40 {
                let a = v.add();
                a.sid = r.read(16).ok_or(t)? as u16;
                a.asu = r.read(16).ok_or(t)? as u16;
                r.pass(5).ok_or(t)?;
                let n = r.read(3).ok_or(t)? as usize;
                a.clusters.clear();
                for _ in 0..n {
                    a.clusters.push(r.read(8).ok_or(t)? as u8);
                }
            }
            Fig::AnnouncementSupport(v.filled())
        }
        (0, 19) => {
            let v = &mut lists.asw;
            v.clear();
            while r.left() >= 32 {
                let cluster = r.read(8).ok_or(t)? as u8;
                let asw = r.read(16).ok_or(t)? as u16;
                let new = r.flag().ok_or(t)?;
                let region_flag = r.flag().ok_or(t)?;
                let subch = r.read(6).ok_or(t)? as u8;
                let region = if region_flag {
                    r.pass(2).ok_or(t)?;
                    Some(r.read(6).ok_or(t)? as u8)
                } else {
                    None
                };
                v.push(AswEntry {
                    cluster,
                    asw,
                    new,
                    region,
                    subch,
                });
            }
            Fig::AnnouncementSwitching(v)
        }
        (0, 21) => {
            let v = &mut lists.freqs;
            v.clear();
            while r.left() >= 16 {
                r.pass(11).ok_or(t)?;
                let len = r.read(5).ok_or(t)? as usize;
                let lists = r.take_bytes(len).ok_or(t)?;
                let mut q = BitReader::new(lists);
                while q.left() >= 24 {
                    let id = q.read(16).ok_or(t)?;
                    let rm = q.read(4).ok_or(t)? as u8;
                    let continuity = q.flag().ok_or(t)?;
                    let flen = q.read(3).ok_or(t)? as usize;
                    let f = q.take_bytes(flen).ok_or(t)?;
                    let mut fr = BitReader::new(f);
                    let l = v.add();
                    let freqs = &mut l.freqs_khz;
                    freqs.clear();
                    let mut idv = id;
                    match rm {
                        0 | 1 => {
                            while fr.left() >= 24 {
                                let ctl = fr.read(5).ok_or(t)? as u8;
                                let fa = fr.read(19).ok_or(t)?;
                                freqs.push((fa * 16, ctl));
                            }
                        }
                        8 | 9 | 10 | 12 => {
                            while fr.left() >= 8 {
                                let b = fr.read(8).ok_or(t)?;
                                let khz = match rm {
                                    8 | 9 => 87_500 + b * 100,
                                    10 => 144 + b * 9,
                                    _ => 144 + b * 9,
                                };
                                freqs.push((khz, 0));
                            }
                        }
                        6 | 14 => {
                            if fr.left() >= 8 {
                                let id2 = fr.read(8).ok_or(t)?;
                                idv = (id2 << 16) | id;
                            }
                            while fr.left() >= 16 {
                                let m = fr.flag().ok_or(t)?;
                                let fq = fr.read(15).ok_or(t)?;
                                freqs.push((if m && rm == 6 { fq * 10 } else { fq }, 0));
                            }
                        }
                        _ => {}
                    }
                    (l.id, l.rm, l.continuity) = (idv, rm, continuity);
                }
            }
            Fig::Frequencies(v.filled())
        }
        (0, 22) => {
            let (mains, subs) = (&mut lists.tii_mains, &mut lists.tii_subs);
            mains.clear();
            subs.clear();
            while r.left() >= 8 {
                let sub = r.flag().ok_or(t)?;
                let main = r.read(7).ok_or(t)? as u8;
                if !sub {
                    let latc = r.read(16).ok_or(t)? as u16 as i16 as f64;
                    let lonc = r.read(16).ok_or(t)? as u16 as i16 as f64;
                    let latf = r.read(4).ok_or(t)? as f64;
                    let lonf = r.read(4).ok_or(t)? as f64;
                    let lat = latc * 90.0 / 32768.0 + latf * 90.0 / 524_288.0;
                    let lon = lonc * 180.0 / 32768.0 + lonf * 180.0 / 524_288.0;
                    mains.push(TiiMain { main, lat, lon });
                } else {
                    r.pass(5).ok_or(t)?;
                    let n = r.read(3).ok_or(t)? as usize;
                    let s = subs.add();
                    s.main = main;
                    s.subs.clear();
                    for _ in 0..n {
                        let id = r.read(5).ok_or(t)? as u8;
                        let td = r.read(11).ok_or(t)? as u16;
                        let dlat = r.read(16).ok_or(t)? as u16 as i16 as f64 * 90.0 / 524_288.0;
                        let dlon = r.read(16).ok_or(t)? as u16 as i16 as f64 * 180.0 / 524_288.0;
                        s.subs.push((id, td, dlat, dlon));
                    }
                }
            }
            Fig::Tii {
                mains,
                subs: subs.filled(),
            }
        }
        (0, 24) => {
            let v = &mut lists.oe_services;
            v.clear();
            while r.left() > 0 {
                let o = v.add();
                o.sid = sid(&mut r, pd).ok_or(t)?;
                r.pass(1).ok_or(t)?;
                o.caid = r.read(3).ok_or(t)? as u8;
                let n = r.read(4).ok_or(t)? as usize;
                o.eids.clear();
                for _ in 0..n {
                    o.eids.push(r.read(16).ok_or(t)? as u16);
                }
            }
            Fig::OeServices(v.filled())
        }
        (0, 25) => {
            let v = &mut lists.oe_asu;
            v.clear();
            while r.left() >= 40 {
                let (s, asu, eids) = v.add();
                *s = r.read(16).ok_or(t)? as u16;
                *asu = r.read(16).ok_or(t)? as u16;
                r.pass(4).ok_or(t)?;
                let n = r.read(4).ok_or(t)? as usize;
                eids.clear();
                for _ in 0..n {
                    eids.push(r.read(16).ok_or(t)? as u16);
                }
            }
            Fig::OeAnnouncementSupport(v.filled())
        }
        (0, 26) => Fig::OeAnnouncementSwitching,
        (1, ext) => {
            let charset = d[0] >> 4;
            let of = match ext {
                0 => LabelOf::Ensemble(r.read(16).ok_or(t)? as u16),
                1 => LabelOf::Service(r.read(16).ok_or(t)?),
                3 => {
                    r.pass(2).ok_or(t)?;
                    LabelOf::Region(r.read(6).ok_or(t)? as u8)
                }
                4 => {
                    let pd = r.flag().ok_or(t)?;
                    r.pass(3).ok_or(t)?;
                    let scids = r.read(4).ok_or(t)? as u8;
                    LabelOf::Component {
                        sid: sid(&mut r, pd).ok_or(t)?,
                        scids,
                    }
                }
                5 => LabelOf::DataService(r.read(32).ok_or(t)?),
                6 => {
                    let pd = r.flag().ok_or(t)?;
                    r.pass(3).ok_or(t)?;
                    let scids = r.read(4).ok_or(t)? as u8;
                    let s = sid(&mut r, pd).ok_or(t)?;
                    r.pass(3).ok_or(t)?;
                    let app = r.read(5).ok_or(t)? as u8;
                    LabelOf::XpadApp { sid: s, scids, app }
                }
                e => LabelOf::Other(e),
            };
            let text: [u8; 16] = r.take_bytes(16).ok_or(t)?.try_into().map_err(|_| t)?;
            let flags = r.read(16).ok_or(t)? as u16;
            Fig::Label {
                of,
                charset,
                text,
                flags,
            }
        }
        (2, ext) => {
            let toggle = d[0] & 0x80 != 0;
            let segment = (d[0] >> 4) & 7;
            let rfu = d[0] & 0x08 != 0;
            let of = match ext {
                0 => LabelOf::Ensemble(r.read(16).ok_or(t)? as u16),
                1 => LabelOf::Service(r.read(16).ok_or(t)?),
                4 => {
                    let pd = r.flag().ok_or(t)?;
                    r.pass(3).ok_or(t)?;
                    let scids = r.read(4).ok_or(t)? as u8;
                    LabelOf::Component {
                        sid: sid(&mut r, pd).ok_or(t)?,
                        scids,
                    }
                }
                5 => LabelOf::DataService(r.read(32).ok_or(t)?),
                e => LabelOf::Other(e),
            };
            let at = r.pos() / 8;
            Fig::ExtLabel {
                of,
                toggle,
                segment,
                rfu,
                data: &body[at..],
            }
        }
        _ => Fig::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t: u8, bytes: &[u8]) -> FigRecord {
        FigRecord::new(t, bytes)
    }

    #[test]
    fn ensemble_information() {
        // EId 0xCE15, CIF count 5/0 (1250).
        let r = rec(0, &[0x00, 0xCE, 0x15, 0x05, 0x00]);
        let mut lists = FigLists::default();
        let f = decode(&r, &mut lists).unwrap();
        assert_eq!(
            f,
            Fig::Ensemble {
                eid: 0xCE15,
                change: 0,
                alarm: false,
                cif_count: 1250,
                occurrence: None
            }
        );
    }

    #[test]
    fn subchannel_long_form() {
        // Sub-channel 3, start 100, long form, option 0, level 3-A (2), size 36.
        let mut w_buf = Vec::new();
        let mut w = crate::bits::BitWriter::new(&mut w_buf);
        w.put(0x01, 8);
        w.put(3, 6);
        w.put(100, 10);
        w.put(1, 1);
        w.put(0, 3);
        w.put(2, 2);
        w.put(36, 10);
        let _ = w.finish();
        let r = rec(0, &w_buf);
        let mut lists = FigLists::default();
        let f = decode(&r, &mut lists).unwrap();
        assert_eq!(
            f,
            Fig::SubChannels(&[SubChannelOrg {
                id: 3,
                start: 100,
                form: SubChForm::Long {
                    option: 0,
                    level: 3,
                    size: 36
                }
            }])
        );
    }

    #[test]
    fn truncated_service_organisation_is_an_error() {
        let mut lists = FigLists::default();
        let short = rec(0, &[0x02, 0xCE, 0x01, 0x01, 0x00]);
        assert!(decode(&short, &mut lists).is_err());
        let whole = rec(0, &[0x02, 0xCE, 0x01, 0x01, 0x3F, 0x0E]);
        let ok = decode(&whole, &mut lists).unwrap();
        match ok {
            Fig::Services(s) => {
                assert_eq!(s[0].sid, 0xCE01);
                assert_eq!(
                    s[0].components[0],
                    ComponentOrg {
                        tmid: 0,
                        ty: 63,
                        subch: Some(3),
                        scid: None,
                        primary: true,
                        ca: false
                    }
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
