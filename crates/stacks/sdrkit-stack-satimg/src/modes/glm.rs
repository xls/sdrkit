//! GLM lightning data of the GOES-R Rebroadcast (`goes_grb`): the generic payload header, the flash, group and event units
//! of APIDs 302, 303 and 301, their values with units, the store of the newest flashes and groups with their parent links,
//! and the GLM part of the test signal.
//!
//! Source: GOES-R Series Product Definition and Users' Guide, Volume 4: GRB (DCN 7035538, revision H.1): 4.5 (byte
//! order), 4.5.1 (sequence count per APID), 5.3.1 (generic payload header), 5.3.2 (control fields), Table 7.2.1.1-1 (field
//! of view), 7.2.1.6.1 (record layouts and flag values), Table 7.2.1.6.2 (scale factors, offsets, fill values and valid
//! ranges), Appendix A.2 (APIDs).
//!
//! The generic payload header is big endian; the data unit after it is little endian (4.5). A unit is a count (8 bytes)
//! and that many records of a fixed size (5.3.2).

use std::collections::{HashMap, VecDeque};

/// APID of the GLM metadata (an NcML text).
pub const APID_METADATA: u16 = 0x300;
/// APID of the event units.
pub const APID_EVENTS: u16 = 0x301;
/// APID of the flash units.
pub const APID_FLASHES: u16 = 0x302;
/// APID of the group units.
pub const APID_GROUPS: u16 = 0x303;
/// Bytes of the generic payload header.
pub const GENERIC_HEADER: usize = 21;
/// The J2000 epoch of the headers (2000-01-01 12:00:00 UTC) in seconds since 1970.
pub const J2000_UNIX: f64 = 946_728_000.0;
/// Bytes of the count before the records.
pub const COUNT_BYTES: usize = 8;
/// Bytes of a flash record.
pub const FLASH_BYTES: usize = 24;
/// Bytes of a group record (the offsets of the guide describe the first 24 of them).
pub const GROUP_BYTES: usize = 28;
/// Bytes of an event record.
pub const EVENT_BYTES: usize = 16;
/// Scale of the time offsets, in seconds.
pub const TIME_SCALE: f64 = 3.814756e-4;
/// Offset of the time offsets, in seconds.
pub const TIME_OFFSET: f64 = -5.0;
/// Scale of the event latitude and longitude, in degrees.
pub const EVENT_DEG_SCALE: f64 = 0.00203128;
/// Offset of the event latitude, in degrees.
pub const EVENT_LAT_OFFSET: f64 = -66.56;
/// Scale of the radiant energy, in joules.
pub const ENERGY_SCALE: f64 = 1.52597e-15;
/// Scale of the area, in square kilometres.
pub const AREA_SCALE: f64 = 0.152601862;
/// The fill value of the scaled integers and the flags (absent).
pub const FILL: u16 = 65_535;
/// The largest valid area and energy value.
pub const VALID_MAX: u16 = 65_530;
/// The north and south bound of the field of view, in degrees (the same at every position).
pub const LAT_BOUND: f64 = 66.56;
/// The westmost bound of the field of view of all positions (GOES-West), in degrees east.
pub const LON_WEST_BOUND: f64 = -203.56;
/// Rows of flashes and of groups kept.
pub const ROWS: usize = 64;
/// Products whose parent links are kept.
const LINK_PRODUCTS: usize = 3;
/// Entries of one map of parent links.
const MAX_LINKS: usize = 1 << 16;

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn lef32(b: &[u8], at: usize) -> f32 {
    f32::from_bits(le32(b, at))
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The generic payload header (5.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericHeader {
    /// Compression of the data unit: 0 none, 1 JPEG 2000, 2 SZIP.
    pub compression: u8,
    /// Product time: seconds since the J2000 epoch.
    pub seconds: u32,
    /// Product time: microseconds of the second.
    pub micros: u32,
    /// Data unit sequence count inside the product.
    pub unit_count: u32,
}

impl GenericHeader {
    /// The header at the start of `d`, or `None` if `d` is shorter than [`GENERIC_HEADER`].
    pub fn parse(d: &[u8]) -> Option<Self> {
        (d.len() >= GENERIC_HEADER).then(|| GenericHeader {
            compression: d[0],
            seconds: be32(d, 1),
            micros: be32(d, 5),
            unit_count: be32(d, 17),
        })
    }

    /// The product time in seconds since 1970 (UTC; days of 86 400 s, leap seconds not counted).
    pub fn product_time(&self) -> f64 {
        J2000_UNIX + f64::from(self.seconds) + f64::from(self.micros) * 1e-6
    }

    /// A key of the product instance: its time in microseconds since J2000.
    pub fn product_key(&self) -> u64 {
        u64::from(self.seconds) * 1_000_000 + u64::from(self.micros)
    }
}

sdrkit_core::choice_enum! {
    /// The data quality flag of a flash or a group (Tables 7.2.1.6.1.4-1 and 7.2.1.6.1.4-2). For a group, the value 1 also
    /// means that the parent flash is abnormal.
    pub enum GlmQuality {
        /// Value 0.
        Good = ("good", "Good"),
        /// Value 1.
        TimeOrder = ("time_order", "Degraded: events out of time order"),
        /// Value 3.
        EventCount = ("event_count", "Degraded: too many events"),
        /// Value 5.
        Duration = ("duration", "Degraded: duration too long"),
    }
}

/// A flag value: `Ok(None)` for the fill value, `Err` for a value without a meaning.
fn quality(raw: u16) -> Result<Option<GlmQuality>, ()> {
    match raw {
        0 => Ok(Some(GlmQuality::Good)),
        1 => Ok(Some(GlmQuality::TimeOrder)),
        3 => Ok(Some(GlmQuality::EventCount)),
        5 => Ok(Some(GlmQuality::Duration)),
        FILL => Ok(None),
        _ => Err(()),
    }
}

/// An area or energy value times `scale`: `Ok(None)` for the fill value, `Err` above the valid range.
fn scaled(raw: u16, scale: f64) -> Result<Option<f64>, ()> {
    match raw {
        FILL => Ok(None),
        r if r > VALID_MAX => Err(()),
        r => Ok(Some(f64::from(r) * scale)),
    }
}

/// A time offset in seconds after the product time.
pub fn time_offset(raw: u16) -> f64 {
    f64::from(raw) * TIME_SCALE + TIME_OFFSET
}

/// A longitude below -180 degrees moved into -180 to 180.
fn normalised(lon: f64) -> f64 {
    if lon < -180.0 {
        lon + 360.0
    } else {
        lon
    }
}

/// A centroid in degrees, or `None` when it is not finite or outside the field of view of every position.
fn centroid(lat: f32, lon: f32) -> Option<(f64, f64)> {
    let (lat, lon) = (f64::from(lat), f64::from(lon));
    let ok = lat.is_finite()
        && lon.is_finite()
        && lat.abs() <= LAT_BOUND
        && (LON_WEST_BOUND..=180.0).contains(&lon);
    ok.then(|| (lat, normalised(lon)))
}

/// A flash (Table 7.2.1.6.1.1). Times are seconds since 1970 (UTC).
#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    /// Product-unique flash identifier.
    pub id: u16,
    /// Time of the first constituent event.
    pub first: f64,
    /// Time of the last constituent event.
    pub last: f64,
    /// Frame time of the first constituent event.
    pub frame_first: f64,
    /// Frame time of the last constituent event.
    pub frame_last: f64,
    /// Latitude of the energy-weighted centroid, degrees north.
    pub lat: f64,
    /// Longitude of the energy-weighted centroid, degrees east (-180 to 180).
    pub lon: f64,
    /// Area covered, km² (`None`: fill value).
    pub area_km2: Option<f64>,
    /// Radiant energy, J (`None`: fill value).
    pub energy_j: Option<f64>,
    /// Data quality (`None`: fill value).
    pub quality: Option<GlmQuality>,
}

/// A group (Table 7.2.1.6.1.2). Times are seconds since 1970 (UTC).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Product-unique group identifier.
    pub id: u32,
    /// Mean time of the constituent events.
    pub time: f64,
    /// Mean frame time of the constituent events.
    pub frame_time: f64,
    /// Latitude of the energy-weighted centroid, degrees north.
    pub lat: f64,
    /// Longitude of the energy-weighted centroid, degrees east (-180 to 180).
    pub lon: f64,
    /// Area covered, km² (`None`: fill value).
    pub area_km2: Option<f64>,
    /// Radiant energy, J (`None`: fill value).
    pub energy_j: Option<f64>,
    /// The flash the group belongs to.
    pub parent: u16,
    /// Data quality (`None`: fill value).
    pub quality: Option<GlmQuality>,
}

