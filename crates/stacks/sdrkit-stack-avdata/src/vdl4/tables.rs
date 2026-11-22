//! The VDL4 results: the message list, the aircraft with positions, the ground stations, the channels, the status line and the
//! events. The message list, the aircraft and the ground stations are kept through a restart and in the saved state.

use super::reader::{type_name, Message};
use super::{ChannelState, FrontState};
use crate::report::{self, Address, Heard};
use crate::typed::{
    Vdl4AircraftRow, Vdl4ChannelRow, Vdl4Fields, Vdl4MessageEvent, Vdl4MessageRow, Vdl4StationRow,
    VDL4_AIRCRAFT, VDL4_CHANNELS, VDL4_MESSAGE, VDL4_MESSAGES, VDL4_REMOVE, VDL4_STATIONS,
    VDL4_STATION_ROW,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RateFrom, RunContext,
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

/// Messages kept.
pub const MESSAGE_CAP: usize = 400;
/// Aircraft kept.
pub const AIRCRAFT_CAP: usize = 200;
/// Ground stations kept.
pub const STATION_CAP: usize = 64;

impl Shaped for Heard<Message> {
    type Row = Vdl4MessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = &self.item;
        let t = report::text;
        let mut hex = [0; 6];
        out.put(&Vdl4MessageRow {
            time: self.wall.map(Time),
            signal_s: self.at,
            frequency: Hertz(m.freq_hz),
            level_db: f64::from(m.level_db),
            snr_db: f64::from(m.snr_db),
            address: report::hex6(m.addr, &mut hex),
            address_kind: type_name(m.addr_type),
            message: t(&m.msg_name),
            summary: t(&m.summary),
            dls_text: t(&m.dls_text),
            position: m.position.as_ref().map(|p| (p.lat, p.lon)),
            alt_ft: report::finite(m.position.as_ref().and_then(|p| p.alt_ft)),
        });
    }
}

/// What the aircraft table keeps of the link for each aircraft.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// The channel of the last burst, Hz.
    pub freq_hz: f64,
    /// Messages heard.
    pub messages: u64,
    /// The aircraft's address, six hex digits: the map label.
    pub address: String,
}

impl Shift for Link {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The map point of an aircraft: its last fix, labelled with its address and headed along its track.
fn aircraft_point(row: &Aircraft<Link>) -> Option<Point<'_>> {
    let (_, fix) = row.fix?;
    let p = Point::new(fix.position, row.extra.address.as_str());
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
    /// Messages.
    pub messages: u64,
    /// Channel.
    pub freq_hz: f64,
    /// Position (latitude, longitude).
    pub position: Option<(f64, f64)>,
}

impl Shift for Station {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Station {
    type Row = Vdl4StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Vdl4StationRow {
            address: &self.address,
            count: i64::try_from(self.messages).unwrap_or(i64::MAX),
            frequency: Hertz(self.freq_hz),
            last_heard: columns::last_heard(seen).map(Time),
            position: self.position,
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.position?;
        Some(Point::new(Position::new(lat, lon), self.address.as_str()))
    }
}

impl Shaped for ChannelState {
    type Row = Vdl4ChannelRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        out.put(&Vdl4ChannelRow {
            frequency: Hertz(self.freq_hz),
            in_band: self.in_band,
            found: n(self.found),
            good: n(self.good),
            bad: n(self.bad),
            level_db: f64::from(self.level_db),
            snr_db: f64::from(self.snr_db),
        });
    }
}

/// What is kept across sessions: the messages, the aircraft and the ground stations.
#[derive(Serialize, Deserialize)]
pub struct Store {
    messages: LogRing<Heard<Message>>,
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
    const FORMAT: StateFormat = StateFormat::new("avdata.vdl4", 2);
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
        /// Forget every message, aircraft and ground station.
        Clear = objcmd::CLEAR,
        /// Forget one ground station.
        Remove = VDL4_REMOVE.command,
    }
}

