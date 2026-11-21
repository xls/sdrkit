//! The frame log, the link table and the counters that `rc_links` and `mavlink_sik` share: one row for each frame, one for each
//! link (link name and transmitter id), the last position of a link when it gives one.

use crate::typed::{
    HeardLinkRow, LinkFields, LinkFrameRow, MavlinkSikVehicleEvent, RcLinksLinkEvent, FRAMES,
    LINKS, LINK_REMOVE, LINK_ROW, MAVLINK_SIK_VEHICLE, RC_LINKS_LINK,
};
use sdrkit_core::interface::{Fields as _, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{EventDef, Record};
use sdrkit_graph::Item;
use sdrkit_util_geo::Position;
use sdrkit_core::command::CommandError;
use sdrkit_core::{TableId, Values};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Frames kept in the log.
pub const FRAME_CAP: usize = 300;
/// Links kept in the table.
pub const LINK_CAP: usize = 100;
/// A frame within this many seconds of signal makes the state 2.
pub const RECENT_S: f64 = 5.0;
/// Bytes of a frame shown as hex.
pub const HEX_BYTES: usize = 64;

/// One frame of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The link name.
    pub link: String,
    /// The transmitter id as text.
    pub id: String,
    /// What the frame is.
    pub kind: String,
    /// The fields as text.
    pub detail: String,
    /// The first bytes.
    pub bytes: Vec<u8>,
    /// The frequency, Hz (absolute when the radio centre is known).
    pub freq_hz: f64,
    /// The level over the floor, dB.
    pub snr_db: f64,
    /// The payload is encrypted.
    pub encrypted: bool,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// One link of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkRow {
    /// The link name.
    pub link: String,
    /// The transmitter id as text.
    pub id: String,
    /// Frames heard.
    pub frames: u64,
    /// The type of the last frame.
    pub last_kind: String,
    /// The fields of the last frame.
    pub last_detail: String,
    /// The last position, with its altitude in metres.
    pub position: Option<(Position, f64)>,
}

impl Shift for LinkRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Frames decoded.
    pub frames: u64,
    /// Bursts analysed.
    pub bursts: u64,
    /// Bursts without a frame.
    pub without_frame: u64,
}

/// What a stack of this kind keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct LinkHeld {
    /// The links.
    pub links: ObjectTable<String, LinkRow>,
    /// The frame log.
    pub frames: LogRing<FrameRow>,
    /// The counters.
    pub counters: Counters,
}

impl Default for LinkHeld {
    fn default() -> Self {
        LinkHeld {
            links: ObjectTable::new(LINK_CAP),
            frames: LogRing::new(FRAME_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for LinkHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.links.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl Tables for LinkHeld {
    const FORMAT: StateFormat = StateFormat::new("drones.links", 1);
    fn clear(&mut self) {
        self.links.clear();
        self.frames.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.links.adopt(saved.links);
        self.frames.adopt(saved.frames);
        self.counters = saved.counters;
    }
}

impl Shaped for FrameRow {
    type Row = LinkFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&LinkFrameRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            link: &self.link,
            id: &self.id,
            r#type: &self.kind,
            detail: &self.detail,
            hex: &hex(&self.bytes),
            freq_hz: self.freq_hz,
            snr_db: self.snr_db,
            encrypted: self.encrypted,
        });
    }
}

impl Shaped for LinkRow {
    type Row = HeardLinkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HeardLinkRow {
            link: &self.link,
            id: &self.id,
            frames: i64::try_from(self.frames).unwrap_or(i64::MAX),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            last_type: &self.last_kind,
            last_detail: &self.last_detail,
            position: self.position.map(|(p, _)| (p.lat_deg, p.lon_deg)),
            alt_m: self.position.map(|(_, a)| a),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (p, alt) = self.position?;
        Some(Point::new(p, format!("{} {} {alt:.0} m", self.link, self.id)))
    }
}


sdrkit_core::commands! {
    /// The object commands of the link table.
    pub enum LinkCommand {
        /// Forgets one link.
        Remove = LINK_REMOVE.command,
    }
}

/// Bytes as upper-case hex pairs with spaces.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(3 * b.len());
    for (i, x) in b.iter().enumerate() {
        // Writing to a `String` cannot fail.
        let _ = write!(s, "{}{x:02X}", if i > 0 { " " } else { "" });
    }
    s
}

/// A link heard for the first time.
#[derive(Clone, Debug, PartialEq)]
pub struct NewLink {
    /// The link name.
    pub link: String,
    /// The transmitter id.
    pub id: String,
    /// The position, when known.
    pub position: Option<(f64, f64)>,
}

impl NewLink {
    fn rc_event(&self) -> RcLinksLinkEvent<'_> {
        RcLinksLinkEvent {
            source: &self.id,
            link: &self.link,
            family: Family::Aircraft,
        }
    }
    fn vehicle_event(&self) -> MavlinkSikVehicleEvent<'_> {
        MavlinkSikVehicleEvent {
            source: &self.id,
            link: &self.link,
            position: self.position,
            family: Family::Aircraft,
        }
    }
}