/// An event (Table 7.2.1.6.1.3). The time is seconds since 1970 (UTC).
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Product-unique event identifier.
    pub id: u32,
    /// Time of occurrence.
    pub time: f64,
    /// Latitude, degrees north.
    pub lat: f64,
    /// Longitude, degrees east (-180 to 180); `None` until a metadata payload gave its offset.
    pub lon: Option<f64>,
    /// Radiant energy, J (`None`: fill value).
    pub energy_j: Option<f64>,
    /// The group the event belongs to.
    pub parent: u32,
}

/// The records of one data unit that passed the range checks, and the number that did not.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit<T> {
    /// Records with valid values.
    pub records: Vec<T>,
    /// Records with an impossible value.
    pub rejected: u64,
}

impl<T> Default for Unit<T> {
    fn default() -> Self {
        Unit {
            records: Vec::new(),
            rejected: 0,
        }
    }
}

/// The data unit length does not equal the count times the record size plus 8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadLength;

/// The records of `unit`, `size` bytes each, after the count; [`BadLength`] if the length does not match the count.
fn records(unit: &[u8], size: usize) -> Result<std::slice::ChunksExact<'_, u8>, BadLength> {
    if unit.len() < COUNT_BYTES {
        return Err(BadLength);
    }
    let mut c = [0u8; 8];
    c.copy_from_slice(&unit[..COUNT_BYTES]);
    let n = u64::from_le_bytes(c);
    let want = usize::try_from(n)
        .ok()
        .and_then(|n| n.checked_mul(size))
        .and_then(|b| b.checked_add(COUNT_BYTES));
    if want != Some(unit.len()) {
        return Err(BadLength);
    }
    Ok(unit[COUNT_BYTES..].chunks_exact(size))
}

