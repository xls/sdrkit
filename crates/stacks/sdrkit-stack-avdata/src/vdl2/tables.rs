//! The VDL2 results: the frame list, the aircraft (with dead reckoning on the WGS-84 ellipsoid), the ground stations, the
//! channels, the counters, the status line and the events. The frame list, the aircraft and the ground stations are kept through
//! a restart and in the saved state.

use super::avlc::type_name;
use super::{BankState, ChannelState, RxFrame};
use crate::messages::{self, Located};
use crate::report::{self, Address, Heard};
use crate::typed::{
    Vdl2AircraftRow, Vdl2ChannelRow, Vdl2Fields, Vdl2MessageEvent, Vdl2MessageRow, Vdl2StationRow,
    VDL2_AIRCRAFT, VDL2_CHANNELS, VDL2_MESSAGE, VDL2_MESSAGES, VDL2_REMOVE, VDL2_STATIONS,
    VDL2_STATION_ROW,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_proto_aero::aircraft::{Aircraft, AircraftTable};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables as _,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Frames kept.
pub const MESSAGE_CAP: usize = 400;
/// Aircraft kept.
pub const AIRCRAFT_CAP: usize = 200;
/// Ground stations kept.
pub const STATION_CAP: usize = 64;
/// Channels shown.
pub const CHANNEL_CAP: usize = 64;

/// A frame as kept in the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// Channel, Hz.
    pub freq_hz: f64,
    /// Burst level, dBFS.
    pub level_db: f32,
    /// Burst SNR, dB.
    pub snr_db: f32,
    /// Source address and type.
    pub src: (u32, u8),
    /// Destination address and type.
    pub dst: (u32, u8),
    /// The source address, six hex digits.
    pub from: String,
    /// The destination address, six hex digits.
    pub to: String,
    /// I, S or U.
    pub kind: String,
    /// The frame's name.
    pub name: String,
    /// The source's C/R bit.
    pub response: bool,
    /// The deepest layer read.
    pub proto: String,
    /// One line.
    pub summary: String,
    /// Every layer, one line each, joined by " | ".
    pub detail: String,
    /// The ACARS message, when there is one.
    pub acars: Option<messages::Kept>,
    /// The ACARS check sequence passed.
    pub acars_ok: bool,
    /// The position decoded from the frame.
    pub position: Option<Located>,
}

impl Kept {
    fn of(rx: &RxFrame) -> Kept {
        let f = &rx.frame;
        Kept {
            freq_hz: rx.freq_hz,
            level_db: rx.level_db,
            snr_db: rx.snr_db,
            src: (f.src, f.src_type),
            dst: (f.dst, f.dst_type),
            from: format!("{:06X}", f.src),
            to: format!("{:06X}", f.dst),
            kind: f.kind.clone(),
            name: f.name.clone(),
            response: f.response,
            proto: f.proto.clone(),
            summary: f.summary.clone(),
            detail: f.detail.join(" | "),
            acars: f
                .acars
                .as_ref()
                .map(|m| messages::Kept::of(m, rx.freq_hz, rx.level_db)),
            acars_ok: f.acars_crc_ok,
            position: f.position.clone(),
        }
    }
}

impl Shaped for Heard<Kept> {
    type Row = Vdl2MessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let k = &self.item;
        let t = report::text;
        let a = k.acars.as_ref();
        let r = messages::reading_cells(a);
        out.put(&Vdl2MessageRow {
            time: self.wall.map(Time),
            signal_s: self.at,
            frequency: Hertz(k.freq_hz),
            level_db: f64::from(k.level_db),
            snr_db: f64::from(k.snr_db),
            from: &k.from,
            from_type: i64::from(k.src.1),
            to: &k.to,
            to_type: i64::from(k.dst.1),
            frame_kind: t(&k.kind),
            frame: t(&k.name),
            response: k.response,
            protocol: t(&k.proto),
            summary: t(&k.summary),
            registration: a.and_then(|m| t(&m.registration)),
            flight: a.and_then(|m| t(&m.flight)),
            label: a.and_then(|m| t(&m.label)),
            text: a.and_then(|m| t(&m.text)),
            acars_ok: k.acars_ok,
            blocks: a.map(|m| i64::from(m.blocks)),
            reading_kind: r.kind,
            reading_category: r.category,
            cpdlc_imi: r.imi,
            cpdlc_ground: r.ground,
            cpdlc_aircraft: r.aircraft,
            cpdlc_crc_ok: r.crc_ok,
            cpdlc_parsed: r.parsed,
            cpdlc_text: r.text,
            position: k.position.as_ref().map(|p| (p.lat, p.lon)),
            alt_ft: report::finite(k.position.as_ref().and_then(|p| p.alt_ft)),
            detail: t(&k.detail),
        });
    }
}