/// A new radio-control link, as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct RcLinkSeen(pub NewLink);

impl Item for RcLinkSeen {
    const KIND: &'static str = "rc_links.link";
    fn event_def() -> Option<&'static EventDef> {
        Some(RC_LINKS_LINK.def())
    }
    fn write_fields(&self, f: &mut Record) {
        self.0.rc_event().write(f);
    }
}

/// A new telemetry link with its first position, as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleSeen(pub NewLink);

impl Item for VehicleSeen {
    const KIND: &'static str = "mavlink_sik.vehicle";
    fn event_def() -> Option<&'static EventDef> {
        Some(MAVLINK_SIK_VEHICLE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        self.0.vehicle_event().write(f);
    }
}

/// One decoded frame as the stack hands it to [`LinkHeld::take`].
#[derive(Clone, Debug, PartialEq)]
pub struct Heard<'a> {
    /// Signal time, s.
    pub t: f64,
    /// The link name.
    pub link: &'a str,
    /// The transmitter id.
    pub id: String,
    /// What the frame is.
    pub kind: String,
    /// The fields.
    pub detail: String,
    /// The bytes.
    pub bytes: &'a [u8],
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// The level over the floor, dB.
    pub snr_db: f64,
    /// The payload is encrypted.
    pub encrypted: bool,
    /// A position the frame gives (latitude, longitude, altitude in metres).
    pub position: Option<(f64, f64, f64)>,
}

/// What [`LinkHeld::take`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// The link is new.
    pub new_link: bool,
    /// The link has its first position.
    pub first_position: bool,
}

impl LinkHeld {
    /// Logs one frame and updates its link.
    pub fn take(&mut self, h: &Heard<'_>, at: Stamp) -> Taken {
        self.counters.frames += 1;
        let n = h.bytes.len().min(HEX_BYTES);
        self.frames.push(FrameRow {
            t: h.t,
            wall: at.wall,
            link: h.link.to_string(),
            id: h.id.clone(),
            kind: h.kind.clone(),
            detail: h.detail.clone(),
            bytes: h.bytes[..n].to_vec(),
            freq_hz: h.freq_hz,
            snr_db: h.snr_db,
            encrypted: h.encrypted,
        });
        let key = format!("{}/{}", h.link, h.id);
        let mut taken = Taken::default();
        if let Ok(up) = self.links.upsert(&key, at, || LinkRow {
            link: h.link.to_string(),
            id: h.id.clone(),
            frames: 0,
            last_kind: String::new(),
            last_detail: String::new(),
            position: None,
        }) {
            taken.new_link = up.new;
            let r = up.row;
            r.frames += 1;
            r.last_kind.clone_from(&h.kind);
            r.last_detail.clone_from(&h.detail);
            if let Some((lat, lon, alt)) = h.position {
                let p = Position::new(lat, lon);
                if p.validate().is_ok() {
                    taken.first_position = r.position.is_none();
                    r.position = Some((p, alt));
                }
            }
        }
        taken
    }

    /// Runs the `remove` command on the link table `table`: forgets the link whose row key is the argument `row`, and adds the
    /// key to `removed` so that the next report takes its row out of the table.
    ///
    /// # Errors
    ///
    /// [`CommandError`] for a missing argument or a row that is not there.
    pub fn remove(
        &mut self,
        args: &Values,
        table: TableId,
        removed: &mut Vec<String>,
    ) -> Result<(), CommandError> {
        let row = args.arg(&LINK_ROW)?;
        if self.links.remove(row).is_none() {
            return Err(CommandError::NoSuchRow {
                table,
                key: row.to_owned(),
            });
        }
        removed.push(row.to_owned());
        Ok(())
    }

    /// The result fields: the counters, the state (with a frame `recent`), the span `covered`, the SNR and the note.
    pub fn fields<'a>(
        &self,
        recent: bool,
        covered: &'a str,
        snr_db: Option<f64>,
        note: &'a str,
    ) -> LinkFields<'a> {
        let c = &self.counters;
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        LinkFields {
            frames_ok: n(c.frames),
            bursts: n(c.bursts),
            bursts_without_frame: n(c.without_frame),
            state: if recent {
                2
            } else if c.bursts > 0 {
                1
            } else {
                0
            },
            links_heard: self.links.len() as i64,
            covered,
            snr_db,
            note,
        }
    }

    /// Writes the tables and the map points (when `points`) into `p`.
    pub fn rows(&self, now: f64, points: bool, p: &mut Published) {
        shaped::rows(&self.links, &LINKS, now, p);
        shaped::log_rows(&self.frames, &FRAMES, now, p);
        if points {
            shaped::points(&self.links, "links/", p.results_mut());
        }
    }
}