/// The flashes of a data unit of APID 302 whose product time is `product` (seconds since 1970).
///
/// # Errors
///
/// [`BadLength`] if the unit length does not match its count.
pub fn decode_flashes(unit: &[u8], product: f64) -> Result<Unit<Flash>, BadLength> {
    let mut u = Unit::default();
    for r in records(unit, FLASH_BYTES)? {
        let f = (|| {
            let (lat, lon) = centroid(lef32(r, 10), lef32(r, 14))?;
            let (t_first, t_last) = (le16(r, 2), le16(r, 4));
            if t_first > t_last {
                return None;
            }
            Some(Flash {
                id: le16(r, 0),
                first: product + time_offset(t_first),
                last: product + time_offset(t_last),
                frame_first: product + time_offset(le16(r, 6)),
                frame_last: product + time_offset(le16(r, 8)),
                lat,
                lon,
                area_km2: scaled(le16(r, 18), AREA_SCALE).ok()?,
                energy_j: scaled(le16(r, 20), ENERGY_SCALE).ok()?,
                quality: quality(le16(r, 22)).ok()?,
            })
        })();
        match f {
            Some(f) => u.records.push(f),
            None => u.rejected += 1,
        }
    }
    Ok(u)
}

/// The groups of a data unit of APID 303. Bytes 24 to 27 of each record are not described by the guide and are not read.
///
/// # Errors
///
/// [`BadLength`] if the unit length does not match its count.
pub fn decode_groups(unit: &[u8], product: f64) -> Result<Unit<Group>, BadLength> {
    let mut u = Unit::default();
    for r in records(unit, GROUP_BYTES)? {
        let g = (|| {
            let (lat, lon) = centroid(lef32(r, 8), lef32(r, 12))?;
            Some(Group {
                id: le32(r, 0),
                time: product + time_offset(le16(r, 4)),
                frame_time: product + time_offset(le16(r, 6)),
                lat,
                lon,
                area_km2: scaled(le16(r, 16), AREA_SCALE).ok()?,
                energy_j: scaled(le16(r, 18), ENERGY_SCALE).ok()?,
                parent: le16(r, 20),
                quality: quality(le16(r, 22)).ok()?,
            })
        })();
        match g {
            Some(g) => u.records.push(g),
            None => u.rejected += 1,
        }
    }
    Ok(u)
}

/// The events of a data unit of APID 301. `lon_offset` is the `add_offset` of `event_lon` (the west bound of the field of
/// view of the satellite's position); without it the longitudes are `None`. Every 16-bit value is a valid event value.
///
/// # Errors
///
/// [`BadLength`] if the unit length does not match its count.
pub fn decode_events(
    unit: &[u8],
    product: f64,
    lon_offset: Option<f64>,
) -> Result<Unit<Event>, BadLength> {
    let mut u = Unit::default();
    for r in records(unit, EVENT_BYTES)? {
        let energy = le16(r, 10);
        u.records.push(Event {
            id: le32(r, 0),
            time: product + time_offset(le16(r, 4)),
            lat: f64::from(le16(r, 6)) * EVENT_DEG_SCALE + EVENT_LAT_OFFSET,
            lon: lon_offset.map(|o| normalised(f64::from(le16(r, 8)) * EVENT_DEG_SCALE + o)),
            energy_j: (energy != FILL).then(|| f64::from(energy) * ENERGY_SCALE),
            parent: le32(r, 12),
        });
    }
    Ok(u)
}