/// The position of an airborne aircraft `age_s` after a fix at `fix`, moving along `track_deg` at `gs_kt`: on the WGS-84
/// ellipsoid, from 2 s to 45 min after the fix and at 15 kt or more. Otherwise the fix itself.
pub fn dead_reckoning(fix: Position, age_s: f64, track_deg: f64, gs_kt: f64) -> Position {
    if gs_kt >= 15.0 && (2.0..=2700.0).contains(&age_s) {
        fix.destination(track_deg, gs_kt * 1852.0 / 3600.0 * age_s)
    } else {
        fix
    }
}

/// What the aircraft table keeps of the link for each aircraft.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// The channel of the last frame, Hz.
    pub freq_hz: f64,
    /// The level of its last frame, dB.
    pub level_db: f32,
    /// The ground station it last talked to.
    pub ground_station: Option<u32>,
    /// The ground station as its row key, six hex digits.
    pub ground_station_text: String,
    /// The aircraft's address, six hex digits: the map label when the registration is not known.
    pub address: String,
    /// On the ground.
    pub on_ground: bool,
    /// Frames to or from it.
    pub frames: u64,
}

impl Shift for Link {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The position now of an aircraft at signal time `now_s`: its last fix, moved by dead reckoning while it is airborne.
fn estimate(row: &Aircraft<Link>, now_s: f64) -> Option<Position> {
    row.fix.map(|(t0, f)| match (row.gs_kt, row.track_deg) {
        (Some(g), Some(k)) if !row.extra.on_ground => dead_reckoning(f.position, now_s - t0, k, g),
        _ => f.position,
    })
}

/// The map point of an aircraft: its last fix, labelled with its registration or else its address, and headed along
/// its track.
fn aircraft_point(row: &Aircraft<Link>) -> Option<Point<'_>> {
    let (_, fix) = row.fix?;
    let label = if row.registration.is_empty() {
        row.extra.address.as_str()
    } else {
        row.registration.as_str()
    };
    let p = Point::new(fix.position, label);
    Some(match row.track_deg {
        Some(t) => p.heading(t as f32),
        None => p,
    })
}

/// A ground station.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// The address, six hex digits.
    pub address: String,
    /// Type (4 or 5).
    pub typ: u8,
    /// Frames it sent.
    pub frames: u64,
    /// Channel.
    pub freq_hz: f64,
    /// Level.
    pub level_db: f32,
    /// From its GSIF.
    pub position: Option<(f64, f64)>,
    /// Airport coverage.
    pub airports: String,
}

impl Shift for Station {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Station {
    type Row = Vdl2StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Vdl2StationRow {
            address: &self.address,
            kind: type_name(self.typ),
            count: i64::try_from(self.frames).unwrap_or(i64::MAX),
            frequency: Hertz(self.freq_hz),
            level_db: f64::from(self.level_db),
            last_heard: columns::last_heard(seen).map(Time),
            airports: report::text(&self.airports),
            position: self.position,
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.position?;
        Some(Point::new(Position::new(lat, lon), self.address.as_str()))
    }
}

/// A channel row: it is active with a good burst in the last 2 s of signal time `now_s`.
impl Shaped for ChannelState {
    type Row = Vdl2ChannelRow;
    fn row(&self, _seen: Option<&Seen>, now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        out.put(&Vdl2ChannelRow {
            frequency: Hertz(self.freq_hz),
            in_band: self.in_band,
            active: active(self, now_s),
            level_db: f64::from(self.level_db),
            snr_db: f64::from(self.snr_db),
            found: n(self.bursts_found),
            good: n(self.good),
            bad: n(self.bad),
            frames: n(self.frames),
            offset_hz: Hertz(f64::from(self.last_offset_hz)),
        });
    }
}

/// A channel with a good burst in the last 2 s of signal time `now_s`.
fn active(c: &ChannelState, now_s: f64) -> bool {
    c.good > 0 && now_s - c.last_heard_s <= 2.0
}

/// Counters of the current input.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good frames.
    pub frames_ok: u64,
    /// Bad bursts.
    pub bursts_bad: u64,
    /// Octets corrected.
    pub octets_corrected: u64,
    /// Headers that needed their FEC.
    pub headers_fixed: u64,
    /// Frames with a bad FCS.
    pub fcs_bad: u64,
    /// Positions.
    pub positions: u64,
}