/// The results component (`avdata.vdl4_tables`).
pub struct Tables {
    published: Published,
    store: Store,
    channels: Vec<ChannelState>,
    frames: u64,
    last_good: f64,
    now_s: f64,
    refused: String,
    summary: String,
    events: Vec<Event>,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Tables {
            published: crate::typed::results(super::ID),
            store: Store::default(),
            channels: Vec::new(),
            frames: 0,
            last_good: f64::NEG_INFINITY,
            now_s: 0.0,
            refused: String::new(),
            summary: String::new(),
            events: Vec::new(),
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

impl Tables {
    /// The aircraft, by 24-bit address.
    pub fn aircraft(&self) -> &AircraftTable<Link> {
        &self.store.aircraft
    }

    fn message(&mut self, m: &Message) {
        let t = m.time_s;
        let wall = report::wall(self.live, &self.clock);
        let mut at = Stamp::at(t);
        if let Some(w) = wall {
            at = at.with_wall(w);
        }
        self.last_good = t;
        self.frames += 1;
        self.store.messages.push(Heard {
            at: t,
            wall,
            item: m.clone(),
        });
        if m.addr_type == 1 {
            let known = self.store.aircraft.get(Some(m.addr), "").is_some();
            if known || m.position.is_some() {
                if let Some(a) = self.store.aircraft.heard(Some(m.addr), "", at) {
                    a.extra.messages += 1;
                    a.extra.freq_hz = m.freq_hz;
                    if a.extra.address.is_empty() {
                        let _ = write!(a.extra.address, "{:06X}", m.addr);
                    }
                    if let Some(p) = &m.position {
                        let _ = a.position(t, p.fix(), p.motion());
                    }
                }
            }
        }
        if m.uplink {
            if let Ok(up) = self
                .store
                .stations
                .upsert(&Address(m.addr), at, || Station {
                    address: Address(m.addr).to_string(),
                    ..Station::default()
                })
            {
                up.row.messages += 1;
                up.row.freq_hz = m.freq_hz;
                if let Some(p) = &m.position {
                    up.row.position = Some((p.lat, p.lon));
                }
            }
        }
        let mut hex = [0; 6];
        self.events.push(VDL4_MESSAGE.event(
            t,
            m.freq_hz,
            &Vdl4MessageEvent {
                source: report::hex6(m.addr, &mut hex),
                text: &m.summary,
                snr_db: Some(f64::from(m.snr_db)).filter(|s| s.is_finite()),
                family: Family::Aircraft,
                message: &m.msg_name,
                position: m.position.as_ref().map(|p| (p.lat, p.lon)),
            },
        ));
    }

    fn report(&mut self, st: &FrontState) {
        self.now_s = st.time_s;
        self.channels.clone_from(&st.channels);
        self.refused.clone_from(&st.refused);
        self.publish();
    }

    fn publish(&mut self) {
        let now = self.now_s;
        let any = self.channels.iter().any(|c| c.found > 0);
        let state = if now - self.last_good <= 30.0 {
            2
        } else if any {
            1
        } else {
            0
        };
        let inband = self.channels.iter().filter(|c| c.in_band).count();
        let active = self
            .channels
            .iter()
            .filter(|c| c.good > 0 && now - c.last_heard_s <= 2.0)
            .count();
        let bad: u64 = self.channels.iter().map(|c| c.bad).sum();
        self.summary.clear();
        let _ = if !self.refused.is_empty() {
            write!(self.summary, "VDL4: {}", self.refused)
        } else if inband == 0 {
            write!(
                self.summary,
                "VDL4: no channel inside the band, tune nearer 136.85 MHz"
            )
        } else if state == 0 {
            write!(self.summary, "VDL4: listening to {inband} channels")
        } else {
            write!(
                self.summary,
                "VDL4: {}, {active} of {inband} channels active, {} frames, {bad} bad, {} aircraft, {} ground stations",
                if state == 2 { "decoding" } else { "bursts" },
                self.frames,
                self.store.aircraft.len(),
                self.store.stations.len()
            )
        };
        let snr = self
            .channels
            .iter()
            .filter(|c| c.good > 0)
            .max_by(|a, b| a.last_heard_s.total_cmp(&b.last_heard_s))
            .map(|c| c.snr_db);
        let p = &mut self.published;
        report::status(p.results_mut(), state, snr, None, None, &self.summary);
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        p.put(&Vdl4Fields {
            frames: n(self.frames),
            bad: n(bad),
            aircraft: n(self.store.aircraft.len() as u64),
            ground_stations: n(self.store.stations.len() as u64),
        });
        shaped::log_rows(&self.store.messages, &VDL4_MESSAGES, now, p);
        shaped::rows_by(
            self.store.aircraft.rows(),
            &VDL4_AIRCRAFT,
            p,
            |_| true,
            |r, out| {
                let mut address = [0u8; 7];
                let s = r.record.standard(Some(r.seen), &mut address);
                let x = &r.record.extra;
                out.put(&Vdl4AircraftRow {
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
                    count: i64::try_from(x.messages).unwrap_or(i64::MAX),
                    frequency: Hertz(x.freq_hz),
                    trail_points: i64::try_from(r.record.trail.len()).unwrap_or(i64::MAX),
                });
            },
        );
        shaped::row_points_by(
            self.store.aircraft.rows(),
            VDL4_AIRCRAFT.def(),
            "aircraft/",
            p.results_mut(),
            aircraft_point,
        );
        shaped::rows(&self.store.stations, &VDL4_STATIONS, now, p);
        shaped::row_points(
            &self.store.stations,
            &VDL4_STATIONS,
            "station/",
            p.results_mut(),
        );
        shaped::rows_of(
            &self.channels,
            |c, w| write!(w, "{}", c.freq_hz.round() as u64),
            &VDL4_CHANNELS,
            now,
            p,
        );
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl4_tables",
            vec![
                Port::typed::<Message>("messages"),
                Port::typed::<FrontState>("state"),
            ],
            vec![Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the lists: their times move back by the old signal time.
        if self.configured {
            self.store.shift_times(-self.now_s);
        }
        self.configured = true;
        self.now_s = 0.0;
        self.last_good = f64::NEG_INFINITY;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for m in io.items::<Message>(0) {
            self.message(&m.value);
        }
        for s in io.items::<FrontState>(1) {
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
                self.frames = 0;
            }
            TablesCommand::Remove => {
                let table = self
                    .published
                    .results()
                    .decl()
                    .table_id(VDL4_STATIONS.key)
                    .expect("the declaration holds the ground station table");
                objcmd::remove(
                    &mut self.store.stations,
                    table,
                    &mut self.published,
                    args,
                    &VDL4_STATION_ROW,
                )?;
            }
        }
        self.publish();
        Ok(None)
    }

    fn forget(&mut self, kept: &Kept) {
        self.channels.clear();
        self.last_good = f64::NEG_INFINITY;
        if !kept.table(VDL4_MESSAGES.key) {
            self.store.messages.clear();
        }
        if !kept.table(VDL4_AIRCRAFT.key) {
            self.store.aircraft.clear();
        }
        if !kept.table(VDL4_STATIONS.key) {
            self.store.stations.clear();
        }
        if !kept.field("frames") {
            self.frames = 0;
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