/// The `add_offset` attribute of the variable `event_lon` in an NcML metadata text, if it holds a number.
pub fn event_lon_offset(ncml: &[u8]) -> Option<f64> {
    let t = std::str::from_utf8(ncml).ok()?;
    let var = &t[t.find("name=\"event_lon\"")?..];
    let var = &var[..var.find("</variable>").unwrap_or(var.len())];
    let attr = &var[var.find("name=\"add_offset\"")?..];
    let v = &attr[attr.find("value=\"")? + 7..];
    v[..v.find('"')?].trim().parse().ok()
}

/// Counters of the GLM data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlmStats {
    /// Complete GLM payloads (APIDs 300 to 303).
    pub payloads: u64,
    /// Payloads dropped because a packet of them was lost or damaged.
    pub dropped: u64,
    /// Packets missing from the sequence counts of the GLM APIDs.
    pub packets_lost: u64,
    /// Payloads compressed (JPEG 2000, SZIP or another value), saved as files and not decoded.
    pub not_decoded: u64,
    /// Flash and event units whose length does not match their count (saved as files).
    pub bad_length: u64,
    /// Group units whose length does not match their count (saved as files).
    pub group_bad_length: u64,
    /// Flashes taken.
    pub flashes: u64,
    /// Groups taken.
    pub groups: u64,
    /// Events taken.
    pub events: u64,
    /// Records with an impossible value.
    pub rejected: u64,
}

/// A flash with the key of its product.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashEntry {
    /// The product key ([`GenericHeader::product_key`]).
    pub product: u64,
    /// The flash.
    pub flash: Flash,
    /// Groups seen with this flash as parent.
    pub groups: u32,
    /// Events seen in those groups.
    pub events: u32,
}

/// A group with the key of its product.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupEntry {
    /// The product key.
    pub product: u64,
    /// The group.
    pub group: Group,
    /// Events seen with this group as parent.
    pub events: u32,
}

/// The parent links of one product instance.
#[derive(Clone, Debug, Default)]
struct Links {
    product: u64,
    flash_groups: HashMap<u16, u32>,
    flash_events: HashMap<u16, u32>,
    group_parent: HashMap<u32, u16>,
    group_events: HashMap<u32, u32>,
}

fn bump<K: std::hash::Hash + Eq>(m: &mut HashMap<K, u32>, k: K, n: u32) {
    if m.len() < MAX_LINKS || m.contains_key(&k) {
        *m.entry(k).or_default() += n;
    }
}

/// What the GLM store gives out at each report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlmReport {
    /// The counters.
    pub stats: GlmStats,
    /// The newest flashes, oldest first, with their link counts.
    pub flashes: Vec<FlashEntry>,
    /// The newest groups, oldest first, with their link counts.
    pub groups: Vec<GroupEntry>,
    /// The time of the newest flash (first event), seconds since 1970.
    pub newest_flash: Option<f64>,
    /// Flashes whose first event lies in the 60 s up to the newest flash.
    pub rate_per_minute: f64,
}

/// What the caller does with a GLM payload after [`Glm::take`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    /// The records were decoded.
    Decoded,
    /// The payload is metadata or was not decoded: it leaves as a file.
    File,
}

/// The GLM store: counters, the newest flashes and groups, the parent links and the flash rate. Every part is bounded.
#[derive(Clone, Debug)]
pub struct Glm {
    /// The counters.
    pub stats: GlmStats,
    flashes: VecDeque<FlashEntry>,
    groups: VecDeque<GroupEntry>,
    links: VecDeque<Links>,
    /// Flashes per second of flash time, at index second mod 64: (second, count).
    rate: [(i64, u32); 64],
    /// The time of the newest flash, seconds since 1970.
    pub newest_flash: Option<f64>,
    /// The `add_offset` of `event_lon` from the newest metadata payload.
    pub event_lon_offset: Option<f64>,
}

impl Default for Glm {
    fn default() -> Self {
        Glm {
            stats: GlmStats::default(),
            flashes: VecDeque::new(),
            groups: VecDeque::new(),
            links: VecDeque::new(),
            rate: [(i64::MIN, 0); 64],
            newest_flash: None,
            event_lon_offset: None,
        }
    }
}

/// True for the APIDs of GLM (300 to 303).
pub fn is_glm(apid: u16) -> bool {
    (APID_METADATA..=APID_GROUPS).contains(&apid)
}