/// What is kept across sessions: the frame list, the aircraft and the ground stations.
#[derive(Serialize, Deserialize)]
pub struct Store {
    messages: LogRing<Heard<Kept>>,
    aircraft: AircraftTable<Link>,
    stations: ObjectTable<Address, Station>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            messages: LogRing::new(MESSAGE_CAP),
            aircraft: AircraftTable::new(AIRCRAFT_CAP),
            stations: ObjectTable::new(STATION_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.aircraft.shift_times(by_s);
        self.stations.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.vdl2", 2);
    fn clear(&mut self) {
        self.messages.clear();
        self.aircraft.clear();
        self.stations.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.aircraft.adopt(saved.aircraft);
        self.stations.adopt(saved.stations);
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forget every frame, aircraft and ground station.
        Clear = objcmd::CLEAR,
        /// Forget one ground station.
        Remove = VDL2_REMOVE.command,
    }
}

/// The results component (`avdata.vdl2_tables`).
pub struct Tables {
    published: Published,
    store: Store,
    counters: Counters,
    channels: Vec<ChannelState>,
    events: Vec<Event>,
    last_good_s: f64,
    now_s: f64,
    tune_error_hz: Option<f64>,
    snr_db: Option<f32>,
    burst_seen: bool,
    refused: String,
    summary: String,
    live: bool,
    clock: Clock,
    configured_once: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Tables {
            published: crate::typed::results(super::ID),
            store: Store::default(),
            counters: Counters::default(),
            channels: Vec::new(),
            events: Vec::new(),
            last_good_s: f64::NEG_INFINITY,
            now_s: 0.0,
            tune_error_hz: None,
            snr_db: None,
            burst_seen: false,
            refused: String::new(),
            summary: String::new(),
            live: false,
            clock: Clock::fixed(0.0),
            configured_once: false,
        }
    }
}

impl Tables {
    /// The aircraft heard, by 24-bit address.
    pub fn aircraft(&self) -> &AircraftTable<Link> {
        &self.store.aircraft
    }