impl Glm {
    fn links(&mut self, product: u64) -> &mut Links {
        let i = match self.links.iter().position(|l| l.product == product) {
            Some(i) => i,
            None => {
                if self.links.len() == LINK_PRODUCTS {
                    if let Some(old) = self.links.pop_front() {
                        self.freeze(&old);
                    }
                }
                self.links.push_back(Links {
                    product,
                    ..Links::default()
                });
                self.links.len() - 1
            }
        };
        &mut self.links[i]
    }

    /// Copies the link counts of a product whose links are let go onto its kept rows.
    fn freeze(&mut self, l: &Links) {
        for e in self.flashes.iter_mut().filter(|e| e.product == l.product) {
            e.groups = l.flash_groups.get(&e.flash.id).copied().unwrap_or(0);
            e.events = l.flash_events.get(&e.flash.id).copied().unwrap_or(0);
        }
        for e in self.groups.iter_mut().filter(|e| e.product == l.product) {
            e.events = l.group_events.get(&e.group.id).copied().unwrap_or(0);
        }
    }

    fn count_rate(&mut self, t: f64) {
        let s = t.floor() as i64;
        let b = &mut self.rate[s.rem_euclid(64) as usize];
        if b.0 == s {
            b.1 += 1;
        } else if s > b.0 {
            *b = (s, 1);
        }
    }

    /// Takes one complete generic payload (header included) of GLM APID `apid`.
    pub fn take(&mut self, apid: u16, d: &[u8]) -> Taken {
        let Some(h) = GenericHeader::parse(d) else {
            self.stats.bad_length += 1;
            return Taken::File;
        };
        self.stats.payloads += 1;
        if h.compression != 0 {
            self.stats.not_decoded += 1;
            return Taken::File;
        }
        let unit = &d[GENERIC_HEADER..];
        let (pt, key) = (h.product_time(), h.product_key());
        match apid {
            APID_FLASHES => match decode_flashes(unit, pt) {
                Ok(u) => {
                    self.stats.rejected += u.rejected;
                    self.stats.flashes += u.records.len() as u64;
                    self.links(key);
                    for f in u.records {
                        self.count_rate(f.first);
                        self.newest_flash =
                            Some(self.newest_flash.map_or(f.first, |n| n.max(f.first)));
                        if self.flashes.len() == ROWS {
                            self.flashes.pop_front();
                        }
                        self.flashes.push_back(FlashEntry {
                            product: key,
                            flash: f,
                            groups: 0,
                            events: 0,
                        });
                    }
                }
                Err(BadLength) => {
                    self.stats.bad_length += 1;
                    return Taken::File;
                }
            },
            APID_GROUPS => match decode_groups(unit, pt) {
                Ok(u) => {
                    self.stats.rejected += u.rejected;
                    self.stats.groups += u.records.len() as u64;
                    for g in u.records {
                        let l = self.links(key);
                        if l.group_parent.len() < MAX_LINKS {
                            l.group_parent.insert(g.id, g.parent);
                        }
                        bump(&mut l.flash_groups, g.parent, 1);
                        let pending = l.group_events.get(&g.id).copied().unwrap_or(0);
                        if pending > 0 {
                            bump(&mut l.flash_events, g.parent, pending);
                        }
                        if self.groups.len() == ROWS {
                            self.groups.pop_front();
                        }
                        self.groups.push_back(GroupEntry {
                            product: key,
                            group: g,
                            events: 0,
                        });
                    }
                }
                Err(BadLength) => {
                    self.stats.group_bad_length += 1;
                    return Taken::File;
                }
            },
            APID_EVENTS => match decode_events(unit, pt, self.event_lon_offset) {
                Ok(u) => {
                    self.stats.events += u.records.len() as u64;
                    let l = self.links(key);
                    for e in u.records {
                        bump(&mut l.group_events, e.parent, 1);
                        if let Some(&f) = l.group_parent.get(&e.parent) {
                            bump(&mut l.flash_events, f, 1);
                        }
                    }
                }
                Err(BadLength) => {
                    self.stats.bad_length += 1;
                    return Taken::File;
                }
            },
            _ => {
                if let Some(o) = event_lon_offset(unit) {
                    self.event_lon_offset = Some(o);
                }
                return Taken::File;
            }
        }
        Taken::Decoded
    }

    fn link_counts(&self, product: u64) -> Option<&Links> {
        self.links.iter().find(|l| l.product == product)
    }

    /// The counters, the newest flashes and groups with their link counts, the newest flash time and the rate. Rows of a
    /// product whose links are kept take the counts from them; older rows keep the counts copied when the links went.
    pub fn report(&self) -> GlmReport {
        let flashes = self
            .flashes
            .iter()
            .map(|e| match self.link_counts(e.product) {
                Some(l) => FlashEntry {
                    groups: l.flash_groups.get(&e.flash.id).copied().unwrap_or(0),
                    events: l.flash_events.get(&e.flash.id).copied().unwrap_or(0),
                    ..e.clone()
                },
                None => e.clone(),
            })
            .collect();
        let groups = self
            .groups
            .iter()
            .map(|e| match self.link_counts(e.product) {
                Some(l) => GroupEntry {
                    events: l.group_events.get(&e.group.id).copied().unwrap_or(0),
                    ..e.clone()
                },
                None => e.clone(),
            })
            .collect();
        let rate_per_minute = self.newest_flash.map_or(0.0, |n| {
            let top = n.floor() as i64;
            self.rate
                .iter()
                .filter(|(s, _)| *s > top - 60 && *s <= top)
                .map(|(_, c)| f64::from(*c))
                .sum()
        });
        GlmReport {
            stats: self.stats.clone(),
            flashes,
            groups,
            newest_flash: self.newest_flash,
            rate_per_minute,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal, written from the record tables of the guide.

/// Product time of the first test product: seconds since J2000.
pub const TEST_PRODUCT_SECONDS: u32 = 815_000_000;
/// Product time of the test products: microseconds of the second.
pub const TEST_PRODUCT_MICROS: u32 = 250_000;
/// Seconds between test products (a product spans about 20.5 s).
pub const TEST_PRODUCT_STEP: u32 = 20;
/// The `add_offset` of `event_lon` in the test metadata: the west bound of GOES-East (Table 7.2.1.1-1).
pub const TEST_EVENT_LON_OFFSET: f64 = -141.56;

/// A flash record as sent (Table 7.2.1.6.1.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawFlash {
    /// `flash_id`.
    pub id: u16,
    /// `flash_time_offset_of_first_event`.
    pub t_first: u16,
    /// `flash_time_offset_of_last_event`.
    pub t_last: u16,
    /// `flash_frame_time_offset_of_first_event`.
    pub ft_first: u16,
    /// `flash_frame_time_offset_of_last_event`.
    pub ft_last: u16,
    /// `flash_lat`.
    pub lat: f32,
    /// `flash_lon`.
    pub lon: f32,
    /// `flash_area`.
    pub area: u16,
    /// `flash_energy`.
    pub energy: u16,
    /// `flash_quality_flag`.
    pub quality: u16,
}

/// A group record as sent (Table 7.2.1.6.1.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawGroup {
    /// `group_id`.
    pub id: u32,
    /// `group_time_offset`.
    pub t: u16,
    /// `group_frame_time_offset`.
    pub ft: u16,
    /// `group_lat`.
    pub lat: f32,
    /// `group_lon`.
    pub lon: f32,
    /// `group_area`.
    pub area: u16,
    /// `group_energy`.
    pub energy: u16,
    /// `group_parent_flash_id`.
    pub parent: u16,
    /// `group_quality_flag`.
    pub quality: u16,
}

/// An event record as sent (Table 7.2.1.6.1.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawEvent {
    /// `event_id`.
    pub id: u32,
    /// `event_time_offset`.
    pub t: u16,
    /// `event_lat`.
    pub lat: u16,
    /// `event_lon`.
    pub lon: u16,
    /// `event_energy`.
    pub energy: u16,
    /// `event_parent_group_id`.
    pub parent: u32,
}

const fn flash(
    id: u16,
    t: [u16; 4],
    lat: f32,
    lon: f32,
    area: u16,
    energy: u16,
    quality: u16,
) -> RawFlash {
    RawFlash {
        id,
        t_first: t[0],
        t_last: t[1],
        ft_first: t[2],
        ft_last: t[3],
        lat,
        lon,
        area,
        energy,
        quality,
    }
}

#[allow(clippy::too_many_arguments)]
const fn group(
    id: u32,
    t: u16,
    ft: u16,
    lat: f32,
    lon: f32,
    area: u16,
    energy: u16,
    parent: u16,
    quality: u16,
) -> RawGroup {
    RawGroup {
        id,
        t,
        ft,
        lat,
        lon,
        area,
        energy,
        parent,
        quality,
    }
}