    fn frame(&mut self, rx: &RxFrame) {
        if rx.held {
            return;
        }
        let f = &rx.frame;
        let t = rx.time_s;
        let wall = report::wall(self.live, &self.clock);
        let mut at = Stamp::at(t);
        if let Some(w) = wall {
            at = at.with_wall(w);
        }
        self.counters.frames_ok += 1;
        self.last_good_s = t;
        if f.position.is_some() {
            self.counters.positions += 1;
        }
        let kept = Kept::of(rx);
        self.events.push(VDL2_MESSAGE.event(
            t,
            rx.freq_hz,
            &Vdl2MessageEvent {
                source: &kept.from,
                target: &kept.to,
                text: &f.summary,
                snr_db: Some(f64::from(rx.snr_db)).filter(|s| s.is_finite()),
                family: Family::Aircraft,
                from_air: f.src_type == 1,
                protocol: &f.proto,
                frame: &f.name,
            },
        ));
        self.store.messages.push(Heard {
            at: t,
            wall,
            item: kept,
        });
        // Ground stations: the sender of a frame, or its addressee.
        for (addr, typ, sent) in [(f.src, f.src_type, true), (f.dst, f.dst_type, false)] {
            if !(typ == 4 || typ == 5) {
                continue;
            }
            let Ok(up) = self.store.stations.upsert(&Address(addr), at, || Station {
                address: format!("{addr:06X}"),
                typ,
                ..Default::default()
            }) else {
                continue;
            };
            if sent {
                let s = up.row;
                s.frames += 1;
                s.freq_hz = rx.freq_hz;
                s.level_db = rx.level_db;
                if f.name == "GSIF" {
                    if let Some(p) = &f.position {
                        s.position = Some((p.lat, p.lon));
                    }
                    if !f.airports.is_empty() {
                        s.airports.clone_from(&f.airports);
                    }
                }
            }
        }
        // The aircraft: the source when it is one, else the destination.
        let (addr, sent, other) = if f.src_type == 1 {
            (f.src, true, (f.dst, f.dst_type))
        } else if f.dst_type == 1 {
            (f.dst, false, (f.src, f.src_type))
        } else {
            return;
        };
        let reg = f.acars.as_ref().map_or("", |m| m.registration.as_str());
        let Some(a) = self.store.aircraft.heard(Some(addr), reg, at) else {
            return;
        };
        a.extra.frames += 1;
        a.extra.freq_hz = rx.freq_hz;
        if a.extra.address.is_empty() {
            let _ = write!(a.extra.address, "{addr:06X}");
        }
        if (other.1 == 4 || other.1 == 5) && a.extra.ground_station != Some(other.0) {
            a.extra.ground_station = Some(other.0);
            a.extra.ground_station_text.clear();
            let _ = write!(a.extra.ground_station_text, "{:06X}", other.0);
        }
        if sent {
            a.extra.level_db = rx.level_db;
            a.extra.on_ground = f.on_ground;
            if let Some(m) = &f.acars {
                a.message(&m.flight, &m.label, &m.text);
                if let Some(ad) = messages::adsc_of(m) {
                    a.adsc(t, ad);
                }
            }
            if let Some(p) = &f.position {
                if f.acars.as_ref().and_then(messages::adsc_of).is_none() {
                    let _ = a.position(t, p.fix(), p.motion());
                }
            }
            messages::fill_motion(a, 3.0, 7200.0);
        }
    }

    fn report(&mut self, st: &BankState) {
        self.now_s = st.time_s;
        self.tune_error_hz = Some(st.tune_error_hz);
        self.burst_seen |= st.burst_found;
        self.channels.clear();
        self.channels.extend_from_slice(&st.channels);
        let c = &mut self.counters;
        c.bursts_bad = st.channels.iter().map(|c| c.bad).sum();
        c.octets_corrected = st.channels.iter().map(|c| c.corrected).sum();
        c.headers_fixed = st.channels.iter().map(|c| c.headers_fixed).sum();
        c.fcs_bad = st.channels.iter().map(|c| c.fcs_bad).sum();
        self.snr_db = st
            .channels
            .iter()
            .filter(|c| c.good > 0)
            .max_by(|a, b| a.last_heard_s.total_cmp(&b.last_heard_s))
            .map(|c| c.snr_db);
        self.refused.clone_from(&st.refused);
        self.publish();
    }

    fn state(&self) -> u8 {
        if self.now_s - self.last_good_s <= 30.0 {
            2
        } else if self.burst_seen {
            1
        } else {
            0
        }
    }

    fn publish(&mut self) {
        let now = self.now_s;
        let state = self.state();
        let inband = self.channels.iter().filter(|c| c.in_band).count();
        let n_active = self.channels.iter().filter(|c| active(c, now)).count();
        let c = &self.counters;
        self.summary.clear();
        let _ = if !self.refused.is_empty() {
            write!(self.summary, "VDL2: {}", self.refused)
        } else if inband == 0 {
            write!(
                self.summary,
                "VDL2: no channel inside the band, tune nearer 136.85 MHz"
            )
        } else if state == 0 {
            write!(self.summary, "VDL2: listening to {inband} channels")
        } else {
            write!(
                self.summary,
                "VDL2: {}, {n_active} of {inband} channels active, {} frames, {} bad, {} aircraft, {} ground stations",
                if state == 2 { "decoding" } else { "bursts" },
                c.frames_ok,
                c.bursts_bad,
                self.store.aircraft.len(),
                self.store.stations.len()
            )
        };
        let p = &mut self.published;
        report::status(
            p.results_mut(),
            state,
            self.snr_db,
            None,
            self.tune_error_hz,
            &self.summary,
        );
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        p.put(&Vdl2Fields {
            frames_ok: n(c.frames_ok),
            bursts_bad: n(c.bursts_bad),
            octets_corrected: n(c.octets_corrected),
            headers_fixed: n(c.headers_fixed),
            fcs_bad: n(c.fcs_bad),
            positions: n(c.positions),
            tuning_error: self.tune_error_hz.map(Hertz),
            aircraft: n(self.store.aircraft.len() as u64),
            ground_stations: n(self.store.stations.len() as u64),
            messages: n(self.store.messages.len() as u64),
        });
        shaped::log_rows(&self.store.messages, &VDL2_MESSAGES, now, p);
        shaped::rows_by(
            self.store.aircraft.rows(),
            &VDL2_AIRCRAFT,
            p,
            |_| true,
            |r, out| {
                let mut address = [0u8; 7];
                let s = r.record.standard(Some(r.seen), &mut address);
                let x = &r.record.extra;
                out.put(&Vdl2AircraftRow {
                    address: s.address,
                    registration: s.registration,
                    country: s.country,
                    flight: s.flight,
                    label: s.label,
                    messages: s.messages,
                    position: s.position,
                    alt_ft: s.alt_ft,
                    track_deg: s.track_deg,
                    gs_kt: s.gs_kt,
                    vs_fpm: s.vs_fpm,
                    last_heard: s.last_heard.map(Time),
                    count: i64::try_from(x.frames).unwrap_or(i64::MAX),
                    frequency: Hertz(x.freq_hz),
                    ground_station: report::text(&x.ground_station_text),
                    estimate: estimate(r.record, now).map(|p| (p.lat_deg, p.lon_deg)),
                    trail_points: i64::try_from(r.record.trail.len()).unwrap_or(i64::MAX),
                });
            },
        );
        shaped::row_points_by(
            self.store.aircraft.rows(),
            VDL2_AIRCRAFT.def(),
            "aircraft/",
            p.results_mut(),
            aircraft_point,
        );
        shaped::rows(&self.store.stations, &VDL2_STATIONS, now, p);
        shaped::row_points(
            &self.store.stations,
            &VDL2_STATIONS,
            "station/",
            p.results_mut(),
        );
        shaped::rows_of(
            &self.channels,
            |c, w| write!(w, "{}", c.freq_hz.round() as u64),
            &VDL2_CHANNELS,
            now,
            p,
        );
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl2_tables",
            vec![
                Port::typed::<RxFrame>("frames"),
                Port::typed::<BankState>("state"),
            ],
            vec![Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the lists: their times move back by the old signal time.
        if self.configured_once {
            let by = -self.now_s;
            self.store.shift_times(by);
            self.last_good_s += by;
        }
        self.configured_once = true;
        self.now_s = 0.0;
        self.burst_seen = false;
        self.counters = Counters::default();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for f in io.items::<RxFrame>(0) {
            self.frame(&f.value);
        }
        for s in io.items::<BankState>(1) {
            self.report(&s.value);
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.counters = Counters::default();
            }
            TablesCommand::Remove => {
                let table = self
                    .published
                    .results()
                    .decl()
                    .table_id(VDL2_STATIONS.key)
                    .expect("the declaration holds the ground station table");
                objcmd::remove(
                    &mut self.store.stations,
                    table,
                    &mut self.published,
                    args,
                    &VDL2_STATION_ROW,
                )?;
            }
        }
        self.publish();
        Ok(None)
    }

    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.channels.clear();
        self.counters = Counters::default();
        self.last_good_s = f64::NEG_INFINITY;
        self.tune_error_hz = None;
        self.snr_db = None;
        self.burst_seen = false;
        if !kept.table(VDL2_MESSAGES.key) {
            self.store.messages.clear();
        }
        if !kept.table(VDL2_AIRCRAFT.key) {
            self.store.aircraft.clear();
        }
        if !kept.table(VDL2_STATIONS.key) {
            self.store.stations.clear();
        }
        self.publish();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish();
        Ok(())
    }
}