/// The flashes of each test product: three flashes with 2, 1 and 3 groups.
pub const TEST_FLASHES: [RawFlash; 3] = [
    flash(
        1,
        [20_000, 20_524, 19_990, 20_520],
        29.5,
        -95.25,
        100,
        6553,
        0,
    ),
    flash(
        2,
        [30_000, 30_262, 29_995, 30_260],
        -12.75,
        -60.5,
        52,
        1000,
        1,
    ),
    flash(
        3,
        [40_000, 41_000, 39_990, 40_995],
        5.0,
        -40.0,
        FILL,
        300,
        3,
    ),
];

/// The flashes of the second flash unit of each test product, whose last packet is not sent.
pub const TEST_LOST_FLASHES: [RawFlash; 2] = [
    flash(4, [42_000, 42_100, 41_990, 42_095], 10.0, -50.0, 20, 50, 0),
    flash(5, [43_000, 43_100, 42_990, 43_095], 11.0, -51.0, 20, 50, 0),
];

/// The groups of each test product.
pub const TEST_GROUPS: [RawGroup; 6] = [
    group(101, 20_000, 19_990, 29.5, -95.25, 40, 3000, 1, 0),
    group(102, 20_524, 20_520, 29.25, -95.5, 60, 3553, 1, 0),
    group(103, 30_100, 30_095, -12.75, -60.5, 52, 1000, 2, 1),
    group(104, 40_000, 39_990, 5.0, -40.0, FILL, 100, 3, 5),
    group(105, 40_500, 40_495, 5.0, -40.0, FILL, 100, 3, 0),
    group(106, 41_000, 40_995, 5.0, -40.0, FILL, 100, 3, 0),
];

/// The events of each test product: two in each group, event `j` of group `i` with id 1001 + 2i + j.
pub fn test_events() -> Vec<RawEvent> {
    TEST_GROUPS
        .iter()
        .enumerate()
        .flat_map(|(i, g)| {
            (0..2u16).map(move |j| RawEvent {
                id: 1001 + 2 * i as u32 + u32::from(j),
                t: g.t,
                lat: 47_000 + 10 * i as u16 + j,
                lon: 20_000 + 10 * i as u16 + j,
                energy: 1500 + j,
                parent: g.id,
            })
        })
        .collect()
}

/// A flash unit: the count (uint64) then 24 bytes a flash, little endian, at the byte offsets of Table 7.2.1.6.1.1.
pub fn flash_unit(flashes: &[RawFlash]) -> Vec<u8> {
    let mut u = (flashes.len() as u64).to_le_bytes().to_vec();
    for f in flashes {
        let at = u.len();
        u.resize(at + 24, 0);
        let r = &mut u[at..];
        r[0..2].copy_from_slice(&f.id.to_le_bytes());
        r[2..4].copy_from_slice(&f.t_first.to_le_bytes());
        r[4..6].copy_from_slice(&f.t_last.to_le_bytes());
        r[6..8].copy_from_slice(&f.ft_first.to_le_bytes());
        r[8..10].copy_from_slice(&f.ft_last.to_le_bytes());
        r[10..14].copy_from_slice(&f.lat.to_le_bytes());
        r[14..18].copy_from_slice(&f.lon.to_le_bytes());
        r[18..20].copy_from_slice(&f.area.to_le_bytes());
        r[20..22].copy_from_slice(&f.energy.to_le_bytes());
        r[22..24].copy_from_slice(&f.quality.to_le_bytes());
    }
    u
}

/// A group unit: the count then 28 bytes a group at the offsets of Table 7.2.1.6.1.2; the 4 bytes the table does not
/// describe are zero.
pub fn group_unit(groups: &[RawGroup]) -> Vec<u8> {
    let mut u = (groups.len() as u64).to_le_bytes().to_vec();
    for g in groups {
        let at = u.len();
        u.resize(at + 28, 0);
        let r = &mut u[at..];
        r[0..4].copy_from_slice(&g.id.to_le_bytes());
        r[4..6].copy_from_slice(&g.t.to_le_bytes());
        r[6..8].copy_from_slice(&g.ft.to_le_bytes());
        r[8..12].copy_from_slice(&g.lat.to_le_bytes());
        r[12..16].copy_from_slice(&g.lon.to_le_bytes());
        r[16..18].copy_from_slice(&g.area.to_le_bytes());
        r[18..20].copy_from_slice(&g.energy.to_le_bytes());
        r[20..22].copy_from_slice(&g.parent.to_le_bytes());
        r[22..24].copy_from_slice(&g.quality.to_le_bytes());
    }
    u
}

/// An event unit: the count then 16 bytes an event at the offsets of Table 7.2.1.6.1.3.
pub fn event_unit(events: &[RawEvent]) -> Vec<u8> {
    let mut u = (events.len() as u64).to_le_bytes().to_vec();
    for e in events {
        u.extend_from_slice(&e.id.to_le_bytes());
        u.extend_from_slice(&e.t.to_le_bytes());
        u.extend_from_slice(&e.lat.to_le_bytes());
        u.extend_from_slice(&e.lon.to_le_bytes());
        u.extend_from_slice(&e.energy.to_le_bytes());
        u.extend_from_slice(&e.parent.to_le_bytes());
    }
    u
}

/// A generic payload: the 21-byte header of Table 5.3.1-1 (big endian; reserved bytes zero) then `unit`.
pub fn generic(
    compression: u8,
    seconds: u32,
    micros: u32,
    unit_count: u32,
    unit: &[u8],
) -> Vec<u8> {
    let mut p = vec![compression];
    p.extend_from_slice(&seconds.to_be_bytes());
    p.extend_from_slice(&micros.to_be_bytes());
    p.extend_from_slice(&[0; 8]);
    p.extend_from_slice(&unit_count.to_be_bytes());
    p.extend_from_slice(unit);
    p
}

/// The NcML metadata of a test product: the attributes of `event_lon` of Table 7.2.1.6.2 with `lon_offset`.
pub fn test_metadata(lon_offset: f64) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<netcdf xmlns=\"http://www.unidata.ucar.edu/namespaces/netcdf/ncml-2.2\">\n   \
         <variable name=\"event_lon\" type=\"short\" shape=\"number_of_events\">\n      \
         <attribute name=\"_Unsigned\" value=\"true\" type=\"string\"/>\n      \
         <attribute name=\"scale_factor\" value=\"0.00203128\" type=\"float\"/>\n      \
         <attribute name=\"add_offset\" value=\"{lon_offset}\" type=\"float\"/>\n      \
         <attribute name=\"units\" value=\"degrees_east\" type=\"string\"/>\n   </variable>\n</netcdf>\n"
    )
    .into_bytes()
}

/// The GLM packets of the test signal, one product at a time, each APID with its own sequence count (4.5.1).
#[derive(Clone, Debug, Default)]
pub struct GlmTestSource {
    seq: [u16; 4],
    product: u32,
}

impl GlmTestSource {
    /// The product time (seconds since J2000) of the product `n` (from 0).
    pub fn product_seconds(n: u32) -> u32 {
        TEST_PRODUCT_SECONDS + TEST_PRODUCT_STEP * n
    }

    /// Cuts `payload` into packets of at most `cut` data bytes on `apid`; `send_last` false leaves out the last packet
    /// (its sequence count is used all the same).
    fn packets(
        &mut self,
        apid: u16,
        payload: &[u8],
        cut: usize,
        send_last: bool,
        out: &mut Vec<Vec<u8>>,
    ) {
        let seq = &mut self.seq[usize::from(apid - APID_METADATA)];
        let chunks: Vec<&[u8]> = payload.chunks(cut).collect();
        let k = chunks.len();
        for (i, c) in chunks.iter().enumerate() {
            let flags = match (i == 0, i + 1 == k) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            };
            if send_last || i + 1 < k {
                out.push(super::s2::crc_packet(apid, flags, *seq, &[0; 8], c, true));
            }
            *seq = (*seq + 1) & 0x3FFF;
        }
    }

    /// The packets of the next product: the flashes (one packet), the groups (two packets), the events (three packets), a
    /// second flash unit cut in two whose last packet is not sent, and the metadata.
    pub fn next_product(&mut self) -> Vec<Vec<u8>> {
        let s = Self::product_seconds(self.product);
        self.product += 1;
        let us = TEST_PRODUCT_MICROS;
        let mut out = Vec::new();
        let flashes = generic(0, s, us, 0, &flash_unit(&TEST_FLASHES));
        self.packets(APID_FLASHES, &flashes, 1400, true, &mut out);
        let groups = generic(0, s, us, 0, &group_unit(&TEST_GROUPS));
        self.packets(APID_GROUPS, &groups, 100, true, &mut out);
        let events = generic(0, s, us, 0, &event_unit(&test_events()));
        self.packets(APID_EVENTS, &events, 80, true, &mut out);
        let lost = generic(0, s, us, 1, &flash_unit(&TEST_LOST_FLASHES));
        self.packets(APID_FLASHES, &lost, 40, false, &mut out);
        let meta = generic(0, s, us, 0, &test_metadata(TEST_EVENT_LON_OFFSET));
        self.packets(APID_METADATA, &meta, 1400, true, &mut out);
        out
    }
}
